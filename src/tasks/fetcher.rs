//! Fetch worker: mpsc queue + worker coroutines.
//!
//! Most of what this module used to do (HTTP, retry, extraction, saving) now
//! lives in `src/fetch/`. This file is only responsible for:
//! - the `FetchJob` struct (queue item),
//! - the `FetchQueue` handle (used by API handlers and metrics),
//! - `start_fetch_worker` which spawns N worker coroutines that pull jobs off
//!   the channel and invoke `fetch::pipeline::process`.

use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::config::Config;
use crate::fetch::{http, pipeline};
use crate::search::SearchIndex;
use crate::storage::ImageStorage;

#[derive(Debug, Clone)]
pub struct FetchJob {
    pub entry_id: Uuid,
    pub user_id: Uuid,
    pub url: String,
}

#[derive(Clone)]
pub struct FetchQueue {
    tx: mpsc::Sender<FetchJob>,
    pub queue_depth: Arc<AtomicUsize>,
}

impl FetchQueue {
    pub async fn send(&self, job: FetchJob) -> Result<(), String> {
        self.queue_depth.fetch_add(1, Ordering::Relaxed);
        self.tx.send(job).await.map_err(|e| {
            self.queue_depth.fetch_sub(1, Ordering::Relaxed);
            e.to_string()
        })
    }
}

pub fn start_fetch_worker(
    pool: PgPool,
    concurrency: usize,
    image_storage: Arc<dyn ImageStorage>,
    search_index: SearchIndex,
    config: &Config,
) -> FetchQueue {
    let (tx, rx) = mpsc::channel::<FetchJob>(5000);
    let rx = Arc::new(Mutex::new(rx));
    let queue_depth = Arc::new(AtomicUsize::new(0));

    let client = http::build_client(config);
    let max_retries = config.fetch_max_retries;

    // One RenderService shared across all workers. Lazy-launches Chromium on
    // first use, so boot is unaffected when rendering is never triggered.
    #[cfg(feature = "rendering")]
    let render_service = if config.rendering_runtime_enabled() {
        Some(Arc::new(crate::fetch::render::RenderService::new(
            config.chromium_path.clone(),
            config.render_concurrency,
            config.render_timeout_ms,
        )))
    } else {
        tracing::info!("render fallback disabled via LETTURA_RENDERING_ENABLED");
        None
    };

    tracing::info!(concurrency, "starting fetch workers");

    for _ in 0..concurrency {
        let rx = rx.clone();
        let pool = pool.clone();
        let storage = image_storage.clone();
        let search_index = search_index.clone();
        let client = client.clone();
        let depth = queue_depth.clone();
        // Each worker gets its own rate limiter — domains are independent
        // across workers, so contention on a shared Mutex isn't useful.
        let rate_limiter = Arc::new(Mutex::new(http::DomainRateLimiter::new()));

        let ctx = pipeline::FetchContext {
            pool,
            image_storage: storage,
            search_index,
            client,
            max_retries,
            rate_limiter,
            #[cfg(feature = "rendering")]
            render_service: render_service.clone(),
        };

        tokio::spawn(async move {
            loop {
                let job = {
                    let mut rx = rx.lock().await;
                    rx.recv().await
                };
                match job {
                    Some(job) => {
                        pipeline::process(&ctx, &job).await;
                        depth.fetch_sub(1, Ordering::Relaxed);
                    }
                    None => break,
                }
            }
        });
    }

    FetchQueue { tx, queue_depth }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn send_increments_queue_depth() {
        let (tx, mut rx) = mpsc::channel::<FetchJob>(100);
        let queue = FetchQueue {
            tx,
            queue_depth: Arc::new(AtomicUsize::new(0)),
        };

        let job = FetchJob {
            entry_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            url: "https://example.com".to_string(),
        };

        queue.send(job).await.unwrap();
        assert_eq!(queue.queue_depth.load(Ordering::Relaxed), 1);

        // Drain the channel so the sender doesn't hang
        let _ = rx.recv().await;
    }

    #[tokio::test]
    async fn send_fails_when_channel_closed() {
        let (tx, rx) = mpsc::channel::<FetchJob>(100);
        let queue = FetchQueue {
            tx,
            queue_depth: Arc::new(AtomicUsize::new(0)),
        };

        // Close the receiver so send will fail
        drop(rx);

        let job = FetchJob {
            entry_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            url: "https://example.com".to_string(),
        };

        let result = queue.send(job).await;
        assert!(result.is_err());
        // queue_depth should be rolled back to 0 on error
        assert_eq!(queue.queue_depth.load(Ordering::Relaxed), 0);
    }
}
