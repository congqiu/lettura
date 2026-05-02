//! Top-level fetch pipeline: orchestrates rewrite → HTTP → extract → save.
//!
//! This module is the entry point for each `FetchJob` popped off the queue.
//! The render fallback is wired in a later task; for now the pipeline handles
//! the static path end-to-end.

use crate::extract::{self, ExtractResult};
use crate::fetch::{http, json_extract, rewrite};
use crate::models::entry;
use crate::search::SearchIndex;
use crate::site_config::{self, RenderMode, ResponseType, SiteConfig};
use crate::storage::ImageStorage;
use crate::tasks::fetcher::FetchJob;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Minimum text length below which we consider the extracted content too
/// short and attempt the render fallback (when available and mode != Never).
const SHORT_CONTENT_THRESHOLD: usize = 100;

/// Resources shared by all fetch workers. Built once at startup.
pub struct FetchContext {
    pub pool: PgPool,
    pub image_storage: Arc<dyn ImageStorage>,
    pub search_index: SearchIndex,
    pub client: reqwest::Client,
    pub max_retries: u32,
    pub rate_limiter: Arc<Mutex<http::DomainRateLimiter>>,
    #[cfg(feature = "rendering")]
    pub render_service: Option<Arc<crate::fetch::render::RenderService>>,
}

