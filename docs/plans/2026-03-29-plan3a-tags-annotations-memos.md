# Plan 3a: Tags + Annotations + Memos

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现标签系统（tags + entry_tags）、文章注释/高亮（annotations）、快速捕获收集箱（memos + promote to entry）。

**Architecture:** 三个独立的 CRUD 子系统，各自有独立的迁移、模型、API handler。Tags 通过 entry_tags 关联表与 entries 多对多关联。Memos 可通过 promote 操作转化为 Entry（如果包含 URL 则触发抓取）。Annotations 绑定到具体 entry，当 entry content 被编辑时标记为 orphaned。

**Tech Stack:** Axum (已有), SQLx (已有), 遵循现有代码模式

---

## 文件结构

```
lettura/
├── migrations/
│   ├── 004_create_tags.sql
│   ├── 005_create_annotations.sql
│   └── 006_create_memos.sql
├── src/
│   ├── models/
│   │   ├── mod.rs              — 添加 tag, annotation, memo
│   │   ├── tag.rs              — Tag 模型 + entry_tags CRUD
│   │   ├── annotation.rs       — Annotation 模型 + CRUD
│   │   └── memo.rs             — Memo 模型 + CRUD + promote
│   ├── api/
│   │   ├── mod.rs              — 添加路由
│   │   ├── tags.rs             — Tag API handler
│   │   ├── annotations.rs      — Annotation API handler
│   │   └── memos.rs            — Memo API handler
├── tests/
│   ├── integration_tags.rs
│   ├── integration_annotations.rs
│   └── integration_memos.rs
```

---

### Task 1: Tags 迁移 + 模型

**Files:**
- Create: `migrations/004_create_tags.sql`
- Create: `src/models/tag.rs`
- Modify: `src/models/mod.rs`

- [ ] **Step 1: 创建 tags 迁移**

`migrations/004_create_tags.sql`:
```sql
CREATE TABLE tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    label VARCHAR(100) NOT NULL,
    slug VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX idx_tags_user_slug ON tags (user_id, slug);

CREATE TABLE entry_tags (
    entry_id UUID NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (entry_id, tag_id)
);

CREATE INDEX idx_entry_tags_tag ON entry_tags (tag_id);
```

- [ ] **Step 2: 实现 Tag 模型**

`src/models/tag.rs`:
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Tag {
    pub id: Uuid,
    pub user_id: Uuid,
    pub label: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
}

