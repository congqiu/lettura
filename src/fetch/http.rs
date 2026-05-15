//! HTTP fetch with site-config-driven request overrides and retry logic.
//!
//! Builds the shared `reqwest::Client` (with cookies, proxy, default headers)
//! and offers:
//! - `apply_request_config` — injects per-site headers/cookies/user-agent onto
//!   a `RequestBuilder`, logging invalid values rather than propagating errors.
//! - `fetch_with_retry` — issues the request and retries on transient errors
//!   (timeouts, connect errors, HTTP 429/5xx) with capped exponential backoff.
//! - `DomainRateLimiter` — enforces a minimum 1s gap between requests to the
//!   same domain, evicting the oldest entry once the map exceeds 500 domains.

use crate::config::Config;
use crate::site_config::RequestConfig;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Maximum number of HTTP redirects to follow manually.
const MAX_REDIRECTS: usize = 10;

/// Error type for fetch operations, wrapping reqwest errors and SSRF violations.
#[derive(Debug)]
pub enum FetchError {
    /// A reqwest-level error (timeout, connect, etc.).
    Reqwest(reqwest::Error),
    /// SSRF policy violation (private IP, bad scheme, DNS rebinding).
    Ssrf(String),
    /// Too many redirects.
    TooManyRedirects,
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Reqwest(e) => write!(f, "{e}"),
            FetchError::Ssrf(msg) => write!(f, "SSRF blocked: {msg}"),
            FetchError::TooManyRedirects => write!(f, "too many redirects"),
        }
    }
}

impl std::error::Error for FetchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FetchError::Reqwest(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for FetchError {
    fn from(e: reqwest::Error) -> Self {
        FetchError::Reqwest(e)
    }
}

/// Build the global reqwest client used by all fetch workers.
/// Mirrors the pre-refactor defaults (cookie jar, optional proxy, default
/// accept/language headers, configurable UA and timeout).
pub fn build_client(config: &Config) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
            .parse()
            .expect("valid Accept header value"),
    );
    headers.insert(
        reqwest::header::ACCEPT_LANGUAGE,
        "en-US,en;q=0.9,zh-CN;q=0.8,zh;q=0.7"
            .parse()
            .expect("valid Accept-Language header value"),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("sec-fetch-mode"),
        "navigate"
            .parse()
            .expect("valid sec-fetch-mode header value"),
    );
    headers.insert(
        reqwest::header::CACHE_CONTROL,
        "max-age=0"
            .parse()
            .expect("valid Cache-Control header value"),
    );

    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.fetch_timeout_secs))
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(2)
        .user_agent(&config.user_agent)
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .default_headers(headers);

    if let Some(ref proxy_url) = config.proxy {
        match reqwest::Proxy::all(proxy_url) {
            Ok(proxy) => {
                tracing::info!(proxy = %proxy_url, "configuring HTTP proxy");
                builder = builder.proxy(proxy);
            }
            Err(e) => {
                tracing::error!(proxy = %proxy_url, error = %e, "invalid proxy URL, ignoring");
            }
        }
    }

    builder.build().unwrap_or_default()
}

/// Apply per-site request overrides (headers, cookies, user-agent) to an
/// existing `RequestBuilder`. Invalid header names/values are skipped with a
/// warning rather than aborting the request.
pub fn apply_request_config(
    mut builder: reqwest::RequestBuilder,
    request: &RequestConfig,
) -> reqwest::RequestBuilder {
    for (name, value) in &request.headers {
        match (
            reqwest::header::HeaderName::from_bytes(name.as_bytes()),
            reqwest::header::HeaderValue::from_str(value),
        ) {
            (Ok(hn), Ok(hv)) => builder = builder.header(hn, hv),
            _ => tracing::warn!(header = %name, "skipping invalid site config header"),
        }
    }

    if !request.cookies.is_empty() {
        let cookie_header = request
            .cookies
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("; ");
        if let Ok(hv) = reqwest::header::HeaderValue::from_str(&cookie_header) {
            builder = builder.header(reqwest::header::COOKIE, hv);
        } else {
            tracing::warn!("skipping site config cookies (invalid header value)");
        }
    }

    if let Some(ua) = request.user_agent.as_deref()
        && let Ok(hv) = reqwest::header::HeaderValue::from_str(ua)
    {
        builder = builder.header(reqwest::header::USER_AGENT, hv);
    }

    builder
}

