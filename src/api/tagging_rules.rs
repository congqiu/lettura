use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::tagging_rule::{self, CreateTaggingRule, UpdateTaggingRule};
use crate::state::AppState;

use super::validate::ValidatedJson;

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
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::CreateTaggingRule,
        Some(AuditResourceType::TaggingRule),
        Some(rule.id),
        serde_json::json!({}),
    )
    .await;
    Ok(Json(rule))
}

pub async fn update_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(rule_id): Path<Uuid>,
    Json(params): Json<UpdateTaggingRule>,
) -> Result<Json<tagging_rule::TaggingRule>, ApiError> {
    let updated = tagging_rule::update_rule(&state.pool, auth.user_id, rule_id, &params).await?;
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::UpdateTaggingRule,
        Some(AuditResourceType::TaggingRule),
        Some(rule_id),
        serde_json::json!({}),
    )
    .await;
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
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::DeleteTaggingRule,
        Some(AuditResourceType::TaggingRule),
        Some(rule_id),
        serde_json::json!({}),
    )
    .await;
    Ok(Json(serde_json::json!({"message": "deleted"})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn create_tagging_rule_request_validation_empty_tags() {
        let params = CreateTaggingRule {
            rule: serde_json::json!({"operator": "AND", "conditions": []}),
            tags: vec![],
            priority: None,
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn create_tagging_rule_request_validation_valid() {
        let params = CreateTaggingRule {
            rule: serde_json::json!({"operator": "AND", "conditions": []}),
            tags: vec!["rust".to_string()],
            priority: Some(1),
        };
        assert!(params.validate().is_ok());
    }
}
