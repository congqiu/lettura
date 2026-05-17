use axum::{
    Router,
    http::{HeaderValue, Method},
    routing::{delete, get, patch, post},
};
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

use crate::config::Config;
use crate::rate_limit::{GlobalRateLimit, rate_limit_middleware};
use crate::search::SearchIndex;
use crate::state::AppState;
use crate::tasks::fetcher;

/// Derive the auth source string from the authenticated user.
/// Centralized here to avoid duplication across handler files.
pub fn auth_source_str(auth: &crate::auth::middleware::AuthUser) -> String {
    match auth.source {
        crate::auth::middleware::AuthSource::Jwt => "jwt".to_string(),
        crate::auth::middleware::AuthSource::Pat { .. } => "pat".to_string(),
    }
}

pub mod admin;
pub mod annotations;
pub mod audit_logs;
pub mod auth;
pub mod backup;
pub mod bulk;
pub mod entries;
pub mod error;
pub mod export;
pub mod feed;
pub mod fetch_jobs;
pub mod health;
pub mod import;
pub mod memos;
pub mod pages;
pub mod pages_public;
pub mod site_rules;
pub mod skills;
pub mod tagging_rules;
pub mod tags;
pub mod tokens;
pub mod validate;

pub fn router(pool: PgPool, config: Config) -> Router {
    router_with_search(pool, config, None).0
}

/// Build router and return handles to internal components for metrics/monitoring.
/// `caches` is returned so the binary can share the same instance with spawned
/// workers (cache invalidations from background tagging must reach handlers).
pub fn router_with_handles(
    pool: PgPool,
    config: Config,
) -> (
    Router,
    SearchIndex,
    fetcher::FetchQueue,
    std::sync::Arc<dyn crate::storage::ImageStorage>,
    std::sync::Arc<crate::cache::Caches>,
) {
    let (router, search, fq, storage, caches) = router_with_search(pool, config, None);
    (router, search, fq, storage, caches)
}

/// Redirect handler for legacy `/api/{path}` routes.
/// Returns 301 Moved Permanently to `/api/v1/{path}`, preserving query string.
async fn api_redirect(
    axum::extract::Path(path): axum::extract::Path<String>,
    req: axum::extract::Request,
) -> impl axum::response::IntoResponse {
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let location = format!("/api/v1/{path}{query}");
    (
        axum::http::StatusCode::MOVED_PERMANENTLY,
        [(axum::http::header::LOCATION, location)],
    )
}

