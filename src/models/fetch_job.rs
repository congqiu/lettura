use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::models::error::ModelError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize)]
#[sqlx(type_name = "fetch_job_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
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

/// Insert or update a fetch job for the given entry.
///
/// - If no active row exists, INSERT a new pending row.
/// - If a 'pending' or 'failed' row exists, UPDATE with the max priority and
///   the earliest run_after, so a higher-priority re-enqueue cannot be
///   downgraded by a stale lower-priority one.
/// - If a 'running' row exists, set refetch_requested_at so the worker
///   reschedules instead of deleting on complete. Status is not touched.
/// - If only 'dead' rows exist for the entry, INSERT a new pending row
///   (dead rows are excluded from the partial unique index, so the conflict
///   target won't match them).
///
/// Emits pg_notify('fetch_jobs_new') so any LISTEN-ing worker wakes up
/// immediately. Notification failure is non-fatal — the worker polling
/// fallback (5s) picks the job up regardless.
pub async fn enqueue(
    pool: &PgPool,
    entry_id: Uuid,
    user_id: Uuid,
    url: &str,
    priority: i16,
) -> Result<Uuid, ModelError> {
    let row: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO fetch_jobs (entry_id, user_id, url, priority)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (entry_id) WHERE status IN ('pending', 'running', 'failed')
        DO UPDATE SET
            priority             = GREATEST(fetch_jobs.priority, EXCLUDED.priority),
            run_after            = LEAST(fetch_jobs.run_after, NOW()),
            refetch_requested_at = CASE
                WHEN fetch_jobs.status = 'running' THEN NOW()
                ELSE fetch_jobs.refetch_requested_at
            END,
            updated_at           = NOW()
        RETURNING id
        "#,
    )
    .bind(entry_id)
    .bind(user_id)
    .bind(url)
    .bind(priority)
    .fetch_one(pool)
    .await?;

    // Best-effort notification. Workers have a 5s polling fallback, so a
    // dropped notification only delays pickup by at most one poll interval.
    let _ = sqlx::query("SELECT pg_notify('fetch_jobs_new', '')")
        .execute(pool)
        .await;

    Ok(row.0)
}

