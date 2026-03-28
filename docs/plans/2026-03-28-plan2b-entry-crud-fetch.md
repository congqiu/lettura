# Plan 2b: Entry CRUD + 抓取队列

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 Entry 的完整 CRUD API，集成内容提取引擎，实现后台抓取队列（内存队列 + tokio worker + per-domain 速率限制）。

**Architecture:** Entry 创建时 URL 进入内存抓取队列，后台 tokio task 消费队列，调用 reqwest 抓取 HTML，再调用 extract 模块提取内容，结果写回 DB。per-domain 令牌桶限速。

**Tech Stack:** Axum (已有), SQLx (已有), reqwest, tokio, extract 模块 (已有)

**编译/测试：** 远程 Docker (`rust:latest`) + PostgreSQL (`docker compose up -d postgres`)

---

## 文件结构

```
lettura/
├── migrations/
│   └── 003_create_entries.sql        — entries 表
├── src/
│   ├── models/
│   │   ├── mod.rs                    — 添加 entry
│   │   └── entry.rs                  — Entry 模型 + CRUD 查询
│   ├── api/
│   │   ├── mod.rs                    — 添加 entries 路由
│   │   └── entries.rs                — Entry API handler
│   └── tasks/
│       ├── mod.rs                    — re-export
│       └── fetcher.rs                — 抓取队列 + worker
├── tests/
│   └── integration_entries.rs        — Entry 集成测试
```

---

### Task 1: entries 数据库迁移

**Files:**
- Create: `migrations/003_create_entries.sql`

- [ ] **Step 1: 创建 entries 迁移**

```sql
CREATE TABLE entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    given_url TEXT NOT NULL,
    hashed_url VARCHAR(40) NOT NULL,
    hashed_given_url VARCHAR(40) NOT NULL,
    title TEXT,
    content TEXT,
    text_content TEXT,
    content_type VARCHAR(20) NOT NULL DEFAULT 'article',
    extract_method VARCHAR(20) NOT NULL DEFAULT 'pending',
    is_content_edited BOOLEAN NOT NULL DEFAULT false,
    language VARCHAR(20),
    http_status SMALLINT,
    reading_time INT,
    preview_picture TEXT,
    domain_name VARCHAR(255),
    published_by TEXT,
    metadata JSONB DEFAULT '{}',
    is_archived BOOLEAN NOT NULL DEFAULT false,
    archived_at TIMESTAMPTZ,
    is_starred BOOLEAN NOT NULL DEFAULT false,
    starred_at TIMESTAMPTZ,
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX idx_entries_user_hashed_url ON entries (user_id, hashed_url);
CREATE INDEX idx_entries_user_hashed_given_url ON entries (user_id, hashed_given_url);
CREATE INDEX idx_entries_user_created ON entries (user_id, created_at DESC);
CREATE INDEX idx_entries_user_archived ON entries (user_id, is_archived, archived_at DESC);
CREATE INDEX idx_entries_user_starred ON entries (user_id, is_starred, starred_at DESC);
CREATE INDEX idx_entries_domain ON entries (domain_name, user_id);
CREATE INDEX idx_entries_user_language ON entries (user_id, language);
```

- [ ] **Step 2: 验证编译通过**

Run: `cargo check`

- [ ] **Step 3: Commit**

```bash
git add migrations/003_create_entries.sql
git commit -m "feat: add entries table migration with indexes"
```

---

### Task 2: Entry 模型与查询

**Files:**
- Create: `src/models/entry.rs`
- Modify: `src/models/mod.rs`

- [ ] **Step 1: 实现 Entry 模型**