/// Process a single fetch job end-to-end: look up the site config, apply URL
/// rewrite and request overrides, issue the HTTP request, extract content,
/// save it to the DB + search index, and run tagging rules.
pub async fn process(ctx: &FetchContext, job: &FetchJob) {
    tracing::info!(entry_id = %job.entry_id, url = %job.url, "fetch job started");

    // Per-domain politeness: 1 request/sec.
    if let Some(domain) = entry::extract_domain(&job.url) {
        let mut rl = ctx.rate_limiter.lock().await;
        rl.wait_if_needed(&domain).await;
    }

    // Resolve site config for this URL (YAML file match).
    let domain_str = entry::extract_domain(&job.url).unwrap_or_default();
    let site_config = site_config::store::find_config(&domain_str, &job.url);

    // render.mode == force → skip the static path entirely.
    if let Some(ref sc) = site_config {
        if sc.render.mode == RenderMode::Force {
            if try_render_then_extract(ctx, job, sc, 200).await {
                return;
            }
            tracing::warn!(
                entry_id = %job.entry_id,
                "render forced but fallback unavailable or failed; attempting static fetch"
            );
        }
    }

    // Apply URL rewrite rules if present.
    let effective_url = match &site_config {
        Some(sc) if !sc.rewrite.is_empty() => {
            let rewritten = rewrite::apply(&job.url, &sc.rewrite);
            if rewritten != job.url {
                tracing::info!(entry_id = %job.entry_id, from = %job.url, to = %rewritten, "URL rewritten");
            }
            rewritten
        }
        _ => job.url.clone(),
    };

    // Build the request with per-site overrides.
    let mut builder = ctx.client.get(&effective_url);
    if let Some(ref sc) = site_config {
        builder = http::apply_request_config(builder, &sc.request);
    }

    let fetch_result =
        http::fetch_with_retry(builder, &effective_url, &ctx.client, ctx.max_retries).await;

    match fetch_result {
        Ok(response) => {
            let status = response.status().as_u16() as i16;
            match response.text().await {
                Ok(body) => {
                    process_body(ctx, job, body, status, site_config.as_ref()).await;
                }
                Err(e) => {
                    tracing::warn!(entry_id = %job.entry_id, status, error = %e, "failed to read response body");
                    if !fallback_render(ctx, job, site_config.as_ref(), status).await {
                        mark_failed(&ctx.pool, job.entry_id, status).await;
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                entry_id = %job.entry_id,
                url = %effective_url,
                error = %e,
                is_timeout = e.is_timeout(),
                is_connect = e.is_connect(),
                "fetch HTTP error after retries"
            );
            if !fallback_render(ctx, job, site_config.as_ref(), 0).await {
                mark_failed(&ctx.pool, job.entry_id, 0).await;
            }
        }
    }
}

/// Decide how to extract content based on site config response type and dispatch.
/// Takes `body` by value so the HTML path can move it into `spawn_blocking`
/// without an extra clone — for large pages this is the heaviest allocation
/// in the fetch pipeline.
async fn process_body(
    ctx: &FetchContext,
    job: &FetchJob,
    body: String,
    status: i16,
    site_config: Option<&SiteConfig>,
) {
    let response_type = site_config
        .map(|sc| sc.response.response_type)
        .unwrap_or(ResponseType::Html);

    match response_type {
        ResponseType::Json => {
            let Some(sc) = site_config else {
                tracing::warn!(entry_id = %job.entry_id, "JSON response type requires site config");
                mark_failed(&ctx.pool, job.entry_id, status).await;
                return;
            };
            let Some(rules) = sc.response.json.as_ref() else {
                tracing::warn!(entry_id = %job.entry_id, "JSON response type without json rules");
                mark_failed(&ctx.pool, job.entry_id, status).await;
                return;
            };
            match json_extract::extract(&body, rules) {
                Ok(result) => {
                    save(ctx, job, &result, status, "site_rule").await;
                }
                Err(e) => {
                    tracing::warn!(entry_id = %job.entry_id, error = %e, "JSON extraction failed");
                    mark_failed(&ctx.pool, job.entry_id, status).await;
                }
            }
        }
        ResponseType::Html => {
            let site_rule_config = html_rules_from_config(ctx, job, site_config).await;
            let url_owned = job.url.clone();
            let cfg_owned = site_rule_config.clone();
            let start = std::time::Instant::now();
            let extract_outcome = tokio::task::spawn_blocking(move || {
                extract::extract_with_fallback(&body, Some(&url_owned), cfg_owned.as_ref())
            })
            .await;
            let elapsed = start.elapsed().as_secs_f64();
            let extract_result = match extract_outcome {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(entry_id = %job.entry_id, error = %e, "extract task panicked");
                    metrics::histogram!("extract_duration_seconds", "method" => "panic")
                        .record(elapsed);
                    mark_failed(&ctx.pool, job.entry_id, status).await;
                    return;
                }
            };
            match extract_result {
                Ok(result) => {
                    let method = match result.method {
                        extract::ExtractMethod::SiteRule => "site_rule",
                        extract::ExtractMethod::Readability => "readability",
                        extract::ExtractMethod::BodyFallback | extract::ExtractMethod::RawHtml => {
                            "fallback"
                        }
                    };
                    metrics::histogram!("extract_duration_seconds", "method" => method)
                        .record(elapsed);
                    // Content too short → try rendering (if allowed by config + available).
                    if result.inner.text_content.len() < SHORT_CONTENT_THRESHOLD {
                        if let Some(sc) = site_config {
                            if sc.render.mode != RenderMode::Never
                                && try_render_then_extract(ctx, job, sc, status).await
                            {
                                return;
                            }
                        }
                    }
                    save(ctx, job, &result.inner, status, method).await;
                }
                Err(_) => {
                    metrics::histogram!("extract_duration_seconds", "method" => "error")
                        .record(elapsed);
                    tracing::warn!(entry_id = %job.entry_id, status, "all HTML extraction methods failed");
                    if !fallback_render(ctx, job, site_config, status).await {
                        mark_failed(&ctx.pool, job.entry_id, status).await;
                    }
                }
            }
        }
    }
}

/// Build the extract::SiteRuleConfig from a YAML SiteConfig's HTML rules, falling
/// back to the legacy DB site_rules table if no YAML-level HTML rules exist.
async fn html_rules_from_config(
    ctx: &FetchContext,
    job: &FetchJob,
    site_config: Option<&SiteConfig>,
) -> Option<extract::SiteRuleConfig> {
    if let Some(sc) = site_config {
        if let Some(html) = sc.response.html.as_ref() {
            return Some(extract::SiteRuleConfig {
                content_selector: html.body.first().cloned(),
                title_selector: html.title.clone(),
                strip_selectors: if html.strip.is_empty() {
                    None
                } else {
                    Some(html.strip.clone())
                },
            });
        }
    }
    // DB site_rules fallback (preserves Plan 3a behavior).
    if let Some(ref domain) = entry::extract_domain(&job.url) {
        if let Ok(Some(rule)) =
            crate::models::site_rule::find_by_domain(&ctx.pool, job.user_id, domain).await
        {
            return Some(extract::SiteRuleConfig {
                content_selector: Some(rule.content_selector),
                title_selector: rule.title_selector,
                strip_selectors: rule.strip_selectors,
            });
        }
    }
    None
}

/// Persist extracted content, update the search index, apply tagging rules.
async fn save(
    ctx: &FetchContext,
    job: &FetchJob,
    result: &ExtractResult,
    status: i16,
    method: &str,
) {
    // Save original content first, then queue async image processing
    if let Err(e) = entry::update_entry_content(
        &ctx.pool,
        job.entry_id,
        result.title.as_deref(),
        Some(&result.content), // Original HTML with external image URLs
        Some(&result.text_content),
        result.language.as_deref(),
        result.preview_image.as_deref(),
        result.author.as_deref(),
        Some(result.reading_time as i32),
        status,
        method,
    )
    .await
    {
        tracing::warn!(entry_id = %job.entry_id, "failed to update entry content: {e}");
    }

    // Queue async image processing job
    if let Err(e) = crate::models::image_process_job::create(
        &ctx.pool,
        job.entry_id,
        &result.content,
    )
    .await
    {
        tracing::warn!(
            entry_id = %job.entry_id,
            error = %e,
            "failed to create image process job"
        );
    }

    let domain = match entry::find_entry_by_id(&ctx.pool, job.user_id, job.entry_id).await {
        Ok(Some(e)) => e.domain_name,
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(entry_id = %job.entry_id, "failed to fetch entry for domain: {e}");
            None
        }
    };

    if let Err(e) = ctx
        .search_index
        .upsert(
            job.entry_id,
            job.user_id,
            result.title.as_deref().unwrap_or(""),
            &result.text_content,
            &job.url,
            domain.as_deref().unwrap_or(""),
        )
        .await
    {
        tracing::error!("Failed to index entry {}: {e}", job.entry_id);
    }

    apply_tagging_rules(&ctx.pool, job.user_id, job.entry_id, &job.url, result).await;

    tracing::debug!(
        entry_id = %job.entry_id,
        method,
        reading_time = result.reading_time,
        "fetch job completed"
    );
}