pub fn router_with_search(
    pool: PgPool,
    config: Config,
    search: Option<SearchIndex>,
) -> (
    Router,
    SearchIndex,
    fetcher::FetchQueue,
    std::sync::Arc<dyn crate::storage::ImageStorage>,
    std::sync::Arc<crate::cache::Caches>,
) {
    let search_index = search.unwrap_or_else(|| {
        SearchIndex::open(std::path::Path::new(&config.index_path))
            .expect("failed to open search index")
    });
    let storage: std::sync::Arc<dyn crate::storage::ImageStorage> =
        std::sync::Arc::from(crate::storage::create_storage(&config));
    crate::site_config::store::init_store(config.site_configs_path.clone());
    // Worker lifecycle is owned by main.rs (or by tests that need a real
    // worker). The router only enqueues jobs via FetchQueue::new(pool).
    let fetch_queue = fetcher::FetchQueue::new(pool.clone());

    // Caches are per-instance (no global static). Owned by AppState so handler
    // code reaches them via &state.caches; the same Arc is also shared with
    // any spawned worker via WorkerConfig so cache invalidations from
    // background tagging apply consistently.
    let caches = std::sync::Arc::new(crate::cache::Caches::new());

    let search_clone = search_index.clone();
    let fq_clone = fetch_queue.clone();
    let storage_clone = storage.clone();
    let caches_clone = caches.clone();

    let state = AppState {
        pool,
        config: config.clone(),
        fetch_queue,
        search_index,
        storage,
        caches,
    };

    // Auth routes with strict rate limiting (10 req/min for brute-force protection)
    let auth_public = Router::new()
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .with_state(state.clone())
        .layer(axum::middleware::from_fn_with_state(
            GlobalRateLimit::new(config.auth_rate_limit).with_trust_proxy(config.trust_proxy),
            rate_limit_middleware,
        ));

    let router = Router::new()
        // Health (no auth required, no version prefix)
        .route("/api/health", get(health::health_check))
        // Skills (no auth required)
        .route("/skills/lettura.md", get(skills::skill_lettura))
        // Auth (other endpoints — normal rate limit)
        .route("/api/v1/auth/refresh", post(auth::refresh))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/regenerate-feed-token", post(auth::regenerate_feed_token))
        .route("/api/v1/auth/change-password", post(auth::change_password))
        .route("/api/v1/auth/me", get(auth::me))
        // Tokens (PAT management — requires JWT)
        .route("/api/v1/tokens", get(tokens::list_tokens).post(tokens::create_token))
        .route("/api/v1/tokens/{id}", delete(tokens::delete_token))
        // Merge auth public routes with strict rate limit
        .merge(auth_public)
        // Audit logs
        .route("/api/v1/audit-logs", get(audit_logs::list_audit_logs))
        // Entries
        .route(
            "/api/v1/entries",
            get(entries::list_entries).post(entries::create_entry),
        )
        .route(
            "/api/v1/entries/{id}",
            get(entries::get_entry)
                .patch(entries::update_entry)
                .delete(entries::delete_entry),
        )
        .route("/api/v1/entries/{id}/refetch", post(entries::refetch_entry))
        .route("/api/v1/entries/{id}/restore", post(entries::restore_entry))
        .route("/api/v1/entries/{id}/permanent", delete(entries::permanently_delete_entry))
        // Bulk operations
        .route("/api/v1/entries/bulk/tag", post(bulk::bulk_tag_add))
        .route("/api/v1/entries/bulk/untag", post(bulk::bulk_untag))
        .route("/api/v1/entries/bulk/archive", post(bulk::bulk_archive))
        .route("/api/v1/entries/bulk/star", post(bulk::bulk_star))
        .route("/api/v1/entries/bulk/tag-by-ids", post(bulk::bulk_tag_by_ids))
        .route("/api/v1/entries/bulk/untag-by-ids", post(bulk::bulk_untag_by_ids))
        .route("/api/v1/entries/bulk/delete-by-ids", post(bulk::bulk_delete_by_ids))
        .route("/api/v1/entries/bulk/archive-by-ids", post(bulk::bulk_archive_by_ids))
        // Tags
        .route("/api/v1/tags", get(tags::list_tags))
        .route("/api/v1/tags/stats", get(tags::tags_stats))
        .route("/api/v1/entries/{id}/tags", get(tags::list_tags_for_entry))
        .route("/api/v1/entries/{id}/tags", post(tags::add_tag_to_entry))
        .route(
            "/api/v1/entries/{entry_id}/tags/{tag_id}",
            delete(tags::remove_tag_from_entry),
        )
        .route(
            "/api/v1/entries/{entry_id}/tags/by-label/{label}",
            delete(tags::remove_tag_from_entry_by_label),
        )
        .route("/api/v1/tags/{id}", delete(tags::delete_tag).patch(tags::rename_tag_handler))
        // Annotations
        .route(
            "/api/v1/entries/{id}/annotations",
            get(annotations::list_annotations).post(annotations::create_annotation),
        )
        .route(
            "/api/v1/annotations/{id}",
            patch(annotations::update_annotation).delete(annotations::delete_annotation),
        )
        // Memos
        .route(
            "/api/v1/memos",
            get(memos::list_memos).post(memos::create_memo),
        )
        .route("/api/v1/memos/{id}", delete(memos::delete_memo))
        .route("/api/v1/memos/{id}/promote", post(memos::promote_memo))
        // Tagging Rules
        .route(
            "/api/v1/tagging-rules",
            get(tagging_rules::list_rules).post(tagging_rules::create_rule),
        )
        .route(
            "/api/v1/tagging-rules/{id}",
            patch(tagging_rules::update_rule).delete(tagging_rules::delete_rule),
        )
        // Site Rules
        .route(
            "/api/v1/site-rules",
            get(site_rules::list_rules).post(site_rules::create_rule),
        )
        .route(
            "/api/v1/site-rules/{id}",
            patch(site_rules::update_rule).delete(site_rules::delete_rule),
        )
        // Import/Export
        .route("/api/v1/import/wallabag", post(import::import_wallabag))
        .route("/api/v1/import/browser", post(import::import_browser))
        .route("/api/v1/import/lettura", post(import::import_lettura))
        .route("/api/v1/export", get(export::export_all))
        // RSS Feeds (no auth - uses feed token)
        .route("/feed/{user_token}/unread", get(feed::feed_unread))
        .route("/feed/{user_token}/starred", get(feed::feed_starred))
        .route("/feed/{user_token}/archive", get(feed::feed_archive))
        // Admin
        .route("/api/v1/admin/users", get(admin::list_users))
        .route("/api/v1/admin/reindex", post(admin::reindex))
        .route("/api/v1/admin/backup", get(backup::backup))
        .route("/api/v1/admin/restore", post(backup::restore))
        // Admin: fetch jobs queue management (JWT-only; PATs have is_admin=false)
        // retry-all-dead must be registered BEFORE /{id} so axum's matcher
        // doesn't treat "retry-all-dead" as a UUID path param.
        .route("/api/v1/admin/fetch-jobs", get(fetch_jobs::list))
        .route("/api/v1/admin/fetch-jobs/retry-all-dead", post(fetch_jobs::retry_all_dead))
        .route("/api/v1/admin/fetch-jobs/{id}", get(fetch_jobs::get).delete(fetch_jobs::delete))
        .route("/api/v1/admin/fetch-jobs/{id}/retry", post(fetch_jobs::retry))
        .route("/api/v1/pages/upload", post(pages::upload_files))
        .route("/api/v1/pages", get(pages::list_pages_handler).post(pages::create_page_handler))
        .route("/api/v1/pages/{id}", patch(pages::update_page_handler).delete(pages::delete_page_handler))
        .route("/api/v1/pages/{id}/restore", post(pages::restore_page_handler))
        .route("/api/v1/pages/{id}/share-url", get(pages::get_share_url_handler))
        // Local storage file serving
        .route("/storage/{*path}", get(serve_storage))
        // Legacy API redirect: /api/{path} -> /api/v1/{path} (301)
        // Note: /api/health is a more specific route and takes priority over this catch-all
        .route("/api/{*path}", get(api_redirect).post(api_redirect).patch(api_redirect).delete(api_redirect))
        .nest("/p", {
            let page_router = Router::new()
                .route("/{slug}", get(pages_public::serve_page))
                .route("/{slug}/{*file}", get(pages_public::serve_page_file))
                .route("/{slug}/auth", post(pages_public::auth_page))
                .with_state(state.clone());
            // Allow same-origin framing for shared pages (overrides global frame-ancestors 'none')
            // CSP for shared pages: no scripts allowed (prevents stored XSS from user HTML),
            // inline styles allowed for readability, images from self/data/https/blob.
            page_router.layer(
                SetResponseHeaderLayer::overriding(
                    axum::http::header::HeaderName::from_static("x-frame-options"),
                    HeaderValue::from_static("SAMEORIGIN"),
                )
            )
            .layer(SetResponseHeaderLayer::overriding(
                axum::http::header::HeaderName::from_static("content-security-policy"),
                HeaderValue::from_static(
                    "default-src 'self'; script-src 'none'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; font-src 'self' https://fonts.gstatic.com; connect-src 'self'; frame-ancestors 'self'; base-uri 'self'; form-action 'self'"
                ),
            ))
        })
        .fallback(crate::spa::spa_handler)
        .with_state(state)
        // CORS
        .layer({
            let cors = CorsLayer::new()
                .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE, Method::OPTIONS])
                .allow_headers([axum::http::header::AUTHORIZATION, axum::http::header::CONTENT_TYPE])
                .expose_headers([axum::http::header::HeaderName::from_static("x-next-cursor")]);
            if config.cors_origins == "*" {
                cors.allow_origin(Any)
            } else {
                let origins: Vec<HeaderValue> = config.cors_origins
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
                if origins.is_empty() {
                    panic!("CORS_ORIGINS is set but no valid origins could be parsed. Check for typos — each entry must be a valid URL (e.g. https://example.com)");
                }
                cors.allow_origin(origins)
            }
        })
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("x-xss-protection"),
            HeaderValue::from_static("1; mode=block"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        // Content-Security-Policy (use if_not_present so /p/ routes can set their own CSP)
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; font-src 'self' https://fonts.gstatic.com; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
            ),
        ))
        // Permissions-Policy
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"),
        ));

    // HSTS (only in production — dev environments often use HTTP)
    let router = if config.production {
        router.layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=63072000; includeSubDomains; preload"),
        ))
    } else {
        router
    };

    let router = router
        // Request tracing: adds request_id to spans and logs.
        // Headers are intentionally excluded to avoid logging sensitive
        // values (Authorization, Cookie, etc.).
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO)),
        )
        // Request ID middleware: propagates or generates X-Request-Id
        .layer(axum::middleware::from_fn(
            crate::middleware::request_id_layer,
        ))
        // Global rate limiting: 100 requests per minute.
        // Applied as the outermost layer so rate-limited requests are rejected
        // early without consuming downstream resources.
        .layer(axum::middleware::from_fn_with_state(
            GlobalRateLimit::new(config.global_rate_limit).with_trust_proxy(config.trust_proxy),
            rate_limit_middleware,
        ));

    (router, search_clone, fq_clone, storage_clone, caches_clone)
}

