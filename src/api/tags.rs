use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::{AuthSource, AuthUser};
use crate::state::AppState;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::{entry, tag};

fn auth_source_str(auth: &AuthUser) -> String {
    match auth.source {
        AuthSource::Jwt => "jwt".to_string(),
        AuthSource::Pat { .. } => "pat".to_string(),
    }
}

pub async fn list_tags(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<tag::Tag>>, ApiError> {
    // Use cached version for list endpoint
    let tags = tag::list_tags_cached(&state.pool, auth.user_id).await?;
    Ok(Json(tags))
}

pub async fn list_tags_for_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<Vec<tag::Tag>>, ApiError> {
    entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;
    let tags = tag::list_tags_for_entry(&state.pool, entry_id).await?;
    Ok(Json(tags))
}

#[derive(Deserialize)]
pub struct AddTagRequest {
    pub label: String,
}

pub async fn add_tag_to_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
    Json(req): Json<AddTagRequest>,
) -> Result<Json<tag::Tag>, ApiError> {
    entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;
    let t = tag::find_or_create_tag(&state.pool, auth.user_id, &req.label).await?;
    tag::add_tag_to_entry(&state.pool, entry_id, t.id).await?;

    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::AddTagToEntry,
            resource_type: Some(AuditResourceType::Entry),
            resource_id: Some(entry_id),
            status: "success".to_string(),
            details: serde_json::to_value(serde_json::json!({"tag_label": req.label, "tag_id": t.id})).unwrap_or_default(),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;

    Ok(Json(t))
}

pub async fn remove_tag_from_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((entry_id, tag_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;
    tag::remove_tag_from_entry(&state.pool, entry_id, tag_id).await?;

    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::RemoveTagFromEntry,
            resource_type: Some(AuditResourceType::Entry),
            resource_id: Some(entry_id),
            status: "success".to_string(),
            details: serde_json::to_value(serde_json::json!({"tag_id": tag_id})).unwrap_or_default(),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;

    Ok(Json(serde_json::json!({"message": "removed"})))
}

pub async fn delete_tag(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(tag_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = tag::delete_tag(&state.pool, auth.user_id, tag_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("tag not found".to_string()));
    }
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::DeleteTag,
            resource_type: Some(AuditResourceType::Tag),
            resource_id: Some(tag_id),
            status: "success".to_string(),
            details: serde_json::json!({}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;
    Ok(Json(serde_json::json!({"message": "deleted"})))
}

pub async fn remove_tag_from_entry_by_label(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((entry_id, label)): Path<(Uuid, String)>,
) -> Result<StatusCode, ApiError> {
    let (tag_id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM tags WHERE user_id = $1 AND label = $2"
    )
    .bind(auth.user_id)
    .bind(&label)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound(format!("tag with label '{label}'")))?;

    if tag::remove_tag_from_entry(&state.pool, entry_id, tag_id).await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound(format!("tag '{label}' not on entry")))
    }
}
