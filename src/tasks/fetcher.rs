use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::extract;
use crate::models::entry;
use crate::search::SearchIndex;
use crate::storage::{self, ImageStorage};

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

pub fn start_fetch_worker(pool: PgPool, concurrency: usize, image_storage: Arc<dyn ImageStorage>, search_index: SearchIndex) -> FetchQueue {
    let (tx, rx) = mpsc::channel::<FetchJob>(5000);
    let rx = Arc::new(Mutex::new(rx));
    let queue_depth = Arc::new(AtomicUsize::new(0));
    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Lettura/0.1")
            .build()
            .unwrap_or_default(),
    );

    for _ in 0..concurrency {
        let rx = rx.clone();
        let pool = pool.clone();
        let storage = image_storage.clone();
        let search_index = search_index.clone();
        let rate_limiter = Arc::new(Mutex::new(DomainRateLimiter::new()));
        let depth = queue_depth.clone();
        let client = client.clone();

        tokio::spawn(async move {
            loop {
                let job = {
                    let mut rx = rx.lock().await;
                    rx.recv().await
                };
                match job {
                    Some(job) => {
                        process_job(&pool, &rate_limiter, &storage, &search_index, &client, &job).await;
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
    search_index: &SearchIndex,
    client: &reqwest::Client,
    job: &FetchJob,
) {
    tracing::debug!(entry_id = %job.entry_id, url = %job.url, "fetch job started");

    if let Some(domain) = entry::extract_domain(&job.url) {
        let mut rl = rate_limiter.lock().await;
        tracing::debug!(domain = %domain, "rate limiting domain");
        rl.wait_if_needed(&domain).await;
    }

    let fetch_result = client.get(&job.url).send().await;

    match fetch_result {
        Ok(response) => {
            let status = response.status().as_u16() as i16;
            match response.text().await {
                Ok(html) => {
                    let site_rule_config = if let Some(ref domain) = entry::extract_domain(&job.url) {
                        match crate::models::site_rule::find_by_domain(pool, job.user_id, domain).await {
                            Ok(Some(rule)) => Some(extract::SiteRuleConfig {
                                content_selector: Some(rule.content_selector),
                                title_selector: rule.title_selector,
                                strip_selectors: rule.strip_selectors,
                            }),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    match extract::extract(&html, Some(&job.url), site_rule_config.as_ref()) {
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
                            let domain = entry::find_entry_by_id(pool, job.user_id, job.entry_id)
                                .await
                                .ok()
                                .flatten()
                                .and_then(|e| e.domain_name);
                            let _ = search_index.upsert(
                                job.entry_id,
                                job.user_id,
                                result.title.as_deref().unwrap_or(""),
                                &result.text_content,
                                &job.url,
                                domain.as_deref().unwrap_or(""),
                            ).await;
                            apply_tagging_rules(pool, job.user_id, job.entry_id, &job.url, &result).await;
                            tracing::debug!(entry_id = %job.entry_id, "fetch job completed");
                        }
                        Err(_) => {
                            tracing::warn!(entry_id = %job.entry_id, "content extraction failed");
                            entry::update_entry_content(pool, job.entry_id, None, None, None, None, None, None, None, status, "failed").await.ok();
                        }
                    }
                }
                Err(_) => {
                    entry::update_entry_content(pool, job.entry_id, None, None, None, None, None, None, None, status, "failed").await.ok();
                }
            }
        }
        Err(_) => {
            tracing::warn!(entry_id = %job.entry_id, url = %job.url, "fetch HTTP error");
            entry::update_entry_content(pool, job.entry_id, None, None, None, None, None, None, None, 0, "failed").await.ok();
        }
    }
}

async fn apply_tagging_rules(
    pool: &PgPool,
    user_id: Uuid,
    entry_id: Uuid,
    url: &str,
    result: &extract::ExtractResult,
) {
    let rules = match crate::models::tagging_rule::list_rules(pool, user_id).await {
        Ok(r) => r,
        Err(_) => return,
    };
    let domain = entry::extract_domain(url).unwrap_or_default();
    let fields = crate::models::tagging_rule::EntryFields {
        title: result.title.clone().unwrap_or_default(),
        url: url.to_string(),
        domain_name: domain,
        language: result.language.clone().unwrap_or_default(),
        reading_time: result.reading_time as i32,
        content_type: "article".to_string(),
    };
    for rule in rules {
        if crate::models::tagging_rule::evaluate_rule(&rule.rule, &fields) {
            for tag_label in &rule.tags {
                if let Ok(tag) = crate::models::tag::find_or_create_tag(pool, user_id, tag_label).await {
                    crate::models::tag::add_tag_to_entry(pool, entry_id, tag.id).await.ok();
                }
            }
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
