# 全局优化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 按优先级依次修复项目中的安全性、稳定性、代码质量和性能问题

**Architecture:** 分 8 个 Task，从高优先级（安全+稳定性）到低优先级（运维+前端），每个 Task 产出一个原子性 commit。Task 1 和 Task 5 都修改 `src/api/mod.rs`，必须串行执行。

**Tech Stack:** Rust 2024, Axum, SQLx, Docker, React 19, TypeScript

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/models/audit_log.rs` | Modify | 添加 `log_success` helper，更新 `fire_and_forget`，消除样板代码 |
| `src/api/mod.rs` | Modify | 提取 `auth_source_str`（Task 1），添加可配置参数（Task 5） |
| `src/api/entries.rs` | Modify | 使用共享 `auth_source_str` + `log_success` helper + 修复 search_index `let _ =` |
| `src/api/pages.rs` | Modify | 同上 + 修复 unwrap + 可配置上传限制 |
| `src/api/tags.rs` | Modify | 同上 |
| `src/api/memos.rs` | Modify | 同上 |
| `src/api/bulk.rs` | Modify | 同上 |
| `src/api/annotations.rs` | Modify | 同上 |
| `src/api/auth.rs` | Modify | 同上（inline auth_source 也替换） |
| `src/api/admin.rs` | Modify | 同上 + 修复 `.ok()` |
| `src/api/tagging_rules.rs` | Modify | 同上 |
| `src/api/site_rules.rs` | Modify | 同上 |
| `src/api/tokens.rs` | Modify | 同上 |
| `src/api/export.rs` | Modify | 同上（inline auth_source 也替换） |
| `src/api/import.rs` | Modify | 修复 `.ok()` + inline auth_source 替换 + 可配置 import 限制 |
| `src/api/backup.rs` | Modify | 修复 `.ok()` + 批量 INSERT + inline auth_source 替换 |
| `src/api/pages_public.rs` | Modify | 修复 unwrap |
| `src/tasks/image_processor.rs` | Modify | 修复 unwrap + 可配置参数 |
| `src/site_config/store.rs` | Modify | 修复 RwLock unwrap |
| `src/spa.rs` | Modify | 修复 HeaderValue unwrap |
| `src/main.rs` | Modify | 修复 tracing unwrap |
| `src/fetch/render.rs` | Modify | 修复 unwrap + 可配置参数 |
| `src/fetch/pipeline.rs` | Modify | 修复 `.ok()` 静默吞错误 |
| `src/config.rs` | Modify | 添加新的可配置参数 + 更新 User-Agent |
| `Dockerfile` | Modify | 添加 USER 指令 + pin pnpm |
| `docker-compose.yml` | Modify | 参数化凭据、移除暴露端口 |
| `.dockerignore` | Modify | 添加缺失条目（不含 skills/） |
| `web/src/api/tags.ts` | Modify | 添加 renameTag、tagStats |
| `web/src/api/entries.ts` | Modify | createEntry 支持 title/tag |

---

### Task 1: 审计日志静默失败修复 + 样板代码消除

**Files:**
- Modify: `src/models/audit_log.rs`
- Modify: `src/api/entries.rs`, `src/api/pages.rs`, `src/api/tags.rs`, `src/api/memos.rs`, `src/api/bulk.rs`, `src/api/annotations.rs`, `src/api/auth.rs`, `src/api/admin.rs`, `src/api/tagging_rules.rs`, `src/api/site_rules.rs`, `src/api/tokens.rs`, `src/api/export.rs`, `src/api/import.rs`, `src/api/backup.rs`

**问题:** ~40 处 `let _ = audit_log::insert(...)` 完全吞掉错误；`InsertAuditLog` 构造高度重复（每处 11 字段，其中 5 个永远是固定值）；`auth_source_str` 在 9 个文件中复制粘贴，另有 4 个文件使用 inline match 模式。

- [ ] **Step 1: 在 `src/models/audit_log.rs` 添加 helper 函数并更新 `fire_and_forget`**

在 `InsertAuditLog` struct 之后添加：

```rust
/// Create an `InsertAuditLog` with common fields pre-filled.
/// Eliminates boilerplate: `status`, `error_message`, `ip_address`,
/// `user_agent`, `request_id` are set to defaults.
pub fn new_entry(
    user_id: Option<Uuid>,
    auth_source: String,
    action: AuditAction,
    resource_type: Option<AuditResourceType>,
    resource_id: Option<Uuid>,
    details: serde_json::Value,
) -> InsertAuditLog {
    InsertAuditLog {
        user_id,
        auth_source,
        action,
        resource_type,
        resource_id,
        status: "success".to_string(),
        details,
        error_message: None,
        ip_address: None,
        user_agent: None,
        request_id: None,
    }
}

