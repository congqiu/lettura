use axum::extract::State;
use axum::Json;

use crate::api::error::ApiError;
use crate::auth::middleware::{AppState, AuthUser};

pub async fn export_all(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let entries: Vec<crate::models::entry::Entry> = sqlx::query_as(
        "SELECT * FROM entries WHERE user_id = $1 AND deleted_at IS NULL ORDER BY created_at",
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let tags: Vec<crate::models::tag::Tag> = sqlx::query_as(
        "SELECT * FROM tags WHERE user_id = $1",
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "entries": entries,
        "tags": tags,
        "exported_at": chrono::Utc::now().to_rfc3339(),
    })))
}
