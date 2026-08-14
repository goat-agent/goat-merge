use axum::body::Body;
use axum::extract::Request;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../web/dist"]
struct Console;

pub async fn serve(request: Request) -> Response {
    let path = request.uri().path().trim_start_matches('/');
    if path.starts_with("api/") || path.starts_with("auth/") {
        return crate::api::nowhere().await.into_response();
    }
    match Console::get(path) {
        Some(file) => sent(request.uri(), file.data.into_owned()),
        None => match Console::get("index.html") {
            Some(page) => (
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                page.data.into_owned(),
            )
                .into_response(),
            None => (
                StatusCode::NOT_FOUND,
                "the console was not built into this binary. Run `pnpm --dir web build`",
            )
                .into_response(),
        },
    }
}

fn sent(uri: &Uri, body: Vec<u8>) -> Response {
    let kind = mime_guess::from_path(uri.path()).first_or_octet_stream();
    Response::builder()
        .header(header::CONTENT_TYPE, kind.as_ref())
        .body(Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
