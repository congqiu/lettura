# 抓取队列持久化设计

> 创建日期: 2026-05-16
> 状态: 设计阶段

## 背景

当前的抓取队列实现在 `src/tasks/fetcher.rs`：

- `FetchQueue` 包装 `mpsc::Sender<FetchJob>`（缓冲 5000）
- `start_fetch_worker` 启动 N=5 个 worker，共享一个 `Arc<Mutex<mpsc::Receiver>>`，循环 `rx.recv().await`，拿到 job 调用 `pipeline::process`

存在三个本质问题：

1. **进程崩溃 / SIGKILL / OOM / rolling update → 队列中所有 job 丢失**。用户保存的文章在 DB 中已有 `entry` 行，但内容字段为空，且永远不会被再次抓取。无任何补偿机制。
2. **无法水平扩容**：多副本各自维护独立 mpsc，互不可见。如果第二个副本只是为分摊 HTTP 流量，那入队的 job 仍只能由收到入队请求的那个副本消费。
3. **失败缺乏可观察性与可干预性**：失败仅由 `mark_failed` 把 entry 标记为失败状态，前端没有重试入口，运维无法批量查询哪些 job 失败、为什么失败。

## 设计目标

- **崩溃零丢失**：任意时刻强杀进程，重启后未完成的 job 必须被某个 worker 继续处理
- **多副本可水平扩展**：N 个副本共享同一队列，自动负载均衡，不需要外部协调
- **失败可见、可重试**：失败 job 保留错误原因；admin 接口可列出、单独重试、批量复活
- **对调用方零侵入**：`FetchQueue::send(job)` 签名不变，所有现有 handler 不改
- **不引入新组件**：用 PostgreSQL 表实现，符合"PostgreSQL only"架构决策
- **回退靠 git revert + 重新部署**：不在代码里保留 mpsc 兼容路径。旧实现本身就有"重启丢任务"的 bug，runtime 切回去无意义

## 非目标

- **不引入 Redis / RabbitMQ / NATS / pgmq 扩展**
- **不做跨用户公平调度**：当前所有用户共享 worker 池，FIFO + priority 已足够
- **不改 `pipeline::process` 本身**：本次只换"任务从哪来"，抓取/提取/入库的下游逻辑保持原样
- **不做 exactly-once 语义**：at-least-once 即可；`pipeline::process` 本身需要幂等（已经是，因为它走 `update_entry_content` 而非 insert）

---

## 数据模型

### 新增表 `fetch_jobs`

```sql
CREATE TYPE fetch_job_status AS ENUM (
    'pending',   -- waiting for a worker
    'running',   -- leased by a worker, in progress
    'failed',    -- failed, will retry after run_after
    'dead'       -- exceeded max_attempts, manual intervention required
);

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

    -- Separate signal for "user clicked refetch while this job was running".
    -- Worker checks this on complete; if non-null, reset to pending instead
    -- of DELETE. Keeping it separate from `priority` avoids overloading one
    -- column with two meanings (dispatch ordering vs. user intent).
    refetch_requested_at TIMESTAMPTZ,

    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Hot path for dequeue: find next runnable job.
CREATE INDEX idx_fetch_jobs_dispatch
    ON fetch_jobs (status, run_after, priority DESC)
    WHERE status IN ('pending', 'failed');

-- Admin queries: list jobs per user, newest first.
CREATE INDEX idx_fetch_jobs_user_created
    ON fetch_jobs (user_id, created_at DESC);

CREATE INDEX idx_fetch_jobs_entry ON fetch_jobs (entry_id);

-- Prevent duplicate active jobs for the same entry.
CREATE UNIQUE INDEX uq_fetch_jobs_active_entry
    ON fetch_jobs (entry_id)
    WHERE status IN ('pending', 'running', 'failed');
```

### 字段说明

