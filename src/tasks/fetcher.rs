use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::config::Config;
use crate::extract;
use crate::models::entry;
use crate::search::SearchIndex;
use crate::site_config;
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

    let client = build_http_client(config);

    tracing::info!(concurrency, "starting fetch workers");

    for _ in 0..concurrency {
        let rx = rx.clone();
        let pool = pool.clone();
        let storage = image_storage.clone();
        let search_index = search_index.clone();
        let rate_limiter = Arc::new(Mutex::new(DomainRateLimiter::new()));
        let depth = queue_depth.clone();
        let client = client.clone();
        let max_retries = config.fetch_max_retries;
        let rendering_url = config.rendering_url.clone();

        tokio::spawn(async move {
            loop {
                let job = {
                    let mut rx = rx.lock().await;
                    rx.recv().await
                };
                match job {
                    Some(job) => {
                        process_job(
                            &pool,
                            &rate_limiter,
                            &storage,
                            &search_index,
                            &client,
                            &job,
                            max_retries,
                            rendering_url.as_deref(),
                        ).await;
                        depth.fetch_sub(1, Ordering::Relaxed);
                    }
                    None => break,
                }
            }
        });
    }

    FetchQueue { tx, queue_depth }
}

fn build_http_client(config: &Config) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();

    headers.insert(
        reqwest::header::ACCEPT,
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
            .parse()
            .unwrap(),
    );
    headers.insert(
        reqwest::header::ACCEPT_LANGUAGE,
        "en-US,en;q=0.9,zh-CN;q=0.8,zh;q=0.7"
            .parse()
            .unwrap(),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("sec-fetch-mode"),
        "navigate".parse().unwrap(),
    );
    headers.insert(
        reqwest::header::CACHE_CONTROL,
        "max-age=0".parse().unwrap(),
    );

    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.fetch_timeout_secs))
        .user_agent(&config.user_agent)
        .cookie_store(true)
        .default_headers(headers);

    if let Some(ref proxy_url) = config.proxy {
        match reqwest::Proxy::all(proxy_url) {
            Ok(proxy) => {
                tracing::info!(proxy = %proxy_url, "configuring HTTP proxy");
                builder = builder.proxy(proxy);
            }
            Err(e) => {
                tracing::error!(proxy = %proxy_url, error = %e, "invalid proxy URL, ignoring");
            }
        }
    }

    builder.build().unwrap_or_default()
}

/// Fetch URL with retry on transient errors.
/// Retries on: timeout, connection errors, HTTP 5xx, HTTP 429.
/// Does NOT retry on 4xx (except 429).
async fn fetch_with_retry(
    client: &reqwest::Client,
    url: &str,
    max_retries: u32,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut last_error = None;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = Duration::from_millis(1000 * 2u64.pow(attempt - 1));
            // Add jitter: ±25%
            let jitter = (delay.as_millis() as f64 * 0.25 * (rand_simple() - 0.5).abs()) as u64;
            let actual_delay = delay + Duration::from_millis(jitter);
            tracing::debug!(attempt, delay_ms = actual_delay.as_millis(), url = %url, "retrying fetch");
            tokio::time::sleep(actual_delay).await;
        }

        match client.get(url).send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    return Ok(response);
                }
                // Retry on server errors and rate limiting
                if status.as_u16() == 429 || status.is_server_error() {
                    tracing::warn!(
                        attempt,
                        status = status.as_u16(),
                        url = %url,
                        "retryable HTTP error"
                    );
                    last_error = None; // We got a response, but need to retry
                    // For 429, try to respect Retry-After header
                    if status.as_u16() == 429 {
                        if let Some(retry_after) = response.headers().get("retry-after") {
                            if let Ok(secs) = retry_after.to_str().unwrap_or("0").parse::<u64>() {
                                tokio::time::sleep(Duration::from_secs(secs)).await;
                            }
                        }
                    }
                    continue;
                }
                // Client errors (4xx except 429): don't retry
                return Ok(response);
            }
            Err(e) => {
                let is_retryable = e.is_timeout() || e.is_connect() || e.is_request();
                tracing::warn!(
                    attempt,
                    error = %e,
                    is_timeout = e.is_timeout(),
                    is_connect = e.is_connect(),
                    url = %url,
                    "HTTP request error"
                );
                if is_retryable && attempt < max_retries {
                    last_error = Some(e);
                    continue;
                }
                return Err(e);
            }
        }
    }

    // All retries exhausted with HTTP error status codes (5xx/429)
    // Re-fetch one last time to return the actual response for the caller to handle
    match client.get(url).send().await {
        Ok(response) => Ok(response),
        Err(e) => Err(last_error.unwrap_or(e)),
    }
}

