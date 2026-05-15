use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::api::validate::ValidatedJson;
use crate::auth::middleware::AuthUser;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::{
    entry::{self, ListParams},
    tag,
};
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

fn check_max(ids: &[uuid::Uuid], max: Option<i64>) -> Result<(), ApiError> {
    if let Some(m) = max
        && ids.len() as i64 > m
    {
        return Err(ApiError::BadRequest(format!(
            "matched {} exceeds max {}",
            ids.len(),
            m
        )));
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
        return Ok(Json(BulkResult {
            matched: ids.len(),
            updated: 0,
            ids,
        }));
    }
    tag::ensure_and_link(&state.pool, auth.user_id, &ids, &req.add).await?;
    let count = ids.len();

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::BulkTagAdd,
        Some(AuditResourceType::Entry),
        None,
        serde_json::json!({"tags": req.add, "count": count}),
    )
    .await;

    Ok(Json(BulkResult {
        matched: count,
        updated: count,
        ids,
    }))
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
        return Ok(Json(BulkResult {
            matched: ids.len(),
            updated: 0,
            ids,
        }));
    }
    for id in &ids {
        for label in &req.remove {
            if let Some(t) = sqlx::query_as::<_, (uuid::Uuid,)>(
                "SELECT id FROM tags WHERE user_id = $1 AND label = $2",
            )
            .bind(auth.user_id)
            .bind(label)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            {
                tag::remove_tag_from_entry(&state.pool, auth.user_id, *id, t.0)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
            }
        }
    }
    let count = ids.len();

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::BulkUntag,
        Some(AuditResourceType::Entry),
        None,
        serde_json::json!({"tags": req.remove, "count": count}),
    )
    .await;

    Ok(Json(BulkResult {
        matched: count,
        updated: count,
        ids,
    }))
}

pub async fn bulk_archive(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<BulkStateRequest>,
) -> Result<Json<BulkResult>, ApiError> {
    let ids = entry::find_ids_matching(&state.pool, auth.user_id, &req.filter).await?;
    check_max(&ids, req.max)?;
    if req.dry_run {
        return Ok(Json(BulkResult {
            matched: ids.len(),
            updated: 0,
            ids,
        }));
    }
    if !ids.is_empty() {
        sqlx::query(
            "UPDATE entries SET is_archived = $1, \
             archived_at = CASE WHEN $1 THEN now() ELSE NULL END, \
             updated_at = now() \
             WHERE user_id = $2 AND id = ANY($3)",
        )
        .bind(req.value)
        .bind(auth.user_id)
        .bind(&ids)
        .execute(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    let count = ids.len();

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::BulkArchive,
        Some(AuditResourceType::Entry),
        None,
        serde_json::json!({"value": req.value, "count": count}),
    )
    .await;

    Ok(Json(BulkResult {
        matched: count,
        updated: count,
        ids,
    }))
}

pub async fn bulk_star(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<BulkStateRequest>,
) -> Result<Json<BulkResult>, ApiError> {
    let ids = entry::find_ids_matching(&state.pool, auth.user_id, &req.filter).await?;
    check_max(&ids, req.max)?;
    if req.dry_run {
        return Ok(Json(BulkResult {
            matched: ids.len(),
            updated: 0,
            ids,
        }));
    }
    if !ids.is_empty() {
        sqlx::query(
            "UPDATE entries SET is_starred = $1, \
             starred_at = CASE WHEN $1 THEN now() ELSE NULL END, \
             updated_at = now() \
             WHERE user_id = $2 AND id = ANY($3)",
        )
        .bind(req.value)
        .bind(auth.user_id)
        .bind(&ids)
        .execute(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    let count = ids.len();

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::BulkStar,
        Some(AuditResourceType::Entry),
        None,
        serde_json::json!({"value": req.value, "count": count}),
    )
    .await;

    Ok(Json(BulkResult {
        matched: count,
        updated: count,
        ids,
    }))
}

// --- Bulk-by-IDs endpoints ---

#[derive(Deserialize, Validate)]
pub struct BulkTagByIdsRequest {
    #[validate(length(min = 1, max = 100, message = "entry_ids must have 1-100 items"))]
    pub entry_ids: Vec<uuid::Uuid>,
    #[validate(length(min = 1, max = 20, message = "tags must have 1-20 items"))]
    pub tags: Vec<String>,
}

#[derive(Deserialize, Validate)]
pub struct BulkUntagByIdsRequest {
    #[validate(length(min = 1, max = 100, message = "entry_ids must have 1-100 items"))]
    pub entry_ids: Vec<uuid::Uuid>,
    #[validate(length(min = 1, max = 20, message = "tags must have 1-20 items"))]
    pub tags: Vec<String>,
}

#[derive(Deserialize, Validate)]
pub struct BulkDeleteByIdsRequest {
    #[validate(length(min = 1, max = 100, message = "entry_ids must have 1-100 items"))]
    pub entry_ids: Vec<uuid::Uuid>,
}

#[derive(Deserialize, Validate)]
pub struct BulkArchiveByIdsRequest {
    #[validate(length(min = 1, max = 100, message = "entry_ids must have 1-100 items"))]
    pub entry_ids: Vec<uuid::Uuid>,
}

pub async fn bulk_tag_by_ids(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<BulkTagByIdsRequest>,
) -> Result<Json<BulkResult>, ApiError> {
    if req.entry_ids.is_empty() {
        return Err(ApiError::BadRequest("entry_ids cannot be empty".into()));
    }
    if req.tags.is_empty() {
        return Err(ApiError::BadRequest("tags cannot be empty".into()));
    }
    tag::ensure_and_link(&state.pool, auth.user_id, &req.entry_ids, &req.tags).await?;
    let count = req.entry_ids.len();

    // Invalidate tag stats cache since tag counts may have changed
    crate::cache::TAG_STATS_CACHE.invalidate(auth.user_id).await;

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::BulkTagAdd,
        Some(AuditResourceType::Entry),
        None,
        serde_json::json!({"tags": req.tags, "count": count}),
    )
    .await;

    Ok(Json(BulkResult {
        matched: count,
        updated: count,
        ids: req.entry_ids,
    }))
}

pub async fn bulk_untag_by_ids(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<BulkUntagByIdsRequest>,
) -> Result<Json<BulkResult>, ApiError> {
    if req.entry_ids.is_empty() {
        return Err(ApiError::BadRequest("entry_ids cannot be empty".into()));
    }
    if req.tags.is_empty() {
        return Err(ApiError::BadRequest("tags cannot be empty".into()));
    }
    for id in &req.entry_ids {
        for label in &req.tags {
            if let Some(t) = sqlx::query_as::<_, (uuid::Uuid,)>(
                "SELECT id FROM tags WHERE user_id = $1 AND label = $2",
            )
            .bind(auth.user_id)
            .bind(label)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            {
                tag::remove_tag_from_entry(&state.pool, auth.user_id, *id, t.0)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
            }
        }
    }
    let count = req.entry_ids.len();

    // Invalidate tag stats cache since tag counts may have changed
    crate::cache::TAG_STATS_CACHE.invalidate(auth.user_id).await;

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::BulkUntag,
        Some(AuditResourceType::Entry),
        None,
        serde_json::json!({"tags": req.tags, "count": count}),
    )
    .await;

    Ok(Json(BulkResult {
        matched: count,
        updated: count,
        ids: req.entry_ids,
    }))
}