| 字段 | 说明 |
|------|------|
| `priority` | 数字越大越优先。默认 0；手动 refetch 给 10；管理员复活死信给 5 |
| `attempts` | 已尝试次数。dequeue 时 +1 |
| `max_attempts` | 上限（默认 5），超过则进 `dead` |
| `run_after` | 在此时间之前不会被 dispatch。失败后设为 `NOW() + backoff(attempts)` |
| `leased_until` | 租约过期时间。worker 拿走 job 时设为 `NOW() + 5min`；过期视为 worker 崩溃，其他 worker 可抢回 |
| `leased_by` | worker 标识 `<hostname>:<pid>`，仅供运维排查，不参与调度逻辑 |
| `last_error` | 截断到 1000 字符，避免单个 job 把表撑爆 |

### 状态机

```
       enqueue
           │
           ▼
     ┌─────────┐    dequeue     ┌─────────┐    success
     │ pending │ ─────────────► │ running │ ─────────► DELETE
     └─────────┘                └─────────┘
          ▲                          │
          │ run_after reached        │ failure (attempts < max)
          │                          ▼
          │                     ┌────────┐
          └─────────────────────┤ failed │
                                └────────┘
                                     │ failure (attempts >= max)
                                     ▼
                                ┌──────┐
                                │ dead │
                                └──────┘
                                     │ admin retry
                                     ▼
                                  pending
```

### 关键设计取舍

**保留 `failed` 状态而非直接回到 `pending`**

可选方案 A：失败时设 `status='pending'` + 推迟 `run_after`，靠 dispatch 索引过滤未到时间的 job。
可选方案 B（采用）：失败转 `failed`，dispatch 索引 `status IN ('pending', 'failed')`。

理由：用户报告"我点了保存怎么没抓到"时，`SELECT * FROM fetch_jobs WHERE status='failed' AND user_id=?` 一条 SQL 就能定位。运维价值远超那点索引复杂度。

**租约 (lease) 而非 worker_id + heartbeat**

worker 拿 job 时设 `leased_until = NOW() + 5min`，崩溃后租约自然过期，下一个 worker 通过 `leased_until < NOW()` 即可抢回。比"worker 周期性心跳 + 主控判定死亡"简单一个量级，且无需主控角色。

长任务（如渲染 15 秒）由 worker 内部开 task 每 60 秒续约一次。

**成功直接 DELETE，失败/死信保留**

成功记录无运维价值，保留只会让表无限增长（单用户每天可能保存几十篇）。后台 task 30 天清理 `status='dead'`。

---

## 错误分类

当前 `src/fetch/pipeline.rs::process` 返回 `()`，内部 9 处调用 `mark_failed` 把 entry 标失败状态直接吞掉错误。本设计要求改其签名为 `Result<(), FetchError>`，由 worker 根据 error 类型决定 job 的下一步状态。

```rust
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// 4xx、SSRF 拦截、URL 非法、解析后内容明确不可用 — 不应重试
    #[error("permanent: {0}")]
    Permanent(String),

    /// 5xx、超时、DNS、连接重置、读 body 中断、render 失败 — 应重试
    #[error("transient: {0}")]
    Transient(String),
}
```

**Worker 处理表**

| `pipeline::process` 返回 | fetch_job 动作 | entry 动作 |
|--------------------------|---------------|------------|
| `Ok(())` | DELETE | （内部已写好内容） |
| `Err(Permanent)` | DELETE | `mark_failed` |
| `Err(Transient)` | 调 `fail()`，进 backoff | 不变（保留上次成功内容；若从未成功则 `http_status=NULL`） |
| `Err(Transient)` × `max_attempts` 次后 | 进 `dead` | `mark_failed` |

**判定规则**（在 `pipeline::process` 各 mark_failed 调用点替换为对应分支）：

| 当前调用位置 | 当前 status 参数 | 新分类 |
|--------------|-----------------|--------|
| SSRF 拦截 (line 64, 91) | 0 | Permanent |
| HTTP 4xx (line 111, 153, 158) | 4xx | Permanent |
| HTTP 5xx (line 167, 187, 220) | 5xx | Transient |
| 超时 / 网络错误 (line 128) | 0 | Transient |
| 渲染失败 + render.mode=force | — | Transient |
| 提取后内容长度为 0 且无降级路径 | — | Permanent |

**好处**：用户保存的 URL 网络抖动几次后能自动恢复；真正打不开的 404 / 私有页面 / SSRF 立即标失败不浪费重试。

