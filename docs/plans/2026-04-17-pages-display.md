# 极简页面展示模块 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a lightweight HTML page hosting/display module to Lettura, allowing users to upload HTML+JS+CSS files and share them via public URLs like GitHub Pages.

**Architecture:** Server-side: new `pages` table in PostgreSQL, separate storage path for page files (isolated from `/storage/` route), independent `/p/{slug}` route tree with middleware-based password protection. Client-side: single list page with upload modal, integrated into existing navigation.

**Tech Stack:** Rust (Axum, SQLx, argon2, zip crate), React (TypeScript, TanStack Query, Tailwind CSS)

**Design Spec:** `docs/specs/2026-04-17-pages-display-design.md`

---

## File Structure

### New Files
- `migrations/012_create_pages.sql` — pages table
- `src/models/page.rs` — Page struct, CRUD queries, slug generation
- `src/api/pages.rs` — Admin API (upload, create, list, update, delete, restore)
- `src/api/pages_public.rs` — Public page serving, password auth, password input page HTML
- `tests/integration_pages.rs` — Integration tests for pages API
- `web/src/api/pages.ts` — Frontend API client
- `web/src/pages/PagesPage.tsx` — List page with upload modal
- `web/src/components/PageCard.tsx` — List card item
- `web/src/components/PageUploadModal.tsx` — Upload modal

### Modified Files
- `Cargo.toml` — Add `zip` dependency
- `src/config.rs` — Add `pages_storage_path` field
- `src/storage/mod.rs` — Add `get` method to `ImageStorage` trait
- `src/storage/local.rs` — Implement `get`
- `src/storage/oss.rs` — Implement `get`
- `src/models/mod.rs` — Add `page` module
- `src/api/mod.rs` — Register page routes, fix security headers, add `pages_storage` to AppState
- `src/auth/middleware.rs` — Add `pages_storage` to `AppState`
- `web/src/App.tsx` — Add `/pages` route
- `web/src/components/MobileNav.tsx` — Add "展示" nav link

---

