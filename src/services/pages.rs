use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::page::{self, Page, CreatePageParams, UpdatePageParams};
use crate::storage::ImageStorage;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Internal(String),
    #[error("{0}")]
    Conflict(String),
}

impl From<crate::models::error::ModelError> for ServiceError {
    fn from(e: crate::models::error::ModelError) -> Self {
        match e {
            crate::models::error::ModelError::NotFound(msg) => ServiceError::NotFound(msg),
            crate::models::error::ModelError::Conflict(msg) => ServiceError::Conflict(msg),
            crate::models::error::ModelError::Database(msg) => ServiceError::Internal(msg),
        }
    }
}

// ---------------------------------------------------------------------------
// Pure utility functions
// ---------------------------------------------------------------------------

pub fn is_safe_relative_path(name: &str) -> bool {
    if name.contains('\\') {
        return false;
    }
    if name.starts_with('.') || name.contains("/.") || name.contains("__MACOSX") {
        return false;
    }
    if name.ends_with('/') {
        return false;
    }
    use std::path::Component;
    std::path::Path::new(name)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
}

pub fn extract_zip(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ServiceError> {
    const MAX_ENTRIES: usize = 500;
    const MAX_NAME_LEN: usize = 255;

    let reader = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| ServiceError::BadRequest(format!("invalid zip: {e}")))?;
    let mut files = Vec::new();
    let mut total_extracted_size: usize = 0;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        let name = entry.name().to_string();

        if name.is_empty() {
            continue;
        }
        if name.len() > MAX_NAME_LEN {
            return Err(ServiceError::BadRequest(format!(
                "zip entry name too long (max {MAX_NAME_LEN} chars): {name}"
            )));
        }
        if files.len() >= MAX_ENTRIES {
            return Err(ServiceError::BadRequest(format!(
                "too many zip entries (max {MAX_ENTRIES})"
            )));
        }
        if !is_safe_relative_path(&name) {
            tracing::warn!(name = %name, "skipping unsafe zip entry");
            continue;
        }

        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut content)
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        total_extracted_size += content.len();
        if total_extracted_size > 50 * 1024 * 1024 {
            return Err(ServiceError::BadRequest(
                "zip too large (max 50MB uncompressed)".to_string(),
            ));
        }

        files.push((name, content));
    }

    strip_common_prefix(&mut files);

    Ok(files)
}

