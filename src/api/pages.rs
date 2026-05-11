use axum::Json;
use axum::extract::multipart::MultipartError;
use axum::extract::{Multipart, Path, Query, State};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;
use validator::Validate;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::page;
use crate::state::AppState;

use super::validate::ValidatedJson;

fn pages_storage_path(state: &AppState) -> PathBuf {
    PathBuf::from(&state.config.pages_storage_path)
}

fn tmp_dir(state: &AppState) -> PathBuf {
    PathBuf::from(&state.config.pages_storage_path).join("tmp")
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    upload_id: String,
    html_files: Vec<String>,
    default_entry: String,
    suggested_title: String,
    file_count: usize,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePageRequest {
    pub upload_id: String,
    pub entry_file: String,
    #[validate(length(min = 1, max = 500))]
    pub title: String,
    pub description: Option<String>,
    pub password: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListQueryParams {
    pub status: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePageRequest {
    #[validate(length(max = 500))]
    pub title: Option<String>,
    pub description: Option<String>,
    pub password: Option<String>,
    pub status: Option<String>,
    pub expires_at: Option<Option<String>>,
    pub upload_id: Option<String>,
    pub entry_file: Option<String>,
}

#[tracing::instrument(skip(state, multipart), err)]
pub async fn upload_files(
    State(state): State<AppState>,
    auth: AuthUser,
    multipart: Multipart,
) -> Result<Json<UploadResponse>, ApiError> {
    let upload_id = Uuid::new_v4().to_string();
    let temp_base = tmp_dir(&state).join(&upload_id);
    tokio::fs::create_dir_all(&temp_base)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut html_files: Vec<String> = Vec::new();
    let mut file_count: usize = 0;
    let mut total_size: usize = 0;
    let mut saved_files: HashMap<String, Vec<u8>> = HashMap::new();

    let mut multipart = multipart;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e: MultipartError| ApiError::BadRequest(e.to_string()))?
    {
        let filename = field.file_name().unwrap_or("unknown").to_string();
        let data = field
            .bytes()
            .await
            .map_err(|e: MultipartError| ApiError::BadRequest(e.to_string()))?;
        total_size += data.len();
        if total_size > state.config.pages_max_upload_bytes {
            tokio::fs::remove_dir_all(&temp_base).await.ok();
            return Err(ApiError::BadRequest("upload too large".to_string()));
        }

        if filename.ends_with(".zip") {
            let extracted = extract_zip(&data)?;
            for (name, content) in extracted {
                let path = temp_base.join(&name);
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| ApiError::Internal(e.to_string()))?;
                }
                tokio::fs::write(&path, &content)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                if name.ends_with(".html") {
                    html_files.push(name.clone());
                }
                saved_files.insert(name, content);
                file_count += 1;
            }
        } else {
            let safe_name = sanitize_filename(&filename);
            let path = temp_base.join(&safe_name);
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
            }
            tokio::fs::write(&path, &data)
                .await
                .map_err(|e: std::io::Error| ApiError::Internal(e.to_string()))?;
            if safe_name.ends_with(".html") {
                html_files.push(safe_name.clone());
            }
            saved_files.insert(safe_name, data.to_vec());
            file_count += 1;
        }
    }

    if file_count == 0 {
        tokio::fs::remove_dir_all(&temp_base).await.ok();
        return Err(ApiError::BadRequest("no files uploaded".to_string()));
    }

    if html_files.is_empty() {
        tokio::fs::remove_dir_all(&temp_base).await.ok();
        return Err(ApiError::BadRequest("no HTML files found".to_string()));
    }

    let default_entry = html_files
        .iter()
        .find(|f| **f == "index.html" || f.ends_with("/index.html"))
        .or_else(|| html_files.first())
        .expect("html_files is non-empty (checked above)")
        .clone();

    let suggested_title = extract_title(
        saved_files
            .get(&default_entry)
            .expect("default_entry was saved"),
        &default_entry,
    );

    let cleanup_path = temp_base.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1800)).await;
        tokio::fs::remove_dir_all(&cleanup_path).await.ok();
    });

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::UploadPageFiles,
        Some(AuditResourceType::Page),
        None,
        serde_json::json!({"upload_id": upload_id, "file_count": file_count}),
    )
    .await;

    Ok(Json(UploadResponse {
        upload_id,
        html_files,
        default_entry,
        suggested_title,
        file_count,
    }))
}

