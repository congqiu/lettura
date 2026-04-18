use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use governor::{clock::DefaultClock, middleware::NoOpMiddleware, Quota, RateLimiter};
use governor::state::keyed::DefaultKeyedStateStore;
use std::num::NonZeroU32;
use std::sync::Arc;

#[derive(Clone)]
pub struct GlobalRateLimit {
    limiter: Arc<RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock, NoOpMiddleware>>,
}

impl GlobalRateLimit {
    pub fn new(requests_per_minute: u32) -> Self {
        let quota = Quota::per_minute(NonZeroU32::new(requests_per_minute).expect("requests_per_minute must be > 0"));
        Self { limiter: Arc::new(RateLimiter::keyed(quota)) }
    }
}

fn extract_client_ip(request: &Request) -> String {
    if let Some(xff) = request.headers().get("x-forwarded-for") {
        if let Ok(val) = xff.to_str() {
            if let Some(ip) = val.split(',').next() {
                let trimmed = ip.trim();
                if !trimmed.is_empty() { return trimmed.to_string(); }
            }
        }
    }
    if let Some(xri) = request.headers().get("x-real-ip") {
        if let Ok(val) = xri.to_str() {
            let trimmed = val.trim();
            if !trimmed.is_empty() { return trimmed.to_string(); }
        }
    }
    "unknown".to_string()
}

pub async fn rate_limit_middleware(
    State(rate_limit): State<GlobalRateLimit>,
    req: Request,
    next: Next,
) -> Response {
    let ip = extract_client_ip(&req);
    match rate_limit.limiter.check_key(&ip) {
        Ok(_) => next.run(req).await,
        Err(_) => {
            tracing::warn!(ip = %ip, "rate limit exceeded");
            (StatusCode::TOO_MANY_REQUESTS, [(axum::http::header::RETRY_AFTER, "60")], "rate limit exceeded").into_response()
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
}
