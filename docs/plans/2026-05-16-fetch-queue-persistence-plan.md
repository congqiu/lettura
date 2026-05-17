# 抓取队列持久化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `FetchQueue` 从进程内 `mpsc::channel` 替换为基于 PostgreSQL `fetch_jobs` 表的持久化队列，崩溃零丢失、多副本可扩展、失败可见可重试。

**Architecture:** 新建 `fetch_jobs` 表，用 `SELECT FOR UPDATE SKIP LOCKED` 做无锁抢占；租约 + `leased_by` 校验防止接管竞态；`LISTEN/NOTIFY`（用 `pg_notify` 内联到入队 SQL）+ 5s 兜底轮询触发出队；`pipeline::process` 改返回 `Result<(), FetchError>` 区分永久错误（删 job + mark failed）与临时错误（backoff → 死信）；`refetch_requested_at` 独立列处理"running 期间用户又点 refetch"场景，避免与 priority 字段语义冲突。不保留 mpsc 兼容路径，回退靠 git revert + redeploy。

**Tech Stack:** Rust 2024, sqlx 0.8（**注意：项目惯例使用 `sqlx::query(...)` 函数形式 + `.bind(...)` 参数，不使用 `query!` 宏。Dockerfile build 阶段不连数据库、未启用 sqlx offline cache，宏会编译失败**），PostgreSQL 16，`tokio-util::CancellationToken`。

**参考 spec:** `docs/specs/2026-05-16-fetch-queue-persistence.md`

**前置：测试环境**

```bash
# 一次性启动集成测试用的 postgres
docker compose -f docker-compose.test.yml up -d postgres-test

# 每个 Task 跑集成测试
docker compose -f docker-compose.test.yml run --rm lettura cargo test --test integration_fetch_jobs

# 单元测试 / 完整 build check
docker build --target test --build-arg "TEST_ARGS=--lib fetch_job" -t lettura-test .
```

每个 Task 完成后单独 commit。commit 信息遵循项目惯例（`feat:` / `refactor:` / `test:`）。

**SQL 调用风格** — 全程使用：

```rust
sqlx::query("SQL with $1, $2 placeholders")
    .bind(param1)
    .bind(param2)
    .execute(pool).await?;

sqlx::query_as::<_, FetchJobRow>("SELECT * FROM ... WHERE id = $1")
    .bind(id)
    .fetch_optional(pool).await?;

sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ...")
    .fetch_one(pool).await?;
```

参考现有 `src/models/entry.rs` 看典型用法。

---

## Task 0: 测试基础设施 — 一次性把 TestApp 改完

**Files:**
- Modify: `tests/common/mod.rs`

**Why first:** 后续 Task 4/6/7/8/10 的测试都需要 (a) 直接造 user/entry 不走 HTTP API，(b) 拿到 `app.config` 用于构造 WorkerConfig，(c) 复用 `app.search_index`。一次改完，避免之间的 commit 顺序依赖。

- [ ] **Step 1: 改 TestApp struct**

```rust
// tests/common/mod.rs
pub struct TestApp {
    pub addr: String,
    pub pool: PgPool,
    pub client: reqwest::Client,
    pub db_name: String,
    pub search_index: SearchIndex,
    pub config: Config,   // ← 新增：暴露给 worker 测试用
    base_url: String,
}
```

在 `TestApp::new` 末尾构造返回值时加 `config: config.clone(),`（config 在前面已经构造）。

- [ ] **Step 2: 加 fixture helper**

```rust
impl TestApp {
    /// Insert a user directly via SQL, bypassing the auth API.
    /// Returns the new user's ID. Use this in DAO tests that don't need
    /// a real password hash or JWT.
    pub async fn create_user(&self, username: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash) \
             VALUES ($1, $2, $3, 'x-not-a-real-hash')"
        )
        .bind(id)
        .bind(username)
        .bind(format!("{}@test.local", username))
        .execute(&self.pool)
        .await
        .expect("create_user insert");
        id
    }

    /// Insert a minimal entry row for the given user, returning the entry ID.
    pub async fn create_entry(&self, user_id: Uuid, url: &str) -> Uuid {
        use sha1::{Digest, Sha1};
        let hashed = hex::encode(Sha1::digest(url.as_bytes()));
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO entries \
             (id, user_id, url, given_url, hashed_url, hashed_given_url) \
             VALUES ($1, $2, $3, $3, $4, $4)"
        )
        .bind(id).bind(user_id).bind(url).bind(hashed)
        .execute(&self.pool)
        .await
        .expect("create_entry insert");
        id
    }
}
```

注意：`hex` 是项目已有依赖（Cargo.toml line ~71）；`sha1` 同样已有。

- [ ] **Step 3: 跑现有测试确认不破**

```bash
docker compose -f docker-compose.test.yml up -d postgres-test
docker build --target test -t lettura-test .
```

Expected: 所有 integration test 仍通过，新字段不影响现有调用。

- [ ] **Step 4: Commit**

```bash
git add tests/common/mod.rs
git commit -m "test: expose TestApp.config + add create_user/create_entry fixtures"
```

---

## Task 1: Migration + model types + SQL 实地验证

**Files:**
- Create: `migrations/021_create_fetch_jobs.sql`
- Create: `src/models/fetch_job.rs`
- Modify: `src/models/mod.rs`（`pub mod fetch_job;`）

- [ ] **Step 1: Migration**

```sql
-- migrations/021_create_fetch_jobs.sql
CREATE TYPE fetch_job_status AS ENUM ('pending', 'running', 'failed', 'dead');

CREATE TABLE fetch_jobs (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entry_id       UUID NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    user_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    url            TEXT NOT NULL,
    status         fetch_job_status NOT NULL DEFAULT 'pending',
    priority       SMALLINT NOT NULL DEFAULT 0,
    attempts       SMALLINT NOT NULL DEFAULT 0,
    max_attempts   SMALLINT NOT NULL DEFAULT 5,
    run_after      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    leased_until   TIMESTAMPTZ,
    leased_by      TEXT,
    last_error     TEXT,
    last_error_at  TIMESTAMPTZ,
    -- Set when a user clicks refetch while this job is in 'running' status.
    -- complete() checks this to decide DELETE vs reset-to-pending, avoiding
    -- overloading the priority column with two meanings.
    refetch_requested_at TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_fetch_jobs_dispatch
    ON fetch_jobs (status, run_after, priority DESC)
    WHERE status IN ('pending', 'failed');

CREATE INDEX idx_fetch_jobs_user_created
    ON fetch_jobs (user_id, created_at DESC);

CREATE INDEX idx_fetch_jobs_entry ON fetch_jobs (entry_id);

CREATE UNIQUE INDEX uq_fetch_jobs_active_entry
    ON fetch_jobs (entry_id)
    WHERE status IN ('pending', 'running', 'failed');
```

- [ ] **Step 2: Model 类型**

```rust
// src/models/fetch_job.rs
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize)]
#[sqlx(type_name = "fetch_job_status", rename_all = "lowercase")]
pub enum FetchJobStatus {
    Pending,
    Running,
    Failed,
    Dead,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct FetchJobRow {
    pub id: Uuid,
    pub entry_id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub status: FetchJobStatus,
    pub priority: i16,
    pub attempts: i16,
    pub max_attempts: i16,
    pub run_after: DateTime<Utc>,
    pub leased_until: Option<DateTime<Utc>>,
    pub leased_by: Option<String>,
    pub last_error: Option<String>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub refetch_requested_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

- [ ] **Step 3: Migration + 核心 SQL 在真实 PG 上跑通**

这一步在 commit 前完成，防止 Task 2 才发现 SQL 语法问题。

```bash
docker compose -f docker-compose.test.yml up -d postgres-test
docker compose -f docker-compose.test.yml exec postgres-test psql -U lettura -d lettura
```

在 psql 里依次执行：

```sql
-- 1. 应用 migration
\i migrations/021_create_fetch_jobs.sql

-- 2. 造测试数据
INSERT INTO users (username, email, password_hash) VALUES ('t', 't@x', 'x') RETURNING id \gset
INSERT INTO entries (user_id, url, given_url, hashed_url, hashed_given_url)
VALUES (:'id', 'https://x.test/', 'https://x.test/', 'h1', 'h1') RETURNING id \gset entry_

-- 3. 验证 ON CONFLICT 语义（这是 reviewer 标红的 critical 点）
INSERT INTO fetch_jobs (entry_id, user_id, url, priority)
VALUES (:'entry_id', :'id', 'https://x.test/', 0);

INSERT INTO fetch_jobs (entry_id, user_id, url, priority)
VALUES (:'entry_id', :'id', 'https://x.test/', 10)
ON CONFLICT ON CONSTRAINT uq_fetch_jobs_active_entry
DO UPDATE SET priority = GREATEST(fetch_jobs.priority, EXCLUDED.priority),
              updated_at = NOW();

SELECT id, priority FROM fetch_jobs WHERE entry_id = :'entry_id';
-- 应该只有 1 行，priority = 10

-- 4. 验证 dead 行不阻塞新入队
UPDATE fetch_jobs SET status = 'dead' WHERE entry_id = :'entry_id';

INSERT INTO fetch_jobs (entry_id, user_id, url, priority)
VALUES (:'entry_id', :'id', 'https://x.test/', 0);

SELECT status, COUNT(*) FROM fetch_jobs WHERE entry_id = :'entry_id' GROUP BY status;
-- 应该有 1 dead + 1 pending

