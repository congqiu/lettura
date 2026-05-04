use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use super::error::ModelError;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "audit_action", rename_all = "snake_case")]
pub enum AuditAction {
    Register,
    Login,
    Logout,
    RefreshToken,
    ChangePassword,
    RegenerateFeedToken,
    CreatePat,
    DeletePat,
    CreateEntry,
    UpdateEntry,
    SoftDeleteEntry,
    RestoreEntry,
    PermanentDeleteEntry,
    ArchiveEntry,
    UnarchiveEntry,
    StarEntry,
    UnstarEntry,
    RefetchEntry,
    CreateTag,
    DeleteTag,
    AddTagToEntry,
    RemoveTagFromEntry,
    CreateAnnotation,
    UpdateAnnotation,
    DeleteAnnotation,
    CreateMemo,
    DeleteMemo,
    PromoteMemo,
    CreateTaggingRule,
    UpdateTaggingRule,
    DeleteTaggingRule,
    CreateSiteRule,
    UpdateSiteRule,
    DeleteSiteRule,
    ImportWallabag,
    ImportBrowser,
    ExportAll,
    CreatePage,
    UpdatePage,
    DeletePage,
    RestorePage,
    AdminBackup,
    AdminRestore,
    AdminReindex,
    AdminListUsers,
    BulkTagAdd,
    BulkUntag,
    BulkArchive,
    BulkStar,
    BulkSoftDelete,
    RenameTag,
    UploadPageFiles,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "audit_resource_type", rename_all = "snake_case")]
