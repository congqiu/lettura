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

#[utoipa::path(
    get,
    path = "/api/v1/entries/{entry_id}/annotations",
    tag = "annotations",
    params(("entry_id" = Uuid, Path, description = "Entry ID")),
    responses(
        (status = 200, description = "Annotations for the entry", body = Vec<annotation::Annotation>),
        (status = 401, description = "Missing or invalid auth"),
        (status = 404, description = "Entry not found"),
    ),
    security(("bearer" = [])),
)]
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

#[utoipa::path(
    post,
    path = "/api/v1/entries/{entry_id}/annotations",
    tag = "annotations",
    params(("entry_id" = Uuid, Path, description = "Entry ID")),
    request_body = annotation::CreateAnnotation,
    responses(
        (status = 201, description = "Annotation created", body = annotation::Annotation),
        (status = 401, description = "Missing or invalid auth"),
        (status = 404, description = "Entry not found"),
        (status = 422, description = "Validation error"),
    ),
    security(("bearer" = [])),
)]
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

#[utoipa::path(
    patch,
    path = "/api/v1/annotations/{id}",
    tag = "annotations",
    params(("id" = Uuid, Path, description = "Annotation ID")),
    request_body = annotation::UpdateAnnotation,
    responses(
        (status = 200, description = "Annotation updated", body = annotation::Annotation),
        (status = 401, description = "Missing or invalid auth"),
        (status = 404, description = "Annotation not found"),
    ),
    security(("bearer" = [])),
)]
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

#[utoipa::path(
    delete,
    path = "/api/v1/annotations/{id}",
    tag = "annotations",
    params(("id" = Uuid, Path, description = "Annotation ID")),
    responses(
        (status = 200, description = "Annotation deleted"),
        (status = 401, description = "Missing or invalid auth"),
        (status = 404, description = "Annotation not found"),
    ),
    security(("bearer" = [])),
)]
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