/// Insert an audit log entry. On failure, logs a warning instead of propagating the error.
/// Use this for fire-and-forget audit logging where the main operation should not be blocked.
pub async fn log_success(
    pool: &PgPool,
    user_id: Option<Uuid>,
    auth_source: String,
    action: AuditAction,
    resource_type: Option<AuditResourceType>,
    resource_id: Option<Uuid>,
    details: serde_json::Value,
) {
    if let Err(e) = insert(pool, new_entry(user_id, auth_source, action, resource_type, resource_id, details)).await {
        tracing::warn!("audit log insert failed: {e}");
    }
}
```

同时更新已有的 `fire_and_forget` 函数，将 `let _ = insert(...)` 改为 `if let Err(e) = insert(...) { tracing::warn!(...) }`：

```rust
pub fn fire_and_forget(...) {
    tokio::spawn(async move {
        if let Err(e) = insert(&pool, new_entry(user_id, auth_source, action, resource_type, resource_id, details)).await {
            tracing::warn!("audit log insert failed: {e}");
        }
    });
}
```

- [ ] **Step 2: 在 `src/api/mod.rs` 提取共享的 `auth_source_str`**

在 `pub fn router` 之前添加：

```rust
/// Derive the auth source string from the authenticated user.
/// Centralized here to avoid duplication across handler files.
pub fn auth_source_str(auth: &crate::auth::middleware::AuthUser) -> String {
    match auth.source {
        crate::auth::middleware::AuthSource::Jwt => "jwt".to_string(),
        crate::auth::middleware::AuthSource::Pat { .. } => "pat".to_string(),
    }
}
```

- [ ] **Step 3: 逐文件替换所有调用点**

对每个 API handler 文件执行以下替换：

1. **删除文件内的 `fn auth_source_str` 定义**（9 个文件：entries, pages, tags, memos, bulk, annotations, admin, tagging_rules, site_rules）
2. **将 inline `let auth_source = match auth.source { ... }` 替换为 `let auth_source = crate::api::auth_source_str(&auth)`**（4 个文件：auth.rs, import.rs, backup.rs, export.rs）
3. **添加 `use crate::api::auth_source_str;` 到 import**（所有需要 auth_source_str 的文件）
4. **将所有 `let _ = audit_log::insert(&state.pool, InsertAuditLog { ... }).await;` 替换为 `audit_log::log_success(&state.pool, Some(auth.user_id), auth_source_str(&auth), AuditAction::Xxx, Some(AuditResourceType::Xxx), Some(resource_id), details).await;`**
5. **entries.rs 中已有的 `if let Err(e) = audit_log::insert(...)` 也改为使用 `log_success`**（create_entry 第 99 行）
6. **对于 `auth.rs` 中硬编码 `"jwt".to_string()` 的地方**：register/login 场景没有 AuthUser，保持 `"jwt".to_string()`；logout/change_password/regenerate_feed_token 有 AuthUser，改用 `auth_source_str(&auth)`
7. **对于 `tokens.rs` 中硬编码 `"jwt".to_string()` 的地方**，保持原样（PAT 操作限 JWT-only，通过 `require_jwt()` 保证）

- [ ] **Step 4: 运行测试验证**

```bash
docker compose exec lettura cargo test --lib
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(audit): centralize auth_source_str and audit log helper, log failures instead of silently discarding"
```

---

### Task 2: Import/Backup/Pipeline/Entries 中 `.ok()` 和 `let _ =` 静默吞错误修复

**Files:**
- Modify: `src/api/import.rs`（3 处 `.ok()`）
- Modify: `src/api/backup.rs`（2 处 `.ok()`）
- Modify: `src/fetch/pipeline.rs`（3 处 `.ok()`）
- Modify: `src/api/entries.rs`（3 处 `let _ = state.search_index.*`）
- Modify: `src/api/admin.rs`（1 处 `.ok()`）

**问题:** import.rs 3 处 `.ok()` 导致导入失败时条目以空内容/错误状态存在；backup.rs 2 处 `.ok()` 导致恢复后搜索索引可能过时；pipeline.rs 3 处 `.ok()` 导致抓取失败时状态不一致；entries.rs 3 处 `let _ = state.search_index.*` 导致搜索索引与数据库不一致；admin.rs 1 处 `.ok()` 静默吞错误。

- [ ] **Step 1: 修复 `src/api/import.rs`**

将 `update_entry_content` 调用从 `.await.ok()` 改为带日志的错误处理：

```rust
// Before:
update_entry_content(&tx, user_id, &entry_id, &content, ...).await.ok();

