use axum::Json;
use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use validator::Validate;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::auth::jwt;
use crate::auth::middleware::AuthUser;
use crate::auth::password;
use crate::models::audit_log::{self, AuditAction, AuditDetails, AuditResourceType};
use crate::models::user;
use crate::state::AppState;

use super::validate::ValidatedJson;

/// Extract client IP from headers (X-Forwarded-For / X-Real-IP) or
/// fall back to ConnectInfo. Truncates to 45 chars for storage safety.
fn extract_ip(
    headers: &HeaderMap,
    trust_proxy: bool,
    peer_addr: Option<&SocketAddr>,
) -> Option<String> {
    if trust_proxy {
        if let Some(xff) = headers.get("x-forwarded-for")
            && let Ok(val) = xff.to_str()
            && let Some(ip) = val.split(',').next()
        {
            let trimmed = ip.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.chars().take(45).collect());
            }
        }
        if let Some(xri) = headers.get("x-real-ip")
            && let Ok(val) = xri.to_str()
        {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.chars().take(45).collect());
            }
        }
    }
    peer_addr.map(|a| a.ip().to_string())
}

/// Extract User-Agent header, truncated to 255 chars.
fn extract_ua(headers: &HeaderMap) -> Option<String> {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(255).collect())
}

#[derive(Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(length(min = 1, max = 50, message = "username must be 1-50 characters"))]
    pub username: String,
    #[validate(email(message = "invalid email format"))]
    pub email: String,
    #[validate(length(min = 8, max = 128, message = "password must be 8-128 characters"))]
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

#[tracing::instrument(skip(state, headers, req), err)]
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    ValidatedJson(req): ValidatedJson<RegisterRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    if state.config.disable_registration {
        return Err(ApiError::Forbidden("registration is disabled".to_string()));
    }

    // First user becomes admin
    let user_count = user::count_users(&state.pool).await?;
    let is_admin = user_count == 0;

    let new_user = user::create_user(
        &state.pool,
        &req.username,
        &req.email,
        &req.password,
        is_admin,
    )
    .await?;

    tracing::info!(user_id = %new_user.id, "user registered");

    let ip = extract_ip(&headers, state.config.trust_proxy, Some(&addr));
    let ua = extract_ua(&headers);
    audit_log::log_success_with_context(
        &state.pool,
        Some(new_user.id),
        "jwt".to_string(),
        AuditAction::Register,
        Some(AuditResourceType::User),
        Some(new_user.id),
        serde_json::to_value(AuditDetails {
            after: Some(
                serde_json::json!({"username": new_user.username, "email": new_user.email}),
            ),
            ..Default::default()
        })
        .unwrap_or_default(),
        ip,
        ua,
    )
    .await;

    issue_tokens(&state, new_user.id, new_user.is_admin, None).await
}

