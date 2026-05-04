use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
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

pub async fn create_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(params): ValidatedJson<CreateSiteRule>,
) -> Result<Json<site_rule::SiteRule>, ApiError> {
    let rule = site_rule::create_rule(&state.pool, auth.user_id, &params).await?;
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::CreateSiteRule,
        Some(AuditResourceType::SiteRule),
        Some(rule.id),
        serde_json::json!({"domain": rule.domain}),
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
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::UpdateSiteRule,
        Some(AuditResourceType::SiteRule),
        Some(rule_id),
        serde_json::json!({}),
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
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::DeleteSiteRule,
        Some(AuditResourceType::SiteRule),
        Some(rule_id),
        serde_json::json!({}),
    ).await;
    Ok(Json(serde_json::json!({"message": "deleted"})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn create_site_rule_request_validation_empty_domain() {
        let params = CreateSiteRule {
            domain: "".to_string(),
            content_selector: ".content".to_string(),
            title_selector: None,
            strip_selectors: None,
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn create_site_rule_request_validation_valid() {
        let params = CreateSiteRule {
            domain: "example.com".to_string(),
            content_selector: ".content".to_string(),
            title_selector: Some("h1".to_string()),
            strip_selectors: Some(vec![".ad".to_string()]),
        };
        assert!(params.validate().is_ok());
    }
}