-- 5. 验证 SKIP LOCKED dequeue
BEGIN;
WITH next_job AS (
    SELECT id FROM fetch_jobs
    WHERE status IN ('pending', 'failed') AND run_after <= NOW()
      AND (leased_until IS NULL OR leased_until < NOW())
    ORDER BY priority DESC, run_after ASC
    LIMIT 1 FOR UPDATE SKIP LOCKED
)
UPDATE fetch_jobs j
SET status='running', leased_until=NOW() + INTERVAL '5 minutes',
    leased_by='psql-test', attempts=attempts + 1, updated_at=NOW()
FROM next_job WHERE j.id = next_job.id
RETURNING j.id, j.attempts;
COMMIT;

-- 清理
DELETE FROM fetch_jobs;
DELETE FROM entries;
DELETE FROM users WHERE username = 't';
```

把 psql 输出贴在 commit message 里作为佐证（或 plan 注释里）。

- [ ] **Step 4: Commit**

```bash
git add migrations/021_create_fetch_jobs.sql src/models/fetch_job.rs src/models/mod.rs
git commit -m "feat(fetch): fetch_jobs table + model types

Verified in psql against postgres:16-alpine:
- ON CONFLICT ON CONSTRAINT uq_fetch_jobs_active_entry merges duplicate active enqueues
- dead-state rows do not block new pending row for the same entry
- FOR UPDATE SKIP LOCKED dispatches each job to exactly one transaction"
```

---

## Task 2: Enqueue DAO + upsert + refetch_requested_at 测试

**Files:**
- Create: `tests/integration_fetch_jobs.rs`
- Modify: `src/models/fetch_job.rs`

- [ ] **Step 1: 写失败的集成测试**

```rust
// tests/integration_fetch_jobs.rs
mod common;

use lettura::models::fetch_job::{self, FetchJobStatus};

#[tokio::test]
async fn enqueue_inserts_pending_job() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("alice").await;
    let entry_id = app.create_entry(user_id, "https://example.com/a").await;

    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/a", 0)
        .await.unwrap();

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Pending);
    assert_eq!(row.attempts, 0);
    assert_eq!(row.priority, 0);
    assert_eq!(row.max_attempts, 5);
    assert!(row.refetch_requested_at.is_none());

    app.cleanup().await;
}

#[tokio::test]
async fn enqueue_same_entry_pending_upserts_max_priority() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("bob").await;
    let entry_id = app.create_entry(user_id, "https://example.com/b").await;

    let id1 = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/b", 0)
        .await.unwrap();
    let id2 = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/b", 10)
        .await.unwrap();

    assert_eq!(id1, id2, "ON CONFLICT should target same row");
    let row = fetch_job::find_by_id(&app.pool, id1).await.unwrap().unwrap();
    assert_eq!(row.priority, 10);
    // refetch_requested_at only set when conflicting status='running'
    assert!(row.refetch_requested_at.is_none());

    app.cleanup().await;
}

#[tokio::test]
async fn enqueue_against_running_sets_refetch_signal() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("refetch").await;
    let entry_id = app.create_entry(user_id, "https://example.com/r").await;

    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/r", 0)
        .await.unwrap();
    // Simulate worker picking it up.
    sqlx::query(
        "UPDATE fetch_jobs SET status='running', leased_until=NOW() + INTERVAL '5 minutes', \
         leased_by='worker-x', attempts=1 WHERE id=$1"
    ).bind(id).execute(&app.pool).await.unwrap();

    // User clicks refetch mid-flight.
    let id2 = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/r", 10)
        .await.unwrap();
    assert_eq!(id, id2);

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Running, "running not disturbed");
    assert_eq!(row.priority, 10);
    assert!(row.refetch_requested_at.is_some(), "refetch signal recorded");

    app.cleanup().await;
}

#[tokio::test]
async fn enqueue_does_not_create_second_row_against_dead() {
    // dead is OUTSIDE the partial unique index — so dead row must not block
    // new pending row, but only one pending should exist.
    let app = common::TestApp::new().await;
    let user_id = app.create_user("dead").await;
    let entry_id = app.create_entry(user_id, "https://example.com/d").await;

    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/d", 0)
        .await.unwrap();
    sqlx::query("UPDATE fetch_jobs SET status='dead' WHERE id=$1")
        .bind(id).execute(&app.pool).await.unwrap();

    let id2 = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://example.com/d", 0)
        .await.unwrap();
    assert_ne!(id, id2, "new pending row coexists with dead row");

    let counts: Vec<(FetchJobStatus, i64)> = sqlx::query_as(
        "SELECT status, COUNT(*) FROM fetch_jobs WHERE entry_id=$1 GROUP BY status ORDER BY status"
    ).bind(entry_id).fetch_all(&app.pool).await.unwrap();
    assert_eq!(counts.len(), 2);

    app.cleanup().await;
}
```

- [ ] **Step 2: 跑测试确认失败**

```bash
docker compose -f docker-compose.test.yml run --rm lettura cargo test --test integration_fetch_jobs
```

Expected: 编译失败 — `fetch_job::enqueue` not found.

- [ ] **Step 3: 实现 enqueue + find_by_id**

```rust
// src/models/fetch_job.rs (追加)
use sqlx::PgPool;
use crate::models::error::ModelError;

/// Insert or update a fetch job for the given entry.
///
/// - If no active row exists, INSERT.
/// - If a 'pending' or 'failed' row exists, UPDATE max priority + min run_after.
/// - If a 'running' row exists, set refetch_requested_at so the worker
///   reschedules instead of DELETE on complete.
/// - If only 'dead' rows exist, INSERT a new pending row (dead is not in the
///   partial unique index).
///
/// Sends pg_notify('fetch_jobs_new') so any listening worker wakes immediately.
pub async fn enqueue(
    pool: &PgPool,
    entry_id: Uuid,
    user_id: Uuid,
    url: &str,
    priority: i16,
) -> Result<Uuid, ModelError> {
    let row: (Uuid,) = sqlx::query_as(
        r#"
        WITH inserted AS (
            INSERT INTO fetch_jobs (entry_id, user_id, url, priority)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT ON CONSTRAINT uq_fetch_jobs_active_entry
            DO UPDATE SET
                priority             = GREATEST(fetch_jobs.priority, EXCLUDED.priority),
                run_after            = LEAST(fetch_jobs.run_after, NOW()),
                refetch_requested_at = CASE
                    WHEN fetch_jobs.status = 'running' THEN NOW()
                    ELSE fetch_jobs.refetch_requested_at
                END,
                updated_at           = NOW()
            RETURNING id
        )
        SELECT id FROM inserted
        "#
    )
    .bind(entry_id).bind(user_id).bind(url).bind(priority)
    .fetch_one(pool).await?;

    // Best-effort notification. Failure here is non-fatal: workers have a 5s
    // polling fallback that picks up the job either way.
    let _ = sqlx::query("SELECT pg_notify('fetch_jobs_new', '')")
        .execute(pool).await;

    Ok(row.0)
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<FetchJobRow>, ModelError> {
    let row = sqlx::query_as::<_, FetchJobRow>(
        "SELECT * FROM fetch_jobs WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool).await?;
    Ok(row)
}
```

- [ ] **Step 4: 跑测试**

```bash
docker compose -f docker-compose.test.yml run --rm lettura cargo test --test integration_fetch_jobs
```

Expected: 全部 PASS（4 个测试）。

- [ ] **Step 5: Commit**

```bash
git add src/models/fetch_job.rs tests/integration_fetch_jobs.rs
git commit -m "feat(fetch): enqueue DAO with refetch-aware ON CONFLICT semantics"
```

---

## Task 3: Dequeue with SKIP LOCKED + 并发抢占测试

**Files:**
- Modify: `src/models/fetch_job.rs`
- Modify: `tests/integration_fetch_jobs.rs`

- [ ] **Step 1: 测试**

```rust
// tests/integration_fetch_jobs.rs (追加)
#[tokio::test]
async fn dequeue_skip_locked_no_double_consumption() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("scaler").await;

    for i in 0..100 {
        let url = format!("https://x.test/{i}");
        let eid = app.create_entry(user_id, &url).await;
        fetch_job::enqueue(&app.pool, eid, user_id, &url, 0).await.unwrap();
    }

    let mut handles = vec![];
    for w in 0..5 {
        let p = app.pool.clone();
        handles.push(tokio::spawn(async move {
            let worker_id = format!("worker-{w}");
            let mut consumed = vec![];
            while let Some(job) = fetch_job::dequeue_one(&p, &worker_id).await.unwrap() {
                consumed.push(job.id);
            }
            consumed
        }));
    }

    let mut all_ids = vec![];
    for h in handles { all_ids.extend(h.await.unwrap()); }

    assert_eq!(all_ids.len(), 100);
    let unique: std::collections::HashSet<_> = all_ids.iter().collect();
    assert_eq!(unique.len(), 100, "no duplicates across workers");

    app.cleanup().await;
}

#[tokio::test]
async fn dequeue_skips_jobs_with_future_run_after() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("schedule").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await.unwrap();

    sqlx::query("UPDATE fetch_jobs SET run_after = NOW() + INTERVAL '1 hour' WHERE id = $1")
        .bind(id).execute(&app.pool).await.unwrap();

    assert!(fetch_job::dequeue_one(&app.pool, "worker-1").await.unwrap().is_none());
    app.cleanup().await;
}