pub async fn bulk_delete_by_ids(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<BulkDeleteByIdsRequest>,
) -> Result<Json<BulkResult>, ApiError> {
    if req.entry_ids.is_empty() {
        return Err(ApiError::BadRequest("entry_ids cannot be empty".into()));
    }
    let result =
        sqlx::query("UPDATE entries SET deleted_at = NOW() WHERE id = ANY($1) AND user_id = $2")
            .bind(&req.entry_ids)
            .bind(auth.user_id)
            .execute(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

    let count = result.rows_affected() as usize;

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::BulkSoftDelete,
        Some(AuditResourceType::Entry),
        None,
        serde_json::json!({"count": count}),
    )
    .await;

    Ok(Json(BulkResult {
        matched: req.entry_ids.len(),
        updated: count,
        ids: req.entry_ids,
    }))
}

pub async fn bulk_archive_by_ids(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<BulkArchiveByIdsRequest>,
) -> Result<Json<BulkResult>, ApiError> {
    if req.entry_ids.is_empty() {
        return Err(ApiError::BadRequest("entry_ids cannot be empty".into()));
    }
    let result = sqlx::query(
        "UPDATE entries SET is_archived = true, archived_at = NOW(), updated_at = NOW() WHERE id = ANY($1) AND user_id = $2"
    )
    .bind(&req.entry_ids)
    .bind(auth.user_id)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let count = result.rows_affected() as usize;

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::BulkArchive,
        Some(AuditResourceType::Entry),
        None,
        serde_json::json!({"count": count}),
    )
    .await;

    Ok(Json(BulkResult {
        matched: req.entry_ids.len(),
        updated: count,
        ids: req.entry_ids,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ids(n: usize) -> Vec<uuid::Uuid> {
        (0..n).map(|_| uuid::Uuid::new_v4()).collect()
    }

    #[test]
    fn check_max_exceeds_limit() {
        let ids = make_ids(5);
        let result = check_max(&ids, Some(3));
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::BadRequest(msg) => assert!(msg.contains("exceeds max")),
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }

    #[test]
    fn check_max_equals_limit() {
        let ids = make_ids(3);
        let result = check_max(&ids, Some(3));
        assert!(result.is_ok());
    }

    #[test]
    fn check_max_below_limit() {
        let ids = make_ids(2);
        let result = check_max(&ids, Some(3));
        assert!(result.is_ok());
    }

    #[test]
    fn check_max_none_always_ok() {
        let ids = make_ids(999);
        let result = check_max(&ids, None);
        assert!(result.is_ok());
    }
}
