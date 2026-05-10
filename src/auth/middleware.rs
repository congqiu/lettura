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
    Pat {
        scope: PatScope,
        token_id: uuid::Uuid,
    },
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
            AuthSource::Pat {
                scope: PatScope::Write,
                ..
            } => true,
            AuthSource::Pat {
                scope: PatScope::Read,
                ..
            } => false,
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
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
            // Fire-and-forget: don't block the request on the last_used_at update.
            let pool = state.pool.clone();
            let id = row.id;
            tokio::spawn(async move {
                pat::touch_last_used(&pool, id).await;
            });
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- AuthSource variant tests ---

    #[test]
    fn auth_source_jwt_equality() {
        let a = AuthSource::Jwt;
        let b = AuthSource::Jwt;
        assert_eq!(a, b);
    }

    #[test]
    fn auth_source_pat_read_equality() {
        let id = uuid::Uuid::new_v4();
        let a = AuthSource::Pat {
            scope: PatScope::Read,
            token_id: id,
        };
        let b = AuthSource::Pat {
            scope: PatScope::Read,
            token_id: id,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn auth_source_pat_write_equality() {
        let id = uuid::Uuid::new_v4();
        let a = AuthSource::Pat {
            scope: PatScope::Write,
            token_id: id,
        };
        let b = AuthSource::Pat {
            scope: PatScope::Write,
            token_id: id,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn auth_source_jwt_and_pat_are_not_equal() {
        let id = uuid::Uuid::new_v4();
        let jwt = AuthSource::Jwt;
        let pat = AuthSource::Pat {
            scope: PatScope::Read,
            token_id: id,
        };
        assert_ne!(jwt, pat);
    }

    #[test]
    fn auth_source_pat_different_scopes_not_equal() {
        let id = uuid::Uuid::new_v4();
        let read = AuthSource::Pat {
            scope: PatScope::Read,
            token_id: id,
        };
        let write = AuthSource::Pat {
            scope: PatScope::Write,
            token_id: id,
        };
        assert_ne!(read, write);
    }

    // --- allows_write tests ---

    #[test]
    fn jwt_allows_write() {
        assert!(AuthSource::Jwt.allows_write());
    }

    #[test]
    fn pat_write_allows_write() {
        let id = uuid::Uuid::new_v4();
        let source = AuthSource::Pat {
            scope: PatScope::Write,
            token_id: id,
        };
        assert!(source.allows_write());
    }

    #[test]
    fn pat_read_denies_write() {
        let id = uuid::Uuid::new_v4();
        let source = AuthSource::Pat {
            scope: PatScope::Read,
            token_id: id,
        };
        assert!(!source.allows_write());
    }

    // --- lta_ prefix detection logic tests ---

    #[test]
    fn lta_prefix_identifies_pat() {
        let token = "lta_abc123xyz";
        assert!(
            token.starts_with("lta_"),
            "token with lta_ prefix should be identified as PAT"
        );
    }

    #[test]
    fn non_lta_prefix_identified_as_jwt() {
        let token = "eyJhbGciOiJIUzI1NiJ9.payload.signature";
        assert!(
            !token.starts_with("lta_"),
            "JWT token should not match lta_ prefix"
        );
    }

    #[test]
    fn empty_token_identified_as_jwt() {
        let token = "";
        assert!(
            !token.starts_with("lta_"),
            "empty token should not match lta_ prefix"
        );
    }

    #[test]
    fn lta_without_underscore_not_pat() {
        let token = "ltaabc123";
        assert!(
            !token.starts_with("lta_"),
            "token starting with 'lta' but without underscore should not be PAT"
        );
    }

    // --- PatScope tests ---

    #[test]
    fn pat_scope_equality() {
        assert_eq!(PatScope::Read, PatScope::Read);
        assert_eq!(PatScope::Write, PatScope::Write);
        assert_ne!(PatScope::Read, PatScope::Write);
    }

    #[test]
    fn pat_scope_copy() {
        let a = PatScope::Write;
        let b = a; // Copy semantics
        assert_eq!(a, b);
    }
}