#[tokio::test]
async fn dequeue_orders_by_priority_then_run_after() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("prio").await;

    // Older low-priority job
    let e1 = app.create_entry(user_id, "https://x.test/1").await;
    let id1 = fetch_job::enqueue(&app.pool, e1, user_id, "https://x.test/1", 0).await.unwrap();
    sqlx::query("UPDATE fetch_jobs SET created_at = NOW() - INTERVAL '10 minutes' WHERE id=$1")
        .bind(id1).execute(&app.pool).await.unwrap();

    // Newer high-priority job
    let e2 = app.create_entry(user_id, "https://x.test/2").await;
    let id2 = fetch_job::enqueue(&app.pool, e2, user_id, "https://x.test/2", 10).await.unwrap();

    let picked = fetch_job::dequeue_one(&app.pool, "w").await.unwrap().unwrap();
    assert_eq!(picked.id, id2, "higher priority dequeued first");

    app.cleanup().await;
}
```

- [ ] **Step 2: 实现**

```rust
// src/models/fetch_job.rs (追加)
#[derive(Debug, Clone)]
pub struct LeasedJob {
    pub id: Uuid,
    pub entry_id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub attempts: i16,
    pub max_attempts: i16,
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for LeasedJob {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(LeasedJob {
            id: row.try_get("id")?,
            entry_id: row.try_get("entry_id")?,
            user_id: row.try_get("user_id")?,
            url: row.try_get("url")?,
            attempts: row.try_get("attempts")?,
            max_attempts: row.try_get("max_attempts")?,
        })
    }
}

pub async fn dequeue_one(
    pool: &PgPool,
    worker_id: &str,
) -> Result<Option<LeasedJob>, ModelError> {
    let row = sqlx::query_as::<_, LeasedJob>(
        r#"
        WITH next_job AS (
            SELECT id FROM fetch_jobs
            WHERE status IN ('pending', 'failed')
              AND run_after <= NOW()
              AND (leased_until IS NULL OR leased_until < NOW())
            ORDER BY priority DESC, run_after ASC
            LIMIT 1
            FOR UPDATE SKIP LOCKED
        )
        UPDATE fetch_jobs j
        SET status        = 'running',
            leased_until  = NOW() + INTERVAL '5 minutes',
            leased_by     = $1,
            attempts      = attempts + 1,
            updated_at    = NOW()
        FROM next_job
        WHERE j.id = next_job.id
        RETURNING j.id, j.entry_id, j.user_id, j.url, j.attempts, j.max_attempts
        "#
    )
    .bind(worker_id)
    .fetch_optional(pool).await?;
    Ok(row)
}
```

- [ ] **Step 3: 跑测试**

```bash
docker compose -f docker-compose.test.yml run --rm lettura cargo test --test integration_fetch_jobs
# 并发测试至少跑 3 次确认无 flake
for i in 1 2 3; do
  docker compose -f docker-compose.test.yml run --rm lettura \
    cargo test --test integration_fetch_jobs dequeue_skip
done
```

- [ ] **Step 4: Commit**

```bash
git add src/models/fetch_job.rs tests/integration_fetch_jobs.rs
git commit -m "feat(fetch): dequeue_one with SKIP LOCKED for concurrent-safe leasing"
```

---

## Task 4: Complete / Fail / Release / Renew — 全部 `leased_by` 校验

**Files:**
- Modify: `src/models/fetch_job.rs`
- Modify: `tests/integration_fetch_jobs.rs`

**关键设计**：所有改 running 状态行的 SQL 都带 `WHERE leased_by = $worker_id`，防止租约接管后旧 worker 误写。

- [ ] **Step 1: 测试**

```rust
// tests/integration_fetch_jobs.rs (追加)
#[tokio::test]
async fn complete_without_refetch_deletes_row() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("c1").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await.unwrap();
    let _ = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap().unwrap();

    fetch_job::complete(&app.pool, id, "w-1").await.unwrap();

    assert!(fetch_job::find_by_id(&app.pool, id).await.unwrap().is_none());
    app.cleanup().await;
}

#[tokio::test]
async fn complete_with_refetch_signal_resets_to_pending_preserving_priority() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("c2").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await.unwrap();
    let _ = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap();
    // Refetch arrives during processing.
    fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 10).await.unwrap();

    fetch_job::complete(&app.pool, id, "w-1").await.unwrap();

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Pending);
    assert_eq!(row.attempts, 0);
    assert_eq!(row.priority, 10, "priority preserved for re-dispatch");
    assert!(row.refetch_requested_at.is_none(), "signal cleared after honoring");
    app.cleanup().await;
}

#[tokio::test]
async fn complete_rejects_mismatched_worker_id() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("c3").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await.unwrap();
    let _ = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap();

    // Wrong worker tries to complete: no-op.
    fetch_job::complete(&app.pool, id, "w-OTHER").await.unwrap();

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Running, "complete by wrong worker is no-op");
    app.cleanup().await;
}

#[tokio::test]
async fn fail_under_max_uses_60s_min_backoff() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("f1").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await.unwrap();
    let job = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap().unwrap();

    let before = chrono::Utc::now();
    fetch_job::fail(&app.pool, id, "w-1", "boom", job.attempts, job.max_attempts)
        .await.unwrap();

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Failed);
    assert_eq!(row.last_error.as_deref(), Some("boom"));
    let backoff = (row.run_after - before).num_seconds();
    assert!(backoff >= 58 && backoff <= 62, "first failure ~60s, got {backoff}s");
    app.cleanup().await;
}

#[tokio::test]
async fn fail_at_max_attempts_promotes_to_dead() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("f2").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await.unwrap();

    for _ in 0..5 {
        sqlx::query("UPDATE fetch_jobs SET run_after = NOW() WHERE id = $1")
            .bind(id).execute(&app.pool).await.unwrap();
        let job = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap().unwrap();
        fetch_job::fail(&app.pool, id, "w-1", "boom", job.attempts, job.max_attempts)
            .await.unwrap();
    }

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Dead);
    app.cleanup().await;
}

#[tokio::test]
async fn release_restores_to_pending_without_consuming_attempt() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("r1").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await.unwrap();
    let job = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap().unwrap();
    assert_eq!(job.attempts, 1);

    fetch_job::release(&app.pool, id, "w-1").await.unwrap();

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Pending);
    assert_eq!(row.attempts, 0);
    assert!(row.leased_until.is_none());
    app.cleanup().await;
}

#[tokio::test]
async fn release_rejects_mismatched_worker_id() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("r2").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let id = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await.unwrap();
    let _ = fetch_job::dequeue_one(&app.pool, "w-1").await.unwrap();

    fetch_job::release(&app.pool, id, "w-WRONG").await.unwrap();

    let row = fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Running, "release by wrong worker is no-op");
    app.cleanup().await;
}

#[tokio::test]
async fn lease_expired_job_taken_over_by_another_worker() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("lease").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;
    let _ = fetch_job::enqueue(&app.pool, entry_id, user_id, "https://x.test/", 0)
        .await.unwrap();

    let job_a = fetch_job::dequeue_one(&app.pool, "worker-a").await.unwrap().unwrap();
    assert!(fetch_job::dequeue_one(&app.pool, "worker-b").await.unwrap().is_none());

    sqlx::query("UPDATE fetch_jobs SET leased_until = NOW() - INTERVAL '1 second' WHERE id = $1")
        .bind(job_a.id).execute(&app.pool).await.unwrap();

    let job_b = fetch_job::dequeue_one(&app.pool, "worker-b").await.unwrap().unwrap();
    assert_eq!(job_b.id, job_a.id);
    assert_eq!(job_b.attempts, 2);

    // worker-a 现在尝试 complete，因 leased_by 不匹配应静默失败
    fetch_job::complete(&app.pool, job_a.id, "worker-a").await.unwrap();
    let row = fetch_job::find_by_id(&app.pool, job_a.id).await.unwrap().unwrap();
    assert_eq!(row.status, FetchJobStatus::Running);
    assert_eq!(row.leased_by.as_deref(), Some("worker-b"));

    app.cleanup().await;
}
```

- [ ] **Step 2: 实现**

```rust
// src/models/fetch_job.rs (追加)

/// Complete a job atomically:
/// - If refetch_requested_at IS NULL → DELETE
/// - Else → reset to pending (preserve priority for next dispatch)
///
/// Requires leased_by match — protects against late writes from a worker
/// whose lease has been taken over.
pub async fn complete(pool: &PgPool, id: Uuid, worker_id: &str) -> Result<(), ModelError> {
    sqlx::query(
        r#"
        WITH locked AS (
            SELECT id, refetch_requested_at
            FROM fetch_jobs
            WHERE id = $1 AND leased_by = $2 AND status = 'running'
            FOR UPDATE
        ),
        deleted AS (
            DELETE FROM fetch_jobs
            WHERE id IN (SELECT id FROM locked WHERE refetch_requested_at IS NULL)
            RETURNING id
        )
        UPDATE fetch_jobs SET
            status='pending', attempts=0, run_after=NOW(),
            leased_until=NULL, leased_by=NULL,
            refetch_requested_at=NULL,
            last_error=NULL, last_error_at=NULL,
            updated_at=NOW()
        WHERE id IN (SELECT id FROM locked WHERE refetch_requested_at IS NOT NULL)
          AND id NOT IN (SELECT id FROM deleted)
        "#
    )
    .bind(id).bind(worker_id)
    .execute(pool).await?;
    Ok(())
}