`src/models/entry.rs`:
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Entry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub given_url: String,
    pub hashed_url: String,
    pub hashed_given_url: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub text_content: Option<String>,
    pub content_type: String,
    pub extract_method: String,
    pub is_content_edited: bool,
    pub language: Option<String>,
    pub http_status: Option<i16>,
    pub reading_time: Option<i32>,
    pub preview_picture: Option<String>,
    pub domain_name: Option<String>,
    pub published_by: Option<String>,
    pub metadata: serde_json::Value,
    pub is_archived: bool,
    pub archived_at: Option<DateTime<Utc>>,
    pub is_starred: bool,
    pub starred_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Summary for list view (without content)
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct EntrySummary {
    pub id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub title: Option<String>,
    pub content_type: String,
    pub extract_method: String,
    pub language: Option<String>,
    pub reading_time: Option<i32>,
    pub preview_picture: Option<String>,
    pub domain_name: Option<String>,
    pub published_by: Option<String>,
    pub is_archived: bool,
    pub is_starred: bool,
    pub created_at: DateTime<Utc>,
}

pub fn hash_url(url: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(url.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn extract_domain(url_str: &str) -> Option<String> {
    Url::parse(url_str).ok().and_then(|u| u.host_str().map(String::from))
}

pub async fn create_entry(
    pool: &PgPool,
    user_id: Uuid,
    given_url: &str,
) -> Result<Entry, ApiError> {
    let url = given_url.to_string(); // TODO: follow redirects in fetcher
    let hashed_url = hash_url(&url);
    let hashed_given_url = hash_url(given_url);
    let domain_name = extract_domain(&url);

    sqlx::query_as::<_, Entry>(
        r#"
        INSERT INTO entries (user_id, url, given_url, hashed_url, hashed_given_url, domain_name)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(&url)
    .bind(given_url)
    .bind(&hashed_url)
    .bind(&hashed_given_url)
    .bind(&domain_name)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err)
            if db_err.constraint() == Some("idx_entries_user_hashed_url") =>
        {
            ApiError::Conflict("URL already saved".to_string())
        }
        _ => ApiError::Internal(e.to_string()),
    })
}

