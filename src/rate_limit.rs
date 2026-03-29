//! API rate limiting middleware using the `governor` crate.
//!
//! Provides two tiers:
//! - Global: 100 req/min for all API endpoints
//! - Auth: 10 req/min for register/login (brute-force protection)

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;

/// Shared rate limiter state.
#[derive(Clone)]
pub struct GlobalRateLimit {
    limiter: Arc<DefaultDirectRateLimiter>,
}

impl GlobalRateLimit {
    pub fn new(requests_per_minute: u32) -> Self {
        let quota = Quota::per_minute(
            NonZeroU32::new(requests_per_minute).expect("requests_per_minute must be > 0"),
        );
        Self {
            limiter: Arc::new(RateLimiter::direct(quota)),
        }
    }
}

/// Axum middleware that enforces a rate limit.
pub async fn rate_limit_middleware(
    State(rate_limit): State<GlobalRateLimit>,
    req: Request,
    next: Next,
) -> Response {
    match rate_limit.limiter.check() {
        Ok(_) => next.run(req).await,
        Err(_) => (
            StatusCode::TOO_MANY_REQUESTS,
            [(axum::http::header::RETRY_AFTER, "60")],
            "rate limit exceeded",
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_rate_limit_creation() {
        let rl = GlobalRateLimit::new(100);
        assert!(rl.limiter.check().is_ok());
    }

    #[test]
    #[should_panic(expected = "requests_per_minute must be > 0")]
    fn test_global_rate_limit_zero_panics() {
        GlobalRateLimit::new(0);
    }

    #[test]
    fn test_rate_limit_exhaustion() {
        let rl = GlobalRateLimit::new(1);
        assert!(rl.limiter.check().is_ok());
        assert!(rl.limiter.check().is_err());
    }

    #[test]
    fn test_clone_shares_state() {
        let rl = GlobalRateLimit::new(2);
        let rl2 = rl.clone();
        assert!(rl.limiter.check().is_ok());
        assert!(rl.limiter.check().is_ok());
        assert!(rl2.limiter.check().is_err());
    }
}
