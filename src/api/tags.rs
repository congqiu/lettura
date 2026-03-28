use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::{AppState, AuthUser};
use crate::models::{entry, tag};

pub async fn list_tags(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<tag::Tag>>, ApiError> {
    let tags = tag::list_tags(&state.pool, auth.user_id).await?;
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
    Ok(Json(serde_json::json!({"message": "deleted"})))
}