/// Simple deterministic pseudo-random for jitter (avoids pulling in full rand crate here).
/// Returns a value between 0.0 and 1.0.
fn rand_simple() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos as f64 / u32::MAX as f64)
}

/// Fetch with retry using a pre-built request builder (supports custom headers).
async fn fetch_with_retry_from_builder(
    request_builder: reqwest::RequestBuilder,
    url: &str,
    client: &reqwest::Client,
    max_retries: u32,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut last_error = None;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = Duration::from_millis(1000 * 2u64.pow(attempt - 1));
            let jitter = (delay.as_millis() as f64 * 0.25 * (rand_simple() - 0.5).abs()) as u64;
            let actual_delay = delay + Duration::from_millis(jitter);
            tracing::debug!(attempt, delay_ms = actual_delay.as_millis(), url = %url, "retrying fetch");
            tokio::time::sleep(actual_delay).await;
        }

        // Build a fresh request each attempt (RequestBuilder is consumed)
        let req = if attempt == 0 {
            request_builder
                .try_clone()
                .unwrap_or_else(|| client.get(url))
                .send()
                .await
        } else {
            client.get(url).send().await
        };

        match req {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    return Ok(response);
                }
                if status.as_u16() == 429 || status.is_server_error() {
                    tracing::warn!(attempt, status = status.as_u16(), url = %url, "retryable HTTP error");
                    if status.as_u16() == 429 {
                        if let Some(retry_after) = response.headers().get("retry-after") {
                            if let Ok(secs) = retry_after.to_str().unwrap_or("0").parse::<u64>() {
                                tokio::time::sleep(Duration::from_secs(secs)).await;
                            }
                        }
                    }
                    continue;
                }
                return Ok(response);
            }
            Err(e) => {
                let is_retryable = e.is_timeout() || e.is_connect() || e.is_request();
                tracing::warn!(attempt, error = %e, is_timeout = e.is_timeout(), url = %url, "HTTP request error");
                if is_retryable && attempt < max_retries {
                    last_error = Some(e);
                    continue;
                }
                return Err(e);
            }
        }
    }

    match client.get(url).send().await {
        Ok(response) => Ok(response),
        Err(e) => Err(last_error.unwrap_or(e)),
    }
}

/// Process a job that requires JS rendering (from site config).
async fn process_rendered(
    pool: &PgPool,
    image_storage: &Arc<dyn ImageStorage>,
    search_index: &SearchIndex,
    job: &FetchJob,
    client: &reqwest::Client,
    max_retries: u32,
    render_url: &str,
    sc: &site_config::SiteConfig,
) {
    match fetch_rendered(render_url, &job.url, client, max_retries).await {
        Ok(rendered_html) => {
            let rule = site_config_to_rule_config(sc);
            let extract_result = extract::extract_with_fallback(
                &rendered_html, Some(&job.url), Some(&rule),
            );
            match extract_result {
                Ok(result) => {
                    save_extracted_content(
                        pool, image_storage, search_index, job, &result.inner, 200, "rendering",
                    ).await;
                }
                Err(_) => {
                    tracing::warn!(entry_id = %job.entry_id, "rendered content extraction failed");
                    entry::update_entry_content(
                        pool, job.entry_id, None, None, None, None, None, None, None,
                        200, "failed",
                    ).await.ok();
                }
            }
        }
        Err(e) => {
            tracing::warn!(entry_id = %job.entry_id, error = %e, "rendering service failed");
            entry::update_entry_content(
                pool, job.entry_id, None, None, None, None, None, None, None,
                0, "failed",
            ).await.ok();
        }
    }
}