// After:
if let Err(e) = update_entry_content(&tx, user_id, &entry_id, &content, ...).await {
    tracing::warn!("import: failed to update content for entry {entry_id}: {e}");
}
```

同样修复 `update_entry` 调用（设置 archived/starred 状态）。

- [ ] **Step 2: 修复 `src/api/backup.rs`**

将搜索索引操作从 `.ok()` 改为带日志的错误处理：

```rust
// Before:
state.search_index.clear().await.ok();

// After:
if let Err(e) = state.search_index.clear().await {
    tracing::warn!("restore: failed to clear search index: {e}");
}
```

同样修复 `search_index.upsert()` 调用。

- [ ] **Step 3: 修复 `src/fetch/pipeline.rs`**

将 `update_entry_content` 和 `add_tag_to_entry` 调用从 `.ok()` 改为带日志的错误处理。3 处：
- 第 267 行：`entry::update_entry_content(...).await.ok()` → `if let Err(e) = ... { tracing::warn!(...) }`
- 第 344 行：`tag::add_tag_to_entry(...).await.ok()` → `if let Err(e) = ... { tracing::warn!(...) }`
- 第 356 行：`entry::update_entry_content(...).await.ok()` in `mark_failed` → `if let Err(e) = ... { tracing::warn!(...) }`

- [ ] **Step 4: 修复 `src/api/entries.rs` 中 search_index 的 `let _ =`**

3 处：
- 第 263 行：`let _ = state.search_index.delete(entry_id).await;` → `if let Err(e) = ... { tracing::warn!("search index delete failed: {e}") }`
- 第 294 行：`let _ = state.search_index.upsert(...)` → `if let Err(e) = ... { tracing::warn!("search index upsert failed: {e}") }`
- 第 333-334 行：`let _ = state.search_index.delete(...)` 和 `let _ = state.search_index.commit()` → 同样改为带日志

- [ ] **Step 5: 修复 `src/api/admin.rs` 第 112 行的 `.ok()`**

- [ ] **Step 6: 运行测试验证**

```bash
docker compose exec lettura cargo test --lib
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "fix: replace silent .ok()/let _ = error swallowing with logged warnings in import/backup/pipeline/entries"
```

---

### Task 3: 生产环境 `.unwrap()` 替换为安全替代

**Files:**
- Modify: `src/api/pages_public.rs`
- Modify: `src/api/pages.rs`（2 处 unwrap）
- Modify: `src/tasks/image_processor.rs`
- Modify: `src/site_config/store.rs`
- Modify: `src/spa.rs`
- Modify: `src/main.rs`
- Modify: `src/fetch/render.rs`

- [ ] **Step 1: 修复 `src/api/pages_public.rs`**

```rust
// Before (line 80):
pw == page_record.password.as_ref().unwrap()

