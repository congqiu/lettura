# Plan 4b: 改进优化（架构演进、可观测性、开发者体验）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实施 12 项 P2/P3 优化，覆盖 API 版本前缀、请求验证集中化、API 限流、DB 连接池可配置、错误处理增强、前端代码分割、前端测试框架、离线状态提示、软删除、JSONB GIN 索引、复合索引优化、Prometheus Metrics。

**Architecture:** 依赖关系：B1 先做 -> B2/B3/B9 依赖 B1 -> B11 依赖 B9。B4/B5/B6/B7/B8/B10/B12 独立，可并行执行。

**Tech Stack:** Rust (Axum, tower-http, tower-governor, SQLx, validator, metrics, metrics-exporter-prometheus), React 19, TypeScript, Axios, Vitest, Testing Library

**Spec:** `docs/specs/2026-03-29-optimization-design.md` B1-B12

---

## File Structure

| Action | Path | Responsibility |
|--------|------|---------------|
| Modify | `src/api/mod.rs` | API v1 路由前缀迁移 + 301 重定向 + 软删除新路由 |
| Modify | `src/main.rs` | 限流中间件 + metrics 端点 |
| Modify | `web/src/api/client.ts` | baseURL 改为 `/api/v1` |
| Modify | `extension/background.js` | API 路径加 `/v1` 前缀 |
| Modify | `extension/popup.js` | API 路径加 `/v1` 前缀 |
| Modify | `Cargo.toml` | 新增 validator, tower-governor, metrics, metrics-exporter-prometheus |
| Create | `src/api/validate.rs` | ValidatedJson extractor |
| Modify | `src/api/auth.rs` | RegisterRequest derive Validate + 使用 ValidatedJson |
| Modify | `src/api/entries.rs` | CreateEntryRequest derive Validate + 使用 ValidatedJson + 软删除新 handler |
| Modify | `src/api/annotations.rs` | CreateAnnotationRequest derive Validate + 使用 ValidatedJson |
| Modify | `src/api/memos.rs` | CreateMemoRequest derive Validate + 使用 ValidatedJson |
| Modify | `src/config.rs` | DB pool 参数 + metrics_enabled |
| Modify | `src/db.rs` | 使用可配置 pool 参数 |
| Modify | `src/api/error.rs` | From<sqlx::Error> 增强 + ValidationErrors 支持 |
| Modify | `src/models/entry.rs` | 软删除相关函数 + 移除局部 constraint 处理 |
| Modify | `src/api/feed.rs` | 查询加 deleted_at IS NULL |
| Modify | `src/api/export.rs` | 查询加 deleted_at IS NULL |
| Modify | `src/api/admin.rs` | reindex 跳过已删除 |
| Modify | `src/tasks/fetcher.rs` | Arc<AtomicUsize> 队列深度跟踪 |
| Modify | `src/search.rs` | doc_count gauge 更新 |
| Modify | `web/src/App.tsx` | React.lazy 代码分割 + Suspense |
| Modify | `web/src/components/Layout.tsx` | NetworkStatus 组件 |
| Modify | `web/vite.config.ts` | vitest 配置 |
| Modify | `web/package.json` | 测试依赖 + 脚本 |
| Create | `web/src/test-setup.ts` | 测试初始化 |
| Create | `web/src/components/__tests__/ProtectedRoute.test.tsx` | ProtectedRoute 测试 |
| Create | `web/src/components/__tests__/EntryCard.test.tsx` | EntryCard 测试 |
| Create | `web/src/api/__tests__/client.test.ts` | client refresh lock 测试 |
| Create | `web/src/components/NetworkStatus.tsx` | 离线/在线状态提示 |
| Create | `migrations/009_soft_delete.sql` | 软删除 migration |
| Create | `migrations/010_gin_indexes.sql` | GIN 索引 |
| Create | `migrations/011_composite_indexes.sql` | 复合索引优化 |
| Modify | `tests/common/mod.rs` | Config 适配新字段 |

---

### Task 1: API 版本前缀 [B1]

**Files:**
- Modify: `src/api/mod.rs`
- Modify: `web/src/api/client.ts`
- Modify: `extension/background.js`
- Modify: `extension/popup.js`

- [ ] **Step 1: 写集成测试验证 v1 路由和 301 重定向**

在 `tests/integration_api_version.rs` 中：

```rust
mod common;

#[tokio::test]
async fn api_v1_routes_work() {
    let app = common::TestApp::new().await;

    // Register via v1 path
    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "v1user",
            "email": "v1@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    app.cleanup().await;
}

#[tokio::test]
async fn old_api_routes_redirect_to_v1() {
    let app = common::TestApp::new().await;

    // Use a client that does NOT follow redirects
    let no_redirect_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    // Old path should 301 redirect to v1
    let res = no_redirect_client
        .get(app.url("/api/entries"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 301);
    let location = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.ends_with("/api/v1/entries"), "redirect location was: {}", location);

    app.cleanup().await;
}

#[tokio::test]
async fn health_endpoint_not_versioned() {
    let app = common::TestApp::new().await;

    // /api/health should still work without /v1
    let res = app.client.get(app.url("/api/health")).send().await.unwrap();
    assert_eq!(res.status(), 200);

    // /api/v1/health should NOT exist (404 or redirect)
    let no_redirect_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let res = no_redirect_client
        .get(app.url("/api/v1/health"))
        .send()
        .await
        .unwrap();
    // It will get redirected by the catch-all, but health is at /api/health only
    assert_ne!(res.status(), 200);

    app.cleanup().await;
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test api_v1_routes_work old_api_routes_redirect health_endpoint_not_versioned`
Expected: FAIL -- `/api/v1/*` routes don't exist yet

- [ ] **Step 3: 修改 src/api/mod.rs 将路由迁移到 /api/v1/ 并添加 301 重定向**

将 `src/api/mod.rs` 的 `router_with_search` 函数中所有 `/api/` 开头的路由（除 `/api/health`）改为 `/api/v1/` 前缀。添加一个 catch-all 重定向处理旧路径。

完整替换 `router_with_search` 函数体中的路由注册部分：

```rust
pub fn router_with_search(pool: PgPool, config: Config, search: Option<SearchIndex>) -> Router {
    let search_index = search.unwrap_or_else(|| {
        SearchIndex::open(std::path::Path::new(&config.index_path))
            .expect("failed to open search index")
    });
    let storage: std::sync::Arc<dyn crate::storage::ImageStorage> = std::sync::Arc::from(crate::storage::create_storage(&config));
    let fetch_queue = fetcher::start_fetch_worker(pool.clone(), 5, storage.clone());

    let state = AppState {
        pool,
        config: config.clone(),
        fetch_queue,
        search_index,
        storage,
    };

    Router::new()
        // Health (no auth, no version prefix)
        .route("/api/health", get(health::health_check))
        // Auth
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/refresh", post(auth::refresh))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/regenerate-feed-token", post(auth::regenerate_feed_token))
        // Entries
        .route(
            "/api/v1/entries",
            get(entries::list_entries).post(entries::create_entry),
        )
        .route(
            "/api/v1/entries/{id}",
            get(entries::get_entry)
                .patch(entries::update_entry)
                .delete(entries::delete_entry),
        )
        .route("/api/v1/entries/{id}/refetch", post(entries::refetch_entry))
        // Tags
        .route("/api/v1/tags", get(tags::list_tags))
        .route("/api/v1/entries/{id}/tags", post(tags::add_tag_to_entry))
        .route(
            "/api/v1/entries/{entry_id}/tags/{tag_id}",
            delete(tags::remove_tag_from_entry),
        )
        .route("/api/v1/tags/{id}", delete(tags::delete_tag))
        // Annotations
        .route(
            "/api/v1/entries/{id}/annotations",
            get(annotations::list_annotations).post(annotations::create_annotation),
        )
        .route(
            "/api/v1/annotations/{id}",
            patch(annotations::update_annotation).delete(annotations::delete_annotation),
        )
        // Memos
        .route(
            "/api/v1/memos",
            get(memos::list_memos).post(memos::create_memo),
        )
        .route("/api/v1/memos/{id}", delete(memos::delete_memo))
        .route("/api/v1/memos/{id}/promote", post(memos::promote_memo))
        // Tagging Rules
        .route(
            "/api/v1/tagging-rules",
            get(tagging_rules::list_rules).post(tagging_rules::create_rule),
        )
        .route(
            "/api/v1/tagging-rules/{id}",
            patch(tagging_rules::update_rule).delete(tagging_rules::delete_rule),
        )
        // Site Rules
        .route(
            "/api/v1/site-rules",
            get(site_rules::list_rules).post(site_rules::create_rule),
        )
        .route(
            "/api/v1/site-rules/{id}",
            patch(site_rules::update_rule).delete(site_rules::delete_rule),
        )
        // Import/Export
        .route("/api/v1/import/wallabag", post(import::import_wallabag))
        .route("/api/v1/import/browser", post(import::import_browser))
        .route("/api/v1/export", get(export::export_all))
        // RSS Feeds (no auth - uses feed token, no version prefix)
        .route("/feed/{user_token}/unread", get(feed::feed_unread))
        .route("/feed/{user_token}/starred", get(feed::feed_starred))
        .route("/feed/{user_token}/archive", get(feed::feed_archive))
        // Admin
        .route("/api/v1/admin/users", get(admin::list_users))
        .route("/api/v1/admin/reindex", post(admin::reindex))
        // Local storage file serving
        .route("/storage/{*path}", get(serve_storage))
        // Legacy redirect: /api/{path} -> /api/v1/{path} (exclude /api/health)
        .route("/api/{*path}", get(legacy_redirect).post(legacy_redirect).put(legacy_redirect).patch(legacy_redirect).delete(legacy_redirect))
        // SPA fallback
        .fallback(crate::spa::spa_handler)
        .with_state(state)
}

async fn legacy_redirect(
    original_uri: axum::http::Uri,
) -> impl axum::response::IntoResponse {
    let path = original_uri.path();
    // Don't redirect /api/health — it's handled by its own route above
    // This handler only matches paths not caught by explicit routes
    let new_path = path.replacen("/api/", "/api/v1/", 1);
    let new_uri = if let Some(query) = original_uri.query() {
        format!("{}?{}", new_path, query)
    } else {
        new_path
    };
    (
        axum::http::StatusCode::MOVED_PERMANENTLY,
        [(axum::http::header::LOCATION, new_uri)],
    )
}
```