pub fn strip_common_prefix(files: &mut [(String, Vec<u8>)]) {
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

pub fn sanitize_filename(name: &str) -> String {
    name.replace("..", "")
        .split('/')
        .filter(|s| !s.is_empty() && !s.starts_with('.'))
        .collect::<Vec<_>>()
        .join("/")
}

pub fn extract_title(html_content: &[u8], fallback: &str) -> String {
    let content = String::from_utf8_lossy(html_content);
    let lower = content.to_lowercase();
    if let Some(start) = lower.find("<title>")
        && let Some(end) = lower.find("</title>")
    {
        let title = content[start + 7..end].trim();
        if !title.is_empty() {
            return title.to_string();
        }
    }
    fallback.trim_end_matches(".html").to_string()
}

pub fn mime_for_path(path: &str) -> &str {
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

// ---------------------------------------------------------------------------
// Async FS helpers
// ---------------------------------------------------------------------------

type DirEntries = Vec<(String, PathBuf)>;
type DirResult =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<DirEntries, std::io::Error>> + Send>>;

pub fn count_files_recursive(
    dir: PathBuf,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<usize, std::io::Error>> + Send>>
{
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

pub fn copy_dir_recursive(
    src: PathBuf,
    dst: PathBuf,
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

pub fn read_dir_recursive(dir: PathBuf) -> DirResult {
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

// ---------------------------------------------------------------------------
// Service: stage upload
// ---------------------------------------------------------------------------

pub struct StagedUpload {
    pub upload_id: String,
    pub html_files: Vec<String>,
    pub default_entry: String,
    pub suggested_title: String,
    pub file_count: usize,
    pub saved_files: HashMap<String, Vec<u8>>,
}

pub async fn stage_upload(
    temp_base: &Path,
    raw_files: Vec<(String, Vec<u8>)>,
    max_upload_bytes: usize,
) -> Result<StagedUpload, ServiceError> {
    tokio::fs::create_dir_all(temp_base)
        .await
        .map_err(|e| ServiceError::Internal(e.to_string()))?;

    let mut html_files: Vec<String> = Vec::new();
    let mut file_count: usize = 0;
    let mut total_size: usize = 0;
    let mut saved_files: HashMap<String, Vec<u8>> = HashMap::new();

    for (filename, data) in raw_files {
        total_size += data.len();
        if total_size > max_upload_bytes {
            tokio::fs::remove_dir_all(temp_base).await.ok();
            return Err(ServiceError::BadRequest("upload too large".to_string()));
        }

        if filename.ends_with(".zip") {
            let extracted = extract_zip(&data)?;
            for (name, content) in extracted {
                let path = temp_base.join(&name);
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| ServiceError::Internal(e.to_string()))?;
                }
                tokio::fs::write(&path, &content)
                    .await
                    .map_err(|e| ServiceError::Internal(e.to_string()))?;
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
                    .map_err(|e| ServiceError::Internal(e.to_string()))?;
            }
            tokio::fs::write(&path, &data)
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
            if safe_name.ends_with(".html") {
                html_files.push(safe_name.clone());
            }
            saved_files.insert(safe_name, data.to_vec());
            file_count += 1;
        }
    }

    if file_count == 0 {
        tokio::fs::remove_dir_all(temp_base).await.ok();
        return Err(ServiceError::BadRequest("no files uploaded".to_string()));
    }

    if html_files.is_empty() {
        tokio::fs::remove_dir_all(temp_base).await.ok();
        return Err(ServiceError::BadRequest("no HTML files found".to_string()));
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

    let cleanup_path = temp_base.to_path_buf();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1800)).await;
        tokio::fs::remove_dir_all(&cleanup_path).await.ok();
    });

    Ok(StagedUpload {
        upload_id: temp_base
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("")
            .to_string(),
        html_files,
        default_entry,
        suggested_title,
        file_count,
        saved_files,
    })
}

// ---------------------------------------------------------------------------
// Service: create page
// ---------------------------------------------------------------------------

pub struct CreatePageServiceParams {
    pub upload_id: String,
    pub entry_file: String,
    pub title: String,
    pub description: Option<String>,
    pub password: Option<String>,
    pub expires_at: Option<String>,
}

pub async fn create_page(
    pool: &PgPool,
    user_id: Uuid,
    storage: &dyn ImageStorage,
    storage_type: &str,
    pages_storage_path: &str,
    tmp_storage_path: &str,
    params: CreatePageServiceParams,
) -> Result<Page, ServiceError> {
    let temp_base = PathBuf::from(tmp_storage_path).join(&params.upload_id);

    let exists = tokio::fs::try_exists(&temp_base)
        .await
        .map_err(|e| ServiceError::Internal(e.to_string()))?;
    if !exists {
        return Err(ServiceError::NotFound("upload session expired".to_string()));
    }

    let file_count = count_files_recursive(temp_base.clone())
        .await
        .map_err(|e| ServiceError::Internal(e.to_string()))?;

    let password = match &params.password {
        Some(pw) if !pw.is_empty() => {
            Some(crate::auth::password::hash_page_password(pw).map_err(|_| {
                ServiceError::BadRequest("password too long (max 128 chars)".to_string())
            })?)
        }
        _ => None,
    };

    let expires_at = params
        .expires_at
        .as_deref()
        .map(chrono::DateTime::parse_from_rfc3339)
        .transpose()
        .map_err(|_| {
            ServiceError::BadRequest("invalid expires_at format, expected ISO 8601".to_string())
        })?
        .map(|dt| dt.to_utc());

    let new_page = page::create_page_with_retry(
        pool,
        user_id,
        &CreatePageParams {
            title: params.title,
            description: params.description,
            entry_file: params.entry_file,
            password_hash: password,
            file_count: file_count as i32,
            expires_at,
        },
    )
    .await?;

    let slug = new_page.slug.clone();

    if storage_type == "local" {
        let pages_base = PathBuf::from(pages_storage_path).join(&slug);
        tokio::fs::create_dir_all(&pages_base)
            .await
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        copy_dir_recursive(temp_base.clone(), pages_base)
            .await
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
    } else {
        let files = read_dir_recursive(temp_base.clone())
            .await
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        for (relative_path, absolute_path) in &files {
            let key = format!("pages/{}/{}", slug, relative_path);
            let data = tokio::fs::read(absolute_path)
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
            let mime = mime_for_path(relative_path);
            storage
                .store(&key, &data, mime)
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
        }
    }

    tokio::fs::remove_dir_all(&temp_base).await.ok();

    tracing::info!(page_id = %new_page.id, slug = %slug, "page created");

    Ok(new_page)
}

// ---------------------------------------------------------------------------
// Service: update page
// ---------------------------------------------------------------------------

pub struct UpdatePageServiceParams {
    pub title: Option<String>,
    pub description: Option<String>,
    pub password: Option<String>,
    pub status: Option<String>,
    pub expires_at: Option<String>,
    pub upload_id: Option<String>,
    pub entry_file: Option<String>,
}

pub async fn update_page(
    pool: &PgPool,
    user_id: Uuid,
    page_id: Uuid,
    storage: &dyn ImageStorage,
    storage_type: &str,
    pages_storage_path: &str,
    tmp_storage_path: &str,
    params: UpdatePageServiceParams,
) -> Result<Page, ServiceError> {
    if let Some(ref status) = params.status
        && status != "active"
        && status != "disabled"
    {
        return Err(ServiceError::BadRequest(
            "status must be 'active' or 'disabled'".to_string(),
        ));
    }

    let expires_at = match params.expires_at {
        Some(s) if s == "none" => Some(None),
        Some(s) => {
            let dt = chrono::DateTime::parse_from_rfc3339(&s).map_err(|_| {
                ServiceError::BadRequest(
                    "invalid expires_at format, expected ISO 8601".to_string(),
                )
            })?;
            Some(Some(dt.to_utc()))
        }
        None => None,
    };

    let (entry_file, file_count) = if let Some(ref upload_id) = params.upload_id {
        let temp_base = PathBuf::from(tmp_storage_path).join(upload_id);
        let exists = tokio::fs::try_exists(&temp_base)
            .await
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        if !exists {
            return Err(ServiceError::NotFound("upload session expired".to_string()));
        }

        let existing = page::find_page_by_id(pool, user_id, page_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound("page not found".to_string()))?;
        let slug = existing.slug.clone();

        if storage_type == "local" {
            let pages_base = PathBuf::from(pages_storage_path).join(&slug);
            tokio::fs::remove_dir_all(&pages_base).await.ok();
            tokio::fs::create_dir_all(&pages_base)
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
            copy_dir_recursive(temp_base.clone(), pages_base)
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
        } else {
            let prefix = format!("pages/{}/", slug);
            storage
                .delete_prefix(&prefix)
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
            let files = read_dir_recursive(temp_base.clone())
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
            for (relative_path, absolute_path) in &files {
                let key = format!("pages/{}/{}", slug, relative_path);
                let data = tokio::fs::read(absolute_path)
                    .await
                    .map_err(|e| ServiceError::Internal(e.to_string()))?;
                let mime = mime_for_path(relative_path);
                storage
                    .store(&key, &data, mime)
                    .await
                    .map_err(|e| ServiceError::Internal(e.to_string()))?;
            }
        }

        let count = count_files_recursive(temp_base.clone())
            .await
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        tokio::fs::remove_dir_all(&temp_base).await.ok();

        (params.entry_file.clone(), Some(count as i32))
    } else {
        (params.entry_file.clone(), None)
    };

    let password = match params.password {
        Some(ref pw) if !pw.is_empty() => {
            Some(crate::auth::password::hash_page_password(pw).map_err(|_| {
                ServiceError::BadRequest("password too long (max 128 chars)".to_string())
            })?)
        }
        Some(ref pw) if pw.is_empty() => Some(String::new()),
        _ => None,
    };

    let updated = page::update_page(
        pool,
        user_id,
        page_id,
        &UpdatePageParams {
            title: params.title,
            description: params.description,
            password,
            status: params.status,
            expires_at,
            entry_file,
            file_count,
        },
    )
    .await?;

    Ok(updated)
}
