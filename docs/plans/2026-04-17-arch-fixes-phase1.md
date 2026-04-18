# Phase 1: Critical Functionality & Security Fixes

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix dead features (Site Rules & Tagging Rules not wired into pipeline), search user isolation, XSS defense on frontend, and rate limiter IP-keyed enforcement.

**Architecture:** Wire existing-but-unused model functions into the fetch/content pipeline. Add `user_id` field to Tantivy schema for search isolation. Add DOMPurify as a frontend defense layer. Switch rate limiter from `DirectRateLimiter` to IP-keyed.

**Tech Stack:** Rust (Axum, Tantivy, sqlx, governor), TypeScript (React 19, DOMPurify)

---

## Task 1: Add `user_id` to Tantivy search index for user isolation

**Files:**
- Modify: `src/search.rs`
- Modify: `src/tasks/fetcher.rs`
- Modify: `src/api/entries.rs`
- Modify: `src/api/admin.rs`
- Modify: `src/api/backup.rs`

- [ ] **Step 1: Update Tantivy schema to include `user_id` field**

In `src/search.rs`, add a `f_user_id` field to `SearchIndex`. Update both `open()` and `in_memory()` to add the field. Update `upsert()` to accept `user_id` and include it in the document. Update `search()` to accept an optional `user_id` filter.

```rust
// In SearchIndex struct, add:
f_user_id: Field,

// In open() and in_memory(), add to schema builder:
let f_user_id = schema_builder.add_text_field("user_id", STRING | STORED);

// Update upsert signature:
pub async fn upsert(
    &self,
    id: Uuid,
    user_id: Uuid,
    title: &str,
    content: &str,
    url: &str,
    domain: &str,
) -> Result<(), tantivy::TantivyError> {
    let mut writer = self.writer.lock().await;
    let id_str = id.to_string();
    let id_term = tantivy::Term::from_field_text(self.f_id, &id_str);
    writer.delete_term(id_term);
    writer.add_document(doc!(
        self.f_id => id_str,
        self.f_user_id => user_id.to_string(),
        self.f_title => title,
        self.f_content => content,
        self.f_url => url,
        self.f_domain => domain,
    ))?;
    writer.commit()?;
    Ok(())
}

// Update search signature to accept user_id filter:
pub fn search(&self, query_str: &str, user_id: Option<Uuid>, limit: usize) -> Result<Vec<Uuid>, tantivy::TantivyError> {
    use tantivy::query::{BooleanQuery, Occur, TermQuery};
    let searcher = self.reader.searcher();
    let query_parser = QueryParser::for_index(&self.index, vec![self.f_title, self.f_content]);
    let text_query = query_parser.parse_query(query_str)?;

    let final_query: Box<dyn tantivy::query::Query> = if let Some(uid) = user_id {
        let user_term = tantivy::Term::from_field_text(self.f_user_id, &uid.to_string());
        let user_query = TermQuery::new(user_term, tantivy::schema::IndexRecordOption::Basic);
        Box::new(BooleanQuery::intersection(vec![
            Box::new(text_query),
            Box::new(user_query),
        ]))
    } else {
        Box::new(text_query)
    };

    let top_docs = searcher.search(&final_query, &TopDocs::with_limit(limit))?;
    let mut ids = Vec::new();
    for (_score, doc_address) in top_docs {
        let doc: TantivyDocument = searcher.doc(doc_address)?;
        if let Some(id_val) = doc.get_first(self.f_id) {
            if let Some(id_str) = id_val.as_str() {
                if let Ok(uuid) = Uuid::parse_str(id_str) {
                    ids.push(uuid);
                }
            }
        }
    }
    Ok(ids)
}
```

- [ ] **Step 2: Update unit tests in `src/search.rs`**

Update existing tests to pass `user_id` to `upsert()`. Add a new test verifying user-scoped search isolation.

```rust
#[tokio::test]
async fn search_filters_by_user() {
    let idx = SearchIndex::in_memory().unwrap();
    let user1 = Uuid::new_v4();
    let user2 = Uuid::new_v4();
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    idx.upsert(id1, user1, "Rust Guide", "Rust ownership", "https://a.com", "a.com").await.unwrap();
    idx.upsert(id2, user2, "Rust Guide", "Rust borrowing", "https://b.com", "b.com").await.unwrap();
    idx.reader.reload().unwrap();

    let results = idx.search("Rust", Some(user1), 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], id1);
}
```

