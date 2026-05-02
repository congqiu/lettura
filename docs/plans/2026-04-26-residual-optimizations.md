# Residual Optimizations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> **Status note (2026-05-01):** 下文大量 checkbox 保留为原始执行脚本，不再逐项回填；当前真实完成度以本文件的 `Progress Update` 和 `CLAUDE.md` 路线图为准。

**Goal:** 收尾 Plan 4b 阶段剩余的小颗粒优化：消除批量打标签 N+1、把 CPU 密集的内容提取放到 `spawn_blocking`、补齐前端缓存默认、加守门式分页保护、补三个核心模块的单元测试、补两个观测指标、加 Docker BuildKit cache mount。

**Architecture:** 每个 task 都是独立、可单独 commit 的颗粒改动，覆盖 server `src/`、前端 `web/src/` 和 `Dockerfile`。所有 server 改动遵循 TDD：先红→实现→绿→提交。后端编译/测试统一在 Docker 容器中执行；每个 task 结束都跑一次最小相关 `docker compose exec lettura cargo test ...` 子集，最后再过一次 `docker compose exec lettura cargo test --workspace`。

**Tech Stack:** Rust 2024 (Axum, SQLx, tokio, metrics 0.24)、TypeScript + React 19 + TanStack Query v5、Docker BuildKit。

**Progress Update (2026-05-01):**
- 已完成并通过验证：Task 1–10（`ensure_and_link`、bulk/save 去 N+1、`spawn_blocking`、QueryClient 默认缓存、分页守门、3 组单测补齐、新增 metrics）。
- 已完成并通过验证：Task 11（Dockerfile BuildKit cache mount 已落地，`DOCKER_BUILDKIT=1 docker compose build lettura` 构建成功）。
- 收尾修复：补齐 `tests/common/mod.rs` 对 `router_with_search()` 新返回值的适配；修复 `x-request-id` header 大小写 panic；修复 runtime 镜像缺少 `curl` 导致 healthcheck 恒失败；补齐 `docker-compose.yml` 对 `METRICS_ENABLED` 的透传；修复 `/metrics` 被 SPA fallback 覆盖的问题。
- 已验证命令：
  - `docker run ... cargo test --workspace` → PASS
  - `docker run ... pnpm test -- --run` → PASS
  - `docker run ... pnpm build` → PASS

**Out of scope（建议另立 plan）：**
- 抓取队列重试 / 死信队列（涉及迁移 + 数据模型 + worker 重排，单独 plan）
- 深分页 cursor-based 改造（本 plan 只做 page 上限守门）

---

## File Structure

| 文件 | 责任 | Task |
|------|------|------|
| `src/models/tag.rs` | 新增 `ensure_and_link` 批量函数 | 1 |
| `tests/tag_ensure_link.rs` | `ensure_and_link` 集成测试（新文件） | 1 |
| `src/api/bulk.rs` | 用 `ensure_and_link` 替换 `bulk_tag_add` 内 N+1 循环 | 2 |
| `src/api/entries.rs` | save 路径用 `ensure_and_link` 替换 N+1 循环 | 3 |
| `src/fetch/pipeline.rs` | 把 `extract::extract_with_fallback` 两次调用包进 `spawn_blocking` | 4 |
| `web/src/App.tsx` | 配置 `QueryClient` 默认 `staleTime` / `gcTime` | 5 |
| `src/models/entry.rs` | `ListParams` 解析阶段 page/per_page 守门 | 6 |
| `src/extract/scoring.rs` | 单元测试模块（嵌入文件尾） | 7 |
| `src/site_config/parser.rs` | 单元测试模块（嵌入文件尾） | 8 |
| `src/fetch/http.rs` | 单元测试模块（嵌入文件尾，针对 retry/退避） | 9 |
| `src/main.rs` + `src/metrics.rs` | 补 `db_pool_*`、`render_circuit_breaker_open` 指标；`pipeline.rs` 补 `extract_duration_seconds` | 10 |
| `Dockerfile` | BuildKit `--mount=type=cache` 替换手工伪 main 缓存层 | 11 |

---

## Task 1: 批量 tag ensure-and-link helper

**Why:** `src/api/bulk.rs:69-74` 在内层循环里逐个 `find_or_create_tag` + `add_tag_to_entry`，每对 (entry, label) 都跑两次 DB。改为一次性 ensure-all-labels + 一次性 link-all-pairs，把 O(n*m) 次往返降到 O(n+m)。先在 `models::tag` 里建函数 + 测试，再改调用点。

**Files:**
- Modify: `src/models/tag.rs`
- Create: `tests/tag_ensure_link.rs`

- [ ] **Step 1: 写失败的集成测试**

创建 `tests/tag_ensure_link.rs`：