注意：需要在文件顶部的 `use axum::routing::{...}` 中添加 `put`：

```rust
use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
```

- [ ] **Step 4: 更新前端 client.ts baseURL**

在 `web/src/api/client.ts` 中将 `baseURL: '/api'` 改为 `baseURL: '/api/v1'`。

同时需要更新 `doRefresh` 函数中的 refresh 路径，因为 refresh 调用直接用了 `axios.post('/api/auth/refresh', ...)`（不走 `api` 实例），需要改为 `/api/v1/auth/refresh`：

将 `web/src/api/client.ts` 中：
```typescript
const api = axios.create({
  baseURL: '/api',
});
```
改为：
```typescript
const api = axios.create({
  baseURL: '/api/v1',
});
```

将 `doRefresh` 函数中：
```typescript
  const res = await axios.post('/api/auth/refresh', {
```
改为：
```typescript
  const res = await axios.post('/api/v1/auth/refresh', {
```

- [ ] **Step 5: 更新浏览器扩展 background.js API 路径**

在 `extension/background.js` 中，所有 `apiRequest` 和 `authenticatedRequest` 调用的路径都需要加 `/v1` 前缀。修改以下位置：

1. `refreshToken` 函数中：`"/api/auth/refresh"` -> `"/api/v1/auth/refresh"`
2. `handleSavePage` 函数中：`"/api/entries"` -> `"/api/v1/entries"`
3. `handleSaveMemo` 函数中：`"/api/memos"` -> `"/api/v1/memos"`

- [ ] **Step 6: 更新浏览器扩展 popup.js API 路径**

在 `extension/popup.js` 中：

1. `refreshToken` 函数中：`"/api/auth/refresh"` -> `"/api/v1/auth/refresh"`
2. `doLogin` 函数中：`"/api/auth/login"` -> `"/api/v1/auth/login"`
3. `doSave` 函数中：`"/api/entries"` -> `"/api/v1/entries"`

- [ ] **Step 7: 更新现有集成测试**

所有现有集成测试中的 `/api/` 路径需要改为 `/api/v1/`。在每个 `tests/integration_*.rs` 文件中搜索 `"/api/` 并替换为 `"/api/v1/`，但排除 `"/api/health"`。

Run: `grep -rn '"/api/' tests/ --include='*.rs' | grep -v '/api/health' | grep -v '/api/v1/'`

逐一更新所有匹配的路径。

- [ ] **Step 8: 更新 Vite proxy 配置**

`web/vite.config.ts` 中 `/api` proxy 规则无需改动，因为 `/api/v1/*` 仍然匹配 `/api` 前缀。确认编译通过即可。

- [ ] **Step 9: 运行测试确认通过**

Run: `cargo test`
Expected: 全部 PASS

Run: `cd web && npm run build`
Expected: BUILD SUCCESS

- [ ] **Step 10: Commit**

```bash
git add src/api/mod.rs web/src/api/client.ts extension/background.js extension/popup.js tests/
git commit -m "feat: migrate API routes to /api/v1/ with 301 redirect from legacy paths"
```

---

### Task 2: 请求验证集中化 [B2]

**Depends on:** Task 1 (B1)

**Files:**
- Modify: `Cargo.toml`
- Create: `src/api/validate.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/api/error.rs`
- Modify: `src/api/auth.rs`
- Modify: `src/api/entries.rs`
- Modify: `src/api/memos.rs`
- Modify: `src/models/annotation.rs`
- Modify: `src/api/annotations.rs`

- [ ] **Step 1: 写集成测试验证请求验证**

在 `tests/integration_validation.rs` 中：

```rust
mod common;

#[tokio::test]
async fn register_rejects_invalid_email() {
    let app = common::TestApp::new().await;

    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "testuser",
            "email": "not-an-email",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"], "validation");
    assert!(body["fields"]["email"].is_array());

    app.cleanup().await;
}

#[tokio::test]
async fn register_rejects_short_password() {
    let app = common::TestApp::new().await;

    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "short"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"], "validation");
    assert!(body["fields"]["password"].is_array());

    app.cleanup().await;
}

#[tokio::test]
async fn create_entry_rejects_invalid_url() {
    let app = common::TestApp::new().await;

    // Register and get token
    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "valuser",
            "email": "val@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    let auth: serde_json::Value = res.json().await.unwrap();
    let token = auth["access_token"].as_str().unwrap();

    let res = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "url": "not-a-url" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"], "validation");
    assert!(body["fields"]["url"].is_array());

    app.cleanup().await;
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test register_rejects_invalid_email register_rejects_short_password create_entry_rejects_invalid_url`
Expected: FAIL -- no validation error format, just BadRequest or success

- [ ] **Step 3: 添加 validator 依赖**

在 `Cargo.toml` 的 `[dependencies]` 中添加：

```toml
validator = { version = "0.19", features = ["derive"] }
```

- [ ] **Step 4: 创建 ValidatedJson extractor**

创建 `src/api/validate.rs`：

```rust
use axum::extract::rejection::JsonRejection;
use axum::extract::FromRequest;
use axum::http::Request;
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::api::error::ApiError;

/// An Axum extractor that deserializes JSON and then validates with the `validator` crate.
pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
{
    type Rejection = ApiError;

    async fn from_request(
        req: Request<axum::body::Body>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let axum::Json(value) = axum::Json::<T>::from_request(req, state)
            .await
            .map_err(|e| ApiError::BadRequest(e.body_text()))?;

        value.validate().map_err(|e| ApiError::Validation(e))?;

        Ok(ValidatedJson(value))
    }
}
```

- [ ] **Step 5: 在 error.rs 中添加 Validation variant**

在 `src/api/error.rs` 顶部添加 import：

```rust
use std::collections::HashMap;
```

在 `ApiError` 枚举中添加新 variant：

```rust
#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    Validation(validator::ValidationErrors),
    Internal(String),
}
```

在 `IntoResponse` 的 match 中添加 Validation 处理分支（在 `Conflict` 和 `Internal` 之间）：

```rust
            ApiError::Validation(errors) => {
                let mut fields: HashMap<String, Vec<String>> = HashMap::new();
                for (field, field_errors) in errors.field_errors() {
                    let messages: Vec<String> = field_errors
                        .iter()
                        .map(|e| {
                            e.message
                                .as_ref()
                                .map(|m| m.to_string())
                                .unwrap_or_else(|| {
                                    e.code.to_string()
                                })
                        })
                        .collect();
                    fields.insert(field.to_string(), messages);
                }
                let body = serde_json::json!({
                    "error": "validation",
                    "fields": fields,
                });
                return (StatusCode::BAD_REQUEST, axum::Json(body)).into_response();
            }
```

- [ ] **Step 6: 注册 validate 模块**

在 `src/api/mod.rs` 的模块声明中添加：

```rust
pub mod validate;
```

- [ ] **Step 7: 给 RegisterRequest 添加验证 derive**

在 `src/api/auth.rs` 中：

顶部添加 import：
```rust
use validator::Validate;
```

修改 `RegisterRequest`：
```rust
#[derive(Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(length(min = 1, message = "username must not be empty"))]
    pub username: String,
    #[validate(email(message = "invalid email format"))]
    pub email: String,
    #[validate(length(min = 8, message = "password must be at least 8 characters"))]
    pub password: String,
}
```

修改 `register` handler 的签名，将 `Json<RegisterRequest>` 改为 `ValidatedJson<RegisterRequest>`：

```rust
use crate::api::validate::ValidatedJson;

pub async fn register(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<RegisterRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    // Remove manual validation — now handled by ValidatedJson
    // First user becomes admin
    let user_count = user::count_users(&state.pool).await?;
    let is_admin = user_count == 0;

    let new_user =
        user::create_user(&state.pool, &req.username, &req.email, &req.password, is_admin).await?;

    issue_tokens(&state, new_user.id, new_user.is_admin).await
}
```