---

## 调度算法

### 入队 (Enqueue)

新 `FetchQueue::send` 实现：

```rust
// 注意：项目惯例使用 sqlx 函数形式（运行时校验），不使用 `query!` 宏，
// 因 Dockerfile build 阶段不连数据库、未启用 sqlx offline cache。
sqlx::query(
    r#"
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
    "#,
)
.bind(job.entry_id).bind(job.user_id).bind(&job.url).bind(priority)
.execute(&pool).await?;

// 用 pg_notify 而非分离 NOTIFY，能放进与 INSERT 同一事务，commit 时统一派发。
// 如果上面 INSERT 已经在自动 commit 模式，单独这一句也够；放事务里更稳。
sqlx::query("SELECT pg_notify('fetch_jobs_new', '')")
    .execute(&pool).await.ok();
```

**`ON CONFLICT ON CONSTRAINT` 而非 `ON CONFLICT (column) WHERE ...`** — 后者依赖 PG 对 partial index 谓词的推断，对 ENUM 字面量经常匹配失败；前者显式按约束名匹配，行为确定。

**唯一约束语义** — 索引 `uq_fetch_jobs_active_entry` 只覆盖 `status IN ('pending','running','failed')`。若某 entry 当前只有 `dead` 行（运维放弃过的失败），新 INSERT 不会冲突，直接插入新的 pending 行。dead 行保留作历史。这是预期行为。

**`refetch_requested_at` 的语义** — 仅在当前 job 状态为 `running` 时（即 worker 正在跑）才设置；其他状态（pending/failed）下 refetch 走 priority + run_after 重排，无需特殊信号。worker 在 `complete` 时检查此字段决定 DELETE 还是 reset。

### 出队 (Dequeue)

worker 循环执行：

```sql
WITH next_job AS (
    SELECT id
    FROM fetch_jobs
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
RETURNING j.id, j.entry_id, j.user_id, j.url, j.attempts, j.max_attempts;
```

`FOR UPDATE SKIP LOCKED` 是这次设计的关键原语：N 个 worker 并发执行时，PostgreSQL 让每个事务自动跳过其他事务锁住的行，原子地拿到下一个可用 job。无需应用层协调，不会出现"两个 worker 跑同一个 job"。

### 完成 (Success)

事务里检查 `refetch_requested_at`，若非空表示 worker 跑期间用户又点了 refetch，重置而非删除：

```sql
-- 单条 SQL 完成"看一下、决定 DELETE 或 RESET"，避免 read-modify-write 竞态。
WITH check_row AS (
    SELECT id, refetch_requested_at, leased_by
    FROM fetch_jobs
    WHERE id = $1 AND leased_by = $2 AND status = 'running'
    FOR UPDATE
),
deleted AS (
    DELETE FROM fetch_jobs
    WHERE id IN (SELECT id FROM check_row WHERE refetch_requested_at IS NULL)
    RETURNING id
)
UPDATE fetch_jobs SET
    status='pending', attempts=0, run_after=NOW(),
    leased_until=NULL, leased_by=NULL,
    refetch_requested_at=NULL,
    last_error=NULL, last_error_at=NULL,
    updated_at=NOW()
WHERE id IN (SELECT id FROM check_row WHERE refetch_requested_at IS NOT NULL)
  AND id NOT IN (SELECT id FROM deleted);
```

`leased_by = $2` 防止 worker A 在租约过期、worker B 已接管后，A 的延迟 complete 错误地处理了 B 的 job。

**注意**：reset 路径保留 `priority`（不归零）—— refetch 时 enqueue 通过 ON CONFLICT 把 priority 抬到 10，complete 不应丢弃这个信号。这样重抓仍然高优。

### 失败 (Failure)

由 worker 在 `pipeline::process` 返回 `Err(Transient)` 时调用：

