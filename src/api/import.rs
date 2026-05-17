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

#[derive(Deserialize, utoipa::ToSchema)]
pub struct WallabagEntry {
    pub url: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub is_archived: Option<i32>,
    pub is_starred: Option<i32>,
    pub tags: Option<Vec<String>>,
}

#[utoipa::path(
    post,
    path = "/api/v1/import/wallabag",
    tag = "import",
    request_body = Vec<WallabagEntry>,
    responses(
        (status = 200, description = "Import result"),
        (status = 401, description = "Missing or invalid auth"),
    ),
    security(("bearer" = [])),
)]
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

        match entry::create_or_get_entry(&state.pool, auth.user_id, url).await {
            Ok(result) if result.already_existed => {
                skipped += 1;
            }
            Ok(result) => {
                let new_entry = result.entry;
                // If wallabag provided content, use it directly
                if let Some(ref content) = wb_entry.content {
                    if let Err(e) = entry::update_entry_content(
                        &state.pool,
                        new_entry.id,
                        &entry::ExtractedContent {
                            title: wb_entry.title.clone(),
                            content: Some(content.clone()),
                            http_status: 0,
                            extract_method: "manual".to_string(),
                            ..Default::default()
                        },
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
                if let Some(ref tag_labels) = wb_entry.tags
                    && !tag_labels.is_empty()
                    && let Err(e) = crate::models::tag::ensure_and_link(
                        &state.pool,
                        &state.caches,
                        auth.user_id,
                        &[new_entry.id],
                        tag_labels,
                    )
                    .await
                {
                    tracing::warn!(entry_id = %new_entry.id, "failed to import tags: {e}");
                }

                imported += 1;
            }
            Err(e) => {
                tracing::warn!("import: failed to create entry for {url}: {e}");
                skipped += 1;
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

#[utoipa::path(
    post,
    path = "/api/v1/import/browser",
    tag = "import",
    request_body = String,
    responses(
        (status = 200, description = "Import result"),
        (status = 401, description = "Missing or invalid auth"),
    ),
    security(("bearer" = [])),
)]
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
        match entry::create_or_get_entry(&state.pool, auth.user_id, href).await {
            Ok(result) if result.already_existed => {
                skipped += 1;
            }
            Ok(result) => {
                let new_entry = result.entry;
                if !title.is_empty() {
                    if let Err(e) = entry::update_entry_content(
                        &state.pool,
                        new_entry.id,
                        &entry::ExtractedContent {
                            title: Some(title.clone()),
                            http_status: 0,
                            extract_method: "pending".to_string(),
                            ..Default::default()
                        },
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
            Err(e) => {
                tracing::warn!("import: failed to create entry for {href}: {e}");
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

// --- Lettura Export Import ---

#[derive(Deserialize, utoipa::ToSchema)]
pub struct LetturaExport {
    pub version: String,
    #[serde(default)]
    pub entries: Vec<entry::Entry>,
    #[serde(default)]
    pub tags: Vec<crate::models::tag::Tag>,
    #[serde(default)]
    pub entry_tags: Vec<entry::EntryTagLink>,
    #[serde(default)]
    pub annotations: Vec<crate::models::annotation::Annotation>,
    #[serde(default)]
    pub memos: Vec<crate::models::memo::Memo>,
    #[serde(default)]
    pub tagging_rules: Vec<crate::models::tagging_rule::TaggingRule>,
    #[serde(default)]
    pub site_rules: Vec<crate::models::site_rule::SiteRule>,
}

#[utoipa::path(
    post,
    path = "/api/v1/import/lettura",
    tag = "import",
    request_body = LetturaExport,
    responses(
        (status = 200, description = "Import result"),
        (status = 401, description = "Missing or invalid auth"),
    ),
    security(("bearer" = [])),
)]
pub async fn import_lettura(
    State(state): State<AppState>,
    auth: AuthUser,
    body: Body,
) -> Result<Json<serde_json::Value>, ApiError> {
    let bytes = axum::body::to_bytes(body, state.config.import_max_body_bytes)
        .await
        .map_err(|_| ApiError::BadRequest("request body too large".to_string()))?;
    let data: LetturaExport = serde_json::from_slice(&bytes)
        .map_err(|e| ApiError::BadRequest(format!("invalid JSON: {e}")))?;

    // Accept any 1.x export — same major version is forward/backward compatible
    // because new fields use #[serde(default)]. A major version bump signals
    // an incompatible schema change and must be rejected.
    if !data.version.starts_with("1.") {
        return Err(ApiError::BadRequest(format!(
            "unsupported export version: {}",
            data.version
        )));
    }

    let mut imported = 0;
    let mut skipped = 0;
    let mut failed_entries = 0;
    let mut failed_links = 0;
    let mut failed_annotations = 0;
    let mut failed_memos = 0;
    let mut failed_rules = 0;
    let mut entry_id_map = std::collections::HashMap::<uuid::Uuid, uuid::Uuid>::new();
    let mut tag_id_map = std::collections::HashMap::<uuid::Uuid, uuid::Uuid>::new();
    // Staged for post-DB work so a failed search-index upsert can't leave a
    // partial DB state behind.
    let mut entries_to_index: Vec<SearchableImportedEntry> = Vec::new();
    let mut entries_to_fetch: Vec<FetchJob> = Vec::new();

    // Import entries (deduplicated by URL).
    for e in &data.entries {
        match entry::create_or_get_entry(&state.pool, auth.user_id, &e.url).await {
            Ok(result) if result.already_existed => {
                skipped += 1;
            }
            Ok(result) => {
                let new_entry = result.entry;

                // Sanitize HTML content: the export JSON comes from the caller
                // and may have been tampered with even when the original export
                // path produced sanitized HTML. Always re-clean on input.
                let sanitized_content = e
                    .content
                    .as_deref()
                    .map(crate::extract::sanitize::sanitize);

                if let Err(err) = entry::update_entry_content(
                    &state.pool,
                    new_entry.id,
                    &entry::ExtractedContent {
                        title: e.title.clone(),
                        content: sanitized_content.clone(),
                        text_content: e.text_content.clone(),
                        language: e.language.clone(),
                        preview_picture: e.preview_picture.clone(),
                        published_by: e.published_by.clone(),
                        reading_time: e.reading_time,
                        http_status: e.http_status.unwrap_or(0),
                        extract_method: e.extract_method.clone(),
                    },
                )
                .await
                {
                    tracing::warn!(
                        "import_lettura: failed to update content for entry {}: {err}",
                        new_entry.id
                    );
                    failed_entries += 1;
                    continue;
                }

                // Preserve original archived_at / starred_at timestamps —
                // calling update_entry would stamp them with now() and lose
                // backup fidelity.
                if e.is_archived || e.is_starred {
                    if let Err(err) = entry::restore_import_status(
                        &state.pool,
                        auth.user_id,
                        new_entry.id,
                        e.is_archived,
                        e.archived_at,
                        e.is_starred,
                        e.starred_at,
                    )
                    .await
                    {
                        tracing::warn!(
                            "import_lettura: failed to restore status for entry {}: {err}",
                            new_entry.id
                        );
                    }
                }

                let has_content = e
                    .content
                    .as_deref()
                    .is_some_and(|s| !s.trim().is_empty())
                    || e
                        .text_content
                        .as_deref()
                        .is_some_and(|s| !s.trim().is_empty());

                if !has_content {
                    // Half-fetched backup (no content yet): queue a fetch so the
                    // entry actually becomes readable on the new account.
                    entries_to_fetch.push(FetchJob {
                        entry_id: new_entry.id,
                        user_id: auth.user_id,
                        url: new_entry.url.clone(),
                    });
                }

                let domain = entry::extract_domain(&new_entry.url).unwrap_or_default();
                entries_to_index.push(SearchableImportedEntry {
                    id: new_entry.id,
                    title: e.title.clone().unwrap_or_default(),
                    text_content: e.text_content.clone().unwrap_or_default(),
                    url: new_entry.url.clone(),
                    domain,
                });

                entry_id_map.insert(e.id, new_entry.id);
                imported += 1;
            }
            Err(err) => {
                tracing::warn!(
                    "import_lettura: failed to create entry for {}: {err}",
                    e.url
                );
                failed_entries += 1;
            }
        }
    }

    // Import tags (deduplicated by label/slug).
    for t in &data.tags {
        match crate::models::tag::find_or_create_tag(&state.pool, &state.caches, auth.user_id, &t.label).await {
            Ok(tag) => {
                tag_id_map.insert(t.id, tag.id);
            }
            Err(e) => {
                tracing::warn!("import_lettura: failed to create tag {}: {e}", t.label);
            }
        }
    }

    // Rebuild entry_tags (only for newly imported entries).
    for et in &data.entry_tags {
        if let (Some(&new_entry_id), Some(&new_tag_id)) =
            (entry_id_map.get(&et.entry_id), tag_id_map.get(&et.tag_id))
            && let Err(e) = crate::models::tag::add_tag_to_entry(
                &state.pool,
                &state.caches,
                auth.user_id,
                new_entry_id,
                new_tag_id,
            )
            .await
        {
            tracing::warn!("import_lettura: failed to link entry-tag: {e}");
            failed_links += 1;
        }
    }

    // Import annotations (only for newly imported entries).
    for a in &data.annotations {
        if let Some(&new_entry_id) = entry_id_map.get(&a.entry_id)
            && let Err(e) = crate::models::annotation::import_annotation(
                &state.pool,
                a,
                new_entry_id,
                auth.user_id,
            )
            .await
        {
            tracing::warn!("import_lettura: failed to import annotation: {e}");
            failed_annotations += 1;
        }
    }

    // Import memos (user-level notes, not tied to specific entries).
    for m in &data.memos {
        let mapped_promoted_entry = m
            .promoted_entry_id
            .and_then(|pid| entry_id_map.get(&pid).copied());
        if let Err(e) =
            crate::models::memo::import_memo(&state.pool, m, auth.user_id, mapped_promoted_entry)
                .await
        {
            tracing::warn!("import_lettura: failed to import memo: {e}");
            failed_memos += 1;
        }
    }

    // Import tagging_rules.
    for r in &data.tagging_rules {
        if let Err(e) = crate::models::tagging_rule::import_rule(&state.pool, r, auth.user_id).await
        {
            tracing::warn!("import_lettura: failed to import tagging rule: {e}");
            failed_rules += 1;
        }
    }

    // Import site_rules (force user_id = auth.user_id: global rules must not be created via import).
    for sr in &data.site_rules {
        if let Err(e) = crate::models::site_rule::import_rule(&state.pool, sr, auth.user_id).await {
            tracing::warn!("import_lettura: failed to import site rule: {e}");
            failed_rules += 1;
        }
    }

    // Re-index newly imported entries using the data we already have in scope —
    // avoids one find_entry_by_id round-trip per entry.
    for indexed in &entries_to_index {
        if let Err(e) = state
            .search_index
            .upsert(
                indexed.id,
                auth.user_id,
                &indexed.title,
                &indexed.text_content,
                &indexed.url,
                &indexed.domain,
            )
            .await
        {
            tracing::warn!(entry_id = %indexed.id, "failed to index imported entry: {e}");
        }
    }

    // Queue fetches for entries that arrived without content.
    for job in entries_to_fetch {
        let _ = state.fetch_queue.send(job).await;
    }

    tracing::info!(
        imported = imported,
        skipped = skipped,
        total = data.entries.len(),
        failed_entries = failed_entries,
        failed_links = failed_links,
        failed_annotations = failed_annotations,
        failed_memos = failed_memos,
        failed_rules = failed_rules,
        "lettura import completed"
    );

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::ImportLettura,
        Some(AuditResourceType::System),
        None,
        serde_json::json!({
            "imported": imported,
            "skipped": skipped,
            "total": data.entries.len(),
            "failed_entries": failed_entries,
            "failed_links": failed_links,
            "failed_annotations": failed_annotations,
            "failed_memos": failed_memos,
            "failed_rules": failed_rules,
        }),
    )
    .await;

    Ok(Json(serde_json::json!({
        "imported": imported,
        "skipped": skipped,
        "total": data.entries.len(),
        "failed_entries": failed_entries,
        "failed_links": failed_links,
        "failed_annotations": failed_annotations,
        "failed_memos": failed_memos,
        "failed_rules": failed_rules,
    })))
}

struct SearchableImportedEntry {
    id: uuid::Uuid,
    title: String,
    text_content: String,
    url: String,
    domain: String,
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