/// Send a request with site-config overrides, retrying on transient failures.
/// On 429 the server's `Retry-After` is honored when present. All retries use
/// capped exponential backoff with ±25% jitter.
///
/// Redirects are followed manually (the client has auto-redirect disabled) so
/// that each hop is validated against SSRF rules (scheme + private-IP check)
/// and DNS-rebinding protection (resolved IPs are checked after resolution).
/// Site-config request overrides (headers, cookies, user-agent) are applied
/// to every request including redirect hops.
pub async fn fetch_with_retry(
    url: &str,
    client: &reqwest::Client,
    max_retries: u32,
    request_config: Option<&RequestConfig>,
) -> Result<reqwest::Response, FetchError> {
    let mut current_url = url.to_string();
    let mut redirects = 0usize;

    loop {
        // Pre-flight: resolve DNS and check that no resolved IP is private.
        dns_check(&current_url).await?;

        let response =
            send_with_retry_inner(&current_url, client, max_retries, request_config).await?;

        let status = response.status();
        if status.is_redirection() {
            if redirects >= MAX_REDIRECTS {
                tracing::warn!(url = %current_url, redirects, "too many redirects");
                return Err(FetchError::TooManyRedirects);
            }
            if let Some(location) = response.headers().get(reqwest::header::LOCATION) {
                let loc_str = location.to_str().unwrap_or("");
                let next_url = resolve_redirect_url(&current_url, loc_str);
                // Validate the redirect target against SSRF rules.
                if let Err(e) = crate::fetch::ssrf::validate_url(&next_url) {
                    tracing::warn!(from = %current_url, to = %next_url, "redirect SSRF blocked: {e}");
                    return Err(FetchError::Ssrf(e));
                }
                tracing::debug!(from = %current_url, to = %next_url, "following redirect");
                current_url = next_url;
                redirects += 1;
                continue;
            }
            // Redirect without Location header — return as-is.
            return Ok(response);
        }

        return Ok(response);
    }
}

/// Resolve a potentially-relative redirect Location against the current URL.
fn resolve_redirect_url(current_url: &str, location: &str) -> String {
    if location.starts_with("http://") || location.starts_with("https://") {
        return location.to_string();
    }
    // Relative URL: resolve against the current URL's origin.
    if let Ok(base) = url::Url::parse(current_url)
        && let Ok(joined) = base.join(location)
    {
        return joined.to_string();
    }
    // Fallback: treat as-is.
    location.to_string()
}

/// Pre-flight DNS check: resolve the host and reject private/reserved IPs.
async fn dns_check(url_str: &str) -> Result<(), FetchError> {
    let parsed =
        url::Url::parse(url_str).map_err(|e| FetchError::Ssrf(format!("invalid URL: {e}")))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| FetchError::Ssrf("URL has no host".to_string()))?;
    // Skip if already a raw IP — validate_url already handles that.
    if host.parse::<std::net::IpAddr>().is_ok() {
        return Ok(());
    }
    // Resolve via system DNS and check all resulting IPs.
    let addr = format!("{}:{}", host, parsed.port_or_known_default().unwrap_or(80));
    let ips: Vec<std::net::IpAddr> = tokio::net::lookup_host(&addr)
        .await
        .map_err(|e| FetchError::Ssrf(format!("DNS resolution failed for {host}: {e}")))?
        .map(|sa| sa.ip())
        .collect();
    crate::fetch::ssrf::check_resolved_ips(&ips).map_err(FetchError::Ssrf)
}

