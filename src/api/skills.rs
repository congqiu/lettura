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

    let rendered = render_skill_template(SKILL_TEMPLATE, base_url, server_version);

    (
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        rendered,
    )
}

/// Render the skill template by replacing placeholders with actual values.
/// Extracted as a pure function for testability.
fn render_skill_template(template: &str, base_url: &str, server_version: &str) -> String {
    template
        .replace("{{BASE_URL}}", base_url)
        .replace("{{SERVER_VERSION}}", server_version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_replaces_base_url() {
        let template = "Server: {{BASE_URL}}/api/v1";
        let result = render_skill_template(template, "https://example.com", "1.0.0");
        assert_eq!(result, "Server: https://example.com/api/v1");
    }

    #[test]
    fn render_replaces_server_version() {
        let template = "Version: {{SERVER_VERSION}}";
        let result = render_skill_template(template, "http://localhost:3330", "2.5.1");
        assert_eq!(result, "Version: 2.5.1");
    }

    #[test]
    fn render_replaces_both_placeholders() {
        let template = "{{BASE_URL}} v{{SERVER_VERSION}}";
        let result = render_skill_template(template, "https://app.example.com", "3.0.0");
        assert_eq!(result, "https://app.example.com v3.0.0");
    }

    #[test]
    fn render_no_placeholders() {
        let template = "No placeholders here";
        let result = render_skill_template(template, "http://localhost:3330", "1.0.0");
        assert_eq!(result, "No placeholders here");
    }

    #[test]
    fn render_multiple_same_placeholder() {
        let template = "{{BASE_URL}} and {{BASE_URL}} again";
        let result = render_skill_template(template, "https://example.com", "1.0.0");
        assert_eq!(result, "https://example.com and https://example.com again");
    }

    #[test]
    fn render_empty_template() {
        let result = render_skill_template("", "https://example.com", "1.0.0");
        assert_eq!(result, "");
    }

    #[test]
    fn base_url_trailing_slash_stripped_by_caller() {
        // The handler trims trailing slashes from base_url before calling render.
        // This test verifies the render function uses whatever is passed.
        let template = "{{BASE_URL}}/api";
        let result = render_skill_template(template, "https://example.com", "1.0.0");
        assert_eq!(result, "https://example.com/api");
    }
}
