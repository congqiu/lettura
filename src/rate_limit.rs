//! Global API rate limiting middleware using the `governor` crate.
//!
//! Since Lettura is a single-user self-hosted application, a simple global
//! (not per-IP) rate limiter is sufficient. This prevents accidental abuse
//! or runaway clients from overwhelming the server.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;

/// Shared global rate limiter state.
///
/// Uses `Arc` internally so cloning is cheap and all clones share the same
/// token bucket.
#[derive(Clone)]
pub struct GlobalRateLimit {
    limiter: Arc<DefaultDirectRateLimiter>,
}

impl GlobalRateLimit {
    /// Create a new global rate limiter that allows `requests_per_minute`
    /// requests per minute with a burst capacity equal to the same value.
    pub fn new(requests_per_minute: u32) -> Self {
        let quota = Quota::per_minute(
            NonZeroU32::new(requests_per_minute).expect("requests_per_minute must be > 0"),
        );
        Self {
            limiter: Arc::new(RateLimiter::direct(quota)),
        }
    }
}

/// Axum middleware that enforces the global rate limit.
///
/// Designed for use with `axum::middleware::from_fn_with_state`.
/// Returns `429 Too Many Requests` with a `Retry-After: 60` header when the
/// rate limit is exceeded.
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
        // Should allow at least one request immediately
        assert!(rl.limiter.check().is_ok());
    }

    #[test]
    #[should_panic(expected = "requests_per_minute must be > 0")]
    fn test_global_rate_limit_zero_panics() {
        GlobalRateLimit::new(0);
    }

    #[test]
    fn test_rate_limit_exhaustion() {
        // Create a very tight limiter: 1 request per minute
        let rl = GlobalRateLimit::new(1);
        // First request should succeed
        assert!(rl.limiter.check().is_ok());
        // Second request should be rate-limited
        assert!(rl.limiter.check().is_err());
    }

    #[test]
    fn test_clone_shares_state() {
        let rl = GlobalRateLimit::new(2);
        let rl2 = rl.clone();

        // Consume from the first handle
        assert!(rl.limiter.check().is_ok());
        assert!(rl.limiter.check().is_ok());

        // The clone should see the same exhausted state
        assert!(rl2.limiter.check().is_err());
    }
}
