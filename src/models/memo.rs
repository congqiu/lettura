use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Memo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub source_url: Option<String>,
    pub promoted_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateMemo {
    pub content: String,
    pub source_url: Option<String>,
}

pub async fn list_memos(pool: &PgPool, user_id: Uuid) -> Result<Vec<Memo>, ApiError> {
    sqlx::query_as::<_, Memo>("SELECT * FROM memos WHERE user_id = $1 ORDER BY created_at DESC")
        .bind(user_id).fetch_all(pool).await.map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn create_memo(pool: &PgPool, user_id: Uuid, params: &CreateMemo) -> Result<Memo, ApiError> {
    sqlx::query_as::<_, Memo>("INSERT INTO memos (user_id, content, source_url) VALUES ($1, $2, $3) RETURNING *")
        .bind(user_id).bind(&params.content).bind(params.source_url.as_deref())
        .fetch_one(pool).await.map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn delete_memo(pool: &PgPool, user_id: Uuid, memo_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("DELETE FROM memos WHERE id = $1 AND user_id = $2")
        .bind(memo_id).bind(user_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

pub async fn find_memo_by_id(pool: &PgPool, user_id: Uuid, memo_id: Uuid) -> Result<Option<Memo>, ApiError> {
    sqlx::query_as::<_, Memo>("SELECT * FROM memos WHERE id = $1 AND user_id = $2")
        .bind(memo_id).bind(user_id).fetch_optional(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn set_promoted_entry(pool: &PgPool, memo_id: Uuid, entry_id: Uuid) -> Result<(), ApiError> {
    sqlx::query("UPDATE memos SET promoted_entry_id = $2 WHERE id = $1")
        .bind(memo_id).bind(entry_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(())
}
