use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Page {
    pub id: Uuid,
    pub slug: String,
    pub user_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub entry_file: String,
    pub password: Option<String>,
    pub status: String,
    pub file_count: i32,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PageSummary {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub has_password: bool,
    pub status: String,
    pub file_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub fn generate_slug() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..12).map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char).collect()
}

pub async fn create_page(
    pool: &PgPool,
    user_id: Uuid,
    title: &str,
    description: Option<&str>,
    entry_file: &str,
    password_hash: Option<&str>,
    file_count: i32,
) -> Result<Page, ApiError> {
    let slug = generate_slug();
    let password = password_hash.map(|s| s.to_string());

    match sqlx::query_as::<_, Page>(
        "INSERT INTO pages (slug, user_id, title, description, entry_file, password, file_count)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *"
    )
    .bind(&slug).bind(user_id).bind(title).bind(description)
    .bind(entry_file).bind(&password).bind(file_count)
    .fetch_one(pool).await {
        Ok(page) => Ok(page),
        Err(sqlx::Error::Database(db_err)) if db_err.constraint() == Some("pages_slug_key") => {
            Err(ApiError::Conflict("slug collision, please retry".to_string()))
        }
        Err(e) => Err(ApiError::from(e)),
    }
}

pub async fn create_page_with_retry(
    pool: &PgPool,
    user_id: Uuid,
    title: &str,
    description: Option<&str>,
    entry_file: &str,
    password_hash: Option<&str>,
    file_count: i32,
) -> Result<Page, ApiError> {
    for _ in 0..5 {
        match create_page(pool, user_id, title, description, entry_file, password_hash, file_count).await {
            Ok(page) => return Ok(page),
            Err(ApiError::Conflict(_)) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(ApiError::Internal("failed to generate unique slug".to_string()))
}

pub async fn find_page_by_id(pool: &PgPool, user_id: Uuid, page_id: Uuid) -> Result<Option<Page>, ApiError> {
    sqlx::query_as::<_, Page>("SELECT * FROM pages WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL")
        .bind(page_id).bind(user_id)
        .fetch_optional(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn find_page_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Page>, ApiError> {
    sqlx::query_as::<_, Page>(
        "SELECT * FROM pages WHERE slug = $1 AND deleted_at IS NULL AND status = 'active'"
    )
    .bind(slug)
    .fetch_optional(pool).await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn list_pages(
    pool: &PgPool,
    user_id: Uuid,
    status: Option<&str>,
    page: i64,
    limit: i64,
) -> Result<(Vec<PageSummary>, i64), ApiError> {
    let limit = limit.min(100).max(1);
    let offset = (page.max(1) - 1) * limit;

    let (items, count) = match status {
        Some("deleted") => {
            let items = sqlx::query_as::<_, PageSummary>(
                "SELECT id, slug, title, description, password IS NOT NULL as has_password, 'deleted' as status, file_count, created_at, updated_at
                 FROM pages WHERE user_id = $1 AND deleted_at IS NOT NULL ORDER BY deleted_at DESC LIMIT $2 OFFSET $3"
            ).bind(user_id).bind(limit).bind(offset).fetch_all(pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pages WHERE user_id = $1 AND deleted_at IS NOT NULL")
                .bind(user_id).fetch_one(pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            (items, count)
        }
        Some(s) => {
            let items = sqlx::query_as::<_, PageSummary>(
                "SELECT id, slug, title, description, password IS NOT NULL as has_password, status, file_count, created_at, updated_at
                 FROM pages WHERE user_id = $1 AND deleted_at IS NULL AND status = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4"
            ).bind(user_id).bind(s).bind(limit).bind(offset).fetch_all(pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pages WHERE user_id = $1 AND deleted_at IS NULL AND status = $2")
                .bind(user_id).bind(s).fetch_one(pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            (items, count)
        }
        None => {
            let items = sqlx::query_as::<_, PageSummary>(
                "SELECT id, slug, title, description, password IS NOT NULL as has_password, status, file_count, created_at, updated_at
                 FROM pages WHERE user_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC LIMIT $2 OFFSET $3"
            ).bind(user_id).bind(limit).bind(offset).fetch_all(pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pages WHERE user_id = $1 AND deleted_at IS NULL")
                .bind(user_id).fetch_one(pool).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            (items, count)
        }
    };
    Ok((items, count))
}

#[derive(Debug, Deserialize)]
pub struct UpdatePageParams {
    pub title: Option<String>,
    pub description: Option<String>,
    pub password: Option<Option<String>>,
    pub status: Option<String>,
}

pub async fn update_page(
    pool: &PgPool,
    user_id: Uuid,
    page_id: Uuid,
    params: &UpdatePageParams,
) -> Result<Page, ApiError> {
    let existing = find_page_by_id(pool, user_id, page_id).await?
        .ok_or_else(|| ApiError::NotFound("page not found".to_string()))?;

    let title = params.title.as_deref().unwrap_or(&existing.title);
    let description = params.description.as_deref().or(existing.description.as_deref());
    let status = params.status.as_deref().unwrap_or(&existing.status);
    let password = match &params.password {
        Some(Some(pw)) => Some(crate::auth::password::hash_password(pw).map_err(|_| ApiError::Internal("hash failed".to_string()))?),
        Some(None) => None,
        None => existing.password.clone(),
    };

    sqlx::query_as::<_, Page>(
        "UPDATE pages SET title=$3, description=$4, status=$5, password=$6, updated_at=now() WHERE id=$1 AND user_id=$2 RETURNING *"
    )
    .bind(page_id).bind(user_id).bind(title).bind(description)
    .bind(status).bind(password)
    .fetch_one(pool).await.map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn delete_page(pool: &PgPool, user_id: Uuid, page_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("UPDATE pages SET deleted_at = now() WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL")
        .bind(page_id).bind(user_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

pub async fn restore_page(pool: &PgPool, user_id: Uuid, page_id: Uuid) -> Result<(), ApiError> {
    let result = sqlx::query("UPDATE pages SET deleted_at = NULL WHERE id = $1 AND user_id = $2 AND deleted_at IS NOT NULL")
        .bind(page_id).bind(user_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("page not found or not deleted".to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_slug_format() {
        let slug = generate_slug();
        assert_eq!(slug.len(), 12);
        assert!(slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    }

    #[test]
    fn test_generate_slug_uniqueness() {
        let slugs: std::collections::HashSet<String> = (0..100).map(|_| generate_slug()).collect();
        assert_eq!(slugs.len(), 100);
    }
}