pub async fn fail(
    pool: &PgPool,
    id: Uuid,
    worker_id: &str,
    error: &str,
    attempts: i16,
    max_attempts: i16,
) -> Result<(), ModelError> {
    let truncated: String = error.chars().take(1000).collect();
    if attempts >= max_attempts {
        sqlx::query(
            r#"
            UPDATE fetch_jobs
            SET status='dead', last_error=$3, last_error_at=NOW(),
                leased_until=NULL, leased_by=NULL, updated_at=NOW()
            WHERE id=$1 AND leased_by=$2
            "#
        )
        .bind(id).bind(worker_id).bind(truncated)
        .execute(pool).await?;
    } else {
        sqlx::query(
            r#"
            UPDATE fetch_jobs
            SET status='failed', last_error=$3, last_error_at=NOW(),
                run_after = NOW() + (INTERVAL '60 seconds'
                    * POWER(2::numeric, GREATEST(attempts - 1, 0))),
                leased_until=NULL, leased_by=NULL, updated_at=NOW()
            WHERE id=$1 AND leased_by=$2
            "#
        )
        .bind(id).bind(worker_id).bind(truncated)
        .execute(pool).await?;
    }
    Ok(())
}

/// Return a job to the queue without consuming an attempt (graceful shutdown).
pub async fn release(pool: &PgPool, id: Uuid, worker_id: &str) -> Result<(), ModelError> {
    sqlx::query(
        r#"
        UPDATE fetch_jobs
        SET status='pending', leased_until=NULL, leased_by=NULL,
            attempts = GREATEST(attempts - 1, 0), updated_at=NOW()
        WHERE id=$1 AND leased_by=$2 AND status='running'
        "#
    )
    .bind(id).bind(worker_id)
    .execute(pool).await?;
    Ok(())
}

/// Extend the lease for a long-running job. No-op if lease is no longer held.
pub async fn renew_lease(pool: &PgPool, id: Uuid, worker_id: &str) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE fetch_jobs SET leased_until = NOW() + INTERVAL '5 minutes' \
         WHERE id=$1 AND leased_by=$2 AND status='running'"
    )
    .bind(id).bind(worker_id)
    .execute(pool).await?;
    Ok(())
}
```

- [ ] **Step 3: 跑测试**

```bash
docker compose -f docker-compose.test.yml run --rm lettura cargo test --test integration_fetch_jobs
```

Expected: 全部 PASS（8 个新测试 + 之前的）。

- [ ] **Step 4: Commit**

```bash
git add src/models/fetch_job.rs tests/integration_fetch_jobs.rs
git commit -m "feat(fetch): complete/fail/release/renew with leased_by validation"
```

---

## Task 5: 重构 `pipeline::process` 返回 `Result<(), FetchError>`

**Files:**
- Modify: `src/fetch/pipeline.rs`
- Modify: `src/fetch/mod.rs`（`pub use pipeline::FetchError;`）
- Modify: `src/tasks/fetcher.rs`（call site 暂时丢弃 Result）

**判定表**（按 spec 错误分类章节，对应当前 pipeline.rs 行号）：

| 当前 mark_failed 调用点 | 当前 status 参数 | 新返回 |
|------------------------|-----------------|--------|
| line 64 (SSRF in render-force path) | 0 | `Err(Permanent("SSRF blocked: <msg>"))` |
| line 91 (SSRF in static path) | 0 | `Err(Permanent("SSRF blocked: <msg>"))` |
| line 111 (HTTP 4xx static) | 4xx | `Err(Permanent(format!("http {status}")))` |
| line 128 (network/timeout) | 0 | `Err(Transient(<reqwest err>))` |
| line 153 (4xx with retry exhausted) | 4xx | `Err(Permanent(format!("http {status}")))` |
| line 158 (4xx final) | 4xx | `Err(Permanent(format!("http {status}")))` |
| line 167 (5xx with retry exhausted) | 5xx | `Err(Transient(format!("http {status}")))` |
| line 187 (5xx final) | 5xx | `Err(Transient(format!("http {status}")))` |
| line 220 (5xx after render fallback) | 5xx | `Err(Transient(format!("http {status}")))` |

- [ ] **Step 1: 定义 FetchError**

```rust
// src/fetch/pipeline.rs 顶部
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// 4xx, SSRF block, invalid URL, permanent extraction failure.
    /// Worker should not retry; delete the job and mark the entry failed.
    #[error("permanent: {0}")]
    Permanent(String),

    /// 5xx, timeout, network reset, render failure.
    /// Worker should retry with backoff; on max_attempts → dead letter.
    #[error("transient: {0}")]
    Transient(String),
}
```

```rust
// src/fetch/mod.rs
pub use pipeline::FetchError;
```

- [ ] **Step 2: 改 process 签名**

`pub async fn process(ctx: &FetchContext, job: &FetchJob)` →
`pub async fn process(ctx: &FetchContext, job: &FetchJob) -> Result<(), FetchError>`

逐行替换 9 处 `mark_failed(...)` 调用为对应的 `return Err(FetchError::...)`，并删除 mark_failed 调用本身（不再由 pipeline 直接写 entry 失败状态）。函数最后自然 `Ok(())`。

- [ ] **Step 3: `mark_failed` 改 `pub(crate)`**

worker 在收到 Permanent / 死信晋升时仍要调它把 entry 标失败。不删除，改可见性：

```rust
pub(crate) async fn mark_failed(pool: &PgPool, entry_id: Uuid, status: i16) { /* unchanged */ }
```

- [ ] **Step 4: 暂时丢弃 Result 让现有 worker 编译通过**

```rust
// src/tasks/fetcher.rs worker loop 中的调用
let _ = pipeline::process(&ctx, &job).await;
```

这一步是过渡：Task 5 commit 后 server 仍可启动，行为与重构前一致。Task 7 才真正消费 Result。

- [ ] **Step 5: 单元测试**

```rust
// src/fetch/pipeline.rs 末尾 #[cfg(test)] mod tests
#[test]
fn fetch_error_permanent_display() {
    let e = FetchError::Permanent("http 404".into());
    assert_eq!(e.to_string(), "permanent: http 404");
}

#[test]
fn fetch_error_transient_display() {
    let e = FetchError::Transient("timeout".into());
    assert_eq!(e.to_string(), "transient: timeout");
}
```

完整 process 分支集成测试在 Task 7 用 httpmock 覆盖。

- [ ] **Step 6: 跑全量测试**

```bash
docker build --target test -t lettura-test .
docker compose -f docker-compose.test.yml run --rm lettura cargo test --test '*'
```

Expected: 全绿，现有测试零变化。

- [ ] **Step 7: Commit**

```bash
git add src/fetch/pipeline.rs src/fetch/mod.rs src/tasks/fetcher.rs
git commit -m "refactor(fetch): pipeline::process returns Result<(), FetchError>"
```

---

## Task 6: 用 DbQueue 替换 FetchQueue（拆 router、worker 启动外移）

**Files:**
- Modify: `src/tasks/fetcher.rs`（重写 `FetchQueue`）
- Modify: `src/config.rs`（加 4 个字段）
- Modify: `Cargo.toml`（加 `tokio-util`）
- Modify: `src/api/mod.rs`（`router_with_search` 不再启动 worker）
- Modify: `src/main.rs`（独立启动 worker，传 CancellationToken）
- Modify: `tests/common/mod.rs`（Config 字面量加新字段）

- [ ] **Step 1: Cargo.toml + Config 字段**

```toml
# Cargo.toml dependencies 末尾追加
tokio-util = { version = "0.7", features = ["rt"] }
```

```rust
// src/config.rs struct Config 末尾追加
pub fetch_concurrency: usize,
pub fetch_max_attempts: i16,
pub fetch_lease_secs: u64,
pub fetch_dead_ttl_days: i64,

// from_env() 中追加
fetch_concurrency: env::var("LETTURA_FETCH_CONCURRENCY")
    .ok().and_then(|v| v.parse().ok()).unwrap_or(5),
fetch_max_attempts: env::var("LETTURA_FETCH_MAX_ATTEMPTS")
    .ok().and_then(|v| v.parse().ok()).unwrap_or(5),
fetch_lease_secs: env::var("LETTURA_FETCH_LEASE_SECS")
    .ok().and_then(|v| v.parse().ok()).unwrap_or(300),
fetch_dead_ttl_days: env::var("LETTURA_FETCH_DEAD_TTL_DAYS")
    .ok().and_then(|v| v.parse().ok()).unwrap_or(30),
```

`tests/common/mod.rs` 的 Config 字面量同样追加这 4 个字段（默认值同上）。

- [ ] **Step 2: 重写 FetchQueue**

```rust
// src/tasks/fetcher.rs (整体替换)
//! Fetch queue: PostgreSQL-backed durable job queue.
//!
//! Jobs survive process restarts and are dispatched via SELECT FOR UPDATE
//! SKIP LOCKED across all replicas. See docs/specs/2026-05-16-fetch-queue-persistence.md.

use sqlx::PgPool;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use uuid::Uuid;

use crate::models::fetch_job;

#[derive(Debug, Clone)]
pub struct FetchJob {
    pub entry_id: Uuid,
    pub user_id: Uuid,
    pub url: String,
}

#[derive(Clone)]
pub struct FetchQueue {
    pool: PgPool,
    /// Updated periodically by a background task in main.rs (Task 9).
    pub queue_depth: Arc<AtomicUsize>,
}

impl FetchQueue {
    pub fn new(pool: PgPool) -> Self {
        Self { pool, queue_depth: Arc::new(AtomicUsize::new(0)) }
    }