- [ ] **Step 3: Run tests to verify**

Run: `cargo test --lib search`
Expected: all tests pass

- [ ] **Step 4: Update all callers of `search_index.upsert()` and `search_index.search()`**

In `src/tasks/fetcher.rs`, the fetch worker needs access to `user_id`. Update `FetchJob` to include `user_id`:

```rust
#[derive(Debug, Clone)]
pub struct FetchJob {
    pub entry_id: Uuid,
    pub user_id: Uuid,
    pub url: String,
}
```

Update `process_job` to pass `user_id` to search upsert (after content update, re-index):

In `src/api/entries.rs`:
- Update `restore_entry`: pass `user_id` to `search_index.upsert()`
- Update `list_entries`: pass `Some(auth.user_id)` to `search_index.search()`
- Update `create_entry`: include `user_id` in `FetchJob`

In `src/api/memos.rs`:
- Update `promote_memo`: include `user_id` in `FetchJob`

In `src/api/admin.rs`:
- Update `reindex`: pass `user_id` to `upsert()`. Change the query to also select `user_id`.

In `src/api/backup.rs`:
- Update restore's reindex loop: pass `user_id` to `upsert()`.

- [ ] **Step 5: Run full test suite**

Run: `cargo test --lib`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/search.rs src/tasks/fetcher.rs src/api/entries.rs src/api/memos.rs src/api/admin.rs src/api/backup.rs
git commit -m "feat: add user_id to Tantivy index for search isolation"
```

---

## Task 2: Integrate Site Rules into content extraction pipeline

**Files:**
- Modify: `src/tasks/fetcher.rs`
- Modify: `src/extract/mod.rs`
- Modify: `src/extract/readability.rs`

- [ ] **Step 1: Update `extract()` to accept optional site rule selectors**

In `src/extract/mod.rs`, add a `SiteRuleConfig` struct and update `extract()`:

```rust
#[derive(Debug, Clone, Default)]
pub struct SiteRuleConfig {
    pub content_selector: Option<String>,
    pub title_selector: Option<String>,
    pub strip_selectors: Option<Vec<String>>,
}

pub fn extract(html: &str, url: Option<&str>, site_rule: Option<&SiteRuleConfig>) -> Result<ExtractResult, ExtractError> {
    let preprocessed = preprocess::preprocess(html);
    let document = scraper::Html::parse_document(&preprocessed);

    // Apply strip selectors from site rule
    let document = if let Some(rule) = site_rule {
        if let Some(ref strip) = rule.strip_selectors {
            strip_elements(&document, strip)
        } else {
            document
        }
    } else {
        document
    };

    let meta = metadata::extract_metadata(&document, url);
    // If site rule has a title selector, use it
    let title = if let Some(rule) = site_rule {
        if let Some(ref title_sel) = rule.title_selector {
            extract_title_by_selector(&document, title_sel).or(meta.title)
        } else {
            meta.title
        }
    } else {
        meta.title
    };

    let article_html = if let Some(rule) = site_rule {
        if let Some(ref content_sel) = rule.content_selector {
            readability::extract_content_with_selector(&document, content_sel)?
        } else {
            readability::extract_content(&document)?
        }
    } else {
        readability::extract_content(&document)?
    };

    let clean_html = sanitize::sanitize(&article_html, url);
    let text_content = html_to_text(&clean_html);
    let reading_time = estimate_reading_time(&text_content);

    Ok(ExtractResult {
        title,
        content: clean_html,
        text_content,
        author: meta.author,
        language: meta.language,
        preview_image: meta.preview_image,
        excerpt: meta.excerpt,
        reading_time,
    })
}