// After:
let Some(ref pw) = page_record.password else { return false };
```

HMAC 的 unwrap 改为 `expect("jwt_secret validated to be >= 32 chars at startup")`。

- [ ] **Step 2: 修复 `src/api/pages.rs`**

第 139 行：`html_files.iter().find(...).unwrap().clone()` → `.expect("html_files is non-empty (checked above)")`
第 142 行：`saved_files.get(&default_entry).unwrap()` → `.expect("default entry was inserted into saved_files")`

- [ ] **Step 3: 修复 `src/tasks/image_processor.rs`**

```rust
// Before:
self.semaphore.clone().acquire_owned().await.unwrap()

// After:
self.semaphore.clone().acquire_owned().await.expect("semaphore should not be closed")
```

- [ ] **Step 4: 修复 `src/site_config/store.rs`**

```rust
// Before:
STORE.write().unwrap()

// After:
STORE.write().expect("site config store lock poisoned")
```

同样修复 `STORE.read()`。

- [ ] **Step 5: 修复 `src/spa.rs`**

将 `HeaderValue::from_str().unwrap()` 改为 `expect("embedded static file produces valid header values")`。这些值来自 rust-embed（编译时嵌入），不可能无效，所以 `expect` 是正确的选择（比返回空 HeaderValue 更安全）。

- [ ] **Step 6: 修复 `src/main.rs`**

将 `.parse().unwrap()` 改为 `.parse().unwrap_or_else(|e| { eprintln!("invalid tracing directive: {e}"); ... })`。

- [ ] **Step 7: 修复 `src/fetch/render.rs`**

将 `guard.as_ref().unwrap().clone()` 改为 `guard.as_ref().expect("browser was just initialized")`。

- [ ] **Step 8: 运行测试验证**

```bash
docker compose exec lettura cargo test --lib
```

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "fix: replace production unwrap() with expect() for meaningful panic messages"
```

---

### Task 4: Docker 安全加固

**Files:**
- Modify: `Dockerfile`
- Modify: `docker-compose.yml`
- Modify: `.dockerignore`

- [ ] **Step 1: Dockerfile 添加非 root 用户**

在 runtime stage 的 `RUN mkdir -p` 之后添加：

```dockerfile
RUN useradd -r -s /bin/false app && chown -R app:app /data
USER app
```

- [ ] **Step 2: docker-compose.yml 参数化凭据 + 移除暴露端口**

```yaml
# Before:
DATABASE_URL: postgres://lettura:lettura@postgres:5436/lettura
POSTGRES_PASSWORD: lettura
ports:
  - "5436:5436"

# After:
DATABASE_URL: postgres://lettura:${POSTGRES_PASSWORD:-lettura}@postgres:5436/lettura
POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-lettura}
# 移除 postgres 的 ports 暴露（生产环境不需要）
# 开发环境如需直连 postgres，可创建 docker-compose.override.yml 重新开启端口：
#   postgres:
#     ports:
#       - "5436:5436"
```

- [ ] **Step 3: 更新 `.dockerignore`**

添加缺失条目。**注意：不添加 `skills/`**，因为 Dockerfile 第 31 行 `COPY skills/ skills/` 需要此目录：

```
extension/
tests/
scripts/
site-configs-local/
.claude/
```

- [ ] **Step 4: 重建并验证容器启动**

```bash
docker compose down
docker compose build lettura
docker compose up -d
docker compose exec lettura curl -f http://localhost:3330/api/health
```

- [ ] **Step 5: Commit**

```bash
git add Dockerfile docker-compose.yml .dockerignore
git commit -m "fix(docker): run as non-root user, parameterize credentials, improve .dockerignore"
```

---

### Task 5: 硬编码运维参数可配置化

**依赖:** Task 1 必须先完成（两者都修改 `src/api/mod.rs`）

**Files:**
- Modify: `src/config.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/tasks/image_processor.rs`
- Modify: `src/fetch/render.rs`
- Modify: `src/api/pages.rs`
- Modify: `src/api/import.rs`

- [ ] **Step 1: 在 `src/config.rs` 添加新配置字段**