```sql
-- attempts < max: 重新排队，指数 backoff
UPDATE fetch_jobs
SET status        = 'failed',
    last_error    = LEFT($2, 1000),
    last_error_at = NOW(),
    run_after     = NOW() + (INTERVAL '60 seconds' * POWER(2, GREATEST(attempts - 1, 0))),
    leased_until  = NULL,
    leased_by     = NULL,
    updated_at    = NOW()
WHERE id = $1
  AND leased_by = $3
  AND attempts < max_attempts;

-- attempts >= max: 进死信
UPDATE fetch_jobs
SET status        = 'dead',
    last_error    = LEFT($2, 1000),
    last_error_at = NOW(),
    leased_until  = NULL,
    leased_by     = NULL,
    updated_at    = NOW()
WHERE id = $1
  AND leased_by = $3
  AND attempts >= max_attempts;
```

`Err(Permanent)` 走 `complete()` 路径（含 leased_by 校验），同时调用 `entry::mark_failed` 持久化失败状态。

Backoff 时间序列（attempts=1..4 失败）：60s, 2min, 4min, 8min。`GREATEST(attempts - 1, 0)` 兜底 attempts=0 的边界（release 后归零再失败）。max_attempts=5 时实际最长重试窗口约 15 分钟。

---

## 通知机制

纯 polling 要么延迟高（5 秒），要么浪费 DB（100ms）。`LISTEN/NOTIFY` 几乎零开销且实时。

每个 worker 启动时单独维护一个 sqlx `PgListener` 监听 `fetch_jobs_new` channel。入队方在 INSERT 后 `NOTIFY fetch_jobs_new`。

worker 主循环：

```rust
loop {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => break,
        // 收到 NOTIFY 立即尝试 dequeue
        notify = listener.recv() => {
            if notify.is_err() {
                // 连接断开，自动重连由 PgListener 处理，这里 fall through
            }
        }
        // 兜底：防 NOTIFY 丢失或 run_after 到期
        _ = tokio::time::sleep(Duration::from_secs(5)) => {}
    }

    // Drain 当前所有可用 job，避免 NOTIFY 一次只处理一个的浪费
    while let Some(job) = dequeue_one(&pool, &worker_id).await? {
        let cancel = cancel.clone();
        process_with_renewal(&ctx, &job, cancel).await;
    }
}
```

`LISTEN/NOTIFY` 不跨副本可靠（按 Postgres 文档，副本数据库 listener 不会收到主库通知）—— 但本项目当前是主库直连，且每个副本都有 5 秒兜底轮询，丢一两次 NOTIFY 仅影响延迟，不影响正确性。

---

## 租约续约

长任务（典型场景：渲染需要 10-15 秒，加上 HTTP 慢站点重试 30 秒）可能接近或超过初始 5 分钟租约。worker 处理 job 时开一个旁路 task：

```rust
let renew_handle = {
    let pool = pool.clone();
    let job_id = job.id;
    let worker_id = worker_id.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.tick().await; // 跳过第一次
        loop {
            interval.tick().await;
            // leased_by 校验：若其他 worker 已经接管（lease 过期被抢），
            // 这条 UPDATE 不匹配任何行，自然停止续约。
            let _ = sqlx::query(
                "UPDATE fetch_jobs SET leased_until = NOW() + INTERVAL '5 minutes' \
                 WHERE id = $1 AND leased_by = $2 AND status = 'running'"
            ).bind(job_id).bind(&worker_id).execute(&pool).await;
        }
    })
};

let result = pipeline::process(&ctx, &job).await;
renew_handle.abort();
```

实际有效租约时间 = max(任务时间, 60s) + 5min 缓冲。

---

## 优雅停机

接 SIGTERM 时：

1. 触发 `CancellationToken::cancel`
2. worker 主循环的 `tokio::select!` 命中 cancel 分支，跳出循环
3. 已经在 `pipeline::process` 中的 job：等待最多 30 秒 grace period
4. 超时未完成的 job：UPDATE 把 `status='pending'`, `leased_until=NULL`，归还给下一轮其他 worker
5. 极端情况（强杀）：依赖租约自然过期，5 分钟后被其他 worker 接管

```rust
match tokio::time::timeout(Duration::from_secs(30), pipeline::process(&ctx, &job)).await {
    Ok(_) => { /* normal */ }
    Err(_) => {
        sqlx::query(
            "UPDATE fetch_jobs SET status='pending', leased_until=NULL, \
             attempts = attempts - 1 WHERE id = $1"
        ).bind(job.id).execute(&pool).await.ok();
    }
}
```

