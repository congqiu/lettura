use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::api::validate::ValidatedJson;
use crate::auth::middleware::AuthUser;
use crate::state::AppState;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::{entry, tag};

pub async fn list_tags(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<tag::Tag>>, ApiError> {
    let tags = tag::list_tags_cached(&state.pool, auth.user_id).await?;
    Ok(Json(tags))
}

pub async fn tags_stats(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<tag::TagStats>>, ApiError> {
    let stats = tag::TagStats::list_cached(&state.pool, auth.user_id).await?;
    Ok(Json(stats))
}

#[derive(Deserialize, Validate)]
pub struct RenameTagRequest {
    #[validate(length(min = 1, max = 100, message = "label must be 1-100 characters"))]
    pub label: String,
}

pub async fn rename_tag_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(tag_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<RenameTagRequest>,
) -> Result<Json<tag::Tag>, ApiError> {
    let updated = tag::rename_tag(&state.pool, tag_id, auth.user_id, &req.label)
        .await
        .map_err(|e| match e {
            tag::RenameError::Conflict => ApiError::Conflict("a tag with this name already exists".to_string()),
            tag::RenameError::Database(msg) => {
                tracing::error!("rename_tag database error: {msg}");
                ApiError::Internal("internal server error".to_string())
            }
        })?;

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::RenameTag,
        Some(AuditResourceType::Tag),
        Some(tag_id),
        serde_json::json!({"new_label": req.label}),
    ).await;

    Ok(Json(updated))
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

#[derive(Deserialize, Validate)]
pub struct AddTagRequest {
    #[validate(length(min = 1, max = 100, message = "label must be 1-100 characters"))]
    pub label: String,
}

pub async fn add_tag_to_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<AddTagRequest>,
) -> Result<Json<tag::Tag>, ApiError> {
    entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;
    let t = tag::find_or_create_tag(&state.pool, auth.user_id, &req.label).await?;
    tag::add_tag_to_entry(&state.pool, auth.user_id, entry_id, t.id).await?;

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::AddTagToEntry,
        Some(AuditResourceType::Entry),
        Some(entry_id),
        serde_json::to_value(serde_json::json!({"tag_label": req.label, "tag_id": t.id})).unwrap_or_default(),
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
    tag::remove_tag_from_entry(&state.pool, auth.user_id, entry_id, tag_id).await?;

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::RemoveTagFromEntry,
        Some(AuditResourceType::Entry),
        Some(entry_id),
        serde_json::to_value(serde_json::json!({"tag_id": tag_id})).unwrap_or_default(),
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
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::DeleteTag,
        Some(AuditResourceType::Tag),
        Some(tag_id),
        serde_json::json!({}),
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

    if tag::remove_tag_from_entry(&state.pool, auth.user_id, entry_id, tag_id).await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound(format!("tag '{label}' not on entry")))
    }
}