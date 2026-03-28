use axum::http::{header, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "web/dist"]
struct Asset;

pub async fn spa_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Try to serve the exact file
    if let Some(content) = Asset::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_ref())],
            content.data,
        )
            .into_response()
    } else {
        // SPA fallback: serve index.html for non-API routes
        match Asset::get("index.html") {
            Some(content) => {
                Html(String::from_utf8_lossy(&content.data).to_string()).into_response()
            }
            None => (StatusCode::NOT_FOUND, "not found").into_response(),
        }
    }
}