/// Validate that a zip entry path is a safe relative path.
/// Only allows `Component::Normal` segments — rejects absolute paths,
/// `..` traversal, drive letters, and other platform-specific components.
fn is_safe_relative_path(name: &str) -> bool {
    // Reject backslashes (Windows-style paths that could bypass checks).
    if name.contains('\\') {
        return false;
    }
    // Reject hidden files/dirs and __MACOSX metadata.
    if name.starts_with('.') || name.contains("/.") || name.contains("__MACOSX") {
        return false;
    }
    // Skip directory entries.
    if name.ends_with('/') {
        return false;
    }
    // Every component must be Normal (no Prefix, RootDir, ParentDir, CurDir).
    use std::path::Component;
    std::path::Path::new(name)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
}

fn extract_zip(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ApiError> {
    const MAX_ENTRIES: usize = 500;
    const MAX_NAME_LEN: usize = 255;

    let reader = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| ApiError::BadRequest(format!("invalid zip: {e}")))?;
    let mut files = Vec::new();
    let mut total_extracted_size: usize = 0;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        let name = entry.name().to_string();

        if name.is_empty() {
            continue;
        }
        if name.len() > MAX_NAME_LEN {
            return Err(ApiError::BadRequest(format!(
                "zip entry name too long (max {MAX_NAME_LEN} chars): {name}"
            )));
        }
        if files.len() >= MAX_ENTRIES {
            return Err(ApiError::BadRequest(format!(
                "too many zip entries (max {MAX_ENTRIES})"
            )));
        }
        // Reject any path component that isn't a normal relative segment.
        // This blocks absolute paths, ".." traversal, drive letters, and
        // other platform-specific components.
        if !is_safe_relative_path(&name) {
            tracing::warn!(name = %name, "skipping unsafe zip entry");
            continue;
        }

        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut content)
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        total_extracted_size += content.len();
        if total_extracted_size > 50 * 1024 * 1024 {
            return Err(ApiError::BadRequest(
                "zip too large (max 50MB uncompressed)".to_string(),
            ));
        }

        files.push((name, content));
    }

    strip_common_prefix(&mut files);

    Ok(files)
}

fn strip_common_prefix(files: &mut Vec<(String, Vec<u8>)>) {
    if files.is_empty() {
        return;
    }
    let prefix = {
        let first = &files[0].0;
        let slash_pos = match first.find('/') {
            Some(p) => p,
            None => return,
        };
        let candidate = &first[..=slash_pos];
        if files.iter().all(|(n, _)| n.starts_with(candidate)) {
            Some(candidate.to_string())
        } else {
            None
        }
    };
    if let Some(prefix) = prefix {
        for (name, _) in files.iter_mut() {
            *name = name[prefix.len()..].to_string();
        }
    }
}

fn sanitize_filename(name: &str) -> String {
    name.replace("..", "")
        .split('/')
        .filter(|s| !s.is_empty() && !s.starts_with('.'))
        .collect::<Vec<_>>()
        .join("/")
}