    /// Standard enqueue (priority 0).
    pub async fn send(&self, job: FetchJob) -> Result<(), String> {
        fetch_job::enqueue(&self.pool, job.entry_id, job.user_id, &job.url, 0)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// User-driven refetch (priority 10). Same effect as `send` but jumps the
    /// queue and signals worker mid-flight (via refetch_requested_at).
    pub async fn send_refetch(&self, job: FetchJob) -> Result<(), String> {
        fetch_job::enqueue(&self.pool, job.entry_id, job.user_id, &job.url, 10)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

// NOTE: integration tests for FetchQueue::send live in
// tests/integration_fetch_jobs.rs — they need a real PgPool wired via TestApp.
```

`pg_notify` 已在 `fetch_job::enqueue` 内部发，这里不再重复。

- [ ] **Step 3: 拆 router_with_search，不再启动 worker**

```rust
// src/api/mod.rs router_with_search 中
let fetch_queue = FetchQueue::new(pool.clone());  // 不再调用 start_fetch_worker
```

删除原 `start_fetch_worker` 调用 + 相关 render_service 构造。worker 启动搬到 main.rs（下一 step）。`tests/common/mod.rs::TestApp::new` 调 router_with_search 拿到的 FetchQueue 现在是"只写不消费"，对 DAO 测试无影响；需要消费者的测试自己 spawn worker（Task 7 测试）。

- [ ] **Step 4: main.rs 启动 worker + cancellation**

```rust
// src/main.rs (router 构建之后)
use tokio_util::sync::CancellationToken;

let cancel = CancellationToken::new();

{
    let cancel = cancel.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("shutdown signal received");
        cancel.cancel();
    });
}

let http_client = lettura::fetch::http::build_client(&config);
#[cfg(feature = "rendering")]
let render_service = if config.rendering_runtime_enabled() {
    Some(std::sync::Arc::new(
        lettura::fetch::render::RenderService::new(
            config.chromium_path.clone(),
            config.render_concurrency,
            config.render_timeout_ms,
        )
    ))
} else { None };

lettura::tasks::fetch_worker::spawn_workers(
    lettura::tasks::fetch_worker::WorkerConfig {
        pool: pool.clone(),
        image_storage: storage.clone(),
        search_index: search_index.clone(),
        client: http_client,
        max_retries: config.fetch_max_retries,
        #[cfg(feature = "rendering")]
        render_service,
    },
    config.fetch_concurrency,
    cancel.clone(),
);
```

（`spawn_workers` 在 Task 7 实现。本 Task 暂时把这段代码注释，留待 Task 7 unblocking。或者 Task 7 直接接续，不要中间状态。）

**推荐做法**：Task 6 与 Task 7 合并 commit，作为一个原子变更："拆 worker + 实现 DB worker"。否则 Task 6 commit 后 server 起来没 worker，集成测试会卡。

- [ ] **Step 5: 测试**

```rust
// tests/integration_fetch_jobs.rs (追加)
#[tokio::test]
async fn fetch_queue_send_persists_to_db() {
    let app = common::TestApp::new().await;
    let user_id = app.create_user("queue").await;
    let entry_id = app.create_entry(user_id, "https://x.test/").await;

    let queue = lettura::tasks::fetcher::FetchQueue::new(app.pool.clone());
    queue.send(lettura::tasks::fetcher::FetchJob {
        entry_id, user_id, url: "https://x.test/".into(),
    }).await.unwrap();

    let count: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM fetch_jobs WHERE entry_id=$1"
    ).bind(entry_id).fetch_one(&app.pool).await.unwrap();
    assert_eq!(count, 1);
    app.cleanup().await;
}
```

- [ ] **Step 6: Commit（与 Task 7 一起或合并）**

如果 Task 6 / 7 合并：跳到 Task 7 完成时一并 commit。
如果 Task 6 独立 commit：必须在 main.rs 用 `// TODO Task 7` 注释把 spawn_workers 占位，server 启动后没有 worker，已有集成测试会出现"entry 不被 fetch"的状态变化，需明确接受。

---

## Task 7: DB worker — LISTEN/NOTIFY + lease renewal + 错误分类（与 Task 6 合并 commit）

**Files:**
- Create: `src/tasks/fetch_worker.rs`
- Modify: `src/tasks/mod.rs`（`pub mod fetch_worker;`）
- Modify: `src/main.rs`（启用 spawn_workers）

- [ ] **Step 1: Worker 实现**

```rust
// src/tasks/fetch_worker.rs
//! DB-backed fetch worker.

use crate::fetch::pipeline::{self, FetchContext, FetchError};
use crate::models::fetch_job;
use crate::search::SearchIndex;
use crate::storage::ImageStorage;
use crate::tasks::fetcher::FetchJob;
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone)]
pub struct WorkerConfig {
    pub pool: PgPool,
    pub image_storage: Arc<dyn ImageStorage>,
    pub search_index: SearchIndex,
    pub client: reqwest::Client,
    pub max_retries: u32,
    #[cfg(feature = "rendering")]
    pub render_service: Option<Arc<crate::fetch::render::RenderService>>,
}

pub fn spawn_workers(cfg: WorkerConfig, concurrency: usize, cancel: CancellationToken) {
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".into());
    let pid = std::process::id();

    tracing::info!(
        worker_count = concurrency,
        host = %hostname,
        pid,
        "starting DB-backed fetch workers"
    );

    for w in 0..concurrency {
        let worker_id = format!("{hostname}:{pid}/{w}");
        tokio::spawn(worker_loop(cfg.clone(), worker_id, cancel.clone()));
    }
}

async fn worker_loop(cfg: WorkerConfig, worker_id: String, cancel: CancellationToken) {
    let mut listener = match PgListener::connect_with(&cfg.pool).await {
        Ok(mut l) => {
            if let Err(e) = l.listen("fetch_jobs_new").await {
                tracing::warn!("LISTEN failed, polling only: {e}");
                None
            } else { Some(l) }
        }
        Err(e) => {
            tracing::warn!("PgListener connect failed, polling only: {e}");
            None
        }
    };

    let ctx = FetchContext {
        pool: cfg.pool.clone(),
        image_storage: cfg.image_storage.clone(),
        search_index: cfg.search_index.clone(),
        client: cfg.client.clone(),
        max_retries: cfg.max_retries,
        rate_limiter: Arc::new(Mutex::new(crate::fetch::http::DomainRateLimiter::new())),
        #[cfg(feature = "rendering")]
        render_service: cfg.render_service.clone(),
    };

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            _ = async {
                if let Some(l) = listener.as_mut() {
                    let _ = l.recv().await;
                } else {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            } => {}
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
        }

        loop {
            if cancel.is_cancelled() { break; }
            let job = match fetch_job::dequeue_one(&cfg.pool, &worker_id).await {
                Ok(Some(j)) => j,
                Ok(None) => break,
                Err(e) => { tracing::error!("dequeue failed: {e}"); break; }
            };
            process_one(&ctx, &cfg.pool, &worker_id, job, cancel.clone()).await;
        }
    }

    tracing::info!(worker_id, "fetch worker stopped");
}

async fn process_one(
    ctx: &FetchContext,
    pool: &PgPool,
    worker_id: &str,
    leased: fetch_job::LeasedJob,
    cancel: CancellationToken,
) {
    let job = FetchJob {
        entry_id: leased.entry_id,
        user_id: leased.user_id,
        url: leased.url.clone(),
    };

    let renew = spawn_renewer(pool.clone(), leased.id, worker_id.to_string());

    let result = tokio::select! {
        _ = cancel.cancelled() => {
            // CRITICAL ORDER: abort renew BEFORE release so the renewer cannot
            // race with the release UPDATE.
            renew.abort();
            let _ = fetch_job::release(pool, leased.id, worker_id).await;
            tracing::info!(job_id = %leased.id, "released job on shutdown");
            return;
        }
        r = pipeline::process(ctx, &job) => r,
    };
    renew.abort();

    match result {
        Ok(()) => {
            let _ = fetch_job::complete(pool, leased.id, worker_id).await;
        }
        Err(FetchError::Permanent(msg)) => {
            tracing::info!(job_id = %leased.id, "permanent failure: {msg}");
            pipeline::mark_failed(pool, leased.entry_id, 0).await;
            let _ = fetch_job::complete(pool, leased.id, worker_id).await;
        }
        Err(FetchError::Transient(msg)) => {
            tracing::info!(
                job_id = %leased.id, attempts = leased.attempts,
                "transient failure: {msg}"
            );
            let _ = fetch_job::fail(
                pool, leased.id, worker_id, &msg,
                leased.attempts, leased.max_attempts
            ).await;
            if leased.attempts >= leased.max_attempts {
                pipeline::mark_failed(pool, leased.entry_id, 0).await;
            }
        }
    }
}

fn spawn_renewer(pool: PgPool, job_id: Uuid, worker_id: String) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.tick().await; // skip immediate first tick
        loop {
            interval.tick().await;
            if let Err(e) = fetch_job::renew_lease(&pool, job_id, &worker_id).await {
                tracing::warn!(job_id = %job_id, "lease renewal failed: {e}");
                break;
            }
        }
    })
}
```

- [ ] **Step 2: 集成测试 — 真实 pipeline 跑通**

