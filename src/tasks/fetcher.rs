use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::extract;
use crate::models::entry;
use crate::storage::{self, ImageStorage};

#[derive(Debug, Clone)]
pub struct FetchJob {
    pub entry_id: Uuid,
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

pub fn start_fetch_worker(pool: PgPool, concurrency: usize, image_storage: Arc<dyn ImageStorage>) -> FetchQueue {
    let (tx, rx) = mpsc::channel::<FetchJob>(5000);
    let rx = Arc::new(Mutex::new(rx));
    let queue_depth = Arc::new(AtomicUsize::new(0));

    for _ in 0..concurrency {
        let rx = rx.clone();
        let pool = pool.clone();
        let storage = image_storage.clone();
        let rate_limiter = Arc::new(Mutex::new(DomainRateLimiter::new()));
        let depth = queue_depth.clone();

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

    FetchQueue { tx, queue_depth }
}

async fn process_job(
    pool: &PgPool,
    rate_limiter: &Arc<Mutex<DomainRateLimiter>>,
    image_storage: &Arc<dyn ImageStorage>,
    job: &FetchJob,
) {
    if let Some(domain) = entry::extract_domain(&job.url) {
        let mut rl = rate_limiter.lock().await;
        rl.wait_if_needed(&domain).await;
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Lettura/0.1")
        .build()
        .unwrap_or_default();

    let fetch_result = client.get(&job.url).send().await;

    match fetch_result {
        Ok(response) => {
            let status = response.status().as_u16() as i16;
            match response.text().await {
                Ok(html) => match extract::extract(&html, Some(&job.url)) {
                    Ok(result) => {
                        // Process images: download and store locally/OSS
                        let content = storage::process_images(&result.content, image_storage.as_ref()).await;

                        entry::update_entry_content(
                            pool, job.entry_id,
                            result.title.as_deref(), Some(&content), Some(&result.text_content),
                            result.language.as_deref(), result.preview_image.as_deref(),
                            result.author.as_deref(), Some(result.reading_time as i32),
                            status, "readability",
                        ).await.ok();
                    }
                    Err(_) => {
                        entry::update_entry_content(pool, job.entry_id, None, None, None, None, None, None, None, status, "failed").await.ok();
                    }
                },
                Err(_) => {
                    entry::update_entry_content(pool, job.entry_id, None, None, None, None, None, None, None, status, "failed").await.ok();
                }
            }
        }
        Err(_) => {
            entry::update_entry_content(pool, job.entry_id, None, None, None, None, None, None, None, 0, "failed").await.ok();
        }
    }
}

struct DomainRateLimiter {
    last_request: HashMap<String, Instant>,
}

impl DomainRateLimiter {
    fn new() -> Self { Self { last_request: HashMap::new() } }

    async fn wait_if_needed(&mut self, domain: &str) {
        if let Some(last) = self.last_request.get(domain) {
            let elapsed = last.elapsed();
            if elapsed < std::time::Duration::from_secs(1) {
                tokio::time::sleep(std::time::Duration::from_secs(1) - elapsed).await;
            }
        }
        self.last_request.insert(domain.to_string(), Instant::now());
        if self.last_request.len() > 500 {
            let oldest = self.last_request.iter().min_by_key(|(_, v)| *v).map(|(k, _)| k.clone());
            if let Some(key) = oldest { self.last_request.remove(&key); }
        }
    }
}