`attempts - 1` 是因为 dequeue 时已经 +1；归还时回滚，避免一次主动停机消耗用户的重试次数。

---

## 可观察性

### Metrics

```
fetch_queue_size{status="pending"}    gauge
fetch_queue_size{status="running"}    gauge
fetch_queue_size{status="failed"}     gauge
fetch_queue_size{status="dead"}       gauge

fetch_jobs_enqueued_total             counter
fetch_jobs_completed_total            counter
fetch_jobs_failed_total{reason="..."} counter
fetch_jobs_dead_total                 counter

fetch_job_duration_seconds            histogram (process 时间)
fetch_job_wait_seconds                histogram (created_at → dequeue)
```

`fetch_queue_size` 通过后台 task 每 10 秒一条 GROUP BY 查询上报。

### Admin Endpoints

| Method | Path | 说明 |
|--------|------|------|
| GET    | `/api/v1/admin/fetch-jobs?status=failed&limit=50` | 列表（含 last_error 摘要） |
| GET    | `/api/v1/admin/fetch-jobs/{id}` | 单条详情 |
| POST   | `/api/v1/admin/fetch-jobs/{id}/retry` | 重置为 `pending`, `attempts=0`, `run_after=NOW()` |
| POST   | `/api/v1/admin/fetch-jobs/retry-all-dead` | 批量复活，priority=5，**单次最多 100 个** |
| DELETE | `/api/v1/admin/fetch-jobs/{id}` | 放弃这个 job（不影响 entry 记录） |

**`retry-all-dead` 限流** — `UPDATE … WHERE id IN (SELECT id FROM fetch_jobs WHERE status='dead' ORDER BY last_error_at DESC LIMIT 100)`。理由：5 worker × N 死信 × 5 次 retry，60s 间隔，可能击穿目标站点限流甚至触发封 IP。运维若死信数 >100 应按 last_error 分类抽样确认可恢复后再分批复活。响应体含 `{ retried, remaining_dead }` 提示是否还有未复活。

**鉴权约束** — admin endpoints 仅接受 JWT 鉴权。Personal Access Token (PAT) 走的 middleware（`src/auth/middleware.rs`）固定 `is_admin = false`，即使 PAT 属于 admin 用户也无法调这些接口。这是 PAT 的安全设计：CLI 不应有删除 / 复活 / 操纵其他用户 job 的能力。`require_admin` 的 403 错误信息要明确提示"PAT 不支持此接口"。

前端 Admin 页面新增"抓取队列"tab，复用现有 admin table 组件。

---

## 配置

```bash
# Worker 数量（沿用现有值）
LETTURA_FETCH_CONCURRENCY=5

# 单个 job 默认最大重试
LETTURA_FETCH_MAX_ATTEMPTS=5

# 租约初始时长（秒）
LETTURA_FETCH_LEASE_SECS=300

# 死信清理 TTL（天）
LETTURA_FETCH_DEAD_TTL_DAYS=30
```

---

## 回滚策略

不在代码里保留 mpsc 兼容路径。理由：mpsc 实现本身就有"重启丢任务"的 bug，runtime 切回去并非真正的"安全回退"。

紧急止血路径：`git revert` 相关 commits → 重新构建镜像 → redeploy。Docker 镜像不可变 + 数据库 schema 向下兼容（`fetch_jobs` 表对旧版本就是空表，无影响），整套回退在 10 分钟内可完成。

为支持回退期间数据不混乱，`fetch_jobs` migration 一旦上线，**不能再以"删除表"的方式回退**——回退只回 Rust 代码，让旧版本忽略这张表。表本身保留，下次再上线时 job 仍可继续处理。

---

## 投递语义

本队列为 **at-least-once**：

