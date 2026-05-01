use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::{AuthSource, AuthUser};
use crate::state::AppState;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::{annotation, entry};

use super::validate::ValidatedJson;

fn auth_source_str(auth: &AuthUser) -> String {
    match auth.source {
        AuthSource::Jwt => "jwt".to_string(),
        AuthSource::Pat { .. } => "pat".to_string(),
    }
}

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
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::CreateAnnotation,
            resource_type: Some(AuditResourceType::Annotation),
            resource_id: Some(ann.id),
            status: "success".to_string(),
            details: serde_json::json!({"entry_id": entry_id}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;
    Ok(Json(ann))
}

pub async fn update_annotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(annotation_id): Path<Uuid>,
    Json(params): Json<annotation::UpdateAnnotation>,
) -> Result<Json<annotation::Annotation>, ApiError> {
    let updated = annotation::update(&state.pool, annotation_id, auth.user_id, &params).await?;
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::UpdateAnnotation,
            resource_type: Some(AuditResourceType::Annotation),
            resource_id: Some(annotation_id),
            status: "success".to_string(),
            details: serde_json::json!({}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;
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
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::DeleteAnnotation,
            resource_type: Some(AuditResourceType::Annotation),
            resource_id: Some(annotation_id),
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
