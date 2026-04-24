use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::api::error::ApiError;
use crate::auth::middleware::{AuthSource, AuthUser};
use crate::models::pat;
use crate::state::AppState;

use super::validate::ValidatedJson;

/// Ensure the request was authenticated via a JWT (interactive login),
/// not a Personal Access Token.  Token management must be done interactively.
fn require_jwt(auth: &AuthUser) -> Result<(), ApiError> {
    match auth.source {
        AuthSource::Jwt => Ok(()),
        AuthSource::Pat { .. } => Err(ApiError::Forbidden(
            "token management requires interactive login".into(),
        )),
    }
}

fn validate_scope(s: &str) -> Result<(), validator::ValidationError> {
    match s {
        "read" | "write" => Ok(()),
        _ => Err(validator::ValidationError::new("invalid scope")),
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateTokenRequest {
    #[validate(length(min = 1, max = 64))]
    pub name: String,
    #[validate(custom(function = "validate_scope"))]
    pub scope: String,
    pub expires_in_days: Option<i64>,
}

#[derive(Serialize)]
pub struct CreateTokenResponse {
    pub id: uuid::Uuid,
    pub name: String,
    pub scope: String,
    /// Plaintext token — only returned on creation, never again.
    pub token: String,
}

pub async fn create_token(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<CreateTokenRequest>,
) -> Result<(StatusCode, Json<CreateTokenResponse>), ApiError> {
    require_jwt(&auth)?;

    let scope = match req.scope.as_str() {
        "read" => pat::Scope::Read,
        "write" => pat::Scope::Write,
        // unreachable after validation, but fail-safe:
        _ => return Err(ApiError::BadRequest("scope must be 'read' or 'write'".into())),
    };

    let expires_at = req
        .expires_in_days
        .map(|d| chrono::Utc::now() + chrono::Duration::days(d));

    let token = pat::generate_token();
    let hash = pat::hash_token(&token);
    let prefix = pat::token_prefix(&token);
    let scope_str = req.scope.clone();

    let id = pat::insert(
        &state.pool,
        auth.user_id,
        &req.name,
        &hash,
        &prefix,
        scope,
        expires_at,
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateTokenResponse {
            id,
            name: req.name,
            scope: scope_str,
            token,
        }),
    ))
}

pub async fn list_tokens(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<pat::PersonalAccessToken>>, ApiError> {
    require_jwt(&auth)?;

    let rows = pat::list_for_user(&state.pool, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(rows))
}

pub async fn delete_token(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, ApiError> {
    require_jwt(&auth)?;

    if pat::delete(&state.pool, auth.user_id, id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("token".into()))
    }
}
