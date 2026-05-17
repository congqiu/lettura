//! DB-backed fetch worker.
//!
//! Each worker:
//! - listens for `fetch_jobs_new` NOTIFY (5s polling fallback)
//! - dequeues with FOR UPDATE SKIP LOCKED via fetch_job::dequeue_one
//! - renews the lease every 60s for long-running jobs
//! - routes pipeline::process Result to fetch_job state:
//!     Ok(())                -> complete (DELETE or honor refetch)
//!     Err(Permanent(msg))   -> complete + mark_failed
//!     Err(Transient(msg))   -> fail (backoff or dead)

use crate::fetch::pipeline::{self, FetchContext, FetchError};
use crate::models::fetch_job;
use crate::search::SearchBackend;
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
    pub search_index: Arc<dyn SearchBackend>,
    pub client: reqwest::Client,
    pub max_retries: u32,
    /// Shared with `AppState.caches` so cache invalidations performed by
    /// background tagging (via `apply_tagging_rules`) are visible to
    /// concurrent HTTP handlers.
    pub caches: Arc<crate::cache::Caches>,
    #[cfg(feature = "rendering")]
    pub render_service: Option<Arc<crate::fetch::render::RenderService>>,
    /// Test-only escape hatch propagated to [`FetchContext::skip_ssrf`].
    /// Gated behind `cfg(any(test, feature = "test-utils"))` so production
    /// release builds physically cannot construct a worker that bypasses SSRF
    /// validation. See safety note on `FetchContext::skip_ssrf`.
    #[cfg(any(test, feature = "test-utils"))]
    pub skip_ssrf: bool,
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
            } else {
                Some(l)
            }
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
        caches: cfg.caches.clone(),
        #[cfg(feature = "rendering")]
        render_service: cfg.render_service.clone(),
        #[cfg(any(test, feature = "test-utils"))]
        skip_ssrf: cfg.skip_ssrf,
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
            if cancel.is_cancelled() {
                break;
            }
            let job = match fetch_job::dequeue_one(&cfg.pool, &worker_id).await {
                Ok(Some(j)) => j,
                Ok(None) => break,
                Err(e) => {
                    tracing::error!("dequeue failed: {e}");
                    break;
                }
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
            // CRITICAL ORDER: abort renew BEFORE release so the renewer
            // cannot race with the release UPDATE.
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
            metrics::counter!("fetch_jobs_completed_total").increment(1);
            let _ = fetch_job::complete(pool, leased.id, worker_id).await;
        }
        Err(FetchError::Permanent(msg)) => {
            metrics::counter!("fetch_jobs_failed_total", "reason" => "permanent").increment(1);
            tracing::info!(job_id = %leased.id, "permanent failure: {msg}");
            pipeline::mark_failed(pool, leased.entry_id, 0).await;
            let _ = fetch_job::complete(pool, leased.id, worker_id).await;
        }
        Err(FetchError::Transient(msg)) => {
            metrics::counter!("fetch_jobs_failed_total", "reason" => "transient").increment(1);
            tracing::info!(
                job_id = %leased.id, attempts = leased.attempts,
                "transient failure: {msg}"
            );
            let _ = fetch_job::fail(
                pool,
                leased.id,
                worker_id,
                &msg,
                leased.attempts,
                leased.max_attempts,
            )
            .await;
            if leased.attempts >= leased.max_attempts {
                metrics::counter!("fetch_jobs_dead_total").increment(1);
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