```rust
// tests/integration_fetch_jobs.rs (追加)
#[tokio::test]
async fn db_worker_processes_real_fetch_to_entry_content() {
    use httpmock::prelude::*;
    use lettura::tasks::fetch_worker;
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    let mock_server = MockServer::start();
    let _m = mock_server.mock(|when, then| {
        when.method(GET).path("/article");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body><article><h1>Test Title</h1>\
                   <p>This is real extracted content for the worker integration test.</p>\
                   </article></body></html>");
    });

    let app = common::TestApp::new().await;
    let user_id = app.create_user("worker").await;
    let url = mock_server.url("/article");
    let entry_id = app.create_entry(user_id, &url).await;

    let queue = lettura::tasks::fetcher::FetchQueue::new(app.pool.clone());
    queue.send(lettura::tasks::fetcher::FetchJob {
        entry_id, user_id, url: url.clone(),
    }).await.unwrap();

    let cancel = CancellationToken::new();
    fetch_worker::spawn_workers(
        fetch_worker::WorkerConfig {
            pool: app.pool.clone(),
            image_storage: std::sync::Arc::from(
                lettura::storage::create_storage(&app.config)
            ),
            search_index: app.search_index.clone(),
            client: reqwest::Client::new(),
            max_retries: 1,
            #[cfg(feature = "rendering")]
            render_service: None,
        },
        1,
        cancel.clone(),
    );

    // Poll for completion (worker drains within ~1s).
    let mut done = false;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let count: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM fetch_jobs WHERE entry_id=$1"
        ).bind(entry_id).fetch_one(&app.pool).await.unwrap();
        if count == 0 { done = true; break; }
    }
    assert!(done, "job did not complete within 8s");

    let row: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT title, text_content FROM entries WHERE id=$1"
    ).bind(entry_id).fetch_one(&app.pool).await.unwrap();

    assert_eq!(row.0.as_deref(), Some("Test Title"));
    assert!(row.1.unwrap_or_default().contains("real extracted content"));

    cancel.cancel();
    tokio::time::sleep(Duration::from_millis(100)).await;
    app.cleanup().await;
}
```

注：`httpmock` 是项目 dev-dependency（参考现有 `tests/extraction_test.rs` 或类似集成测试的用法）。如果当前不在 dev-deps，本 Task 顺手加 `httpmock = "0.7"`。

- [ ] **Step 3: 跑全量测试**

```bash
docker build --target test -t lettura-test .
docker compose -f docker-compose.test.yml run --rm lettura cargo test --test '*'
```

- [ ] **Step 4: Commit（与 Task 6 合并）**

```bash
git add src/tasks/ src/main.rs src/api/mod.rs src/config.rs src/models/ Cargo.toml Cargo.lock tests/
git commit -m "feat(fetch): DB-backed worker replaces mpsc queue

- FetchQueue::send writes to fetch_jobs + pg_notify
- worker uses PgListener with 5s polling fallback
- lease renewal every 60s for long jobs
- error classification routes Permanent → complete + mark_failed,
  Transient → backoff fail, max attempts → dead letter
- graceful cancel releases in-flight job (renew aborted first to
  prevent UPDATE race)
- all worker writes carry leased_by check"
```

---

## Task 8: 优雅停机端到端测试

**Files:**
- Modify: `tests/integration_fetch_jobs.rs`

- [ ] **Step 1: 测试**

```rust
#[tokio::test]
async fn cancel_during_processing_releases_job() {
    use httpmock::prelude::*;
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;
    use lettura::tasks::fetch_worker;
    use lettura::models::fetch_job::FetchJobStatus;

    let mock_server = MockServer::start();
    let _m = mock_server.mock(|when, then| {
        when.method(GET).path("/slow");
        then.status(200).delay(Duration::from_secs(10))
            .body("<html><body>nope</body></html>");
    });

    let app = common::TestApp::new().await;
    let user_id = app.create_user("cancel").await;
    let url = mock_server.url("/slow");
    let entry_id = app.create_entry(user_id, &url).await;

    let queue = lettura::tasks::fetcher::FetchQueue::new(app.pool.clone());
    queue.send(lettura::tasks::fetcher::FetchJob {
        entry_id, user_id, url: url.clone(),
    }).await.unwrap();

    let cancel = CancellationToken::new();
    fetch_worker::spawn_workers(
        fetch_worker::WorkerConfig {
            pool: app.pool.clone(),
            image_storage: std::sync::Arc::from(
                lettura::storage::create_storage(&app.config)
            ),
            search_index: app.search_index.clone(),
            client: reqwest::Client::new(),
            max_retries: 1,
            #[cfg(feature = "rendering")]
            render_service: None,
        },
        1,
        cancel.clone(),
    );

    // Wait until worker has picked up the job.
    let mut picked = false;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let status: Option<FetchJobStatus> = sqlx::query_scalar::<_, FetchJobStatus>(
            "SELECT status FROM fetch_jobs WHERE entry_id=$1"
        ).bind(entry_id).fetch_optional(&app.pool).await.unwrap();
        if matches!(status, Some(FetchJobStatus::Running)) { picked = true; break; }
    }
    assert!(picked, "worker did not pick up job");

    cancel.cancel();
    tokio::time::sleep(Duration::from_millis(500)).await;

    let row: (FetchJobStatus, i16) = sqlx::query_as(
        "SELECT status, attempts FROM fetch_jobs WHERE entry_id=$1"
    ).bind(entry_id).fetch_one(&app.pool).await.unwrap();
    assert_eq!(row.0, FetchJobStatus::Pending);
    assert_eq!(row.1, 0, "released job's attempt count rolled back");

    app.cleanup().await;
}
```

- [ ] **Step 2: 跑测试**

- [ ] **Step 3: Commit**

```bash
git add tests/integration_fetch_jobs.rs
git commit -m "test(fetch): graceful cancel during processing releases job"
```

---

## Task 9: Metrics — queue size gauge + lifecycle counters

**Files:**
- Modify: `src/models/fetch_job.rs`（`count_by_status`）
- Modify: `src/main.rs`（后台 task 上报）
- Modify: `src/tasks/fetch_worker.rs`（process_one 各分支加 counter）

- [ ] **Step 1: DAO**

```rust
pub async fn count_by_status(pool: &PgPool) -> Result<Vec<(FetchJobStatus, i64)>, ModelError> {
    let rows: Vec<(FetchJobStatus, i64)> = sqlx::query_as(
        "SELECT status, COUNT(*) FROM fetch_jobs GROUP BY status"
    )
    .fetch_all(pool).await?;
    Ok(rows)
}
```

- [ ] **Step 2: 后台 task**

```rust
// src/main.rs (附加)
{
    let pool = pool.clone();
    let cancel = cancel.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = interval.tick() => {}
            }
            if let Ok(counts) = lettura::models::fetch_job::count_by_status(&pool).await {
                let mut seen = std::collections::HashSet::new();
                for (status, n) in counts {
                    let label = status_label(status);
                    seen.insert(label);
                    metrics::gauge!("fetch_queue_size", "status" => label).set(n as f64);
                }
                // Reset gauges for statuses with 0 rows (otherwise they stay at the
                // last non-zero value forever).
                for label in ["pending", "running", "failed", "dead"] {
                    if !seen.contains(label) {
                        metrics::gauge!("fetch_queue_size", "status" => label).set(0.0);
                    }
                }
            }
        }
    });
}

fn status_label(s: lettura::models::fetch_job::FetchJobStatus) -> &'static str {
    use lettura::models::fetch_job::FetchJobStatus::*;
    match s { Pending => "pending", Running => "running", Failed => "failed", Dead => "dead" }
}
```

- [ ] **Step 3: Counters 在 process_one 各分支**

```rust
// src/tasks/fetch_worker.rs process_one 加
match result {
    Ok(()) => {
        metrics::counter!("fetch_jobs_completed_total").increment(1);
        let _ = fetch_job::complete(...).await;
    }
    Err(FetchError::Permanent(msg)) => {
        metrics::counter!("fetch_jobs_failed_total", "reason" => "permanent").increment(1);
        ...
    }
    Err(FetchError::Transient(msg)) => {
        metrics::counter!("fetch_jobs_failed_total", "reason" => "transient").increment(1);
        if leased.attempts >= leased.max_attempts {
            metrics::counter!("fetch_jobs_dead_total").increment(1);
        }
        ...
    }
}

// src/tasks/fetcher.rs FetchQueue::send / send_refetch
metrics::counter!("fetch_jobs_enqueued_total").increment(1);
```

- [ ] **Step 4: 浏览器验证**

```bash
./dev.sh up
docker compose exec lettura curl -s http://localhost:3330/metrics | grep '^fetch_'
```

Expected：4 个 gauge + 3 个 counter family。

- [ ] **Step 5: Commit**

```bash
git add src/models/fetch_job.rs src/main.rs src/tasks/
git commit -m "feat(fetch): metrics for queue size and job lifecycle"
```

---

## Task 10: Admin endpoints — list / get / retry / retry-all-dead / delete

**Files:**
- Create: `src/api/fetch_jobs.rs`
- Modify: `src/api/mod.rs`（路由）
- Modify: `src/models/fetch_job.rs`（admin queries）

- [ ] **Step 1: DAO**

