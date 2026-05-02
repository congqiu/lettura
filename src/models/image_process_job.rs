use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::error::ModelError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ImageProcessJob {
    pub id: Uuid,
    pub entry_id: Uuid,
    pub original_html: String,
    pub status: String,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn create(
    pool: &PgPool,
    entry_id: Uuid,
    original_html: &str,
) -> Result<ImageProcessJob, ModelError> {
    sqlx::query_as::<_, ImageProcessJob>(
        r#"
        INSERT INTO image_process_jobs (entry_id, original_html)
        VALUES ($1, $2)
        RETURNING *
        "#,
    )
    .bind(entry_id)
    .bind(original_html)
    .fetch_one(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))
}

/// Claim a pending job for processing. Returns None if no jobs available.
pub async fn claim_pending(pool: &PgPool) -> Result<Option<ImageProcessJob>, ModelError> {
    sqlx::query_as::<_, ImageProcessJob>(
        r#"
        UPDATE image_process_jobs
        SET status = 'processing', updated_at = NOW()
        WHERE id = (
            SELECT id FROM image_process_jobs
            WHERE status = 'pending'
            ORDER BY created_at
            LIMIT 1
            FOR UPDATE SKIP LOCKED
        )
        RETURNING *
        "#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn mark_completed(pool: &PgPool, job_id: Uuid) -> Result<(), ModelError> {
    sqlx::query(
        r#"
        UPDATE image_process_jobs
        SET status = 'completed', updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .execute(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(())
}

pub async fn mark_failed(
    pool: &PgPool,
    job_id: Uuid,
    error_message: &str,
    max_retries: i32,
) -> Result<(), ModelError> {
    // Check retry count and either retry or mark as permanently failed
    sqlx::query(
        r#"
        UPDATE image_process_jobs
        SET
            status = CASE WHEN retry_count >= $3 THEN 'failed'::image_process_status ELSE 'pending'::image_process_status END,
            error_message = $2,
            retry_count = retry_count + 1,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(error_message)
    .bind(max_retries)
    .execute(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(())
}

/// Get the status of image processing for an entry.
pub async fn get_status_for_entry(
    pool: &PgPool,
    entry_id: Uuid,
) -> Result<Option<String>, ModelError> {
    let result: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT status FROM image_process_jobs
        WHERE entry_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(entry_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;

    Ok(result.map(|(s,)| s))
}

/// Delete completed jobs older than the specified duration (cleanup).
pub async fn cleanup_completed(
    pool: &PgPool,
    older_than: chrono::Duration,
) -> Result<u64, ModelError> {
    let cutoff = Utc::now() - older_than;
    let result = sqlx::query(
        r#"
        DELETE FROM image_process_jobs
        WHERE status = 'completed' AND updated_at < $1
        "#,
    )
    .bind(cutoff)
    .execute(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;

    Ok(result.rows_affected())
}
