use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use super::error::ModelError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Annotation {
    pub id: Uuid,
    pub entry_id: Uuid,
    pub user_id: Uuid,
    pub quote: String,
    pub text: String,
    pub ranges: serde_json::Value,
    pub is_orphaned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize, Validate)]
pub struct CreateAnnotation {
    #[validate(length(min = 1, message = "quote is required"))]
    pub quote: String,
    pub text: Option<String>,
    pub ranges: serde_json::Value,
}

#[derive(Deserialize)]
pub struct UpdateAnnotation {
    pub text: Option<String>,
    pub ranges: Option<serde_json::Value>,
}

pub async fn list_by_entry(pool: &PgPool, entry_id: Uuid, user_id: Uuid) -> Result<Vec<Annotation>, ModelError> {
    sqlx::query_as::<_, Annotation>("SELECT * FROM annotations WHERE entry_id = $1 AND user_id = $2 ORDER BY created_at")
        .bind(entry_id).bind(user_id).fetch_all(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn create(pool: &PgPool, entry_id: Uuid, user_id: Uuid, params: &CreateAnnotation) -> Result<Annotation, ModelError> {
    sqlx::query_as::<_, Annotation>(
        "INSERT INTO annotations (entry_id, user_id, quote, text, ranges) VALUES ($1,$2,$3,$4,$5) RETURNING *")
        .bind(entry_id).bind(user_id).bind(&params.quote)
        .bind(params.text.as_deref().unwrap_or("")).bind(&params.ranges)
        .fetch_one(pool).await.map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn update(pool: &PgPool, annotation_id: Uuid, user_id: Uuid, params: &UpdateAnnotation) -> Result<Annotation, ModelError> {
    let existing = sqlx::query_as::<_, Annotation>("SELECT * FROM annotations WHERE id = $1 AND user_id = $2")
        .bind(annotation_id).bind(user_id).fetch_optional(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?
        .ok_or_else(|| ModelError::NotFound("annotation not found".to_string()))?;

    let text = params.text.as_deref().unwrap_or(&existing.text);
    let ranges = params.ranges.as_ref().unwrap_or(&existing.ranges);

    sqlx::query_as::<_, Annotation>(
        "UPDATE annotations SET text = $3, ranges = $4, updated_at = now() WHERE id = $1 AND user_id = $2 RETURNING *")
        .bind(annotation_id).bind(user_id).bind(text).bind(ranges)
        .fetch_one(pool).await.map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn delete(pool: &PgPool, annotation_id: Uuid, user_id: Uuid) -> Result<bool, ModelError> {
    let result = sqlx::query("DELETE FROM annotations WHERE id = $1 AND user_id = $2")
        .bind(annotation_id).bind(user_id).execute(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_annotation_validation_empty_quote() {
        let annotation = CreateAnnotation {
            quote: "".to_string(),
            text: None,
            ranges: serde_json::json!([]),
        };
        assert!(annotation.validate().is_err());
    }

    #[test]
    fn create_annotation_validation_valid() {
        let annotation = CreateAnnotation {
            quote: "some quoted text".to_string(),
            text: Some("a note".to_string()),
            ranges: serde_json::json!([]),
        };
        assert!(annotation.validate().is_ok());
    }
}
