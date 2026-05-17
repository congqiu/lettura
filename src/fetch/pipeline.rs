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

/// Classification of fetch pipeline failures.
///
/// Returned by [`process`] so the worker can decide between retrying with
/// backoff (`Transient`) and dead-lettering immediately (`Permanent`).
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

/// Minimum text length below which we consider the extracted content too
/// short and attempt the render fallback (when available and mode != Never).
const SHORT_CONTENT_THRESHOLD: usize = 100;

/// Decide whether the extracted content is too short and rendering should be
/// attempted. Pure function — no dependencies on DB, HTTP, or async runtime.
pub(crate) fn should_try_render(text_len: usize, render_mode: RenderMode) -> bool {
    text_len < SHORT_CONTENT_THRESHOLD && render_mode != RenderMode::Never
}

/// Resources shared by all fetch workers. Built once at startup.
pub struct FetchContext {
    pub pool: PgPool,
    pub image_storage: Arc<dyn ImageStorage>,
    pub search_index: SearchIndex,
    pub client: reqwest::Client,
    pub max_retries: u32,
    pub rate_limiter: Arc<Mutex<http::DomainRateLimiter>>,
    /// Same Arc as `AppState.caches`. `apply_tagging_rules` invalidates
    /// `tags` / `tag_stats` here, and handlers reading via state.caches see
    /// the invalidation immediately.
    pub caches: Arc<crate::cache::Caches>,
    #[cfg(feature = "rendering")]
    pub render_service: Option<Arc<crate::fetch::render::RenderService>>,
    /// Test-only escape hatch to bypass SSRF validation. Gated behind
    /// `cfg(any(test, feature = "test-utils"))` so the field literally does
    /// not exist in production release builds — there is no code path that
    /// can silently set it to `true`. NEVER enable the `test-utils` feature
    /// in a release binary.
    #[cfg(any(test, feature = "test-utils"))]
    pub skip_ssrf: bool,
}

/// Resolve the effective SSRF-skip flag for the current build. In production
/// (no `test-utils` and not `cfg(test)`) this is a compile-time `false`, so
/// the optimiser strips the bypass branch entirely.
#[inline(always)]
fn skip_ssrf(_ctx: &FetchContext) -> bool {
    #[cfg(any(test, feature = "test-utils"))]
    {
        _ctx.skip_ssrf
    }
    #[cfg(not(any(test, feature = "test-utils")))]
    {
        false
    }
}