pub enum AuditResourceType {
    User,
    Entry,
    Tag,
    Annotation,
    Memo,
    TaggingRule,
    SiteRule,
    Page,
    Pat,
    System,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct AuditLog {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub auth_source: String,
    pub action: AuditAction,
    pub resource_type: Option<AuditResourceType>,
    pub resource_id: Option<Uuid>,
    pub status: String,
    pub details: serde_json::Value,
    pub error_message: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extras: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct InsertAuditLog {
    pub user_id: Option<Uuid>,
    pub auth_source: String,
    pub action: AuditAction,
    pub resource_type: Option<AuditResourceType>,
    pub resource_id: Option<Uuid>,
    pub status: String,
    pub details: serde_json::Value,
    pub error_message: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<Uuid>,
}

/// Create an `InsertAuditLog` with common fields pre-filled.
/// Eliminates boilerplate: `status`, `error_message`, `ip_address`,
/// `user_agent`, `request_id` are set to defaults.
pub fn new_entry(
    user_id: Option<Uuid>,
    auth_source: String,
    action: AuditAction,
    resource_type: Option<AuditResourceType>,
    resource_id: Option<Uuid>,
    details: serde_json::Value,
) -> InsertAuditLog {
    InsertAuditLog {
        user_id,
        auth_source,
        action,
        resource_type,
        resource_id,
        status: "success".to_string(),
        details,
        error_message: None,
        ip_address: None,
        user_agent: None,
        request_id: None,
    }
}

/// Insert an audit log entry. On failure, logs a warning instead of propagating the error.
/// Use this for fire-and-forget audit logging where the main operation should not be blocked.
pub async fn log_success(
    pool: &PgPool,
    user_id: Option<Uuid>,
    auth_source: String,
    action: AuditAction,
    resource_type: Option<AuditResourceType>,
    resource_id: Option<Uuid>,
    details: serde_json::Value,
) {
    if let Err(e) = insert(pool, new_entry(user_id, auth_source, action, resource_type, resource_id, details)).await {
        tracing::warn!("audit log insert failed: {e}");
    }
}

/// Like `log_success` but includes IP address and User-Agent for security-relevant actions.
pub async fn log_success_with_context(
    pool: &PgPool,
    user_id: Option<Uuid>,
    auth_source: String,
    action: AuditAction,
    resource_type: Option<AuditResourceType>,
    resource_id: Option<Uuid>,
    details: serde_json::Value,
    ip_address: Option<String>,
    user_agent: Option<String>,
) {
    let mut entry = new_entry(user_id, auth_source, action, resource_type, resource_id, details);
    entry.ip_address = ip_address;
    entry.user_agent = user_agent;
    if let Err(e) = insert(pool, entry).await {
        tracing::warn!("audit log insert failed: {e}");
    }
}

/// Fire-and-forget helper for logging an action without blocking the request.
pub fn fire_and_forget(
    pool: PgPool,
    user_id: Option<Uuid>,
    auth_source: String,
    action: AuditAction,
    resource_type: Option<AuditResourceType>,
    resource_id: Option<Uuid>,
    status: String,
    details: serde_json::Value,
) {
    tokio::spawn(async move {
        if let Err(e) = insert(
            &pool,
            InsertAuditLog {
                user_id,
                auth_source,
                action,
                resource_type,
                resource_id,
                status,
                details,
                error_message: None,
                ip_address: None,
                user_agent: None,
                request_id: None,
            },
        )
        .await
        {
            tracing::warn!("audit log insert failed: {e}");
        }
    });
}

pub async fn insert(pool: &PgPool, log: InsertAuditLog) -> Result<AuditLog, ModelError> {
    sqlx::query_as::<_, AuditLog>(
        r#"
        INSERT INTO audit_logs
            (user_id, auth_source, action, resource_type, resource_id,
             status, details, error_message, ip_address, user_agent, request_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING *
        "#,
    )
    .bind(log.user_id)
    .bind(log.auth_source)
    .bind(log.action)
    .bind(log.resource_type)
    .bind(log.resource_id)
    .bind(log.status)
    .bind(log.details)
    .bind(log.error_message)
    .bind(log.ip_address)
    .bind(log.user_agent)
    .bind(log.request_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))
}

#[derive(Debug, Clone)]
pub struct ListAuditLogsFilter {
    pub user_id: Option<Uuid>,
    pub action: Option<AuditAction>,
    pub resource_type: Option<AuditResourceType>,
    pub resource_id: Option<Uuid>,
    pub status: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

impl Default for ListAuditLogsFilter {
    fn default() -> Self {
        Self {
            user_id: None,
            action: None,
            resource_type: None,
            resource_id: None,
            status: None,
            limit: 50,
            offset: 0,
        }
    }
}

pub async fn list(pool: &PgPool, filter: &ListAuditLogsFilter) -> Result<Vec<AuditLog>, ModelError> {
    let mut qb = QueryBuilder::new(
        "SELECT * FROM audit_logs WHERE 1=1"
    );

    if let Some(user_id) = filter.user_id {
        qb.push(" AND user_id = ")
            .push_bind(user_id);
    }
    if let Some(ref action) = filter.action {
        qb.push(" AND action = ")
            .push_bind(action);
    }
    if let Some(ref resource_type) = filter.resource_type {
        qb.push(" AND resource_type = ")
            .push_bind(resource_type);
    }
    if let Some(resource_id) = filter.resource_id {
        qb.push(" AND resource_id = ")
            .push_bind(resource_id);
    }
    if let Some(ref status) = filter.status {
        qb.push(" AND status = ")
            .push_bind(status);
    }

    qb.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(filter.limit)
        .push(" OFFSET ")
        .push_bind(filter.offset);

    qb.build_query_as::<AuditLog>()
        .fetch_all(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))
}

pub async fn count(pool: &PgPool, filter: &ListAuditLogsFilter) -> Result<i64, ModelError> {
    let mut qb = QueryBuilder::new(
        "SELECT COUNT(*) FROM audit_logs WHERE 1=1"
    );

    if let Some(user_id) = filter.user_id {
        qb.push(" AND user_id = ")
            .push_bind(user_id);
    }
    if let Some(ref action) = filter.action {
        qb.push(" AND action = ")
            .push_bind(action);
    }
    if let Some(ref resource_type) = filter.resource_type {
        qb.push(" AND resource_type = ")
            .push_bind(resource_type);
    }
    if let Some(resource_id) = filter.resource_id {
        qb.push(" AND resource_id = ")
            .push_bind(resource_id);
    }
    if let Some(ref status) = filter.status {
        qb.push(" AND status = ")
            .push_bind(status);
    }

    let row = qb.build()
        .fetch_one(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))?;

    Ok(row.try_get::<i64, _>(0).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_entry_defaults() {
        let user_id = Some(Uuid::new_v4());
        let entry = new_entry(
            user_id,
            "jwt".to_string(),
            AuditAction::Login,
            None,
            None,
            json!({}),
        );

        assert_eq!(entry.status, "success");
        assert!(entry.error_message.is_none());
        assert!(entry.ip_address.is_none());
        assert!(entry.user_agent.is_none());
        assert!(entry.request_id.is_none());
        assert_eq!(entry.user_id, user_id);
        assert_eq!(entry.auth_source, "jwt");
        assert_eq!(entry.action, AuditAction::Login);
    }

    #[test]
    fn new_entry_preserves_resource_type() {
        let resource_id = Uuid::new_v4();
        let details = json!({"title": "test"});
        let entry = new_entry(
            Some(Uuid::new_v4()),
            "pat".to_string(),
            AuditAction::CreateEntry,
            Some(AuditResourceType::Entry),
            Some(resource_id),
            details.clone(),
        );

        assert_eq!(entry.resource_type, Some(AuditResourceType::Entry));
        assert_eq!(entry.resource_id, Some(resource_id));
        assert_eq!(entry.details, details);
    }

    #[test]
    fn new_entry_no_user() {
        let entry = new_entry(
            None,
            "system".to_string(),
            AuditAction::AdminReindex,
            Some(AuditResourceType::System),
            None,
            json!({}),
        );

        assert!(entry.user_id.is_none());
        assert_eq!(entry.action, AuditAction::AdminReindex);
        assert_eq!(entry.auth_source, "system");
    }

    #[test]
    fn audit_action_variants_exist() {
        assert_ne!(AuditAction::CreateEntry, AuditAction::UpdateEntry);
        assert_ne!(AuditAction::Login, AuditAction::Logout);
        assert_ne!(AuditAction::StarEntry, AuditAction::UnstarEntry);
        assert_ne!(AuditAction::ArchiveEntry, AuditAction::UnarchiveEntry);
        assert_ne!(AuditAction::BulkArchive, AuditAction::BulkStar);
    }
}