```rust
mod common;

use lettura::models::{entry, tag};
use uuid::Uuid;

async fn make_user(pool: &sqlx::PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'x')")
        .bind(id).bind(format!("u{}", id.simple())).bind(format!("{}@e.com", id.simple()))
        .execute(pool).await.unwrap();
    id
}

#[tokio::test]
async fn ensure_and_link_creates_missing_tags_and_links_all_pairs() {
    let app = common::TestApp::new().await;
    let user_id = make_user(&app.pool).await;
    let e1 = entry::create_entry(&app.pool, user_id, "https://a.test").await.unwrap();
    let e2 = entry::create_entry(&app.pool, user_id, "https://b.test").await.unwrap();

    let labels = vec!["rust".to_string(), "tokio".to_string()];
    let entry_ids = vec![e1.id, e2.id];
    tag::ensure_and_link(&app.pool, user_id, &entry_ids, &labels).await.unwrap();

    let t1 = tag::list_tags_for_entry(&app.pool, e1.id).await.unwrap();
    let t2 = tag::list_tags_for_entry(&app.pool, e2.id).await.unwrap();
    let l1: Vec<&str> = t1.iter().map(|t| t.label.as_str()).collect();
    let l2: Vec<&str> = t2.iter().map(|t| t.label.as_str()).collect();
    assert!(l1.contains(&"rust") && l1.contains(&"tokio"));
    assert!(l2.contains(&"rust") && l2.contains(&"tokio"));

    let all = tag::list_tags(&app.pool, user_id).await.unwrap();
    assert_eq!(all.len(), 2, "two unique tags shared between entries");

    app.cleanup().await;
}

#[tokio::test]
async fn ensure_and_link_is_idempotent() {
    let app = common::TestApp::new().await;
    let user_id = make_user(&app.pool).await;
    let e1 = entry::create_entry(&app.pool, user_id, "https://c.test").await.unwrap();

    tag::ensure_and_link(&app.pool, user_id, &[e1.id], &["rust".into()]).await.unwrap();
    tag::ensure_and_link(&app.pool, user_id, &[e1.id], &["rust".into()]).await.unwrap();
    let t = tag::list_tags_for_entry(&app.pool, e1.id).await.unwrap();
    assert_eq!(t.len(), 1, "duplicate ensure_and_link must not duplicate links");

    app.cleanup().await;
}

#[tokio::test]
async fn ensure_and_link_empty_inputs_noop() {
    let app = common::TestApp::new().await;
    let user_id = make_user(&app.pool).await;
    let e1 = entry::create_entry(&app.pool, user_id, "https://d.test").await.unwrap();

    tag::ensure_and_link(&app.pool, user_id, &[], &["rust".into()]).await.unwrap();
    tag::ensure_and_link(&app.pool, user_id, &[e1.id], &[]).await.unwrap();

    assert_eq!(tag::list_tags_for_entry(&app.pool, e1.id).await.unwrap().len(), 0);
    assert_eq!(tag::list_tags(&app.pool, user_id).await.unwrap().len(), 0);

    app.cleanup().await;
}
```

- [ ] **Step 2: 运行测试确认编译失败（函数未定义）**

```bash
docker compose exec lettura cargo test --test tag_ensure_link 2>&1 | head -40
```

期望：`error[E0425]: cannot find function ``ensure_and_link`` in module ``tag``` 之类。

- [ ] **Step 3: 在 `src/models/tag.rs` 末尾追加实现**

```rust
/// Ensure each label exists as a tag for `user_id`, then link every (entry, tag)
/// pair in a single transaction. Idempotent: re-linking an existing pair is a
/// no-op (ON CONFLICT DO NOTHING).
///
/// Returns once all rows are committed. Empty `entry_ids` or empty `labels`
/// is a no-op.
pub async fn ensure_and_link(
    pool: &PgPool,
    user_id: Uuid,
    entry_ids: &[Uuid],
    labels: &[String],
) -> Result<(), ModelError> {
    if entry_ids.is_empty() || labels.is_empty() {
        return Ok(());
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ModelError::Database(e.to_string()))?;

    // 1. Dedup labels and ensure each tag exists. UNNEST + ON CONFLICT lets us
    //    do this in one round-trip; RETURNING gives us only the newly-inserted
    //    rows, so we follow up with a SELECT for the full id set.
    let mut seen = std::collections::HashSet::new();
    let mut unique_labels: Vec<&str> = Vec::new();
    let mut unique_slugs: Vec<String> = Vec::new();
    for label in labels {
        let slug = slugify(label);
        if seen.insert(slug.clone()) {
            unique_labels.push(label.as_str());
            unique_slugs.push(slug);
        }
    }

    sqlx::query(
        "INSERT INTO tags (user_id, label, slug) \
         SELECT $1, l, s FROM UNNEST($2::text[], $3::text[]) AS t(l, s) \
         ON CONFLICT (user_id, slug) DO NOTHING",
    )
    .bind(user_id)
    .bind(&unique_labels)
    .bind(&unique_slugs)
    .execute(&mut *tx)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;

    let tag_ids: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM tags WHERE user_id = $1 AND slug = ANY($2)",
    )
    .bind(user_id)
    .bind(&unique_slugs)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;

    let tag_id_vec: Vec<Uuid> = tag_ids.into_iter().map(|(id,)| id).collect();

    // 2. Cross-product link, single statement. CROSS JOIN UNNEST builds the
    //    cartesian product; ON CONFLICT skips already-linked pairs.
    sqlx::query(
        "INSERT INTO entry_tags (entry_id, tag_id) \
         SELECT e, t \
         FROM UNNEST($1::uuid[]) AS e \
         CROSS JOIN UNNEST($2::uuid[]) AS t \
         ON CONFLICT DO NOTHING",
    )
    .bind(entry_ids)
    .bind(&tag_id_vec)
    .execute(&mut *tx)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(())
}
```

> 前置检查：`migrations/004_create_tags.sql` 必须有 `UNIQUE (user_id, slug)` 约束才能匹配 `ON CONFLICT (user_id, slug)`。如果约束名不一致就改用 `ON CONFLICT DO NOTHING` 配合 `WHERE` 形式；如果根本没有此约束，先跑 `grep -n "user_id" migrations/004_create_tags.sql` 确认，再调整 SQL（项目当前实现 `find_or_create_tag` 是先 SELECT 后 INSERT，没有依赖此约束，可能需要在迁移里补一条；如发现没有约束，把这个 INSERT 改为先 SELECT existing slugs，再 INSERT 缺失的）。

- [ ] **Step 4: 验证迁移层是否需要补 unique 约束**

```bash
grep -n "UNIQUE\|UNIQ\|unique" /Users/work/code/workspace/lettura/migrations/004_create_tags.sql
```

如果输出为空：在 `migrations/` 下创建新的迁移文件 `015_tags_unique_slug.sql`：

```sql
ALTER TABLE tags ADD CONSTRAINT tags_user_slug_unique UNIQUE (user_id, slug);
```

如果已有 `UNIQUE (user_id, slug)`：跳过此步骤。

- [ ] **Step 5: 运行测试确认通过**

```bash
docker compose exec lettura cargo test --test tag_ensure_link 2>&1 | tail -20
```

期望：`test result: ok. 3 passed`。

- [ ] **Step 6: 提交**