fn strip_elements(document: &scraper::Html, selectors: &[String]) -> scraper::Html {
    let mut html_str = document.html();
    for sel_str in selectors {
        if let Ok(sel) = scraper::Selector::parse(sel_str) {
            let doc = scraper::Html::parse_document(&html_str);
            let ids: Vec<ego_tree::NodeId> = doc.select(&sel).map(|el| el.id()).collect();
            if !ids.is_empty() {
                // Re-parse and remove elements
                let mut html_for_strip = doc.html();
                for sel_str_again in selectors {
                    if let Ok(s) = scraper::Selector::parse(sel_str_again) {
                        let d = scraper::Html::parse_document(&html_for_strip);
                        let mut parts_to_remove: Vec<String> = Vec::new();
                        for el in d.select(&s) {
                            parts_to_remove.push(el.html());
                        }
                        for part in parts_to_remove {
                            html_for_strip = html_for_strip.replace(&part, "");
                        }
                    }
                }
                html_str = html_for_strip;
                break;
            }
        }
    }
    scraper::Html::parse_document(&html_str)
}

fn extract_title_by_selector(document: &scraper::Html, selector: &str) -> Option<String> {
    scraper::Selector::parse(selector).ok().and_then(|sel| {
        document.select(&sel).next().map(|el| {
            el.text().collect::<Vec<_>>().join("").trim().to_string()
        })
    })
}
```

- [ ] **Step 2: Add `extract_content_with_selector` to readability.rs**

In `src/extract/readability.rs`, add a function that uses a custom CSS selector:

```rust
pub fn extract_content_with_selector(document: &Html, selector: &str) -> Result<String, ExtractError> {
    let sel = scraper::Selector::parse(selector)
        .map_err(|_| ExtractError::NoContent)?;
    let element = document.select(&sel).next()
        .ok_or(ExtractError::NoContent)?;
    let content = element.inner_html();
    if content.trim().is_empty() {
        return Err(ExtractError::NoContent);
    }
    Ok(content)
}
```

- [ ] **Step 3: Update `process_job` in `src/tasks/fetcher.rs` to look up site rules**

Before calling `extract::extract()`, look up the site rule for the domain:

```rust
async fn process_job(
    pool: &PgPool,
    rate_limiter: &Arc<Mutex<DomainRateLimiter>>,
    image_storage: &Arc<dyn ImageStorage>,
    job: &FetchJob,
) {
    // ... (existing rate limit code) ...

    let site_rule_config = if let Some(ref domain) = entry::extract_domain(&job.url) {
        match crate::models::site_rule::find_by_domain(pool, job.user_id, domain).await {
            Ok(Some(rule)) => Some(extract::SiteRuleConfig {
                content_selector: Some(rule.content_selector),
                title_selector: rule.title_selector,
                strip_selectors: rule.strip_selectors,
            }),
            _ => None,
        }
    } else {
        None
    };

    // ... then in the match response block, replace extract::extract(&html, Some(&job.url))
    // with extract::extract(&html, Some(&job.url), site_rule_config.as_ref())
    match extract::extract(&html, Some(&job.url), site_rule_config.as_ref()) {
        // ... rest same ...
    }
}
```

- [ ] **Step 4: Update all callers of `extract::extract()` to pass `None` for site_rule**

Search for all calls to `extract::extract()` in the codebase and add `None` as the third argument. The main callers are in `src/tasks/fetcher.rs` (already updated above) and possibly tests in `src/extract/`.

- [ ] **Step 5: Run tests**

Run: `cargo test --lib`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/extract/mod.rs src/extract/readability.rs src/tasks/fetcher.rs
git commit -m "feat: integrate site rules into content extraction pipeline"
```

---

## Task 3: Auto-apply Tagging Rules after entry fetch

**Files:**
- Modify: `src/tasks/fetcher.rs`

- [ ] **Step 1: Add tagging rule evaluation to `process_job`**

After a successful content extraction and DB update, evaluate the user's tagging rules against the entry fields and apply matching tags. Add this at the end of the successful extraction branch in `process_job`:

```rust
// After entry::update_entry_content succeeds in the success branch:
apply_tagging_rules(pool, job.user_id, job.entry_id, &job.url, &result).await;
```

Add a new helper function:

