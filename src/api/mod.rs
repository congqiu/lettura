use axum::{
    http::{HeaderValue, Method},
    routing::{delete, get, patch, post},
    Router,
};
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

use crate::state::AppState;
use crate::config::Config;
use crate::rate_limit::{rate_limit_middleware, GlobalRateLimit};
use crate::search::SearchIndex;
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
pub mod health;
pub mod import;
pub mod memos;
pub mod site_rules;
pub mod skills;
pub mod tagging_rules;
pub mod tags;
pub mod tokens;
pub mod pages;
pub mod pages_public;
pub mod validate;

pub fn router(pool: PgPool, config: Config) -> Router {
    router_with_search(pool, config, None).0
}

/// Build router and return handles to internal components for metrics/monitoring.
pub fn router_with_handles(pool: PgPool, config: Config) -> (Router, SearchIndex, fetcher::FetchQueue, std::sync::Arc<dyn crate::storage::ImageStorage>) {
    let (router, search, fq, storage) = router_with_search(pool, config, None);
    (router, search, fq, storage)
}

/// Redirect handler for legacy `/api/{path}` routes.
/// Returns 301 Moved Permanently to `/api/v1/{path}`, preserving query string.
async fn api_redirect(
    axum::extract::Path(path): axum::extract::Path<String>,
    req: axum::extract::Request,
) -> impl axum::response::IntoResponse {
    let query = req.uri().query().map(|q| format!("?{q}")).unwrap_or_default();
    let location = format!("/api/v1/{path}{query}");
    (
        axum::http::StatusCode::MOVED_PERMANENTLY,
        [(axum::http::header::LOCATION, location)],
    )
}

pub fn router_with_search(pool: PgPool, config: Config, search: Option<SearchIndex>) -> (Router, SearchIndex, fetcher::FetchQueue, std::sync::Arc<dyn crate::storage::ImageStorage>) {
    let search_index = search.unwrap_or_else(|| {
        SearchIndex::open(std::path::Path::new(&config.index_path))
            .expect("failed to open search index")
    });
    let storage: std::sync::Arc<dyn crate::storage::ImageStorage> = std::sync::Arc::from(crate::storage::create_storage(&config));
    crate::site_config::store::init_store(config.site_configs_path.clone());
    let fetch_queue = fetcher::start_fetch_worker(pool.clone(), 5, storage.clone(), search_index.clone(), &config);

    let search_clone = search_index.clone();
    let fq_clone = fetch_queue.clone();
    let storage_clone = storage.clone();

    let state = AppState {
        pool,
        config: config.clone(),
        fetch_queue,
        search_index,
        storage,
    };

    // Auth routes with strict rate limiting (10 req/min for brute-force protection)
    let auth_public = Router::new()
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .with_state(state.clone())
        .layer(axum::middleware::from_fn_with_state(
            GlobalRateLimit::new(config.auth_rate_limit),
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
            page_router.layer(
                SetResponseHeaderLayer::overriding(
                    axum::http::header::HeaderName::from_static("x-frame-options"),
                    HeaderValue::from_static("SAMEORIGIN"),
                )
            )
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
        // Content-Security-Policy
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; font-src 'self' https://fonts.gstatic.com; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
            ),
        ))
        // Permissions-Policy
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"),
        ))
        // Request tracing: adds request_id to spans and logs
        .layer(TraceLayer::new_for_http().make_span_with(
            DefaultMakeSpan::new()
                .level(tracing::Level::INFO)
                .include_headers(true),
        ))
        // Request ID middleware: propagates or generates X-Request-Id
        .layer(axum::middleware::from_fn(
            crate::middleware::request_id_layer,
        ))
        // Global rate limiting: 100 requests per minute.
        // Applied as the outermost layer so rate-limited requests are rejected
        // early without consuming downstream resources.
        .layer(axum::middleware::from_fn_with_state(
            GlobalRateLimit::new(config.global_rate_limit),
            rate_limit_middleware,
        ));

    (router, search_clone, fq_clone, storage_clone)
}

async fn serve_storage(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> impl axum::response::IntoResponse {
    // Prevent path traversal: reject any segment that escapes the base
    // directory or anchors to root. `path.contains("..")` would also reject
    // legitimate filenames like `foo..bar.png`, so check Components instead.
    let candidate = std::path::Path::new(&path);
    for c in candidate.components() {
        match c {
            std::path::Component::Normal(_) => {}
            _ => {
                return (axum::http::StatusCode::FORBIDDEN, "invalid path").into_response();
            }
        }
    }
    let file_path = std::path::Path::new(&state.config.storage_local_path).join(&path);
    match tokio::fs::read(&file_path).await {
        Ok(data) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, mime.as_ref().to_string()),
                 (axum::http::header::CACHE_CONTROL, "public, max-age=31536000".to_string())],
                data,
            ).into_response()
        }
        Err(_) => (axum::http::StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

use axum::extract::{Path, State};
use axum::response::IntoResponse;
