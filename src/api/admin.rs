use axum::extract::State;
use axum::Json;

use crate::api::error::ApiError;
use crate::auth::middleware::{AppState, AuthUser};
use crate::models::user::User;

pub async fn list_users(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<UserSummary>>, ApiError> {
    if !auth.is_admin {
        return Err(ApiError::Forbidden("admin required".to_string()));
    }

    let users: Vec<User> = sqlx::query_as("SELECT * FROM users ORDER BY created_at")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let summaries: Vec<UserSummary> = users
        .into_iter()
        .map(|u| UserSummary {
            id: u.id,
            username: u.username,
            email: u.email,
            is_admin: u.is_admin,
            created_at: u.created_at,
        })
        .collect();

    Ok(Json(summaries))
}

#[derive(serde::Serialize)]
pub struct UserSummary {
    pub id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub is_admin: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn reindex(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !auth.is_admin {
        return Err(ApiError::Forbidden("admin required".to_string()));
    }

    // Clear and rebuild index
    state
        .search_index
        .clear()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let entries: Vec<(uuid::Uuid, Option<String>, Option<String>, String, Option<String>)> =
        sqlx::query_as(
            "SELECT id, title, text_content, url, domain_name FROM entries",
        )
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let count = entries.len();
    for (id, title, text_content, url, domain) in entries {
        state
            .search_index
            .upsert(
                id,
                title.as_deref().unwrap_or(""),
                text_content.as_deref().unwrap_or(""),
                &url,
                domain.as_deref().unwrap_or(""),
            )
            .await
            .ok();
    }

    Ok(Json(serde_json::json!({
        "message": "reindex complete",
        "indexed": count
    })))
}