```rust
async fn apply_tagging_rules(
    pool: &PgPool,
    user_id: Uuid,
    entry_id: Uuid,
    url: &str,
    result: &extract::ExtractResult,
) {
    let rules = match crate::models::tagging_rule::list_rules(pool, user_id).await {
        Ok(r) => r,
        Err(_) => return,
    };

    let domain = entry::extract_domain(url).unwrap_or_default();
    let fields = crate::models::tagging_rule::EntryFields {
        title: result.title.clone().unwrap_or_default(),
        url: url.to_string(),
        domain_name: domain,
        language: result.language.clone().unwrap_or_default(),
        reading_time: result.reading_time as i32,
        content_type: "article".to_string(),
    };

    for rule in rules {
        if crate::models::tagging_rule::evaluate_rule(&rule.rule, &fields) {
            for tag_label in &rule.tags {
                if let Ok(tag) = crate::models::tag::find_or_create_tag(pool, user_id, tag_label).await {
                    crate::models::tag::add_tag_to_entry(pool, entry_id, tag.id).await.ok();
                }
            }
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add src/tasks/fetcher.rs
git commit -m "feat: auto-apply tagging rules after entry content extraction"
```

---

## Task 4: Switch rate limiter to IP-keyed

**Files:**
- Modify: `src/rate_limit.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: Update `GlobalRateLimit` to use `KeyedRateLimiter`**

Replace `DirectRateLimiter` with an IP-keyed limiter. Extract client IP from `X-Forwarded-For` or `X-Real-IP` header, falling back to connection info.

```rust
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;
use tower::ServiceBuilder;

#[derive(Clone)]
pub struct GlobalRateLimit {
    limiter: Arc<governor::RateLimiter<governor::middleware::NoOpMiddleware, governor::middleware::NoOpMiddleware, governor::state::keyed::DefaultKeyedStateStore<String>>>,
}

impl GlobalRateLimit {
    pub fn new(requests_per_minute: u32) -> Self {
        let quota = Quota::per_minute(
            NonZeroU32::new(requests_per_minute).expect("requests_per_minute must be > 0"),
        );
        Self {
            limiter: Arc::new(RateLimiter::keyed(quota)),
        }
    }
}