/// Fetch a single job row by id. Returns None if the row does not exist.
pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<FetchJobRow>, ModelError> {
    let row = sqlx::query_as::<_, FetchJobRow>("SELECT * FROM fetch_jobs WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// A leased job returned by [`dequeue_one`]. Carries only the fields a worker
/// needs to run the job; lease bookkeeping (leased_until, leased_by) is
/// already persisted on the row.
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

/// Atomically lease the next runnable job to `worker_id`.
///
/// Selects the highest-priority job whose `run_after <= NOW()` and whose lease
/// (if any) has expired, using `FOR UPDATE SKIP LOCKED` so concurrent workers
/// never collide on the same row. The chosen row is flipped to `status =
/// 'running'`, `leased_until = NOW() + 5 minutes`, and `attempts` is bumped.
///
/// Returns `None` if no job is currently runnable.
pub async fn dequeue_one(
    pool: &PgPool,
    worker_id: &str,
) -> Result<Option<LeasedJob>, ModelError> {
    let row = sqlx::query_as::<_, LeasedJob>(
        r#"
        WITH next_job AS (
            -- 'running' is included so jobs whose lease has expired can be
            -- taken over by another worker. Active leases are excluded by
            -- the (leased_until < NOW()) clause below.
            SELECT id FROM fetch_jobs
            WHERE status IN ('pending', 'failed', 'running')
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
            -- LEAST(..., max_attempts) prevents SMALLINT overflow under
            -- pathological retry storms (e.g. lease takeovers compounding
            -- with admin-driven retries). The value never exceeds the
            -- 32767 i16 ceiling because max_attempts itself is SMALLINT.
            attempts      = LEAST(attempts + 1, max_attempts),
            updated_at    = NOW()
        FROM next_job
        WHERE j.id = next_job.id
        RETURNING j.id, j.entry_id, j.user_id, j.url, j.attempts, j.max_attempts
        "#,
    )
    .bind(worker_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Complete a job atomically:
/// - If `refetch_requested_at IS NULL` → DELETE the row.
/// - Else → reset to `pending` (preserve priority for next dispatch) and
///   clear the refetch signal.
///
/// Requires `leased_by = $worker_id` so that a worker whose lease has been
/// taken over by another worker cannot write to the row anymore.
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
            status               = 'pending',
            attempts             = 0,
            run_after            = NOW(),
            leased_until         = NULL,
            leased_by            = NULL,
            refetch_requested_at = NULL,
            last_error           = NULL,
            last_error_at        = NULL,
            updated_at           = NOW()
        WHERE id IN (SELECT id FROM locked WHERE refetch_requested_at IS NOT NULL)
          AND id NOT IN (SELECT id FROM deleted)
        "#,
    )
    .bind(id)
    .bind(worker_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Mark a job as failed.
///
/// - If `attempts >= max_attempts` → promote to `dead`.
/// - Else → set status `failed` and schedule the next retry with exponential
///   backoff (60s * 2^(attempts - 1)).
///
/// Both branches require `leased_by = $worker_id` so a stale worker cannot
/// trample the row after losing its lease. `attempts` is left untouched —
/// the +1 happens in `dequeue_one`.
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
            SET status        = 'dead',
                last_error    = $3,
                last_error_at = NOW(),
                leased_until  = NULL,
                leased_by     = NULL,
                updated_at    = NOW()
            WHERE id = $1 AND leased_by = $2
            "#,
        )
        .bind(id)
        .bind(worker_id)
        .bind(truncated)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            r#"
            UPDATE fetch_jobs
            SET status        = 'failed',
                last_error    = $3,
                last_error_at = NOW(),
                run_after     = NOW() + (INTERVAL '60 seconds'
                                * POWER(2::numeric, GREATEST(attempts - 1, 0))),
                leased_until  = NULL,
                leased_by     = NULL,
                updated_at    = NOW()
            WHERE id = $1 AND leased_by = $2
            "#,
        )
        .bind(id)
        .bind(worker_id)
        .bind(truncated)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Return a job to the queue without consuming an attempt (graceful shutdown).
///
/// Decrements `attempts` to undo the +1 from `dequeue_one`, clears the lease,
/// and flips status back to `pending`. Requires `leased_by = $worker_id`.
pub async fn release(pool: &PgPool, id: Uuid, worker_id: &str) -> Result<(), ModelError> {
    sqlx::query(
        r#"
        UPDATE fetch_jobs
        SET status       = 'pending',
            leased_until = NULL,
            leased_by    = NULL,
            attempts     = GREATEST(attempts - 1, 0),
            updated_at   = NOW()
        WHERE id = $1 AND leased_by = $2 AND status = 'running'
        "#,
    )
    .bind(id)
    .bind(worker_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Return `(status, count)` pairs for every status currently present in the
/// `fetch_jobs` table. Statuses with zero rows are omitted — callers that
/// publish gauges must reset unseen labels to 0 themselves.
pub async fn count_by_status(pool: &PgPool) -> Result<Vec<(FetchJobStatus, i64)>, ModelError> {
    let rows: Vec<(FetchJobStatus, i64)> =
        sqlx::query_as("SELECT status, COUNT(*) FROM fetch_jobs GROUP BY status")
            .fetch_all(pool)
            .await?;
    Ok(rows)
}

/// Extend the lease for a long-running job by 5 minutes from now. No-op if
/// the lease is no longer held by `worker_id`.
pub async fn renew_lease(pool: &PgPool, id: Uuid, worker_id: &str) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE fetch_jobs SET leased_until = NOW() + INTERVAL '5 minutes' \
         WHERE id = $1 AND leased_by = $2 AND status = 'running'",
    )
    .bind(id)
    .bind(worker_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// List fetch jobs ordered by most recent first. Optionally filter by status.
/// `limit` is clamped to `[1, 500]`.
pub async fn list_by_status(
    pool: &PgPool,
    status: Option<FetchJobStatus>,
    limit: i64,
) -> Result<Vec<FetchJobRow>, ModelError> {
    let limit = limit.clamp(1, 500);
    let rows = if let Some(s) = status {
        sqlx::query_as::<_, FetchJobRow>(
            "SELECT * FROM fetch_jobs WHERE status = $1 \
             ORDER BY created_at DESC LIMIT $2",
        )
        .bind(s)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, FetchJobRow>(
            "SELECT * FROM fetch_jobs ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

/// Hard-delete a fetch job row regardless of status.
pub async fn delete_by_id(pool: &PgPool, id: Uuid) -> Result<(), ModelError> {
    sqlx::query("DELETE FROM fetch_jobs WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Reset a single job to pending, clearing all error/lease state. Emits a
/// pg_notify so any LISTEN-ing worker picks it up immediately.
pub async fn retry(pool: &PgPool, id: Uuid) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE fetch_jobs \
         SET status='pending', attempts=0, run_after=NOW(), \
             leased_until=NULL, leased_by=NULL, \
             last_error=NULL, last_error_at=NULL, \
             refetch_requested_at=NULL, updated_at=NOW() \
         WHERE id=$1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    // Best-effort notification; workers also poll every 5s.
    let _ = sqlx::query("SELECT pg_notify('fetch_jobs_new', '')")
        .execute(pool)
        .await;
    Ok(())
}

/// Revive at most `limit` dead jobs by resetting them to pending.
/// Returns `(retried_count, remaining_dead)` so callers can decide whether to
/// invoke again. `limit` is clamped to `[1, 500]`.
pub async fn retry_all_dead(pool: &PgPool, limit: i64) -> Result<(u64, i64), ModelError> {
    let limit = limit.clamp(1, 500);
    let result = sqlx::query(
        "UPDATE fetch_jobs \
         SET status='pending', attempts=0, run_after=NOW(), priority=5, \
             leased_until=NULL, leased_by=NULL, updated_at=NOW() \
         WHERE id IN ( \
             SELECT id FROM fetch_jobs WHERE status='dead' \
             ORDER BY last_error_at DESC LIMIT $1 \
         )",
    )
    .bind(limit)
    .execute(pool)
    .await?;
    let retried = result.rows_affected();

    let remaining: i64 =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM fetch_jobs WHERE status='dead'")
            .fetch_one(pool)
            .await?;

    let _ = sqlx::query("SELECT pg_notify('fetch_jobs_new', '')")
        .execute(pool)
        .await;
    Ok((retried, remaining))
}
