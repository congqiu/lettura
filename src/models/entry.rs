use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Entry {
    pub id: Uuid, pub user_id: Uuid, pub url: String, pub given_url: String,
    pub hashed_url: String, pub hashed_given_url: String,
    pub title: Option<String>, pub content: Option<String>, pub text_content: Option<String>,
    pub content_type: String, pub extract_method: String, pub is_content_edited: bool,
    pub language: Option<String>, pub http_status: Option<i16>, pub reading_time: Option<i32>,
    pub preview_picture: Option<String>, pub domain_name: Option<String>,
    pub published_by: Option<String>, pub metadata: serde_json::Value,
    pub is_archived: bool, pub archived_at: Option<DateTime<Utc>>,
    pub is_starred: bool, pub starred_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct EntrySummary {
    pub id: Uuid, pub user_id: Uuid, pub url: String, pub title: Option<String>,
    pub content_type: String, pub extract_method: String, pub language: Option<String>,
    pub reading_time: Option<i32>, pub preview_picture: Option<String>,
    pub domain_name: Option<String>, pub published_by: Option<String>,
    pub is_archived: bool, pub is_starred: bool, pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

pub fn hash_url(url: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(url.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn extract_domain(url_str: &str) -> Option<String> {
    Url::parse(url_str).ok().and_then(|u| u.host_str().map(String::from))
}

pub async fn create_entry(pool: &PgPool, user_id: Uuid, given_url: &str) -> Result<Entry, ApiError> {
    let url = given_url.to_string();
    let hashed_url = hash_url(&url);
    let hashed_given_url = hash_url(given_url);
    let domain_name = extract_domain(&url);

    Ok(sqlx::query_as::<_, Entry>(
        "INSERT INTO entries (user_id, url, given_url, hashed_url, hashed_given_url, domain_name) VALUES ($1,$2,$3,$4,$5,$6) RETURNING *"
    )
    .bind(user_id).bind(&url).bind(given_url).bind(&hashed_url).bind(&hashed_given_url).bind(&domain_name)
    .fetch_one(pool).await?)
}

pub async fn find_entry_by_id(pool: &PgPool, user_id: Uuid, entry_id: Uuid) -> Result<Option<Entry>, ApiError> {
    sqlx::query_as::<_, Entry>("SELECT * FROM entries WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL")
        .bind(entry_id).bind(user_id).fetch_optional(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub page: Option<i64>, pub per_page: Option<i64>,
    pub is_archived: Option<bool>, pub is_starred: Option<bool>, pub domain: Option<String>,
    pub search: Option<String>,
}

pub async fn list_entries(pool: &PgPool, user_id: Uuid, params: &ListParams) -> Result<Vec<EntrySummary>, ApiError> {
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (params.page.unwrap_or(1) - 1).max(0) * per_page;

    let mut sql = String::from(
        "SELECT id, user_id, url, title, content_type, extract_method, language, reading_time, preview_picture, domain_name, published_by, is_archived, is_starred, created_at, deleted_at FROM entries WHERE user_id = $1 AND deleted_at IS NULL"
    );
    let mut param_idx = 2u32;
    if params.is_archived.is_some() { sql.push_str(&format!(" AND is_archived = ${}", param_idx)); param_idx += 1; }
    if params.is_starred.is_some() { sql.push_str(&format!(" AND is_starred = ${}", param_idx)); param_idx += 1; }
    if params.domain.is_some() { sql.push_str(&format!(" AND domain_name = ${}", param_idx)); param_idx += 1; }
    sql.push_str(&format!(" ORDER BY created_at DESC LIMIT ${} OFFSET ${}", param_idx, param_idx + 1));

    let mut query = sqlx::query_as::<_, EntrySummary>(&sql).bind(user_id);
    if let Some(archived) = params.is_archived { query = query.bind(archived); }
    if let Some(starred) = params.is_starred { query = query.bind(starred); }
    if let Some(ref domain) = params.domain { query = query.bind(domain); }
    query = query.bind(per_page).bind(offset);
    query.fetch_all(pool).await.map_err(|e| ApiError::Internal(e.to_string()))
}

#[derive(Debug, Deserialize)]
pub struct UpdateEntryParams {
    pub title: Option<String>, pub content: Option<String>,
    pub is_archived: Option<bool>, pub is_starred: Option<bool>,
}

pub async fn update_entry(pool: &PgPool, user_id: Uuid, entry_id: Uuid, params: &UpdateEntryParams) -> Result<Entry, ApiError> {
    let existing = find_entry_by_id(pool, user_id, entry_id).await?
        .ok_or_else(|| ApiError::NotFound("entry not found".to_string()))?;

    let title = params.title.as_deref().unwrap_or(existing.title.as_deref().unwrap_or(""));
    let content = params.content.as_deref().or(existing.content.as_deref());
    let is_content_edited = if params.content.is_some() { true } else { existing.is_content_edited };
    let is_archived = params.is_archived.unwrap_or(existing.is_archived);
    let archived_at = if params.is_archived == Some(true) && !existing.is_archived { Some(Utc::now()) }
        else if params.is_archived == Some(false) { None } else { existing.archived_at };
    let is_starred = params.is_starred.unwrap_or(existing.is_starred);
    let starred_at = if params.is_starred == Some(true) && !existing.is_starred { Some(Utc::now()) }
        else if params.is_starred == Some(false) { None } else { existing.starred_at };

    sqlx::query_as::<_, Entry>(
        "UPDATE entries SET title=$3, content=$4, is_content_edited=$5, is_archived=$6, archived_at=$7, is_starred=$8, starred_at=$9, updated_at=now() WHERE id=$1 AND user_id=$2 RETURNING *"
    )
    .bind(entry_id).bind(user_id).bind(title).bind(content).bind(is_content_edited)
    .bind(is_archived).bind(archived_at).bind(is_starred).bind(starred_at)
    .fetch_one(pool).await.map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn list_entries_by_ids(pool: &PgPool, user_id: Uuid, ids: &[Uuid]) -> Result<Vec<EntrySummary>, ApiError> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    sqlx::query_as::<_, EntrySummary>(
        "SELECT id, user_id, url, title, content_type, extract_method, language, reading_time, preview_picture, domain_name, published_by, is_archived, is_starred, created_at, deleted_at FROM entries WHERE user_id = $1 AND id = ANY($2) AND deleted_at IS NULL ORDER BY created_at DESC"
    )
    .bind(user_id)
    .bind(ids)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn delete_entry(pool: &PgPool, user_id: Uuid, entry_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("UPDATE entries SET deleted_at = now() WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL")
        .bind(entry_id).bind(user_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_deleted_entries(pool: &PgPool, user_id: Uuid) -> Result<Vec<EntrySummary>, ApiError> {
    sqlx::query_as::<_, EntrySummary>(
        "SELECT id, user_id, url, title, content_type, extract_method, language, reading_time, preview_picture, domain_name, published_by, is_archived, is_starred, created_at, deleted_at FROM entries WHERE user_id = $1 AND deleted_at IS NOT NULL ORDER BY deleted_at DESC"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn restore_entry(pool: &PgPool, entry_id: Uuid, user_id: Uuid) -> Result<(), ApiError> {
    let result = sqlx::query("UPDATE entries SET deleted_at = NULL WHERE id = $1 AND user_id = $2 AND deleted_at IS NOT NULL")
        .bind(entry_id).bind(user_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("entry not found or not deleted".to_string()));
    }
    Ok(())
}

pub async fn permanently_delete_entry(pool: &PgPool, entry_id: Uuid, user_id: Uuid) -> Result<(), ApiError> {
    let result = sqlx::query("DELETE FROM entries WHERE id = $1 AND user_id = $2 AND deleted_at IS NOT NULL")
        .bind(entry_id).bind(user_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("entry not found".to_string()));
    }
    Ok(())
}

pub async fn update_entry_content(
    pool: &PgPool, entry_id: Uuid,
    title: Option<&str>, content: Option<&str>, text_content: Option<&str>,
    language: Option<&str>, preview_picture: Option<&str>, published_by: Option<&str>,
    reading_time: Option<i32>, http_status: i16, extract_method: &str,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE entries SET title=COALESCE($2,title), content=$3, text_content=$4, language=$5, preview_picture=$6, published_by=$7, reading_time=$8, http_status=$9, extract_method=$10, updated_at=now() WHERE id=$1 AND is_content_edited=false"
    )
    .bind(entry_id).bind(title).bind(content).bind(text_content).bind(language)
    .bind(preview_picture).bind(published_by).bind(reading_time).bind(http_status).bind(extract_method)
    .execute(pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}