/// Convert a file-based SiteConfig to the extract module's SiteRuleConfig.
fn site_config_to_rule_config(sc: &site_config::SiteConfig) -> extract::SiteRuleConfig {
    extract::SiteRuleConfig {
        content_selector: sc.body_selectors.first().cloned(),
        title_selector: sc.title_selectors.first().cloned(),
        strip_selectors: if sc.strip_selectors.is_empty() {
            None
        } else {
            Some(sc.strip_selectors.clone())
        },
    }
}

#[allow(clippy::too_many_arguments)]
async fn process_job(
    pool: &PgPool,
    rate_limiter: &Arc<Mutex<DomainRateLimiter>>,
    image_storage: &Arc<dyn ImageStorage>,
    search_index: &SearchIndex,
    client: &reqwest::Client,
    job: &FetchJob,
    max_retries: u32,
    rendering_url: Option<&str>,
) {
    tracing::info!(entry_id = %job.entry_id, url = %job.url, "fetch job started");

    // Rate limiting
    if let Some(domain) = entry::extract_domain(&job.url) {
        let mut rl = rate_limiter.lock().await;
        rl.wait_if_needed(&domain).await;
    }

    // Look up site config (file-based) before fetching
    let domain_str = entry::extract_domain(&job.url).unwrap_or_default();
    let site_config = site_config::store::find_config(&domain_str, &job.url);

    // If config says render: true, skip static fetch and go directly to browserless
    if let Some(ref sc) = site_config {
        if sc.render {
            if let Some(render_url) = rendering_url {
                tracing::info!(
                    entry_id = %job.entry_id,
                    domain = %domain_str,
                    "site config requires JS rendering, skipping static fetch"
                );
                process_rendered(
                    pool, image_storage, search_index, job, client, max_retries,
                    render_url, sc,
                ).await;
                return;
            }
            tracing::warn!(
                entry_id = %job.entry_id,
                "site config requires rendering but no rendering service configured"
            );
        }
    }

    // Build request with optional custom headers from site config
    let mut request_builder = client.get(&job.url);
    if let Some(ref sc) = site_config {
        for (name, value) in &sc.extra_headers {
            if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) {
                if let Ok(header_value) = reqwest::header::HeaderValue::from_str(value) {
                    request_builder = request_builder.header(header_name, header_value);
                }
            }
        }
    }

    let fetch_result = fetch_with_retry_from_builder(request_builder, &job.url, client, max_retries).await;

    match fetch_result {
        Ok(response) => {
            let status = response.status().as_u16() as i16;
            match response.text().await {
                Ok(html) => {
                    process_html(
                        pool, image_storage, search_index, job, &html, status,
                        client, max_retries, rendering_url, site_config.as_ref(),
                    ).await;
                }
                Err(e) => {
                    tracing::warn!(
                        entry_id = %job.entry_id,
                        status,
                        error = %e,
                        "failed to read response body"
                    );
                    entry::update_entry_content(
                        pool, job.entry_id, None, None, None, None, None, None, None,
                        status, "failed",
                    ).await.ok();
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                entry_id = %job.entry_id,
                url = %job.url,
                error = %e,
                is_timeout = e.is_timeout(),
                is_connect = e.is_connect(),
                "fetch HTTP error after retries"
            );
            entry::update_entry_content(
                pool, job.entry_id, None, None, None, None, None, None, None,
                0, "failed",
            ).await.ok();
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn process_html(
    pool: &PgPool,
    image_storage: &Arc<dyn ImageStorage>,
    search_index: &SearchIndex,
    job: &FetchJob,
    html: &str,
    status: i16,
    client: &reqwest::Client,
    max_retries: u32,
    rendering_url: Option<&str>,
    site_config: Option<&site_config::SiteConfig>,
) {
    // Priority for extraction selectors:
    // 1. File-based site config (highest)
    // 2. DB site_rules (medium)
    // 3. Readability auto-detection (default)
    let site_rule_config = if let Some(sc) = site_config {
        Some(site_config_to_rule_config(sc))
    } else if let Some(ref domain) = entry::extract_domain(&job.url) {
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

    // Try extraction with fallback chain
    let extract_result = extract::extract_with_fallback(html, Some(&job.url), site_rule_config.as_ref());

    match extract_result {
        Ok(result) => {
            let text_len = result.inner.text_content.len();

            // If content is very short and rendering service is available, try JS rendering
            if text_len < 100 {
                if let Some(render_url) = rendering_url {
                    tracing::info!(
                        entry_id = %job.entry_id,
                        text_len,
                        "content too short, attempting JS rendering"
                    );
                    if let Ok(rendered_html) = fetch_rendered(render_url, &job.url, client, max_retries).await {
                        let rendered_result = extract::extract_with_fallback(
                            &rendered_html, Some(&job.url), site_rule_config.as_ref(),
                        );
                        if let Ok(r) = rendered_result {
                            if r.inner.text_content.len() > text_len {
                                tracing::info!(
                                    entry_id = %job.entry_id,
                                    original_len = text_len,
                                    rendered_len = r.inner.text_content.len(),
                                    "JS rendering improved content"
                                );
                                save_extracted_content(
                                    pool, image_storage, search_index, job, &r.inner, status, "rendering",
                                ).await;
                                return;
                            }
                        }
                    }
                    tracing::warn!(
                        entry_id = %job.entry_id,
                        "JS rendering did not improve content, using static extraction"
                    );
                }
            }

            let method = match result.method {
                extract::ExtractMethod::SiteRule => "site_rule",
                extract::ExtractMethod::Readability => "readability",
                extract::ExtractMethod::BodyFallback => "fallback",
                extract::ExtractMethod::RawHtml => "fallback",
            };
            save_extracted_content(pool, image_storage, search_index, job, &result.inner, status, method).await;
        }
        Err(_) => {
            tracing::warn!(
                entry_id = %job.entry_id,
                status,
                "all extraction methods failed"
            );
            entry::update_entry_content(
                pool, job.entry_id, None, None, None, None, None, None, None,
                status, "failed",
            ).await.ok();
        }
    }
}

async fn save_extracted_content(
    pool: &PgPool,
    image_storage: &Arc<dyn ImageStorage>,
    search_index: &SearchIndex,
    job: &FetchJob,
    result: &extract::ExtractResult,
    status: i16,
    method: &str,
) {
    let content = storage::process_images(&result.content, image_storage.as_ref()).await;

    entry::update_entry_content(
        pool,
        job.entry_id,
        result.title.as_deref(),
        Some(&content),
        Some(&result.text_content),
        result.language.as_deref(),
        result.preview_image.as_deref(),
        result.author.as_deref(),
        Some(result.reading_time as i32),
        status,
        method,
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

    apply_tagging_rules(pool, job.user_id, job.entry_id, &job.url, result).await;

    tracing::debug!(
        entry_id = %job.entry_id,
        method,
        reading_time = result.reading_time,
        "fetch job completed"
    );
}

/// Fetch rendered HTML from an external browser rendering service (e.g., browserless).
async fn fetch_rendered(
    rendering_url: &str,
    target_url: &str,
    client: &reqwest::Client,
    _max_retries: u32,
) -> Result<String, String> {
    use serde_json::json;

    let endpoint = format!("{}/content", rendering_url.trim_end_matches('/'));
    let body = json!({
        "url": target_url,
    });

    let response = client
        .post(&endpoint)
        .json(&body)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| format!("rendering service request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("rendering service returned status {}", response.status()));
    }

    response
        .text()
        .await
        .map_err(|e| format!("failed to read rendering response: {}", e))
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
            if elapsed < Duration::from_secs(1) {
                tokio::time::sleep(Duration::from_secs(1) - elapsed).await;
            }
        }
        self.last_request.insert(domain.to_string(), Instant::now());
        if self.last_request.len() > 500 {
            let oldest = self.last_request.iter().min_by_key(|(_, v)| *v).map(|(k, _)| k.clone());
            if let Some(key) = oldest { self.last_request.remove(&key); }
        }
    }
}
