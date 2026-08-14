use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::Value;

use crate::engine::Engine;
use crate::github::webhook::{DELIVERY_HEADER, EVENT_HEADER, SIGNATURE_HEADER, signature_holds};

pub async fn arrived(State(engine): State<Engine>, headers: HeaderMap, body: Bytes) -> Response {
    let Ok(Some(app)) = engine.store.app_credentials().await else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "goat-merge has no GitHub App yet",
        )
            .into_response();
    };
    let Some(claimed) = header(&headers, SIGNATURE_HEADER) else {
        return (StatusCode::UNAUTHORIZED, "unsigned").into_response();
    };
    if !signature_holds(&app.webhook_secret, &body, &claimed) {
        return (StatusCode::UNAUTHORIZED, "the signature does not match").into_response();
    }

    let event = header(&headers, EVENT_HEADER).unwrap_or_default();
    let delivery = header(&headers, DELIVERY_HEADER).unwrap_or_default();
    let Ok(payload) = serde_json::from_slice::<Value>(&body) else {
        return (StatusCode::BAD_REQUEST, "the body is not JSON").into_response();
    };

    match engine
        .store
        .write_down_delivery(&delivery, &event, &payload)
        .await
    {
        Ok(false) => return (StatusCode::ACCEPTED, "already seen").into_response(),
        Ok(true) => {}
        Err(problem) => {
            tracing::error!(%problem, "a delivery could not be written down");
            return (StatusCode::SERVICE_UNAVAILABLE, "try again").into_response();
        }
    }

    let working = engine.clone();
    tokio::spawn(async move {
        if let Err(problem) = working.react_to(&event, &payload).await {
            tracing::warn!(%problem, %event, "a delivery could not be acted on");
            return;
        }
        if let Err(problem) = working.store.delivery_is_handled(&delivery).await {
            tracing::warn!(%problem, "a delivery could not be marked handled");
        }
    });

    (StatusCode::ACCEPTED, "queued").into_response()
}

fn header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get(name)?.to_str().ok().map(str::to_owned)
}
