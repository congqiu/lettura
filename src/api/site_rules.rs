use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::{AppState, AuthUser};
use crate::models::site_rule::{self, CreateSiteRule, UpdateSiteRule};

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
    Json(params): Json<CreateSiteRule>,
) -> Result<Json<site_rule::SiteRule>, ApiError> {
    if params.domain.is_empty() || params.content_selector.is_empty() {
        return Err(ApiError::BadRequest(
            "domain and content_selector required".to_string(),
        ));
    }
    let rule = site_rule::create_rule(&state.pool, auth.user_id, &params).await?;
    Ok(Json(rule))
}

pub async fn update_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(rule_id): Path<Uuid>,
    Json(params): Json<UpdateSiteRule>,
) -> Result<Json<site_rule::SiteRule>, ApiError> {
    let updated = site_rule::update_rule(&state.pool, auth.user_id, rule_id, &params).await?;
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
    Ok(Json(serde_json::json!({"message": "deleted"})))
}
