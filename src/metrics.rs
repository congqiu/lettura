use axum::{extract::Request, middleware::Next, response::Response};
use once_cell::sync::Lazy;
use regex::Regex;
use std::time::Instant;

static UUID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap()
});

/// Axum middleware to record HTTP request metrics
pub async fn track_metrics(req: Request, next: Next) -> Response {
    let method = req.method().to_string();
    let path = normalize_path(req.uri().path());
    let start = Instant::now();

    let response = next.run(req).await;

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    metrics::counter!("http_requests_total", "method" => method.clone(), "path" => path.clone(), "status" => status)
        .increment(1);
    metrics::histogram!("http_request_duration_seconds", "method" => method, "path" => path)
        .record(duration);

    response
}

static SLUG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^/p/[^/]+").unwrap());
static FEED_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^/feed/[^/]+").unwrap());

/// Replace dynamic path segments with placeholders to prevent high cardinality.
fn normalize_path(path: &str) -> String {
    let path = SLUG_RE.replace(path, "/p/{slug}").into_owned();
    let path = FEED_RE.replace(&path, "/feed/{token}").into_owned();
    UUID_RE.replace_all(&path, "{id}").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_replaces_uuid() {
        let path = "/api/entries/550e8400-e29b-41d4-a716-446655440000";
        assert_eq!(normalize_path(path), "/api/entries/{id}");
    }

    #[test]
    fn normalize_preserves_non_uuid_path() {
        let path = "/api/tags";
        assert_eq!(normalize_path(path), "/api/tags");
    }

    #[test]
    fn normalize_replaces_multiple_uuids() {
        let path = "/api/entries/550e8400-e29b-41d4-a716-446655440000/tags/660e8400-e29b-41d4-a716-446655440001";
        assert_eq!(
            normalize_path(path),
            "/api/entries/{id}/tags/{id}"
        );
    }
}
