use axum::extract::{Path, Query, State};
use axum::Json;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::{AppState, AuthUser};
use crate::models::entry::{self, ListParams, UpdateEntryParams};
use crate::tasks::fetcher::FetchJob;

#[derive(serde::Deserialize)]
pub struct CreateEntryRequest {
    pub url: String,
}

pub async fn create_entry(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateEntryRequest>,
) -> Result<Json<entry::Entry>, ApiError> {
    if req.url.is_empty() {
        return Err(ApiError::BadRequest("url is required".to_string()));
    }
    url::Url::parse(&req.url)
        .map_err(|_| ApiError::BadRequest("invalid URL".to_string()))?;
    let new_entry = entry::create_entry(&state.pool, auth.user_id, &req.url).await?;
    let _ = state.fetch_queue.send(FetchJob { entry_id: new_entry.id, url: new_entry.url.clone() }).await;
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

pub async fn list_entries(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<entry::EntrySummary>>, ApiError> {
    // If search query provided, use tantivy to get matching IDs first
    if let Some(ref query) = params.search {
        if !query.is_empty() {
            let ids = state
                .search_index
                .search(query, 100)
                .unwrap_or_default();
            if ids.is_empty() {
                return Ok(Json(vec![]));
            }
            let entries = entry::list_entries_by_ids(&state.pool, auth.user_id, &ids).await?;
            return Ok(Json(entries));
        }
    }
    let entries = entry::list_entries(&state.pool, auth.user_id, &params).await?;
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
    Ok(Json(serde_json::json!({"message": "deleted"})))
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
    let _ = state.fetch_queue.send(FetchJob { entry_id: found.id, url: found.url.clone() }).await;
    Ok(Json(serde_json::json!({"message": "refetch queued"})))
}
