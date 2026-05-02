use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::Json;
use uuid::Uuid;
use validator::Validate;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::state::AppState;
use crate::models::audit_log::{self, AuditAction, AuditDetails, AuditResourceType};
use crate::models::entry::{self, ListParams, UpdateEntryParams};
use crate::models::tag;
use crate::tasks::fetcher::FetchJob;

use super::validate::{deserialize_bool_from_string, ValidatedJson};

#[derive(Debug, serde::Deserialize)]
pub struct ListQueryParams {
    #[serde(flatten)]
    pub inner: ListParams,
    #[serde(default, deserialize_with = "deserialize_bool_from_string")]
    pub deleted: Option<bool>,
}

#[derive(serde::Deserialize, Validate)]
pub struct CreateEntryRequest {
    #[validate(url(message = "invalid URL format"))]
    pub url: String,
    pub title: Option<String>,
    #[serde(default)]
    pub tag: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct CreateEntryResponse {
    #[serde(flatten)]
    pub entry: entry::Entry,
    pub already_existed: bool,
    pub tags: Vec<String>,
    pub status: String,
}

#[tracing::instrument(skip(state, req), err)]
pub async fn create_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<CreateEntryRequest>,
) -> Result<Json<CreateEntryResponse>, ApiError> {
    let r = entry::create_or_get_entry(&state.pool, auth.user_id, &req.url).await?;

    // Apply title override only for brand new entries
    if !r.already_existed {
        if let Some(title) = req.title.as_ref() {
            sqlx::query("UPDATE entries SET title = $1 WHERE id = $2")
                .bind(title)
                .bind(r.entry.id)
                .execute(&state.pool)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
        }
    }

    // Union-merge tags (single transaction, batch insert).
    if !req.tag.is_empty() {
        tag::ensure_and_link(&state.pool, auth.user_id, &[r.entry.id], &req.tag).await?;
    }

    // Fetch tag labels for response
    let tag_rows = tag::list_tags_for_entry(&state.pool, r.entry.id).await?;
    let tag_labels: Vec<String> = tag_rows.iter().map(|t| t.label.clone()).collect();

    // Only enqueue for new entries
    let status = if r.already_existed {
        "existing".to_string()
    } else {
        let _ = state.fetch_queue.send(FetchJob {
            entry_id: r.entry.id,
            user_id: auth.user_id,
            url: r.entry.url.clone(),
        }).await;
        "queued".to_string()
    };

    tracing::info!(entry_id = %r.entry.id, already_existed = r.already_existed, "entry save");

    let details = serde_json::to_value(AuditDetails {
        after: Some(serde_json::json!({
            "url": r.entry.url,
            "title": r.entry.title,
            "already_existed": r.already_existed,
        })),
        ..Default::default()
    }).unwrap_or_default();

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::CreateEntry,
        Some(AuditResourceType::Entry),
        Some(r.entry.id),
        details,
    ).await;

    Ok(Json(CreateEntryResponse {
        entry: r.entry,
        already_existed: r.already_existed,
        tags: tag_labels,
        status,
    }))
}

pub async fn get_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<entry::Entry>, ApiError> {
    let found = entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;
    Ok(Json(found))
}

#[tracing::instrument(skip(state), err)]
pub async fn list_entries(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<ListQueryParams>,
) -> Result<axum::response::Response, ApiError> {
    const MAX_PAGE: i64 = 50;
    if params.inner.cursor.is_none() {
        if let Some(p) = params.inner.page {
            if p > MAX_PAGE {
                return Err(ApiError::BadRequest(format!(
                    "page {} exceeds max {} — narrow filter or use cursor",
                    p, MAX_PAGE
                )));
            }
        }
    }

    // Pre-validate the cursor at the API boundary so a malformed cursor turns
    // into a 400 (rather than relying on the model's Database error → 500).
    if let Some(c) = params.inner.cursor.as_deref() {
        if entry::cursor::decode(c).is_none() {
            return Err(ApiError::BadRequest(format!("invalid cursor: {}", c)));
        }
    }

    // If deleted=true, return soft-deleted entries
    if params.deleted == Some(true) {
        let entries = entry::list_deleted_entries(&state.pool, auth.user_id).await?;
        return Ok(Json(entries).into_response());
    }

    // If search query provided, use tantivy to get matching IDs first
    if let Some(ref query) = params.inner.search {
        if !query.is_empty() {
            let ids = match state.search_index.search(query, Some(auth.user_id), 100) {
                Ok(ids) => ids,
                Err(e) => {
                    tracing::warn!("Search query {:?} failed: {e}", query);
                    Vec::new()
                }
            };
            if !ids.is_empty() {
                let entries = entry::list_entries_by_ids(&state.pool, auth.user_id, &ids).await?;
                return Ok(Json(entries).into_response());
            }
            // Tantivy returned 0 results — fall through to SQL ILIKE search
            // which handles cases where the index is incomplete or stale.
        }
    }

    let per_page = params.inner.per_page.unwrap_or(20).min(100);
    let entries = entry::list_entries(&state.pool, auth.user_id, &params.inner).await?;

    // Always compute next_cursor for infinite scroll to work correctly.
    // Page-mode callers simply don't read this header.
    let next = entry::next_cursor_from(&entries, per_page);

    let mut response = Json(entries).into_response();
    if let Some(c) = next {
        response.headers_mut().insert(
            "X-Next-Cursor",
            axum::http::HeaderValue::from_str(&c)
                .map_err(|e| ApiError::Internal(format!("cursor header: {e}")))?,
        );
    }
    Ok(response)
}