async fn serve_storage(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> impl axum::response::IntoResponse {
    // Prevent path traversal: reject any segment that escapes the base
    // directory or anchors to root. `path.contains("..")` would also reject
    // legitimate filenames like `foo..bar.png`, so check Components instead.
    if !is_safe_storage_path(&path) {
        return (axum::http::StatusCode::FORBIDDEN, "invalid path").into_response();
    }
    let file_path = std::path::Path::new(&state.config.storage_local_path).join(&path);
    match tokio::fs::read(&file_path).await {
        Ok(data) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            let mime_str = mime.as_ref();
            let content_disposition = if is_dangerous_mime(mime_str) {
                "attachment"
            } else {
                "inline"
            };
            (
                axum::http::StatusCode::OK,
                [
                    (axum::http::header::CONTENT_TYPE, mime_str.to_string()),
                    (
                        axum::http::header::CACHE_CONTROL,
                        "public, max-age=31536000".to_string(),
                    ),
                    (
                        axum::http::header::CONTENT_DISPOSITION,
                        content_disposition.to_string(),
                    ),
                ],
                data,
            )
                .into_response()
        }
        Err(_) => (axum::http::StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

use axum::extract::{Path, State};
use axum::response::IntoResponse;

/// Check if a path component is safe for storage file serving.
/// Returns false if any component is not `Normal` (e.g., `..`, root, prefix).
/// Extracted as a pure function for testability.
fn is_safe_storage_path(path: &str) -> bool {
    let candidate = std::path::Path::new(path);
    candidate
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_)))
}

