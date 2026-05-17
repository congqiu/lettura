use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::site_rule::{self, CreateSiteRule, UpdateSiteRule};
use crate::state::AppState;

use super::validate::ValidatedJson;

#[utoipa::path(
    get,
    path = "/api/v1/site-rules",
    operation_id = "list_site_rules",
    tag = "site-rules",
    responses(
        (status = 200, description = "List of site rules", body = Vec<site_rule::SiteRule>),
        (status = 401, description = "Missing or invalid auth"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_rules(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<site_rule::SiteRule>>, ApiError> {
    let rules = site_rule::list_rules(&state.pool, auth.user_id).await?;
    Ok(Json(rules))
}

#[utoipa::path(
    post,
    path = "/api/v1/site-rules",
    operation_id = "create_site_rule",
    tag = "site-rules",
    request_body = site_rule::CreateSiteRule,
    responses(
        (status = 201, description = "Site rule created", body = site_rule::SiteRule),
        (status = 401, description = "Missing or invalid auth"),
        (status = 422, description = "Validation error"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(params): ValidatedJson<CreateSiteRule>,
) -> Result<Json<site_rule::SiteRule>, ApiError> {
    let rule = site_rule::create_rule(&state.pool, &state.caches, auth.user_id, &params).await?;
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::CreateSiteRule,
        Some(AuditResourceType::SiteRule),
        Some(rule.id),
        serde_json::json!({"domain": rule.domain}),
    )
    .await;
    Ok(Json(rule))
}

#[utoipa::path(
    patch,
    path = "/api/v1/site-rules/{id}",
    operation_id = "update_site_rule",
    tag = "site-rules",
    params(("id" = Uuid, Path, description = "Site rule ID")),
    request_body = site_rule::UpdateSiteRule,
    responses(
        (status = 200, description = "Site rule updated", body = site_rule::SiteRule),
        (status = 401, description = "Missing or invalid auth"),
        (status = 404, description = "Rule not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn update_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(rule_id): Path<Uuid>,
    Json(params): Json<UpdateSiteRule>,
) -> Result<Json<site_rule::SiteRule>, ApiError> {
    let updated =
        site_rule::update_rule(&state.pool, &state.caches, auth.user_id, rule_id, &params).await?;
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::UpdateSiteRule,
        Some(AuditResourceType::SiteRule),
        Some(rule_id),
        serde_json::json!({}),
    )
    .await;
    Ok(Json(updated))
}

#[utoipa::path(
    delete,
    path = "/api/v1/site-rules/{id}",
    operation_id = "delete_site_rule",
    tag = "site-rules",
    params(("id" = Uuid, Path, description = "Site rule ID")),
    responses(
        (status = 200, description = "Site rule deleted"),
        (status = 401, description = "Missing or invalid auth"),
        (status = 404, description = "Rule not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(rule_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = site_rule::delete_rule(&state.pool, &state.caches, auth.user_id, rule_id).await?;
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
    )
    .await;
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