fn extract_title(html_content: &[u8], fallback: &str) -> String {
    let content = String::from_utf8_lossy(html_content);
    let lower = content.to_lowercase();
    if let Some(start) = lower.find("<title>") {
        if let Some(end) = lower.find("</title>") {
            let title = content[start + 7..end].trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }
    fallback.trim_end_matches(".html").to_string()
}

#[tracing::instrument(skip(state), err)]
pub async fn create_page_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<CreatePageRequest>,
) -> Result<Json<page::Page>, ApiError> {
    let temp_base = tmp_dir(&state).join(&req.upload_id);

    let exists = tokio::fs::try_exists(&temp_base)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    if !exists {
        return Err(ApiError::NotFound("upload session expired".to_string()));
    }

    let file_count = count_files_recursive(temp_base.clone())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let password = match &req.password {
        Some(pw) if !pw.is_empty() => {
            Some(crate::auth::password::hash_page_password(pw).map_err(|_| {
                ApiError::BadRequest("password too long (max 128 chars)".to_string())
            })?)
        }
        _ => None,
    };

    let expires_at = req
        .expires_at
        .as_deref()
        .map(|s| chrono::DateTime::parse_from_rfc3339(s))
        .transpose()
        .map_err(|_| {
            ApiError::BadRequest("invalid expires_at format, expected ISO 8601".to_string())
        })?
        .map(|dt| dt.to_utc());

    let new_page = page::create_page_with_retry(
        &state.pool,
        auth.user_id,
        &req.title,
        req.description.as_deref(),
        &req.entry_file,
        password.as_deref(),
        file_count as i32,
        expires_at,
    )
    .await?;

    let slug = new_page.slug.clone();

    if state.config.storage_type == "local" {
        let pages_base = pages_storage_path(&state).join(&slug);
        tokio::fs::create_dir_all(&pages_base)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        copy_dir_recursive(temp_base.clone(), pages_base)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    } else {
        let files = read_dir_recursive(temp_base.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        for (relative_path, absolute_path) in &files {
            let key = format!("pages/{}/{}", slug, relative_path);
            let data = tokio::fs::read(absolute_path)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            let mime = mime_for_path(relative_path);
            state
                .storage
                .store(&key, &data, mime)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
        }
    }

    tokio::fs::remove_dir_all(&temp_base).await.ok();

    tracing::info!(page_id = %new_page.id, slug = %slug, "page created");

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::CreatePage,
        Some(AuditResourceType::Page),
        Some(new_page.id),
        serde_json::json!({"slug": new_page.slug, "title": new_page.title}),
    )
    .await;

    Ok(Json(new_page))
}

fn count_files_recursive(
    dir: std::path::PathBuf,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<usize, std::io::Error>> + Send>> {
    Box::pin(async move {
        let mut count = 0;
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                count += count_files_recursive(path).await?;
            } else {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn copy_dir_recursive(
    src: std::path::PathBuf,
    dst: std::path::PathBuf,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), std::io::Error>> + Send>> {
    Box::pin(async move {
        tokio::fs::create_dir_all(&dst).await?;
        let mut entries = tokio::fs::read_dir(&src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if src_path.is_dir() {
                copy_dir_recursive(src_path, dst_path).await?;
            } else {
                tokio::fs::copy(&src_path, &dst_path).await?;
            }
        }
        Ok(())
    })
}

fn read_dir_recursive(
    dir: std::path::PathBuf,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<Output = Result<Vec<(String, std::path::PathBuf)>, std::io::Error>>
            + Send,
    >,
> {
    Box::pin(async move {
        let mut files = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let sub = read_dir_recursive(path.clone()).await?;
                for (_rel, abs) in sub {
                    let prefix = dir.to_str().unwrap_or("");
                    let relative = abs
                        .strip_prefix(prefix)
                        .unwrap_or(&abs)
                        .to_str()
                        .unwrap_or("")
                        .trim_start_matches('/')
                        .to_string();
                    files.push((relative, abs));
                }
            } else {
                let relative = path
                    .strip_prefix(&dir)
                    .unwrap_or(&path)
                    .to_str()
                    .unwrap_or("")
                    .trim_start_matches('/')
                    .to_string();
                files.push((relative, path));
            }
        }
        Ok(files)
    })
}

fn mime_for_path(path: &str) -> &str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

#[tracing::instrument(skip(state), err)]
pub async fn list_pages_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<ListQueryParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let page_num = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(20);
    let (items, total) = page::list_pages(
        &state.pool,
        auth.user_id,
        params.status.as_deref(),
        page_num,
        limit,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": page_num,
        "limit": limit,
    })))
}

