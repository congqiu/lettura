use axum::extract::State;
use axum::Json;

use crate::api::error::ApiError;
use crate::auth::middleware::{AuthSource, AuthUser};
use crate::state::AppState;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};

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

    let auth_source = match auth.source {
        AuthSource::Jwt => "jwt".to_string(),
        AuthSource::Pat { .. } => "pat".to_string(),
    };
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source,
            action: AuditAction::ExportAll,
            resource_type: Some(AuditResourceType::System),
            resource_id: None,
            status: "success".to_string(),
            details: serde_json::json!({"entries": entries.len(), "tags": tags.len()}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;

    Ok(Json(serde_json::json!({
        "entries": entries,
        "tags": tags,
        "exported_at": chrono::Utc::now().to_rfc3339(),
    })))
}