pub async fn find_entry_by_id(
    pool: &PgPool,
    user_id: Uuid,
    entry_id: Uuid,
) -> Result<Option<Entry>, ApiError> {
    sqlx::query_as::<_, Entry>("SELECT * FROM entries WHERE id = $1 AND user_id = $2")
        .bind(entry_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub is_archived: Option<bool>,
    pub is_starred: Option<bool>,
    pub domain: Option<String>,
}

pub async fn list_entries(
    pool: &PgPool,
    user_id: Uuid,
    params: &ListParams,
) -> Result<Vec<EntrySummary>, ApiError> {
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (params.page.unwrap_or(1) - 1).max(0) * per_page;

    let mut sql = String::from(
        "SELECT id, user_id, url, title, content_type, extract_method, language, \
         reading_time, preview_picture, domain_name, published_by, is_archived, \
         is_starred, created_at \
         FROM entries WHERE user_id = $1",
    );
    let mut param_idx = 2u32;

    if params.is_archived.is_some() {
        sql.push_str(&format!(" AND is_archived = ${}", param_idx));
        param_idx += 1;
    }
    if params.is_starred.is_some() {
        sql.push_str(&format!(" AND is_starred = ${}", param_idx));
        param_idx += 1;
    }
    if params.domain.is_some() {
        sql.push_str(&format!(" AND domain_name = ${}", param_idx));
        param_idx += 1;
    }

    sql.push_str(&format!(
        " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
        param_idx,
        param_idx + 1
    ));

    let mut query = sqlx::query_as::<_, EntrySummary>(&sql).bind(user_id);

    if let Some(archived) = params.is_archived {
        query = query.bind(archived);
    }
    if let Some(starred) = params.is_starred {
        query = query.bind(starred);
    }
    if let Some(ref domain) = params.domain {
        query = query.bind(domain);
    }

    query = query.bind(per_page).bind(offset);

    query
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

#[derive(Debug, Deserialize)]
pub struct UpdateEntryParams {
    pub title: Option<String>,
    pub content: Option<String>,
    pub is_archived: Option<bool>,
    pub is_starred: Option<bool>,
}

pub async fn update_entry(
    pool: &PgPool,
    user_id: Uuid,
    entry_id: Uuid,
    params: &UpdateEntryParams,
) -> Result<Entry, ApiError> {
    let existing = find_entry_by_id(pool, user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;

    let title = params.title.as_deref().unwrap_or(existing.title.as_deref().unwrap_or(""));
    let content = params.content.as_deref().or(existing.content.as_deref());
    let is_content_edited = if params.content.is_some() {
        true
    } else {
        existing.is_content_edited
    };

    let is_archived = params.is_archived.unwrap_or(existing.is_archived);
    let archived_at = if params.is_archived == Some(true) && !existing.is_archived {
        Some(Utc::now())
    } else if params.is_archived == Some(false) {
        None
    } else {
        existing.archived_at
    };

    let is_starred = params.is_starred.unwrap_or(existing.is_starred);
    let starred_at = if params.is_starred == Some(true) && !existing.is_starred {
        Some(Utc::now())
    } else if params.is_starred == Some(false) {
        None
    } else {
        existing.starred_at
    };

    sqlx::query_as::<_, Entry>(
        r#"
        UPDATE entries SET
            title = $3, content = $4, is_content_edited = $5,
            is_archived = $6, archived_at = $7,
            is_starred = $8, starred_at = $9,
            updated_at = now()
        WHERE id = $1 AND user_id = $2
        RETURNING *
        "#,
    )
    .bind(entry_id)
    .bind(user_id)
    .bind(title)
    .bind(content)
    .bind(is_content_edited)
    .bind(is_archived)
    .bind(archived_at)
    .bind(is_starred)
    .bind(starred_at)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn delete_entry(
    pool: &PgPool,
    user_id: Uuid,
    entry_id: Uuid,
) -> Result<bool, ApiError> {
    let result =
        sqlx::query("DELETE FROM entries WHERE id = $1 AND user_id = $2")
            .bind(entry_id)
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

/// Called by fetcher to update entry with extracted content
pub async fn update_entry_content(
    pool: &PgPool,
    entry_id: Uuid,
    title: Option<&str>,
    content: Option<&str>,
    text_content: Option<&str>,
    language: Option<&str>,
    preview_picture: Option<&str>,
    published_by: Option<&str>,
    reading_time: Option<i32>,
    http_status: i16,
    extract_method: &str,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        UPDATE entries SET
            title = COALESCE($2, title),
            content = $3, text_content = $4,
            language = $5, preview_picture = $6,
            published_by = $7, reading_time = $8,
            http_status = $9, extract_method = $10,
            updated_at = now()
        WHERE id = $1 AND is_content_edited = false
        "#,
    )
    .bind(entry_id)
    .bind(title)
    .bind(content)
    .bind(text_content)
    .bind(language)
    .bind(preview_picture)
    .bind(published_by)
    .bind(reading_time)
    .bind(http_status)
    .bind(extract_method)
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}
```

注意: 需要在 `Cargo.toml` 添加 `sha1` 依赖:
```toml
sha1 = "0.10"
```

- [ ] **Step 2: 更新 src/models/mod.rs**

```rust
pub mod entry;
pub mod user;
```

- [ ] **Step 3: 验证编译通过**

Run: `cargo check`

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml migrations/003_create_entries.sql src/models/
git commit -m "feat: add entries migration and Entry model with CRUD queries"
```

---

### Task 3: 抓取队列 + Worker

**Files:**
- Create: `src/tasks/mod.rs`
- Create: `src/tasks/fetcher.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: 实现抓取队列**

`src/tasks/mod.rs`:
```rust
pub mod fetcher;
```

`src/tasks/fetcher.rs`:
```rust
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::extract;
use crate::models::entry;

#[derive(Debug, Clone)]
pub struct FetchJob {
    pub entry_id: Uuid,
    pub url: String,
}

#[derive(Clone)]
pub struct FetchQueue {
    tx: mpsc::Sender<FetchJob>,
}

impl FetchQueue {
    pub async fn send(&self, job: FetchJob) -> Result<(), String> {
        self.tx.send(job).await.map_err(|e| e.to_string())
    }
}

/// Start the fetch worker and return the queue handle
pub fn start_fetch_worker(pool: PgPool, concurrency: usize) -> FetchQueue {
    let (tx, rx) = mpsc::channel::<FetchJob>(5000);
    let rx = Arc::new(Mutex::new(rx));

    for _ in 0..concurrency {
        let rx = rx.clone();
        let pool = pool.clone();
        let rate_limiter = Arc::new(Mutex::new(DomainRateLimiter::new()));

        tokio::spawn(async move {
            loop {
                let job = {
                    let mut rx = rx.lock().await;
                    rx.recv().await
                };

                match job {
                    Some(job) => {
                        process_job(&pool, &rate_limiter, &job).await;
                    }
                    None => break, // channel closed
                }
            }
        });
    }

    FetchQueue { tx }
}

async fn process_job(
    pool: &PgPool,
    rate_limiter: &Arc<Mutex<DomainRateLimiter>>,
    job: &FetchJob,
) {
    // Rate limit per domain
    if let Some(domain) = entry::extract_domain(&job.url) {
        let mut rl = rate_limiter.lock().await;
        rl.wait_if_needed(&domain).await;
    }

    // Fetch HTML
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Lettura/0.1")
        .build()
        .unwrap_or_default();

    let fetch_result = client.get(&job.url).send().await;

    match fetch_result {
        Ok(response) => {
            let status = response.status().as_u16() as i16;
            match response.text().await {
                Ok(html) => {
                    // Extract content
                    match extract::extract(&html, Some(&job.url)) {
                        Ok(result) => {
                            entry::update_entry_content(
                                pool,
                                job.entry_id,
                                result.title.as_deref(),
                                Some(&result.content),
                                Some(&result.text_content),
                                result.language.as_deref(),
                                result.preview_image.as_deref(),
                                result.author.as_deref(),
                                Some(result.reading_time as i32),
                                status,
                                "readability",
                            )
                            .await
                            .ok();
                        }
                        Err(_) => {
                            entry::update_entry_content(
                                pool,
                                job.entry_id,
                                None, None, None, None, None, None, None,
                                status,
                                "failed",
                            )
                            .await
                            .ok();
                        }
                    }
                }
                Err(_) => {
                    entry::update_entry_content(
                        pool, job.entry_id,
                        None, None, None, None, None, None, None,
                        status, "failed",
                    )
                    .await
                    .ok();
                }
            }
        }
        Err(_) => {
            entry::update_entry_content(
                pool, job.entry_id,
                None, None, None, None, None, None, None,
                0, "failed",
            )
            .await
            .ok();
        }
    }
}

/// Simple per-domain rate limiter: 1 request per second per domain
struct DomainRateLimiter {
    last_request: HashMap<String, Instant>,
}

impl DomainRateLimiter {
    fn new() -> Self {
        Self {
            last_request: HashMap::new(),
        }
    }

    async fn wait_if_needed(&mut self, domain: &str) {
        if let Some(last) = self.last_request.get(domain) {
            let elapsed = last.elapsed();
            if elapsed < std::time::Duration::from_secs(1) {
                tokio::time::sleep(std::time::Duration::from_secs(1) - elapsed).await;
            }
        }
        self.last_request.insert(domain.to_string(), Instant::now());

        // LRU cleanup: keep max 500 domains
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
```

- [ ] **Step 2: 更新 src/lib.rs 添加 tasks 模块**

```rust
pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod extract;
pub mod models;
pub mod tasks;
```

- [ ] **Step 3: 添加 reqwest 到 dependencies (非 dev)**

在 `Cargo.toml` `[dependencies]` 中添加:
```toml
reqwest = { version = "0.12", features = ["json"] }
sha1 = "0.10"
```

并从 `[dev-dependencies]` 中删除 reqwest（它现在是正式依赖了）。

- [ ] **Step 4: 验证编译通过**

Run: `cargo check`

- [ ] **Step 5: Commit**

```bash
git add src/tasks/ src/lib.rs Cargo.toml
git commit -m "feat: implement fetch queue with per-domain rate limiting"
```

---

### Task 4: Entry API 端点

**Files:**
- Create: `src/api/entries.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/auth/middleware.rs` (添加 FetchQueue 到 AppState)

- [ ] **Step 1: 更新 AppState 添加 FetchQueue**

在 `src/auth/middleware.rs` 的 `AppState` 中添加 `fetch_queue` 字段:
```rust
use crate::tasks::fetcher::FetchQueue;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub fetch_queue: FetchQueue,
}
```

- [ ] **Step 2: 实现 entries handler**

`src/api/entries.rs`:
```rust
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::{AppState, AuthUser};
use crate::models::entry::{self, ListParams, UpdateEntryParams};
use crate::tasks::fetcher::FetchJob;

#[derive(Deserialize)]
pub struct CreateEntryRequest {
    pub url: String,
}

pub async fn create_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateEntryRequest>,
) -> Result<Json<entry::Entry>, ApiError> {
    if req.url.is_empty() {
        return Err(ApiError::BadRequest("url is required".to_string()));
    }

    // Validate URL
    url::Url::parse(&req.url)
        .map_err(|_| ApiError::BadRequest("invalid URL".to_string()))?;

    let new_entry = entry::create_entry(&state.pool, auth.user_id, &req.url).await?;

    // Queue fetch job
    let _ = state
        .fetch_queue
        .send(FetchJob {
            entry_id: new_entry.id,
            url: new_entry.url.clone(),
        })
        .await;

    Ok(Json(new_entry))
}

pub async fn get_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<entry::Entry>, ApiError> {
    let found = entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;
    Ok(Json(found))
}

pub async fn list_entries(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<entry::EntrySummary>>, ApiError> {
    let entries = entry::list_entries(&state.pool, auth.user_id, &params).await?;
    Ok(Json(entries))
}

pub async fn update_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
    Json(params): Json<UpdateEntryParams>,
) -> Result<Json<entry::Entry>, ApiError> {
    let updated = entry::update_entry(&state.pool, auth.user_id, entry_id, &params).await?;
    Ok(Json(updated))
}

pub async fn delete_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = entry::delete_entry(&state.pool, auth.user_id, entry_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("entry not found".to_string()));
    }
    Ok(Json(serde_json::json!({"message": "deleted"})))
}

pub async fn refetch_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let found = entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;

    if found.is_content_edited {
        return Err(ApiError::BadRequest(
            "cannot refetch edited content".to_string(),
        ));
    }

    let _ = state
        .fetch_queue
        .send(FetchJob {
            entry_id: found.id,
            url: found.url.clone(),
        })
        .await;

    Ok(Json(serde_json::json!({"message": "refetch queued"})))
}
```

- [ ] **Step 3: 更新路由**

`src/api/mod.rs`:
```rust
use axum::{
    routing::{delete, get, patch, post},
    Router,
};
use sqlx::PgPool;

use crate::auth::middleware::AppState;
use crate::config::Config;
use crate::tasks::fetcher;

pub mod auth;
pub mod entries;
pub mod error;

pub fn router(pool: PgPool, config: Config) -> Router {
    let fetch_queue = fetcher::start_fetch_worker(pool.clone(), 5);

    let state = AppState {
        pool,
        config,
        fetch_queue,
    };

    Router::new()
        // Auth
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/refresh", post(auth::refresh))
        .route("/api/auth/logout", post(auth::logout))
        // Entries
        .route("/api/entries", get(entries::list_entries).post(entries::create_entry))
        .route(
            "/api/entries/{id}",
            get(entries::get_entry)
                .patch(entries::update_entry)
                .delete(entries::delete_entry),
        )
        .route("/api/entries/{id}/refetch", post(entries::refetch_entry))
        .with_state(state)
}
```

- [ ] **Step 4: 更新 main.rs**

`src/main.rs` 不需要改动（router 签名不变）。

- [ ] **Step 5: 验证编译通过**

Run: `cargo check`

- [ ] **Step 6: Commit**

```bash
git add src/api/ src/auth/middleware.rs
git commit -m "feat: implement Entry CRUD API endpoints with fetch queue integration"
```

---

### Task 5: 集成测试

**Files:**
- Create: `tests/integration_entries.rs`

- [ ] **Step 1: 编写 Entry 集成测试**

```rust
mod common;
use serde_json::json;

/// Helper: register + return access token
async fn get_auth_token(app: &common::TestApp) -> String {
    let res = app
        .client
        .post(app.url("/api/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn create_entry_returns_201() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    let res = app
        .client
        .post(app.url("/api/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/article"}))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["given_url"], "https://example.com/article");
    assert!(body["id"].is_string());
    assert_eq!(body["extract_method"], "pending");

    app.cleanup().await;
}

#[tokio::test]
async fn duplicate_url_returns_conflict() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    app.client
        .post(app.url("/api/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/dup"}))
        .send()
        .await
        .unwrap();

    let res = app
        .client
        .post(app.url("/api/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/dup"}))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 409);
    app.cleanup().await;
}

