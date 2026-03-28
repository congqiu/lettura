use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sqlx::PgPool;

use crate::api::error::ApiError;
use crate::auth::jwt;
use crate::config::Config;
use crate::search::SearchIndex;
use crate::storage::ImageStorage;
use crate::tasks::fetcher::FetchQueue;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: uuid::Uuid,
    pub is_admin: bool,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub fetch_queue: FetchQueue,
    pub search_index: SearchIndex,
    pub storage: Arc<dyn ImageStorage>,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::Unauthorized("missing authorization header".to_string()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| ApiError::Unauthorized("invalid authorization format".to_string()))?;

        let claims = jwt::validate_token(token, &state.config.jwt_secret)
            .map_err(|e| ApiError::Unauthorized(e.to_string()))?;

        Ok(AuthUser {
            user_id: claims.sub,
            is_admin: claims.is_admin,
        })
    }
}