#[tracing::instrument(skip(state, headers, req), err)]
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let found = match user::find_user_by_email(&state.pool, &req.email).await? {
        Some(u) => u,
        None => {
            // User not found — verify against a dummy hash to prevent timing
            // attacks that would reveal whether an email is registered.
            let _ = password::verify_password(&req.password, password::DUMMY_HASH);
            let ip = extract_ip(&headers, state.config.trust_proxy, Some(&addr));
            let ua = extract_ua(&headers);
            audit_log::log_success_with_context(
                &state.pool,
                None,
                "jwt".to_string(),
                AuditAction::Login,
                None,
                None,
                serde_json::json!({"email": req.email, "reason": "user_not_found"}),
                ip,
                ua,
            )
            .await;
            return Err(ApiError::Unauthorized("邮箱或密码错误".to_string()));
        }
    };

    let ci = Some(addr);
    password::verify_password(&req.password, &found.password_hash).map_err(|_| {
        let ip = extract_ip(&headers, state.config.trust_proxy, ci.as_ref());
        let ua = extract_ua(&headers);
        let pool = state.pool.clone();
        let email = req.email.clone();
        tokio::spawn(async move {
            audit_log::log_success_with_context(
                &pool,
                None,
                "jwt".to_string(),
                AuditAction::Login,
                None,
                None,
                serde_json::json!({"email": email, "reason": "wrong_password"}),
                ip,
                ua,
            )
            .await;
        });
        ApiError::Unauthorized("邮箱或密码错误".to_string())
    })?;

    tracing::info!(user_id = %found.id, "user logged in");

    let ip = extract_ip(&headers, state.config.trust_proxy, ci.as_ref());
    let ua = extract_ua(&headers);
    audit_log::log_success_with_context(
        &state.pool,
        Some(found.id),
        "jwt".to_string(),
        AuditAction::Login,
        Some(AuditResourceType::User),
        Some(found.id),
        serde_json::json!({}),
        ip,
        ua,
    )
    .await;

    issue_tokens(&state, found.id, found.is_admin, None).await
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let token_hash = jwt::hash_refresh_token(&req.refresh_token);

    let stored = match user::find_refresh_token(&state.pool, &token_hash).await? {
        Some(s) => s,
        None => {
            // Token not found — possible replay. Try to find the family from
            // a recently-deleted token so we can revoke the whole family.
            tracing::warn!("refresh token not found (possible reuse detected)");
            return Err(ApiError::Unauthorized("invalid refresh token".to_string()));
        }
    };

    // Delete old refresh token (rotation).
    let deleted = user::delete_refresh_token(&state.pool, &token_hash).await?;
    if deleted.rows_affected() == 0 {
        // Token was already consumed — replay attack. Revoke entire family.
        tracing::warn!(
            family = %stored.family,
            user_id = %stored.user_id,
            "refresh token replay detected, revoking entire family"
        );
        user::revoke_token_family(&state.pool, stored.family).await?;
        return Err(ApiError::Unauthorized("invalid refresh token".to_string()));
    }

    // Find user to get current is_admin status
    let found = user::find_user_by_id(&state.pool, stored.user_id)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("user not found".to_string()))?;

    issue_tokens(&state, found.id, found.is_admin, Some(stored.family)).await
}

pub async fn logout(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    let token_hash = jwt::hash_refresh_token(&req.refresh_token);
    user::delete_refresh_token(&state.pool, &token_hash).await?;

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::Logout,
        Some(AuditResourceType::User),
        Some(auth.user_id),
        serde_json::json!({}),
    )
    .await;

    Ok(Json(MessageResponse {
        message: "logged out".to_string(),
    }))
}

async fn issue_tokens(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    existing_family: Option<uuid::Uuid>,
) -> Result<Json<AuthResponse>, ApiError> {
    let access_token = jwt::create_access_token(user_id, is_admin, &state.config.jwt_secret)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let refresh_token_raw = jwt::generate_refresh_token();
    let refresh_token_hash = jwt::hash_refresh_token(&refresh_token_raw);
    let expires_at = Utc::now() + Duration::days(30);
    let family = existing_family.unwrap_or_else(uuid::Uuid::new_v4);

    user::store_refresh_token(
        &state.pool,
        user_id,
        &refresh_token_hash,
        expires_at,
        family,
    )
    .await?;

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

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        "jwt".to_string(),
        AuditAction::RegenerateFeedToken,
        Some(AuditResourceType::User),
        Some(auth.user_id),
        serde_json::json!({}),
    )
    .await;

    Ok(Json(FeedTokenResponse {
        feed_token: new_token,
    }))
}

#[derive(Serialize)]
pub struct MeResponse {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub is_admin: bool,
    pub auth_source: &'static str,
}