pub async fn update_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
    Json(params): Json<UpdateEntryParams>,
) -> Result<Json<entry::Entry>, ApiError> {
    let updated = entry::update_entry(&state.pool, auth.user_id, entry_id, &params).await?;

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::UpdateEntry,
        Some(AuditResourceType::Entry),
        Some(entry_id),
        serde_json::to_value(AuditDetails {
            after: Some(serde_json::json!({
                "title": updated.title,
                "is_archived": updated.is_archived,
                "is_starred": updated.is_starred,
            })),
            ..Default::default()
        }).unwrap_or_default(),
    ).await;

    Ok(Json(updated))
}

pub async fn delete_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = entry::delete_entry(&state.pool, auth.user_id, entry_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("entry not found".to_string()));
    }
    // Remove from search index on soft delete
    if let Err(e) = state.search_index.delete(entry_id).await {
        tracing::warn!("search index delete failed for entry {entry_id}: {e}");
    }
    tracing::info!(entry_id = %entry_id, "entry soft-deleted");

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::SoftDeleteEntry,
        Some(AuditResourceType::Entry),
        Some(entry_id),
        serde_json::json!({}),
    ).await;

    Ok(Json(serde_json::json!({"message": "deleted"})))
}

pub async fn restore_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    entry::restore_entry(&state.pool, entry_id, auth.user_id).await?;
    // Re-index restored entry
    if let Ok(Some(restored)) = entry::find_entry_by_id(&state.pool, auth.user_id, entry_id).await {
        if let Err(e) = state.search_index.upsert(
            restored.id,
            auth.user_id,
            restored.title.as_deref().unwrap_or(""),
            restored.text_content.as_deref().unwrap_or(""),
            &restored.url,
            restored.domain_name.as_deref().unwrap_or(""),
        ).await {
            tracing::warn!("search index upsert failed for entry {entry_id}: {e}");
        }
    }
    tracing::info!(entry_id = %entry_id, "entry restored");

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::RestoreEntry,
        Some(AuditResourceType::Entry),
        Some(entry_id),
        serde_json::json!({}),
    ).await;

    Ok(Json(serde_json::json!({"message": "restored"})))
}

pub async fn permanently_delete_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    entry::permanently_delete_entry(&state.pool, entry_id, auth.user_id).await?;
    // Ensure removed from search index. Commit immediately so a stale doc
    // pointing at a deleted entry can never surface (data-privacy guarantee).
    if let Err(e) = state.search_index.delete(entry_id).await {
        tracing::warn!("search index delete failed for entry {entry_id}: {e}");
    }
    if let Err(e) = state.search_index.commit().await {
        tracing::warn!("search index commit failed: {e}");
    }
    tracing::info!(entry_id = %entry_id, "entry permanently deleted");

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::PermanentDeleteEntry,
        Some(AuditResourceType::Entry),
        Some(entry_id),
        serde_json::json!({}),
    ).await;

    Ok(Json(serde_json::json!({"message": "permanently deleted"})))
}

pub async fn refetch_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let found = entry::find_entry_by_id(&state.pool, auth.user_id, entry_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;
    if found.is_content_edited {
        return Err(ApiError::BadRequest("cannot refetch edited content".to_string()));
    }
    let _ = state.fetch_queue.send(FetchJob { entry_id: found.id, user_id: auth.user_id, url: found.url.clone() }).await;

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::RefetchEntry,
        Some(AuditResourceType::Entry),
        Some(entry_id),
        serde_json::json!({}),
    ).await;

    Ok(Json(serde_json::json!({"message": "refetch queued"})))
}