- [ ] **Step 8: 给 CreateEntryRequest 添加验证 derive**

在 `src/api/entries.rs` 中：

顶部添加 import：
```rust
use validator::Validate;
use crate::api::validate::ValidatedJson;
```

修改 `CreateEntryRequest`：
```rust
#[derive(serde::Deserialize, Validate)]
pub struct CreateEntryRequest {
    #[validate(url(message = "invalid URL format"))]
    pub url: String,
}
```

修改 `create_entry` handler 签名和实现：

```rust
pub async fn create_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<CreateEntryRequest>,
) -> Result<Json<entry::Entry>, ApiError> {
    // URL format validation is now handled by ValidatedJson
    let new_entry = entry::create_entry(&state.pool, auth.user_id, &req.url).await?;
    let _ = state.fetch_queue.send(FetchJob { entry_id: new_entry.id, url: new_entry.url.clone() }).await;
    Ok(Json(new_entry))
}
```

- [ ] **Step 9: 给 CreateAnnotation 添加验证 derive**

在 `src/models/annotation.rs` 中：

顶部添加 import：
```rust
use validator::Validate;
```

修改 `CreateAnnotation`：
```rust
#[derive(Deserialize, Validate)]
pub struct CreateAnnotation {
    #[validate(length(min = 1, message = "quote must not be empty"))]
    pub quote: String,
    pub text: Option<String>,
    pub ranges: serde_json::Value,
}
```

在 `src/api/annotations.rs` 中修改 `create_annotation` handler：

顶部添加 import：
```rust
use crate::api::validate::ValidatedJson;
```

```rust
pub async fn create_annotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
    ValidatedJson(params): ValidatedJson<annotation::CreateAnnotation>,
) -> Result<Json<annotation::Annotation>, ApiError> {
    entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;
    let ann = annotation::create(&state.pool, entry_id, auth.user_id, &params).await?;
    Ok(Json(ann))
}
```

- [ ] **Step 10: 给 CreateMemo 添加验证 derive**

在 `src/models/memo.rs` 中：

顶部添加 import：
```rust
use validator::Validate;
```

修改 `CreateMemo`：
```rust
#[derive(Deserialize, Validate)]
pub struct CreateMemo {
    #[validate(length(min = 1, message = "content must not be empty"))]
    pub content: String,
    pub source_url: Option<String>,
}
```

在 `src/api/memos.rs` 中修改 `create_memo` handler：

顶部添加 import：
```rust
use crate::api::validate::ValidatedJson;
```

```rust
pub async fn create_memo(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(params): ValidatedJson<memo::CreateMemo>,
) -> Result<Json<memo::Memo>, ApiError> {
    // content validation is now handled by ValidatedJson
    let m = memo::create_memo(&state.pool, auth.user_id, &params).await?;
    Ok(Json(m))
}
```

- [ ] **Step 11: 运行测试确认通过**

Run: `cargo test`
Expected: 全部 PASS

- [ ] **Step 12: Commit**

```bash
git add Cargo.toml src/api/validate.rs src/api/mod.rs src/api/error.rs src/api/auth.rs src/api/entries.rs src/api/annotations.rs src/api/memos.rs src/models/annotation.rs src/models/memo.rs tests/integration_validation.rs
git commit -m "feat: centralize request validation with validator crate and ValidatedJson extractor"
```

---

### Task 3: 用户级 API 限流 [B3]

**Depends on:** Task 1 (B1)

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`

- [ ] **Step 1: 写集成测试验证限流（auth 端点）**

在 `tests/integration_rate_limit.rs` 中：

```rust
mod common;

#[tokio::test]
async fn auth_rate_limit_triggers_after_10_requests() {
    let app = common::TestApp::new().await;

    // Send 11 login requests in quick succession (auth limit is 10/min)
    let mut last_status = 0;
    for i in 0..11 {
        let res = app
            .client
            .post(app.url("/api/v1/auth/login"))
            .json(&serde_json::json!({
                "email": format!("test{}@example.com", i),
                "password": "password123"
            }))
            .send()
            .await
            .unwrap();
        last_status = res.status().as_u16();
    }

    // The 11th request should be rate limited
    assert_eq!(last_status, 429, "expected 429 Too Many Requests");

    app.cleanup().await;
}
```

注意：此测试可能因为 tower-governor 需要在路由层面使用真实 IP 而在测试环境中行为不同。如果测试环境不支持 per-IP 限流（所有请求来自 127.0.0.1），则所有请求共享同一个 bucket，测试依然有效。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test auth_rate_limit_triggers`
Expected: FAIL -- 没有限流，全部返回 401（invalid credentials）而不是 429

- [ ] **Step 3: 添加 tower-governor 依赖**

在 `Cargo.toml` 的 `[dependencies]` 中添加：

```toml
tower_governor = "0.4"
```

- [ ] **Step 4: 在 src/api/mod.rs 中添加限流层**

重构路由结构，将 auth 路由和其他路由分开以应用不同限流策略。

在 `src/api/mod.rs` 顶部添加 import：

```rust
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
```

将路由构建改为两部分合并的方式。在 `router_with_search` 函数中，将 auth 路由和其他路由分为两个 Router，各自应用不同的限流 layer，然后 merge：

```rust
    // Auth rate limiter: 10 req/min per IP
    let auth_governor_conf = GovernorConfigBuilder::default()
        .per_second(6) // 1 token every 6 seconds = 10/min
        .burst_size(10)
        .finish()
        .unwrap();

    // Global rate limiter: 100 req/min per IP
    let global_governor_conf = GovernorConfigBuilder::default()
        .per_second(1) // 1 token per second, burst allows 100
        .burst_size(100)
        .finish()
        .unwrap();

    let auth_routes = Router::new()
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/refresh", post(auth::refresh))
        .layer(GovernorLayer { config: auth_governor_conf });

    let main_routes = Router::new()
        // Health (no version prefix)
        .route("/api/health", get(health::health_check))
        // Auth (authenticated, not rate-limited separately)
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/regenerate-feed-token", post(auth::regenerate_feed_token))
        // Entries
        .route(
            "/api/v1/entries",
            get(entries::list_entries).post(entries::create_entry),
        )
        .route(
            "/api/v1/entries/{id}",
            get(entries::get_entry)
                .patch(entries::update_entry)
                .delete(entries::delete_entry),
        )
        .route("/api/v1/entries/{id}/refetch", post(entries::refetch_entry))
        // Tags
        .route("/api/v1/tags", get(tags::list_tags))
        .route("/api/v1/entries/{id}/tags", post(tags::add_tag_to_entry))
        .route(
            "/api/v1/entries/{entry_id}/tags/{tag_id}",
            delete(tags::remove_tag_from_entry),
        )
        .route("/api/v1/tags/{id}", delete(tags::delete_tag))
        // Annotations
        .route(
            "/api/v1/entries/{id}/annotations",
            get(annotations::list_annotations).post(annotations::create_annotation),
        )
        .route(
            "/api/v1/annotations/{id}",
            patch(annotations::update_annotation).delete(annotations::delete_annotation),
        )
        // Memos
        .route(
            "/api/v1/memos",
            get(memos::list_memos).post(memos::create_memo),
        )
        .route("/api/v1/memos/{id}", delete(memos::delete_memo))
        .route("/api/v1/memos/{id}/promote", post(memos::promote_memo))
        // Tagging Rules
        .route(
            "/api/v1/tagging-rules",
            get(tagging_rules::list_rules).post(tagging_rules::create_rule),
        )
        .route(
            "/api/v1/tagging-rules/{id}",
            patch(tagging_rules::update_rule).delete(tagging_rules::delete_rule),
        )
        // Site Rules
        .route(
            "/api/v1/site-rules",
            get(site_rules::list_rules).post(site_rules::create_rule),
        )
        .route(
            "/api/v1/site-rules/{id}",
            patch(site_rules::update_rule).delete(site_rules::delete_rule),
        )
        // Import/Export
        .route("/api/v1/import/wallabag", post(import::import_wallabag))
        .route("/api/v1/import/browser", post(import::import_browser))
        .route("/api/v1/export", get(export::export_all))
        // RSS Feeds
        .route("/feed/{user_token}/unread", get(feed::feed_unread))
        .route("/feed/{user_token}/starred", get(feed::feed_starred))
        .route("/feed/{user_token}/archive", get(feed::feed_archive))
        // Admin
        .route("/api/v1/admin/users", get(admin::list_users))
        .route("/api/v1/admin/reindex", post(admin::reindex))
        // Local storage file serving
        .route("/storage/{*path}", get(serve_storage))
        // Legacy redirect
        .route("/api/{*path}", get(legacy_redirect).post(legacy_redirect).put(legacy_redirect).patch(legacy_redirect).delete(legacy_redirect));

    auth_routes
        .merge(main_routes)
        .layer(GovernorLayer { config: global_governor_conf })
        .fallback(crate::spa::spa_handler)
        .with_state(state)
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test auth_rate_limit_triggers`
Expected: PASS