pub async fn me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<MeResponse>, ApiError> {
    let (username, email): (String, String) =
        sqlx::query_as("SELECT username, email FROM users WHERE id = $1")
            .bind(auth.user_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

    let auth_source = match auth.source {
        crate::auth::middleware::AuthSource::Jwt => "jwt",
        crate::auth::middleware::AuthSource::Pat { .. } => "pat",
    };

    Ok(Json(MeResponse {
        user_id: auth.user_id,
        username,
        email,
        is_admin: auth.is_admin,
        auth_source,
    }))
}

#[derive(Deserialize, Validate)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 8, max = 128, message = "password must be 8-128 characters"))]
    pub current_password: String,
    #[validate(length(min = 8, max = 128, message = "password must be 8-128 characters"))]
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

    // Revoke all refresh tokens so other sessions are terminated
    crate::models::user::revoke_all_refresh_tokens(&state.pool, auth.user_id).await?;

    let auth_source = match auth.source {
        crate::auth::middleware::AuthSource::Jwt => "jwt".to_string(),
        crate::auth::middleware::AuthSource::Pat { .. } => "pat".to_string(),
    };
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source,
        AuditAction::ChangePassword,
        Some(AuditResourceType::User),
        Some(auth.user_id),
        serde_json::json!({}),
    )
    .await;

    Ok(Json(MessageResponse {
        message: "password changed".to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn peer() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 1234)
    }

    // --- extract_ip ---

    #[test]
    fn extract_ip_trust_proxy_xff_single() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("1.2.3.4"));
        let result = extract_ip(&headers, true, Some(&peer()));
        assert_eq!(result, Some("1.2.3.4".to_string()));
    }

    #[test]
    fn extract_ip_trust_proxy_xff_multiple() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("  1.2.3.4 , 5.6.7.8 , 9.10.11.12 "),
        );
        let result = extract_ip(&headers, true, Some(&peer()));
        assert_eq!(result, Some("1.2.3.4".to_string()));
    }

    #[test]
    fn extract_ip_trust_proxy_x_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("5.6.7.8"));
        let result = extract_ip(&headers, true, Some(&peer()));
        assert_eq!(result, Some("5.6.7.8".to_string()));
    }

    #[test]
    fn extract_ip_trust_proxy_xff_priority_over_x_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("1.2.3.4"));
        headers.insert("x-real-ip", HeaderValue::from_static("5.6.7.8"));
        let result = extract_ip(&headers, true, Some(&peer()));
        assert_eq!(result, Some("1.2.3.4".to_string()));
    }

    #[test]
    fn extract_ip_no_trust_proxy_falls_back_to_peer_addr() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("1.2.3.4"));
        let result = extract_ip(&headers, false, Some(&peer()));
        assert_eq!(result, Some("10.0.0.1".to_string()));
    }

    #[test]
    fn extract_ip_trust_proxy_no_headers_falls_back_to_peer_addr() {
        let headers = HeaderMap::new();
        let result = extract_ip(&headers, true, Some(&peer()));
        assert_eq!(result, Some("10.0.0.1".to_string()));
    }

    #[test]
    fn extract_ip_trust_proxy_no_headers_no_peer() {
        let headers = HeaderMap::new();
        let result = extract_ip(&headers, true, None);
        assert_eq!(result, None);
    }

    #[test]
    fn extract_ip_truncates_to_45_chars() {
        let long_ip = "a".repeat(60);
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_str(&long_ip).unwrap());
        let result = extract_ip(&headers, true, Some(&peer()));
        assert_eq!(result, Some("a".repeat(45)));
    }

    #[test]
    fn extract_ip_xff_empty_after_comma_falls_through_to_x_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static(" ,5.6.7.8"));
        headers.insert("x-real-ip", HeaderValue::from_static("9.10.11.12"));
        let result = extract_ip(&headers, true, Some(&peer()));
        assert_eq!(result, Some("9.10.11.12".to_string()));
    }

    // --- extract_ua ---

    #[test]
    fn extract_ua_normal() {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", HeaderValue::from_static("Mozilla/5.0"));
        let result = extract_ua(&headers);
        assert_eq!(result, Some("Mozilla/5.0".to_string()));
    }

    #[test]
    fn extract_ua_truncates_to_255_chars() {
        let long_ua = "a".repeat(300);
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", HeaderValue::from_str(&long_ua).unwrap());
        let result = extract_ua(&headers);
        assert_eq!(result, Some("a".repeat(255)));
    }

    #[test]
    fn extract_ua_missing() {
        let headers = HeaderMap::new();
        let result = extract_ua(&headers);
        assert_eq!(result, None);
    }
}
