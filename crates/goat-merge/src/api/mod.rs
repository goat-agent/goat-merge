pub mod auth;
pub mod fault;
pub mod queues;
pub mod setup;
pub mod simulate;
pub mod standing;
pub mod webhook;

use axum::Router;
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::Serialize;
use serde_json::json;

pub use fault::{Fault, Incident, Named, Sent, Wanting};

use crate::engine::Engine;

pub const COOKIE: &str = "goat_merge_session";

pub fn router() -> Router<Engine> {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/me", get(auth::me))
        .route("/api/token", get(auth::token))
        .route("/api/github/webhook", post(webhook::arrived))
        .route("/api/repositories", get(queues::repositories))
        .route("/api/queue/{owner}/{name}/{branch}", get(queues::one_queue))
        .route(
            "/api/queue/{owner}/{name}/{branch}/pause",
            post(queues::pause),
        )
        .route(
            "/api/queue/{owner}/{name}/{branch}/resume",
            post(queues::resume),
        )
        .route("/api/history/{owner}/{name}/{branch}", get(queues::history))
        .route(
            "/api/insights/{owner}/{name}/{branch}",
            get(queues::insights),
        )
        .route("/api/pull/{owner}/{name}/{number}", get(queues::one_pull))
        .route(
            "/api/pull/{owner}/{name}/{number}/enqueue",
            post(queues::enqueue),
        )
        .route(
            "/api/pull/{owner}/{name}/{number}/dequeue",
            post(queues::dequeue),
        )
        .route(
            "/api/pull/{owner}/{name}/{number}/retry",
            post(queues::retry),
        )
        .route(
            "/api/pull/{owner}/{name}/{number}/expedite",
            post(queues::expedite),
        )
        .route(
            "/api/repository/{owner}/{name}/diagnose",
            get(setup::diagnose),
        )
        .route("/api/repository/{owner}/{name}/enable", post(setup::enable))
        .route(
            "/api/repository/{owner}/{name}/disable",
            post(setup::disable),
        )
        .route("/api/events", get(queues::events))
        .route("/api/setup/manifest", get(setup::manifest))
        .route("/api/find/{owner}/{name}", get(simulate::find))
        .route(
            "/api/simulate/{owner}/{name}/{number}",
            post(simulate::simulate),
        )
        .route("/auth/github", get(auth::begin))
        .route("/auth/github/callback", get(auth::callback))
        .route("/auth/logout", post(auth::logout))
        .route("/setup/github", get(setup::app_was_created))
        .route("/setup/install", get(setup::install))
        .method_not_allowed_fallback(nowhere)
}

pub async fn nowhere() -> Fault {
    Fault::NoSuchEndpoint {
        incident: Incident::written_down("looking for a route", &"no route took this request"),
    }
}

async fn health(axum::extract::State(engine): axum::extract::State<Engine>) -> Response {
    Answer(json!({
        "ok": true,
        "set_up": engine.github.is_set_up(),
        "version": env!("CARGO_PKG_VERSION"),
    }))
    .into_response()
}

pub struct Answer<T>(pub T);

impl<T: Serialize> IntoResponse for Answer<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

pub fn cookie_in(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_owned())
}

pub fn set_cookie(name: &str, value: &str, secure: bool, seconds: i64) -> String {
    let mut cookie = format!("{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={seconds}");
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}