```rust
pub struct Config {
    // ... existing fields ...
    /// Fetch worker concurrency (default: 5)
    pub fetch_workers: usize,
    /// Global rate limit per minute (default: 100)
    pub rate_limit_global: u32,
    /// Auth rate limit per minute (default: 10)
    pub rate_limit_auth: u32,
    /// Page upload max size in bytes (default: 10MB)
    pub page_upload_max_bytes: usize,
    /// Import body max size in bytes (default: 500MB)
    pub import_max_bytes: usize,
    /// Image processor max concurrent jobs (default: 4)
    pub image_processor_concurrency: usize,
    /// Image processor max retries (default: 3)
    pub image_processor_max_retries: u32,
    /// Render circuit breaker failure threshold (default: 5)
    pub render_failure_threshold: usize,
    /// Render circuit breaker cooldown in seconds (default: 60)
    pub render_cooldown_secs: u64,
}
```

在 `from_env()` 中添加对应的 env var 解析：

```rust
fetch_workers: env::var("LETTURA_FETCH_WORKERS").ok().and_then(|v| v.parse().ok()).unwrap_or(5),
rate_limit_global: env::var("LETTURA_RATE_LIMIT_GLOBAL").ok().and_then(|v| v.parse().ok()).unwrap_or(100),
rate_limit_auth: env::var("LETTURA_RATE_LIMIT_AUTH").ok().and_then(|v| v.parse().ok()).unwrap_or(10),
page_upload_max_bytes: env::var("LETTURA_PAGE_UPLOAD_MAX_BYTES").ok().and_then(|v| v.parse().ok()).unwrap_or(10 * 1024 * 1024),
import_max_bytes: env::var("LETTURA_IMPORT_MAX_BYTES").ok().and_then(|v| v.parse().ok()).unwrap_or(500 * 1024 * 1024),
image_processor_concurrency: env::var("LETTURA_IMAGE_PROCESSOR_CONCURRENCY").ok().and_then(|v| v.parse().ok()).unwrap_or(4),
image_processor_max_retries: env::var("LETTURA_IMAGE_PROCESSOR_MAX_RETRIES").ok().and_then(|v| v.parse().ok()).unwrap_or(3),
render_failure_threshold: env::var("LETTURA_RENDER_FAILURE_THRESHOLD").ok().and_then(|v| v.parse().ok()).unwrap_or(5),
render_cooldown_secs: env::var("LETTURA_RENDER_COOLDOWN_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(60),
```

- [ ] **Step 2: 为每个新 Config 字段编写解析测试**

在 `src/config.rs` 的 `#[cfg(test)]` 模块中添加测试，验证每个新字段的默认值和自定义值解析。

- [ ] **Step 3: 更新 `src/api/mod.rs` 使用配置值**

```rust
// Before:
fetcher::start_fetch_worker(pool.clone(), 5, ...)
GlobalRateLimit::new(100)
GlobalRateLimit::new(10)

// After:
fetcher::start_fetch_worker(pool.clone(), config.fetch_workers, ...)
GlobalRateLimit::new(config.rate_limit_global)
GlobalRateLimit::new(config.rate_limit_auth)
```

- [ ] **Step 4: 更新 `src/tasks/image_processor.rs` 使用配置值**

将 `MAX_CONCURRENT_JOBS: 4` 和 `MAX_RETRIES: 3` 改为从 `Config` 读取（通过构造函数参数传入）。

- [ ] **Step 5: 更新 `src/fetch/render.rs` 使用配置值**

将 `FAILURE_THRESHOLD` 和 `COOLDOWN` 改为从 `Config` 读取。

- [ ] **Step 6: 更新 `src/api/pages.rs` 使用配置值**

将 `10 * 1024 * 1024` 改为 `state.config.page_upload_max_bytes`。

- [ ] **Step 7: 更新 `src/api/import.rs` 使用配置值**

将 `500 * 1024 * 1024` 改为 `state.config.import_max_bytes`。2 处：第 30 行和第 144 行。

- [ ] **Step 8: 运行测试验证**