#[tracing::instrument(skip(state), err)]
pub async fn update_page_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<UpdatePageRequest>,
) -> Result<Json<page::Page>, ApiError> {
    if let Some(ref status) = req.status {
        if status != "active" && status != "disabled" {
            return Err(ApiError::BadRequest(
                "status must be 'active' or 'disabled'".to_string(),
            ));
        }
    }
    let expires_at = match req.expires_at {
        Some(Some(s)) => {
            let dt = chrono::DateTime::parse_from_rfc3339(&s).map_err(|_| {
                ApiError::BadRequest("invalid expires_at format, expected ISO 8601".to_string())
            })?;
            Some(Some(dt.to_utc()))
        }
        Some(None) => Some(None), // explicitly clear expiration
        None => None,             // don't change
    };

    // Handle file replacement if upload_id is provided
    let (entry_file, file_count) = if let Some(ref upload_id) = req.upload_id {
        let temp_base = tmp_dir(&state).join(upload_id);
        let exists = tokio::fs::try_exists(&temp_base)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        if !exists {
            return Err(ApiError::NotFound("upload session expired".to_string()));
        }

        // Find existing page to get slug
        let existing = page::find_page_by_id(&state.pool, auth.user_id, page_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("page not found".to_string()))?;
        let slug = existing.slug.clone();

        // Delete old files and copy new ones
        if state.config.storage_type == "local" {
            let pages_base = pages_storage_path(&state).join(&slug);
            // Remove old files (ignore if directory doesn't exist)
            tokio::fs::remove_dir_all(&pages_base).await.ok();
            tokio::fs::create_dir_all(&pages_base)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            // Copy new files
            copy_dir_recursive(temp_base.clone(), pages_base)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
        } else {
            // Delete old S3 objects
            let prefix = format!("pages/{}/", slug);
            state
                .storage
                .delete_prefix(&prefix)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            // Upload new files
            let files = read_dir_recursive(temp_base.clone())
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            for (relative_path, absolute_path) in &files {
                let key = format!("pages/{}/{}", slug, relative_path);
                let data = tokio::fs::read(absolute_path)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                let mime = mime_for_path(relative_path);
                state
                    .storage
                    .store(&key, &data, mime)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
            }
        }

        let count = count_files_recursive(temp_base.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        // Cleanup temp dir
        tokio::fs::remove_dir_all(&temp_base).await.ok();

        (req.entry_file.clone(), Some(count as i32))
    } else {
        (req.entry_file.clone(), None)
    };

    // Hash password before storing if a new password is provided
    let password = match req.password {
        Some(ref pw) if !pw.is_empty() => {
            Some(crate::auth::password::hash_page_password(pw).map_err(|_| {
                ApiError::BadRequest("password too long (max 128 chars)".to_string())
            })?)
        }
        Some(ref pw) if pw.is_empty() => Some(String::new()), // empty string signals "clear password"
        _ => None,
    };

    let updated = page::update_page(
        &state.pool,
        auth.user_id,
        page_id,
        &page::UpdatePageParams {
            title: req.title,
            description: req.description,
            password,
            status: req.status,
            expires_at,
            entry_file,
            file_count,
        },
    )
    .await?;
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::UpdatePage,
        Some(AuditResourceType::Page),
        Some(page_id),
        serde_json::json!({}),
    )
    .await;
    Ok(Json(updated))
}

#[tracing::instrument(skip(state), err)]
pub async fn delete_page_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = page::delete_page(&state.pool, auth.user_id, page_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("page not found".to_string()));
    }
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::DeletePage,
        Some(AuditResourceType::Page),
        Some(page_id),
        serde_json::json!({}),
    )
    .await;
    Ok(Json(serde_json::json!({"success": true})))
}

#[tracing::instrument(skip(state), err)]
pub async fn restore_page_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    page::restore_page(&state.pool, auth.user_id, page_id).await?;
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::RestorePage,
        Some(AuditResourceType::Page),
        Some(page_id),
        serde_json::json!({}),
    )
    .await;
    Ok(Json(serde_json::json!({"success": true})))
}

#[derive(Debug, Serialize)]
pub struct ShareUrlResponse {
    pub url: String,
    pub has_password: bool,
}

