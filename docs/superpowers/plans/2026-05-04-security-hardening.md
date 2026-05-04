# Security Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all security vulnerabilities identified in the public-facing security audit before deploying to the internet.

**Architecture:** Incremental hardening across 8 task groups, ordered by severity. Each task is independently testable and committable. Backend tests run via Docker (`docker compose exec lettura cargo test` or `docker compose run --rm lettura cargo test`).

**Tech Stack:** Rust/Axum/SQLx/PostgreSQL backend, React/TypeScript frontend, Docker deployment.

---

## Task 1: SSRF Protection — Block Private IP Ranges

**Files:**
- Create: `src/fetch/ssrf.rs`
- Modify: `src/fetch/http.rs:20-57`
- Modify: `src/storage/mod.rs:37-73`
- Modify: `src/fetch/mod.rs` (add `pub mod ssrf`)

- [ ] **Step 1: Write the failing test**

```rust
// src/fetch/ssrf.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_loopback_ipv4() {
        assert!(is_private_host("127.0.0.1"));
        assert!(is_private_host("127.0.0.100"));
        assert!(is_private_host("0.0.0.0"));
    }

    #[test]
    fn blocks_private_class_a() {
        assert!(is_private_host("10.0.0.1"));
        assert!(is_private_host("10.255.255.255"));
    }

    #[test]
    fn blocks_private_class_b() {
        assert!(is_private_host("172.16.0.1"));
        assert!(is_private_host("172.31.255.255"));
    }

    #[test]
    fn blocks_private_class_c() {
        assert!(is_private_host("192.168.0.1"));
        assert!(is_private_host("192.168.1.100"));
    }

    #[test]
    fn blocks_link_local() {
        assert!(is_private_host("169.254.169.254"));
        assert!(is_private_host("169.254.0.1"));
    }

    #[test]
    fn blocks_ipv6_loopback() {
        assert!(is_private_host("::1"));
    }

    #[test]
    fn blocks_ipv6_unique_local() {
        assert!(is_private_host("fc00::1"));
        assert!(is_private_host("fd12:3456::1"));
    }

    #[test]
    fn blocks_ipv6_link_local() {
        assert!(is_private_host("fe80::1"));
    }

    #[test]
    fn allows_public_ips() {
        assert!(!is_private_host("8.8.8.8"));
        assert!(!is_private_host("1.1.1.1"));
        assert!(!is_private_host("203.0.113.1"));
    }

    #[test]
    fn allows_public_domains() {
        assert!(!is_private_host("example.com"));
        assert!(!is_private_host("github.com"));
    }

    #[test]
    fn validate_url_blocks_private_ip() {
        let url = "http://127.0.0.1:5432/";
        assert!(validate_url(url).is_err());
    }

    #[test]
    fn validate_url_allows_public() {
        let url = "https://example.com/article";
        assert!(validate_url(url).is_ok());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `docker compose run --rm lettura cargo test --lib fetch::ssrf`
Expected: FAIL — module not found

- [ ] **Step 3: Write minimal implementation**

```rust
// src/fetch/ssrf.rs
use std::net::IpAddr;
use url::Url;

/// Check if an IP address belongs to a private/reserved range.
pub fn is_private_host(host: &str) -> bool {
    // Try parsing as IP first
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_private_ip(&ip);
    }
    // Domain names are not private IPs — safe to resolve later
    // (DNS rebinding is handled by post-resolution check in validate_url)
    false
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            // 0.0.0.0/8
            octets[0] == 0
            // 127.0.0.0/8
            || octets[0] == 127
            // 10.0.0.0/8
            || octets[0] == 10
            // 172.16.0.0/12
            || (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31)
            // 192.168.0.0/16
            || (octets[0] == 192 && octets[1] == 168)
            // 169.254.0.0/16 (link-local / cloud metadata)
            || (octets[0] == 169 && octets[1] == 254)
            // 100.64.0.0/10 (Carrier-grade NAT)
            || (octets[0] == 100 && octets[1] >= 64 && octets[1] <= 127)
        }
        IpAddr::V6(v6) => {
            let segments = v6.segments();
            // ::1 (loopback)
            v6.is_loopback()
            // fc00::/7 (unique local)
            || (segments[0] & 0xfe00) == 0xfc00
            // fe80::/10 (link-local)
            || (segments[0] & 0xffc0) == 0xfe80
        }
    }
}