Run: `cargo test`
Expected: 全部 PASS

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/api/mod.rs
git commit -m "feat: add per-IP rate limiting with tower-governor (10/min auth, 100/min global)"
```

---

### Task 4: DB 连接池可配置 [B4]

**Files:**
- Modify: `src/config.rs`
- Modify: `src/db.rs`
- Modify: `tests/common/mod.rs`

- [ ] **Step 1: 写 Config 新字段的单元测试**

在 `src/config.rs` 的 `#[cfg(test)] mod tests` 中添加（如果 Plan A 已经创建了 tests 模块，追加即可）：

```rust
    #[test]
    fn db_pool_defaults() {
        env::set_var("DATABASE_URL", "postgres://test");
        env::set_var("JWT_SECRET", "a]3kf9$mP!qR7vLx2Yw8Hn5Bc6Tj4Ud0Ze");
        let config = Config::from_env().unwrap();
        assert_eq!(config.db_max_connections, 10);
        assert_eq!(config.db_min_connections, 2);
        assert_eq!(config.db_acquire_timeout_secs, 30);
        env::remove_var("JWT_SECRET");
        env::remove_var("DATABASE_URL");
    }

    #[test]
    fn db_pool_custom_values() {
        env::set_var("DATABASE_URL", "postgres://test");
        env::set_var("JWT_SECRET", "a]3kf9$mP!qR7vLx2Yw8Hn5Bc6Tj4Ud0Ze");
        env::set_var("DB_MAX_CONNECTIONS", "20");
        env::set_var("DB_MIN_CONNECTIONS", "5");
        env::set_var("DB_ACQUIRE_TIMEOUT", "60");
        let config = Config::from_env().unwrap();
        assert_eq!(config.db_max_connections, 20);
        assert_eq!(config.db_min_connections, 5);
        assert_eq!(config.db_acquire_timeout_secs, 60);
        env::remove_var("JWT_SECRET");
        env::remove_var("DATABASE_URL");
        env::remove_var("DB_MAX_CONNECTIONS");
        env::remove_var("DB_MIN_CONNECTIONS");
        env::remove_var("DB_ACQUIRE_TIMEOUT");
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test config::tests -- --test-threads=1`
Expected: FAIL -- `db_max_connections` field does not exist

- [ ] **Step 3: 给 Config 添加 DB pool 字段**

在 `src/config.rs` 的 `Config` struct 中添加：

```rust
    // DB pool
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub db_acquire_timeout_secs: u64,
```

在 `from_env` 的 `Ok(Self { ... })` 中添加：

```rust
            db_max_connections: env::var("DB_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            db_min_connections: env::var("DB_MIN_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2),
            db_acquire_timeout_secs: env::var("DB_ACQUIRE_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
```

- [ ] **Step 4: 修改 db.rs 使用配置参数**

将 `src/db.rs` 修改为接受 `Config`：

```rust
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;

use crate::config::Config;

pub async fn create_pool(config: &Config) -> PgPool {
    PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .min_connections(config.db_min_connections)
        .acquire_timeout(Duration::from_secs(config.db_acquire_timeout_secs))
        .connect(&config.database_url)
        .await
        .expect("failed to connect to database")
}

pub async fn run_migrations(pool: &PgPool) {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .expect("failed to run migrations");
}
```

- [ ] **Step 5: 更新 main.rs 的 create_pool 调用**

在 `src/main.rs` 中将：
```rust
    let pool = lettura::db::create_pool(&config.database_url).await;
```
改为：
```rust
    let pool = lettura::db::create_pool(&config).await;
```

- [ ] **Step 6: 更新 tests/common/mod.rs 中的 Config 构造**

在 `tests/common/mod.rs` 的 `Config { ... }` 构造中添加新字段：

```rust
            db_max_connections: 5,
            db_min_connections: 1,
            db_acquire_timeout_secs: 30,
```

- [ ] **Step 7: 运行测试确认通过**

Run: `cargo test config::tests -- --test-threads=1`
Expected: PASS

Run: `cargo test`
Expected: 全部 PASS

- [ ] **Step 8: Commit**

```bash
git add src/config.rs src/db.rs src/main.rs tests/common/mod.rs
git commit -m "feat: make DB connection pool parameters configurable via environment variables"
```

---

### Task 5: 错误处理增强 [B5]

**Files:**
- Modify: `src/api/error.rs`
- Modify: `src/models/entry.rs`
- Modify: `src/api/entries.rs`
- Modify: `src/api/auth.rs`

- [ ] **Step 1: 写集成测试验证增强的错误映射**

在 `tests/integration_errors.rs` 中：

```rust
mod common;

#[tokio::test]
async fn duplicate_entry_returns_conflict_from_unified_handler() {
    let app = common::TestApp::new().await;

    // Register
    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "erruser",
            "email": "err@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    let auth: serde_json::Value = res.json().await.unwrap();
    let token = auth["access_token"].as_str().unwrap();

    // Create entry
    let res = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "url": "https://example.com/dup-test" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // Duplicate entry should return 409
    let res = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "url": "https://example.com/dup-test" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 409);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"], "conflict");
    assert!(body["message"].as_str().unwrap().contains("URL already saved"));

    app.cleanup().await;
}

#[tokio::test]
async fn duplicate_email_returns_conflict() {
    let app = common::TestApp::new().await;

    // Register first user
    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "user1",
            "email": "dup@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // Register with same email
    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "user2",
            "email": "dup@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 409);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"], "conflict");

    app.cleanup().await;
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test duplicate_entry_returns_conflict_from_unified duplicate_email_returns_conflict`
Expected: FAIL (partial) -- duplicate entry already returns 409 from local handler, but duplicate email returns 409 with message "username or email already exists". The test for unified error handling pattern will validate the new `From<sqlx::Error>` works.

- [ ] **Step 3: 增强 From<sqlx::Error> 实现**

在 `src/api/error.rs` 中替换现有的 `From<sqlx::Error>` 实现：

```rust
impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::Database(db_err) => {
                if let Some(constraint) = db_err.constraint() {
                    let message = match constraint {
                        "users_email_key" => "email already exists",
                        "users_username_key" => "username already exists",
                        "idx_entries_user_hashed_url" => "URL already saved",
                        _ => "duplicate record",
                    };
                    return ApiError::Conflict(message.to_string());
                }
                tracing::error!("database error: {}", e);
                ApiError::Internal("internal server error".to_string())
            }
            _ => {
                tracing::error!("database error: {}", e);
                ApiError::Internal("internal server error".to_string())
            }
        }
    }
}
```

- [ ] **Step 4: 从 entry.rs 移除局部 constraint 处理**

在 `src/models/entry.rs` 的 `create_entry` 函数中，将 `.map_err(|e| match e { ... })` 简化为 `.map_err(ApiError::from)`（或直接用 `?`），因为 `From<sqlx::Error>` 已经统一处理了：

将 `create_entry` 函数的 SQL 执行部分从：

```rust
    sqlx::query_as::<_, Entry>(
        "INSERT INTO entries (user_id, url, given_url, hashed_url, hashed_given_url, domain_name) VALUES ($1,$2,$3,$4,$5,$6) RETURNING *"
    )
    .bind(user_id).bind(&url).bind(given_url).bind(&hashed_url).bind(&hashed_given_url).bind(&domain_name)
    .fetch_one(pool).await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.constraint() == Some("idx_entries_user_hashed_url") => {
            ApiError::Conflict("URL already saved".to_string())
        }
        _ => ApiError::Internal(e.to_string()),
    })
```

改为：

```rust
    Ok(sqlx::query_as::<_, Entry>(
        "INSERT INTO entries (user_id, url, given_url, hashed_url, hashed_given_url, domain_name) VALUES ($1,$2,$3,$4,$5,$6) RETURNING *"
    )
    .bind(user_id).bind(&url).bind(given_url).bind(&hashed_url).bind(&hashed_given_url).bind(&domain_name)
    .fetch_one(pool).await?)
```

- [ ] **Step 5: 从 user.rs 移除局部 constraint 处理**

在 `src/models/user.rs` 的 `create_user` 函数中，将 `.map_err(|e| match e { ... })` 简化为 `?`：

将：
```rust
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.constraint().is_some() => {
            ApiError::Conflict("username or email already exists".to_string())
        }
        _ => ApiError::Internal(e.to_string()),
    })
```

改为：
```rust
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)
```

这样 `users_email_key` 和 `users_username_key` 的约束错误会由统一的 `From<sqlx::Error>` 分别映射为具体消息。

- [ ] **Step 6: 给高频 handler 添加 tracing::instrument**

在 `src/api/entries.rs` 中：

```rust
use tracing;
```

给 `create_entry` 和 `list_entries` 添加 instrument 宏：

```rust
#[tracing::instrument(skip(state), err)]
pub async fn create_entry(
    ...
```

```rust
#[tracing::instrument(skip(state), err)]
pub async fn list_entries(
    ...
```

在 `src/api/auth.rs` 中给 `register` 和 `login` 添加：

```rust
#[tracing::instrument(skip(state, req), err)]
pub async fn register(
    ...
```

```rust
#[tracing::instrument(skip(state, req), err)]
pub async fn login(
    ...
```

注意：`skip(state)` 防止打印整个 AppState；`skip(req)` 防止打印密码。`err` 自动记录 Err 返回值。

