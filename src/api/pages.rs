use axum::extract::{Multipart, Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;
use validator::Validate;
use axum::extract::multipart::MultipartError;

use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::state::AppState;
use crate::models::page;

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
    tokio::fs::create_dir_all(&temp_base).await.map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut html_files: Vec<String> = Vec::new();
    let mut file_count: usize = 0;
    let mut total_size: usize = 0;
    let mut saved_files: HashMap<String, Vec<u8>> = HashMap::new();

    let mut multipart = multipart;
    while let Some(field) = multipart.next_field().await.map_err(|e: MultipartError| ApiError::BadRequest(e.to_string()))? {
        let filename = field.file_name().unwrap_or("unknown").to_string();
        let data = field.bytes().await.map_err(|e: MultipartError| ApiError::BadRequest(e.to_string()))?;
        total_size += data.len();
        if total_size > 10 * 1024 * 1024 {
            tokio::fs::remove_dir_all(&temp_base).await.ok();
            return Err(ApiError::BadRequest("upload too large (max 10MB)".to_string()));
        }

        if filename.ends_with(".zip") {
            let extracted = extract_zip(&data)?;
            for (name, content) in extracted {
                let path = temp_base.join(&name);
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await.map_err(|e| ApiError::Internal(e.to_string()))?;
                }
                tokio::fs::write(&path, &content).await.map_err(|e| ApiError::Internal(e.to_string()))?;
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
                tokio::fs::create_dir_all(parent).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            }
            tokio::fs::write(&path, &data).await.map_err(|e: std::io::Error| ApiError::Internal(e.to_string()))?;
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

    let default_entry = html_files.iter()
        .find(|f| **f == "index.html" || f.ends_with("/index.html"))
        .or_else(|| html_files.first())
        .unwrap()
        .clone();

    let suggested_title = extract_title(saved_files.get(&default_entry).unwrap(), &default_entry);

    let cleanup_path = temp_base.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1800)).await;
        tokio::fs::remove_dir_all(&cleanup_path).await.ok();
    });

    Ok(Json(UploadResponse {
        upload_id,
        html_files,
        default_entry,
        suggested_title,
        file_count,
    }))
}

fn extract_zip(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ApiError> {
    let reader = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| ApiError::BadRequest(format!("invalid zip: {e}")))?;
    let mut files = Vec::new();
    let mut total_extracted_size: usize = 0;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| ApiError::Internal(e.to_string()))?;
        let name = entry.name().to_string();

        if name.ends_with('/') || name.starts_with('.') || name.contains("__MACOSX") || name.contains("/.") {
            continue;
        }
        if name.contains("..") {
            continue;
        }

        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut content).map_err(|e| ApiError::Internal(e.to_string()))?;
        total_extracted_size += content.len();
        if total_extracted_size > 50 * 1024 * 1024 {
            return Err(ApiError::BadRequest("zip too large (max 50MB uncompressed)".to_string()));
        }

        files.push((name, content));
    }

    strip_common_prefix(&mut files);

    Ok(files)
}

fn strip_common_prefix(files: &mut Vec<(String, Vec<u8>)>) {
    if files.is_empty() { return; }
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
) -> Result<Json<serde_json::Value>, ApiError> {
    let temp_base = tmp_dir(&state).join(&req.upload_id);

    let exists = tokio::fs::try_exists(&temp_base).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    if !exists {
        return Err(ApiError::NotFound("upload session expired".to_string()));
    }

    let file_count = count_files_recursive(temp_base.clone()).await.map_err(|e| ApiError::Internal(e.to_string()))?;

    let password = match &req.password {
        Some(pw) if !pw.is_empty() => Some(pw.clone()),
        _ => None,
    };

    let expires_at = req.expires_at.as_deref()
        .map(|s| chrono::DateTime::parse_from_rfc3339(s))
        .transpose()
        .map_err(|_| ApiError::BadRequest("invalid expires_at format, expected ISO 8601".to_string()))?
        .map(|dt| dt.to_utc());

    let new_page = page::create_page_with_retry(
        &state.pool, auth.user_id, &req.title,
        req.description.as_deref(), &req.entry_file,
        password.as_deref(), file_count as i32,
        expires_at,
    ).await?;

    let slug = new_page.slug.clone();
    let password_for_url = req.password.clone();

    if state.config.storage_type == "local" {
        let pages_base = pages_storage_path(&state).join(&slug);
        tokio::fs::create_dir_all(&pages_base).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        copy_dir_recursive(temp_base.clone(), pages_base).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    } else {
        let files = read_dir_recursive(temp_base.clone()).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        for (relative_path, absolute_path) in &files {
            let key = format!("pages/{}/{}", slug, relative_path);
            let data = tokio::fs::read(absolute_path).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            let mime = mime_for_path(relative_path);
            state.storage.store(&key, &data, mime).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        }
    }

    tokio::fs::remove_dir_all(&temp_base).await.ok();

    tracing::info!(page_id = %new_page.id, slug = %slug, "page created");

    let url = format!("/p/{}", new_page.slug);
    let url_with_password = password_for_url.as_ref()
        .filter(|pw| !pw.is_empty())
        .map(|pw| format!("{}?p={}", url, pw));

    Ok(Json(serde_json::json!({
        "id": new_page.id,
        "slug": new_page.slug,
        "title": new_page.title,
        "url": url,
        "url_with_password": url_with_password,
        "has_password": password_for_url.as_ref().filter(|pw| !pw.is_empty()).is_some(),
        "created_at": new_page.created_at,
    })))
}

