use axum::extract::State;
use axum::Json;
use serde::Deserialize;

use crate::api::error::ApiError;
use crate::auth::middleware::{AppState, AuthUser};
use crate::models::entry;
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
    Json(entries): Json<Vec<WallabagEntry>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut imported = 0;
    let mut skipped = 0;

    for wb_entry in &entries {
        let url = match wb_entry.url.as_deref() {
            Some(u) if !u.is_empty() => u,
            _ => { skipped += 1; continue; }
        };

        match entry::create_entry(&state.pool, auth.user_id, url).await {
            Ok(new_entry) => {
                // If wallabag provided content, use it directly
                if let Some(ref content) = wb_entry.content {
                    entry::update_entry_content(
                        &state.pool,
                        new_entry.id,
                        wb_entry.title.as_deref(),
                        Some(content),
                        None, None, None, None, None, 0, "manual",
                    ).await.ok();
                } else {
                    // Queue for fetching
                    let _ = state.fetch_queue.send(FetchJob {
                        entry_id: new_entry.id,
                        url: new_entry.url.clone(),
                    }).await;
                }

                // Apply archived/starred status
                if wb_entry.is_archived == Some(1) || wb_entry.is_starred == Some(1) {
                    let params = entry::UpdateEntryParams {
                        title: None,
                        content: None,
                        is_archived: if wb_entry.is_archived == Some(1) { Some(true) } else { None },
                        is_starred: if wb_entry.is_starred == Some(1) { Some(true) } else { None },
                    };
                    entry::update_entry(&state.pool, auth.user_id, new_entry.id, &params).await.ok();
                }

                imported += 1;
            }
            Err(ApiError::Conflict(_)) => { skipped += 1; }
            Err(_) => { skipped += 1; }
        }
    }

    tracing::info!(imported = imported, skipped = skipped, total = entries.len(), "wallabag import completed");
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
    body: String,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut imported = 0;
    let mut skipped = 0;

    // Parse simple bookmark HTML format: <A HREF="...">title</A>
    // Collect all bookmarks first to avoid holding non-Send ElementRef across await points
    let bookmarks: Vec<(String, String)> = {
        let doc = scraper::Html::parse_document(&body);
        let a_selector = scraper::Selector::parse("a[href]").unwrap();
        doc.select(&a_selector)
            .filter_map(|element| {
                let href = element.value().attr("href")?;
                if href.starts_with("http") {
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
                    entry::update_entry_content(
                        &state.pool, new_entry.id,
                        Some(title), None, None, None, None, None, None, 0, "pending",
                    ).await.ok();
                }
                let _ = state.fetch_queue.send(FetchJob {
                    entry_id: new_entry.id,
                    url: new_entry.url.clone(),
                }).await;
                imported += 1;
            }
            Err(_) => { skipped += 1; }
        }
    }

    tracing::info!(imported = imported, skipped = skipped, "browser import completed");
    Ok(Json(serde_json::json!({
        "imported": imported,
        "skipped": skipped
    })))
}