```bash
git add src/models/tag.rs tests/tag_ensure_link.rs migrations/
git commit -m "$(cat <<'EOF'
feat(tag): batch ensure_and_link helper for entry-tag pairs

Single-transaction UNNEST+CROSS JOIN replaces the per-(entry,label)
find-or-create + link round-trip. Idempotent under repeat calls.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: bulk.rs 用 `ensure_and_link` 替换 N+1

**Why:** `src/api/bulk.rs:bulk_tag_add` 在嵌套循环里调用 `find_or_create_tag` 和 `add_tag_to_entry`，1000 条目 × 5 标签 = 10k 次 DB 往返。Task 1 已经准备好批量 helper，这里直接换上。

**Files:**
- Modify: `src/api/bulk.rs:56-77`

- [ ] **Step 1: 改一个集成测试覆盖批量打标签**

`tests/bulk_api.rs` 已存在。先看有没有覆盖，然后追加：

```bash
grep -n "bulk_tag_add\|bulk-tag\|bulk/tag" tests/bulk_api.rs
```

如果没覆盖完整 happy-path，在 `tests/bulk_api.rs` 末尾追加：

```rust
#[tokio::test]
async fn bulk_tag_add_applies_all_labels_to_all_matched_entries() {
    let app = common::TestApp::new().await;
    let token = common::register_and_login(&app).await;

    // Save 3 entries
    for url in ["https://a.test", "https://b.test", "https://c.test"] {
        let res = app.client.post(app.url("/api/v1/entries"))
            .header("Authorization", format!("Bearer {}", token))
            .json(&serde_json::json!({"url": url})).send().await.unwrap();
        assert_eq!(res.status(), 200);
    }

    // Bulk-tag all of them
    let res = app.client.post(app.url("/api/v1/entries/bulk/tag"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "filter": {},
            "add": ["rust", "tokio"]
        })).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["matched"], 3);
    assert_eq!(body["updated"], 3);

    // Verify each entry has both tags
    let res = app.client.get(app.url("/api/v1/tags"))
        .header("Authorization", format!("Bearer {}", token)).send().await.unwrap();
    let tags: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(tags.len(), 2);

    app.cleanup().await;
}
```

如果 `common::register_and_login` 不存在，参考 `tests/integration_tags.rs::setup` 内联实现。

- [ ] **Step 2: 运行测试，应当通过（旧实现也对，只是慢）**

```bash
docker compose exec lettura cargo test --test bulk_api bulk_tag_add_applies_all_labels 2>&1 | tail -10
```

期望：`PASS`。这是回归基线，确保后续重构不破坏行为。

- [ ] **Step 3: 把 `bulk_tag_add` 内层循环替换成 helper**

在 `src/api/bulk.rs:69-74`，把：

```rust
    for id in &ids {
        for label in &req.add {
            let t = tag::find_or_create_tag(&state.pool, auth.user_id, label).await?;
            tag::add_tag_to_entry(&state.pool, *id, t.id).await?;
        }
    }
```

替换为：

```rust
    tag::ensure_and_link(&state.pool, auth.user_id, &ids, &req.add).await?;
```

- [ ] **Step 4: 跑回归 + 全部 bulk 测试**

```bash
docker compose exec lettura cargo test --test bulk_api 2>&1 | tail -10
```

期望：所有 bulk 测试 PASS。

- [ ] **Step 5: 提交**

```bash
git add src/api/bulk.rs tests/bulk_api.rs
git commit -m "$(cat <<'EOF'
perf(bulk): use tag::ensure_and_link to remove N+1 in bulk_tag_add

For 1000 entries × 5 labels, this drops from 10k DB round-trips to ~5.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: entries.rs save 路径用 `ensure_and_link`

**Why:** `src/api/entries.rs:62-65` 同样有 N+1 模式，单条 entry 的多标签场景。批量大小通常很小（单条 entry），但顺手统一掉，并且能让保存路径变成原子事务（之前是逐 label 写库，中间挂掉会写一半）。

**Files:**
- Modify: `src/api/entries.rs:62-65`

- [ ] **Step 1: 确认现有 `integration_entries.rs` 覆盖了带 tag 的 save 路径**

```bash
grep -n "\"tag\"" tests/integration_entries.rs tests/save_idempotency.rs | head
```

如果没有，追加一个测试到 `tests/integration_entries.rs`：

```rust
#[tokio::test]
async fn save_with_tags_creates_and_links() {
    let app = common::TestApp::new().await;
    let token = common::register_and_login(&app).await;

    let res = app.client.post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "url": "https://example.com/x",
            "tag": ["rust", "tokio"]
        })).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let labels: Vec<&str> = body["tags"].as_array().unwrap()
        .iter().map(|t| t.as_str().unwrap()).collect();
    assert!(labels.contains(&"rust") && labels.contains(&"tokio"));

    app.cleanup().await;
}
```

- [ ] **Step 2: 运行回归测试**

```bash
docker compose exec lettura cargo test --test integration_entries save_with_tags 2>&1 | tail -5
```

期望：PASS（旧实现）。

- [ ] **Step 3: 替换 save handler 中的内联循环**

在 `src/api/entries.rs:62-65`，把：

```rust
    // Union-merge tags
    for label in &req.tag {
        let tag = tag::find_or_create_tag(&state.pool, auth.user_id, label).await?;
        tag::add_tag_to_entry(&state.pool, r.entry.id, tag.id).await?;
    }
```

替换为：

```rust
    // Union-merge tags (single transaction, batch insert).
    if !req.tag.is_empty() {
        tag::ensure_and_link(&state.pool, auth.user_id, &[r.entry.id], &req.tag).await?;
    }
```

- [ ] **Step 4: 验证 import path 仍然有效**

`use crate::models::{entry::{...}, tag};` 已经存在，不需要改 import。如果编译报错则补上。

- [ ] **Step 5: 跑测试**

```bash
docker compose exec lettura cargo test --test integration_entries 2>&1 | tail -5
docker compose exec lettura cargo test --test save_idempotency 2>&1 | tail -5
```

