use axum::Json;
use axum::body::Body;
use axum::extract::State;
use serde::Deserialize;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::entry;
use crate::state::AppState;
use crate::tasks::fetcher::FetchJob;

// --- Wallabag JSON Import ---

#[derive(Deserialize)]
pub struct WallabagEntry {
    pub url: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub is_archived: Option<i32>,
    pub is_starred: Option<i32>,
    pub tags: Option<Vec<String>>,
}

pub async fn import_wallabag(
    State(state): State<AppState>,
    auth: AuthUser,
    body: Body,
) -> Result<Json<serde_json::Value>, ApiError> {
    let bytes = axum::body::to_bytes(body, state.config.import_max_body_bytes)
        .await
        .map_err(|_| ApiError::BadRequest("request body too large".to_string()))?;
    let entries: Vec<WallabagEntry> = serde_json::from_slice(&bytes)
        .map_err(|e| ApiError::BadRequest(format!("invalid JSON: {e}")))?;
    let mut imported = 0;
    let mut skipped = 0;

    for wb_entry in &entries {
        let url = match wb_entry.url.as_deref() {
            Some(u) if !u.is_empty() => u,
            _ => {
                skipped += 1;
                continue;
            }
        };

        match entry::create_entry(&state.pool, auth.user_id, url).await {
            Ok(new_entry) => {
                // If wallabag provided content, use it directly
                if let Some(ref content) = wb_entry.content {
                    if let Err(e) = entry::update_entry_content(
                        &state.pool,
                        new_entry.id,
                        wb_entry.title.as_deref(),
                        Some(content),
                        None,
                        None,
                        None,
                        None,
                        None,
                        0,
                        "manual",
                    )
                    .await
                    {
                        tracing::warn!(
                            "import: failed to update content for entry {}: {e}",
                            new_entry.id
                        );
                    }

                    // Index imported content so it's immediately searchable
                    let domain = entry::extract_domain(&new_entry.url).unwrap_or_default();
                    if let Err(e) = state
                        .search_index
                        .upsert(
                            new_entry.id,
                            auth.user_id,
                            wb_entry.title.as_deref().unwrap_or(""),
                            content,
                            &new_entry.url,
                            &domain,
                        )
                        .await
                    {
                        tracing::warn!(entry_id = %new_entry.id, "failed to index imported entry: {e}");
                    }
                } else {
                    // Queue for fetching
                    let _ = state
                        .fetch_queue
                        .send(FetchJob {
                            entry_id: new_entry.id,
                            user_id: auth.user_id,
                            url: new_entry.url.clone(),
                        })
                        .await;
                }

                // Apply archived/starred status
                if wb_entry.is_archived == Some(1) || wb_entry.is_starred == Some(1) {
                    let params = entry::UpdateEntryParams {
                        title: None,
                        content: None,
                        is_archived: if wb_entry.is_archived == Some(1) {
                            Some(true)
                        } else {
                            None
                        },
                        is_starred: if wb_entry.is_starred == Some(1) {
                            Some(true)
                        } else {
                            None
                        },
                    };
                    if let Err(e) =
                        entry::update_entry(&state.pool, auth.user_id, new_entry.id, &params).await
                    {
                        tracing::warn!(
                            "import: failed to update status for entry {}: {e}",
                            new_entry.id
                        );
                    }
                }

                // Import tags from Wallabag
                if let Some(ref tag_labels) = wb_entry.tags {
                    if !tag_labels.is_empty() {
                        if let Err(e) = crate::models::tag::ensure_and_link(
                            &state.pool,
                            auth.user_id,
                            &[new_entry.id],
                            tag_labels,
                        )
                        .await
                        {
                            tracing::warn!(entry_id = %new_entry.id, "failed to import tags: {e}");
                        }
                    }
                }

                imported += 1;
            }
            Err(e) => {
                if matches!(e, crate::models::error::ModelError::Conflict(_)) {
                    skipped += 1;
                } else {
                    skipped += 1;
                }
            }
        }
    }

    tracing::info!(
        imported = imported,
        skipped = skipped,
        total = entries.len(),
        "wallabag import completed"
    );

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::ImportWallabag,
        Some(AuditResourceType::System),
        None,
        serde_json::json!({"imported": imported, "skipped": skipped, "total": entries.len()}),
    )
    .await;

    Ok(Json(serde_json::json!({
        "imported": imported,
        "skipped": skipped,
        "total": entries.len()
    })))
}