async fn apply_tagging_rules(
    pool: &PgPool,
    user_id: Uuid,
    entry_id: Uuid,
    url: &str,
    result: &ExtractResult,
) {
    // Use cached version - this is called on every fetch
    let rules = match crate::models::tagging_rule::list_rules_cached(pool, user_id).await {
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
                if let Ok(tag) =
                    crate::models::tag::find_or_create_tag(pool, user_id, tag_label).await
                {
                    if let Err(e) = crate::models::tag::add_tag_to_entry(pool, user_id, entry_id, tag.id).await {
                        tracing::warn!(entry_id = %entry_id, tag_id = %tag.id, "failed to apply auto-tag: {e}");
                    }
                }
            }
        }
    }
}

async fn mark_failed(pool: &PgPool, entry_id: Uuid, status: i16) {
    if let Err(e) = entry::update_entry_content(
        pool, entry_id, None, None, None, None, None, None, None, status, "failed",
    )
    .await
    {
        tracing::warn!(entry_id = %entry_id, "failed to mark entry as failed: {e}");
    }
}

/// Entry point for falling back to rendering when static fetch fails or content
/// is too short. Returns true if rendering produced a usable save; false if the
/// caller should treat this as a hard failure.
async fn fallback_render(
    ctx: &FetchContext,
    job: &FetchJob,
    site_config: Option<&SiteConfig>,
    status: i16,
) -> bool {
    let Some(sc) = site_config else {
        return false;
    };
    if sc.render.mode == RenderMode::Never {
        return false;
    }
    try_render_then_extract(ctx, job, sc, status).await
}

#[cfg(feature = "rendering")]
async fn try_render_then_extract(
    ctx: &FetchContext,
    job: &FetchJob,
    sc: &SiteConfig,
    status: i16,
) -> bool {
    let Some(rs) = ctx.render_service.as_ref() else {
        return false;
    };
    let wait_for = sc.render.wait_for.as_deref();
    let timeout_override = sc
        .render
        .timeout_ms
        .map(std::time::Duration::from_millis);
    tracing::info!(entry_id = %job.entry_id, "invoking render fallback");
    match rs.render(&job.url, wait_for, timeout_override).await {
        Ok(html) => {
            let site_rule_config = html_rules_from_config(ctx, job, Some(sc)).await;
            let url_owned = job.url.clone();
            let cfg_owned = site_rule_config.clone();
            let start = std::time::Instant::now();
            let extract_outcome = tokio::task::spawn_blocking(move || {
                extract::extract_with_fallback(&html, Some(&url_owned), cfg_owned.as_ref())
            })
            .await;
            let elapsed = start.elapsed().as_secs_f64();
            let extract_result = match extract_outcome {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(entry_id = %job.entry_id, error = %e, "render-path extract task panicked");
                    metrics::histogram!("extract_duration_seconds", "method" => "render_panic")
                        .record(elapsed);
                    return false;
                }
            };
            match extract_result {
                Ok(result) => {
                    metrics::histogram!("extract_duration_seconds", "method" => "rendering")
                        .record(elapsed);
                    save(ctx, job, &result.inner, status, "rendering").await;
                    true
                }
                Err(_) => {
                    metrics::histogram!("extract_duration_seconds", "method" => "render_error")
                        .record(elapsed);
                    tracing::warn!(entry_id = %job.entry_id, "rendered HTML extraction failed");
                    false
                }
            }
        }
        Err(e) => {
            tracing::warn!(entry_id = %job.entry_id, error = %e, "render fallback failed");
            false
        }
    }
}

#[cfg(not(feature = "rendering"))]
async fn try_render_then_extract(
    _ctx: &FetchContext,
    _job: &FetchJob,
    _sc: &SiteConfig,
    _status: i16,
) -> bool {
    false
}