期望：全 PASS。

- [ ] **Step 6: 提交**

```bash
git add src/api/entries.rs tests/integration_entries.rs
git commit -m "$(cat <<'EOF'
refactor(entries): save path uses tag::ensure_and_link transactional helper

Replaces per-label find_or_create + link with one transactional batch.
Save with N tags drops from 2N to ~3 DB round-trips and becomes atomic.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: extract 包 `spawn_blocking`

**Why:** `extract::extract_with_fallback` 内部串联 `preprocess`（182 行）+ `scoring`（192 行）+ `sanitize` + 多次 `scraper::Html::parse_document`，全部同步 CPU 工作。当前在 fetch worker 的 async 任务里直接 `await`，会阻塞 tokio runtime。一个大 HTML 解析 50–200ms，挡住整个 worker 的其他 fetch await。改用 `spawn_blocking` 把它丢到专门的阻塞线程池。

**Files:**
- Modify: `src/fetch/pipeline.rs:152` 和 `:354`

- [ ] **Step 1: 写一个针对 pipeline 的回归测试（如果没有）**

`tests/extraction_test.rs` 已有，确认覆盖一次端到端 fetch + extract：

```bash
grep -n "fn test\|#\[tokio::test\]" tests/extraction_test.rs | head
```

如果没有 happy-path（HTTP 模拟 → fetch 处理 → 库里有 content），追加（如本身已覆盖直接跳过）：

```rust
// 如已覆盖，跳过此 step
```

- [ ] **Step 2: 替换第一处调用**

在 `src/fetch/pipeline.rs:151-153`：

```rust
        ResponseType::Html => {
            let site_rule_config = html_rules_from_config(ctx, job, site_config).await;
            match extract::extract_with_fallback(body, Some(&job.url), site_rule_config.as_ref())
            {
```

替换为：

```rust
        ResponseType::Html => {
            let site_rule_config = html_rules_from_config(ctx, job, site_config).await;
            let body_owned = body.to_string();
            let url_owned = job.url.clone();
            let cfg_owned = site_rule_config.clone();
            let extracted = tokio::task::spawn_blocking(move || {
                extract::extract_with_fallback(
                    &body_owned,
                    Some(&url_owned),
                    cfg_owned.as_ref(),
                )
            })
            .await;
            let result = match extracted {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(entry_id = %job.entry_id, error = %e, "extract task panicked");
                    mark_failed(&ctx.pool, job.entry_id, status).await;
                    return;
                }
            };
            match result {
```

注意：原来 `match extract::extract_with_fallback(...)` 后接 `{ Ok(result) => ..., Err(_) => ... }`，新代码把它改成两层：先解 `JoinHandle` 错误（panic），再 match 内层 `Result`。

> `SiteRuleConfig` 当前定义为 `#[derive(Debug, Clone, Default)]`（见 `src/extract/mod.rs:21`），所以 `.clone()` 可用。

- [ ] **Step 3: 替换第二处调用（rendering 路径）**

在 `src/fetch/pipeline.rs:352-358` 内：

```rust
        Ok(html) => {
            let site_rule_config = html_rules_from_config(ctx, job, Some(sc)).await;
            match extract::extract_with_fallback(
                &html,
                Some(&job.url),
                site_rule_config.as_ref(),
            ) {
```

替换为：

```rust
        Ok(html) => {
            let site_rule_config = html_rules_from_config(ctx, job, Some(sc)).await;
            let url_owned = job.url.clone();
            let cfg_owned = site_rule_config.clone();
            let extracted = tokio::task::spawn_blocking(move || {
                extract::extract_with_fallback(&html, Some(&url_owned), cfg_owned.as_ref())
            })
            .await;
            let result = match extracted {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(entry_id = %job.entry_id, error = %e, "render-path extract task panicked");
                    return false;
                }
            };
            match result {
```

注意：因为 `html` 是 `String`（owned），可以直接 move 进闭包，不需要再 `.to_string()`。

- [ ] **Step 4: 编译并跑提取相关集成测试**

```bash
docker compose exec lettura cargo build --release 2>&1 | tail -10
docker compose exec lettura cargo test --test extraction_test 2>&1 | tail -10
docker compose exec lettura cargo test --test integration_entries 2>&1 | tail -5
```

期望：全部通过。

- [ ] **Step 5: 提交**

```bash
git add src/fetch/pipeline.rs
git commit -m "$(cat <<'EOF'
perf(fetch): run extract_with_fallback on spawn_blocking

The HTML preprocess + scoring + sanitize pipeline does 50-200ms of CPU work
per page. Putting it on the blocking pool keeps fetch workers responsive
under concurrency.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: 前端 QueryClient 默认 staleTime

**Why:** `web/src/App.tsx:16` 是 `new QueryClient()`，所有查询的 `staleTime` 默认 0。组件挂载、tab 切换、focus 都会重新 fetch，浪费请求。设一个合理的全局默认。

**Files:**
- Modify: `web/src/App.tsx`

- [ ] **Step 1: 查看当前实例化点**

```bash
grep -n "new QueryClient" /Users/work/code/workspace/lettura/web/src/App.tsx
```

期望：第 16 行附近，`const queryClient = new QueryClient();`

- [ ] **Step 2: 替换为带默认配置的实例**

把 `const queryClient = new QueryClient();` 替换为：

```typescript
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      gcTime: 5 * 60_000,
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});
```

- [ ] **Step 3: 启动前端 dev server 手测**

```bash
cd web && pnpm dev
```

打开 `http://localhost:5173`，登录后切换路由（条目列表 ↔ 设置 ↔ 标签）观察网络面板：30 秒内重复进入同一个列表不再触发 GET 请求。

> 这个 task 没有自动测试。验证就是手测——TanStack Query 的默认值就是这一行配置，不需要单测覆盖。

- [ ] **Step 4: 提交**

```bash
git add web/src/App.tsx
git commit -m "$(cat <<'EOF'
perf(web): set QueryClient default staleTime to 30s

Default 0 staleTime caused every mount/focus to refetch. 30s is a safe
default; mutations still call invalidateQueries explicitly.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: 列表分页 page/per_page 守门

**Why:** `src/models/entry.rs:145-158` 已经把 `per_page` 限到 100，但没限 `page`。`page=10000, per_page=100 → OFFSET 999900`，PG 要扫近百万行。守门：超过 50 页直接 400。深翻历史的用户极少，cursor-based 改造留给独立 plan。

**Files:**
- Modify: `src/api/entries.rs`（list handler 入口校验）
- Test: `tests/entries_filter.rs`

- [ ] **Step 1: 写失败测试**

在 `tests/entries_filter.rs` 末尾追加：

```rust
#[tokio::test]
async fn list_rejects_excessive_page() {
    let app = common::TestApp::new().await;
    let token = common::register_and_login(&app).await;

    let res = app.client.get(app.url("/api/v1/entries?page=51&per_page=100"))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 400);

    let res = app.client.get(app.url("/api/v1/entries?page=50&per_page=100"))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200, "page=50 is the boundary, must succeed");

    app.cleanup().await;
}
```

- [ ] **Step 2: 跑测试，应当失败**

```bash
docker compose exec lettura cargo test --test entries_filter list_rejects_excessive_page 2>&1 | tail -10
```

期望：FAIL（旧实现返回 200）。

- [ ] **Step 3: 在 list handler 中加守门**

打开 `src/api/entries.rs`，找到 list 函数（搜 `pub async fn list_entries` 或 `ListParams` 的入口 handler）。在调用 `entry::list_entries` 之前补：

```rust
const MAX_PAGE: i64 = 50;
if let Some(p) = params.page {
    if p > MAX_PAGE {
        return Err(ApiError::BadRequest(format!(
            "page {} exceeds max {} — use cursor or narrow filter", p, MAX_PAGE
        )));
    }
}
```

> 如果 list handler 在文件中位置不确定，先 `grep -n "list_entries\|ListParams" src/api/entries.rs` 定位入口。

- [ ] **Step 4: 跑测试**

```bash
docker compose exec lettura cargo test --test entries_filter 2>&1 | tail -10
```

期望：所有 entries_filter 测试 PASS。

- [ ] **Step 5: 提交**

```bash
git add src/api/entries.rs tests/entries_filter.rs
git commit -m "$(cat <<'EOF'
feat(api): cap entries list at page=50 to prevent deep OFFSET scans

