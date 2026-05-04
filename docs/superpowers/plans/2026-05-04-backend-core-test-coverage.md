# Backend Core Logic Test Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add pure-function unit tests to 6 backend modules that currently have zero or minimal test coverage.

**Architecture:** Test-only changes — no production code changes except in `pipeline.rs` where a small pure function is extracted. All tests run via `docker compose exec lettura cargo test`.

**Tech Stack:** Rust 2024, serde_qs (dev-dep), chrono, url, sha1, uuid

**Note on TDD:** For existing pure functions, the TDD flow simplifies to "write tests → verify pass" since the implementation already exists. For new extracted functions (Task 5), we follow "write tests → verify compile fail → implement → verify pass".

---

## Task 1: `models/serde_helpers.rs` — Custom Deserializer Tests

**Files:**
- Modify: `Cargo.toml` (add `serde_qs` dev-dependency)
- Modify: `src/models/serde_helpers.rs` (add `#[cfg(test)]` module at bottom)
- Test: inline `tests` module

- [ ] **Step 1: Add `serde_qs` dev-dependency**

Add to `Cargo.toml` under `[dev-dependencies]`:
```toml
serde_qs = "0.14"
```

- [ ] **Step 2: Write tests for `deserialize_i64_from_string` and `deserialize_bool_from_string`**

Add to bottom of `src/models/serde_helpers.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct WrapI64 {
        #[serde(default, deserialize_with = "deserialize_i64_from_string")]
        val: Option<i64>,
    }

    #[test]
    fn i64_from_numeric_string() {
        let w: WrapI64 = serde_qs::from_str("val=42").unwrap();
        assert_eq!(w.val, Some(42));
    }

    #[test]
    fn i64_from_negative_string() {
        let w: WrapI64 = serde_qs::from_str("val=-7").unwrap();
        assert_eq!(w.val, Some(-7));
    }

    #[test]
    fn i64_from_non_numeric_string_is_error() {
        let result: Result<WrapI64, _> = serde_qs::from_str("val=abc");
        assert!(result.is_err());
    }

    #[test]
    fn i64_absent_is_none() {
        // Absent field → None via serde default
        let w: WrapI64 = serde_qs::from_str("").unwrap();
        assert_eq!(w.val, None);
    }

    #[derive(Deserialize)]
    struct WrapBool {
        #[serde(default, deserialize_with = "deserialize_bool_from_string")]
        val: Option<bool>,
    }

    #[test]
    fn bool_from_true_string() {
        let w: WrapBool = serde_qs::from_str("val=true").unwrap();
        assert_eq!(w.val, Some(true));
    }

    #[test]
    fn bool_from_false_string() {
        let w: WrapBool = serde_qs::from_str("val=false").unwrap();
        assert_eq!(w.val, Some(false));
    }

    #[test]
    fn bool_from_invalid_string_is_error() {
        let result: Result<WrapBool, _> = serde_qs::from_str("val=yes");
        assert!(result.is_err());
    }

    #[test]
    fn bool_from_numeric_string_is_error() {
        // "1" and "0" are NOT accepted — only "true"/"false" strings
        let result: Result<WrapBool, _> = serde_qs::from_str("val=1");
        assert!(result.is_err());
    }

    #[test]
    fn bool_absent_is_none() {
        let w: WrapBool = serde_qs::from_str("").unwrap();
        assert_eq!(w.val, None);
    }
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `docker compose exec lettura cargo test --lib models::serde_helpers`
Expected: All 9 tests PASS

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml src/models/serde_helpers.rs
git commit -m "test: add unit tests for serde_helpers deserializers"
```

---

## Task 2: `models/entry.rs` — Pure Function Tests

**Files:**
- Modify: `src/models/entry.rs` (add `#[cfg(test)]` module at bottom)
- Test: inline `tests` module

- [ ] **Step 1: Write tests for `hash_url`, `extract_domain`, and `next_cursor_from`**

