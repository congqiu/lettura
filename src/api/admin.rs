use axum::extract::State;
use axum::Json;

use crate::api::error::ApiError;
use crate::auth::middleware::{AuthSource, AuthUser};
use crate::state::AppState;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::user::User;

fn auth_source_str(auth: &AuthUser) -> String {
    match auth.source {
        AuthSource::Jwt => "jwt".to_string(),
        AuthSource::Pat { .. } => "pat".to_string(),
    }
}

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

    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::AdminListUsers,
            resource_type: Some(AuditResourceType::System),
            resource_id: None,
            status: "success".to_string(),
            details: serde_json::json!({"count": summaries.len()}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;

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

    // Clear and rebuild index. Commit the clear immediately so a panic during
    // the upsert phase below cannot leave the index in a half-cleared state.
    state
        .search_index
        .clear()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    state
        .search_index
        .commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let entries: Vec<(uuid::Uuid, uuid::Uuid, Option<String>, Option<String>, String, Option<String>)> =
        sqlx::query_as(
            "SELECT id, user_id, title, text_content, url, domain_name FROM entries WHERE deleted_at IS NULL",
        )
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let count = entries.len();
    for (id, user_id, title, text_content, url, domain) in entries {
        state
            .search_index
            .upsert(
                id,
                user_id,
                title.as_deref().unwrap_or(""),
                text_content.as_deref().unwrap_or(""),
                &url,
                domain.as_deref().unwrap_or(""),
            )
            .await
            .ok();
    }

    // Flush the bulk changes immediately so they are searchable.
    state.search_index.commit().await.map_err(|e| ApiError::Internal(e.to_string()))?;

    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::AdminReindex,
            resource_type: Some(AuditResourceType::System),
            resource_id: None,
            status: "success".to_string(),
            details: serde_json::json!({"indexed": count}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;

    Ok(Json(serde_json::json!({
        "message": "reindex complete",
        "indexed": count
    })))
}