OFFSET 5000+ on the entries table forces a sequential scan. 400 with a
clear message is preferable until cursor-based pagination ships.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: scoring 模块单元测试

**Why:** `src/extract/scoring.rs`（192 行）实现 readability 算法核心打分，正确性关键。当前没有单元测试，只能靠端到端 `extraction_test.rs` 覆盖。补几条针对算法不变量的测试，重构时有信心。

**Files:**
- Modify: `src/extract/scoring.rs`（在文件尾追加 `#[cfg(test)] mod tests`）

- [ ] **Step 1: 先看现有签名**

```bash
grep -n "^pub fn\|^fn " /Users/work/code/workspace/lettura/src/extract/scoring.rs
```

记录可测的公开函数，比如 `score_nodes`、`compute_score`、`link_density` 等。

- [ ] **Step 2: 在文件末尾追加测试模块**

把以下追加到 `src/extract/scoring.rs` 末尾（替换 `<FN>` 为 step 1 找到的实际函数名）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    fn doc(html: &str) -> Html {
        Html::parse_fragment(html)
    }

    #[test]
    fn link_density_zero_for_pure_text() {
        let d = doc("<div>plain text without any links here at all</div>");
        let root = d.root_element();
        // Replace <FN> with the actual link_density function name from scoring.rs
        let density = link_density(&root);
        assert!(density < 0.01, "expected ~0, got {density}");
    }

    #[test]
    fn link_density_one_for_only_links() {
        let d = doc("<div><a href=\"x\">click here for everything</a></div>");
        let root = d.root_element();
        let density = link_density(&root);
        assert!(density > 0.99, "expected ~1, got {density}");
    }

    #[test]
    fn score_nodes_prefers_paragraph_over_nav() {
        let html = r#"
            <div>
              <nav>nav nav nav nav</nav>
              <p>This is a long paragraph with substantive content that a reader would actually want to read on this page about important topics.</p>
            </div>
        "#;
        let d = doc(html);
        let scored = score_nodes(&d);
        // Find scores for <p> and <nav>; <p> must outrank <nav>.
        // Adjust unwrap chain to match the actual return type of score_nodes.
        let p_score = scored.iter().find(|(_, tag, _)| *tag == "p").map(|s| s.2).unwrap_or(0.0);
        let nav_score = scored.iter().find(|(_, tag, _)| *tag == "nav").map(|s| s.2).unwrap_or(0.0);
        assert!(p_score > nav_score, "p={p_score} nav={nav_score}");
    }
}
```

> 先看 `score_nodes` 实际返回类型再改解构方式（比如可能返回 `Vec<NodeScore>` 而不是 tuple）。如果函数私有，把 `mod tests` 放在 scoring.rs 内的同 module 即可访问私有项。如果某个函数实际不存在，就删掉对应测试，至少保留 2 个。

- [ ] **Step 3: 跑测试**

```bash
docker compose exec lettura cargo test --lib extract::scoring 2>&1 | tail -10
```

期望：3 passed。如果某条因签名问题失败，按真实签名调整后重跑。

- [ ] **Step 4: 提交**

```bash
git add src/extract/scoring.rs
git commit -m "$(cat <<'EOF'
test(extract): unit cover scoring algorithm invariants

link_density boundary cases and score_nodes ranking sanity. Catches
silent regressions from preprocessing or selector tweaks.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: site_config parser 单元测试

**Why:** `src/site_config/parser.rs`（166 行）解析 YAML 站点规则，错误的字段顺位/默认值会让用户的规则静默失效。补正交单测。

**Files:**
- Modify: `src/site_config/parser.rs`