### Task 1: Add Dependencies and Config

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/config.rs`
- Modify: `tests/common/mod.rs`

- [ ] **Step 1: Add `zip` crate to Cargo.toml**

In `[dependencies]` section, add after `tempfile = "3"`:

```toml
zip = "2"
```

- [ ] **Step 2: Add `pages_storage_path` to Config**

In `src/config.rs`, add field to `Config` struct after `storage_local_path`:

```rust
pub pages_storage_path: String,
```

In `Config::from_env()`, add after `storage_local_path`:

```rust
pages_storage_path: env::var("PAGES_STORAGE_PATH").unwrap_or_else(|_| "/data/pages".to_string()),
```

In `cleanup_env()` test helper, add:

```rust
env::remove_var("PAGES_STORAGE_PATH");
```

- [ ] **Step 3: Update TestApp config**

In `tests/common/mod.rs`, add to the `Config` struct literal:

```rust
pages_storage_path: "/tmp/lettura-test-pages".to_string(),
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: compiles successfully (field used in router_with_search next)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/config.rs tests/common/mod.rs
git commit -m "feat(pages): add zip dependency and pages_storage_path config"
```

---

### Task 2: Database Migration

**Files:**
- Create: `migrations/012_create_pages.sql`

- [ ] **Step 1: Create migration file**

```sql
-- Pages: lightweight HTML page hosting
CREATE TABLE pages (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        VARCHAR(12) NOT NULL UNIQUE,
    user_id     UUID NOT NULL REFERENCES users(id),
    title       VARCHAR(500) NOT NULL,
    description TEXT,
    entry_file  VARCHAR(500) NOT NULL,
    password    VARCHAR(255),
    status      VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    file_count  INTEGER NOT NULL DEFAULT 0,
    deleted_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_pages_slug ON pages(slug) WHERE deleted_at IS NULL;
CREATE INDEX idx_pages_user ON pages(user_id) WHERE deleted_at IS NULL;
```

- [ ] **Step 2: Verify migration runs**

Run: `cargo test --lib -- models` (or any test that triggers migration)
Expected: passes (migration applied automatically)

- [ ] **Step 3: Commit**

```bash
git add migrations/012_create_pages.sql
git commit -m "feat(pages): add pages table migration"
```

---

### Task 3: Extend ImageStorage Trait with `get`

**Files:**
- Modify: `src/storage/mod.rs`
- Modify: `src/storage/local.rs`
- Modify: `src/storage/oss.rs`

- [ ] **Step 1: Write failing test for `get`**

In `src/storage/local.rs`, add test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_store_and_get() {
        let dir = TempDir::new().unwrap();
        let storage = LocalStorage::new(dir.path().to_str().unwrap());
        storage.store("test/hello.txt", b"hello world", "text/plain").await.unwrap();
        let data = storage.get("test/hello.txt").await.unwrap();
        assert_eq!(data, Some(b"hello world".to_vec()));
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let dir = TempDir::new().unwrap();
        let storage = LocalStorage::new(dir.path().to_str().unwrap());
        let data = storage.get("nope.txt").await.unwrap();
        assert!(data.is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib storage::local`
Expected: FAIL (method `get` not found)

- [ ] **Step 3: Add `get` to trait in `src/storage/mod.rs`**

```rust
#[async_trait]
pub trait ImageStorage: Send + Sync {
    async fn store(&self, key: &str, data: &[u8], content_type: &str) -> Result<String, StorageError>;
    async fn delete(&self, key: &str) -> Result<(), StorageError>;
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;
}
```

- [ ] **Step 4: Implement `get` in `src/storage/local.rs`**

Add to `impl ImageStorage for LocalStorage`:

```rust
async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
    let file_path = self.base_path.join(key);
    match tokio::fs::read(&file_path).await {
        Ok(data) => Ok(Some(data)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(StorageError::Io(e.to_string())),
    }
}
```

- [ ] **Step 5: Implement `get` in `src/storage/oss.rs`**

Add to `impl ImageStorage for OssStorage`:

```rust
async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
    match self.bucket.get_object(key).await {
        Ok(data) => Ok(Some(data.to_vec())),
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            if msg.contains("no such key") || msg.contains("not found") || msg.contains("404") {
                Ok(None)
            } else {
                Err(StorageError::Io(e.to_string()))
            }
        }
    }
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --lib storage`
Expected: all tests pass

- [ ] **Step 7: Commit**

```bash
git add src/storage/
git commit -m "feat(storage): add get method to ImageStorage trait"
```

---

### Task 4: Page Model

**Files:**
- Create: `src/models/page.rs`
- Modify: `src/models/mod.rs`

- [ ] **Step 1: Create `src/models/page.rs`**

```rust
use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Page {
    pub id: Uuid,
    pub slug: String,
    pub user_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub entry_file: String,
    pub password: Option<String>,
    pub status: String,
    pub file_count: i32,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PageSummary {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub has_password: bool,
    pub status: String,
    pub file_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub fn generate_slug() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..12).map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char).collect()
}

pub async fn create_page(
    pool: &PgPool,
    user_id: Uuid,
    title: &str,
    description: Option<&str>,
    entry_file: &str,
    password_hash: Option<&str>,
    file_count: i32,
) -> Result<Page, ApiError> {
    let slug = generate_slug();
    let password = password_hash.map(|s| s.to_string());

    match sqlx::query_as::<_, Page>(
        "INSERT INTO pages (slug, user_id, title, description, entry_file, password, file_count)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *"
    )
    .bind(&slug).bind(user_id).bind(title).bind(description)
    .bind(entry_file).bind(&password).bind(file_count)
    .fetch_one(pool).await {
        Ok(page) => Ok(page),
        Err(sqlx::Error::Database(db_err)) if is_unique_violation(&db_err) => {
            // slug collision, retry with new slug (up to caller to loop)
            Err(ApiError::Conflict("slug collision, please retry".to_string()))
        }
        Err(e) => Err(ApiError::from(e)),
    }
}

fn is_unique_violation(db_err: &Box<dyn std::error::Error + Send + Sync>) -> bool {
    db_err.to_string().contains("pages_slug_key")
}

pub async fn create_page_with_retry(
    pool: &PgPool,
    user_id: Uuid,
    title: &str,
    description: Option<&str>,
    entry_file: &str,
    password_hash: Option<&str>,
    file_count: i32,
) -> Result<Page, ApiError> {
    for _ in 0..5 {
        match create_page(pool, user_id, title, description, entry_file, password_hash, file_count).await {
            Ok(page) => return Ok(page),
            Err(ApiError::Conflict(_)) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(ApiError::Internal("failed to generate unique slug".to_string()))
}

pub async fn find_page_by_id(pool: &PgPool, user_id: Uuid, page_id: Uuid) -> Result<Option<Page>, ApiError> {
    sqlx::query_as::<_, Page>("SELECT * FROM pages WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL")
        .bind(page_id).bind(user_id)
        .fetch_optional(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn find_page_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Page>, ApiError> {
    sqlx::query_as::<_, Page>(
        "SELECT * FROM pages WHERE slug = $1 AND deleted_at IS NULL AND status = 'active'"
    )
    .bind(slug)
    .fetch_optional(pool).await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn list_pages(
    pool: &PgPool,
    user_id: Uuid,
    status: Option<&str>,
    page: i64,
    limit: i64,
) -> Result<(Vec<PageSummary>, i64), ApiError> {
    let limit = limit.min(100).max(1);
    let offset = (page.max(1) - 1) * limit;

    let (items, count) = match status {
        Some("deleted") => {
            let items = sqlx::query_as::<_, PageSummary>(
                "SELECT id, slug, title, description, password IS NOT NULL as has_password, status, file_count, created_at, updated_at
                 FROM pages WHERE user_id = $1 AND deleted_at IS NOT NULL ORDER BY deleted_at DESC LIMIT $2 OFFSET $3"
            ).bind(user_id).bind(limit).bind(offset).fetch_all(pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pages WHERE user_id = $1 AND deleted_at IS NOT NULL")
                .bind(user_id).fetch_one(pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            (items, count)
        }
        Some(s) => {
            let items = sqlx::query_as::<_, PageSummary>(
                "SELECT id, slug, title, description, password IS NOT NULL as has_password, status, file_count, created_at, updated_at
                 FROM pages WHERE user_id = $1 AND deleted_at IS NULL AND status = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4"
            ).bind(user_id).bind(s).bind(limit).bind(offset).fetch_all(pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pages WHERE user_id = $1 AND deleted_at IS NULL AND status = $2")
                .bind(user_id).bind(s).fetch_one(pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            (items, count)
        }
        None => {
            let items = sqlx::query_as::<_, PageSummary>(
                "SELECT id, slug, title, description, password IS NOT NULL as has_password, status, file_count, created_at, updated_at
                 FROM pages WHERE user_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC LIMIT $2 OFFSET $3"
            ).bind(user_id).bind(limit).bind(offset).fetch_all(pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pages WHERE user_id = $1 AND deleted_at IS NULL")
                .bind(user_id).fetch_one(pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            (items, count)
        }
    };
    Ok((items, count))
}

#[derive(Debug, Deserialize)]
pub struct UpdatePageParams {
    pub title: Option<String>,
    pub description: Option<String>,
    pub password: Option<Option<String>>,
    pub status: Option<String>,
}

pub async fn update_page(
    pool: &PgPool,
    user_id: Uuid,
    page_id: Uuid,
    params: &UpdatePageParams,
) -> Result<Page, ApiError> {
    let existing = find_page_by_id(pool, user_id, page_id).await?
        .ok_or_else(|| ApiError::NotFound("page not found".to_string()))?;

    let title = params.title.as_deref().unwrap_or(&existing.title);
    let description = params.description.as_deref().or(existing.description.as_deref());
    let status = params.status.as_deref().unwrap_or(&existing.status);
    let password = match &params.password {
        Some(Some(pw)) => Some(crate::auth::password::hash_password(pw).map_err(|_| ApiError::Internal("hash failed".to_string()))?),
        Some(None) => None,
        None => existing.password.clone(),
    };

    sqlx::query_as::<_, Page>(
        "UPDATE pages SET title=$3, description=$4, status=$5, password=$6, updated_at=now() WHERE id=$1 AND user_id=$2 RETURNING *"
    )
    .bind(page_id).bind(user_id).bind(title).bind(description)
    .bind(status).bind(password)
    .fetch_one(pool).await.map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn delete_page(pool: &PgPool, user_id: Uuid, page_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("UPDATE pages SET deleted_at = now() WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL")
        .bind(page_id).bind(user_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

pub async fn restore_page(pool: &PgPool, user_id: Uuid, page_id: Uuid) -> Result<(), ApiError> {
    let result = sqlx::query("UPDATE pages SET deleted_at = NULL WHERE id = $1 AND user_id = $2 AND deleted_at IS NOT NULL")
        .bind(page_id).bind(user_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("page not found or not deleted".to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_slug_format() {
        let slug = generate_slug();
        assert_eq!(slug.len(), 12);
        assert!(slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    }

    #[test]
    fn test_generate_slug_uniqueness() {
        let slugs: std::collections::HashSet<String> = (0..100).map(|_| generate_slug()).collect();
        assert_eq!(slugs.len(), 100, "slugs should be unique");
    }
}
```

- [ ] **Step 2: Register module in `src/models/mod.rs`**

Add line:

```rust
pub mod page;
```

- [ ] **Step 3: Run unit tests**

Run: `cargo test --lib -- models::page`
Expected: 2 tests pass (slug generation)

- [ ] **Step 4: Commit**

```bash
git add src/models/page.rs src/models/mod.rs
git commit -m "feat(pages): add Page model with CRUD queries and slug generation"
```

---

### Task 5: Admin API — Upload Handler

**Files:**
- Create: `src/api/pages.rs`

This task implements the file upload, ZIP extraction, HTML parsing logic.

- [ ] **Step 1: Create `src/api/pages.rs` with upload handler**

```rust
use axum::extract::{Multipart, Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;
use validator::Validate;

use crate::api::error::ApiError;
use crate::auth::middleware::{AppState, AuthUser};
use crate::models::page;

use super::validate::ValidatedJson;

fn pages_storage_path(state: &AppState) -> PathBuf {
    PathBuf::from(&state.config.pages_storage_path)
}

fn tmp_dir(state: &AppState) -> PathBuf {
    PathBuf::from(&state.config.pages_storage_path).join("tmp")
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    upload_id: String,
    html_files: Vec<String>,
    default_entry: String,
    suggested_title: String,
    file_count: usize,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePageRequest {
    pub upload_id: String,
    pub entry_file: String,
    #[validate(length(min = 1, max = 500))]
    pub title: String,
    pub description: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListQueryParams {
    pub status: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePageRequest {
    #[validate(length(max = 500))]
    pub title: Option<String>,
    pub description: Option<String>,
    pub password: Option<Option<String>>,
    pub status: Option<String>,
}

#[tracing::instrument(skip(state, multipart), err)]
pub async fn upload_files(
    State(state): State<AppState>,
    auth: AuthUser,
    multipart: Multipart,
) -> Result<Json<UploadResponse>, ApiError> {
    let upload_id = Uuid::new_v4().to_string();
    let temp_base = tmp_dir(&state).join(&upload_id);
    tokio::fs::create_dir_all(&temp_base).await.map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut html_files: Vec<String> = Vec::new();
    let mut file_count: usize = 0;
    let mut saved_files: HashMap<String, Vec<u8>> = HashMap::new();

    let mut multipart = multipart;
    while let Some(field) = multipart.next_field().await.map_err(|e| ApiError::BadRequest(e.to_string()))? {
        let filename = field.file_name().unwrap_or("unknown").to_string();
        let data = field.bytes().await.map_err(|e| ApiError::BadRequest(e.to_string()))?;

        if filename.ends_with(".zip") {
            let extracted = extract_zip(&data)?;
            for (name, content) in extracted {
                let path = temp_base.join(&name);
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await.map_err(|e| ApiError::Internal(e.to_string()))?;
                }
                tokio::fs::write(&path, &content).await.map_err(|e| ApiError::Internal(e.to_string()))?;
                if name.ends_with(".html") {
                    html_files.push(name.clone());
                }
                saved_files.insert(name, content);
                file_count += 1;
            }
        } else {
            let safe_name = sanitize_filename(&filename);
            let path = temp_base.join(&safe_name);
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            }
            tokio::fs::write(&path, &data).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            if safe_name.ends_with(".html") {
                html_files.push(safe_name.clone());
            }
            saved_files.insert(safe_name, data.to_vec());
            file_count += 1;
        }
    }

    if file_count == 0 {
        tokio::fs::remove_dir_all(&temp_base).await.ok();
        return Err(ApiError::BadRequest("no files uploaded".to_string()));
    }

    if html_files.is_empty() {
        tokio::fs::remove_dir_all(&temp_base).await.ok();
        return Err(ApiError::BadRequest("no HTML files found".to_string()));
    }

    let default_entry = html_files.iter()
        .find(|f| f == "index.html" || f.ends_with("/index.html"))
        .or_else(|| html_files.first())
        .unwrap()
        .clone();

    let suggested_title = extract_title(saved_files.get(&default_entry).unwrap());

    // Schedule cleanup after 30 minutes
    let cleanup_path = temp_base.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1800)).await;
        tokio::fs::remove_dir_all(&cleanup_path).await.ok();
    });

    Ok(Json(UploadResponse {
        upload_id,
        html_files,
        default_entry,
        suggested_title,
        file_count,
    }))
}

fn extract_zip(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ApiError> {
    let reader = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| ApiError::BadRequest(format!("invalid zip: {e}")))?;
    let mut files = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| ApiError::Internal(e.to_string()))?;
        let name = entry.name().to_string();

        // Skip directories, hidden files, macOS metadata
        if name.ends_with('/') || name.starts_with('.') || name.contains("__MACOSX") || name.contains("/.") {
            continue;
        }
        // Security: no path traversal
        if name.contains("..") {
            continue;
        }

        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut content).map_err(|e| ApiError::Internal(e.to_string()))?;

        // Strip leading directory prefix if all files share one
        files.push((name, content));
    }

    // Strip common prefix if all files are under one directory
    strip_common_prefix(&mut files);

    Ok(files)
}

fn strip_common_prefix(files: &mut Vec<(String, Vec<u8>)>) {
    if files.is_empty() { return; }
    let first = &files[0].0;
    let slash_pos = first.find('/').unwrap_or(first.len());
    if slash_pos == first.len() { return; }
    let prefix = &first[..=slash_pos];
    if files.iter().all(|(n, _)| n.starts_with(prefix)) {
        for (name, _) in files.iter_mut() {
            *name = name[prefix.len()..].to_string();
        }
    }
}

fn sanitize_filename(name: &str) -> String {
    name.replace("..", "")
        .split('/')
        .filter(|s| !s.is_empty() && !s.starts_with('.'))
        .collect::<Vec<_>>()
        .join("/")
}

fn extract_title(html_content: &[u8]) -> String {
    let content = String::from_utf8_lossy(html_content);
    if let Some(start) = content.find("<title>") {
        if let Some(end) = content.find("</title>") {
            let title = content[start + 7..end].trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }
    // Fallback: also try <Title>, <TITLE> etc (case-insensitive)
    let lower = content.to_lowercase();
    if let Some(start) = lower.find("<title>") {
        if let Some(end) = lower.find("</title>") {
            let title = content[start + 7..end].trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }
    "Untitled".to_string()
}

#[tracing::instrument(skip(state), err)]
pub async fn create_page_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<CreatePageRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let temp_base = tmp_dir(&state).join(&req.upload_id);
    if !tokio::fs::try_exists(&temp_base).await.map_err(|e| ApiError::Internal(e.to_string()))? {
        return Err(ApiError::NotFound("upload session expired".to_string()));
    }

    // Count files in temp dir
    let file_count = count_files_recursive(&temp_base).await.map_err(|e| ApiError::Internal(e.to_string()))?;

    let password_hash = match &req.password {
        Some(pw) if !pw.is_empty() => Some(crate::auth::password::hash_password(pw).map_err(|_| ApiError::Internal("hash failed".to_string()))?),
        _ => None,
    };

    let new_page = page::create_page_with_retry(
        &state.pool, auth.user_id, &req.title,
        req.description.as_deref(), &req.entry_file,
        password_hash.as_deref(), file_count as i32,
    ).await?;

    // Move files from temp to pages storage
    let slug = new_page.slug.clone();
    let pages_base = pages_storage_path(&state).join(&slug);
    tokio::fs::create_dir_all(&pages_base).await.map_err(|e| ApiError::Internal(e.to_string()))?;

    // Copy all files from temp to pages storage
    copy_dir_recursive(&temp_base, &pages_base).await.map_err(|e| ApiError::Internal(e.to_string()))?;

    // Cleanup temp
    tokio::fs::remove_dir_all(&temp_base).await.ok();

    tracing::info!(page_id = %new_page.id, slug = %slug, "page created");

    Ok(Json(serde_json::json!({
        "id": new_page.id,
        "slug": new_page.slug,
        "title": new_page.title,
        "url": format!("/p/{}", new_page.slug),
        "created_at": new_page.created_at,
    })))
}

async fn count_files_recursive(dir: &std::path::Path) -> Result<usize, std::io::Error> {
    let mut count = 0;
    let mut entries = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            count += count_files_recursive(&path).await?;
        } else {
            count += 1;
        }
    }
    Ok(count)
}

async fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), std::io::Error> {
    tokio::fs::create_dir_all(dst).await?;
    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path).await?;
        } else {
            tokio::fs::copy(&src_path, &dst_path).await?;
        }
    }
    Ok(())
}

#[tracing::instrument(skip(state), err)]
pub async fn list_pages_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<ListQueryParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let page_num = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(20);
    let (items, total) = page::list_pages(
        &state.pool, auth.user_id,
        params.status.as_deref(), page_num, limit,
    ).await?;
    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": page_num,
        "limit": limit,
    })))
}

#[tracing::instrument(skip(state), err)]
pub async fn update_page_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<UpdatePageRequest>,
) -> Result<Json<page::Page>, ApiError> {
    if let Some(ref status) = req.status {
        if status != "active" && status != "disabled" {
            return Err(ApiError::BadRequest("status must be 'active' or 'disabled'".to_string()));
        }
    }
    let updated = page::update_page(
        &state.pool, auth.user_id, page_id,
        &page::UpdatePageParams {
            title: req.title,
            description: req.description,
            password: req.password,
            status: req.status,
        },
    ).await?;
    Ok(Json(updated))
}

#[tracing::instrument(skip(state), err)]
pub async fn delete_page_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = page::delete_page(&state.pool, auth.user_id, page_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("page not found".to_string()));
    }
    Ok(Json(serde_json::json!({"success": true})))
}

#[tracing::instrument(skip(state), err)]
pub async fn restore_page_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    page::restore_page(&state.pool, auth.user_id, page_id).await?;
    Ok(Json(serde_json::json!({"success": true})))
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: compiles (route registration in next task)

- [ ] **Step 3: Commit**

```bash
git add src/api/pages.rs
git commit -m "feat(pages): add admin API handlers (upload, create, list, update, delete)"
```

---

### Task 6: Public Page Serving + Password Auth

**Files:**
- Create: `src/api/pages_public.rs`

- [ ] **Step 1: Create `src/api/pages_public.rs`**

```rust
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::auth::middleware::AppState;
use crate::models::page;

type HmacSha256 = Hmac<Sha256>;

fn pages_base(state: &AppState) -> std::path::PathBuf {
    std::path::PathBuf::from(&state.config.pages_storage_path)
}

fn sign_cookie(state: &AppState, slug: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(state.config.jwt_secret.as_bytes()).unwrap();
    mac.update(slug.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn verify_cookie(state: &AppState, slug: &str, value: &str) -> bool {
    let mut mac = HmacSha256::new_from_slice(state.config.jwt_secret.as_bytes()).unwrap();
    mac.update(slug.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    sig == value
}

fn get_cookie_value(headers: &HeaderMap, slug: &str) -> Option<String> {
    let cookie_name = format!("page_auth_{}", slug);
    headers.get("cookie").and_then(|v| v.to_str().ok()).and_then(|cookies| {
        cookies.split(';')
            .map(|c| c.trim())
            .find(|c| c.starts_with(&format!("{}=", cookie_name)))
            .map(|c| c[cookie_name.len() + 1..].to_string())
    })
}

pub async fn serve_page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    headers: HeaderMap,
) -> Response {
    serve_page_file(&state, &slug, None, &headers).await
}

pub async fn serve_page_file(
    State(state): State<AppState>,
    Path((slug, file)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    serve_page_file_inner(&state, &slug, Some(&file), &headers).await
}

async fn serve_page_file_inner(
    state: &AppState,
    slug: &str,
    sub_path: Option<&str>,
    headers: &HeaderMap,
) -> Response {
    // Lookup page
    let page_record = match page::find_page_by_slug(&state.pool, slug).await {
        Ok(Some(p)) => p,
        _ => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };

    // Password check
    if page_record.password.is_some() {
        let authenticated = get_cookie_value(headers, slug)
            .map(|v| verify_cookie(state, slug, &v))
            .unwrap_or(false);
        if !authenticated {
            return render_password_page(slug, false);
        }
    }

    // Determine file path
    let file_name = match sub_path {
        Some(p) => p,
        None => &page_record.entry_file,
    };

    let file_path = pages_base(state).join(slug).join(file_name);

    match tokio::fs::read(&file_path).await {
        Ok(data) => {
            let mime = mime_for_file(file_name);
            (StatusCode::OK, [("content-type", mime)], data).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

#[derive(serde::Deserialize)]
pub struct AuthRequest {
    password: String,
}

pub async fn auth_page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<AuthRequest>,
) -> Response {
    let page_record = match page::find_page_by_slug(&state.pool, &slug).await {
        Ok(Some(p)) => p,
        _ => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };

    match &page_record.password {
        Some(hash) => {
            if crate::auth::password::verify_password(&form.password, hash).is_ok() {
                let sig = sign_cookie(&state, &slug);
                let cookie = format!(
                    "page_auth_{}={}; Path=/p/{}; Max-Age=86400; HttpOnly; SameSite=Lax",
                    slug, sig, slug
                );
                (
                    StatusCode::FOUND,
                    [
                        ("location", format!("/p/{}", slug)),
                        ("set-cookie", cookie),
                    ],
                ).into_response()
            } else {
                render_password_page(&slug, true)
            }
        }
        None => (
            StatusCode::FOUND,
            [("location", format!("/p/{}", slug))],
        ).into_response(),
    }
}

fn render_password_page(slug: &str, error: bool) -> Response {
    let error_html = if error {
        r#"<p style="color:#ef4444;margin-top:8px;font-size:14px;">密码错误</p>"#
    } else {
        ""
    };
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="zh"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>需要密码</title>
<style>
body{{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;display:flex;justify-content:center;align-items:center;min-height:100vh;margin:0;background:#f9fafb;color:#111827}}
.card{{background:#fff;border-radius:12px;box-shadow:0 1px 3px rgba(0,0,0,.1);padding:32px;width:100%;max-width:360px;text-align:center}}
h1{{font-size:18px;font-weight:600;margin:0 0 16px}}
input[type=password]{{width:100%;padding:10px 12px;border:1px solid #d1d5db;border-radius:8px;font-size:15px;box-sizing:border-box;outline:none}}
input[type=password]:focus{{border-color:#3b82f6;box-shadow:0 0 0 3px rgba(59,130,246,.1)}}
button{{margin-top:12px;width:100%;padding:10px;background:#3b82f6;color:#fff;border:none;border-radius:8px;font-size:15px;font-weight:500;cursor:pointer}}
button:hover{{background:#2563eb}}
</style></head><body>
<div class="card"><h1>此页面需要密码</h1>
<form method="POST" action="/p/{}/auth">
<input type="password" name="password" placeholder="请输入密码" autofocus required>{}
<button type="submit">确认</button>
</form></div></body></html>"#,
        error_html, slug
    );
    (
        StatusCode::OK,
        [("content-type", "text/html; charset=utf-8")],
        html,
    ).into_response()
}

fn mime_for_file(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "eot" => "application/vnd.ms-fontobject",
        "webmanifest" => "application/manifest+json",
        "xml" => "application/xml",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: compiles

- [ ] **Step 3: Commit**

```bash
git add src/api/pages_public.rs
git commit -m "feat(pages): add public page serving with password protection"
```

---

### Task 7: Route Integration + Security Headers Fix

**Files:**
- Modify: `src/api/mod.rs`
- Modify: `src/auth/middleware.rs`

- [ ] **Step 1: Add `pages_storage` to AppState**

In `src/auth/middleware.rs`, add field to `AppState`:

```rust
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub fetch_queue: FetchQueue,
    pub search_index: SearchIndex,
    pub storage: Arc<dyn ImageStorage>,
}
```

No change needed — we use `config.pages_storage_path` directly in handlers instead of a separate storage instance.

- [ ] **Step 2: Register routes in `src/api/mod.rs`**

Add module declarations at top of `mod.rs`:

```rust
pub mod pages;
pub mod pages_public;
```

In the `router_with_search` function, add routes. After the `.route("/storage/{*path}", ...)` line and before the SPA fallback, add the pages admin routes to the main router:

```rust
// Pages Admin API
.route("/api/v1/pages/upload", post(pages::upload_files))
.route("/api/v1/pages", get(pages::list_pages_handler).post(pages::create_page_handler))
.route("/api/v1/pages/{id}", patch(pages::update_page_handler).delete(pages::delete_page_handler))
.route("/api/v1/pages/{id}/restore", post(pages::restore_page_handler))
```

Then, for public page serving, create a separate Nest with different security headers. Before the `.fallback(...)` call, add:

```rust
// Public page serving (/p/{slug}) — separate nest for relaxed security headers
.nest("/p", {
    let page_router = Router::new()
        .route("/{slug}", get(pages_public::serve_page))
        .route("/{slug}/*file", get(pages_public::serve_page_file))
        .route("/{slug}/auth", post(pages_public::auth_page))
        .with_state(state.clone());
    // Override X-Frame-Options for page display (allow iframe embedding)
    page_router.layer(
        SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("SAMEORIGIN"),
        )
    )
})
```

- [ ] **Step 3: Fix global security headers to use `if_not_present`**

Change the existing global `SetResponseHeaderLayer` for `x-frame-options` from `overriding` to `if_not_present`:

```rust
.layer(SetResponseHeaderLayer::if_not_present(
    axum::http::header::HeaderName::from_static("x-frame-options"),
    HeaderValue::from_static("DENY"),
))
```

This way:
- Normal routes: no inner layer sets the header → global `if_not_present` sets DENY
- `/p/` routes: inner layer sets SAMEORIGIN → global `if_not_present` sees it already set → keeps SAMEORIGIN

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src/api/mod.rs src/auth/middleware.rs
git commit -m "feat(pages): register page routes and fix X-Frame-Options for public pages"
```

---

### Task 8: Integration Tests

**Files:**
- Create: `tests/integration_pages.rs`

- [ ] **Step 1: Create integration test file**

```rust
mod common;
use serde_json::json;

async fn get_auth_token(app: &common::TestApp) -> String {
    let res = app.client.post(app.url("/api/v1/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

fn auth_header(token: &str) -> String {
    format!("Bearer {}", token)
}

#[tokio::test]
async fn upload_and_create_page() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    // Upload HTML file
    let html_content = "<html><head><title>Test Page</title></head><body>Hello</body></html>";
    let res = app.client.post(app.url("/api/v1/pages/upload"))
        .header("Authorization", auth_header(&token))
        .multipart(reqwest::multipart::Form::new()
            .part("files", reqwest::multipart::Part::text(html_content)
                .file_name("index.html")
                .mime_str("text/html").unwrap()))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let upload: serde_json::Value = res.json().await.unwrap();
    assert_eq!(upload["html_files"].as_array().unwrap().len(), 1);
    assert_eq!(upload["default_entry"], "index.html");
    assert_eq!(upload["suggested_title"], "Test Page");
    assert!(upload["upload_id"].is_string());

    // Create page
    let upload_id = upload["upload_id"].as_str().unwrap();
    let res = app.client.post(app.url("/api/v1/pages"))
        .header("Authorization", auth_header(&token))
        .json(&json!({
            "upload_id": upload_id,
            "entry_file": "index.html",
            "title": "Test Page",
        }))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let page: serde_json::Value = res.json().await.unwrap();
    assert!(page["slug"].is_string());
    assert_eq!(page["title"], "Test Page");
    assert!(page["url"].as_str().unwrap().starts_with("/p/"));

    app.cleanup().await;
}

#[tokio::test]
async fn list_pages() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    // List should be empty initially
    let res = app.client.get(app.url("/api/v1/pages"))
        .header("Authorization", auth_header(&token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["total"], 0);

    app.cleanup().await;
}

#[tokio::test]
async fn public_access_page() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    // Upload and create
    let html_content = "<html><head><title>Public</title></head><body>Public content</body></html>";
    let upload_res = app.client.post(app.url("/api/v1/pages/upload"))
        .header("Authorization", auth_header(&token))
        .multipart(reqwest::multipart::Form::new()
            .part("files", reqwest::multipart::Part::text(html_content)
                .file_name("index.html").mime_str("text/html").unwrap()))
        .send().await.unwrap();
    let upload: serde_json::Value = upload_res.json().await.unwrap();

    let create_res = app.client.post(app.url("/api/v1/pages"))
        .header("Authorization", auth_header(&token))
        .json(&json!({"upload_id": upload["upload_id"], "entry_file": "index.html", "title": "Public"}))
        .send().await.unwrap();
    let page: serde_json::Value = create_res.json().await.unwrap();
    let slug = page["slug"].as_str().unwrap();

    // Access without auth
    let res = app.client.get(app.url(&format!("/p/{}", slug)))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    assert!(body.contains("Public content"));

    app.cleanup().await;
}

#[tokio::test]
async fn password_protected_page() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    // Upload and create with password
    let html_content = "<html><head><title>Secret</title></head><body>Secret content</body></html>";
    let upload_res = app.client.post(app.url("/api/v1/pages/upload"))
        .header("Authorization", auth_header(&token))
        .multipart(reqwest::multipart::Form::new()
            .part("files", reqwest::multipart::Part::text(html_content)
                .file_name("index.html").mime_str("text/html").unwrap()))
        .send().await.unwrap();
    let upload: serde_json::Value = upload_res.json().await.unwrap();

    let create_res = app.client.post(app.url("/api/v1/pages"))
        .header("Authorization", auth_header(&token))
        .json(&json!({
            "upload_id": upload["upload_id"],
            "entry_file": "index.html",
            "title": "Secret",
            "password": "mypass123"
        }))
        .send().await.unwrap();
    assert_eq!(create_res.status(), 200);
    let page: serde_json::Value = create_res.json().await.unwrap();
    let slug = page["slug"].as_str().unwrap();

    // Access without password → password page
    let res = app.client.get(app.url(&format!("/p/{}", slug)))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    assert!(body.contains("需要密码"));
    assert!(!body.contains("Secret content"));

    // Submit wrong password
    let res = app.client.post(app.url(&format!("/p/{}/auth", slug)))
        .form(&json!({"password": "wrong"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    assert!(body.contains("密码错误"));

    // Submit correct password → get cookie + redirect
    let res = app.client.post(app.url(&format!("/p/{}/auth", slug)))
        .form(&json!({"password": "mypass123"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 302);

    app.cleanup().await;
}

#[tokio::test]
async fn update_and_delete_page() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    // Create page first
    let html_content = "<html><body>Test</body></html>";
    let upload_res = app.client.post(app.url("/api/v1/pages/upload"))
        .header("Authorization", auth_header(&token))
        .multipart(reqwest::multipart::Form::new()
            .part("files", reqwest::multipart::Part::text(html_content)
                .file_name("index.html").mime_str("text/html").unwrap()))
        .send().await.unwrap();
    let upload: serde_json::Value = upload_res.json().await.unwrap();

    let create_res = app.client.post(app.url("/api/v1/pages"))
        .header("Authorization", auth_header(&token))
        .json(&json!({"upload_id": upload["upload_id"], "entry_file": "index.html", "title": "Original"}))
        .send().await.unwrap();
    let page: serde_json::Value = create_res.json().await.unwrap();
    let page_id = page["id"].as_str().unwrap();

    // Update title
    let res = app.client.patch(app.url(&format!("/api/v1/pages/{}", page_id)))
        .header("Authorization", auth_header(&token))
        .json(&json!({"title": "Updated"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let updated: serde_json::Value = res.json().await.unwrap();
    assert_eq!(updated["title"], "Updated");

    // Soft delete
    let res = app.client.delete(app.url(&format!("/api/v1/pages/{}", page_id)))
        .header("Authorization", auth_header(&token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);

    // List deleted
    let res = app.client.get(app.url("/api/v1/pages?status=deleted"))
        .header("Authorization", auth_header(&token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["total"], 1);

    // Restore
    let res = app.client.post(app.url(&format!("/api/v1/pages/{}/restore", page_id)))
        .header("Authorization", auth_header(&token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);

    app.cleanup().await;
}

#[tokio::test]
async fn upload_requires_auth() {
    let app = common::TestApp::new().await;
    let res = app.client.post(app.url("/api/v1/pages/upload"))
        .send().await.unwrap();
    assert_eq!(res.status(), 401);
    app.cleanup().await;
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test integration_pages`
Expected: all 6 tests pass

- [ ] **Step 3: Commit**

```bash
git add tests/integration_pages.rs
git commit -m "test(pages): add integration tests for pages API"
```

---

### Task 9: Frontend API Client

**Files:**
- Create: `web/src/api/pages.ts`

- [ ] **Step 1: Create API client**

```typescript
import api from './client';

export interface PageSummary {
  id: string;
  slug: string;
  title: string;
  description: string | null;
  has_password: boolean;
  status: string;
  file_count: number;
  created_at: string;
  updated_at: string;
}

export interface PageListResponse {
  items: PageSummary[];
  total: number;
  page: number;
  limit: number;
}

export interface UploadResponse {
  upload_id: string;
  html_files: string[];
  default_entry: string;
  suggested_title: string;
  file_count: number;
}

export interface CreatePageResponse {
  id: string;
  slug: string;
  title: string;
  url: string;
  created_at: string;
}

export async function uploadFiles(files: File[]): Promise<UploadResponse> {
  const formData = new FormData();
  files.forEach(f => formData.append('files', f));
  const res = await api.post('/pages/upload', formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
  });
  return res.data;
}

export async function createPage(data: {
  upload_id: string;
  entry_file: string;
  title: string;
  description?: string;
  password?: string;
}): Promise<CreatePageResponse> {
  const res = await api.post('/pages', data);
  return res.data;
}

export async function listPages(params?: {
  status?: string;
  page?: number;
  limit?: number;
}): Promise<PageListResponse> {
  const res = await api.get('/pages', { params });
  return res.data;
}

export async function updatePage(
  id: string,
  data: {
    title?: string;
    description?: string;
    password?: string | null;
    status?: string;
  }
): Promise<void> {
  await api.patch(`/pages/${id}`, data);
}

export async function deletePage(id: string): Promise<void> {
  await api.delete(`/pages/${id}`);
}

export async function restorePage(id: string): Promise<void> {
  await api.post(`/pages/${id}/restore`);
}
```

- [ ] **Step 2: Commit**

```bash
git add web/src/api/pages.ts
git commit -m "feat(pages): add frontend API client"
```

---

### Task 10: Frontend Components

**Files:**
- Create: `web/src/components/PageCard.tsx`
- Create: `web/src/components/PageUploadModal.tsx`
- Create: `web/src/pages/PagesPage.tsx`

- [ ] **Step 1: Create `PageCard.tsx`**

```tsx
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { updatePage, deletePage, restorePage, type PageSummary } from '../api/pages';
import { ExternalLink, Copy, Lock, Trash2, RotateCcw, EyeOff } from 'lucide-react';
import { toast } from './Toast';

function timeAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 60) return `${mins}分钟前`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}小时前`;
  const days = Math.floor(hrs / 24);
  return `${days}天前`;
}

export default function PageCard({ page }: { page: PageSummary }) {
  const qc = useQueryClient();
  const pageUrl = `${window.location.origin}/p/${page.slug}`;

  const toggleStatus = useMutation({
    mutationFn: () => updatePage(page.id, { status: page.status === 'active' ? 'disabled' : 'active' }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['pages'] });
      toast('success', page.status === 'active' ? '已禁用' : '已启用');
    },
  });

  const handleDelete = useMutation({
    mutationFn: () => deletePage(page.id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['pages'] });
      toast('success', '已删除');
    },
  });

  const handleRestore = useMutation({
    mutationFn: () => restorePage(page.id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['pages'] });
      toast('success', '已恢复');
    },
  });

  const copyLink = () => {
    navigator.clipboard.writeText(pageUrl);
    toast('success', '链接已复制');
  };

  const isDeleted = page.status === 'deleted';

  return (
    <div className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-xl p-4 hover:shadow-sm transition-all">
      <div className="flex items-start justify-between gap-3">
        <div className="flex-1 min-w-0">
          <h3 className="font-semibold text-gray-900 dark:text-gray-100 text-sm sm:text-base truncate">
            {page.title}
          </h3>
          <div className="flex items-center gap-2 mt-1 text-xs text-gray-500 dark:text-gray-500 flex-wrap">
            <button
              onClick={() => window.open(pageUrl, '_blank')}
              className="flex items-center gap-1 hover:text-blue-600 dark:hover:text-blue-400 font-mono"
            >
              /p/{page.slug}
            </button>
            {page.has_password && <Lock size={11} className="text-yellow-500 shrink-0" />}
            <span>{page.file_count} 个文件</span>
            <span>{timeAgo(page.created_at)}</span>
            {page.status === 'disabled' && (
              <span className="text-yellow-600 dark:text-yellow-500">已禁用</span>
            )}
          </div>
        </div>
        <div className="flex items-center gap-0.5 shrink-0">
          {!isDeleted && (
            <>
              <a
                href={pageUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="p-2 text-gray-400 dark:text-gray-600 hover:text-gray-600 dark:hover:text-gray-300 rounded-md transition-colors"
                title="新窗口打开"
              >
                <ExternalLink size={15} />
              </a>
              <button
                onClick={copyLink}
                className="p-2 text-gray-400 dark:text-gray-600 hover:text-gray-600 dark:hover:text-gray-300 rounded-md transition-colors"
                title="复制链接"
              >
                <Copy size={15} />
              </button>
              <button
                onClick={() => toggleStatus.mutate()}
                className="p-2 text-gray-400 dark:text-gray-600 hover:text-gray-600 dark:hover:text-gray-300 rounded-md transition-colors"
                title={page.status === 'active' ? '禁用' : '启用'}
              >
                <EyeOff size={15} />
              </button>
              <button
                onClick={() => handleDelete.mutate()}
                className="p-2 text-gray-400 dark:text-gray-600 hover:text-red-500 rounded-md transition-colors"
                title="删除"
              >
                <Trash2 size={15} />
              </button>
            </>
          )}
          {isDeleted && (
            <button
              onClick={() => handleRestore.mutate()}
              className="p-2 text-gray-400 dark:text-gray-600 hover:text-green-500 rounded-md transition-colors"
              title="恢复"
            >
              <RotateCcw size={15} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create `PageUploadModal.tsx`**

```tsx
import { useState, useRef, useCallback } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { uploadFiles, createPage } from '../api/pages';
import { Upload, X, Loader2, RefreshCw } from 'lucide-react';
import { toast } from './Toast';

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function PageUploadModal({ open, onClose }: Props) {
  const qc = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [files, setFiles] = useState<File[]>([]);
  const [uploadResult, setUploadResult] = useState<{
    upload_id: string;
    html_files: string[];
    default_entry: string;
    suggested_title: string;
    file_count: number;
  } | null>(null);
  const [entryFile, setEntryFile] = useState('');
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [password, setPassword] = useState('');
  const [dragOver, setDragOver] = useState(false);

  const handleFiles = useCallback(async (fileList: FileList | File[]) => {
    const arr = Array.from(fileList);
    setFiles(arr);
    try {
      const result = await uploadFiles(arr);
      setUploadResult(result);
      setEntryFile(result.default_entry);
      setTitle(result.suggested_title);
    } catch {
      toast('error', '上传失败');
    }
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    if (e.dataTransfer.files.length > 0) {
      handleFiles(e.dataTransfer.files);
    }
  }, [handleFiles]);

  const generatePassword = () => {
    const chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
    const pw = Array.from({ length: 8 }, () => chars[Math.floor(Math.random() * chars.length)]).join('');
    setPassword(pw);
  };

  const createMutation = useMutation({
    mutationFn: () => createPage({
      upload_id: uploadResult!.upload_id,
      entry_file: entryFile,
      title,
      description: description || undefined,
      password: password || undefined,
    }),
    onSuccess: (data) => {
      qc.invalidateQueries({ queryKey: ['pages'] });
      const url = `${window.location.origin}${data.url}`;
      navigator.clipboard.writeText(url);
      toast('success', `页面已发布，链接已复制: /p/${data.slug}`);
      handleClose();
    },
    onError: () => {
      toast('error', '创建失败');
    },
  });

  const handleClose = () => {
    setFiles([]);
    setUploadResult(null);
    setEntryFile('');
    setTitle('');
    setDescription('');
    setPassword('');
    onClose();
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="fixed inset-0 bg-black/40" onClick={handleClose} />
      <div className="relative bg-white dark:bg-gray-900 rounded-2xl shadow-2xl w-full max-w-lg max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-800">
          <h2 className="font-bold text-lg">上传页面</h2>
          <button onClick={handleClose} className="p-2 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-full">
            <X size={18} />
          </button>
        </div>
        <div className="p-4 space-y-4">
          {!uploadResult ? (
            <div
              onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
              onDragLeave={() => setDragOver(false)}
              onDrop={handleDrop}
              onClick={() => fileInputRef.current?.click()}
              className={`border-2 border-dashed rounded-xl p-8 text-center cursor-pointer transition-colors ${
                dragOver
                  ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20'
                  : 'border-gray-300 dark:border-gray-700 hover:border-gray-400 dark:hover:border-gray-600'
              }`}
            >
              <Upload size={32} className="mx-auto text-gray-400 mb-3" />
              <p className="text-sm text-gray-600 dark:text-gray-400">
                拖拽文件到此处，或点击选择
              </p>
              <p className="text-xs text-gray-400 mt-1">
                支持 HTML / CSS / JS / 图片 / ZIP
              </p>
              <input
                ref={fileInputRef}
                type="file"
                multiple
                accept=".html,.css,.js,.zip,.png,.jpg,.jpeg,.gif,.svg,.webp"
                className="hidden"
                onChange={(e) => e.target.files && handleFiles(e.target.files)}
              />
            </div>
          ) : (
            <>
              <div className="space-y-3">
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">入口文件</label>
                  {uploadResult.html_files.length > 1 ? (
                    <select
                      value={entryFile}
                      onChange={(e) => setEntryFile(e.target.value)}
                      className="w-full px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800 text-sm"
                    >
                      {uploadResult.html_files.map(f => (
                        <option key={f} value={f}>{f}</option>
                      ))}
                    </select>
                  ) : (
                    <p className="text-sm text-gray-600 dark:text-gray-400 font-mono">{entryFile}</p>
                  )}
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">标题</label>
                  <input
                    type="text"
                    value={title}
                    onChange={(e) => setTitle(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800 text-sm"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">描述（可选）</label>
                  <input
                    type="text"
                    value={description}
                    onChange={(e) => setDescription(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800 text-sm"
                    placeholder="可选的页面描述"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">访问密码（可选）</label>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      className="flex-1 px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800 text-sm font-mono"
                      placeholder="留空则无需密码"
                    />
                    <button
                      onClick={generatePassword}
                      className="px-3 py-2 text-sm border border-gray-300 dark:border-gray-700 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
                      title="自动生成密码"
                    >
                      <RefreshCw size={14} />
                    </button>
                  </div>
                </div>
                <p className="text-xs text-gray-400">{uploadResult.file_count} 个文件</p>
              </div>
              <button
                onClick={() => createMutation.mutate()}
                disabled={createMutation.isPending || !title}
                className="w-full py-2.5 bg-blue-600 text-white rounded-lg font-medium text-sm hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center justify-center gap-2"
              >
                {createMutation.isPending && <Loader2 size={16} className="animate-spin" />}
                发布
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Create `PagesPage.tsx`**

```tsx
import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { listPages, type PageSummary } from '../api/pages';
import PageCard from '../components/PageCard';
import PageUploadModal from '../components/PageUploadModal';
import { Plus, Loader2 } from 'lucide-react';

const TABS = [
  { key: 'active', label: '活跃' },
  { key: 'disabled', label: '已禁用' },
  { key: 'deleted', label: '已删除' },
] as const;

export default function PagesPage() {
  const [tab, setTab] = useState<string>('active');
  const [uploadOpen, setUploadOpen] = useState(false);

  const { data, isLoading } = useQuery({
    queryKey: ['pages', tab],
    queryFn: () => listPages({ status: tab }),
  });

  return (
    <>
      <div className="flex items-center justify-between mb-4">
        <div className="flex gap-1">
          {TABS.map(t => (
            <button
              key={t.key}
              onClick={() => setTab(t.key)}
              className={`px-3 py-1.5 rounded-md text-sm transition-colors ${
                tab === t.key
                  ? 'bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300 font-medium'
                  : 'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800'
              }`}
            >
              {t.label}
            </button>
          ))}
        </div>
        <button
          onClick={() => setUploadOpen(true)}
          className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 text-white rounded-lg text-sm font-medium hover:bg-blue-700 transition-colors"
        >
          <Plus size={15} />
          上传
        </button>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <Loader2 size={24} className="animate-spin text-gray-400" />
        </div>
      ) : data && data.items.length > 0 ? (
        <div className="space-y-3">
          {data.items.map((page: PageSummary) => (
            <PageCard key={page.id} page={page} />
          ))}
        </div>
      ) : (
        <div className="text-center py-12 text-gray-400">
          <p className="text-sm">暂无页面</p>
        </div>
      )}

      <PageUploadModal open={uploadOpen} onClose={() => setUploadOpen(false)} />
    </>
  );
}
```

- [ ] **Step 4: Commit**

```bash
git add web/src/components/PageCard.tsx web/src/components/PageUploadModal.tsx web/src/pages/PagesPage.tsx
git commit -m "feat(pages): add frontend page list, card, and upload modal components"
```

---

### Task 11: Frontend Routing + Navigation

**Files:**
- Modify: `web/src/App.tsx`
- Modify: `web/src/components/MobileNav.tsx`

- [ ] **Step 1: Add route in `App.tsx`**

Add lazy import at top:

```tsx
const PagesPage = lazy(() => import('./pages/PagesPage'));
```

Add route inside the `<ProtectedRoute>` wrapper, after the memos route:

```tsx
<Route path="pages" element={<PagesPage />} />
```

- [ ] **Step 2: Add nav link in `MobileNav.tsx`**

In the `links` array, add:

```typescript
{ to: '/pages', label: '展示', end: false },
```

- [ ] **Step 3: Verify frontend builds**

Run: `cd web && pnpm run build`
Expected: builds successfully

- [ ] **Step 4: Commit**

```bash
git add web/src/App.tsx web/src/components/MobileNav.tsx
git commit -m "feat(pages): add /pages route and navigation entry"
```

---

### Task 12: Full Verification

- [ ] **Step 1: Run backend tests**

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

Expected: all pass

- [ ] **Step 2: Run frontend build**

```bash
cd web && pnpm install --frozen-lockfile && pnpm run build
```

Expected: builds successfully

- [ ] **Step 3: Final commit if any fixes needed**