#[tokio::test]
async fn list_entries_empty() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    let res = app
        .client
        .get(app.url("/api/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: Vec<serde_json::Value> = res.json().await.unwrap();
    assert!(body.is_empty());

    app.cleanup().await;
}

#[tokio::test]
async fn get_entry_by_id() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    let res = app
        .client
        .post(app.url("/api/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/get-test"}))
        .send()
        .await
        .unwrap();
    let created: serde_json::Value = res.json().await.unwrap();
    let entry_id = created["id"].as_str().unwrap();

    let res = app
        .client
        .get(app.url(&format!("/api/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["id"], entry_id);

    app.cleanup().await;
}

#[tokio::test]
async fn update_entry_star_and_archive() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    let res = app
        .client
        .post(app.url("/api/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/update-test"}))
        .send()
        .await
        .unwrap();
    let created: serde_json::Value = res.json().await.unwrap();
    let entry_id = created["id"].as_str().unwrap();

    // Star it
    let res = app
        .client
        .patch(app.url(&format!("/api/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"is_starred": true}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["is_starred"], true);
    assert!(body["starred_at"].is_string());

    // Archive it
    let res = app
        .client
        .patch(app.url(&format!("/api/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"is_archived": true}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["is_archived"], true);

    app.cleanup().await;
}

#[tokio::test]
async fn delete_entry() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    let res = app
        .client
        .post(app.url("/api/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/delete-test"}))
        .send()
        .await
        .unwrap();
    let created: serde_json::Value = res.json().await.unwrap();
    let entry_id = created["id"].as_str().unwrap();

    let res = app
        .client
        .delete(app.url(&format!("/api/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // Should be gone
    let res = app
        .client
        .get(app.url(&format!("/api/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);

    app.cleanup().await;
}

#[tokio::test]
async fn unauthenticated_request_rejected() {
    let app = common::TestApp::new().await;

    let res = app
        .client
        .get(app.url("/api/entries"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
    app.cleanup().await;
}
```

- [ ] **Step 2: 运行集成测试**

```bash
docker run --rm --network=host \
  -v "$HOME/workspace/lettura":/app \
  -v lettura-cargo-registry:/usr/local/cargo/registry \
  -v lettura-cargo-target:/app/target \
  -w /app \
  -e DATABASE_URL=postgres://lettura:lettura@127.0.0.1:5432/lettura \
  rust:latest \
  cargo test --test integration_entries 2>&1
```

Expected: 7 tests PASS

- [ ] **Step 3: 运行全部测试**

```bash
cargo test
```

Expected: 所有单元测试 + auth 集成测试 + entries 集成测试 PASS

- [ ] **Step 4: Commit**

```bash
git add tests/integration_entries.rs
git commit -m "feat: add Entry CRUD integration tests"
```
