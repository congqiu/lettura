//! Request ID middleware for distributed tracing.
//!
//! Generates or propagates a unique request ID for each HTTP request,
//! enabling correlation of logs across services and requests.

use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

/// Header name for the request ID.
///
/// `HeaderName::from_static` requires lowercase ASCII bytes.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Request ID extracted from headers or generated.
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

impl RequestId {
    /// Generate a new random request ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Parse from header value.
    pub fn from_header(value: &str) -> Self {
        // Accept any non-empty string as valid request ID
        if value.is_empty() {
            Self::new()
        } else {
            Self(value.to_string())
        }
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Middleware that attaches a request ID to each request.
///
/// If the client provides an `X-Request-Id` header, it will be used.
/// Otherwise, a new UUID is generated.
///
/// The request ID is:
/// - Added to the request extensions for use in handlers
/// - Set in the tracing span for structured logging
/// - Returned in the response header for client correlation
pub async fn request_id_layer(
    mut request: Request,
    next: Next,
) -> Response {
    // Extract or generate request ID
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(RequestId::from_header)
        .unwrap_or_default();

    // Add to tracing span
    tracing::Span::current().record("request_id", &request_id.0);

    // Add to request extensions
    request.extensions_mut().insert(request_id.clone());

    // Process request
    let mut response = next.run(request).await;

    // Add to response headers
    if let Ok(header_value) = (&request_id.0).parse() {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static(REQUEST_ID_HEADER),
            header_value,
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use super::REQUEST_ID_HEADER;

    #[test]
    fn request_id_header_name_is_valid_static_header() {
        let header = axum::http::HeaderName::from_static(REQUEST_ID_HEADER);
        assert_eq!(header.as_str(), "x-request-id");
    }
}