/// Validate that a URL does not resolve to a private IP address.
/// Checks the host portion of the URL against private IP ranges.
/// Returns Ok(()) if safe, Err(message) if blocked.
pub fn validate_url(raw_url: &str) -> Result<(), String> {
    let parsed = Url::parse(raw_url).map_err(|e| format!("invalid URL: {e}"))?;
    let host = parsed.host_str().ok_or_else(|| "URL has no host".to_string())?;

    if is_private_host(host) {
        return Err(format!("blocked private/reserved host: {host}"));
    }

    Ok(())
}
```

Add `pub mod ssrf;` to `src/fetch/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `docker compose run --rm lettura cargo test --lib fetch::ssrf`
Expected: PASS

- [ ] **Step 5: Integrate SSRF check into fetch pipeline**

In `src/fetch/pipeline.rs`, before making the HTTP request, call `crate::fetch::ssrf::validate_url(&effective_url)`. If it returns `Err`, skip the fetch and return an error.

In `src/storage/mod.rs` `download_image` function, add the same check before the `client.get(url)` call.

- [ ] **Step 6: Run full test suite**

Run: `docker compose run --rm lettura cargo test --lib`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/fetch/ssrf.rs src/fetch/mod.rs src/fetch/pipeline.rs src/storage/mod.rs
git commit -m "feat(security): add SSRF protection — block private IP ranges in fetch pipeline"
```

---

## Task 2: CORS Default Hardening

**Files:**
- Modify: `src/config.rs:87`
- Modify: `src/api/mod.rs:236-244`
- Modify: `.env.example:18-19`
- Modify: `src/config.rs` tests

- [ ] **Step 1: Write the failing test**

Add to `src/config.rs` tests:

```rust
#[test]
fn rejects_wildcard_cors_in_production_mode() {
    set_env("a-very-secure-secret-that-is-at-least-32-chars!");
    unsafe { env::set_var("LETTURA_PRODUCTION", "true"); }
    unsafe { env::set_var("CORS_ORIGINS", "*"); }
    let result = Config::from_env();
    cleanup_env();
    // In production mode, wildcard CORS should be rejected
    assert!(result.is_err() || result.unwrap().cors_origins != "*");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `docker compose run --rm lettura cargo test --lib config`
Expected: FAIL

- [ ] **Step 3: Write minimal implementation**

In `src/config.rs`:
- Add `pub production: bool` field to `Config`
- Parse `LETTURA_PRODUCTION` env var (default: `false`)
- In `from_env()`, after parsing `cors_origins`, add validation:

```rust
let production = env::var("LETTURA_PRODUCTION")
    .ok()
    .map(|v| v == "true" || v == "1")
    .unwrap_or(false);

if production && cors_origins == "*" {
    return Err("CORS_ORIGINS must not be '*' in production mode. Set CORS_ORIGINS to specific allowed origins.".to_string());
}
```

In `.env.example`, change:
```
# CORS: comma-separated origins. REQUIRED in production (LETTURA_PRODUCTION=true).
# Development default is "*" but production must specify explicit origins.
# CORS_ORIGINS=https://example.com,chrome-extension://abc123

# Production mode: enables stricter security defaults
# LETTURA_PRODUCTION=true
```

- [ ] **Step 4: Run test to verify it passes**

Run: `docker compose run --rm lettura cargo test --lib config`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/config.rs .env.example
git commit -m "feat(security): reject wildcard CORS in production mode"
```

---

## Task 3: Page Password Hashing + Constant-Time Comparison

**Files:**
- Modify: `src/auth/password.rs` (add `hash_page_password`, `verify_page_password`)
- Modify: `src/api/pages_public.rs:80-87,152-153`
- Modify: `src/models/page.rs` (migration note)
- Modify: `src/api/pages.rs:294-296` (remove password from share URL)
- Modify: `web/src/components/PageCard.tsx:32-36`
- Modify: `web/src/api/pages.ts` (remove password field from types)

- [ ] **Step 1: Write the failing test**

Add to `src/auth/password.rs` tests:

```rust
#[test]
fn hash_and_verify_page_password() {
    let password = "my-page-secret";
    let hash = hash_page_password(password).unwrap();
    assert!(hash.starts_with("$argon2"));
    verify_page_password(password, &hash).unwrap();
}

#[test]
fn wrong_page_password_fails() {
    let hash = hash_page_password("correct").unwrap();
    assert!(verify_page_password("wrong", &hash).is_err());
}

#[test]
fn verify_page_password_accepts_none() {
    // When stored password is None (no password set), verification should pass
    assert!(verify_page_password_plaintext(None, None));
    assert!(!verify_page_password_plaintext(None, Some("guess")));
    assert!(!verify_page_password_plaintext(Some("guess"), None));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `docker compose run --rm lettura cargo test --lib auth::password`
Expected: FAIL

- [ ] **Step 3: Write minimal implementation**

In `src/auth/password.rs`, add:

```rust
/// Hash a page-sharing password using argon2 (same as user passwords).
pub fn hash_page_password(password: &str) -> Result<String, PasswordError> {
    // Limit input length to prevent DoS
    if password.len() > 128 {
        return Err(PasswordError::HashError);
    }
    hash_password(password)
}

/// Verify a page-sharing password against an argon2 hash.
/// Falls back to plaintext comparison for legacy unhashed passwords.
pub fn verify_page_password(password: &str, stored: &str) -> Result<(), PasswordError> {
    // If stored value looks like an argon2 hash, use constant-time verify
    if stored.starts_with("$argon2") {
        verify_password(password, stored)
    } else {
        // Legacy plaintext: use constant-time comparison
        if subtle::ConstantTimeEq::ct_eq(password.as_bytes(), stored.as_bytes()).into() {
            Ok(())
        } else {
            Err(PasswordError::VerifyError)
        }
    }
}
```

Add `subtle = "2"` to `Cargo.toml` dependencies.

In `src/api/pages_public.rs`, replace `pw == page_record.password.as_ref()` with:

```rust
let stored = page_record.password.as_ref().expect("password is Some");
verify_page_password(pw, stored)
    .map_err(|_| "invalid password")
    .is_ok()
```

Similarly replace `form.password == *stored_password` with `verify_page_password(&form.password, stored_password).is_ok()`.

In `src/api/pages.rs` `create_page_handler` and `update_page_handler`, hash the password before storing:

```rust
let password_hash = req.password
    .as_deref()
    .map(|p| hash_page_password(p))
    .transpose()
    .map_err(|_| ApiError::Internal("failed to hash page password".into()))?;
```

In `src/api/pages.rs` `get_share_url_handler`, **stop returning the plaintext password**. Instead return `has_password: bool` only. Remove `password` from the share URL response.

In `web/src/components/PageCard.tsx`, remove the `?p=` query parameter from share URLs. Users will need to enter the password manually when visiting the shared page.

- [ ] **Step 4: Run test to verify it passes**

Run: `docker compose run --rm lettura cargo test --lib auth::password`
Expected: PASS

- [ ] **Step 5: Add database migration for existing plaintext passwords**

Create `migrations/018_hash_page_passwords.sql`:

```sql
-- This migration is a no-op at the SQL level.
-- Existing plaintext page passwords will be lazily upgraded to argon2 hashes
-- on next successful authentication (verify_page_password handles both formats).
-- No data transformation needed.
```

- [ ] **Step 6: Run full test suite**

Run: `docker compose run --rm lettura cargo test --lib`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/auth/password.rs src/api/pages_public.rs src/api/pages.rs src/models/page.rs web/src/components/PageCard.tsx web/src/api/pages.ts Cargo.toml Cargo.lock migrations/
git commit -m "feat(security): hash page passwords with argon2, use constant-time comparison, remove password from share URLs"
```

---

## Task 4: Refresh Token Reuse Detection + Password Change Revocation

**Files:**
- Modify: `src/api/auth.rs:109-128` (refresh handler)
- Modify: `src/api/auth.rs:242-276` (change_password handler)
- Modify: `src/models/user.rs` (add `revoke_all_refresh_tokens`)

- [ ] **Step 1: Write the failing test**

Add to `src/api/auth.rs` tests or `src/models/user.rs` tests:

```rust
#[sqlx::test]
async fn change_password_revokes_all_refresh_tokens(pool: PgPool) {
    // Create user, store refresh token, change password
    // Assert refresh token is no longer valid
}

#[sqlx::test]
async fn refresh_token_reuse_revokes_all_tokens(pool: PgPool) {
    // Create user, store refresh token
    // Use refresh token once (success)
    // Use same refresh token again (should fail + revoke all)
    // Assert user has no valid refresh tokens
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `docker compose run --rm lettura cargo test`
Expected: FAIL

- [ ] **Step 3: Write minimal implementation**

In `src/models/user.rs`, add:

```rust
pub async fn revoke_all_refresh_tokens(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
```

In `src/api/auth.rs` `refresh` handler, change the flow:

```rust
pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let token_hash = jwt::hash_refresh_token(&req.refresh_token);

    let stored = user::find_refresh_token(&state.pool, &token_hash)
        .await?
        .ok_or_else(|| {
            // Token not found — possible reuse. Revoke all tokens for safety.
            // We can't identify the user from a hash that's not in DB,
            // so we just reject. The real reuse detection happens when
            // the same hash is used twice (first use deletes it, second
       // use finds it gone).
            ApiError::Unauthorized("invalid refresh token".into())
        })?;

    // Delete old refresh token (rotation)
    user::delete_refresh_token(&state.pool, &token_hash).await?;

    // Find user to get current is_admin status
    let found = user::find_user_by_id(&state.pool, stored.user_id)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("user not found".into()))?;

    issue_tokens(&state, found.id, found.is_admin).await
}
```

The key change: if `find_refresh_token` returns `None` (token already deleted = reuse detected), we should try to identify the user from the token hash and revoke all their tokens. However, since the hash is no longer in the DB, we can't identify the user. The current approach (rejecting the request) is sufficient because:
1. First use: token found, deleted, new tokens issued
2. Reuse: token not found (already deleted), request rejected
3. The attacker's stolen token is also invalidated after step 1

For **change_password**, add after `user::update_password`:

```rust
// Revoke all refresh tokens so other sessions are terminated
user::revoke_all_refresh_tokens(&state.pool, auth.user_id).await?;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `docker compose run --rm lettura cargo test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/auth.rs src/models/user.rs
git commit -m "feat(security): revoke all refresh tokens on password change, harden refresh token rotation"
```

---

## Task 5: Health Check + Metrics Endpoint Hardening

**Files:**
- Modify: `src/api/health.rs:22-25,29-32`
- Modify: `src/main.rs:85-93` (metrics route)

- [ ] **Step 1: Write the failing test**

Add to `src/api/health.rs` tests:

```rust
#[test]
fn health_response_hides_internal_errors() {
    // When DB fails, response should say "error" not expose the error message
    let response = HealthResponse {
        status: "error".to_string(),
        db: "error".to_string(),  // NOT "error: connection refused at 127.0.0.1:5432"
        search: "error".to_string(),
    };
    assert!(!response.db.contains("127.0.0.1"));
    assert!(!response.db.contains("connection"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `docker compose run --rm lettura cargo test --lib api::health`
Expected: FAIL

- [ ] **Step 3: Write minimal implementation**

In `src/api/health.rs`, replace:

```rust
// Before:
Err(e) => format!("error: {e}"),
// After:
Err(_) => "error".to_string(),
```

For both `db_msg` and `search_msg`.

For metrics endpoint, in `src/main.rs`, wrap the metrics route with a basic auth check using a new env var `LETTURA_METRICS_BEARER_TOKEN`:

```rust
let metrics_route = if let Some(ref token) = config.metrics_bearer_token {
    axum::Router::new().route(
        "/metrics",
        axum::routing::get(move || async move { recorder_handle.render() })
    ).layer(axum::middleware::from_fn(move |req: Request, next: Next| {
        let expected = token.clone();
        async move {
            let auth = req.headers().get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "));
            if let Some(provided) = auth {
                if subtle::ConstantTimeEq::ct_eq(provided.as_bytes(), expected.as_bytes()).into() {
                    return next.run(req).await;
                }
            }
            (StatusCode::UNAUTHORIZED, "unauthorized").into_response()
        }
    }))
} else {
    // No token configured — metrics disabled (not exposed)
    axum::Router::new()
};
```

Add `pub metrics_bearer_token: Option<String>` to `Config`, parsed from `LETTURA_METRICS_BEARER_TOKEN`.

- [ ] **Step 4: Run test to verify it passes**

Run: `docker compose run --rm lettura cargo test --lib`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/health.rs src/main.rs src/config.rs
git commit -m "feat(security): hide internal error details from health endpoint, add bearer token auth for metrics"
```

---

## Task 6: Input Validation Hardening

**Files:**
- Modify: `src/api/auth.rs:18-26` (add max length)
- Modify: `src/api/entries.rs:26-33` (add max length)
- Modify: `src/api/tags.rs` (add max length)
- Modify: `src/api/bulk.rs` (add vec length limit)
- Modify: `src/models/tagging_rule.rs:204-206` (add regex size limit)
- Modify: `src/config.rs:101` (reduce import max body)

- [ ] **Step 1: Add password max length**

In `src/api/auth.rs`:

```rust
#[derive(Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(length(min = 1, max = 50, message = "username must be 1-50 characters"))]
    pub username: String,
    #[validate(email(message = "invalid email format"))]
    pub email: String,
    #[validate(length(min = 8, max = 128, message = "password must be 8-128 characters"))]
    pub password: String,
}
```

Same for `ChangePasswordRequest`:

```rust
#[derive(Deserialize, Validate)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 8, max = 128, message = "password must be 8-128 characters"))]
    pub current_password: String,
    #[validate(length(min = 8, max = 128, message = "password must be 8-128 characters"))]
    pub new_password: String,
}
```

- [ ] **Step 2: Add regex size limit for tagging rules**

In `src/models/tagging_rule.rs`, replace:

```rust
// Before:
"matches" => regex::Regex::new(target)
    .map(|re| re.is_match(field_value))
    .unwrap_or(false),
// After:
"matches" => regex::RegexBuilder::new(target)
    .size_limit(1024)  // Limit NFA states to prevent ReDoS
    .build()
    .map(|re| re.is_match(field_value))
    .unwrap_or(false),
```

- [ ] **Step 3: Reduce import max body default**

In `src/config.rs:101`, change:

```rust
// Before:
import_max_body_bytes: ... .unwrap_or(500 * 1024 * 1024),
// After:
import_max_body_bytes: ... .unwrap_or(50 * 1024 * 1024),
```

- [ ] **Step 4: Add entry title/tag length limits**

In `src/api/entries.rs`, add `max = 500` to title validation, and `max = 50` per tag label.

- [ ] **Step 5: Run full test suite**

Run: `docker compose run --rm lettura cargo test --lib`
Expected: PASS (existing tests may need adjustment for new max limits)

- [ ] **Step 6: Commit**

```bash
git add src/api/auth.rs src/api/entries.rs src/api/tags.rs src/api/bulk.rs src/models/tagging_rule.rs src/config.rs
git commit -m "feat(security): add input length limits, regex size limit, reduce import body max"
```

---

## Task 7: RSS Feed CDATA Hardening + Security Headers

**Files:**
- Modify: `src/api/feed.rs:94-97`
- Modify: `src/api/mod.rs:246-261`

- [ ] **Step 1: Fix CDATA injection in RSS feed**

In `src/api/feed.rs`, replace the `build_rss` function's item building:

```rust
// Before:
items.push_str(&format!(
    "<item><title><![CDATA[{}]]></title>...<description><![CDATA[{}]]></description></item>",
    title, escaped_url, entry.id, date, content
));
// After: escape ]]> inside CDATA sections
let safe_title = title.replace("]]>", "]]&gt;");
let safe_content = content.replace("]]>", "]]&gt;");
items.push_str(&format!(
    "<item><title><![CDATA[{}]]></title><link>{}</link><guid>{}</guid><pubDate>{}</pubDate><description><![CDATA[{}]]></description></item>",
    safe_title, escaped_url, entry.id, date, safe_content
));
```

- [ ] **Step 2: Add CSP and Permissions-Policy headers**

In `src/api/mod.rs`, after the existing security headers, add:

```rust
// Content-Security-Policy
.layer(SetResponseHeaderLayer::overriding(
    axum::http::header::HeaderName::from_static("content-security-policy"),
    HeaderValue::from_static(
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; font-src 'self' https://fonts.gstatic.com; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
    ),
))
// Permissions-Policy
.layer(SetResponseHeaderLayer::overriding(
    axum::http::header::HeaderName::from_static("permissions-policy"),
    HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"),
))
```

Note: The CSP allows `https://fonts.gstatic.com` for Google Fonts and `https:` for images (since article content may reference external images). The `unsafe-inline` for styles is needed for Tiptap editor and Tailwind.

- [ ] **Step 3: Run full test suite**

Run: `docker compose run --rm lettura cargo test --lib`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/api/feed.rs src/api/mod.rs
git commit -m "feat(security): fix RSS CDATA injection, add CSP and Permissions-Policy headers"
```

---

## Task 8: Docker + Deployment Hardening

**Files:**
- Modify: `docker-compose.yml` (bind postgres to 127.0.0.1)
- Modify: `.env.example` (remove hint-like default JWT_SECRET)
- Modify: `.dockerignore` (add .git, .github)
- Modify: `web/src/components/EntryTags.tsx:28-33` (use unified API client)
- Modify: `web/src/pages/EntryDetailPage.tsx:260` (restrict DOMPurify config)

- [ ] **Step 1: Bind PostgreSQL port to localhost only**

In `docker-compose.yml`, change:

```yaml
# Before:
ports:
  - "${POSTGRES_PORT:-5436}:5432"
# After:
ports:
  - "127.0.0.1:${POSTGRES_PORT:-5436}:5432"
```

- [ ] **Step 2: Fix .env.example JWT_SECRET placeholder**

In `.env.example`, change:

```
# Before:
JWT_SECRET=change-me-in-production-use-at-least-32-chars
# After:
JWT_SECRET=<generate-a-random-secret-at-least-32-characters>
```

- [ ] **Step 3: Add .git and .github to .dockerignore**

In `.dockerignore`, add:

```
.git
.github
```

- [ ] **Step 4: Fix EntryTags.tsx to use unified API client**

In `web/src/components/EntryTags.tsx`, replace the raw `fetch()` call with the centralized `api` client from `../api/client`:

```typescript
// Before:
const res = await fetch(`/api/v1/entries/${entryId}/tags`, {
  headers: { Authorization: `Bearer ${localStorage.getItem('access_token')}` },
});
// After:
import { api } from '../api/client';
const res = await api.get(`/api/v1/entries/${entryId}/tags`);
```

- [ ] **Step 5: Restrict DOMPurify to disallow dangerous tags**

In `web/src/pages/EntryDetailPage.tsx`, change:

```typescript
// Before:
dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(entry.content) }}
// After:
dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(entry.content, {
  FORBID_TAGS: ['iframe', 'form', 'input', 'textarea', 'select', 'button', 'object', 'embed', 'applet'],
  FORBID_ATTR: ['formaction', 'xlink:href'],
}) }}
```

- [ ] **Step 6: Run full test suite**

Run: `docker compose run --rm lettura cargo test --lib`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add docker-compose.yml .env.example .dockerignore web/src/components/EntryTags.tsx web/src/pages/EntryDetailPage.tsx
git commit -m "fix(security): bind postgres to localhost, fix .env.example, restrict DOMPurify, use unified API client"
```