```bash
docker compose exec lettura cargo test --lib
```

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat(config): make rate limits, worker counts, upload limits configurable via env vars"
```

---

### Task 6: 备份恢复 N+1 查询优化

**Files:**
- Modify: `src/api/backup.rs`

**问题:** restore 端点逐行 INSERT，大数据集下极慢。

- [ ] **Step 1: 编写批量 INSERT 的测试用例**

在 `tests/` 中添加或扩展集成测试，验证批量恢复的正确性（行数、数据完整性）。

- [ ] **Step 2: 为每个表的恢复循环改为批量 INSERT**

将 `for` 循环中的单条 INSERT 改为使用 `QueryBuilder` 批量 INSERT（每批 100 条）：

```rust
// Before (伪代码):
for entry in entries {
    sqlx::query("INSERT INTO entries ...").bind(...).execute(&mut tx).await?;
}

// After:
for chunk in entries.chunks(100) {
    let mut qb = QueryBuilder::new("INSERT INTO entries (id, user_id, url, ...) ");
    qb.push_values(chunk, |mut b, entry| {
        b.push_bind(entry.id)
         .push_bind(entry.user_id)
         .push_bind(&entry.url)
         // ... other fields
    });
    qb.build().execute(&mut *tx).await?;
}
```

对 entries、tags、entry_tags、annotations、memos、tagging_rules、site_rules 表都做此优化。

- [ ] **Step 3: 运行集成测试验证**

```bash
docker compose -f docker-compose.test.yml up -d postgres-test
docker compose -f docker-compose.test.yml run --rm lettura cargo test --test integration_import_export
```

- [ ] **Step 4: Commit**

```bash
git add src/api/backup.rs
git commit -m "perf(backup): batch INSERT in restore to avoid N+1 query pattern"
```

---

### Task 7: 前端标签 API 对接

**Files:**
- Modify: `web/src/api/tags.ts`
- Modify: `web/src/api/entries.ts`

**问题:** 后端已实现 `GET /tags/stats`、`PATCH /tags/{id}`，但前端 API 层未暴露。

- [ ] **Step 1: 更新 `web/src/api/tags.ts`**

添加缺失的 API 函数：

```typescript
export interface TagStats {
  id: string;
  label: string;
  slug: string;
  entry_count: number;
}

export async function listTagStats(): Promise<TagStats[]> {
  const res = await api.get('/tags/stats');
  return res.data;
}

export async function renameTag(tagId: string, label: string): Promise<Tag> {
  const res = await api.patch(`/tags/${tagId}`, { label });
  return res.data;
}
```

- [ ] **Step 2: 更新 `web/src/api/entries.ts`**

`createEntry` 支持 `title` 和 `tag` 参数。**注意：后端字段名是 `tag`（单数），不是 `tags`**：

```typescript
export interface CreateEntryParams {
  url: string;
  title?: string;
  tag?: string[];  // matches backend CreateEntryRequest.tag
}

export async function createEntry(params: CreateEntryParams | string): Promise<Entry> {
  const body = typeof params === 'string' ? { url: params } : params;
  const res = await api.post('/entries', body);
  return res.data;
}
```

- [ ] **Step 3: 验证前端编译**

```bash
cd web && pnpm run build
```

- [ ] **Step 4: Commit**

```bash
git add web/src/api/tags.ts web/src/api/entries.ts
git commit -m "feat(frontend): add tag stats/rename API and createEntry title/tag support"
```

---

### Task 8: Docker 构建优化 + User-Agent 更新

**Files:**
- Modify: `Dockerfile`
- Modify: `src/config.rs`

- [ ] **Step 1: Dockerfile pin pnpm 版本**

先确认当前 pnpm 最新稳定版本号，然后 pin：

```dockerfile
# Before:
RUN corepack enable && corepack prepare pnpm@latest --activate

# After (verify version first):
RUN corepack enable && corepack prepare pnpm@10.9.0 --activate
```

- [ ] **Step 2: 更新默认 User-Agent**

在 `src/config.rs` 中更新 Chrome 版本号为较新版本：

```rust
// Before: Chrome/131.0.0.0
// After: Chrome/137.0.0.0
"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36"
```

- [ ] **Step 3: 重建并验证**

```bash
docker compose build lettura
docker compose up -d
docker compose exec lettura curl -f http://localhost:3330/api/health
```

- [ ] **Step 4: Commit**

```bash
git add Dockerfile src/config.rs
git commit -m "chore: pin pnpm version in Dockerfile, update default User-Agent"
```