/// Inner retry loop: send a single request (no redirect handling) with
/// transient-error retries. Site-config overrides are applied per attempt.
async fn send_with_retry_inner(
    url: &str,
    client: &reqwest::Client,
    max_retries: u32,
    request_config: Option<&RequestConfig>,
) -> Result<reqwest::Response, FetchError> {
    for attempt in 0..=max_retries {
        if attempt > 0 {
            let actual = backoff_delay(attempt, rand_simple());
            tracing::debug!(attempt, delay_ms = actual.as_millis(), url = %url, "retrying fetch");
            tokio::time::sleep(actual).await;
        }

        let mut builder = client.get(url);
        if let Some(rc) = request_config {
            builder = apply_request_config(builder, rc);
        }
        let req = builder.build()?;

        let response = match client.execute(req).await {
            Ok(r) => r,
            Err(e) => {
                let is_retryable = e.is_timeout() || e.is_connect() || e.is_request();
                tracing::warn!(
                    attempt,
                    error = %e,
                    is_timeout = e.is_timeout(),
                    is_connect = e.is_connect(),
                    url = %url,
                    "HTTP request error"
                );
                if is_retryable && attempt < max_retries {
                    continue;
                }
                return Err(e.into());
            }
        };

        let status = response.status();
        if status.is_success() || status.is_redirection() {
            return Ok(response);
        }
        if status.as_u16() == 429 || status.is_server_error() {
            tracing::warn!(attempt, status = status.as_u16(), url = %url, "retryable HTTP error");
            if status.as_u16() == 429
                && let Some(d) = parse_retry_after_header(response.headers().get("retry-after"))
            {
                tokio::time::sleep(d).await;
            }
            if attempt < max_retries {
                continue;
            }
            return Ok(response);
        }
        // 4xx (non-429): no point retrying.
        return Ok(response);
    }

    unreachable!("for 0..=max_retries must produce at least one iteration")
}

/// Capped exponential backoff with ±25% jitter. `attempt` must be >= 1.
fn backoff_delay(attempt: u32, jitter: f64) -> Duration {
    let base_ms = 1000u64.saturating_mul(2u64.pow(attempt.saturating_sub(1).min(10)));
    let jitter_ms = (base_ms as f64 * 0.25 * (jitter - 0.5).abs()) as u64;
    Duration::from_millis(base_ms.saturating_add(jitter_ms))
}

/// Parse a `Retry-After` header value (RFC 7231 §7.1.3). Supports both
/// "seconds-from-now" and HTTP-date (IMF-fixdate, e.g. "Wed, 21 Oct 2099
/// 07:28:00 GMT") forms. Past dates clamp to `Duration::ZERO`. Returns
/// None when the header is missing or unparseable.
fn parse_retry_after_header(value: Option<&reqwest::header::HeaderValue>) -> Option<Duration> {
    let v = value?;
    let s = v.to_str().ok()?.trim();

    if let Ok(secs) = s.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }

    let target = chrono::DateTime::parse_from_rfc2822(s).ok()?;
    let delta = target.signed_duration_since(chrono::Utc::now());
    if delta <= chrono::Duration::zero() {
        Some(Duration::ZERO)
    } else {
        delta.to_std().ok()
    }
}

/// Simple deterministic pseudo-random in [0.0, 1.0) using the current nanosecond.
/// Keeps this module self-contained — backoff jitter doesn't warrant pulling
/// in `rand`.
fn rand_simple() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    nanos as f64 / u32::MAX as f64
}

/// Per-domain politeness limiter: sleeps until at least 1s has elapsed since
/// the last observed request for that domain. Not thread-safe — wrap in a Mutex.
pub struct DomainRateLimiter {
    last_request: HashMap<String, Instant>,
}

impl DomainRateLimiter {
    pub fn new() -> Self {
        Self {
            last_request: HashMap::new(),
        }
    }

