use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::{AuthSource, AuthUser};
use crate::state::AppState;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::tagging_rule::{self, CreateTaggingRule, UpdateTaggingRule};

use super::validate::ValidatedJson;

fn auth_source_str(auth: &AuthUser) -> String {
    match auth.source {
        AuthSource::Jwt => "jwt".to_string(),
        AuthSource::Pat { .. } => "pat".to_string(),
    }
}

pub async fn list_rules(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<tagging_rule::TaggingRule>>, ApiError> {
    let rules = tagging_rule::list_rules(&state.pool, auth.user_id).await?;
    Ok(Json(rules))
}

pub async fn create_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(params): ValidatedJson<CreateTaggingRule>,
) -> Result<Json<tagging_rule::TaggingRule>, ApiError> {
    let rule = tagging_rule::create_rule(&state.pool, auth.user_id, &params).await?;
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::CreateTaggingRule,
            resource_type: Some(AuditResourceType::TaggingRule),
            resource_id: Some(rule.id),
            status: "success".to_string(),
            details: serde_json::json!({}),
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
    Json(params): Json<UpdateTaggingRule>,
) -> Result<Json<tagging_rule::TaggingRule>, ApiError> {
    let updated = tagging_rule::update_rule(&state.pool, auth.user_id, rule_id, &params).await?;
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::UpdateTaggingRule,
            resource_type: Some(AuditResourceType::TaggingRule),
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
    let deleted = tagging_rule::delete_rule(&state.pool, auth.user_id, rule_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("rule not found".to_string()));
    }
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::DeleteTaggingRule,
            resource_type: Some(AuditResourceType::TaggingRule),
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
