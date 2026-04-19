use axum::extract::{Path, Query, State};
use axum::Json;
use uuid::Uuid;
use validator::Validate;

use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::state::AppState;
use crate::models::entry::{self, ListParams, UpdateEntryParams};
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
}

#[tracing::instrument(skip(state, req), err)]
pub async fn create_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<CreateEntryRequest>,
) -> Result<Json<entry::Entry>, ApiError> {
    let new_entry = entry::create_entry(&state.pool, auth.user_id, &req.url).await?;
    let _ = state.fetch_queue.send(FetchJob { entry_id: new_entry.id, user_id: auth.user_id, url: new_entry.url.clone() }).await;
    tracing::info!(entry_id = %new_entry.id, url = %req.url, "entry created");
    Ok(Json(new_entry))
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
) -> Result<Json<Vec<entry::EntrySummary>>, ApiError> {
    // If deleted=true, return soft-deleted entries
    if params.deleted == Some(true) {
        let entries = entry::list_deleted_entries(&state.pool, auth.user_id).await?;
        return Ok(Json(entries));
    }

    // If search query provided, use tantivy to get matching IDs first
    if let Some(ref query) = params.inner.search {
        if !query.is_empty() {
            let ids = state
                .search_index
                .search(query, Some(auth.user_id), 100)
                .unwrap_or_default();
            if ids.is_empty() {
                return Ok(Json(vec![]));
            }
            let entries = entry::list_entries_by_ids(&state.pool, auth.user_id, &ids).await?;
            return Ok(Json(entries));
        }
    }
    let entries = entry::list_entries(&state.pool, auth.user_id, &params.inner).await?;
    Ok(Json(entries))
}

pub async fn update_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
    Json(params): Json<UpdateEntryParams>,
) -> Result<Json<entry::Entry>, ApiError> {
    let updated = entry::update_entry(&state.pool, auth.user_id, entry_id, &params).await?;
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
    let _ = state.search_index.delete(entry_id).await;
    tracing::info!(entry_id = %entry_id, "entry soft-deleted");
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
        let _ = state.search_index.upsert(
            restored.id,
            auth.user_id,
            restored.title.as_deref().unwrap_or(""),
            restored.text_content.as_deref().unwrap_or(""),
            &restored.url,
            restored.domain_name.as_deref().unwrap_or(""),
        ).await;
    }
    tracing::info!(entry_id = %entry_id, "entry restored");
    Ok(Json(serde_json::json!({"message": "restored"})))
}

pub async fn permanently_delete_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entry_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    entry::permanently_delete_entry(&state.pool, entry_id, auth.user_id).await?;
    // Ensure removed from search index
    let _ = state.search_index.delete(entry_id).await;
    tracing::info!(entry_id = %entry_id, "entry permanently deleted");
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
    Ok(Json(serde_json::json!({"message": "refetch queued"})))
}
