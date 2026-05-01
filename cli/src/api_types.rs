use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct EntrySummary {
    pub id: uuid::Uuid,
    pub url: String,
    pub title: Option<String>,
    pub domain_name: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub is_starred: bool,
    #[serde(default)]
    pub is_archived: bool,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub reading_time: Option<i32>,
    pub language: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EntryFull {
    pub id: uuid::Uuid,
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
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Tag {
    pub id: uuid::Uuid,
    pub label: String,
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuditLog {
    pub id: uuid::Uuid,
    pub user_id: Option<uuid::Uuid>,
    pub auth_source: String,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<uuid::Uuid>,
    pub status: String,
    pub details: serde_json::Value,
    pub error_message: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ListAuditLogsResponse {
    pub data: Vec<AuditLog>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}