```rust
// src/models/fetch_job.rs (追加)
pub async fn list_by_status(
    pool: &PgPool,
    status: Option<FetchJobStatus>,
    limit: i64,
) -> Result<Vec<FetchJobRow>, ModelError> {
    let limit = limit.clamp(1, 500);
    let rows = if let Some(s) = status {
        sqlx::query_as::<_, FetchJobRow>(
            "SELECT * FROM fetch_jobs WHERE status = $1 \
             ORDER BY created_at DESC LIMIT $2"
        ).bind(s).bind(limit).fetch_all(pool).await?
    } else {
        sqlx::query_as::<_, FetchJobRow>(
            "SELECT * FROM fetch_jobs ORDER BY created_at DESC LIMIT $1"
        ).bind(limit).fetch_all(pool).await?
    };
    Ok(rows)
}

pub async fn delete_by_id(pool: &PgPool, id: Uuid) -> Result<(), ModelError> {
    sqlx::query("DELETE FROM fetch_jobs WHERE id = $1")
        .bind(id).execute(pool).await?;
    Ok(())
}

pub async fn retry(pool: &PgPool, id: Uuid) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE fetch_jobs \
         SET status='pending', attempts=0, run_after=NOW(), \
             leased_until=NULL, leased_by=NULL, \
             last_error=NULL, last_error_at=NULL, \
             refetch_requested_at=NULL, updated_at=NOW() \
         WHERE id=$1"
    ).bind(id).execute(pool).await?;
    // Notify so workers pick it up immediately
    let _ = sqlx::query("SELECT pg_notify('fetch_jobs_new', '')").execute(pool).await;
    Ok(())
}

/// Revive at most `limit` dead jobs (default 100 in handler). Returns
/// (retried, remaining_dead) so the operator knows whether to call again.
pub async fn retry_all_dead(pool: &PgPool, limit: i64) -> Result<(u64, i64), ModelError> {
    let limit = limit.clamp(1, 500);
    let result = sqlx::query(
        "UPDATE fetch_jobs \
         SET status='pending', attempts=0, run_after=NOW(), priority=5, \
             leased_until=NULL, leased_by=NULL, updated_at=NOW() \
         WHERE id IN ( \
             SELECT id FROM fetch_jobs WHERE status='dead' \
             ORDER BY last_error_at DESC LIMIT $1 \
         )"
    ).bind(limit).execute(pool).await?;
    let retried = result.rows_affected();

    let remaining: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM fetch_jobs WHERE status='dead'"
    ).fetch_one(pool).await?;

    let _ = sqlx::query("SELECT pg_notify('fetch_jobs_new', '')").execute(pool).await;
    Ok((retried, remaining))
}
```

- [ ] **Step 2: Handlers**

```rust
// src/api/fetch_jobs.rs
use axum::{extract::{Path, Query, State}, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::models::fetch_job::{self, FetchJobRow, FetchJobStatus};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct ListResponse {
    pub items: Vec<FetchJobRow>,
}

#[derive(Serialize)]
pub struct RetryAllResponse {
    pub retried: u64,
    pub remaining_dead: i64,
}

fn require_admin(auth: &AuthUser) -> Result<(), ApiError> {
    if auth.is_admin {
        Ok(())
    } else {
        // PATs always carry is_admin=false (see src/auth/middleware.rs).
        Err(ApiError::Forbidden(
            "admin role required (PAT does not grant admin access)".into()
        ))
    }
}

fn parse_status(s: &str) -> Option<FetchJobStatus> {
    match s {
        "pending" => Some(FetchJobStatus::Pending),
        "running" => Some(FetchJobStatus::Running),
        "failed"  => Some(FetchJobStatus::Failed),
        "dead"    => Some(FetchJobStatus::Dead),
        _ => None,
    }
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse>, ApiError> {
    require_admin(&auth)?;
    let status = q.status.as_deref().and_then(parse_status);
    let items = fetch_job::list_by_status(&state.pool, status, q.limit.unwrap_or(50))
        .await.map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(ListResponse { items }))
}

pub async fn get(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<FetchJobRow>, ApiError> {
    require_admin(&auth)?;
    let row = fetch_job::find_by_id(&state.pool, id)
        .await.map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("fetch job".into()))?;
    Ok(Json(row))
}

pub async fn delete(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_admin(&auth)?;
    fetch_job::delete_by_id(&state.pool, id)
        .await.map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn retry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_admin(&auth)?;
    fetch_job::retry(&state.pool, id)
        .await.map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn retry_all_dead(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<RetryAllResponse>, ApiError> {
    require_admin(&auth)?;
    let (retried, remaining_dead) = fetch_job::retry_all_dead(&state.pool, 100)
        .await.map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(RetryAllResponse { retried, remaining_dead }))
}
```

- [ ] **Step 3: 路由**

```rust
// src/api/mod.rs (在 admin 路由附近)
.route("/api/v1/admin/fetch-jobs", get(fetch_jobs::list))
.route("/api/v1/admin/fetch-jobs/{id}", get(fetch_jobs::get).delete(fetch_jobs::delete))
.route("/api/v1/admin/fetch-jobs/{id}/retry", post(fetch_jobs::retry))
.route("/api/v1/admin/fetch-jobs/retry-all-dead", post(fetch_jobs::retry_all_dead))
```

- [ ] **Step 4: 集成测试**

```rust
// tests/integration_fetch_jobs_admin.rs
mod common;
use serde_json::json;

async fn admin_token(app: &common::TestApp) -> String {
    // The first registered user becomes admin (see src/api/auth.rs line ~104).
    let res = app.client.post(app.url("/api/v1/auth/register"))
        .json(&json!({"username":"admin","email":"a@x.test","password":"password123"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn admin_list_dead_jobs() {
    let app = common::TestApp::new().await;
    let token = admin_token(&app).await;

    // Use the admin's own user_id for the entry (any user works for DAO test).
    let admin_uid: uuid::Uuid = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM users WHERE username='admin'"
    ).fetch_one(&app.pool).await.unwrap();
    let entry_id = app.create_entry(admin_uid, "https://x.test/").await;
    let id = lettura::models::fetch_job::enqueue(
        &app.pool, entry_id, admin_uid, "https://x.test/", 0
    ).await.unwrap();
    sqlx::query("UPDATE fetch_jobs SET status='dead', last_error='boom' WHERE id=$1")
        .bind(id).execute(&app.pool).await.unwrap();

    let res = app.client.get(app.url("/api/v1/admin/fetch-jobs?status=dead"))
        .header("Authorization", format!("Bearer {token}"))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["last_error"], "boom");

    app.cleanup().await;
}

#[tokio::test]
async fn admin_retry_dead_resets_to_pending() {
    let app = common::TestApp::new().await;
    let token = admin_token(&app).await;
    let admin_uid: uuid::Uuid = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM users WHERE username='admin'"
    ).fetch_one(&app.pool).await.unwrap();
    let entry_id = app.create_entry(admin_uid, "https://x.test/").await;
    let id = lettura::models::fetch_job::enqueue(
        &app.pool, entry_id, admin_uid, "https://x.test/", 0
    ).await.unwrap();
    sqlx::query("UPDATE fetch_jobs SET status='dead', attempts=5 WHERE id=$1")
        .bind(id).execute(&app.pool).await.unwrap();

    let res = app.client.post(app.url(&format!("/api/v1/admin/fetch-jobs/{id}/retry")))
        .header("Authorization", format!("Bearer {token}"))
        .send().await.unwrap();
    assert_eq!(res.status(), 204);

    let row = lettura::models::fetch_job::find_by_id(&app.pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, lettura::models::fetch_job::FetchJobStatus::Pending);
    assert_eq!(row.attempts, 0);
    app.cleanup().await;
}

#[tokio::test]
async fn retry_all_dead_capped_at_100() {
    let app = common::TestApp::new().await;
    let token = admin_token(&app).await;
    let admin_uid: uuid::Uuid = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM users WHERE username='admin'"
    ).fetch_one(&app.pool).await.unwrap();

    // Create 150 dead jobs.
    for i in 0..150 {
        let url = format!("https://x.test/{i}");
        let eid = app.create_entry(admin_uid, &url).await;
        let id = lettura::models::fetch_job::enqueue(
            &app.pool, eid, admin_uid, &url, 0
        ).await.unwrap();
        sqlx::query("UPDATE fetch_jobs SET status='dead' WHERE id=$1")
            .bind(id).execute(&app.pool).await.unwrap();
    }

    let res = app.client.post(app.url("/api/v1/admin/fetch-jobs/retry-all-dead"))
        .header("Authorization", format!("Bearer {token}"))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["retried"], 100);
    assert_eq!(body["remaining_dead"], 50);

    app.cleanup().await;
}

#[tokio::test]
async fn non_admin_forbidden() {
    let app = common::TestApp::new().await;
    // First user is admin — register an admin first, then a normie.
    let _ = admin_token(&app).await;
    let res = app.client.post(app.url("/api/v1/auth/register"))
        .json(&json!({"username":"normie","email":"n@x.test","password":"password123"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let token = body["access_token"].as_str().unwrap();

    let res = app.client.get(app.url("/api/v1/admin/fetch-jobs"))
        .header("Authorization", format!("Bearer {token}"))
        .send().await.unwrap();
    assert_eq!(res.status(), 403);
    app.cleanup().await;
}
```

- [ ] **Step 5: 跑测试 + Commit**

```bash
docker compose -f docker-compose.test.yml run --rm lettura cargo test --test integration_fetch_jobs_admin
git add src/api/fetch_jobs.rs src/api/mod.rs src/models/fetch_job.rs tests/integration_fetch_jobs_admin.rs
git commit -m "feat(fetch): admin endpoints with capped retry_all_dead (max 100/call)"
```

---

## Task 11: 死信清理 + 文档

**Files:**
- Modify: `src/main.rs`
- Modify: `CLAUDE.md`
- Modify: `.env.example`

- [ ] **Step 1: 清理 task**

