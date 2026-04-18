use axum::extract::State;
use axum::Json;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::api::error::ApiError;
use crate::auth::jwt;
use crate::auth::middleware::AuthUser;
use crate::state::AppState;
use crate::auth::password;
use crate::models::user;

use super::validate::ValidatedJson;

#[derive(Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(length(min = 1, message = "username is required"))]
    pub username: String,
    #[validate(email(message = "invalid email format"))]
    pub email: String,
    #[validate(length(min = 8, message = "password must be at least 8 characters"))]
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

#[tracing::instrument(skip(state, req), err)]
pub async fn register(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<RegisterRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    // First user becomes admin
    let user_count = user::count_users(&state.pool).await?;
    let is_admin = user_count == 0;

    let new_user =
        user::create_user(&state.pool, &req.username, &req.email, &req.password, is_admin).await?;

    tracing::info!(user_id = %new_user.id, "user registered");
    issue_tokens(&state, new_user.id, new_user.is_admin).await
}

#[tracing::instrument(skip(state, req), err)]
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let found = user::find_user_by_email(&state.pool, &req.email)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("invalid credentials".to_string()))?;

    password::verify_password(&req.password, &found.password_hash)
        .map_err(|_| ApiError::Unauthorized("invalid credentials".to_string()))?;

    tracing::info!(user_id = %found.id, "user logged in");
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

#[derive(Serialize)]
pub struct FeedTokenResponse {
    pub feed_token: String,
}

pub async fn regenerate_feed_token(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<FeedTokenResponse>, ApiError> {
    let new_token = user::regenerate_feed_token(&state.pool, auth.user_id).await?;
    Ok(Json(FeedTokenResponse { feed_token: new_token }))
}

#[derive(Deserialize, Validate)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 8, message = "password must be at least 8 characters"))]
    pub current_password: String,
    #[validate(length(min = 8, message = "password must be at least 8 characters"))]
    pub new_password: String,
}

pub async fn change_password(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<ChangePasswordRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    let found = user::find_user_by_id(&state.pool, auth.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".to_string()))?;

    password::verify_password(&req.current_password, &found.password_hash)
        .map_err(|_| ApiError::BadRequest("current password is incorrect".to_string()))?;

    let new_hash = password::hash_password(&req.new_password)
        .map_err(|_| ApiError::Internal("failed to hash password".to_string()))?;

    user::update_password(&state.pool, auth.user_id, &new_hash).await?;

    Ok(Json(MessageResponse {
        message: "password changed".to_string(),
    }))
}
