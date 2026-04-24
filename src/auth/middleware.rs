use axum::extract::FromRequestParts;
use axum::http::{Method, request::Parts};

use crate::api::error::ApiError;
use crate::auth::jwt;
use crate::models::pat;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: uuid::Uuid,
    pub is_admin: bool,
    pub source: AuthSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthSource {
    Jwt,
    Pat { scope: PatScope, token_id: uuid::Uuid },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatScope {
    Read,
    Write,
}

impl AuthSource {
    pub fn allows_write(&self) -> bool {
        match self {
            AuthSource::Jwt => true,
            AuthSource::Pat { scope: PatScope::Write, .. } => true,
            AuthSource::Pat { scope: PatScope::Read, .. } => false,
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .ok_or_else(|| ApiError::Unauthorized("missing authorization header".into()))?;

        if token.starts_with("lta_") {
            let hash = pat::hash_token(token);
            let row = pat::find_valid_by_hash(&state.pool, &hash)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?
                .ok_or_else(|| ApiError::Unauthorized("invalid or expired token".into()))?;
            let scope = match row.scope.as_str() {
                "write" => PatScope::Write,
                _ => PatScope::Read,
            };
            // Scope enforcement by HTTP method: read tokens can only do GET/HEAD.
            if !matches!(parts.method, Method::GET | Method::HEAD) && scope == PatScope::Read {
                return Err(ApiError::Forbidden(
                    "token scope 'read' cannot perform write".into(),
                ));
            }
            pat::touch_last_used(&state.pool, row.id).await;
            Ok(AuthUser {
                user_id: row.user_id,
                is_admin: false, // admin capability requires interactive JWT login
                source: AuthSource::Pat {
                    scope,
                    token_id: row.id,
                },
            })
        } else {
            let claims = jwt::validate_token(token, &state.config.jwt_secret)
                .map_err(|e| ApiError::Unauthorized(e.to_string()))?;
            Ok(AuthUser {
                user_id: claims.sub,
                is_admin: claims.is_admin,
                source: AuthSource::Jwt,
            })
        }
    }
}
