use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::state::AppState;
use crate::models::{annotation, entry};

use super::validate::ValidatedJson;

pub async fn list_annotations(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<Vec<annotation::Annotation>>, ApiError> {
    entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;
    let annotations = annotation::list_by_entry(&state.pool, entry_id, auth.user_id).await?;
    Ok(Json(annotations))
}

pub async fn create_annotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
    ValidatedJson(params): ValidatedJson<annotation::CreateAnnotation>,
) -> Result<Json<annotation::Annotation>, ApiError> {
    entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;
    let ann = annotation::create(&state.pool, entry_id, auth.user_id, &params).await?;
    Ok(Json(ann))
}

pub async fn update_annotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(annotation_id): Path<Uuid>,
    Json(params): Json<annotation::UpdateAnnotation>,
) -> Result<Json<annotation::Annotation>, ApiError> {
    let updated = annotation::update(&state.pool, annotation_id, auth.user_id, &params).await?;
    Ok(Json(updated))
}

pub async fn delete_annotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(annotation_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = annotation::delete(&state.pool, annotation_id, auth.user_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("annotation not found".to_string()));
    }
    Ok(Json(serde_json::json!({"message": "deleted"})))
}
