//! Fetch queue: PostgreSQL-backed durable job queue.
//!
//! Jobs survive process restarts and are dispatched via SELECT FOR UPDATE
//! SKIP LOCKED across all replicas. See docs/specs/2026-05-16-fetch-queue-persistence.md.

use sqlx::PgPool;
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
}

impl FetchQueue {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Standard enqueue (priority 0).
    pub async fn send(&self, job: FetchJob) -> Result<(), String> {
        fetch_job::enqueue(&self.pool, job.entry_id, job.user_id, &job.url, 0)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())?;
        metrics::counter!("fetch_jobs_enqueued_total", "priority" => "normal").increment(1);
        Ok(())
    }

    /// User-driven refetch (priority 10). Same enqueue path but jumps the
    /// queue; if a worker is currently processing this entry, signals it via
    /// refetch_requested_at so complete() reschedules.
    pub async fn send_refetch(&self, job: FetchJob) -> Result<(), String> {
        fetch_job::enqueue(&self.pool, job.entry_id, job.user_id, &job.url, 10)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())?;
        metrics::counter!("fetch_jobs_enqueued_total", "priority" => "refetch").increment(1);
        Ok(())
    }
}

// NOTE: integration tests for FetchQueue::send live in
// tests/integration_fetch_jobs.rs — they need a real PgPool wired via TestApp.
