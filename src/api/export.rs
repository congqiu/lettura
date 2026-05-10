use axum::Json;
use axum::extract::State;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::state::AppState;

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

    let tags: Vec<crate::models::tag::Tag> =
        sqlx::query_as("SELECT * FROM tags WHERE user_id = $1")
            .bind(auth.user_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::ExportAll,
        Some(AuditResourceType::System),
        None,
        serde_json::json!({"entries": entries.len(), "tags": tags.len()}),
    )
    .await;

    Ok(Json(serde_json::json!({
        "entries": entries,
        "tags": tags,
        "exported_at": chrono::Utc::now().to_rfc3339(),
    })))
}
