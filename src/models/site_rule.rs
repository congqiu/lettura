use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SiteRule {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub domain: String,
    pub content_selector: String,
    pub title_selector: Option<String>,
    pub strip_selectors: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize, Validate)]
pub struct CreateSiteRule {
    #[validate(length(min = 1, message = "domain is required"))]
    pub domain: String,
    #[validate(length(min = 1, message = "content_selector is required"))]
    pub content_selector: String,
    pub title_selector: Option<String>,
    pub strip_selectors: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct UpdateSiteRule {
    pub content_selector: Option<String>,
    pub title_selector: Option<String>,
    pub strip_selectors: Option<Vec<String>>,
}

pub async fn list_rules(pool: &PgPool, user_id: Uuid) -> Result<Vec<SiteRule>, ApiError> {
    sqlx::query_as::<_, SiteRule>(
        "SELECT * FROM site_rules WHERE user_id = $1 OR user_id IS NULL ORDER BY domain",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn find_by_domain(pool: &PgPool, user_id: Uuid, domain: &str) -> Result<Option<SiteRule>, ApiError> {
    sqlx::query_as::<_, SiteRule>(
        "SELECT * FROM site_rules WHERE domain = $1 AND (user_id = $2 OR user_id IS NULL) ORDER BY user_id DESC NULLS LAST LIMIT 1",
    )
    .bind(domain)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn create_rule(pool: &PgPool, user_id: Uuid, params: &CreateSiteRule) -> Result<SiteRule, ApiError> {
    sqlx::query_as::<_, SiteRule>(
        "INSERT INTO site_rules (user_id, domain, content_selector, title_selector, strip_selectors) VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(user_id)
    .bind(&params.domain)
    .bind(&params.content_selector)
    .bind(params.title_selector.as_deref())
    .bind(&params.strip_selectors)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn update_rule(pool: &PgPool, user_id: Uuid, rule_id: Uuid, params: &UpdateSiteRule) -> Result<SiteRule, ApiError> {
    let existing = sqlx::query_as::<_, SiteRule>("SELECT * FROM site_rules WHERE id = $1 AND user_id = $2")
        .bind(rule_id).bind(user_id).fetch_optional(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("site rule not found".to_string()))?;

    let content_selector = params.content_selector.as_deref().unwrap_or(&existing.content_selector);
    let title_selector = params.title_selector.as_deref().or(existing.title_selector.as_deref());
    let strip_selectors = params.strip_selectors.as_ref().or(existing.strip_selectors.as_ref());

    sqlx::query_as::<_, SiteRule>(
        "UPDATE site_rules SET content_selector = $3, title_selector = $4, strip_selectors = $5 WHERE id = $1 AND user_id = $2 RETURNING *",
    )
    .bind(rule_id).bind(user_id).bind(content_selector).bind(title_selector).bind(strip_selectors)
    .fetch_one(pool).await.map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn delete_rule(pool: &PgPool, user_id: Uuid, rule_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("DELETE FROM site_rules WHERE id = $1 AND user_id = $2")
        .bind(rule_id).bind(user_id).execute(pool).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}