/// Process a single fetch job end-to-end: look up the site config, apply URL
/// rewrite and request overrides, issue the HTTP request, extract content,
/// save it to the DB + search index, and run tagging rules.
///
/// Returns:
/// - `Ok(())` on success (entry content saved, possibly via render fallback).
/// - `Err(FetchError::Permanent(_))` for 4xx, SSRF blocks, or permanent
///   extraction failures — the worker should drop the job and mark the entry
///   failed.
/// - `Err(FetchError::Transient(_))` for 5xx, timeouts, network errors, or
///   render failures — the worker should retry with backoff.
///
/// On error the pipeline does NOT write entry failure state itself; that is
/// the worker's responsibility once it has consumed this `Result`.
pub async fn process(ctx: &FetchContext, job: &FetchJob) -> Result<(), FetchError> {
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
    if let Some(ref sc) = site_config
        && sc.render.mode == RenderMode::Force
    {
        // SSRF protection must apply to the render path too.
        if !skip_ssrf(ctx)
            && let Err(e) = crate::fetch::ssrf::validate_url(&job.url)
        {
            tracing::warn!(entry_id = %job.entry_id, url = %job.url, "SSRF blocked (render): {e}");
            return Err(FetchError::Permanent(format!("SSRF blocked: {e}")));
        }
        if try_render_then_extract(ctx, job, sc, 200).await {
            return Ok(());
        }
        tracing::warn!(
            entry_id = %job.entry_id,
            "render forced but fallback unavailable or failed; attempting static fetch"
        );
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

    // SSRF protection: block requests to private/reserved IPs.
    if !skip_ssrf(ctx)
        && let Err(e) = crate::fetch::ssrf::validate_url(&effective_url)
    {
        tracing::warn!(entry_id = %job.entry_id, url = %effective_url, "SSRF blocked: {e}");
        return Err(FetchError::Permanent(format!("SSRF blocked: {e}")));
    }

    // Build request config from site config overrides.
    let request_config = site_config.as_ref().map(|sc| &sc.request);

    let fetch_result = http::fetch_with_retry(
        &effective_url,
        &ctx.client,
        ctx.max_retries,
        request_config,
        skip_ssrf(ctx),
    )
    .await;

    match fetch_result {
        Ok(response) => {
            let status = response.status().as_u16() as i16;
            match response.text().await {
                Ok(body) => process_body(ctx, job, body, status, site_config.as_ref()).await,
                Err(e) => {
                    tracing::warn!(entry_id = %job.entry_id, status, error = %e, "failed to read response body");
                    if fallback_render(ctx, job, site_config.as_ref(), status).await {
                        Ok(())
                    } else if (400..500).contains(&status) {
                        // 4xx response whose body could not be read — treat as
                        // a permanent failure so the worker does not retry.
                        Err(FetchError::Permanent(format!("http {status}")))
                    } else {
                        // 5xx or other status with unreadable body — retry-eligible.
                        Err(FetchError::Transient(format!("http {status}")))
                    }
                }
            }
        }
        Err(e) => {
            let is_timeout = matches!(&e, http::FetchError::Reqwest(re) if re.is_timeout());
            let is_connect = matches!(&e, http::FetchError::Reqwest(re) if re.is_connect());
            tracing::warn!(
                entry_id = %job.entry_id,
                url = %effective_url,
                error = %e,
                is_timeout,
                is_connect,
                "fetch HTTP error after retries"
            );
            if fallback_render(ctx, job, site_config.as_ref(), 0).await {
                Ok(())
            } else {
                Err(FetchError::Transient(e.to_string()))
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
) -> Result<(), FetchError> {
    let response_type = site_config
        .map(|sc| sc.response.response_type)
        .unwrap_or(ResponseType::Html);

    // Classify failures by HTTP status: 4xx → Permanent, otherwise (5xx or
    // missing config / extraction error on a 2xx body) → Transient. The
    // status-based split lets the worker route by retry semantics without
    // duplicating logic here.
    let classify = |msg: String| -> FetchError {
        if (400..500).contains(&status) {
            FetchError::Permanent(msg)
        } else {
            FetchError::Transient(msg)
        }
    };

    match response_type {
        ResponseType::Json => {
            let Some(sc) = site_config else {
                tracing::warn!(entry_id = %job.entry_id, "JSON response type requires site config");
                return Err(classify(format!("http {status}")));
            };
            let Some(rules) = sc.response.json.as_ref() else {
                tracing::warn!(entry_id = %job.entry_id, "JSON response type without json rules");
                return Err(classify(format!("http {status}")));
            };
            match json_extract::extract(&body, rules) {
                Ok(result) => {
                    save(ctx, job, &result, status, "site_rule").await;
                    Ok(())
                }
                Err(e) => {
                    tracing::warn!(entry_id = %job.entry_id, error = %e, "JSON extraction failed");
                    Err(classify(format!("http {status}")))
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
                    return Err(classify(format!("http {status}")));
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
                    if should_try_render(
                        result.inner.text_content.len(),
                        site_config
                            .map(|sc| sc.render.mode)
                            .unwrap_or(RenderMode::Never),
                    ) && let Some(sc) = site_config
                        && try_render_then_extract(ctx, job, sc, status).await
                    {
                        return Ok(());
                    }
                    save(ctx, job, &result.inner, status, method).await;
                    Ok(())
                }
                Err(_) => {
                    metrics::histogram!("extract_duration_seconds", "method" => "error")
                        .record(elapsed);
                    tracing::warn!(entry_id = %job.entry_id, status, "all HTML extraction methods failed");
                    if fallback_render(ctx, job, site_config, status).await {
                        Ok(())
                    } else {
                        Err(classify(format!("http {status}")))
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
    if let Some(sc) = site_config
        && let Some(html) = sc.response.html.as_ref()
    {
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
    // DB site_rules fallback (preserves Plan 3a behavior).
    if let Some(ref domain) = entry::extract_domain(&job.url)
        && let Ok(Some(rule)) =
            crate::models::site_rule::find_by_domain(&ctx.pool, job.user_id, domain).await
    {
        return Some(extract::SiteRuleConfig {
            content_selector: Some(rule.content_selector),
            title_selector: rule.title_selector,
            strip_selectors: rule.strip_selectors,
        });
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
    if let Err(e) = entry::update_entry_content(
        &ctx.pool,
        job.entry_id,
        &entry::ExtractedContent {
            title: result.title.clone(),
            content: Some(result.content.clone()),
            text_content: Some(result.text_content.clone()),
            language: result.language.clone(),
            preview_picture: result.preview_image.clone(),
            published_by: result.author.clone(),
            reading_time: Some(result.reading_time as i32),
            http_status: status,
            extract_method: method.to_string(),
        },
    )
    .await
    {
        tracing::warn!(entry_id = %job.entry_id, "failed to update entry content: {e}");
    }

    // Queue async image processing job
    if let Err(e) =
        crate::models::image_process_job::create(&ctx.pool, job.entry_id, &result.content).await
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

    apply_tagging_rules(&ctx.pool, &ctx.caches, job.user_id, job.entry_id, &job.url, result).await;

    tracing::debug!(
        entry_id = %job.entry_id,
        method,
        reading_time = result.reading_time,
        "fetch job completed"
    );
}

async fn apply_tagging_rules(
    pool: &PgPool,
    caches: &crate::cache::Caches,
    user_id: Uuid,
    entry_id: Uuid,
    url: &str,
    result: &ExtractResult,
) {
    // Use cached version - this is called on every fetch
    let rules = match crate::models::tagging_rule::list_rules_cached(pool, caches, user_id).await {
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
                    crate::models::tag::find_or_create_tag(pool, caches, user_id, tag_label).await
                    && let Err(e) =
                        crate::models::tag::add_tag_to_entry(pool, caches, user_id, entry_id, tag.id).await
                {
                    tracing::warn!(entry_id = %entry_id, tag_id = %tag.id, "failed to apply auto-tag: {e}");
                }
            }
        }
    }
}

/// Mark an entry as failed (called by the worker on `FetchError::Permanent`
/// and when retries are exhausted). The pipeline itself no longer invokes
/// this — it returns `FetchError` instead and lets the worker decide.
pub(crate) async fn mark_failed(pool: &PgPool, entry_id: Uuid, status: i16) {
    if let Err(e) = entry::update_entry_content(
        pool,
        entry_id,
        &entry::ExtractedContent {
            http_status: status,
            extract_method: "failed".to_string(),
            ..Default::default()
        },
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
    let timeout_override = sc.render.timeout_ms.map(std::time::Duration::from_millis);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_content_auto_mode_triggers_render() {
        assert!(should_try_render(50, RenderMode::Auto));
    }

    #[test]
    fn short_content_never_mode_skips_render() {
        assert!(!should_try_render(50, RenderMode::Never));
    }

    #[test]
    fn long_content_skips_render() {
        assert!(!should_try_render(500, RenderMode::Auto));
    }

    #[test]
    fn threshold_boundary_below() {
        assert!(should_try_render(99, RenderMode::Auto));
    }

    #[test]
    fn threshold_boundary_at() {
        assert!(!should_try_render(100, RenderMode::Auto));
    }

    #[test]
    fn force_mode_with_short_content() {
        assert!(should_try_render(50, RenderMode::Force));
    }

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
}