Add to bottom of `src/models/entry.rs` (after the `attach_tags` function, outside `cursor` module):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_url_deterministic() {
        let h1 = hash_url("https://example.com/article");
        let h2 = hash_url("https://example.com/article");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_url_different_urls() {
        let h1 = hash_url("https://example.com/a");
        let h2 = hash_url("https://example.com/b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_url_empty() {
        let h = hash_url("");
        assert!(!h.is_empty());
    }

    #[test]
    fn extract_domain_common() {
        assert_eq!(extract_domain("https://example.com/path"), Some("example.com".to_string()));
    }

    #[test]
    fn extract_domain_with_port() {
        assert_eq!(extract_domain("http://localhost:3000/api"), Some("localhost".to_string()));
    }

    #[test]
    fn extract_domain_subdomain() {
        assert_eq!(extract_domain("https://blog.example.com/post"), Some("blog.example.com".to_string()));
    }

    #[test]
    fn extract_domain_invalid() {
        assert_eq!(extract_domain("not-a-url"), None);
    }

    #[test]
    fn extract_domain_ip() {
        assert_eq!(extract_domain("http://192.168.1.1/path"), Some("192.168.1.1".to_string()));
    }

    fn mock_summary(id: Uuid, created_at: DateTime<Utc>) -> EntrySummary {
        EntrySummary {
            id,
            user_id: Uuid::new_v4(),
            url: "https://example.com".to_string(),
            title: None,
            content_type: "article".to_string(),
            extract_method: "readability".to_string(),
            language: None,
            reading_time: None,
            preview_picture: None,
            domain_name: None,
            published_by: None,
            is_archived: false,
            is_starred: false,
            created_at,
            deleted_at: None,
            tags: vec![],
        }
    }

    #[test]
    fn next_cursor_full_page() {
        let per_page = 3;
        let items: Vec<EntrySummary> = (0..3)
            .map(|i| mock_summary(Uuid::new_v4(), Utc::now() + chrono::Duration::seconds(i)))
            .collect();
        assert!(next_cursor_from(&items, per_page).is_some());
    }

    #[test]
    fn next_cursor_partial_page() {
        let per_page = 5;
        let items: Vec<EntrySummary> = (0..3)
            .map(|i| mock_summary(Uuid::new_v4(), Utc::now() + chrono::Duration::seconds(i)))
            .collect();
        assert!(next_cursor_from(&items, per_page).is_none());
    }

    #[test]
    fn next_cursor_empty() {
        let items: Vec<EntrySummary> = vec![];
        assert!(next_cursor_from(&items, 20).is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `docker compose exec lettura cargo test --lib models::entry::tests`
Expected: All 12 tests PASS (2 existing cursor tests + 10 new tests)

Note: `mock_summary` must match the actual `EntrySummary` struct fields exactly. If compilation fails, compare against the struct definition and adjust field names/values.

- [ ] **Step 3: Commit**

```bash
git add src/models/entry.rs
git commit -m "test: add unit tests for entry pure functions (hash_url, extract_domain, next_cursor_from)"
```

---

## Task 3: `models/tag.rs` — `slugify` Tests

**Files:**
- Modify: `src/models/tag.rs` (add `#[cfg(test)]` module at bottom)
- Test: inline `tests` module

- [ ] **Step 1: Write tests for `slugify`**

Add to bottom of `src/models/tag.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_english() {
        assert_eq!(slugify("Rust Programming"), "rust-programming");
    }

    #[test]
    fn slugify_lowercase_input() {
        assert_eq!(slugify("rust"), "rust");
    }

    #[test]
    fn slugify_special_characters() {
        // +, +, space, &, space → 5 dashes between c and rust
        assert_eq!(slugify("C++ & Rust"), "c-----rust");
    }

    #[test]
    fn slugify_empty() {
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn slugify_chinese() {
        // Chinese chars are Unicode alphanumeric → preserved as-is
        let result = slugify("技术博客");
        assert_eq!(result, "技术博客");
    }

    #[test]
    fn slugify_leading_trailing_dashes() {
        assert_eq!(slugify("--hello--"), "hello");
    }

    #[test]
    fn slugify_already_slugified() {
        assert_eq!(slugify("my-tag"), "my-tag");
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `docker compose exec lettura cargo test --lib models::tag::tests`
Expected: All 7 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/models/tag.rs
git commit -m "test: add unit tests for tag::slugify"
```

---

## Task 4: `models/audit_log.rs` — `new_entry` Constructor Tests

**Files:**
- Modify: `src/models/audit_log.rs` (add `#[cfg(test)]` module at bottom)
- Test: inline `tests` module

- [ ] **Step 1: Write tests for `new_entry`**

Add to bottom of `src/models/audit_log.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_entry_defaults() {
        let user_id = Uuid::new_v4();
        let log = new_entry(
            Some(user_id),
            "jwt".to_string(),
            AuditAction::CreateEntry,
            Some(AuditResourceType::Entry),
            Some(Uuid::new_v4()),
            serde_json::json!({}),
        );
        assert_eq!(log.user_id, Some(user_id));
        assert_eq!(log.auth_source, "jwt");
        assert_eq!(log.action, AuditAction::CreateEntry);
        assert_eq!(log.status, "success");
        assert!(log.error_message.is_none());
        assert!(log.ip_address.is_none());
        assert!(log.user_agent.is_none());
        assert!(log.request_id.is_none());
    }

    #[test]
    fn new_entry_preserves_resource_type() {
        let log = new_entry(
            None,
            "pat".to_string(),
            AuditAction::Login,
            Some(AuditResourceType::User),
            None,
            serde_json::json!({"method": "password"}),
        );
        assert_eq!(log.resource_type, Some(AuditResourceType::User));
        assert!(log.resource_id.is_none());
        assert_eq!(log.details["method"], "password");
    }

    #[test]
    fn new_entry_no_user() {
        let log = new_entry(
            None,
            "public".to_string(),
            AuditAction::AdminBackup,
            Some(AuditResourceType::System),
            None,
            serde_json::json!(null),
        );
        assert!(log.user_id.is_none());
        assert_eq!(log.action, AuditAction::AdminBackup);
    }

    #[test]
    fn audit_action_variants_exist() {
        // Verify key variants compile and are distinct
        assert_ne!(AuditAction::CreateEntry, AuditAction::UpdateEntry);
        assert_ne!(AuditAction::Login, AuditAction::Logout);
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `docker compose exec lettura cargo test --lib models::audit_log::tests`
Expected: All 4 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/models/audit_log.rs
git commit -m "test: add unit tests for audit_log::new_entry"
```

---

## Task 5: `fetch/pipeline.rs` — Extract and Test `should_try_render`

**Files:**
- Modify: `src/fetch/pipeline.rs` (extract pure function, add tests)
- Test: inline `tests` module

- [ ] **Step 1: Write tests for `should_try_render` (function does not yet exist — will compile fail)**

In `src/fetch/pipeline.rs`, add this function after the `SHORT_CONTENT_THRESHOLD` constant:

```rust
/// Decide whether the extracted content is too short and rendering should be
/// attempted. Pure function — no dependencies on DB, HTTP, or async runtime.
pub fn should_try_render(text_len: usize, render_mode: RenderMode) -> bool {
    text_len < SHORT_CONTENT_THRESHOLD && render_mode != RenderMode::Never
}
```

Then replace the inline condition in `process_body` (around line 185-192):

```rust
// Before:
if result.inner.text_content.len() < SHORT_CONTENT_THRESHOLD {
    if let Some(sc) = site_config {
        if sc.render.mode != RenderMode::Never
            && try_render_then_extract(ctx, job, sc, status).await
        {
            return;
        }
    }
}

// After:
if should_try_render(result.inner.text_content.len(), site_config.map(|sc| sc.render.mode).unwrap_or(RenderMode::Never)) {
    if let Some(sc) = site_config {
        if try_render_then_extract(ctx, job, sc, status).await {
            return;
        }
    }
}
```

Add to bottom of `src/fetch/pipeline.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_content_auto_mode_triggers_render() {
        assert!(should_try_render(50, RenderMode::Auto));
    }

    #[test]
    fn short_content_never_mode_skips_render() {
        assert!(!should_try_render(50, RenderMode::Never));
    }

    #[test]
    fn long_content_skips_render() {
        assert!(!should_try_render(500, RenderMode::Auto));
    }

    #[test]
    fn threshold_boundary_below() {
        assert!(should_try_render(99, RenderMode::Auto));
    }

    #[test]
    fn threshold_boundary_at() {
        assert!(!should_try_render(100, RenderMode::Auto));
    }

    #[test]
    fn force_mode_with_short_content() {
        // Force mode skips static path entirely, but should_try_render
        // only governs the post-extraction fallback check.
        assert!(should_try_render(50, RenderMode::Force));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail to compile (function not yet defined)**

Run: `docker compose exec lettura cargo test --lib fetch::pipeline::tests`
Expected: Compilation error — `should_try_render` is not defined

- [ ] **Step 3: Extract `should_try_render` as a pure function**

In `src/fetch/pipeline.rs`, add this function after the `SHORT_CONTENT_THRESHOLD` constant:

```rust
/// Decide whether the extracted content is too short and rendering should be
/// attempted. Pure function — no dependencies on DB, HTTP, or async runtime.
pub(crate) fn should_try_render(text_len: usize, render_mode: RenderMode) -> bool {
    text_len < SHORT_CONTENT_THRESHOLD && render_mode != RenderMode::Never
}
```

Then replace the inline condition in `process_body` (around line 185-192):

```rust
// Before:
if result.inner.text_content.len() < SHORT_CONTENT_THRESHOLD {
    if let Some(sc) = site_config {
        if sc.render.mode != RenderMode::Never
            && try_render_then_extract(ctx, job, sc, status).await
        {
            return;
        }
    }
}

// After:
if should_try_render(result.inner.text_content.len(), site_config.map(|sc| sc.render.mode).unwrap_or(RenderMode::Never)) {
    if let Some(sc) = site_config {
        if try_render_then_extract(ctx, job, sc, status).await {
            return;
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `docker compose exec lettura cargo test --lib fetch::pipeline::tests`
Expected: All 6 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/fetch/pipeline.rs
git commit -m "refactor: extract should_try_render from pipeline, add unit tests"
```

---

## Task 6: `tasks/fetcher.rs` — Queue Logic Tests

**Files:**
- Modify: `src/tasks/fetcher.rs` (add `#[cfg(test)]` module at bottom)
- Test: inline `tests` module

- [ ] **Step 1: Write tests for `FetchQueue::send`**

Add to bottom of `src/tasks/fetcher.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn send_increments_queue_depth() {
        let (tx, mut rx) = mpsc::channel::<FetchJob>(100);
        let queue_depth = Arc::new(AtomicUsize::new(0));
        let queue = FetchQueue { tx, queue_depth: queue_depth.clone() };

        let job = FetchJob {
            entry_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            url: "https://example.com".to_string(),
        };

        queue.send(job).await.unwrap();
        assert_eq!(queue_depth.load(Ordering::Relaxed), 1);

        // Drain the channel
        let _ = rx.recv().await;
    }

    #[tokio::test]
    async fn send_fails_when_channel_closed() {
        let (tx, rx) = mpsc::channel::<FetchJob>(100);
        let queue_depth = Arc::new(AtomicUsize::new(0));
        let queue = FetchQueue { tx, queue_depth: queue_depth.clone() };

        drop(rx); // Close the receiver

        let job = FetchJob {
            entry_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            url: "https://example.com".to_string(),
        };

        let result = queue.send(job).await;
        assert!(result.is_err());
        // queue_depth should be rolled back on error
        assert_eq!(queue_depth.load(Ordering::Relaxed), 0);
    }
}
```

Note: `FetchQueue` and `FetchJob` are `pub(crate)` structs with public fields. If the struct has additional fields not shown here, add them with default values. Verify the actual struct definition before writing tests.

- [ ] **Step 2: Run tests to verify they pass**

Run: `docker compose exec lettura cargo test --lib tasks::fetcher::tests`
Expected: All 2 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/tasks/fetcher.rs
git commit -m "test: add unit tests for FetchQueue send and error handling"
```

---

## Task 7: Final Verification

- [ ] **Step 1: Run all unit tests together**

Run: `docker compose exec lettura cargo test --lib`
Expected: All tests pass, including the ~38 new tests across 6 modules.

- [ ] **Step 2: Run full workspace tests to check for regressions**

Run: `docker compose exec lettura cargo test --workspace`
Expected: All existing tests still pass.

- [ ] **Step 3: Commit any remaining changes**