#[tracing::instrument(skip(state), err)]
pub async fn get_share_url_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
) -> Result<Json<ShareUrlResponse>, ApiError> {
    let page = page::find_page_by_id(&state.pool, auth.user_id, page_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("page not found".to_string()))?;

    let base_url = format!("/p/{}", page.slug);
    let has_password = page.password.is_some();

    Ok(Json(ShareUrlResponse {
        url: base_url,
        has_password,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ── is_safe_relative_path ──────────────────────────────────────

    #[test]
    fn is_safe_relative_path_normal_relative() {
        assert!(is_safe_relative_path("dir/file.html"));
    }

    #[test]
    fn is_safe_relative_path_traversal() {
        assert!(!is_safe_relative_path("../etc/passwd"));
    }

    #[test]
    fn is_safe_relative_path_absolute() {
        assert!(!is_safe_relative_path("/etc/passwd"));
    }

    #[test]
    fn is_safe_relative_path_backslash() {
        assert!(!is_safe_relative_path("dir\\file.html"));
    }

    #[test]
    fn is_safe_relative_path_hidden_file() {
        assert!(!is_safe_relative_path(".env"));
    }

    #[test]
    fn is_safe_relative_path_hidden_dir() {
        assert!(!is_safe_relative_path(".git/config"));
    }

    #[test]
    fn is_safe_relative_path_macosx() {
        assert!(!is_safe_relative_path("__MACOSX/file"));
    }

    #[test]
    fn is_safe_relative_path_directory_entry() {
        assert!(!is_safe_relative_path("dir/"));
    }

    #[test]
    fn is_safe_relative_path_normal_multi_level() {
        assert!(is_safe_relative_path("css/style.css"));
    }

    #[test]
    fn is_safe_relative_path_dot_in_middle() {
        assert!(is_safe_relative_path("my.file.html"));
    }

    // ── sanitize_filename ──────────────────────────────────────────

    #[test]
    fn sanitize_filename_removes_double_dot() {
        assert_eq!(sanitize_filename("dir/../file.html"), "dir/file.html");
    }

    #[test]
    fn sanitize_filename_filters_empty_and_dot_prefixed() {
        // "../.hidden/./file.html" → after replace("..","") → "./.hidden/./file.html"
        // split by '/' → ["", ".hidden", "", "file.html"]
        // filter empty & dot-prefixed → ["file.html"]
        assert_eq!(sanitize_filename("../.hidden/./file.html"), "file.html");
    }

    #[test]
    fn sanitize_filename_normal_unchanged() {
        assert_eq!(sanitize_filename("style.css"), "style.css");
    }

    #[test]
    fn sanitize_filename_collapses_multiple_slashes() {
        assert_eq!(sanitize_filename("dir///file.html"), "dir/file.html");
    }

    // ── extract_title ──────────────────────────────────────────────

    #[test]
    fn extract_title_from_tag() {
        let html = b"<html><head><title>My Title</title></head><body></body></html>";
        assert_eq!(extract_title(html, "fallback.html"), "My Title");
    }

    #[test]
    fn extract_title_case_insensitive() {
        // The implementation lowercases the content for searching, then slices
        // the original — so mixed-case tags are found but the extracted text
        // retains its original casing from the source bytes.
        let html = b"<HTML><HEAD><TITLE>My Title</TITLE></HEAD><BODY></BODY></HTML>";
        assert_eq!(extract_title(html, "fallback.html"), "My Title");
    }

    #[test]
    fn extract_title_empty_falls_back_to_filename() {
        let html = b"<html><head><title></title></head><body></body></html>";
        assert_eq!(extract_title(html, "page.html"), "page");
    }

    #[test]
    fn extract_title_no_tag_falls_back_to_filename() {
        let html = b"<html><head></head><body></body></html>";
        assert_eq!(extract_title(html, "page.html"), "page");
    }

    #[test]
    fn extract_title_whitespace_trimmed() {
        let html = b"<html><head><title>  Spaced Title  </title></head><body></body></html>";
        assert_eq!(extract_title(html, "fallback.html"), "Spaced Title");
    }

    // ── strip_common_prefix ────────────────────────────────────────

    #[test]
    fn strip_common_prefix_strips() {
        let mut files = vec![
            ("root/index.html".to_string(), vec![1u8]),
            ("root/style.css".to_string(), vec![2u8]),
        ];
        strip_common_prefix(&mut files);
        assert_eq!(files[0].0, "index.html");
        assert_eq!(files[1].0, "style.css");
    }

    #[test]
    fn strip_common_prefix_no_common() {
        let mut files = vec![
            ("a/index.html".to_string(), vec![1u8]),
            ("b/style.css".to_string(), vec![2u8]),
        ];
        strip_common_prefix(&mut files);
        assert_eq!(files[0].0, "a/index.html");
        assert_eq!(files[1].0, "b/style.css");
    }

    #[test]
    fn strip_common_prefix_single_file_no_slash() {
        let mut files = vec![("index.html".to_string(), vec![1u8])];
        strip_common_prefix(&mut files);
        assert_eq!(files[0].0, "index.html");
    }

    #[test]
    fn strip_common_prefix_empty_list() {
        let mut files: Vec<(String, Vec<u8>)> = vec![];
        strip_common_prefix(&mut files);
        assert!(files.is_empty());
    }

    // ── mime_for_path ──────────────────────────────────────────────

    #[test]
    fn mime_for_path_html() {
        assert_eq!(mime_for_path("page.html"), "text/html; charset=utf-8");
    }

    #[test]
    fn mime_for_path_htm() {
        assert_eq!(mime_for_path("page.htm"), "text/html; charset=utf-8");
    }

    #[test]
    fn mime_for_path_css() {
        assert_eq!(mime_for_path("style.css"), "text/css; charset=utf-8");
    }

    #[test]
    fn mime_for_path_js() {
        assert_eq!(mime_for_path("app.js"), "application/javascript");
    }

    #[test]
    fn mime_for_path_mjs() {
        assert_eq!(mime_for_path("app.mjs"), "application/javascript");
    }

    #[test]
    fn mime_for_path_json() {
        assert_eq!(mime_for_path("data.json"), "application/json");
    }

    #[test]
    fn mime_for_path_svg() {
        assert_eq!(mime_for_path("logo.svg"), "image/svg+xml");
    }

    #[test]
    fn mime_for_path_png() {
        assert_eq!(mime_for_path("img.png"), "image/png");
    }

    #[test]
    fn mime_for_path_jpg() {
        assert_eq!(mime_for_path("img.jpg"), "image/jpeg");
    }

    #[test]
    fn mime_for_path_jpeg() {
        assert_eq!(mime_for_path("img.jpeg"), "image/jpeg");
    }

    #[test]
    fn mime_for_path_gif() {
        assert_eq!(mime_for_path("img.gif"), "image/gif");
    }

    #[test]
    fn mime_for_path_webp() {
        assert_eq!(mime_for_path("img.webp"), "image/webp");
    }

    #[test]
    fn mime_for_path_ico() {
        assert_eq!(mime_for_path("favicon.ico"), "image/x-icon");
    }

    #[test]
    fn mime_for_path_woff() {
        assert_eq!(mime_for_path("font.woff"), "font/woff");
    }

    #[test]
    fn mime_for_path_woff2() {
        assert_eq!(mime_for_path("font.woff2"), "font/woff2");
    }

    #[test]
    fn mime_for_path_ttf() {
        assert_eq!(mime_for_path("font.ttf"), "font/ttf");
    }

    #[test]
    fn mime_for_path_unknown() {
        assert_eq!(mime_for_path("archive.zip"), "application/octet-stream");
    }

    // ── extract_zip ────────────────────────────────────────────────

    /// Helper: build a zip archive in memory with the given entries.
    fn build_zip(entries: Vec<(&str, &[u8])>) -> Vec<u8> {
        let buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            writer.start_file(name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn extract_zip_too_many_entries() {
        // Build a zip with 501 entries (limit is 500).
        let entries: Vec<(&str, &[u8])> = (0..501)
            .map(|i| {
                let name = Box::leak(format!("file{i}.html").into_boxed_str());
                (name as &str, b"x" as &[u8])
            })
            .collect();
        let data = build_zip(entries);
        let result = extract_zip(&data);
        match result {
            Err(ApiError::BadRequest(msg)) => {
                assert!(
                    msg.contains("too many zip entries"),
                    "unexpected msg: {msg}"
                );
            }
            other => panic!("expected BadRequest, got: {other:?}"),
        }
    }

    #[test]
    fn extract_zip_entry_name_too_long() {
        let long_name = "a".repeat(256); // limit is 255
        let entries: Vec<(&str, &[u8])> = vec![(&long_name, b"data" as &[u8])];
        let data = build_zip(entries);
        let result = extract_zip(&data);
        match result {
            Err(ApiError::BadRequest(msg)) => {
                assert!(msg.contains("name too long"), "unexpected msg: {msg}");
            }
            other => panic!("expected BadRequest, got: {other:?}"),
        }
    }

    #[test]
    fn extract_zip_too_large() {
        // Limit is 50 MB uncompressed. Build one entry that exceeds it.
        let big_data = vec![0u8; 51 * 1024 * 1024]; // 51 MB
        let entries: Vec<(&str, &[u8])> = vec![("big.html", &big_data)];
        let data = build_zip(entries);
        let result = extract_zip(&data);
        match result {
            Err(ApiError::BadRequest(msg)) => {
                assert!(msg.contains("zip too large"), "unexpected msg: {msg}");
            }
            other => panic!("expected BadRequest, got: {other:?}"),
        }
    }
}
