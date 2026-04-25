use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

use super::error::ModelError;

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

pub struct CreateEntryResult {
    pub entry: Entry,
    pub already_existed: bool,
}

/// Idempotent entry creation: returns existing entry on URL conflict.
/// Uses ON CONFLICT (user_id, hashed_url) DO NOTHING + fallback SELECT.
pub async fn create_or_get_entry(
    pool: &PgPool,
    user_id: Uuid,
    given_url: &str,
) -> Result<CreateEntryResult, ModelError> {
    let hashed_url = hash_url(given_url);
    let hashed_given_url = hash_url(given_url);
    let domain_name = extract_domain(given_url);

    // Try insert; ON CONFLICT return None
    let inserted: Option<Entry> = sqlx::query_as(
        "INSERT INTO entries (user_id, url, given_url, hashed_url, hashed_given_url, domain_name) \
         VALUES ($1,$2,$3,$4,$5,$6) \
         ON CONFLICT (user_id, hashed_url) DO NOTHING \
         RETURNING *",
    )
    .bind(user_id)
    .bind(given_url)
    .bind(given_url)
    .bind(&hashed_url)
    .bind(&hashed_given_url)
    .bind(&domain_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;

    if let Some(entry) = inserted {
        return Ok(CreateEntryResult { entry, already_existed: false });
    }

    // Fallback: look up the existing entry
    let existing: Entry = sqlx::query_as(
        "SELECT * FROM entries WHERE user_id = $1 AND hashed_url = $2 AND deleted_at IS NULL",
    )
    .bind(user_id)
    .bind(&hashed_url)
    .fetch_one(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;

    Ok(CreateEntryResult { entry: existing, already_existed: true })
}

pub async fn create_entry(pool: &PgPool, user_id: Uuid, given_url: &str) -> Result<Entry, ModelError> {
    create_or_get_entry(pool, user_id, given_url)
        .await
        .map(|r| r.entry)
}

pub async fn find_entry_by_id(pool: &PgPool, user_id: Uuid, entry_id: Uuid) -> Result<Option<Entry>, ModelError> {
    sqlx::query_as::<_, Entry>("SELECT * FROM entries WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL")
        .bind(entry_id).bind(user_id).fetch_optional(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    #[serde(default, deserialize_with = "crate::models::serde_helpers::deserialize_i64_from_string")]
    pub page: Option<i64>,
    #[serde(default, deserialize_with = "crate::models::serde_helpers::deserialize_i64_from_string")]
    pub per_page: Option<i64>,
    #[serde(default, deserialize_with = "crate::models::serde_helpers::deserialize_bool_from_string")]
    pub is_archived: Option<bool>,
    #[serde(default, deserialize_with = "crate::models::serde_helpers::deserialize_bool_from_string")]
    pub is_starred: Option<bool>,
    /// Alias for `is_archived` (CLI Filter DSL compatibility).
    /// `is_read=true` → `is_archived = true`, `is_read=false` → `is_archived = false`.
    #[serde(default, deserialize_with = "crate::models::serde_helpers::deserialize_bool_from_string")]
    pub is_read: Option<bool>,
    pub domain: Option<String>,
    /// Comma-separated tag labels; AND semantics (entry must have ALL listed tags).
    pub tag: Option<String>,
    /// Comma-separated tag labels to exclude; entry must NOT have any of these tags.
    pub exclude_tag: Option<String>,
    /// When true, return only entries with no tags.
    #[serde(default, deserialize_with = "crate::models::serde_helpers::deserialize_bool_from_string")]
    pub untagged: Option<bool>,
    /// Return entries created at or after this timestamp.
    pub since: Option<DateTime<Utc>>,
    /// Return entries created strictly before this timestamp.
    pub before: Option<DateTime<Utc>>,
    /// Search query. Handled by tantivy in the API handler; unused in list_entries itself.
    pub search: Option<String>,
    /// Field-projection hint — placeholder for Task 15, not used in query construction.
    pub fields: Option<String>,
}

pub async fn list_entries(
    pool: &PgPool,
    user_id: Uuid,
    params: &ListParams,
) -> Result<Vec<EntrySummary>, ModelError> {
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (params.page.unwrap_or(1) - 1).max(0) * per_page;

    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT id, user_id, url, title, content_type, extract_method, language, reading_time, \
         preview_picture, domain_name, published_by, is_archived, is_starred, created_at, deleted_at \
         FROM entries WHERE user_id = ",
    );
    build_where_clause(&mut qb, user_id, params);

    qb.push(" ORDER BY created_at DESC LIMIT ");
    qb.push_bind(per_page);
    qb.push(" OFFSET ");
    qb.push_bind(offset);

    qb.build_query_as::<EntrySummary>()
        .fetch_all(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))
}

/// Build the shared WHERE clause for entry list queries.
fn build_where_clause(
    qb: &mut sqlx::QueryBuilder<'static, sqlx::Postgres>,
    user_id: Uuid,
    params: &ListParams,
) {
    qb.push_bind(user_id);
    qb.push(" AND deleted_at IS NULL");

    if let Some(b) = params.is_archived {
        qb.push(" AND is_archived = ");
        qb.push_bind(b);
    }
    if let Some(b) = params.is_read {
        qb.push(" AND is_archived = ");
        qb.push_bind(b);
    }
    if let Some(b) = params.is_starred {
        qb.push(" AND is_starred = ");
        qb.push_bind(b);
    }
    if let Some(d) = &params.domain {
        qb.push(" AND domain_name = ");
        qb.push_bind(d.clone());
    }
    if let Some(t) = params.since {
        qb.push(" AND created_at >= ");
        qb.push_bind(t);
    }
    if let Some(t) = params.before {
        qb.push(" AND created_at < ");
        qb.push_bind(t);
    }
    if let Some(true) = params.untagged {
        qb.push(
            " AND NOT EXISTS (SELECT 1 FROM entry_tags et WHERE et.entry_id = entries.id)",
        );
    }
    if let Some(tags_csv) = &params.tag {
        for t in tags_csv.split(',').filter(|s| !s.trim().is_empty()) {
            qb.push(
                " AND EXISTS (SELECT 1 FROM entry_tags et JOIN tags tg ON tg.id = et.tag_id \
                 WHERE et.entry_id = entries.id AND tg.user_id = ",
            );
            qb.push_bind(user_id);
            qb.push(" AND tg.label = ");
            qb.push_bind(t.trim().to_string());
            qb.push(")");
        }
    }
    if let Some(tags_csv) = &params.exclude_tag {
        for t in tags_csv.split(',').filter(|s| !s.trim().is_empty()) {
            qb.push(
                " AND NOT EXISTS (SELECT 1 FROM entry_tags et JOIN tags tg ON tg.id = et.tag_id \
                 WHERE et.entry_id = entries.id AND tg.user_id = ",
            );
            qb.push_bind(user_id);
            qb.push(" AND tg.label = ");
            qb.push_bind(t.trim().to_string());
            qb.push(")");
        }
    }
    if let Some(s) = &params.search {
        if !s.is_empty() {
            qb.push(" AND (title ILIKE ");
            qb.push_bind(format!("%{s}%"));
            qb.push(" OR content ILIKE ");
            qb.push_bind(format!("%{s}%"));
            qb.push(")");
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateEntryParams {
    pub title: Option<String>, pub content: Option<String>,
    pub is_archived: Option<bool>, pub is_starred: Option<bool>,
}

pub async fn update_entry(pool: &PgPool, user_id: Uuid, entry_id: Uuid, params: &UpdateEntryParams) -> Result<Entry, ModelError> {
    let existing = find_entry_by_id(pool, user_id, entry_id).await?
        .ok_or_else(|| ModelError::NotFound("entry not found".to_string()))?;

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
    .fetch_one(pool).await.map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn list_entries_by_ids(pool: &PgPool, user_id: Uuid, ids: &[Uuid]) -> Result<Vec<EntrySummary>, ModelError> {
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
    .map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn delete_entry(pool: &PgPool, user_id: Uuid, entry_id: Uuid) -> Result<bool, ModelError> {
    let result = sqlx::query("UPDATE entries SET deleted_at = now() WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL")
        .bind(entry_id).bind(user_id).execute(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_deleted_entries(pool: &PgPool, user_id: Uuid) -> Result<Vec<EntrySummary>, ModelError> {
    sqlx::query_as::<_, EntrySummary>(
        "SELECT id, user_id, url, title, content_type, extract_method, language, reading_time, preview_picture, domain_name, published_by, is_archived, is_starred, created_at, deleted_at FROM entries WHERE user_id = $1 AND deleted_at IS NOT NULL ORDER BY deleted_at DESC"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn restore_entry(pool: &PgPool, entry_id: Uuid, user_id: Uuid) -> Result<(), ModelError> {
    let result = sqlx::query("UPDATE entries SET deleted_at = NULL WHERE id = $1 AND user_id = $2 AND deleted_at IS NOT NULL")
        .bind(entry_id).bind(user_id).execute(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    if result.rows_affected() == 0 {
        return Err(ModelError::NotFound("entry not found or not deleted".to_string()));
    }
    Ok(())
}

pub async fn permanently_delete_entry(pool: &PgPool, entry_id: Uuid, user_id: Uuid) -> Result<(), ModelError> {
    let result = sqlx::query("DELETE FROM entries WHERE id = $1 AND user_id = $2 AND deleted_at IS NOT NULL")
        .bind(entry_id).bind(user_id).execute(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    if result.rows_affected() == 0 {
        return Err(ModelError::NotFound("entry not found".to_string()));
    }
    Ok(())
}

/// Find all entry IDs matching the given filter params (no pagination).
pub async fn find_ids_matching(
    pool: &PgPool,
    user_id: Uuid,
    params: &ListParams,
) -> Result<Vec<Uuid>, ModelError> {
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT id FROM entries WHERE user_id = ",
    );
    build_where_clause(&mut qb, user_id, params);

    let rows: Vec<(Uuid,)> = qb.build_query_as().fetch_all(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

pub async fn update_entry_content(
    pool: &PgPool, entry_id: Uuid,
    title: Option<&str>, content: Option<&str>, text_content: Option<&str>,
    language: Option<&str>, preview_picture: Option<&str>, published_by: Option<&str>,
    reading_time: Option<i32>, http_status: i16, extract_method: &str,
) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE entries SET title=COALESCE($2,title), content=$3, text_content=$4, language=$5, preview_picture=$6, published_by=$7, reading_time=$8, http_status=$9, extract_method=$10, updated_at=now() WHERE id=$1 AND is_content_edited=false"
    )
    .bind(entry_id).bind(title).bind(content).bind(text_content).bind(language)
    .bind(preview_picture).bind(published_by).bind(reading_time).bind(http_status).bind(extract_method)
    .execute(pool).await.map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(())
}
