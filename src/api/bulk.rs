use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use crate::auth::middleware::{AuthSource, AuthUser};
use crate::api::error::ApiError;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::{entry::{self, ListParams}, tag};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct BulkTagRequest {
    pub filter: ListParams,
    #[serde(default)]
    pub add: Vec<String>,
    #[serde(default)]
    pub dry_run: bool,
    pub max: Option<i64>,
}

#[derive(Deserialize)]
pub struct BulkUntagRequest {
    pub filter: ListParams,
    #[serde(default)]
    pub remove: Vec<String>,
    #[serde(default)]
    pub dry_run: bool,
    pub max: Option<i64>,
}

#[derive(Deserialize)]
pub struct BulkStateRequest {
    pub filter: ListParams,
    pub value: bool,
    #[serde(default)]
    pub dry_run: bool,
    pub max: Option<i64>,
}

#[derive(Serialize)]
pub struct BulkResult {
    pub matched: usize,
    pub updated: usize,
    pub ids: Vec<uuid::Uuid>,
}

fn auth_source_str(auth: &AuthUser) -> String {
    match auth.source {
        AuthSource::Jwt => "jwt".to_string(),
        AuthSource::Pat { .. } => "pat".to_string(),
    }
}

fn check_max(ids: &[uuid::Uuid], max: Option<i64>) -> Result<(), ApiError> {
    if let Some(m) = max {
        if ids.len() as i64 > m {
            return Err(ApiError::BadRequest(format!(
                "matched {} exceeds max {}", ids.len(), m
            )));
        }
    }
    Ok(())
}

pub async fn bulk_tag_add(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<BulkTagRequest>,
) -> Result<Json<BulkResult>, ApiError> {
    if req.add.is_empty() {
        return Err(ApiError::BadRequest("add cannot be empty".into()));
    }
    let ids = entry::find_ids_matching(&state.pool, auth.user_id, &req.filter).await?;
    check_max(&ids, req.max)?;
    if req.dry_run {
        return Ok(Json(BulkResult { matched: ids.len(), updated: 0, ids }));
    }
    tag::ensure_and_link(&state.pool, auth.user_id, &ids, &req.add).await?;
    let count = ids.len();

    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::BulkTagAdd,
            resource_type: Some(AuditResourceType::Entry),
            resource_id: None,
            status: "success".to_string(),
            details: serde_json::json!({"tags": req.add, "count": count}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;

    Ok(Json(BulkResult { matched: count, updated: count, ids }))
}

pub async fn bulk_untag(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<BulkUntagRequest>,
) -> Result<Json<BulkResult>, ApiError> {
    if req.remove.is_empty() {
        return Err(ApiError::BadRequest("remove cannot be empty".into()));
    }
    let ids = entry::find_ids_matching(&state.pool, auth.user_id, &req.filter).await?;
    check_max(&ids, req.max)?;
    if req.dry_run {
        return Ok(Json(BulkResult { matched: ids.len(), updated: 0, ids }));
    }
    for id in &ids {
        for label in &req.remove {
            if let Some(t) = sqlx::query_as::<_, (uuid::Uuid,)>(
                "SELECT id FROM tags WHERE user_id = $1 AND label = $2"
            )
            .bind(auth.user_id).bind(label)
            .fetch_optional(&state.pool).await
            .map_err(|e| ApiError::Internal(e.to_string()))? {
                tag::remove_tag_from_entry(&state.pool, *id, t.0).await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
            }
        }
    }
    let count = ids.len();

    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::BulkUntag,
            resource_type: Some(AuditResourceType::Entry),
            resource_id: None,
            status: "success".to_string(),
            details: serde_json::json!({"tags": req.remove, "count": count}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;

    Ok(Json(BulkResult { matched: count, updated: count, ids }))
}

pub async fn bulk_archive(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<BulkStateRequest>,
) -> Result<Json<BulkResult>, ApiError> {
    let ids = entry::find_ids_matching(&state.pool, auth.user_id, &req.filter).await?;
    check_max(&ids, req.max)?;
    if req.dry_run {
        return Ok(Json(BulkResult { matched: ids.len(), updated: 0, ids }));
    }
    if !ids.is_empty() {
        sqlx::query(
            "UPDATE entries SET is_archived = $1, \
             archived_at = CASE WHEN $1 THEN now() ELSE NULL END, \
             updated_at = now() \
             WHERE user_id = $2 AND id = ANY($3)"
        )
        .bind(req.value).bind(auth.user_id).bind(&ids)
        .execute(&state.pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    let count = ids.len();

    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::BulkArchive,
            resource_type: Some(AuditResourceType::Entry),
            resource_id: None,
            status: "success".to_string(),
            details: serde_json::json!({"value": req.value, "count": count}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;

    Ok(Json(BulkResult { matched: count, updated: count, ids }))
}

pub async fn bulk_star(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<BulkStateRequest>,
) -> Result<Json<BulkResult>, ApiError> {
    let ids = entry::find_ids_matching(&state.pool, auth.user_id, &req.filter).await?;
    check_max(&ids, req.max)?;
    if req.dry_run {
        return Ok(Json(BulkResult { matched: ids.len(), updated: 0, ids }));
    }
    if !ids.is_empty() {
        sqlx::query(
            "UPDATE entries SET is_starred = $1, \
             starred_at = CASE WHEN $1 THEN now() ELSE NULL END, \
             updated_at = now() \
             WHERE user_id = $2 AND id = ANY($3)"
        )
        .bind(req.value).bind(auth.user_id).bind(&ids)
        .execute(&state.pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    let count = ids.len();

    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::BulkStar,
            resource_type: Some(AuditResourceType::Entry),
            resource_id: None,
            status: "success".to_string(),
            details: serde_json::json!({"value": req.value, "count": count}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;

    Ok(Json(BulkResult { matched: count, updated: count, ids }))
}