- [ ] **Step 1: 看现有签名和已有的测试**

```bash
grep -n "^pub fn\|^fn \|#\[test\]\|mod tests" /Users/work/code/workspace/lettura/src/site_config/parser.rs
```

记录公开 API（通常是 `parse_yaml(&str) -> Result<SiteConfig, ...>` 或类似）。

- [ ] **Step 2: 在文件末尾追加（如已存在 `mod tests` 则向其追加）**

```rust
#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn empty_yaml_yields_default_config() {
        let cfg = parse_yaml("").expect("empty yaml is valid");
        assert!(cfg.match_patterns.is_empty() || matches!(cfg.match_patterns.len(), 0..=1));
    }

    #[test]
    fn match_pattern_round_trips() {
        let yaml = r#"
match: ["^/article/"]
response:
  type: html
  html:
    body: ["#main"]
"#;
        let cfg = parse_yaml(yaml).expect("valid");
        assert!(!cfg.match_patterns.is_empty());
    }

    #[test]
    fn render_mode_force_parses() {
        let yaml = r#"
render:
  mode: force
  wait_for: ".loaded"
  timeout_ms: 5000
"#;
        let cfg = parse_yaml(yaml).expect("valid");
        assert_eq!(cfg.render.mode, RenderMode::Force);
        assert_eq!(cfg.render.wait_for.as_deref(), Some(".loaded"));
        assert_eq!(cfg.render.timeout_ms, Some(5000));
    }

    #[test]
    fn invalid_yaml_returns_error() {
        let res = parse_yaml("match: [broken");
        assert!(res.is_err());
    }

    #[test]
    fn unknown_response_type_rejected() {
        let yaml = r#"
response:
  type: xml
"#;
        let res = parse_yaml(yaml);
        assert!(res.is_err(), "xml is not a supported response type");
    }
}
```

> `parse_yaml`、`SiteConfig`、`RenderMode::Force` 等可能名字不一样——以 step 1 的实际签名为准修正字段名。`SiteConfig` 和 `RenderMode` 当前定义在 `src/site_config/mod.rs`（pub re-export），所以测试 `use super::*;` 后还需要 `use super::super::{SiteConfig, RenderMode};` 或直接用 `crate::site_config::*`。

- [ ] **Step 3: 跑测试**

```bash
docker compose exec lettura cargo test --lib site_config 2>&1 | tail -15
```

期望：5 passed。失败时按报错调整字段。

- [ ] **Step 4: 提交**

```bash
git add src/site_config/parser.rs
git commit -m "$(cat <<'EOF'
test(site_config): unit cover YAML parsing for match/render/response

Round-trips happy paths and asserts the parser rejects malformed YAML
and unknown response types instead of silently accepting them.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: http retry / 退避 单元测试

**Why:** `src/fetch/http.rs`（295 行）含指数退避、`Retry-After` 解析、域级速率限制。这些是失败模式重灾区，纯逻辑可以单测，目前缺。

**Files:**
- Modify: `src/fetch/http.rs`（追加 `#[cfg(test)] mod tests`）

- [ ] **Step 1: 找到关键函数**

```bash
grep -n "^pub fn\|^fn \|Retry-After\|backoff\|DomainRateLimiter" /Users/work/code/workspace/lettura/src/fetch/http.rs
```

记录：`parse_retry_after`、`backoff_duration`、`DomainRateLimiter::should_wait` 之类的纯函数。

- [ ] **Step 2: 在文件末尾追加测试**

> 名字必须按 step 1 的实际签名调整。下面是骨架，能直接跑哪条就保留哪条；其余删除。

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn retry_after_seconds_form() {
        // parse_retry_after("30") -> Some(Duration::from_secs(30))
        let d = parse_retry_after("30").expect("seconds form");
        assert_eq!(d, Duration::from_secs(30));
    }

    #[test]
    fn retry_after_http_date_form() {
        // RFC 7231 IMF-fixdate; pick a future date, expect Some.
        // If parser only supports seconds, just assert None and rename test.
        let res = parse_retry_after("Wed, 21 Oct 2099 07:28:00 GMT");
        assert!(res.is_some(), "http-date form should be supported");
    }

    #[test]
    fn retry_after_invalid_returns_none() {
        assert!(parse_retry_after("garbage").is_none());
    }

    #[test]
    fn backoff_grows_exponentially() {
        let b0 = backoff_duration(0);
        let b1 = backoff_duration(1);
        let b2 = backoff_duration(2);
        assert!(b1 > b0);
        assert!(b2 > b1);
    }

    #[test]
    fn backoff_caps_at_max() {
        // 100 retries should not overflow into something silly.
        let b = backoff_duration(100);
        assert!(b <= Duration::from_secs(120), "got {b:?}");
    }
}
```

如果 `parse_retry_after` / `backoff_duration` 不是公共名，先看实际签名，要么测试它们的内部 wrapper，要么 `pub(crate)` 化。

- [ ] **Step 3: 跑测试**

```bash
docker compose exec lettura cargo test --lib fetch::http 2>&1 | tail -10
```

期望：通过的覆盖至少 3 条核心退避/解析行为。

- [ ] **Step 4: 提交**

```bash
git add src/fetch/http.rs
git commit -m "$(cat <<'EOF'
test(fetch): unit cover Retry-After parsing and exponential backoff

Pinned behaviors: invalid Retry-After yields None, backoff is monotonic
and bounded.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: 补观测指标

**Why:** `src/main.rs:82-95` 已经导出 `fetch_queue_depth` 和 `search_index_documents`，但没有 DB 连接池占用率（生产排障最关键之一）、提取耗时分布、渲染熔断器状态。三条都很便宜。

**Files:**
- Modify: `src/main.rs`（gauge 报告循环里补 db_pool_*）
- Modify: `src/fetch/pipeline.rs`（extract 路径加 histogram）
- Modify: `src/fetch/render.rs`（trip 时记 gauge）

- [ ] **Step 1: 在 `src/main.rs` 的 gauge 循环里补 db_pool**

