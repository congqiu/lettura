use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const ACCESS_TOKEN_DURATION_MINUTES: i64 = 15;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: i64,
    pub iat: i64,
    pub is_admin: bool,
}

#[derive(Error, Debug)]
pub enum JwtError {
    #[error("failed to create token")]
    CreationError,
    #[error("invalid token")]
    ValidationError,
    #[error("token expired")]
    Expired,
}

pub fn create_access_token(
    user_id: Uuid,
    is_admin: bool,
    secret: &str,
) -> Result<String, JwtError> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        exp: (now + Duration::minutes(ACCESS_TOKEN_DURATION_MINUTES)).timestamp(),
        iat: now.timestamp(),
        is_admin,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| JwtError::CreationError)
}

pub fn validate_token(token: &str, secret: &str) -> Result<Claims, JwtError> {
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_required_spec_claims(&["exp", "iat", "sub"]);
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::Expired,
        _ => JwtError::ValidationError,
    })?;
    Ok(data.claims)
}

pub fn generate_refresh_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.r#gen();
    hex::encode(bytes)
}

pub fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_validate_access_token() {
        let user_id = Uuid::new_v4();
        let secret = "test-secret-at-least-32-characters-long";
        let token = create_access_token(user_id, false, secret).unwrap();
        let claims = validate_token(&token, secret).unwrap();
        assert_eq!(claims.sub, user_id);
        assert!(!claims.is_admin);
    }

    #[test]
    fn admin_claim_roundtrips() {
        let user_id = Uuid::new_v4();
        let secret = "test-secret-at-least-32-characters-long";
        let token = create_access_token(user_id, true, secret).unwrap();
        let claims = validate_token(&token, secret).unwrap();
        assert!(claims.is_admin);
    }

    #[test]
    fn wrong_secret_fails() {
        let user_id = Uuid::new_v4();
        let token =
            create_access_token(user_id, false, "secret-one-at-least-32-characters").unwrap();
        let result = validate_token(&token, "secret-two-at-least-32-characters");
        assert!(result.is_err());
    }

    #[test]
    fn refresh_token_is_random() {
        let t1 = generate_refresh_token();
        let t2 = generate_refresh_token();
        assert_ne!(t1, t2);
        assert!(t1.len() >= 32);
    }

    #[test]
    fn refresh_token_hash_is_deterministic() {
        let token = "some-refresh-token";
        let h1 = hash_refresh_token(token);
        let h2 = hash_refresh_token(token);
        assert_eq!(h1, h2);
        assert_ne!(h1, token);
    }
}