- [ ] **Step 7: 运行测试确认通过**

Run: `cargo test`
Expected: 全部 PASS

- [ ] **Step 8: Commit**

```bash
git add src/api/error.rs src/models/entry.rs src/models/user.rs src/api/entries.rs src/api/auth.rs tests/integration_errors.rs
git commit -m "feat: enhance From<sqlx::Error> with constraint-aware mapping and add tracing instrumentation"
```

---

### Task 6: 前端代码分割 [B6]

**Files:**
- Modify: `web/src/App.tsx`

- [ ] **Step 1: 修改 App.tsx 添加 React.lazy 和 Suspense**

将 `web/src/App.tsx` 修改为：

```tsx
import { lazy, Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import Layout from './components/Layout';
import ProtectedRoute from './components/ProtectedRoute';
import ErrorBoundary from './components/ErrorBoundary';
import LoginPage from './pages/LoginPage';
import RegisterPage from './pages/RegisterPage';
import EntryListPage from './pages/EntryListPage';
import EntryDetailPage from './pages/EntryDetailPage';

// Lazy-loaded pages (low frequency)
const MemosPage = lazy(() => import('./pages/MemosPage'));
const SettingsPage = lazy(() => import('./pages/SettingsPage'));

const queryClient = new QueryClient();

function App() {
  return (
    <ErrorBoundary level="app">
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <Suspense fallback={<div className="p-8 text-center">Loading...</div>}>
            <Routes>
              <Route path="/login" element={<LoginPage />} />
              <Route path="/register" element={<RegisterPage />} />
              <Route
                path="/"
                element={
                  <ProtectedRoute>
                    <Layout />
                  </ProtectedRoute>
                }
              >
                <Route index element={<EntryListPage filter="unread" />} />
                <Route path="archived" element={<EntryListPage filter="archived" />} />
                <Route path="starred" element={<EntryListPage filter="starred" />} />
                <Route path="entry/:id" element={<EntryDetailPage />} />
                <Route path="memos" element={<MemosPage />} />
                <Route path="settings" element={<SettingsPage />} />
              </Route>
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </Suspense>
        </BrowserRouter>
      </QueryClientProvider>
    </ErrorBoundary>
  );
}

export default App;
```

注意：`MemosPage` 和 `SettingsPage` 必须使用 `export default` 导出（检查确认）。`EntryDetailPage` 保持同步导入以避免列表->详情的点击闪烁。

- [ ] **Step 2: 前端编译验证**

Run: `cd web && npm run build`
Expected: BUILD SUCCESS，且输出中可以看到额外的 chunk 文件（MemosPage 和 SettingsPage 被分割）

- [ ] **Step 3: Commit**

```bash
git add web/src/App.tsx
git commit -m "feat: add React.lazy code splitting for SettingsPage and MemosPage"
```

---

### Task 7: 前端测试框架搭建 [B7]

**Files:**
- Modify: `web/package.json`
- Modify: `web/vite.config.ts`
- Modify: `web/tsconfig.app.json`
- Create: `web/src/test-setup.ts`
- Create: `web/src/components/__tests__/ProtectedRoute.test.tsx`
- Create: `web/src/components/__tests__/EntryCard.test.tsx`
- Create: `web/src/api/__tests__/client.test.ts`

- [ ] **Step 1: 安装测试依赖**

```bash
cd web && npm install -D vitest @testing-library/react @testing-library/jest-dom @testing-library/user-event jsdom
```

- [ ] **Step 2: 配置 vite.config.ts 添加 test**

在 `web/vite.config.ts` 中添加 test 配置。注意需要添加 `/// <reference types="vitest" />` 并添加 test 块：

```typescript
/// <reference types="vitest" />
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    proxy: {
      '/api': 'http://localhost:3000',
      '/feed': 'http://localhost:3000',
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: './src/test-setup.ts',
  },
})
```

- [ ] **Step 3: 更新 tsconfig.app.json 添加 vitest 类型**

在 `web/tsconfig.app.json` 的 `compilerOptions.types` 中添加 `"vitest/globals"`：

```json
    "types": ["vite/client", "vitest/globals"],
```

- [ ] **Step 4: 创建 test-setup.ts**

创建 `web/src/test-setup.ts`：

```typescript
import '@testing-library/jest-dom';
```

- [ ] **Step 5: 添加 package.json 测试脚本**

在 `web/package.json` 的 `scripts` 中添加：

```json
    "test": "vitest run",
    "test:watch": "vitest"
```

- [ ] **Step 6: 创建 ProtectedRoute 测试**

创建 `web/src/components/__tests__/ProtectedRoute.test.tsx`：

```tsx
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import ProtectedRoute from '../ProtectedRoute';

// Mock the auth store
vi.mock('../../store/auth', () => ({
  useAuthStore: vi.fn(),
}));

import { useAuthStore } from '../../store/auth';

describe('ProtectedRoute', () => {
  it('renders children when authenticated', () => {
    vi.mocked(useAuthStore).mockImplementation((selector: any) =>
      selector({ isAuthenticated: true })
    );

    render(
      <MemoryRouter>
        <ProtectedRoute>
          <div data-testid="protected-content">Protected</div>
        </ProtectedRoute>
      </MemoryRouter>
    );

    expect(screen.getByTestId('protected-content')).toBeInTheDocument();
  });

  it('redirects to /login when not authenticated', () => {
    vi.mocked(useAuthStore).mockImplementation((selector: any) =>
      selector({ isAuthenticated: false })
    );

    render(
      <MemoryRouter initialEntries={['/dashboard']}>
        <ProtectedRoute>
          <div data-testid="protected-content">Protected</div>
        </ProtectedRoute>
      </MemoryRouter>
    );

    expect(screen.queryByTestId('protected-content')).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 7: 创建 EntryCard 测试**

创建 `web/src/components/__tests__/EntryCard.test.tsx`：

```tsx
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import EntryCard from '../EntryCard';
import type { EntrySummary } from '../../api/entries';

const mockEntry: EntrySummary = {
  id: '123e4567-e89b-12d3-a456-426614174000',
  url: 'https://example.com/article',
  title: 'Test Article Title',
  content_type: 'article',
  extract_method: 'readability',
  language: 'en',
  reading_time: 5,
  preview_picture: null,
  domain_name: 'example.com',
  is_archived: false,
  is_starred: false,
  created_at: new Date().toISOString(),
};

function renderWithProviders(ui: React.ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>{ui}</MemoryRouter>
    </QueryClientProvider>
  );
}