// --- Browser Bookmarks HTML Import ---

pub async fn import_browser(
    State(state): State<AppState>,
    auth: AuthUser,
    body: Body,
) -> Result<Json<serde_json::Value>, ApiError> {
    let body_bytes = axum::body::to_bytes(body, state.config.import_max_body_bytes)
        .await
        .map_err(|_| ApiError::BadRequest("request body too large".to_string()))?;
    let body_str = std::str::from_utf8(&body_bytes)
        .map_err(|e| ApiError::BadRequest(format!("invalid UTF-8: {e}")))?;

    let mut imported = 0;
    let mut skipped = 0;

    // Parse simple bookmark HTML format: <A HREF="...">title</A>
    let bookmarks: Vec<(String, String)> = {
        let doc = scraper::Html::parse_document(body_str);
        let a_selector = scraper::Selector::parse("a[href]").expect("valid CSS selector");
        doc.select(&a_selector)
            .filter_map(|element| {
                let href = element.value().attr("href")?;
                if href.starts_with("http://") || href.starts_with("https://") {
                    let title: String = element.text().collect();
                    Some((href.to_string(), title.trim().to_string()))
                } else {
                    None
                }
            })
            .collect()
    };

    for (href, title) in &bookmarks {
        match entry::create_entry(&state.pool, auth.user_id, href).await {
            Ok(new_entry) => {
                if !title.is_empty() {
                    if let Err(e) = entry::update_entry_content(
                        &state.pool,
                        new_entry.id,
                        Some(title),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        0,
                        "pending",
                    )
                    .await
                    {
                        tracing::warn!(
                            "import: failed to update content for entry {}: {e}",
                            new_entry.id
                        );
                    }

                    // Index the title so the entry is at least partially searchable
                    // before the fetch job completes
                    let domain = entry::extract_domain(&new_entry.url).unwrap_or_default();
                    if let Err(e) = state
                        .search_index
                        .upsert(
                            new_entry.id,
                            auth.user_id,
                            title,
                            "",
                            &new_entry.url,
                            &domain,
                        )
                        .await
                    {
                        tracing::warn!(entry_id = %new_entry.id, "failed to index imported entry: {e}");
                    }
                }
                let _ = state
                    .fetch_queue
                    .send(FetchJob {
                        entry_id: new_entry.id,
                        user_id: auth.user_id,
                        url: new_entry.url.clone(),
                    })
                    .await;
                imported += 1;
            }
            Err(_) => {
                skipped += 1;
            }
        }
    }

    tracing::info!(
        imported = imported,
        skipped = skipped,
        "browser import completed"
    );

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::ImportBrowser,
        Some(AuditResourceType::System),
        None,
        serde_json::json!({"imported": imported, "skipped": skipped}),
    )
    .await;

    Ok(Json(serde_json::json!({
        "imported": imported,
        "skipped": skipped
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallabag_entry_deserialization() {
        let json = r#"{
            "url": "https://example.com/article",
            "title": "Example Article",
            "content": "<p>Hello</p>",
            "is_archived": 1,
            "is_starred": 0,
            "tags": ["rust", "programming"]
        }"#;
        let entry: WallabagEntry =
            serde_json::from_str(json).expect("deserialization should succeed");
        assert_eq!(entry.url.as_deref(), Some("https://example.com/article"));
        assert_eq!(entry.title.as_deref(), Some("Example Article"));
        assert_eq!(entry.content.as_deref(), Some("<p>Hello</p>"));
        assert_eq!(entry.is_archived, Some(1));
        assert_eq!(entry.is_starred, Some(0));
        assert_eq!(
            entry.tags,
            Some(vec!["rust".to_string(), "programming".to_string()])
        );
    }

    #[test]
    fn wallabag_entry_with_null_url() {
        let json = r#"{"url": null, "title": "No URL"}"#;
        let entry: WallabagEntry =
            serde_json::from_str(json).expect("deserialization should succeed");
        assert!(entry.url.is_none());
        assert_eq!(entry.title.as_deref(), Some("No URL"));
    }

    #[test]
    fn wallabag_entry_with_empty_url() {
        let json = r#"{"url": "", "title": "Empty URL"}"#;
        let entry: WallabagEntry =
            serde_json::from_str(json).expect("deserialization should succeed");
        assert_eq!(entry.url, Some("".to_string()));
        assert_eq!(entry.title.as_deref(), Some("Empty URL"));
    }
}