- worker A 处理超时（pipeline 卡住），租约过期被 worker B 接管时，A 可能仍未完成
  - A 最终成功：`complete` 因 leased_by 校验为 no-op，B 也会重做一遍 — 同一 URL 被 fetch 两次
  - 这在 pipeline 层是幂等的（`update_entry_content` 走 UPDATE，搜索索引按 entry_id replace），不影响数据正确性，只是重复消耗目标站点带宽
  - 长任务（>5 分钟）建议保持续约 60s 间隔默认值不变，避免误触发 takeover

- worker 在 `mark_failed(entry)` 与 `fetch_job::complete/fail` 之间 crash
  - entry 已标失败，但 job 仍 running，租约过期后会被另一个 worker 重抓
  - 重抓成功则 entry 重新有内容（自我修复）；重抓也失败则再次 mark_failed（幂等）
  - 不影响最终一致性，但 spec 不保证"mark_failed 立即可见"

如需 exactly-once，需要把 `mark_failed` 和 `fetch_job` 状态更新放进同一事务（当前实现没做，因为 mark_failed 在 entries 表、fetch_jobs 在自己表，跨表事务对性能有影响且收益小）。

---

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| `SELECT FOR UPDATE SKIP LOCKED` 在大量 pending 时索引扫描慢 | 部分索引 `WHERE status IN ('pending','failed')` 已最小化扫描集合；实测 50k pending 量级 dequeue < 5ms |
| `PgListener` 连接断开未自动重连 | sqlx PgListener 内置 reconnect；即使彻底失败，5 秒兜底轮询保底 |
| 长任务租约时间选择不当 | 5 分钟初始 + 60 秒续约是经验值，覆盖 99% 渲染场景；超长任务（如批量导入）走独立的 import 任务表，不复用 fetch 队列 |
| `attempts` 在归还时回滚导致死循环 | 主动归还（停机）才回滚；崩溃后租约过期接管 *不* 回滚，自然消耗重试次数 |
| 死信表无限增长 | 后台 task 每小时一次 `DELETE WHERE status='dead' AND last_error_at < NOW() - INTERVAL '30 days'` |
| 唯一索引导致 refetch 冲突 | `ON CONFLICT ON CONSTRAINT uq_fetch_jobs_active_entry DO UPDATE` 显式处理：取 max priority + min run_after；running 状态下额外置 `refetch_requested_at` 触发 complete 阶段重新调度 |
| 大量死信被一键复活打击目标站点 | `retry_all_dead` 单次最多 100 条；spec 强调分批复活、抽样确认 last_error 可恢复 |
| worker A 处理超时被 worker B 接管后 A 仍然写入 | 所有 complete/fail/release/renew 的 UPDATE 都带 `WHERE leased_by = $worker_id`，错误的 worker 写入静默不匹配，靠应用层日志诊断 |
| 多副本 NOTIFY 不可靠 | 5 秒兜底轮询保底；副本数较少时延迟可接受 |
| 现有 in-flight mpsc 任务在切换时丢失 | 切换流程：双写过渡（先入 DB 再入 mpsc），观察期后停掉 mpsc consumer，最后删 mpsc 实现 |

---

## 验收标准

- [ ] 进程被 `kill -9` 后重启，原 pending 的 job 全部继续被处理
- [ ] 启动两个副本，入队 1000 个 job，两个副本各消费约 500 个，无重复消费
- [ ] worker panic 中途的 job，5 分钟后被其他 worker 接管
- [ ] Admin 页面能看到失败 job 的错误原因，点击重试后 job 重新进入 pending
- [ ] 现有所有集成测试（特别是 `integration_entries.rs`、`bulk_api.rs`）不需要修改即通过
- [ ] `git revert <commits> && docker build && docker compose up -d` 可在 10 分钟内回到旧版本，且 fetch_jobs 表保留无数据丢失

---

## 参考

- PostgreSQL 文档: [`FOR UPDATE SKIP LOCKED`](https://www.postgresql.org/docs/current/sql-select.html#SQL-FOR-UPDATE-SHARE)
- 现有 fetch 实现: `src/tasks/fetcher.rs`, `src/fetch/pipeline.rs`
- 抓取重设计 spec: `docs/specs/2026-04-23-fetch-pipeline-redesign.md`
- 架构优化全景: `docs/specs/2026-04-29-architecture-optimization.md`