fn count_files_recursive(dir: std::path::PathBuf) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<usize, std::io::Error>> + Send>> {
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

fn copy_dir_recursive(src: std::path::PathBuf, dst: std::path::PathBuf) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), std::io::Error>> + Send>> {
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

fn read_dir_recursive(dir: std::path::PathBuf) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<(String, std::path::PathBuf)>, std::io::Error>> + Send>> {
    Box::pin(async move {
        let mut files = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let sub = read_dir_recursive(path.clone()).await?;
                for (_rel, abs) in sub {
                    let prefix = dir.to_str().unwrap_or("");
                    let relative = abs.strip_prefix(prefix).unwrap_or(&abs).to_str().unwrap_or("").trim_start_matches('/').to_string();
                    files.push((relative, abs));
                }
            } else {
                let relative = path.strip_prefix(&dir).unwrap_or(&path).to_str().unwrap_or("").trim_start_matches('/').to_string();
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
        &state.pool, auth.user_id,
        params.status.as_deref(), page_num, limit,
    ).await?;
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
            return Err(ApiError::BadRequest("status must be 'active' or 'disabled'".to_string()));
        }
    }
    let expires_at = match req.expires_at {
        Some(Some(s)) => {
            let dt = chrono::DateTime::parse_from_rfc3339(&s)
                .map_err(|_| ApiError::BadRequest("invalid expires_at format, expected ISO 8601".to_string()))?;
            Some(Some(dt.to_utc()))
        }
        Some(None) => Some(None), // explicitly clear expiration
        None => None, // don't change
    };

    // Handle file replacement if upload_id is provided
    let (entry_file, file_count) = if let Some(ref upload_id) = req.upload_id {
        let temp_base = tmp_dir(&state).join(upload_id);
        let exists = tokio::fs::try_exists(&temp_base).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        if !exists {
            return Err(ApiError::NotFound("upload session expired".to_string()));
        }

        // Find existing page to get slug
        let existing = page::find_page_by_id(&state.pool, auth.user_id, page_id).await?
            .ok_or_else(|| ApiError::NotFound("page not found".to_string()))?;
        let slug = existing.slug.clone();

        // Delete old files and copy new ones
        if state.config.storage_type == "local" {
            let pages_base = pages_storage_path(&state).join(&slug);
            // Remove old files (ignore if directory doesn't exist)
            tokio::fs::remove_dir_all(&pages_base).await.ok();
            tokio::fs::create_dir_all(&pages_base).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            // Copy new files
            copy_dir_recursive(temp_base.clone(), pages_base).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        } else {
            // Delete old S3 objects
            let prefix = format!("pages/{}/", slug);
            state.storage.delete_prefix(&prefix).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            // Upload new files
            let files = read_dir_recursive(temp_base.clone()).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            for (relative_path, absolute_path) in &files {
                let key = format!("pages/{}/{}", slug, relative_path);
                let data = tokio::fs::read(absolute_path).await.map_err(|e| ApiError::Internal(e.to_string()))?;
                let mime = mime_for_path(relative_path);
                state.storage.store(&key, &data, mime).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            }
        }

        let count = count_files_recursive(temp_base.clone()).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        // Cleanup temp dir
        tokio::fs::remove_dir_all(&temp_base).await.ok();

        (req.entry_file.clone(), Some(count as i32))
    } else {
        (req.entry_file.clone(), None)
    };

    let updated = page::update_page(
        &state.pool, auth.user_id, page_id,
        &page::UpdatePageParams {
            title: req.title,
            description: req.description,
            password: req.password,
            status: req.status,
            expires_at,
            entry_file,
            file_count,
        },
    ).await?;
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
    Ok(Json(serde_json::json!({"success": true})))
}

#[tracing::instrument(skip(state), err)]
pub async fn restore_page_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    page::restore_page(&state.pool, auth.user_id, page_id).await?;
    Ok(Json(serde_json::json!({"success": true})))
}

#[derive(Debug, Serialize)]
pub struct ShareUrlResponse {
    pub url: String,
    pub password: Option<String>,
    pub has_password: bool,
}

#[tracing::instrument(skip(state), err)]
pub async fn get_share_url_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
) -> Result<Json<ShareUrlResponse>, ApiError> {
    let page = page::find_page_by_id(&state.pool, auth.user_id, page_id).await?
        .ok_or_else(|| ApiError::NotFound("page not found".to_string()))?;
    
    let base_url = format!("/p/{}", page.slug);
    let has_password = page.password.is_some();
    
    Ok(Json(ShareUrlResponse {
        url: base_url,
        password: page.password,
        has_password,
    }))
}
