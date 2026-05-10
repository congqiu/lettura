use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::{annotation, entry};
use crate::state::AppState;

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
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::CreateAnnotation,
        Some(AuditResourceType::Annotation),
        Some(ann.id),
        serde_json::json!({"entry_id": entry_id}),
    )
    .await;
    Ok(Json(ann))
}

pub async fn update_annotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(annotation_id): Path<Uuid>,
    Json(params): Json<annotation::UpdateAnnotation>,
) -> Result<Json<annotation::Annotation>, ApiError> {
    let updated = annotation::update(&state.pool, annotation_id, auth.user_id, &params).await?;
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::UpdateAnnotation,
        Some(AuditResourceType::Annotation),
        Some(annotation_id),
        serde_json::json!({}),
    )
    .await;
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
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::DeleteAnnotation,
        Some(AuditResourceType::Annotation),
        Some(annotation_id),
        serde_json::json!({}),
    )
    .await;
    Ok(Json(serde_json::json!({"message": "deleted"})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn create_annotation_request_validation() {
        // Empty quote should fail validation (min length = 1)
        let params = annotation::CreateAnnotation {
            quote: "".to_string(),
            text: None,
            ranges: serde_json::json!({}),
        };
        assert!(params.validate().is_err());

        // Non-empty quote should pass validation
        let params = annotation::CreateAnnotation {
            quote: "highlighted text".to_string(),
            text: Some("a note".to_string()),
            ranges: serde_json::json!({}),
        };
        assert!(params.validate().is_ok());
    }
}
