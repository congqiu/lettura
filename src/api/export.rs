use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::state::AppState;

#[derive(Debug, Deserialize, Default, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportScope {
    #[default]
    All,
    Unread,
    Archived,
    Starred,
}

impl std::fmt::Display for ExportScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportScope::Unread => write!(f, "unread"),
            ExportScope::Archived => write!(f, "archived"),
            ExportScope::Starred => write!(f, "starred"),
            ExportScope::All => write!(f, "all"),
        }
    }
}

impl ExportScope {
    fn as_sql_filter(&self) -> &'static str {
        match self {
            ExportScope::Unread => " AND is_archived = false",
            ExportScope::Archived => " AND is_archived = true",
            ExportScope::Starred => " AND is_starred = true",
            ExportScope::All => "",
        }
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ExportQuery {
    #[serde(default)]
    scope: ExportScope,
}

#[utoipa::path(
    get,
    path = "/api/v1/export",
    tag = "export",
    params(ExportQuery),
    responses(
        (status = 200, description = "Full data export"),
        (status = 401, description = "Missing or invalid auth"),
    ),
    security(("bearer" = [])),
)]
pub async fn export_all(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ExportQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let scope_filter = query.scope.as_sql_filter();

    // Entries
    let entries_query = format!(
        "SELECT * FROM entries WHERE user_id = $1 AND deleted_at IS NULL{} ORDER BY created_at",
        scope_filter
    );
    let entries: Vec<crate::models::entry::Entry> = sqlx::query_as(&entries_query)
        .bind(auth.user_id)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let entry_ids: Vec<uuid::Uuid> = entries.iter().map(|e| e.id).collect();

    // Tags
    let tags: Vec<crate::models::tag::Tag> =
        sqlx::query_as("SELECT * FROM tags WHERE user_id = $1")
            .bind(auth.user_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Entry-tag links (only for exported entries)
    let entry_tags: Vec<crate::models::entry::EntryTagLink> = if entry_ids.is_empty() {
        vec![]
    } else {
        sqlx::query_as::<_, crate::models::entry::EntryTagLink>(
            "SELECT entry_id, tag_id FROM entry_tags WHERE entry_id = ANY($1)",
        )
        .bind(&entry_ids)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    };

    // Annotations (only for exported entries)
    let annotations: Vec<crate::models::annotation::Annotation> = if entry_ids.is_empty() {
        vec![]
    } else {
        sqlx::query_as(
            "SELECT * FROM annotations WHERE user_id = $1 AND entry_id = ANY($2) ORDER BY created_at",
        )
        .bind(auth.user_id)
        .bind(&entry_ids)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    };

    // Memos
    let memos: Vec<crate::models::memo::Memo> =
        sqlx::query_as("SELECT * FROM memos WHERE user_id = $1 ORDER BY created_at DESC")
            .bind(auth.user_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Tagging rules
    let tagging_rules: Vec<crate::models::tagging_rule::TaggingRule> =
        sqlx::query_as("SELECT * FROM tagging_rules WHERE user_id = $1 ORDER BY priority")
            .bind(auth.user_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Site rules (user-specific only; skip global rules)
    let site_rules: Vec<crate::models::site_rule::SiteRule> =
        sqlx::query_as("SELECT * FROM site_rules WHERE user_id = $1 ORDER BY domain")
            .bind(auth.user_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

    let scope_str = query.scope.to_string();

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::ExportAll,
        Some(AuditResourceType::System),
        None,
        serde_json::json!({
            "entries": entries.len(),
            "tags": tags.len(),
            "annotations": annotations.len(),
            "memos": memos.len(),
            "tagging_rules": tagging_rules.len(),
            "site_rules": site_rules.len(),
            "scope": scope_str,
        }),
    )
    .await;

    Ok(Json(serde_json::json!({
        "version": "1.0",
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "scope": scope_str,
        "entries": entries,
        "tags": tags,
        "entry_tags": entry_tags,
        "annotations": annotations,
        "memos": memos,
        "tagging_rules": tagging_rules,
        "site_rules": site_rules,
    })))
}
