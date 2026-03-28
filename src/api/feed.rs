use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};

use crate::api::error::ApiError;
use crate::auth::middleware::AppState;

pub async fn feed_unread(
    State(state): State<AppState>,
    Path(user_token): Path<String>,
) -> Result<Response, ApiError> {
    let user = find_user_by_feed_token(&state, &user_token).await?;
    let entries: Vec<FeedEntry> = sqlx::query_as(
        "SELECT id, url, title, content, created_at FROM entries WHERE user_id = $1 AND is_archived = false AND deleted_at IS NULL ORDER BY created_at DESC LIMIT 50",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(build_rss("Lettura - Unread", &entries))
}

pub async fn feed_starred(
    State(state): State<AppState>,
    Path(user_token): Path<String>,
) -> Result<Response, ApiError> {
    let user = find_user_by_feed_token(&state, &user_token).await?;
    let entries: Vec<FeedEntry> = sqlx::query_as(
        "SELECT id, url, title, content, created_at FROM entries WHERE user_id = $1 AND is_starred = true AND deleted_at IS NULL ORDER BY starred_at DESC LIMIT 50",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(build_rss("Lettura - Starred", &entries))
}

pub async fn feed_archive(
    State(state): State<AppState>,
    Path(user_token): Path<String>,
) -> Result<Response, ApiError> {
    let user = find_user_by_feed_token(&state, &user_token).await?;
    let entries: Vec<FeedEntry> = sqlx::query_as(
        "SELECT id, url, title, content, created_at FROM entries WHERE user_id = $1 AND is_archived = true AND deleted_at IS NULL ORDER BY archived_at DESC LIMIT 50",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(build_rss("Lettura - Archive", &entries))
}

#[derive(sqlx::FromRow)]
struct FeedEntry {
    id: uuid::Uuid,
    url: String,
    title: Option<String>,
    content: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

async fn find_user_by_feed_token(
    state: &AppState,
    token: &str,
) -> Result<crate::models::user::User, ApiError> {
    sqlx::query_as::<_, crate::models::user::User>(
        "SELECT * FROM users WHERE feed_token = $1",
    )
    .bind(token)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("invalid feed token".to_string()))
}

fn build_rss(channel_title: &str, entries: &[FeedEntry]) -> Response {
    let mut items = String::new();
    for entry in entries {
        let title = entry.title.as_deref().unwrap_or("Untitled");
        let content = entry.content.as_deref().unwrap_or("");
        let date = entry.created_at.to_rfc2822();
        items.push_str(&format!(
            "<item><title><![CDATA[{}]]></title><link>{}</link><guid>{}</guid><pubDate>{}</pubDate><description><![CDATA[{}]]></description></item>",
            title, entry.url, entry.id, date, content
        ));
    }

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><rss version="2.0"><channel><title>{}</title><description>Lettura RSS Feed</description>{}</channel></rss>"#,
        channel_title, items
    );

    (
        [
            (header::CONTENT_TYPE, "application/rss+xml; charset=utf-8"),
            (header::REFERRER_POLICY, "no-referrer"),
        ],
        xml,
    )
        .into_response()
}