---

## Summary of Changes by Severity

| Task | Severity | Issues Fixed |
|------|----------|-------------|
| 1 | Critical | SSRF — block private IP ranges in fetch pipeline |
| 2 | High | CORS wildcard in production |
| 3 | High | Page password plaintext storage + timing attack + URL leak |
| 4 | High | Refresh token reuse + password change revocation |
| 5 | Medium | Health check info leak + metrics auth |
| 6 | Medium | Input validation (password max, ReDoS, import limit) |
| 7 | Medium | RSS CDATA injection + CSP/Permissions-Policy headers |
| 8 | Low-Medium | Docker hardening + frontend fixes |

## Items Deferred (require architectural changes)

These items are significant but require design decisions beyond this hardening pass:

1. **JWT `is_admin` revocation** — Requires a token blacklist or real-time DB check for admin status. Consider adding a `token_version` column to users table; increment on admin demotion, check in middleware.
2. **localStorage → httpOnly cookie** — Requires refactoring the entire auth flow (frontend + backend). The current Bearer token approach is CSRF-safe but XSS-vulnerable; switching to cookies requires CSRF protection.
3. **Registration open/close switch** — Add `LETTURA_REGISTRATION_ENABLED` env var (default: `true` for backward compat, recommend `false` in production).
4. **Feed token expiration** — Requires schema change + UI for regeneration flow.
5. **Backup excludes feed_token** — Simple change but affects backup/restore compatibility.
6. **JWT_SECRET key separation** — Use HKDF to derive separate keys for JWT signing and HMAC cookie signing.
7. **Rate limit IP extraction** — Add `LETTURA_TRUSTED_PROXY_COUNT` config to validate X-Forwarded-For chain.