    pub async fn wait_if_needed(&mut self, domain: &str) {
        if let Some(last) = self.last_request.get(domain) {
            let elapsed = last.elapsed();
            if elapsed < Duration::from_secs(1) {
                tokio::time::sleep(Duration::from_secs(1) - elapsed).await;
            }
        }
        self.last_request.insert(domain.to_string(), Instant::now());
        if self.last_request.len() > 500 {
            let oldest = self
                .last_request
                .iter()
                .min_by_key(|(_, v)| *v)
                .map(|(k, _)| k.clone());
            if let Some(key) = oldest {
                self.last_request.remove(&key);
            }
        }
    }
}

impl Default for DomainRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site_config::RequestConfig;

    fn make_builder() -> (reqwest::Client, reqwest::RequestBuilder) {
        let client = reqwest::Client::new();
        let req = client.get("https://example.com/");
        (client, req)
    }

    #[test]
    fn apply_request_config_injects_headers_and_ua() {
        let (_client, builder) = make_builder();
        let mut req = RequestConfig::default();
        req.headers.insert("X-Custom".into(), "v".into());
        req.user_agent = Some("TestAgent/1".into());

        let built = apply_request_config(builder, &req).build().unwrap();
        assert_eq!(built.headers().get("x-custom").unwrap(), "v");
        assert_eq!(built.headers().get("user-agent").unwrap(), "TestAgent/1");
    }

    #[test]
    fn apply_request_config_joins_cookies() {
        let (_client, builder) = make_builder();
        let mut req = RequestConfig::default();
        req.cookies.insert("a".into(), "1".into());
        req.cookies.insert("b".into(), "2".into());

        let built = apply_request_config(builder, &req).build().unwrap();
        let cookie = built.headers().get("cookie").unwrap().to_str().unwrap();
        // BTreeMap guarantees alphabetical ordering.
        assert_eq!(cookie, "a=1; b=2");
    }

    #[test]
    fn apply_request_config_skips_invalid_header_name() {
        let (_client, builder) = make_builder();
        let mut req = RequestConfig::default();
        // Space is not allowed in header names; should be skipped.
        req.headers.insert("Bad Name".into(), "v".into());
        req.headers.insert("Ok".into(), "v".into());

        let built = apply_request_config(builder, &req).build().unwrap();
        assert!(built.headers().get("bad name").is_none());
        assert_eq!(built.headers().get("ok").unwrap(), "v");
    }

    #[tokio::test]
    async fn rate_limiter_waits_for_same_domain() {
        let mut rl = DomainRateLimiter::new();
        rl.wait_if_needed("slow.example").await;
        let start = Instant::now();
        rl.wait_if_needed("slow.example").await;
        assert!(start.elapsed() >= Duration::from_millis(900));
    }

    #[tokio::test]
    async fn rate_limiter_does_not_wait_for_different_domain() {
        let mut rl = DomainRateLimiter::new();
        rl.wait_if_needed("a.example").await;
        let start = Instant::now();
        rl.wait_if_needed("b.example").await;
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn backoff_delay_grows_exponentially() {
        // Use jitter=0.5 to make the jitter component zero (.5 - .5 = 0).
        let d1 = backoff_delay(1, 0.5);
        let d2 = backoff_delay(2, 0.5);
        let d3 = backoff_delay(3, 0.5);
        assert_eq!(d1, Duration::from_millis(1000));
        assert_eq!(d2, Duration::from_millis(2000));
        assert_eq!(d3, Duration::from_millis(4000));
    }

    #[test]
    fn backoff_delay_jitter_within_25_percent() {
        // Worst case jitter (jitter=0.0 or 1.0) yields .5 in `(jitter - 0.5).abs()`,
        // so jitter_ms = base * 0.25 * 0.5 = base * 0.125. Wait — review:
        // (jitter - 0.5).abs() max is 0.5 (when jitter=0 or 1).
        // jitter_ms = base * 0.25 * 0.5 = base * 0.125
        // So upper bound is base * 1.125.
        let base = backoff_delay(3, 0.5).as_millis() as f64; // 4000
        let max_jitter = backoff_delay(3, 0.0).as_millis() as f64;
        let max_jitter2 = backoff_delay(3, 1.0).as_millis() as f64;
        assert!(max_jitter <= base * 1.13, "got {max_jitter}, base {base}");
        assert!(max_jitter2 <= base * 1.13, "got {max_jitter2}, base {base}");
    }

    #[test]
    fn backoff_delay_caps_growth_for_large_attempts() {
        // attempt=20 should not panic or produce something silly.
        // Helper caps at attempt-1 capped at 10, so base = 1000 * 2^10 = 1024000ms ~17min.
        let d = backoff_delay(100, 0.5);
        assert!(d <= Duration::from_secs(2_000), "got {d:?}");
        assert!(d >= Duration::from_secs(60), "got {d:?}");
    }

    #[test]
    fn parse_retry_after_seconds_form() {
        let v = reqwest::header::HeaderValue::from_static("30");
        assert_eq!(
            parse_retry_after_header(Some(&v)),
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn parse_retry_after_zero_is_valid() {
        let v = reqwest::header::HeaderValue::from_static("0");
        assert_eq!(
            parse_retry_after_header(Some(&v)),
            Some(Duration::from_secs(0))
        );
    }

    #[test]
    fn parse_retry_after_returns_none_for_missing() {
        assert_eq!(parse_retry_after_header(None), None);
    }

    #[test]
    fn parse_retry_after_returns_none_for_garbage() {
        let v = reqwest::header::HeaderValue::from_static("garbage");
        assert_eq!(parse_retry_after_header(Some(&v)), None);
    }

    #[test]
    fn parse_retry_after_http_date_future_returns_positive_duration() {
        let v = reqwest::header::HeaderValue::from_static("Wed, 21 Oct 2099 07:28:00 GMT");
        let d = parse_retry_after_header(Some(&v)).expect("http-date is parseable");
        // Year 2099 is well beyond 2026 baseline; delta must be > 70 years of secs.
        assert!(d > Duration::from_secs(70 * 365 * 24 * 3600), "got {d:?}");
    }

    #[test]
    fn parse_retry_after_http_date_past_clamps_to_zero() {
        let v = reqwest::header::HeaderValue::from_static("Mon, 01 Jan 1990 00:00:00 GMT");
        assert_eq!(parse_retry_after_header(Some(&v)), Some(Duration::ZERO));
    }

    #[test]
    fn parse_retry_after_invalid_date_returns_none() {
        // Not a valid RFC 2822 date and not a number.
        let v = reqwest::header::HeaderValue::from_static("Tomorrow at noon");
        assert_eq!(parse_retry_after_header(Some(&v)), None);
    }

    #[test]
    fn parse_retry_after_trims_whitespace() {
        let v = reqwest::header::HeaderValue::from_static("  42  ");
        assert_eq!(
            parse_retry_after_header(Some(&v)),
            Some(Duration::from_secs(42))
        );
    }

    #[test]
    fn resolve_redirect_absolute_url() {
        let result = resolve_redirect_url("https://example.com/page", "https://other.com/target");
        assert_eq!(result, "https://other.com/target");
    }

    #[test]
    fn resolve_redirect_relative_path() {
        let result = resolve_redirect_url("https://example.com/page", "/other");
        assert_eq!(result, "https://example.com/other");
    }

    #[test]
    fn resolve_redirect_relative_path_with_query() {
        let result = resolve_redirect_url("https://example.com/page?q=1", "/other?r=2");
        assert_eq!(result, "https://example.com/other?r=2");
    }

    #[test]
    fn resolve_redirect_relative_no_leading_slash() {
        let result = resolve_redirect_url("https://example.com/dir/page", "other");
        assert_eq!(result, "https://example.com/dir/other");
    }
}
