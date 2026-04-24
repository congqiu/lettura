use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::error::ModelError;

/// Scope for a Personal Access Token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Read,
    Write,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::Read => "read",
            Scope::Write => "write",
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PersonalAccessToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub token_prefix: String,
    #[serde(skip_serializing)]
    pub token_hash: String,
    pub scope: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

const TOKEN_PREFIX: &str = "lta_";
const TOKEN_BODY_LEN: usize = 40;

pub fn generate_token() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    let body: String = (0..TOKEN_BODY_LEN)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect();
    format!("{TOKEN_PREFIX}{body}")
}

pub fn token_prefix(token: &str) -> String {
    token.chars().take(12).collect()
}

pub fn hash_token(token: &str) -> String {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    hex::encode(h.finalize())
}

/// Insert a new PAT and return its generated UUID.
pub async fn insert(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    name: &str,
    hash: &str,
    prefix: &str,
    scope: Scope,
    expires_at: Option<DateTime<Utc>>,
) -> Result<Uuid, ModelError> {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO personal_access_tokens \
         (user_id, name, token_hash, token_prefix, scope, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .bind(hash)
    .bind(prefix)
    .bind(scope.as_str())
    .bind(expires_at)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Look up a PAT by its hash (includes expired tokens).
pub async fn find_by_hash(
    pool: &sqlx::PgPool,
    hash: &str,
) -> Result<Option<PersonalAccessToken>, ModelError> {
    let row = sqlx::query_as::<_, PersonalAccessToken>(
        "SELECT * FROM personal_access_tokens WHERE token_hash = $1",
    )
    .bind(hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Look up a PAT by hash, returning None if the token has expired.
pub async fn find_valid_by_hash(
    pool: &sqlx::PgPool,
    hash: &str,
) -> Result<Option<PersonalAccessToken>, ModelError> {
    let row = sqlx::query_as::<_, PersonalAccessToken>(
        "SELECT * FROM personal_access_tokens \
         WHERE token_hash = $1 \
           AND (expires_at IS NULL OR expires_at > now())",
    )
    .bind(hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// List all PATs for a user, ordered by created_at DESC.
pub async fn list_for_user(
    pool: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<Vec<PersonalAccessToken>, ModelError> {
    let rows = sqlx::query_as::<_, PersonalAccessToken>(
        "SELECT * FROM personal_access_tokens \
         WHERE user_id = $1 \
         ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Delete a PAT by id and user_id.  Returns true if a row was deleted.
pub async fn delete(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    id: Uuid,
) -> Result<bool, ModelError> {
    let result = sqlx::query(
        "DELETE FROM personal_access_tokens WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Best-effort debounced update of last_used_at.  Only updates when the last
/// recorded use is more than 60 seconds ago (or has never been set).
/// Errors are silently swallowed — this must never panic.
pub async fn touch_last_used(pool: &sqlx::PgPool, id: Uuid) {
    let _ = sqlx::query(
        "UPDATE personal_access_tokens \
         SET last_used_at = now() \
         WHERE id = $1 \
           AND (last_used_at IS NULL OR last_used_at < now() - INTERVAL '60 seconds')",
    )
    .bind(id)
    .execute(pool)
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_token_has_lta_prefix() {
        let t = generate_token();
        assert!(t.starts_with("lta_"), "token must start with lta_");
        assert!(t.len() >= 44, "token too short: {}", t.len());
    }

    #[test]
    fn token_prefix_takes_first_12_bytes() {
        let t = "lta_abcdefghijklmnop";
        assert_eq!(token_prefix(t), "lta_abcdefgh");
    }

    #[test]
    fn hash_is_deterministic() {
        let t = "lta_sometoken";
        assert_eq!(hash_token(t), hash_token(t));
    }

    #[test]
    fn hash_differs_for_different_tokens() {
        assert_ne!(hash_token("lta_a"), hash_token("lta_b"));
    }

    #[test]
    fn generated_tokens_are_unique() {
        use std::collections::HashSet;
        let set: HashSet<_> = (0..100).map(|_| generate_token()).collect();
        assert_eq!(set.len(), 100);
    }

    #[test]
    fn personal_access_token_does_not_serialize_token_hash() {
        let pat = PersonalAccessToken {
            id: uuid::Uuid::nil(),
            user_id: uuid::Uuid::nil(),
            name: "t".into(),
            token_prefix: "lta_abc".into(),
            token_hash: "SECRET_HASH_SHOULD_NEVER_LEAK".into(),
            scope: "read".into(),
            last_used_at: None,
            expires_at: None,
            created_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&pat).unwrap();
        assert!(!json.contains("SECRET_HASH_SHOULD_NEVER_LEAK"),
            "token_hash leaked in JSON: {json}");
        assert!(!json.contains("token_hash"),
            "token_hash key appeared in JSON: {json}");
    }
}
