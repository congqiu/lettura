use axum::extract::State;
use axum::Json;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::auth::jwt;
use crate::auth::middleware::{AppState, AuthUser};
use crate::auth::password;
use crate::models::user;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub message: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    if req.username.is_empty() || req.email.is_empty() || req.password.len() < 8 {
        return Err(ApiError::BadRequest(
            "username, email required; password must be >= 8 chars".to_string(),
        ));
    }

    // First user becomes admin
    let user_count = user::count_users(&state.pool).await?;
    let is_admin = user_count == 0;

    let new_user =
        user::create_user(&state.pool, &req.username, &req.email, &req.password, is_admin).await?;

    issue_tokens(&state, new_user.id, new_user.is_admin).await
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let found = user::find_user_by_email(&state.pool, &req.email)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("invalid credentials".to_string()))?;

    password::verify_password(&req.password, &found.password_hash)
        .map_err(|_| ApiError::Unauthorized("invalid credentials".to_string()))?;

    issue_tokens(&state, found.id, found.is_admin).await
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let token_hash = jwt::hash_refresh_token(&req.refresh_token);

    let stored = user::find_refresh_token(&state.pool, &token_hash)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("invalid refresh token".to_string()))?;

    // Delete old refresh token (rotation)
    user::delete_refresh_token(&state.pool, &token_hash).await?;

    // Find user to get current is_admin status
    let found = user::find_user_by_id(&state.pool, stored.user_id)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("user not found".to_string()))?;

    issue_tokens(&state, found.id, found.is_admin).await
}

pub async fn logout(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    let token_hash = jwt::hash_refresh_token(&req.refresh_token);
    user::delete_refresh_token(&state.pool, &token_hash).await?;
    Ok(Json(MessageResponse {
        message: "logged out".to_string(),
    }))
}

async fn issue_tokens(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
) -> Result<Json<AuthResponse>, ApiError> {
    let access_token = jwt::create_access_token(user_id, is_admin, &state.config.jwt_secret)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let refresh_token_raw = jwt::generate_refresh_token();
    let refresh_token_hash = jwt::hash_refresh_token(&refresh_token_raw);
    let expires_at = Utc::now() + Duration::days(30);

    user::store_refresh_token(&state.pool, user_id, &refresh_token_hash, expires_at).await?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token: refresh_token_raw,
        token_type: "Bearer".to_string(),
        expires_in: 900,
    }))
}
