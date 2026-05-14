use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct TagLabel {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EntrySummary {
    pub id: String,
    pub url: String,
    pub title: Option<String>,
    pub domain_name: Option<String>,
    #[serde(default)]
    pub tags: Vec<TagLabel>,
    #[serde(default)]
    pub is_starred: bool,
    #[serde(default)]
    pub is_archived: bool,
    pub created_at: Option<String>,
    pub reading_time: Option<i32>,
    pub language: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EntryFull {
    pub id: String,
    pub url: String,
    pub title: Option<String>,
    pub domain_name: Option<String>,
    pub content: Option<String>,
    pub content_type: Option<String>,
    pub language: Option<String>,
    pub reading_time: Option<i32>,
    #[serde(default)]
    pub is_starred: bool,
    #[serde(default)]
    pub is_archived: bool,
    pub created_at: Option<String>,
    #[serde(default)]
    pub tags: Vec<TagLabel>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Tag {
    pub id: String,
    pub label: String,
    pub slug: Option<String>,
    pub user_id: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuditLog {
    pub id: String,
    pub user_id: Option<String>,
    pub auth_source: String,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub status: String,
    pub details: serde_json::Value,
    pub error_message: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ListAuditLogsResponse {
    pub data: Vec<AuditLog>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

// Pages API types

#[derive(Debug, Deserialize, Serialize)]
pub struct UploadResponse {
    pub upload_id: String,
    pub html_files: Vec<String>,
    pub default_entry: String,
    pub suggested_title: Option<String>,
}

/// Response from POST /api/v1/pages and PATCH /api/v1/pages/{id}
/// (server returns the full Page model)
#[derive(Debug, Deserialize, Serialize)]
pub struct PageResponse {
    pub id: String,
    pub slug: String,
    pub user_id: String,
    pub title: String,
    pub description: Option<String>,
    pub entry_file: String,
    pub has_password: bool,
    pub status: Option<String>,
    pub file_count: i64,
    pub expires_at: Option<String>,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

/// Response from GET /api/v1/pages (list)
/// (server returns PageSummary with has_password instead of password hash)
#[derive(Debug, Deserialize, Serialize)]
pub struct PageListItem {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub has_password: bool,
    pub status: Option<String>,
    pub file_count: i64,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PageListResponse {
    pub items: Vec<PageListItem>,
    pub total: u32,
    pub page: u32,
    pub limit: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PageShareResponse {
    pub url: String,
    pub has_password: bool,
}
