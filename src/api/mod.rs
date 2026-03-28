use axum::{
    http::HeaderValue,
    routing::{delete, get, patch, post},
    Router,
};
use sqlx::PgPool;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::auth::middleware::AppState;
use crate::config::Config;
use crate::search::SearchIndex;
use crate::tasks::fetcher;

pub mod admin;
pub mod annotations;
pub mod auth;
pub mod entries;
pub mod error;
pub mod export;
pub mod feed;
pub mod health;
pub mod import;
pub mod memos;
pub mod site_rules;
pub mod tagging_rules;
pub mod tags;

pub fn router(pool: PgPool, config: Config) -> Router {
    router_with_search(pool, config, None)
}

pub fn router_with_search(pool: PgPool, config: Config, search: Option<SearchIndex>) -> Router {
    let search_index = search.unwrap_or_else(|| {
        SearchIndex::open(std::path::Path::new(&config.index_path))
            .expect("failed to open search index")
    });
    let storage: std::sync::Arc<dyn crate::storage::ImageStorage> = std::sync::Arc::from(crate::storage::create_storage(&config));
    let fetch_queue = fetcher::start_fetch_worker(pool.clone(), 5, storage.clone());

    let state = AppState {
        pool,
        config: config.clone(),
        fetch_queue,
        search_index,
        storage,
    };

    Router::new()
        // Health (no auth required)
        .route("/api/health", get(health::health_check))
        // Auth
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/refresh", post(auth::refresh))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/regenerate-feed-token", post(auth::regenerate_feed_token))
        // Entries
        .route(
            "/api/entries",
            get(entries::list_entries).post(entries::create_entry),
        )
        .route(
            "/api/entries/{id}",
            get(entries::get_entry)
                .patch(entries::update_entry)
                .delete(entries::delete_entry),
        )
        .route("/api/entries/{id}/refetch", post(entries::refetch_entry))
        // Tags
        .route("/api/tags", get(tags::list_tags))
        .route("/api/entries/{id}/tags", post(tags::add_tag_to_entry))
        .route(
            "/api/entries/{entry_id}/tags/{tag_id}",
            delete(tags::remove_tag_from_entry),
        )
        .route("/api/tags/{id}", delete(tags::delete_tag))
        // Annotations
        .route(
            "/api/entries/{id}/annotations",
            get(annotations::list_annotations).post(annotations::create_annotation),
        )
        .route(
            "/api/annotations/{id}",
            patch(annotations::update_annotation).delete(annotations::delete_annotation),
        )
        // Memos
        .route(
            "/api/memos",
            get(memos::list_memos).post(memos::create_memo),
        )
        .route("/api/memos/{id}", delete(memos::delete_memo))
        .route("/api/memos/{id}/promote", post(memos::promote_memo))
        // Tagging Rules
        .route(
            "/api/tagging-rules",
            get(tagging_rules::list_rules).post(tagging_rules::create_rule),
        )
        .route(
            "/api/tagging-rules/{id}",
            patch(tagging_rules::update_rule).delete(tagging_rules::delete_rule),
        )
        // Site Rules
        .route(
            "/api/site-rules",
            get(site_rules::list_rules).post(site_rules::create_rule),
        )
        .route(
            "/api/site-rules/{id}",
            patch(site_rules::update_rule).delete(site_rules::delete_rule),
        )
        // Import/Export
        .route("/api/import/wallabag", post(import::import_wallabag))
        .route("/api/import/browser", post(import::import_browser))
        .route("/api/export", get(export::export_all))
        // RSS Feeds (no auth - uses feed token)
        .route("/feed/{user_token}/unread", get(feed::feed_unread))
        .route("/feed/{user_token}/starred", get(feed::feed_starred))
        .route("/feed/{user_token}/archive", get(feed::feed_archive))
        // Admin
        .route("/api/admin/users", get(admin::list_users))
        .route("/api/admin/reindex", post(admin::reindex))
        // Local storage file serving
        .route("/storage/{*path}", get(serve_storage))
        // SPA fallback
        .fallback(crate::spa::spa_handler)
        .with_state(state)
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
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
}

async fn serve_storage(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> impl axum::response::IntoResponse {
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