fn extract_client_ip(request: &Request) -> String {
    // Try X-Forwarded-For first (first IP in the list)
    if let Some(xff) = request.headers().get("x-forwarded-for") {
        if let Ok(val) = xff.to_str() {
            if let Some(ip) = val.split(',').next() {
                let trimmed = ip.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    // Try X-Real-IP
    if let Some(xri) = request.headers().get("x-real-ip") {
        if let Ok(val) = xri.to_str() {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    // Fallback
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
            (
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                [(axum::http::header::RETRY_AFTER, "60")],
                "rate limit exceeded",
            )
                .into_response()
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib rate_limit`
Expected: all tests pass. The existing tests need updating to work with the new keyed limiter structure.

Update tests:

```rust
#[test]
fn test_global_rate_limit_creation() {
    let rl = GlobalRateLimit::new(100);
    assert!(rl.limiter.check_key(&"127.0.0.1".to_string()).is_ok());
}

#[test]
fn test_rate_limit_exhaustion() {
    let rl = GlobalRateLimit::new(1);
    assert!(rl.limiter.check_key(&"127.0.0.1".to_string()).is_ok());
    assert!(rl.limiter.check_key(&"127.0.0.1".to_string()).is_err());
}

#[test]
fn test_different_ips_independent() {
    let rl = GlobalRateLimit::new(1);
    assert!(rl.limiter.check_key(&"127.0.0.1".to_string()).is_ok());
    assert!(rl.limiter.check_key(&"127.0.0.2".to_string()).is_ok());
}

#[test]
fn test_clone_shares_state() {
    let rl = GlobalRateLimit::new(2);
    let rl2 = rl.clone();
    assert!(rl.limiter.check_key(&"127.0.0.1".to_string()).is_ok());
    assert!(rl.limiter.check_key(&"127.0.0.1".to_string()).is_ok());
    assert!(rl2.limiter.check_key(&"127.0.0.1".to_string()).is_err());
}
```

- [ ] **Step 3: Commit**

```bash
git add src/rate_limit.rs
git commit -m "fix: switch rate limiter to IP-keyed for per-client enforcement"
```

---

## Task 5: Add image download size limit

**Files:**
- Modify: `src/storage/mod.rs`

- [ ] **Step 1: Add content-length check and response body size limit**

In `download_image()`, check `Content-Length` header before downloading, and use `content-length` based streaming with a max size of 10MB:

```rust
const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB

pub async fn download_image(url: &str) -> Result<(Vec<u8>, String), StorageError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Lettura/0.1")
        .build()
        .map_err(|e| StorageError::Io(e.to_string()))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| StorageError::Io(e.to_string()))?;

    // Check Content-Length header before downloading body
    if let Some(content_length) = resp.content_length() {
        if content_length as usize > MAX_IMAGE_SIZE {
            return Err(StorageError::Io(format!(
                "image too large: {} bytes (max {} bytes)",
                content_length, MAX_IMAGE_SIZE
            )));
        }
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| StorageError::Io(e.to_string()))?;

    if bytes.len() > MAX_IMAGE_SIZE {
        return Err(StorageError::Io(format!(
            "image too large: {} bytes (max {} bytes)",
            bytes.len(), MAX_IMAGE_SIZE
        )));
    }

    Ok((bytes.to_vec(), content_type))
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add src/storage/mod.rs
git commit -m "fix: add 10MB size limit for downloaded images"
```

---

## Task 6: Add DOMPurify XSS defense on frontend

**Files:**
- Modify: `web/package.json` (add dompurify)
- Modify: `web/src/pages/EntryDetailPage.tsx`

- [ ] **Step 1: Install DOMPurify**

Run: `cd web && pnpm add dompurify && pnpm add -D @types/dompurify`

- [ ] **Step 2: Add sanitization in EntryDetailPage**

In `web/src/pages/EntryDetailPage.tsx`, sanitize the HTML before rendering:

```tsx
import DOMPurify from 'dompurify';

// Replace the dangerouslySetInnerHTML line:
// Before:
//   <article className="prose ..." dangerouslySetInnerHTML={{ __html: entry.content }} />
// After:
//   <article className="prose ..." dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(entry.content) }} />
```

- [ ] **Step 3: Run build and tests**

Run: `cd web && pnpm run build`
Expected: build succeeds

- [ ] **Step 4: Commit**

```bash
git add web/package.json web/pnpm-lock.yaml web/src/pages/EntryDetailPage.tsx
git commit -m "fix: add DOMPurify XSS defense for article content rendering"
```

---

## Task 7: Fix Pages path traversal TOCTOU race

**Files:**
- Modify: `src/api/pages_public.rs`

- [ ] **Step 1: Use canonicalize on the base once, then validate the file path**

Replace the current TOCTOU-vulnerable code with a single canonicalize that ensures the resolved path starts with the canonical base:

```rust
if state.config.storage_type == "local" {
    let base_path = std::path::PathBuf::from(&state.config.pages_storage_path).join(slug);
    let canonical_base = match std::fs::canonicalize(&base_path) {
        Ok(p) => p,
        Err(_) => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };

    let file_path = canonical_base.join(file_name);

    // Verify the resolved path is still under the canonical base
    match std::fs::canonicalize(&file_path) {
        Ok(canonical_file) if canonical_file.starts_with(&canonical_base) => {
            match tokio::fs::read(&canonical_file).await {
                Ok(data) => {
                    let mime = mime_for_file(file_name);
                    (StatusCode::OK, [("content-type", mime)], data).into_response()
                }
                Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
            }
        }
        _ => (StatusCode::NOT_FORBIDDEN, "forbidden").into_response(),
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add src/api/pages_public.rs
git commit -m "fix: resolve TOCTOU race in page file path traversal check"
```

---

## Task 8: Add expired refresh token cleanup

**Files:**
- Modify: `src/models/user.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add cleanup function to `src/models/user.rs`**

```rust
pub async fn cleanup_expired_refresh_tokens(pool: &PgPool) -> Result<u64, ApiError> {
    let result = sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < now()")
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected())
}
```

- [ ] **Step 2: Spawn a background cleanup task in `src/main.rs`**

After the fetch worker is started, add:

```rust
{
    let cleanup_pool = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // 1 hour
        loop {
            interval.tick().await;
            match lettura::models::user::cleanup_expired_refresh_tokens(&cleanup_pool).await {
                Ok(count) if count > 0 => tracing::info!(removed = count, "cleaned up expired refresh tokens"),
                Err(e) => tracing::warn!("refresh token cleanup failed: {e}"),
                _ => {}
            }
        }
    });
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
git add src/models/user.rs src/main.rs
git commit -m "feat: add hourly background cleanup for expired refresh tokens"
```

---

## Task 9: Share `reqwest::Client` across fetch jobs

**Files:**
- Modify: `src/tasks/fetcher.rs`

- [ ] **Step 1: Move `reqwest::Client` creation to `start_fetch_worker` and share via `Arc`**

```rust
pub fn start_fetch_worker(pool: PgPool, concurrency: usize, image_storage: Arc<dyn ImageStorage>) -> FetchQueue {
    let (tx, rx) = mpsc::channel::<FetchJob>(5000);
    let rx = Arc::new(Mutex::new(rx));
    let queue_depth = Arc::new(AtomicUsize::new(0));
    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Lettura/0.1")
            .build()
            .unwrap_or_default(),
    );

    for _ in 0..concurrency {
        let rx = rx.clone();
        let pool = pool.clone();
        let storage = image_storage.clone();
        let rate_limiter = Arc::new(Mutex::new(DomainRateLimiter::new()));
        let depth = queue_depth.clone();
        let client = client.clone();

        tokio::spawn(async move {
            loop {
                let job = {
                    let mut rx = rx.lock().await;
                    rx.recv().await
                };
                match job {
                    Some(job) => {
                        process_job(&pool, &rate_limiter, &storage, &client, &job).await;
                        depth.fetch_sub(1, Ordering::Relaxed);
                    }
                    None => break,
                }
            }
        });
    }

    FetchQueue { tx, queue_depth }
}
```

Update `process_job` to accept `&Arc<reqwest::Client>` instead of creating one:

```rust
async fn process_job(
    pool: &PgPool,
    rate_limiter: &Arc<Mutex<DomainRateLimiter>>,
    image_storage: &Arc<dyn ImageStorage>,
    client: &reqwest::Client,
    job: &FetchJob,
) {
    // Remove the client creation inside process_job
    let fetch_result = client.get(&job.url).send().await;
    // ... rest same ...
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add src/tasks/fetcher.rs
git commit -m "perf: share reqwest Client across fetch jobs for connection pooling"
```

---

## Task 10: Move `AppState` to a dedicated `state.rs` module

**Files:**
- Create: `src/state.rs`
- Modify: `src/auth/middleware.rs`
- Modify: `src/lib.rs`
- Modify: All files that import `AppState` from `auth::middleware`

- [ ] **Step 1: Create `src/state.rs`**

```rust
use sqlx::PgPool;
use std::sync::Arc;

use crate::config::Config;
use crate::search::SearchIndex;
use crate::storage::ImageStorage;
use crate::tasks::fetcher::FetchQueue;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub fetch_queue: FetchQueue,
    pub search_index: SearchIndex,
    pub storage: Arc<dyn ImageStorage>,
}
```

- [ ] **Step 2: Update `src/lib.rs` to add `pub mod state;`**

- [ ] **Step 3: Update `src/auth/middleware.rs` to import `AppState` from `crate::state`**

Remove the `AppState` struct definition from `middleware.rs` and import it:

```rust
use crate::state::AppState;
```

- [ ] **Step 4: Update all imports across the codebase**

Find all files that import `AppState` from `crate::auth::middleware::AppState` and change them to `crate::state::AppState`. Files to update:
- `src/api/mod.rs`
- `src/api/entries.rs`
- `src/api/memos.rs`
- `src/api/annotations.rs`
- `src/api/tags.rs`
- `src/api/feed.rs`
- `src/api/import.rs`
- `src/api/export.rs`
- `src/api/admin.rs`
- `src/api/backup.rs`
- `src/api/site_rules.rs`
- `src/api/tagging_rules.rs`
- `src/api/pages.rs`
- `src/api/pages_public.rs`

- [ ] **Step 5: Run tests**

Run: `cargo test --lib`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/state.rs src/lib.rs src/auth/middleware.rs src/api/
git commit -m "refactor: extract AppState to dedicated state.rs module"
```
