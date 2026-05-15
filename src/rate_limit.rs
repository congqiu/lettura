use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use governor::state::keyed::DefaultKeyedStateStore;
use governor::{Quota, RateLimiter, clock::DefaultClock, middleware::NoOpMiddleware};
use std::num::NonZeroU32;
use std::sync::Arc;

#[derive(Clone)]
pub struct GlobalRateLimit {
    limiter: Arc<RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock, NoOpMiddleware>>,
    pub trust_proxy: bool,
}

impl GlobalRateLimit {
    pub fn new(requests_per_minute: u32) -> Self {
        Self {
            limiter: Arc::new(RateLimiter::keyed(Quota::per_minute(
                NonZeroU32::new(requests_per_minute).expect("requests_per_minute must be > 0"),
            ))),
            trust_proxy: false,
        }
    }

    pub fn with_trust_proxy(mut self, trust: bool) -> Self {
        self.trust_proxy = trust;
        self
    }
}

pub fn extract_client_ip(request: &Request, trust_proxy: bool) -> String {
    if trust_proxy {
        if let Some(xff) = request.headers().get("x-forwarded-for")
            && let Ok(val) = xff.to_str()
            && let Some(ip) = val.split(',').next()
        {
            let trimmed = ip.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        if let Some(xri) = request.headers().get("x-real-ip")
            && let Ok(val) = xri.to_str()
        {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    // Fallback: use the direct connection IP from Axum's ConnectInfo if available,
    // otherwise "unknown"
    request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub async fn rate_limit_middleware(
    State(rate_limit): State<GlobalRateLimit>,
    req: Request,
    next: Next,
) -> Response {
    let ip = extract_client_ip(&req, rate_limit.trust_proxy);
    match rate_limit.limiter.check_key(&ip) {
        Ok(_) => next.run(req).await,
        Err(_) => {
            tracing::warn!(ip = %ip, "rate limit exceeded");
            (
                StatusCode::TOO_MANY_REQUESTS,
                [(axum::http::header::RETRY_AFTER, "60")],
                "rate limit exceeded",
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_rate_limit_creation() {
        let rl = GlobalRateLimit::new(100);
        assert!(rl.limiter.check_key(&"127.0.0.1".to_string()).is_ok());
    }

    #[test]
    #[should_panic(expected = "requests_per_minute must be > 0")]
    fn test_global_rate_limit_zero_panics() {
        GlobalRateLimit::new(0);
    }

    #[test]
    fn test_rate_limit_exhaustion() {
        let rl = GlobalRateLimit::new(1);
        assert!(rl.limiter.check_key(&"127.0.0.1".to_string()).is_ok());
        assert!(rl.limiter.check_key(&"127.0.0.1".to_string()).is_err());
    }

    #[test]
    fn test_clone_shares_state() {
        let rl = GlobalRateLimit::new(2);
        let rl2 = rl.clone();
        assert!(rl.limiter.check_key(&"127.0.0.1".to_string()).is_ok());
        assert!(rl.limiter.check_key(&"127.0.0.1".to_string()).is_ok());
        assert!(rl2.limiter.check_key(&"127.0.0.1".to_string()).is_err());
    }

    #[test]
    fn test_extract_client_ip_ignores_proxy_headers_when_not_trusted() {
        let req = Request::builder()
            .header("x-forwarded-for", "1.2.3.4")
            .header("x-real-ip", "5.6.7.8")
            .body(axum::body::Body::empty())
            .unwrap();
        // Without ConnectInfo, falls back to "unknown"
        let ip = extract_client_ip(&req, false);
        assert_eq!(ip, "unknown");
    }

    #[test]
    fn test_extract_client_ip_uses_xff_when_trusted() {
        let req = Request::builder()
            .header("x-forwarded-for", "1.2.3.4, 10.0.0.1")
            .body(axum::body::Body::empty())
            .unwrap();
        let ip = extract_client_ip(&req, true);
        assert_eq!(ip, "1.2.3.4");
    }

    #[test]
    fn test_extract_client_ip_uses_xri_when_trusted() {
        let req = Request::builder()
            .header("x-real-ip", "5.6.7.8")
            .body(axum::body::Body::empty())
            .unwrap();
        let ip = extract_client_ip(&req, true);
        assert_eq!(ip, "5.6.7.8");
    }
}