```rust
// src/main.rs (附加)
{
    let pool = pool.clone();
    let cancel = cancel.clone();
    let ttl_days = config.fetch_dead_ttl_days;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = interval.tick() => {}
            }
            let r = sqlx::query(
                "DELETE FROM fetch_jobs WHERE status='dead' \
                 AND last_error_at < NOW() - ($1 || ' days')::interval"
            ).bind(ttl_days.to_string()).execute(&pool).await;
            if let Ok(r) = r {
                if r.rows_affected() > 0 {
                    tracing::info!(deleted = r.rows_affected(), "cleaned up dead fetch jobs");
                }
            }
        }
    });
}
```

- [ ] **Step 2: CLAUDE.md "可选环境变量"表追加**

```markdown
| `LETTURA_FETCH_CONCURRENCY` | 5 | 抓取 worker 并发数 |
| `LETTURA_FETCH_MAX_ATTEMPTS` | 5 | 单 job 最大重试（超过进死信） |
| `LETTURA_FETCH_LEASE_SECS` | 300 | job 租约初始秒数 |
| `LETTURA_FETCH_DEAD_TTL_DAYS` | 30 | 死信保留天数 |
```

并在合适位置加一小节"## 抓取队列"，简要说明 `fetch_jobs` 表是持久化队列、admin endpoint 位置、回退方式（git revert + redeploy，迁移表保留）。

- [ ] **Step 3: `.env.example`**

```
# Fetch queue tuning (defaults shown)
# LETTURA_FETCH_CONCURRENCY=5
# LETTURA_FETCH_MAX_ATTEMPTS=5
# LETTURA_FETCH_LEASE_SECS=300
# LETTURA_FETCH_DEAD_TTL_DAYS=30
```

- [ ] **Step 4: Commit**

```bash
git add src/main.rs CLAUDE.md .env.example
git commit -m "feat(fetch): dead letter cleanup + docs for queue config"
```

---

## Task 12: 前端 admin panel — 抓取队列 tab

**Files:**
- Create: `web/src/api/fetchJobs.ts`
- Create: `web/src/components/settings/FetchJobsPanel.tsx`
- Modify: `web/src/pages/SettingsPage.tsx`（admin tab）

- [ ] **Step 1: 确认 client export 名**

```bash
grep "^export\|^const api" web/src/api/client.ts | head -5
```

实际是 `axios.create({ baseURL: '/api/v1' })` 赋给 `api`，**导出方式根据现有文件确认是 default 还是命名导出**。下面假设 default：

- [ ] **Step 2: API client**

```ts
// web/src/api/fetchJobs.ts
import api from './client';

export interface FetchJob {
  id: string;
  entry_id: string;
  user_id: string;
  url: string;
  status: 'pending' | 'running' | 'failed' | 'dead';
  attempts: number;
  max_attempts: number;
  last_error: string | null;
  last_error_at: string | null;
  created_at: string;
}

export interface ListResponse { items: FetchJob[]; }
export interface RetryAllResponse { retried: number; remaining_dead: number; }

// baseURL is already /api/v1 — handler paths start with /admin/...
export const listFetchJobs = (status?: string, limit = 100) =>
  api.get<ListResponse>('/admin/fetch-jobs', { params: { status, limit } });

export const retryFetchJob = (id: string) =>
  api.post<void>(`/admin/fetch-jobs/${id}/retry`);

export const retryAllDead = () =>
  api.post<RetryAllResponse>('/admin/fetch-jobs/retry-all-dead');

export const deleteFetchJob = (id: string) =>
  api.delete<void>(`/admin/fetch-jobs/${id}`);
```

- [ ] **Step 3: Panel 组件**

```tsx
// web/src/components/settings/FetchJobsPanel.tsx
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { listFetchJobs, retryFetchJob, retryAllDead, FetchJob } from '@/api/fetchJobs';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { toast } from 'sonner';

const STATUSES = ['failed', 'dead', 'running', 'pending'] as const;
type Status = typeof STATUSES[number];

export function FetchJobsPanel() {
  const [status, setStatus] = useState<Status>('failed');
  const qc = useQueryClient();

  const { data, isLoading } = useQuery({
    queryKey: ['fetch-jobs', status],
    queryFn: () => listFetchJobs(status).then(r => r.data.items),
    refetchInterval: 5000,
  });

  const retryOne = useMutation({
    mutationFn: retryFetchJob,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['fetch-jobs'] });
      toast.success('已重新入队');
    },
    onError: (e: any) => toast.error(`重试失败: ${e?.message ?? 'unknown'}`),
  });

  const retryAll = useMutation({
    mutationFn: retryAllDead,
    onSuccess: (r) => {
      qc.invalidateQueries({ queryKey: ['fetch-jobs'] });
      const remaining = r.data.remaining_dead;
      toast.success(
        `已复活 ${r.data.retried} 个死信任务` +
        (remaining > 0 ? `（还有 ${remaining} 个未复活，再点击一次继续）` : '')
      );
    },
  });

  return (
    <div className="space-y-4">
      <div className="flex gap-2">
        {STATUSES.map(s => (
          <Button
            key={s}
            variant={s === status ? 'default' : 'outline'}
            size="sm"
            onClick={() => setStatus(s)}
          >
            {s}
          </Button>
        ))}
        {status === 'dead' && (data?.length ?? 0) > 0 && (
          <Button
            variant="destructive"
            size="sm"
            className="ml-auto"
            onClick={() => retryAll.mutate()}
            disabled={retryAll.isPending}
          >
            复活 100 个死信
          </Button>
        )}
      </div>

      {isLoading ? (
        <div className="text-sm text-muted-foreground">加载中…</div>
      ) : (data ?? []).length === 0 ? (
        <div className="text-sm text-muted-foreground">没有 {status} 状态的任务</div>
      ) : (
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left border-b">
              <th className="p-2">URL</th>
              <th className="p-2">尝试</th>
              <th className="p-2">最后错误</th>
              <th className="p-2">时间</th>
              <th className="p-2"></th>
            </tr>
          </thead>
          <tbody>
            {(data ?? []).map((j: FetchJob) => (
              <tr key={j.id} className="border-b align-top">
                <td className="p-2 truncate max-w-xs" title={j.url}>{j.url}</td>
                <td className="p-2"><Badge variant="secondary">{j.attempts}/{j.max_attempts}</Badge></td>
                <td className="p-2 text-red-600 truncate max-w-md" title={j.last_error ?? ''}>
                  {j.last_error ?? '—'}
                </td>
                <td className="p-2 text-muted-foreground whitespace-nowrap">
                  {(j.last_error_at ?? j.created_at).slice(0, 19).replace('T', ' ')}
                </td>
                <td className="p-2">
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => retryOne.mutate(j.id)}
                    disabled={retryOne.isPending}
                  >
                    重试
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
```

- [ ] **Step 4: 接到 SettingsPage 的 admin 区**

参考现有 `web/src/components/settings/*Panel.tsx` 与 `web/src/pages/SettingsPage.tsx` 的 tab 组织方式。仅 `user?.is_admin` 用户可见。

- [ ] **Step 5: 浏览器手动验证**

```bash
./dev.sh up
```

- 登录 admin → Settings → 抓取队列
- 保存 invalid URL（`https://thisdoesnotexist.test.invalid/`）→ 等约 15 分钟 → 出现在 dead tab
- 点"重试" → 状态变 pending → 几秒后又进 dead
- "复活 100 个死信" → 显示复活数量和剩余数量

- [ ] **Step 6: Commit**

```bash
git add web/src/
git commit -m "feat(web): admin panel for fetch queue inspection and retry"
```

---

## 验收清单

完成全部 Task 0-12 后逐项手动验证：

- [ ] `docker compose down && docker compose up -d`：原 pending 的 job 在重启后继续被处理（看 fetch_jobs 表 status 流转）
- [ ] worker 在 process 中途用 `docker compose kill -s KILL lettura` 强杀，重启后该 job 5 分钟内被新 worker 接管（看 leased_by 变化）
- [ ] 多副本：临时改 `docker-compose.yml` 注释 `ports`、`docker compose up -d --scale lettura=2`，入队 100 个 job，两副本各消费约 50 个（`SELECT leased_by, COUNT(*) FROM fetch_jobs GROUP BY 1`）
- [ ] Admin 页面看到死信 → 点击重试 → 状态变 pending → 被消费
- [ ] Permanent 错误（SSRF 内网 IP）只跑 1 次就 complete + entry mark_failed，不进 backoff
- [ ] Transient 错误（mock 503）按 60s/2m/4m/8m 重试，第 5 次进 dead
- [ ] `git revert <hash range> && docker compose build && docker compose up -d`：10 分钟内回到旧版本，fetch_jobs 表保留，下次再上线 job 继续处理
- [ ] 集成测试一字未改即通过的范围：`integration_auth.rs` / `bulk_api.rs` / `cli_contract.rs` / `tag_*` / `remove_tag_by_label.rs` / `save_idempotency.rs` / `pat_*`。**例外**：`integration_entries.rs` 中如有依赖 entry 在 create 后立刻被 fetch 完成的断言，在 Task 7 commit 时显式 review 并视情况微调（worker 改为 DB 驱动后，时序仍由 worker 循环决定，应无变化）

---

## 不做的事

- 不做按 user 的公平调度（FIFO + priority 足够）
- 不引入 pg_cron / pgmq 扩展
- 不为成功的 job 保留审计行（用 counter metrics 替代）
- 不保留 mpsc 兼容路径（回退靠 git revert，本身的 bug 不值得留）
- 不改 entry 的失败状态展示（前端继续读 `entry.http_status`）
- 不支持 PAT 调 admin endpoints（沿用现有 PAT 设计：is_admin 固定 false）