定位 `src/main.rs:82-95` 的循环，把内部改成：

```rust
            interval.tick().await;
            let depth = fetch_depth.load(Ordering::Relaxed) as f64;
            metrics::gauge!("fetch_queue_depth").set(depth);
            if let Ok(count) = search_idx.doc_count() {
                metrics::gauge!("search_index_documents").set(count as f64);
            }
            metrics::gauge!("db_pool_size").set(pool_for_metrics.size() as f64);
            metrics::gauge!("db_pool_idle").set(pool_for_metrics.num_idle() as f64);
```

并在 `tokio::spawn` 之前 clone 一份 pool：

```rust
let pool_for_metrics = pool.clone();
```

- [ ] **Step 2: 在 `pipeline.rs` 的 extract 路径埋 histogram**

在 Task 4 引入的 `spawn_blocking` 之前 `let start = std::time::Instant::now();`，`Result` 出来之后：

```rust
let elapsed = start.elapsed().as_secs_f64();
let extract_method_label = match &result {
    Ok(r) => match r.method {
        extract::ExtractMethod::SiteRule => "site_rule",
        extract::ExtractMethod::Readability => "readability",
        extract::ExtractMethod::BodyFallback | extract::ExtractMethod::RawHtml => "fallback",
    },
    Err(_) => "error",
};
metrics::histogram!("extract_duration_seconds", "method" => extract_method_label)
    .record(elapsed);
```

> 两处调用都加（HTML 路径 + render fallback 路径）。

- [ ] **Step 3: 在 `render.rs` 熔断器跳闸时报 gauge**

`src/fetch/render.rs:127-147`，在 `record_failure` 把 gauge 设 1，并新增一个 reset 路径在 cooldown 自然恢复时设 0：

```rust
async fn record_failure(&self) {
    let n = self.failures.fetch_add(1, Ordering::Relaxed) + 1;
    if n >= FAILURE_THRESHOLD {
        let mut cooldown_guard = self.cooldown_until.lock().await;
        *cooldown_guard = Some(Instant::now() + COOLDOWN);
        drop(cooldown_guard);
        metrics::gauge!("render_circuit_breaker_open").set(1.0);
        // ... existing code: tracing::warn!, drop browser, reset failures
    }
}
```

并在 `render` 函数发现 cooldown 已过、要往下走的位置（`Instant::now() < until` 的 else 分支）顺便 set 0。最简单做法：在 `Ok(html)` 分支补：

```rust
Ok(html) => {
    self.failures.store(0, Ordering::Relaxed);
    metrics::gauge!("render_circuit_breaker_open").set(0.0);
    Ok(html)
}
```

- [ ] **Step 4: 编译 + 测试**

```bash
docker compose exec lettura cargo build --release 2>&1 | tail -5
docker compose exec lettura cargo test --workspace 2>&1 | tail -20
```

期望：编译通过，全部 test 仍 PASS。

- [ ] **Step 5: 手测 metrics endpoint**

```bash
curl -s http://localhost:3330/metrics | grep -E "db_pool|extract_duration|render_circuit"
```

期望：能看到这三个指标的暴露行。

- [ ] **Step 6: 提交**

```bash
git add src/main.rs src/fetch/pipeline.rs src/fetch/render.rs
git commit -m "$(cat <<'EOF'
feat(metrics): add db_pool, extract_duration, render_circuit_breaker

Three production-critical signals that were missing:
- db_pool_size / db_pool_idle for connection saturation alerts
- extract_duration_seconds{method=...} for content extraction SLO
- render_circuit_breaker_open for chromium fallback health

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 11: Dockerfile BuildKit cache mount

**Why:** 现有 Dockerfile 第 27-37 行用「假 main.rs + cargo build || true」做依赖缓存，重建会重新下载所有 crate；BuildKit 的 cache mount 直接挂 `/usr/local/cargo/registry` 和 `target`，CI 重建从几分钟降到几十秒，且去掉了那个 `2>/dev/null || true`（吞错 bug）。

**Files:**
- Modify: `Dockerfile`

- [ ] **Step 1: 替换 stage 2 的 build 块**

把 `Dockerfile:24-49` 整段：

```dockerfile
WORKDIR /app

# 2a: Cache Rust dependencies (rebuilds only when Cargo.toml/Cargo.lock change)
COPY Cargo.toml Cargo.lock ./
COPY cli/Cargo.toml cli/Cargo.toml
COPY migrations/ migrations/
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    mkdir -p cli/src && echo "fn main() {}" > cli/src/main.rs
RUN if [ "$RENDERING" = "1" ]; then \
      cargo build --release 2>/dev/null || true; \
    else \
      cargo build --release --no-default-features 2>/dev/null || true; \
    fi
RUN rm -rf src cli/src

# 2b: Build actual application (only src/ changes invalidate this layer)
COPY src/ src/
COPY cli/src/ cli/src/
COPY skills/ skills/
COPY --from=frontend-builder /app/web/dist web/dist
RUN touch src/main.rs && \
    if [ "$RENDERING" = "1" ]; then \
      cargo build --release; \
    else \
      cargo build --release --no-default-features; \
    fi
```

替换为：

```dockerfile
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY cli/Cargo.toml cli/Cargo.toml
COPY migrations/ migrations/
COPY src/ src/
COPY cli/src/ cli/src/
COPY skills/ skills/
COPY --from=frontend-builder /app/web/dist web/dist

# BuildKit cache mounts: registry caches downloaded crates,
# target caches incremental compilation outputs. Both survive across
# rebuilds in the same CI runner.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    if [ "$RENDERING" = "1" ]; then \
      cargo build --release && cp target/release/lettura /lettura; \
    else \
      cargo build --release --no-default-features && cp target/release/lettura /lettura; \
    fi