describe('EntryCard', () => {
  it('renders title', () => {
    renderWithProviders(<EntryCard entry={mockEntry} />);
    expect(screen.getByText('Test Article Title')).toBeInTheDocument();
  });

  it('renders domain name', () => {
    renderWithProviders(<EntryCard entry={mockEntry} />);
    expect(screen.getByText('example.com')).toBeInTheDocument();
  });

  it('renders reading time', () => {
    renderWithProviders(<EntryCard entry={mockEntry} />);
    expect(screen.getByText(/5 分钟/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 8: 创建 client refresh lock 测试**

创建 `web/src/api/__tests__/client.test.ts`：

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';

// We test the refresh lock logic by importing the module and simulating 401s
// Since the actual client has side effects (axios interceptors), we test
// the core logic pattern: multiple concurrent 401s should only trigger one refresh

describe('refresh lock pattern', () => {
  let refreshPromise: Promise<string> | null = null;
  let refreshCallCount: number;

  function doRefresh(): Promise<string> {
    refreshCallCount++;
    return new Promise((resolve) => {
      setTimeout(() => resolve('new-token'), 10);
    });
  }

  async function handleUnauthorized(): Promise<string> {
    if (!refreshPromise) {
      refreshPromise = doRefresh().finally(() => {
        refreshPromise = null;
      });
    }
    return refreshPromise;
  }

  beforeEach(() => {
    refreshPromise = null;
    refreshCallCount = 0;
  });

  it('deduplicates concurrent refresh calls', async () => {
    // Simulate 5 concurrent 401 responses
    const results = await Promise.all([
      handleUnauthorized(),
      handleUnauthorized(),
      handleUnauthorized(),
      handleUnauthorized(),
      handleUnauthorized(),
    ]);

    // Only one actual refresh should have been made
    expect(refreshCallCount).toBe(1);

    // All should receive the same token
    for (const token of results) {
      expect(token).toBe('new-token');
    }
  });

  it('allows refresh after previous one completes', async () => {
    await handleUnauthorized();
    expect(refreshCallCount).toBe(1);

    // After the promise resolves and resets, a new refresh should work
    await handleUnauthorized();
    expect(refreshCallCount).toBe(2);
  });
});
```

- [ ] **Step 9: 运行前端测试**

Run: `cd web && npm test`
Expected: 3 test files, all PASS

- [ ] **Step 10: Commit**

```bash
git add web/package.json web/vite.config.ts web/tsconfig.app.json web/src/test-setup.ts web/src/components/__tests__/ web/src/api/__tests__/
git commit -m "feat: set up vitest + testing-library with 3 baseline test files"
```

---

### Task 8: 离线/网络状态提示 [B8]

**Files:**
- Create: `web/src/components/NetworkStatus.tsx`
- Modify: `web/src/components/Layout.tsx`

- [ ] **Step 1: 创建 NetworkStatus 组件**

创建 `web/src/components/NetworkStatus.tsx`：

```tsx
import { useState, useEffect } from 'react';

export default function NetworkStatus() {
  const [isOnline, setIsOnline] = useState(navigator.onLine);
  const [showRecovered, setShowRecovered] = useState(false);

  useEffect(() => {
    function handleOnline() {
      setIsOnline(true);
      setShowRecovered(true);
      setTimeout(() => setShowRecovered(false), 2000);
    }

    function handleOffline() {
      setIsOnline(false);
      setShowRecovered(false);
    }

    window.addEventListener('online', handleOnline);
    window.addEventListener('offline', handleOffline);

    return () => {
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
    };
  }, []);

  if (isOnline && !showRecovered) {
    return null;
  }

  return (
    <div
      className={`fixed top-0 left-0 right-0 z-50 px-4 py-2 text-center text-sm font-medium transition-colors ${
        isOnline
          ? 'bg-green-600 text-white'
          : 'bg-red-600 text-white'
      }`}
      role="alert"
    >
      {isOnline
        ? '网络连接已恢复'
        : '网络连接已断开，请检查网络后重试'}
    </div>
  );
}
```

- [ ] **Step 2: 在 Layout.tsx 中添加 NetworkStatus**

在 `web/src/components/Layout.tsx` 顶部添加 import：

```tsx
import NetworkStatus from './NetworkStatus';
```

在 `return` 的最外层 `<div>` 中，在 `<header>` 之前添加：

```tsx
      <NetworkStatus />
```

即：
```tsx
  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-950 text-gray-900 dark:text-gray-100 transition-colors">
      <NetworkStatus />
      <header className="bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-800 sticky top-0 z-10">
```

- [ ] **Step 3: 前端编译验证**

Run: `cd web && npm run build`
Expected: BUILD SUCCESS

- [ ] **Step 4: Commit**

```bash
git add web/src/components/NetworkStatus.tsx web/src/components/Layout.tsx
git commit -m "feat: add NetworkStatus component showing offline/online notification bar"
```

---

### Task 9: 软删除机制 [B9]

**Depends on:** Task 1 (B1)

**Files:**
- Create: `migrations/009_soft_delete.sql`
- Modify: `src/models/entry.rs`
- Modify: `src/api/entries.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/api/feed.rs`
- Modify: `src/api/export.rs`
- Modify: `src/api/admin.rs`

- [ ] **Step 1: 创建 migration**

创建 `migrations/009_soft_delete.sql`：

```sql
ALTER TABLE entries ADD COLUMN deleted_at TIMESTAMPTZ;

CREATE INDEX idx_entries_deleted ON entries (deleted_at) WHERE deleted_at IS NOT NULL;
```

- [ ] **Step 2: 写集成测试**

在 `tests/integration_soft_delete.rs` 中：

```rust
mod common;

#[tokio::test]
async fn soft_delete_hides_entry_from_list() {
    let app = common::TestApp::new().await;

    // Register
    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "sduser",
            "email": "sd@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    let auth: serde_json::Value = res.json().await.unwrap();
    let token = auth["access_token"].as_str().unwrap();

    // Create entry
    let res = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "url": "https://example.com/soft-delete-test" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let entry: serde_json::Value = res.json().await.unwrap();
    let entry_id = entry["id"].as_str().unwrap();

    // Soft delete
    let res = app
        .client
        .delete(app.url(&format!("/api/v1/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // Entry should not appear in list
    let res = app
        .client
        .get(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let entries: Vec<serde_json::Value> = res.json().await.unwrap();
    assert!(entries.iter().all(|e| e["id"].as_str().unwrap() != entry_id));

    // Entry should not be accessible by ID
    let res = app
        .client
        .get(app.url(&format!("/api/v1/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);

    app.cleanup().await;
}

#[tokio::test]
async fn restore_entry_makes_it_visible_again() {
    let app = common::TestApp::new().await;

    // Register
    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "restuser",
            "email": "rest@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    let auth: serde_json::Value = res.json().await.unwrap();
    let token = auth["access_token"].as_str().unwrap();

    // Create entry
    let res = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "url": "https://example.com/restore-test" }))
        .send()
        .await
        .unwrap();
    let entry: serde_json::Value = res.json().await.unwrap();
    let entry_id = entry["id"].as_str().unwrap();

    // Soft delete
    app.client
        .delete(app.url(&format!("/api/v1/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    // Restore
    let res = app
        .client
        .post(app.url(&format!("/api/v1/entries/{}/restore", entry_id)))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // Entry should be accessible again
    let res = app
        .client
        .get(app.url(&format!("/api/v1/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    app.cleanup().await;
}

#[tokio::test]
async fn permanent_delete_removes_entry() {
    let app = common::TestApp::new().await;

    // Register
    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "permuser",
            "email": "perm@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    let auth: serde_json::Value = res.json().await.unwrap();
    let token = auth["access_token"].as_str().unwrap();

    // Create entry
    let res = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "url": "https://example.com/perm-delete-test" }))
        .send()
        .await
        .unwrap();
    let entry: serde_json::Value = res.json().await.unwrap();
    let entry_id = entry["id"].as_str().unwrap();

    // Soft delete first
    app.client
        .delete(app.url(&format!("/api/v1/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    // Permanent delete
    let res = app
        .client
        .delete(app.url(&format!("/api/v1/entries/{}/permanent", entry_id)))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // Cannot restore after permanent delete
    let res = app
        .client
        .post(app.url(&format!("/api/v1/entries/{}/restore", entry_id)))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);

    app.cleanup().await;
}

#[tokio::test]
async fn list_deleted_entries_shows_trash() {
    let app = common::TestApp::new().await;

    // Register
    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "trashuser",
            "email": "trash@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    let auth: serde_json::Value = res.json().await.unwrap();
    let token = auth["access_token"].as_str().unwrap();

    // Create and soft delete entry
    let res = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "url": "https://example.com/trash-test" }))
        .send()
        .await
        .unwrap();
    let entry: serde_json::Value = res.json().await.unwrap();
    let entry_id = entry["id"].as_str().unwrap();

    app.client
        .delete(app.url(&format!("/api/v1/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    // List deleted entries
    let res = app
        .client
        .get(app.url("/api/v1/entries?deleted=true"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let entries: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["id"].as_str().unwrap(), entry_id);

    app.cleanup().await;
}
```

- [ ] **Step 3: 运行测试确认失败**

Run: `cargo test soft_delete_hides restore_entry permanent_delete list_deleted_entries`
Expected: FAIL

- [ ] **Step 4: 修改 models/entry.rs**

在 `Entry` struct 中添加 `deleted_at` 字段（在 `updated_at` 之后）：

```rust
    pub deleted_at: Option<DateTime<Utc>>,
```

同样在 `EntrySummary` struct 不需要添加 `deleted_at`（列表查询不返回此字段）。

修改 `ListParams` 添加 `deleted` 参数：

```rust
#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub page: Option<i64>, pub per_page: Option<i64>,
    pub is_archived: Option<bool>, pub is_starred: Option<bool>, pub domain: Option<String>,
    pub search: Option<String>,
    pub deleted: Option<bool>,
}
```

修改 `find_entry_by_id` 添加 `AND deleted_at IS NULL`：

```rust
pub async fn find_entry_by_id(pool: &PgPool, user_id: Uuid, entry_id: Uuid) -> Result<Option<Entry>, ApiError> {
    sqlx::query_as::<_, Entry>("SELECT * FROM entries WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL")
        .bind(entry_id).bind(user_id).fetch_optional(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))
}
```

修改 `list_entries` 添加 `deleted_at` 过滤逻辑：

```rust
pub async fn list_entries(pool: &PgPool, user_id: Uuid, params: &ListParams) -> Result<Vec<EntrySummary>, ApiError> {
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (params.page.unwrap_or(1) - 1).max(0) * per_page;

    let mut sql = String::from(
        "SELECT id, user_id, url, title, content_type, extract_method, language, reading_time, preview_picture, domain_name, published_by, is_archived, is_starred, created_at FROM entries WHERE user_id = $1"
    );

    // Filter by deleted status
    if params.deleted == Some(true) {
        sql.push_str(" AND deleted_at IS NOT NULL");
    } else {
        sql.push_str(" AND deleted_at IS NULL");
    }

    let mut param_idx = 2u32;
    if params.is_archived.is_some() { sql.push_str(&format!(" AND is_archived = ${}", param_idx)); param_idx += 1; }
    if params.is_starred.is_some() { sql.push_str(&format!(" AND is_starred = ${}", param_idx)); param_idx += 1; }
    if params.domain.is_some() { sql.push_str(&format!(" AND domain_name = ${}", param_idx)); param_idx += 1; }
    sql.push_str(&format!(" ORDER BY created_at DESC LIMIT ${} OFFSET ${}", param_idx, param_idx + 1));

    let mut query = sqlx::query_as::<_, EntrySummary>(&sql).bind(user_id);
    if let Some(archived) = params.is_archived { query = query.bind(archived); }
    if let Some(starred) = params.is_starred { query = query.bind(starred); }
    if let Some(ref domain) = params.domain { query = query.bind(domain); }
    query = query.bind(per_page).bind(offset);
    query.fetch_all(pool).await.map_err(|e| ApiError::Internal(e.to_string()))
}
```

修改 `list_entries_by_ids` 添加 `AND deleted_at IS NULL`：

```rust
pub async fn list_entries_by_ids(pool: &PgPool, user_id: Uuid, ids: &[Uuid]) -> Result<Vec<EntrySummary>, ApiError> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    sqlx::query_as::<_, EntrySummary>(
        "SELECT id, user_id, url, title, content_type, extract_method, language, reading_time, preview_picture, domain_name, published_by, is_archived, is_starred, created_at FROM entries WHERE user_id = $1 AND id = ANY($2) AND deleted_at IS NULL ORDER BY created_at DESC"
    )
    .bind(user_id)
    .bind(ids)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}
```

修改 `delete_entry` 改为软删除：

```rust
pub async fn delete_entry(pool: &PgPool, user_id: Uuid, entry_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("UPDATE entries SET deleted_at = now() WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL")
        .bind(entry_id).bind(user_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}
```

添加新函数 `list_deleted_entries`：

```rust
pub async fn list_deleted_entries(pool: &PgPool, user_id: Uuid) -> Result<Vec<EntrySummary>, ApiError> {
    sqlx::query_as::<_, EntrySummary>(
        "SELECT id, user_id, url, title, content_type, extract_method, language, reading_time, preview_picture, domain_name, published_by, is_archived, is_starred, created_at FROM entries WHERE user_id = $1 AND deleted_at IS NOT NULL ORDER BY deleted_at DESC"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}
```

添加 `restore_entry`：

```rust
pub async fn restore_entry(pool: &PgPool, user_id: Uuid, entry_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("UPDATE entries SET deleted_at = NULL WHERE id = $1 AND user_id = $2 AND deleted_at IS NOT NULL")
        .bind(entry_id).bind(user_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}
```

添加 `permanently_delete_entry`：

```rust
pub async fn permanently_delete_entry(pool: &PgPool, user_id: Uuid, entry_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("DELETE FROM entries WHERE id = $1 AND user_id = $2 AND deleted_at IS NOT NULL")
        .bind(entry_id).bind(user_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}
```

- [ ] **Step 5: 修改 api/entries.rs 添加新 handler**

在 `src/api/entries.rs` 中修改 `delete_entry` handler，添加搜索索引删除：

```rust
pub async fn delete_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = entry::delete_entry(&state.pool, auth.user_id, entry_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("entry not found".to_string()));
    }
    // Remove from search index on soft delete
    state.search_index.delete(entry_id).await.ok();
    Ok(Json(serde_json::json!({"message": "deleted"})))
}
```

添加新 handler：

```rust
pub async fn restore_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let restored = entry::restore_entry(&state.pool, auth.user_id, entry_id).await?;
    if !restored {
        return Err(ApiError::NotFound("entry not found in trash".to_string()));
    }
    // Re-index restored entry
    // Need to fetch the entry to get indexable fields (find with deleted_at IS NULL now that it's restored)
    if let Ok(Some(e)) = entry::find_entry_by_id(&state.pool, auth.user_id, entry_id).await {
        state.search_index.upsert(
            e.id,
            e.title.as_deref().unwrap_or(""),
            e.text_content.as_deref().unwrap_or(""),
            &e.url,
            e.domain_name.as_deref().unwrap_or(""),
        ).await.ok();
    }
    Ok(Json(serde_json::json!({"message": "restored"})))
}

pub async fn permanently_delete_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = entry::permanently_delete_entry(&state.pool, auth.user_id, entry_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("entry not found in trash".to_string()));
    }
    // Ensure removed from search index
    state.search_index.delete(entry_id).await.ok();
    Ok(Json(serde_json::json!({"message": "permanently deleted"})))
}
```

- [ ] **Step 6: 在 api/mod.rs 注册新路由**

在 entries 路由块中，在 `refetch` 路由之后添加：

```rust
        .route("/api/v1/entries/{id}/restore", post(entries::restore_entry))
        .route("/api/v1/entries/{id}/permanent", delete(entries::permanently_delete_entry))
```

- [ ] **Step 7: 修改 api/feed.rs 添加 deleted_at IS NULL 过滤**

在 `feed_unread` 的 SQL 查询中添加过滤：

```rust
        "SELECT id, url, title, content, created_at FROM entries WHERE user_id = $1 AND is_archived = false AND deleted_at IS NULL ORDER BY created_at DESC LIMIT 50",
```

在 `feed_starred` 的 SQL 查询中添加过滤：

```rust
        "SELECT id, url, title, content, created_at FROM entries WHERE user_id = $1 AND is_starred = true AND deleted_at IS NULL ORDER BY starred_at DESC LIMIT 50",
```

在 `feed_archive` 的 SQL 查询中添加过滤：

```rust
        "SELECT id, url, title, content, created_at FROM entries WHERE user_id = $1 AND is_archived = true AND deleted_at IS NULL ORDER BY archived_at DESC LIMIT 50",
```

- [ ] **Step 8: 修改 api/export.rs 添加 deleted_at IS NULL 过滤**

在 `export_all` 的 entries 查询中添加过滤：

```rust
    let entries: Vec<crate::models::entry::Entry> = sqlx::query_as(
        "SELECT * FROM entries WHERE user_id = $1 AND deleted_at IS NULL ORDER BY created_at",
    )
```

- [ ] **Step 9: 修改 api/admin.rs reindex 跳过已删除条目**

在 `reindex` 的 entries 查询中添加过滤：

```rust
    let entries: Vec<(uuid::Uuid, Option<String>, Option<String>, String, Option<String>)> =
        sqlx::query_as(
            "SELECT id, title, text_content, url, domain_name FROM entries WHERE deleted_at IS NULL",
        )
```

- [ ] **Step 10: 运行测试确认通过**

Run: `cargo test`
Expected: 全部 PASS

- [ ] **Step 11: Commit**

```bash
git add migrations/009_soft_delete.sql src/models/entry.rs src/api/entries.rs src/api/mod.rs src/api/feed.rs src/api/export.rs src/api/admin.rs tests/integration_soft_delete.rs
git commit -m "feat: implement soft delete with trash, restore, and permanent delete for entries"
```

---

### Task 10: JSONB GIN 索引 [B10]

**Files:**
- Create: `migrations/010_gin_indexes.sql`

- [ ] **Step 1: 创建 migration**

创建 `migrations/010_gin_indexes.sql`：

```sql
-- GIN indexes for JSONB columns to speed up JSONB queries
CREATE INDEX idx_entries_metadata ON entries USING GIN (metadata);
CREATE INDEX idx_tagging_rules_rule ON tagging_rules USING GIN (rule);
```

- [ ] **Step 2: 运行测试确认 migration 通过**

Run: `cargo test`
Expected: 全部 PASS（migration 在测试中自动执行）

- [ ] **Step 3: Commit**

```bash
git add migrations/010_gin_indexes.sql
git commit -m "feat: add GIN indexes on entries.metadata and tagging_rules.rule JSONB columns"
```

---

### Task 11: 复合索引优化 [B11]

**Depends on:** Task 9 (B9)

**Files:**
- Create: `migrations/011_composite_indexes.sql`

- [ ] **Step 1: 创建 migration**

创建 `migrations/011_composite_indexes.sql`：

```sql
-- Drop old indexes from 003_create_entries.sql (superseded by partial indexes below)
-- Old indexes don't include deleted_at IS NULL condition and overlap with new indexes
DROP INDEX IF EXISTS idx_entries_user_created;
DROP INDEX IF EXISTS idx_entries_user_archived;
DROP INDEX IF EXISTS idx_entries_user_starred;

-- Unread list (highest frequency query): non-deleted, non-archived entries
CREATE INDEX idx_entries_user_unread ON entries (user_id, created_at DESC)
    WHERE deleted_at IS NULL AND is_archived = false;

-- Archived list: non-deleted, archived entries
CREATE INDEX idx_entries_user_archived_v2 ON entries (user_id, archived_at DESC)
    WHERE deleted_at IS NULL AND is_archived = true;

-- Starred list: non-deleted, starred entries
CREATE INDEX idx_entries_user_starred_v2 ON entries (user_id, starred_at DESC)
    WHERE deleted_at IS NULL AND is_starred = true;
```

- [ ] **Step 2: 运行测试确认 migration 通过**

Run: `cargo test`
Expected: 全部 PASS

- [ ] **Step 3: Commit**

```bash
git add migrations/011_composite_indexes.sql
git commit -m "feat: replace old indexes with partial composite indexes (include deleted_at filter)"
```

---

### Task 12: 可观测性 Prometheus Metrics [B12]

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/config.rs`
- Modify: `src/main.rs`
- Modify: `src/tasks/fetcher.rs`
- Modify: `tests/common/mod.rs`

- [ ] **Step 1: 写 Config metrics_enabled 字段的测试**

在 `src/config.rs` 的测试模块中添加：

```rust
    #[test]
    fn metrics_disabled_by_default() {
        env::set_var("DATABASE_URL", "postgres://test");
        env::set_var("JWT_SECRET", "a]3kf9$mP!qR7vLx2Yw8Hn5Bc6Tj4Ud0Ze");
        let config = Config::from_env().unwrap();
        assert!(!config.metrics_enabled);
        env::remove_var("JWT_SECRET");
        env::remove_var("DATABASE_URL");
    }

    #[test]
    fn metrics_enabled_via_env() {
        env::set_var("DATABASE_URL", "postgres://test");
        env::set_var("JWT_SECRET", "a]3kf9$mP!qR7vLx2Yw8Hn5Bc6Tj4Ud0Ze");
        env::set_var("METRICS_ENABLED", "true");
        let config = Config::from_env().unwrap();
        assert!(config.metrics_enabled);
        env::remove_var("JWT_SECRET");
        env::remove_var("DATABASE_URL");
        env::remove_var("METRICS_ENABLED");
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test config::tests -- --test-threads=1`
Expected: FAIL -- `metrics_enabled` field does not exist

- [ ] **Step 3: 添加 metrics 依赖**

在 `Cargo.toml` 的 `[dependencies]` 中添加：

```toml
metrics = "0.24"
metrics-exporter-prometheus = "0.16"
```

- [ ] **Step 4: 给 Config 添加 metrics_enabled 字段**

在 `src/config.rs` 的 `Config` struct 中添加：

```rust
    // Metrics
    pub metrics_enabled: bool,
```

在 `from_env` 的 `Ok(Self { ... })` 中添加：

```rust
            metrics_enabled: env::var("METRICS_ENABLED")
                .ok()
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
```

- [ ] **Step 5: 更新 tests/common/mod.rs 中的 Config 构造**

在 `Config { ... }` 中添加：

```rust
            metrics_enabled: false,
```

- [ ] **Step 6: 修改 FetchQueue 添加 Arc<AtomicUsize> 队列深度跟踪**

在 `src/tasks/fetcher.rs` 中：

顶部添加 import：

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
```

修改 `FetchQueue` 结构体：

```rust
#[derive(Clone)]
pub struct FetchQueue {
    tx: mpsc::Sender<FetchJob>,
    depth: Arc<AtomicUsize>,
}

impl FetchQueue {
    pub async fn send(&self, job: FetchJob) -> Result<(), String> {
        self.depth.fetch_add(1, Ordering::Relaxed);
        self.tx.send(job).await.map_err(|e| {
            self.depth.fetch_sub(1, Ordering::Relaxed);
            e.to_string()
        })
    }

    /// Return current queue depth for metrics
    pub fn queue_depth(&self) -> usize {
        self.depth.load(Ordering::Relaxed)
    }
}
```

修改 `start_fetch_worker` 返回带 depth 追踪的 FetchQueue：

```rust
pub fn start_fetch_worker(pool: PgPool, concurrency: usize, image_storage: Arc<dyn ImageStorage>) -> FetchQueue {
    let (tx, rx) = mpsc::channel::<FetchJob>(5000);
    let rx = Arc::new(Mutex::new(rx));
    let depth = Arc::new(AtomicUsize::new(0));

    for _ in 0..concurrency {
        let rx = rx.clone();
        let pool = pool.clone();
        let storage = image_storage.clone();
        let rate_limiter = Arc::new(Mutex::new(DomainRateLimiter::new()));
        let depth = depth.clone();

        tokio::spawn(async move {
            loop {
                let job = {
                    let mut rx = rx.lock().await;
                    rx.recv().await
                };
                match job {
                    Some(job) => {
                        process_job(&pool, &rate_limiter, &storage, &job).await;
                        depth.fetch_sub(1, Ordering::Relaxed);
                    }
                    None => break,
                }
            }
        });
    }

    FetchQueue { tx, depth }
}
```

- [ ] **Step 7: 修改 main.rs 添加 metrics 初始化和路由**

在 `src/main.rs` 中添加 metrics 初始化逻辑。在 `let app = ...` 之前：

```rust
    // Initialize Prometheus metrics if enabled
    if config.metrics_enabled {
        let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
        let handle = builder
            .install_recorder()
            .expect("failed to install Prometheus recorder");

        // The handle is used to render metrics later
        // We store it in a global or pass it through state
        // For simplicity, use a once_cell global
        METRICS_HANDLE.set(handle).ok();
    }
```

在文件顶部（`main` 函数之外）添加全局 handle：

```rust
use std::sync::OnceLock;

static METRICS_HANDLE: OnceLock<metrics_exporter_prometheus::PrometheusHandle> = OnceLock::new();
```

在路由构建之后，如果 metrics 启用，添加 metrics 路由。修改 `main.rs`：

```rust
    let app = lettura::api::router(pool.clone(), config.clone());

    // Add metrics endpoint if enabled
    let app = if config.metrics_enabled {
        app.route("/metrics", axum::routing::get(metrics_handler))
    } else {
        app
    };
```

添加 metrics handler 函数：

```rust
async fn metrics_handler() -> String {
    METRICS_HANDLE
        .get()
        .map(|h| h.render())
        .unwrap_or_default()
}
```

添加 metrics middleware 层。在 router 构建后添加 middleware 层：

```rust
    // Add request metrics middleware if enabled
    let app = if config.metrics_enabled {
        app.layer(axum::middleware::from_fn(request_metrics_middleware))
    } else {
        app
    };
```

添加 middleware 函数：

```rust
async fn request_metrics_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = req.method().to_string();
    let path = normalize_metrics_path(req.uri().path());
    let start = std::time::Instant::now();

    let response = next.run(req).await;

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    metrics::counter!("http_requests_total", "method" => method.clone(), "path" => path.clone(), "status" => status).increment(1);
    metrics::histogram!("http_request_duration_seconds", "method" => method, "path" => path).record(duration);

    response
}

/// Normalize path for metrics to avoid high cardinality from UUIDs
fn normalize_metrics_path(path: &str) -> String {
    // Replace UUID patterns with {id}
    let re = regex::Regex::new(
        r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"
    ).unwrap();
    re.replace_all(path, "{id}").to_string()
}
```

完整的 `src/main.rs`：

```rust
use std::sync::OnceLock;
use tracing_subscriber::EnvFilter;

static METRICS_HANDLE: OnceLock<metrics_exporter_prometheus::PrometheusHandle> = OnceLock::new();

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    dotenvy::dotenv().ok();
    let config = lettura::config::Config::from_env().unwrap_or_else(|e| {
        eprintln!("Configuration error: {e}");
        std::process::exit(1);
    });

    // Initialize Prometheus metrics if enabled
    if config.metrics_enabled {
        let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
        let handle = builder
            .install_recorder()
            .expect("failed to install Prometheus recorder");
        METRICS_HANDLE.set(handle).ok();
    }

    let pool = lettura::db::create_pool(&config).await;
    lettura::db::run_migrations(&pool).await;

    let app = lettura::api::router(pool.clone(), config.clone());

    // Add metrics endpoint and middleware if enabled
    let app = if config.metrics_enabled {
        app.route("/metrics", axum::routing::get(metrics_handler))
            .layer(axum::middleware::from_fn(request_metrics_middleware))
    } else {
        app
    };

    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("failed to bind listener");

    tracing::info!("listening on {}", config.listen_addr);
    axum::serve(listener, app).await.expect("server error");
}

async fn metrics_handler() -> String {
    METRICS_HANDLE
        .get()
        .map(|h| h.render())
        .unwrap_or_default()
}

async fn request_metrics_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = req.method().to_string();
    let path = normalize_metrics_path(req.uri().path());
    let start = std::time::Instant::now();

    let response = next.run(req).await;

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    metrics::counter!("http_requests_total", "method" => method.clone(), "path" => path.clone(), "status" => status).increment(1);
    metrics::histogram!("http_request_duration_seconds", "method" => method, "path" => path).record(duration);

    response
}

/// Normalize path for metrics to avoid high cardinality from UUIDs
fn normalize_metrics_path(path: &str) -> String {
    let re = regex::Regex::new(
        r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"
    ).unwrap();
    re.replace_all(path, "{id}").to_string()
}
```

- [ ] **Step 8: 运行测试确认通过**

Run: `cargo test config::tests -- --test-threads=1`
Expected: PASS

Run: `cargo test`
Expected: 全部 PASS（metrics 在测试中默认关闭，不影响现有测试）

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml src/config.rs src/main.rs src/tasks/fetcher.rs tests/common/mod.rs
git commit -m "feat: add optional Prometheus metrics endpoint with request counter, histogram, and queue depth"
```

---

## Final Verification

- [ ] **Step 1: 全量后端测试**

Run: `cargo test`
Expected: 全部 PASS

- [ ] **Step 2: 前端编译**

Run: `cd web && npm run build`
Expected: BUILD SUCCESS

- [ ] **Step 3: 前端测试**

Run: `cd web && npm test`
Expected: 全部 PASS

- [ ] **Step 4: Docker build 验证**

Run: `docker build -t lettura:test .`
Expected: BUILD SUCCESS（可选，取决于环境）
