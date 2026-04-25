use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::error::ModelError;
use crate::auth::password;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub feed_token: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RefreshToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

pub async fn create_user(
    pool: &PgPool,
    username: &str,
    email: &str,
    raw_password: &str,
    is_admin: bool,
) -> Result<User, ModelError> {
    let password_hash =
        password::hash_password(raw_password).map_err(|e| ModelError::Database(e.to_string()))?;

    Ok(sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (username, email, password_hash, is_admin)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(username)
    .bind(email)
    .bind(&password_hash)
    .bind(is_admin)
    .fetch_one(pool)
    .await?)
}

pub async fn find_user_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, ModelError> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn find_user_by_id(pool: &PgPool, id: Uuid) -> Result<Option<User>, ModelError> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn count_users(pool: &PgPool) -> Result<i64, ModelError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(row.0)
}

pub async fn store_refresh_token(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), ModelError> {
    sqlx::query(
        "INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(())
}

pub async fn find_refresh_token(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<RefreshToken>, ModelError> {
    sqlx::query_as::<_, RefreshToken>(
        "SELECT * FROM refresh_tokens WHERE token_hash = $1 AND expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn delete_refresh_token(pool: &PgPool, token_hash: &str) -> Result<(), ModelError> {
    sqlx::query("DELETE FROM refresh_tokens WHERE token_hash = $1")
        .bind(token_hash)
        .execute(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(())
}

pub async fn regenerate_feed_token(pool: &PgPool, user_id: Uuid) -> Result<String, ModelError> {
    let new_token = generate_feed_token();
    let row: (String,) = sqlx::query_as(
        "UPDATE users SET feed_token = $1 WHERE id = $2 RETURNING feed_token",
    )
    .bind(&new_token)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(row.0)
}

pub async fn cleanup_expired_refresh_tokens(pool: &PgPool) -> Result<u64, ModelError> {
    let result = sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < now()")
        .execute(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(result.rows_affected())
}

pub async fn update_password(pool: &PgPool, user_id: Uuid, password_hash: &str) -> Result<(), ModelError> {
    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(password_hash)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(())
}

fn generate_feed_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}