/// Determine whether a MIME type is dangerous (should be served as attachment).
fn is_dangerous_mime(mime_str: &str) -> bool {
    mime_str.starts_with("text/html")
        || mime_str.starts_with("application/javascript")
        || mime_str.starts_with("application/xhtml")
        || mime_str.starts_with("text/javascript")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::middleware::{AuthSource, AuthUser, PatScope};

    // --- auth_source_str tests ---

    #[test]
    fn auth_source_str_jwt() {
        let auth = AuthUser {
            user_id: uuid::Uuid::new_v4(),
            is_admin: false,
            source: AuthSource::Jwt,
        };
        assert_eq!(auth_source_str(&auth), "jwt");
    }

    #[test]
    fn auth_source_str_pat_read() {
        let auth = AuthUser {
            user_id: uuid::Uuid::new_v4(),
            is_admin: false,
            source: AuthSource::Pat {
                scope: PatScope::Read,
                token_id: uuid::Uuid::new_v4(),
            },
        };
        assert_eq!(auth_source_str(&auth), "pat");
    }

    #[test]
    fn auth_source_str_pat_write() {
        let auth = AuthUser {
            user_id: uuid::Uuid::new_v4(),
            is_admin: true,
            source: AuthSource::Pat {
                scope: PatScope::Write,
                token_id: uuid::Uuid::new_v4(),
            },
        };
        assert_eq!(auth_source_str(&auth), "pat");
    }

    // --- is_safe_storage_path tests ---

    #[test]
    fn safe_path_simple_filename() {
        assert!(is_safe_storage_path("image.png"));
    }

    #[test]
    fn safe_path_nested_normal() {
        assert!(is_safe_storage_path("subdir/image.png"));
    }

    #[test]
    fn safe_path_deeply_nested() {
        assert!(is_safe_storage_path("a/b/c/image.png"));
    }

    #[test]
    fn unsafe_path_parent_traversal() {
        assert!(!is_safe_storage_path("../etc/passwd"));
    }

    #[test]
    fn unsafe_path_mixed_traversal() {
        assert!(!is_safe_storage_path("images/../../etc/passwd"));
    }

    #[test]
    fn unsafe_path_absolute() {
        assert!(!is_safe_storage_path("/etc/passwd"));
    }

    #[test]
    fn unsafe_path_current_dir_prefix() {
        assert!(!is_safe_storage_path("./secret"));
    }

    #[test]
    fn safe_path_double_dot_in_filename() {
        // Filenames like `foo..bar.png` should be allowed
        assert!(is_safe_storage_path("foo..bar.png"));
    }

    // --- is_dangerous_mime tests ---

    #[test]
    fn dangerous_html() {
        assert!(is_dangerous_mime("text/html"));
        assert!(is_dangerous_mime("text/html; charset=utf-8"));
    }

    #[test]
    fn dangerous_javascript() {
        assert!(is_dangerous_mime("application/javascript"));
        assert!(is_dangerous_mime("text/javascript"));
    }

    #[test]
    fn dangerous_xhtml() {
        assert!(is_dangerous_mime("application/xhtml+xml"));
    }

    #[test]
    fn safe_image_mime() {
        assert!(!is_dangerous_mime("image/png"));
        assert!(!is_dangerous_mime("image/jpeg"));
    }

    #[test]
    fn safe_json_mime() {
        assert!(!is_dangerous_mime("application/json"));
    }

    #[test]
    fn safe_plain_text() {
        assert!(!is_dangerous_mime("text/plain"));
    }
}
