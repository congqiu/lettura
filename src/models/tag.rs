use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Tag {
    pub id: Uuid,
    pub user_id: Uuid,
    pub label: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
}

pub fn slugify(label: &str) -> String {
    label
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub async fn list_tags(pool: &PgPool, user_id: Uuid) -> Result<Vec<Tag>, ApiError> {
    sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE user_id = $1 ORDER BY label")
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn find_or_create_tag(pool: &PgPool, user_id: Uuid, label: &str) -> Result<Tag, ApiError> {
    let slug = slugify(label);
    if let Some(tag) = sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE user_id = $1 AND slug = $2")
        .bind(user_id).bind(&slug).fetch_optional(pool).await.map_err(|e| ApiError::Internal(e.to_string()))? {
        return Ok(tag);
    }
    sqlx::query_as::<_, Tag>("INSERT INTO tags (user_id, label, slug) VALUES ($1, $2, $3) RETURNING *")
        .bind(user_id).bind(label).bind(&slug).fetch_one(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn add_tag_to_entry(pool: &PgPool, entry_id: Uuid, tag_id: Uuid) -> Result<(), ApiError> {
    sqlx::query("INSERT INTO entry_tags (entry_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(entry_id).bind(tag_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}

pub async fn remove_tag_from_entry(pool: &PgPool, entry_id: Uuid, tag_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("DELETE FROM entry_tags WHERE entry_id = $1 AND tag_id = $2")
        .bind(entry_id).bind(tag_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_tag(pool: &PgPool, user_id: Uuid, tag_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("DELETE FROM tags WHERE id = $1 AND user_id = $2")
        .bind(tag_id).bind(user_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}
