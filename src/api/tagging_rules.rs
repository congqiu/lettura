use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::state::AppState;
use crate::models::tagging_rule::{self, CreateTaggingRule, UpdateTaggingRule};

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
    Ok(Json(rule))
}

pub async fn update_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(rule_id): Path<Uuid>,
    Json(params): Json<UpdateTaggingRule>,
) -> Result<Json<tagging_rule::TaggingRule>, ApiError> {
    let updated = tagging_rule::update_rule(&state.pool, auth.user_id, rule_id, &params).await?;
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
    Ok(Json(serde_json::json!({"message": "deleted"})))
}
