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