use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use super::error::ModelError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Memo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub source_url: Option<String>,
    pub promoted_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize, Validate)]
pub struct CreateMemo {
    #[validate(length(min = 1, message = "content is required"))]
    pub content: String,
    pub source_url: Option<String>,
}

pub async fn list_memos(pool: &PgPool, user_id: Uuid) -> Result<Vec<Memo>, ModelError> {
    sqlx::query_as::<_, Memo>("SELECT * FROM memos WHERE user_id = $1 ORDER BY created_at DESC")
        .bind(user_id).fetch_all(pool).await.map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn create_memo(pool: &PgPool, user_id: Uuid, params: &CreateMemo) -> Result<Memo, ModelError> {
    sqlx::query_as::<_, Memo>("INSERT INTO memos (user_id, content, source_url) VALUES ($1, $2, $3) RETURNING *")
        .bind(user_id).bind(&params.content).bind(params.source_url.as_deref())
        .fetch_one(pool).await.map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn delete_memo(pool: &PgPool, user_id: Uuid, memo_id: Uuid) -> Result<bool, ModelError> {
    let result = sqlx::query("DELETE FROM memos WHERE id = $1 AND user_id = $2")
        .bind(memo_id).bind(user_id).execute(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

pub async fn find_memo_by_id(pool: &PgPool, user_id: Uuid, memo_id: Uuid) -> Result<Option<Memo>, ModelError> {
    sqlx::query_as::<_, Memo>("SELECT * FROM memos WHERE id = $1 AND user_id = $2")
        .bind(memo_id).bind(user_id).fetch_optional(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn set_promoted_entry(pool: &PgPool, memo_id: Uuid, entry_id: Uuid) -> Result<(), ModelError> {
    sqlx::query("UPDATE memos SET promoted_entry_id = $2 WHERE id = $1")
        .bind(memo_id).bind(entry_id).execute(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_memo_validation_empty_content() {
        let memo = CreateMemo {
            content: "".to_string(),
            source_url: None,
        };
        assert!(memo.validate().is_err());
    }

    #[test]
    fn create_memo_validation_valid() {
        let memo = CreateMemo {
            content: "some memo content".to_string(),
            source_url: Some("https://example.com".to_string()),
        };
        assert!(memo.validate().is_ok());
    }
}