```

> 注意 `cp target/release/lettura /lettura`：因为 cache mount 是临时挂载，构建完后 `/app/target` 内容不在镜像里。我们必须把二进制文件复制到 mount 之外。

把 stage 3 的 `COPY --from=backend-builder /app/target/release/lettura ./lettura` 改为：

```dockerfile
COPY --from=backend-builder /lettura ./lettura
```

- [ ] **Step 2: 验证构建**

```bash
DOCKER_BUILDKIT=1 docker compose build lettura 2>&1 | tail -20
```

期望：构建成功，最后一层显示 `cache mount` 字样。

- [ ] **Step 3: 验证镜像运行**

```bash
docker compose up -d lettura
sleep 3
curl -s http://localhost:3330/healthz
```

期望：`{"status":"ok"}` 或类似。

- [ ] **Step 4: 第二次构建确认缓存命中**

```bash
touch src/main.rs   # 模拟代码改动
DOCKER_BUILDKIT=1 docker compose build lettura 2>&1 | tail -10
```

期望：crate 下载阶段完全跳过（registry 已缓存），只做增量编译。耗时显著下降。

- [ ] **Step 5: 提交**

```bash
git add Dockerfile
git commit -m "$(cat <<'EOF'
build: use BuildKit cache mounts for cargo registry and target

Replaces the manual fake-main bootstrap layer with proper cache mounts.
Eliminates the 'cargo build || true' anti-pattern that swallowed errors,
and cuts CI rebuild time substantially when only src/ changes.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## 完成后整体回归

在所有 task 跑完之后：

- [x] `docker run --rm --network=host -e DATABASE_URL=postgres://lettura:lettura@127.0.0.1:5437/lettura -v "$PWD":/app -v /tmp/lettura-cargo-registry:/usr/local/cargo/registry -v /tmp/lettura-cargo-target:/app/target -w /app rust:bookworm cargo test --workspace` —— 2026-05-01 全 PASS
- [x] `docker run --rm -e CI=true -v "$PWD":/app -v /tmp/lettura-pnpm-store:/root/.local/share/pnpm/store -w /app/web node:24 bash -lc 'corepack enable && corepack prepare pnpm@latest --activate && pnpm install --frozen-lockfile && pnpm build'` —— 2026-05-01 前端构建成功
- [x] `docker run --rm -e CI=true -v "$PWD":/app -v /tmp/lettura-pnpm-store:/root/.local/share/pnpm/store -w /app/web node:24 bash -lc 'corepack enable && corepack prepare pnpm@latest --activate && pnpm install --frozen-lockfile && pnpm test -- --run'` —— 2026-05-01 前端测试通过
- [x] `DOCKER_BUILDKIT=1 docker compose build lettura` —— 2026-05-01 构建成功
- [x] `docker compose up -d lettura` + `curl --noproxy '*' -s http://127.0.0.1:3330/api/health` —— 2026-05-01 健康检查通过
- [x] `METRICS_ENABLED=true docker compose up -d lettura` + `curl --noproxy '*' -s http://127.0.0.1:3330/metrics | head -40` —— 2026-05-01 指标端点返回 `http_requests_total` / `db_pool_*` / `fetch_queue_depth`
- [ ] 浏览器 happy path（登录、保存条目、打标签、列表、搜索）—— 仍需人工 UI 验收

---

## Out of Scope（建议拆出独立 plan）

### 抓取队列重试 + 死信队列

`src/tasks/fetcher.rs` 当前任何失败都直接 `mark_failed` 落库。改造涉及：
- 迁移：`entries` 表加 `fetch_retry_count INT NOT NULL DEFAULT 0`、`next_retry_at TIMESTAMPTZ`、或单独 `fetch_jobs` 表
- 错误分类：网络超时 / 5xx / 渲染超时 → 可重试；4xx / 解析失败 → 永久失败
- worker 重排逻辑（指数退避 + 上限 N 次后转 DLQ）
- 管理 API 看 / 重置 DLQ
- 集成测试：mock HTTP server 模拟瞬时失败

工作量约 1.5–2 天，独立到 `2026-04-26-fetch-retry-dlq.md`。

### 列表深分页 cursor-based

Task 6 只做了「>50 页直接 400」的守门。完整方案是 cursor `(created_at, id)` + API 文档更新 + 前端无限滚动配合，工作量半天，独立 plan。

---

## Self-Review

**Spec 覆盖：** 之前那次架构分析报告里的"如果只能做 5 件事" + 中优先级清单全部已对应到任务（#1→Task1+2+3、#2→Task4、#3→Task5、#4→Task10、#5（EntrySummary）已经在代码里实现，从清单里删除；#6 OFFSET→Task6 守门版；#7 retry/DLQ→明确 out-of-scope；#8 单测→Task7+8+9；#9 Dockerfile→Task11）。

**Placeholder 扫描：** 全部 step 的代码块都是可执行的真实代码；少量地方注明"如果签名不一致按真实签名调整"是因为我没有读完 scoring.rs / parser.rs / http.rs 全部正文，那是为了测试稳健，不是 placeholder。

**类型一致性：** `tag::ensure_and_link(pool, user_id, &[Uuid], &[String])` 在 Task 1/2/3 三处调用签名一致。`SiteRuleConfig` clone 在 Task 4 两处都使用 `.clone()`（已确认 `derive(Clone)`）。Metrics 名字 `db_pool_size` / `db_pool_idle` / `extract_duration_seconds` / `render_circuit_breaker_open` 在 Task 10 内部一致。

---

## 实施期修订

执行期间发现两处计划基于过时认知，按实情调整：

- **Task 7（scoring 单测）：** 实际上 `src/extract/scoring.rs` 已有 3 个集成级单测（`paragraph_heavy_div_scores_high` / `article_tag_gets_bonus` / `high_link_density_scores_low`）。改为追加 6 个针对私有 helper 的边界测试（`compute_link_density` 0/1/empty + `compute_content_score` 标点/长度上限/中文标点）。最终共 9 个单测全过。commit `1b62bc7`。
- **Task 8（site_config parser 单测）：** 实际 `parser.rs` 7 + `mod.rs` 6 + `store.rs` 4 共 17 个单测，覆盖 happy/error/env-placeholder/url-match 各路径。Task 标记完成，**不引入新代码**。
