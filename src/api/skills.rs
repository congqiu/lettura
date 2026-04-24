use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;

use crate::state::AppState;

const SKILL_TEMPLATE: &str = include_str!("../../skills/lettura.md");

pub async fn skill_lettura(State(state): State<AppState>) -> impl IntoResponse {
    let base_url = state
        .config
        .public_base_url
        .as_deref()
        .unwrap_or("http://localhost:3330")
        .trim_end_matches('/');
    let server_version = env!("CARGO_PKG_VERSION");

    let rendered = SKILL_TEMPLATE
        .replace("{{BASE_URL}}", base_url)
        .replace("{{SERVER_VERSION}}", server_version);

    (
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        rendered,
    )
}