pub fn slugify(label: &str) -> String {
    label
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub async fn list_tags(pool: &PgPool, user_id: Uuid) -> Result<Vec<Tag>, ApiError> {
    sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE user_id = $1 ORDER BY label")
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn find_or_create_tag(
    pool: &PgPool,
    user_id: Uuid,
    label: &str,
) -> Result<Tag, ApiError> {
    let slug = slugify(label);

    // Try to find existing
    if let Some(tag) = sqlx::query_as::<_, Tag>(
        "SELECT * FROM tags WHERE user_id = $1 AND slug = $2",
    )
    .bind(user_id)
    .bind(&slug)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    {
        return Ok(tag);
    }

    // Create new
    sqlx::query_as::<_, Tag>(
        "INSERT INTO tags (user_id, label, slug) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(user_id)
    .bind(label)
    .bind(&slug)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn add_tag_to_entry(
    pool: &PgPool,
    entry_id: Uuid,
    tag_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO entry_tags (entry_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(entry_id)
    .bind(tag_id)
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}

pub async fn remove_tag_from_entry(
    pool: &PgPool,
    entry_id: Uuid,
    tag_id: Uuid,
) -> Result<bool, ApiError> {
    let result =
        sqlx::query("DELETE FROM entry_tags WHERE entry_id = $1 AND tag_id = $2")
            .bind(entry_id)
            .bind(tag_id)
            .execute(pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

pub async fn get_entry_tags(pool: &PgPool, entry_id: Uuid) -> Result<Vec<Tag>, ApiError> {
    sqlx::query_as::<_, Tag>(
        "SELECT t.* FROM tags t JOIN entry_tags et ON t.id = et.tag_id WHERE et.entry_id = $1 ORDER BY t.label",
    )
    .bind(entry_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn delete_tag(pool: &PgPool, user_id: Uuid, tag_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("DELETE FROM tags WHERE id = $1 AND user_id = $2")
        .bind(tag_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}
```

- [ ] **Step 3: 更新 models/mod.rs**

```rust
pub mod annotation;
pub mod entry;
pub mod memo;
pub mod tag;
pub mod user;
```

（先创建 annotation.rs 和 memo.rs 占位文件 `// Placeholder`）

- [ ] **Step 4: 验证编译通过**
- [ ] **Step 5: Commit**

```bash
git add migrations/004_create_tags.sql src/models/
git commit -m "feat: add tags/entry_tags migration and Tag model"
```

---

### Task 2: Tags API

**Files:**
- Create: `src/api/tags.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: 实现 tags handler**

`src/api/tags.rs`:
```rust
use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::{AppState, AuthUser};
use crate::models::{entry, tag};

pub async fn list_tags(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<tag::Tag>>, ApiError> {
    let tags = tag::list_tags(&state.pool, auth.user_id).await?;
    Ok(Json(tags))
}

#[derive(Deserialize)]
pub struct AddTagRequest {
    pub label: String,
}

pub async fn add_tag_to_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
    Json(req): Json<AddTagRequest>,
) -> Result<Json<tag::Tag>, ApiError> {
    // Verify entry belongs to user
    entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;

    let t = tag::find_or_create_tag(&state.pool, auth.user_id, &req.label).await?;
    tag::add_tag_to_entry(&state.pool, entry_id, t.id).await?;
    Ok(Json(t))
}

pub async fn remove_tag_from_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((entry_id, tag_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;

    tag::remove_tag_from_entry(&state.pool, entry_id, tag_id).await?;
    Ok(Json(serde_json::json!({"message": "removed"})))
}

pub async fn delete_tag(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(tag_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = tag::delete_tag(&state.pool, auth.user_id, tag_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("tag not found".to_string()));
    }
    Ok(Json(serde_json::json!({"message": "deleted"})))
}
```

- [ ] **Step 2: 添加路由到 api/mod.rs**

在 Router 中添加:
```rust
pub mod tags;

// 在 Router::new() 链中添加:
.route("/api/tags", get(tags::list_tags))
.route("/api/entries/{id}/tags", post(tags::add_tag_to_entry))
.route("/api/entries/{entry_id}/tags/{tag_id}", delete(tags::remove_tag_from_entry))
.route("/api/tags/{id}", delete(tags::delete_tag))
```

- [ ] **Step 3: 验证编译通过**
- [ ] **Step 4: Commit**

```bash
git add src/api/tags.rs src/api/mod.rs
git commit -m "feat: implement Tags API (list, add to entry, remove, delete)"
```

---

### Task 3: Annotations 迁移 + 模型 + API

**Files:**
- Create: `migrations/005_create_annotations.sql`
- Create: `src/models/annotation.rs`
- Create: `src/api/annotations.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: 创建 annotations 迁移**

`migrations/005_create_annotations.sql`:
```sql
CREATE TABLE annotations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entry_id UUID NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    quote TEXT NOT NULL,
    text TEXT NOT NULL DEFAULT '',
    ranges JSONB NOT NULL DEFAULT '[]',
    is_orphaned BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_annotations_entry ON annotations (entry_id);
CREATE INDEX idx_annotations_user ON annotations (user_id);
```

- [ ] **Step 2: 实现 Annotation 模型**

`src/models/annotation.rs`:
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Annotation {
    pub id: Uuid,
    pub entry_id: Uuid,
    pub user_id: Uuid,
    pub quote: String,
    pub text: String,
    pub ranges: serde_json::Value,
    pub is_orphaned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateAnnotation {
    pub quote: String,
    pub text: Option<String>,
    pub ranges: serde_json::Value,
}

#[derive(Deserialize)]
pub struct UpdateAnnotation {
    pub text: Option<String>,
    pub ranges: Option<serde_json::Value>,
}

pub async fn list_by_entry(
    pool: &PgPool,
    entry_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<Annotation>, ApiError> {
    sqlx::query_as::<_, Annotation>(
        "SELECT * FROM annotations WHERE entry_id = $1 AND user_id = $2 ORDER BY created_at",
    )
    .bind(entry_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn create(
    pool: &PgPool,
    entry_id: Uuid,
    user_id: Uuid,
    params: &CreateAnnotation,
) -> Result<Annotation, ApiError> {
    sqlx::query_as::<_, Annotation>(
        "INSERT INTO annotations (entry_id, user_id, quote, text, ranges) VALUES ($1,$2,$3,$4,$5) RETURNING *",
    )
    .bind(entry_id)
    .bind(user_id)
    .bind(&params.quote)
    .bind(params.text.as_deref().unwrap_or(""))
    .bind(&params.ranges)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn update(
    pool: &PgPool,
    annotation_id: Uuid,
    user_id: Uuid,
    params: &UpdateAnnotation,
) -> Result<Annotation, ApiError> {
    let existing = sqlx::query_as::<_, Annotation>(
        "SELECT * FROM annotations WHERE id = $1 AND user_id = $2",
    )
    .bind(annotation_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("annotation not found".to_string()))?;

    let text = params.text.as_deref().unwrap_or(&existing.text);
    let ranges = params.ranges.as_ref().unwrap_or(&existing.ranges);

    sqlx::query_as::<_, Annotation>(
        "UPDATE annotations SET text = $3, ranges = $4, updated_at = now() WHERE id = $1 AND user_id = $2 RETURNING *",
    )
    .bind(annotation_id)
    .bind(user_id)
    .bind(text)
    .bind(ranges)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn delete(pool: &PgPool, annotation_id: Uuid, user_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("DELETE FROM annotations WHERE id = $1 AND user_id = $2")
        .bind(annotation_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

/// Mark all annotations for an entry as orphaned (called when entry content is edited)
pub async fn orphan_by_entry(pool: &PgPool, entry_id: Uuid) -> Result<(), ApiError> {
    sqlx::query("UPDATE annotations SET is_orphaned = true, updated_at = now() WHERE entry_id = $1")
        .bind(entry_id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}
```

- [ ] **Step 3: 实现 annotations handler**

`src/api/annotations.rs`:
```rust
use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::{AppState, AuthUser};
use crate::models::{annotation, entry};

pub async fn list_annotations(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<Vec<annotation::Annotation>>, ApiError> {
    entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;

    let annotations = annotation::list_by_entry(&state.pool, entry_id, auth.user_id).await?;
    Ok(Json(annotations))
}

pub async fn create_annotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
    Json(params): Json<annotation::CreateAnnotation>,
) -> Result<Json<annotation::Annotation>, ApiError> {
    entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;

    let ann = annotation::create(&state.pool, entry_id, auth.user_id, &params).await?;
    Ok(Json(ann))
}

pub async fn update_annotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(annotation_id): Path<Uuid>,
    Json(params): Json<annotation::UpdateAnnotation>,
) -> Result<Json<annotation::Annotation>, ApiError> {
    let updated = annotation::update(&state.pool, annotation_id, auth.user_id, &params).await?;
    Ok(Json(updated))
}

pub async fn delete_annotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(annotation_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = annotation::delete(&state.pool, annotation_id, auth.user_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("annotation not found".to_string()));
    }
    Ok(Json(serde_json::json!({"message": "deleted"})))
}
```

- [ ] **Step 4: 添加路由**

```rust
pub mod annotations;

// 路由:
.route("/api/entries/{id}/annotations", get(annotations::list_annotations).post(annotations::create_annotation))
.route("/api/annotations/{id}", patch(annotations::update_annotation).delete(annotations::delete_annotation))
```

- [ ] **Step 5: 验证编译通过**
- [ ] **Step 6: Commit**

```bash
git add migrations/005_create_annotations.sql src/models/annotation.rs src/api/annotations.rs src/api/mod.rs
git commit -m "feat: implement Annotations (CRUD + orphan on content edit)"
```

---

### Task 4: Memos 迁移 + 模型 + API

**Files:**
- Create: `migrations/006_create_memos.sql`
- Create: `src/models/memo.rs`
- Create: `src/api/memos.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: 创建 memos 迁移**

`migrations/006_create_memos.sql`:
```sql
CREATE TABLE memos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    source_url TEXT,
    promoted_entry_id UUID REFERENCES entries(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_memos_user ON memos (user_id, created_at DESC);
```

- [ ] **Step 2: 实现 Memo 模型**

`src/models/memo.rs`:
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Memo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub source_url: Option<String>,
    pub promoted_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateMemo {
    pub content: String,
    pub source_url: Option<String>,
}

pub async fn list_memos(pool: &PgPool, user_id: Uuid) -> Result<Vec<Memo>, ApiError> {
    sqlx::query_as::<_, Memo>(
        "SELECT * FROM memos WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn create_memo(
    pool: &PgPool,
    user_id: Uuid,
    params: &CreateMemo,
) -> Result<Memo, ApiError> {
    sqlx::query_as::<_, Memo>(
        "INSERT INTO memos (user_id, content, source_url) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(user_id)
    .bind(&params.content)
    .bind(params.source_url.as_deref())
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn delete_memo(pool: &PgPool, user_id: Uuid, memo_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("DELETE FROM memos WHERE id = $1 AND user_id = $2")
        .bind(memo_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

pub async fn find_memo_by_id(
    pool: &PgPool,
    user_id: Uuid,
    memo_id: Uuid,
) -> Result<Option<Memo>, ApiError> {
    sqlx::query_as::<_, Memo>("SELECT * FROM memos WHERE id = $1 AND user_id = $2")
        .bind(memo_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn set_promoted_entry(
    pool: &PgPool,
    memo_id: Uuid,
    entry_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query("UPDATE memos SET promoted_entry_id = $2 WHERE id = $1")
        .bind(memo_id)
        .bind(entry_id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}
```

- [ ] **Step 3: 实现 memos handler**

`src/api/memos.rs`:
```rust
use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::{AppState, AuthUser};
use crate::models::{entry, memo};
use crate::tasks::fetcher::FetchJob;

pub async fn list_memos(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<memo::Memo>>, ApiError> {
    let memos = memo::list_memos(&state.pool, auth.user_id).await?;
    Ok(Json(memos))
}

pub async fn create_memo(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(params): Json<memo::CreateMemo>,
) -> Result<Json<memo::Memo>, ApiError> {
    if params.content.is_empty() {
        return Err(ApiError::BadRequest("content is required".to_string()));
    }
    let m = memo::create_memo(&state.pool, auth.user_id, &params).await?;
    Ok(Json(m))
}

pub async fn delete_memo(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(memo_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = memo::delete_memo(&state.pool, auth.user_id, memo_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("memo not found".to_string()));
    }
    Ok(Json(serde_json::json!({"message": "deleted"})))
}

pub async fn promote_memo(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(memo_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let m = memo::find_memo_by_id(&state.pool, auth.user_id, memo_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("memo not found".to_string()))?;

    if m.promoted_entry_id.is_some() {
        return Err(ApiError::BadRequest("memo already promoted".to_string()));
    }

    // If memo contains a URL, create entry and fetch
    let url = extract_url(&m.content).or(m.source_url.as_deref().map(String::from));

    if let Some(url) = url {
        let new_entry = entry::create_entry(&state.pool, auth.user_id, &url).await?;
        memo::set_promoted_entry(&state.pool, memo_id, new_entry.id).await?;
        let _ = state
            .fetch_queue
            .send(FetchJob {
                entry_id: new_entry.id,
                url: new_entry.url.clone(),
            })
            .await;
        Ok(Json(serde_json::json!({
            "message": "promoted to entry",
            "entry_id": new_entry.id
        })))
    } else {
        // No URL — create entry with memo content as content
        let new_entry = entry::create_entry(&state.pool, auth.user_id, &format!("memo:{}", memo_id)).await?;
        // Directly set content from memo
        entry::update_entry_content(
            &state.pool,
            new_entry.id,
            Some(&m.content),
            Some(&format!("<p>{}</p>", m.content)),
            Some(&m.content),
            None, None, None, Some(1), 0, "manual",
        )
        .await?;
        memo::set_promoted_entry(&state.pool, memo_id, new_entry.id).await?;
        Ok(Json(serde_json::json!({
            "message": "promoted to entry",
            "entry_id": new_entry.id
        })))
    }
}

fn extract_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|word| word.starts_with("http://") || word.starts_with("https://"))
        .and_then(|word| url::Url::parse(word).ok())
        .map(|u| u.to_string())
}
```

- [ ] **Step 4: 添加路由**

```rust
pub mod memos;

// 路由:
.route("/api/memos", get(memos::list_memos).post(memos::create_memo))
.route("/api/memos/{id}", delete(memos::delete_memo))
.route("/api/memos/{id}/promote", post(memos::promote_memo))
```

- [ ] **Step 5: 验证编译通过**
- [ ] **Step 6: Commit**

```bash
git add migrations/006_create_memos.sql src/models/memo.rs src/api/memos.rs src/api/mod.rs
git commit -m "feat: implement Memos (CRUD + promote to entry)"
```

---

### Task 5: 集成测试 — Tags

**Files:**
- Create: `tests/integration_tags.rs`

- [ ] **Step 1: 编写测试**

```rust
mod common;
use serde_json::json;

async fn setup(app: &common::TestApp) -> (String, String) {
    let res = app.client.post(app.url("/api/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let token = body["access_token"].as_str().unwrap().to_string();

    let res = app.client.post(app.url("/api/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/tagged"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let entry_id = body["id"].as_str().unwrap().to_string();
    (token, entry_id)
}

#[tokio::test]
async fn add_and_list_tags() {
    let app = common::TestApp::new().await;
    let (token, entry_id) = setup(&app).await;

    let res = app.client.post(app.url(&format!("/api/entries/{}/tags", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"label": "Rust"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let tag: serde_json::Value = res.json().await.unwrap();
    assert_eq!(tag["label"], "Rust");
    assert_eq!(tag["slug"], "rust");

    let res = app.client.get(app.url("/api/tags"))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let tags: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(tags.len(), 1);

    app.cleanup().await;
}

#[tokio::test]
async fn remove_tag_from_entry() {
    let app = common::TestApp::new().await;
    let (token, entry_id) = setup(&app).await;

    let res = app.client.post(app.url(&format!("/api/entries/{}/tags", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"label": "ToRemove"}))
        .send().await.unwrap();
    let tag: serde_json::Value = res.json().await.unwrap();
    let tag_id = tag["id"].as_str().unwrap();

    let res = app.client.delete(app.url(&format!("/api/entries/{}/tags/{}", entry_id, tag_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);

    app.cleanup().await;
}

#[tokio::test]
async fn delete_tag() {
    let app = common::TestApp::new().await;
    let (token, entry_id) = setup(&app).await;

    let res = app.client.post(app.url(&format!("/api/entries/{}/tags", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"label": "Deletable"}))
        .send().await.unwrap();
    let tag: serde_json::Value = res.json().await.unwrap();
    let tag_id = tag["id"].as_str().unwrap();

    let res = app.client.delete(app.url(&format!("/api/tags/{}", tag_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);

    app.cleanup().await;
}
```

- [ ] **Step 2: 运行测试**
- [ ] **Step 3: Commit**

---

### Task 6: 集成测试 — Annotations

**Files:**
- Create: `tests/integration_annotations.rs`

- [ ] **Step 1: 编写测试**

```rust
mod common;
use serde_json::json;

async fn setup(app: &common::TestApp) -> (String, String) {
    let res = app.client.post(app.url("/api/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let token = body["access_token"].as_str().unwrap().to_string();

    let res = app.client.post(app.url("/api/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/annotated"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let entry_id = body["id"].as_str().unwrap().to_string();
    (token, entry_id)
}

#[tokio::test]
async fn create_and_list_annotations() {
    let app = common::TestApp::new().await;
    let (token, entry_id) = setup(&app).await;

    let res = app.client.post(app.url(&format!("/api/entries/{}/annotations", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"quote": "important text", "text": "my note", "ranges": []}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let ann: serde_json::Value = res.json().await.unwrap();
    assert_eq!(ann["quote"], "important text");
    assert_eq!(ann["text"], "my note");

    let res = app.client.get(app.url(&format!("/api/entries/{}/annotations", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let anns: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(anns.len(), 1);

    app.cleanup().await;
}

#[tokio::test]
async fn update_annotation() {
    let app = common::TestApp::new().await;
    let (token, entry_id) = setup(&app).await;

    let res = app.client.post(app.url(&format!("/api/entries/{}/annotations", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"quote": "text", "ranges": []}))
        .send().await.unwrap();
    let ann: serde_json::Value = res.json().await.unwrap();
    let ann_id = ann["id"].as_str().unwrap();

    let res = app.client.patch(app.url(&format!("/api/annotations/{}", ann_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"text": "updated note"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let updated: serde_json::Value = res.json().await.unwrap();
    assert_eq!(updated["text"], "updated note");

    app.cleanup().await;
}

#[tokio::test]
async fn delete_annotation() {
    let app = common::TestApp::new().await;
    let (token, entry_id) = setup(&app).await;

    let res = app.client.post(app.url(&format!("/api/entries/{}/annotations", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"quote": "to delete", "ranges": []}))
        .send().await.unwrap();
    let ann: serde_json::Value = res.json().await.unwrap();
    let ann_id = ann["id"].as_str().unwrap();

    let res = app.client.delete(app.url(&format!("/api/annotations/{}", ann_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);

    app.cleanup().await;
}
```

- [ ] **Step 2: 运行测试**
- [ ] **Step 3: Commit**

---

### Task 7: 集成测试 — Memos

**Files:**
- Create: `tests/integration_memos.rs`

- [ ] **Step 1: 编写测试**

```rust
mod common;
use serde_json::json;

async fn get_token(app: &common::TestApp) -> String {
    let res = app.client.post(app.url("/api/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn create_and_list_memos() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let res = app.client.post(app.url("/api/memos"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"content": "remember this"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let memo: serde_json::Value = res.json().await.unwrap();
    assert_eq!(memo["content"], "remember this");

    let res = app.client.get(app.url("/api/memos"))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    let memos: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(memos.len(), 1);

    app.cleanup().await;
}

#[tokio::test]
async fn delete_memo() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let res = app.client.post(app.url("/api/memos"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"content": "to delete"}))
        .send().await.unwrap();
    let memo: serde_json::Value = res.json().await.unwrap();
    let memo_id = memo["id"].as_str().unwrap();

    let res = app.client.delete(app.url(&format!("/api/memos/{}", memo_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);

    app.cleanup().await;
}

#[tokio::test]
async fn promote_memo_with_url() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let res = app.client.post(app.url("/api/memos"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"content": "check out https://example.com/promoted"}))
        .send().await.unwrap();
    let memo: serde_json::Value = res.json().await.unwrap();
    let memo_id = memo["id"].as_str().unwrap();

    let res = app.client.post(app.url(&format!("/api/memos/{}/promote", memo_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["entry_id"].is_string());

    // Cannot promote again
    let res = app.client.post(app.url(&format!("/api/memos/{}/promote", memo_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 400);

    app.cleanup().await;
}

#[tokio::test]
async fn empty_memo_rejected() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let res = app.client.post(app.url("/api/memos"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"content": ""}))
        .send().await.unwrap();
    assert_eq!(res.status(), 400);

    app.cleanup().await;
}
```

- [ ] **Step 2: 运行全部测试**
- [ ] **Step 3: Commit**
