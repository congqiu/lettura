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
