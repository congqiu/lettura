use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TaggingRule {
    pub id: Uuid,
    pub user_id: Uuid,
    pub rule: serde_json::Value,
    pub tags: Vec<String>,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize, Validate)]
pub struct CreateTaggingRule {
    pub rule: serde_json::Value,
    #[validate(length(min = 1, message = "tags must not be empty"))]
    pub tags: Vec<String>,
    pub priority: Option<i32>,
}

#[derive(Deserialize)]
pub struct UpdateTaggingRule {
    pub rule: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
    pub priority: Option<i32>,
}

pub async fn list_rules(pool: &PgPool, user_id: Uuid) -> Result<Vec<TaggingRule>, ApiError> {
    sqlx::query_as::<_, TaggingRule>(
        "SELECT * FROM tagging_rules WHERE user_id = $1 ORDER BY priority",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn create_rule(
    pool: &PgPool,
    user_id: Uuid,
    params: &CreateTaggingRule,
) -> Result<TaggingRule, ApiError> {
    sqlx::query_as::<_, TaggingRule>(
        "INSERT INTO tagging_rules (user_id, rule, tags, priority) VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(user_id)
    .bind(&params.rule)
    .bind(&params.tags)
    .bind(params.priority.unwrap_or(0))
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn update_rule(
    pool: &PgPool,
    user_id: Uuid,
    rule_id: Uuid,
    params: &UpdateTaggingRule,
) -> Result<TaggingRule, ApiError> {
    let existing = sqlx::query_as::<_, TaggingRule>(
        "SELECT * FROM tagging_rules WHERE id = $1 AND user_id = $2",
    )
    .bind(rule_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("rule not found".to_string()))?;

    let rule = params.rule.as_ref().unwrap_or(&existing.rule);
    let tags = params.tags.as_ref().unwrap_or(&existing.tags);
    let priority = params.priority.unwrap_or(existing.priority);

    sqlx::query_as::<_, TaggingRule>(
        "UPDATE tagging_rules SET rule = $3, tags = $4, priority = $5 WHERE id = $1 AND user_id = $2 RETURNING *",
    )
    .bind(rule_id)
    .bind(user_id)
    .bind(rule)
    .bind(tags)
    .bind(priority)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))
}

pub async fn delete_rule(pool: &PgPool, user_id: Uuid, rule_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query("DELETE FROM tagging_rules WHERE id = $1 AND user_id = $2")
        .bind(rule_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

/// Evaluate a rule against entry fields, return true if matches
pub fn evaluate_rule(rule: &serde_json::Value, entry_fields: &EntryFields) -> bool {
    let operator = rule["operator"].as_str().unwrap_or("AND");
    let conditions = match rule["conditions"].as_array() {
        Some(c) => c,
        None => return false,
    };

    let results: Vec<bool> = conditions
        .iter()
        .map(|cond| {
            // Nested group
            if cond.get("operator").is_some() {
                return evaluate_rule(cond, entry_fields);
            }
            evaluate_condition(cond, entry_fields)
        })
        .collect();

    match operator {
        "OR" => results.iter().any(|&r| r),
        _ => results.iter().all(|&r| r), // AND
    }
}

pub struct EntryFields {
    pub title: String,
    pub url: String,
    pub domain_name: String,
    pub language: String,
    pub reading_time: i32,
    pub content_type: String,
}

fn evaluate_condition(cond: &serde_json::Value, fields: &EntryFields) -> bool {
    let field = cond["field"].as_str().unwrap_or("");
    let op = cond["op"].as_str().unwrap_or("");
    let value = &cond["value"];

    let field_value = match field {
        "title" => &fields.title,
        "url" => &fields.url,
        "domainName" => &fields.domain_name,
        "language" => &fields.language,
        "contentType" => &fields.content_type,
        "readingTime" => {
            // Numeric comparison
            let target = value.as_i64().unwrap_or(0) as i32;
            return match op {
                "eq" => fields.reading_time == target,
                "neq" => fields.reading_time != target,
                "gt" => fields.reading_time > target,
                "lt" => fields.reading_time < target,
                _ => false,
            };
        }
        _ => return false,
    };

    let target = value.as_str().unwrap_or("");

    match op {
        "eq" => field_value == target,
        "neq" => field_value != target,
        "contains" => field_value.contains(target),
        "not_contains" => !field_value.contains(target),
        "matches" => regex::Regex::new(target)
            .map(|re| re.is_match(field_value))
            .unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_fields() -> EntryFields {
        EntryFields {
            title: "Rust Ownership Guide".to_string(),
            url: "https://github.com/example/repo".to_string(),
            domain_name: "github.com".to_string(),
            language: "en".to_string(),
            reading_time: 10,
            content_type: "article".to_string(),
        }
    }

    #[test]
    fn and_rule_matches() {
        let rule = json!({
            "operator": "AND",
            "conditions": [
                {"field": "domainName", "op": "eq", "value": "github.com"},
                {"field": "readingTime", "op": "gt", "value": 5}
            ]
        });
        assert!(evaluate_rule(&rule, &test_fields()));
    }

    #[test]
    fn and_rule_fails_when_one_condition_false() {
        let rule = json!({
            "operator": "AND",
            "conditions": [
                {"field": "domainName", "op": "eq", "value": "github.com"},
                {"field": "readingTime", "op": "lt", "value": 5}
            ]
        });
        assert!(!evaluate_rule(&rule, &test_fields()));
    }

    #[test]
    fn or_rule_matches_one() {
        let rule = json!({
            "operator": "OR",
            "conditions": [
                {"field": "language", "op": "eq", "value": "zh"},
                {"field": "title", "op": "contains", "value": "Rust"}
            ]
        });
        assert!(evaluate_rule(&rule, &test_fields()));
    }

    #[test]
    fn contains_operator() {
        let rule = json!({
            "operator": "AND",
            "conditions": [
                {"field": "url", "op": "contains", "value": "github"}
            ]
        });
        assert!(evaluate_rule(&rule, &test_fields()));
    }

    #[test]
    fn matches_regex() {
        let rule = json!({
            "operator": "AND",
            "conditions": [
                {"field": "title", "op": "matches", "value": "(?i)rust.*guide"}
            ]
        });
        assert!(evaluate_rule(&rule, &test_fields()));
    }
}
