use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::{AuthSource, AuthUser};
use crate::state::AppState;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::site_rule::{self, CreateSiteRule, UpdateSiteRule};

use super::validate::ValidatedJson;

pub async fn list_rules(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<site_rule::SiteRule>>, ApiError> {
    let rules = site_rule::list_rules(&state.pool, auth.user_id).await?;
    Ok(Json(rules))
}

fn auth_source_str(auth: &AuthUser) -> String {
    match auth.source {
        AuthSource::Jwt => "jwt".to_string(),
        AuthSource::Pat { .. } => "pat".to_string(),
    }
}

pub async fn create_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(params): ValidatedJson<CreateSiteRule>,
) -> Result<Json<site_rule::SiteRule>, ApiError> {
    let rule = site_rule::create_rule(&state.pool, auth.user_id, &params).await?;
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::CreateSiteRule,
            resource_type: Some(AuditResourceType::SiteRule),
            resource_id: Some(rule.id),
            status: "success".to_string(),
            details: serde_json::json!({"domain": rule.domain}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;
    Ok(Json(rule))
}

pub async fn update_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(rule_id): Path<Uuid>,
    Json(params): Json<UpdateSiteRule>,
) -> Result<Json<site_rule::SiteRule>, ApiError> {
    let updated = site_rule::update_rule(&state.pool, auth.user_id, rule_id, &params).await?;
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::UpdateSiteRule,
            resource_type: Some(AuditResourceType::SiteRule),
            resource_id: Some(rule_id),
            status: "success".to_string(),
            details: serde_json::json!({}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;
    Ok(Json(updated))
}

pub async fn delete_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(rule_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = site_rule::delete_rule(&state.pool, auth.user_id, rule_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("site rule not found".to_string()));
    }
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::DeleteSiteRule,
            resource_type: Some(AuditResourceType::SiteRule),
            resource_id: Some(rule_id),
            status: "success".to_string(),
            details: serde_json::json!({}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;
    Ok(Json(serde_json::json!({"message": "deleted"})))
}
