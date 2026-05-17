use axum::Json;
use axum::extract::multipart::MultipartError;
use axum::extract::{Multipart, Path, Query, State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::page;
use crate::models::page::{PageResponse, PageSummaryResponse};
use crate::services::pages::{
    self as svc, CreatePageServiceParams, ServiceError, UpdatePageServiceParams,
};
use crate::state::AppState;

use super::validate::ValidatedJson;

// ---------------------------------------------------------------------------
// Response / request types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PageListResponse {
    pub items: Vec<PageSummaryResponse>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PageDeleteResponse {
    pub success: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PageRestoreResponse {
    pub id: Uuid,
    pub success: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct UploadResponse {
    upload_id: String,
    html_files: Vec<String>,
    default_entry: String,
    suggested_title: String,
    file_count: usize,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreatePageRequest {
    pub upload_id: String,
    pub entry_file: String,
    #[validate(length(min = 1, max = 500))]
    pub title: String,
    pub description: Option<String>,
    pub password: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ListQueryParams {
    pub status: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdatePageRequest {
    #[validate(length(max = 500))]
    pub title: Option<String>,
    pub description: Option<String>,
    pub password: Option<String>,
    pub status: Option<String>,
    pub expires_at: Option<String>,
    pub upload_id: Option<String>,
    pub entry_file: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ShareUrlResponse {
    pub url: String,
    pub has_password: bool,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

impl From<ServiceError> for ApiError {
    fn from(e: ServiceError) -> Self {
        match e {
            ServiceError::BadRequest(msg) => ApiError::BadRequest(msg),
            ServiceError::NotFound(msg) => ApiError::NotFound(msg),
            ServiceError::Internal(msg) => ApiError::Internal(msg),
            ServiceError::Conflict(msg) => ApiError::Conflict(msg),
        }
    }
}

fn tmp_dir(state: &AppState) -> std::path::PathBuf {
    std::path::PathBuf::from(&state.config.pages_storage_path).join("tmp")
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[tracing::instrument(skip(state, multipart), err)]
pub async fn upload_files(
    State(state): State<AppState>,
    auth: AuthUser,
    multipart: Multipart,
) -> Result<Json<UploadResponse>, ApiError> {
    let upload_id = Uuid::new_v4().to_string();
    let temp_base = tmp_dir(&state).join(&upload_id);

    let mut raw_files: Vec<(String, Vec<u8>)> = Vec::new();
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
        raw_files.push((filename, data.to_vec()));
    }

    let staged =
        svc::stage_upload(&temp_base, raw_files, state.config.pages_max_upload_bytes).await?;

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::UploadPageFiles,
        Some(AuditResourceType::Page),
        None,
        serde_json::json!({"upload_id": staged.upload_id, "file_count": staged.file_count}),
    )
    .await;

    Ok(Json(UploadResponse {
        upload_id: staged.upload_id,
        html_files: staged.html_files,
        default_entry: staged.default_entry,
        suggested_title: staged.suggested_title,
        file_count: staged.file_count,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/pages",
    tag = "pages",
    request_body = CreatePageRequest,
    responses(
        (status = 200, description = "Page created", body = PageResponse),
        (status = 401, description = "Missing or invalid auth"),
        (status = 404, description = "Upload session expired"),
        (status = 422, description = "Validation error"),
    ),
    security(("bearer" = [])),
)]
#[tracing::instrument(skip(state), err)]
pub async fn create_page_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<CreatePageRequest>,
) -> Result<Json<page::Page>, ApiError> {
    let new_page = svc::create_page(
        &state.pool,
        auth.user_id,
        state.storage.as_ref(),
        &state.config.storage_type,
        &state.config.pages_storage_path,
        &tmp_dir(&state).to_string_lossy(),
        CreatePageServiceParams {
            upload_id: req.upload_id,
            entry_file: req.entry_file,
            title: req.title,
            description: req.description,
            password: req.password,
            expires_at: req.expires_at,
        },
    )
    .await?;

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

#[utoipa::path(
    get,
    path = "/api/v1/pages",
    tag = "pages",
    params(ListQueryParams),
    responses(
        (status = 200, description = "List of pages", body = PageListResponse),
        (status = 401, description = "Missing or invalid auth"),
    ),
    security(("bearer" = [])),
)]
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

#[utoipa::path(
    patch,
    path = "/api/v1/pages/{id}",
    tag = "pages",
    params(
        ("id" = Uuid, Path, description = "Page ID"),
    ),
    request_body = UpdatePageRequest,
    responses(
        (status = 200, description = "Page updated", body = PageResponse),
        (status = 401, description = "Missing or invalid auth"),
        (status = 404, description = "Page not found"),
        (status = 422, description = "Validation error"),
    ),
    security(("bearer" = [])),
)]
#[tracing::instrument(skip(state), err)]
pub async fn update_page_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
    ValidatedJson(req): ValidatedJson<UpdatePageRequest>,
) -> Result<Json<page::Page>, ApiError> {
    let updated = svc::update_page(
        &state.pool,
        auth.user_id,
        page_id,
        state.storage.as_ref(),
        &state.config.storage_type,
        &state.config.pages_storage_path,
        &tmp_dir(&state).to_string_lossy(),
        UpdatePageServiceParams {
            title: req.title,
            description: req.description,
            password: req.password,
            status: req.status,
            expires_at: req.expires_at,
            upload_id: req.upload_id,
            entry_file: req.entry_file,
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

#[utoipa::path(
    delete,
    path = "/api/v1/pages/{id}",
    tag = "pages",
    params(
        ("id" = Uuid, Path, description = "Page ID"),
    ),
    responses(
        (status = 200, description = "Page deleted", body = PageDeleteResponse),
        (status = 401, description = "Missing or invalid auth"),
        (status = 404, description = "Page not found"),
    ),
    security(("bearer" = [])),
)]
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

#[utoipa::path(
    post,
    path = "/api/v1/pages/{id}/restore",
    tag = "pages",
    params(
        ("id" = Uuid, Path, description = "Page ID"),
    ),
    responses(
        (status = 200, description = "Page restored", body = PageRestoreResponse),
        (status = 401, description = "Missing or invalid auth"),
        (status = 404, description = "Page not found or not deleted"),
    ),
    security(("bearer" = [])),
)]
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
    Ok(Json(serde_json::json!({"id": page_id, "success": true})))
}

#[utoipa::path(
    get,
    path = "/api/v1/pages/{id}/share-url",
    tag = "pages",
    params(
        ("id" = Uuid, Path, description = "Page ID"),
    ),
    responses(
        (status = 200, description = "Share URL", body = ShareUrlResponse),
        (status = 401, description = "Missing or invalid auth"),
        (status = 404, description = "Page not found"),
    ),
    security(("bearer" = [])),
)]
#[tracing::instrument(skip(state), err)]
pub async fn get_share_url_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
) -> Result<Json<ShareUrlResponse>, ApiError> {
    let page = page::find_page_by_id(&state.pool, auth.user_id, page_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("page not found".to_string()))?;

    Ok(Json(ShareUrlResponse {
        url: format!("/p/{}", page.slug),
        has_password: page.password.is_some(),
    }))
}

// ---------------------------------------------------------------------------
// Tests (moved from handler; test the pure service functions)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ── is_safe_relative_path ──────────────────────────────────────

    #[test]
    fn is_safe_relative_path_normal_relative() {
        assert!(svc::is_safe_relative_path("dir/file.html"));
    }

    #[test]
    fn is_safe_relative_path_traversal() {
        assert!(!svc::is_safe_relative_path("../etc/passwd"));
    }

    #[test]
    fn is_safe_relative_path_absolute() {
        assert!(!svc::is_safe_relative_path("/etc/passwd"));
    }

    #[test]
    fn is_safe_relative_path_backslash() {
        assert!(!svc::is_safe_relative_path("dir\\file.html"));
    }

    #[test]
    fn is_safe_relative_path_hidden_file() {
        assert!(!svc::is_safe_relative_path(".env"));
    }

    #[test]
    fn is_safe_relative_path_hidden_dir() {
        assert!(!svc::is_safe_relative_path(".git/config"));
    }

    #[test]
    fn is_safe_relative_path_macosx() {
        assert!(!svc::is_safe_relative_path("__MACOSX/file"));
    }

    #[test]
    fn is_safe_relative_path_directory_entry() {
        assert!(!svc::is_safe_relative_path("dir/"));
    }

    #[test]
    fn is_safe_relative_path_normal_multi_level() {
        assert!(svc::is_safe_relative_path("css/style.css"));
    }

    #[test]
    fn is_safe_relative_path_dot_in_middle() {
        assert!(svc::is_safe_relative_path("my.file.html"));
    }

    // ── sanitize_filename ──────────────────────────────────────────

    #[test]
    fn sanitize_filename_removes_double_dot() {
        assert_eq!(svc::sanitize_filename("dir/../file.html"), "dir/file.html");
    }

    #[test]
    fn sanitize_filename_filters_empty_and_dot_prefixed() {
        assert_eq!(
            svc::sanitize_filename("../.hidden/./file.html"),
            "file.html"
        );
    }

    #[test]
    fn sanitize_filename_normal_unchanged() {
        assert_eq!(svc::sanitize_filename("style.css"), "style.css");
    }

    #[test]
    fn sanitize_filename_collapses_multiple_slashes() {
        assert_eq!(svc::sanitize_filename("dir///file.html"), "dir/file.html");
    }

    // ── extract_title ──────────────────────────────────────────────

    #[test]
    fn extract_title_from_tag() {
        let html = b"<html><head><title>My Title</title></head><body></body></html>";
        assert_eq!(svc::extract_title(html, "fallback.html"), "My Title");
    }

    #[test]
    fn extract_title_case_insensitive() {
        let html = b"<HTML><HEAD><TITLE>My Title</TITLE></HEAD><BODY></BODY></HTML>";
        assert_eq!(svc::extract_title(html, "fallback.html"), "My Title");
    }

    #[test]
    fn extract_title_empty_falls_back_to_filename() {
        let html = b"<html><head><title></title></head><body></body></html>";
        assert_eq!(svc::extract_title(html, "page.html"), "page");
    }

    #[test]
    fn extract_title_no_tag_falls_back_to_filename() {
        let html = b"<html><head></head><body></body></html>";
        assert_eq!(svc::extract_title(html, "page.html"), "page");
    }

    #[test]
    fn extract_title_whitespace_trimmed() {
        let html = b"<html><head><title>  Spaced Title  </title></head><body></body></html>";
        assert_eq!(svc::extract_title(html, "fallback.html"), "Spaced Title");
    }

    // ── strip_common_prefix ────────────────────────────────────────

    #[test]
    fn strip_common_prefix_strips() {
        let mut files = vec![
            ("root/index.html".to_string(), vec![1u8]),
            ("root/style.css".to_string(), vec![2u8]),
        ];
        svc::strip_common_prefix(&mut files);
        assert_eq!(files[0].0, "index.html");
        assert_eq!(files[1].0, "style.css");
    }

    #[test]
    fn strip_common_prefix_no_common() {
        let mut files = vec![
            ("a/index.html".to_string(), vec![1u8]),
            ("b/style.css".to_string(), vec![2u8]),
        ];
        svc::strip_common_prefix(&mut files);
        assert_eq!(files[0].0, "a/index.html");
        assert_eq!(files[1].0, "b/style.css");
    }

    #[test]
    fn strip_common_prefix_single_file_no_slash() {
        let mut files = vec![("index.html".to_string(), vec![1u8])];
        svc::strip_common_prefix(&mut files);
        assert_eq!(files[0].0, "index.html");
    }

    #[test]
    fn strip_common_prefix_empty_list() {
        let mut files: Vec<(String, Vec<u8>)> = vec![];
        svc::strip_common_prefix(&mut files);
        assert!(files.is_empty());
    }

    // ── mime_for_path ──────────────────────────────────────────────

    #[test]
    fn mime_for_path_html() {
        assert_eq!(svc::mime_for_path("page.html"), "text/html; charset=utf-8");
    }

    #[test]
    fn mime_for_path_htm() {
        assert_eq!(svc::mime_for_path("page.htm"), "text/html; charset=utf-8");
    }

    #[test]
    fn mime_for_path_css() {
        assert_eq!(svc::mime_for_path("style.css"), "text/css; charset=utf-8");
    }

    #[test]
    fn mime_for_path_js() {
        assert_eq!(svc::mime_for_path("app.js"), "application/javascript");
    }

    #[test]
    fn mime_for_path_mjs() {
        assert_eq!(svc::mime_for_path("app.mjs"), "application/javascript");
    }

    #[test]
    fn mime_for_path_json() {
        assert_eq!(svc::mime_for_path("data.json"), "application/json");
    }

    #[test]
    fn mime_for_path_svg() {
        assert_eq!(svc::mime_for_path("logo.svg"), "image/svg+xml");
    }

    #[test]
    fn mime_for_path_png() {
        assert_eq!(svc::mime_for_path("img.png"), "image/png");
    }

    #[test]
    fn mime_for_path_jpg() {
        assert_eq!(svc::mime_for_path("img.jpg"), "image/jpeg");
    }

    #[test]
    fn mime_for_path_jpeg() {
        assert_eq!(svc::mime_for_path("img.jpeg"), "image/jpeg");
    }

    #[test]
    fn mime_for_path_gif() {
        assert_eq!(svc::mime_for_path("img.gif"), "image/gif");
    }

    #[test]
    fn mime_for_path_webp() {
        assert_eq!(svc::mime_for_path("img.webp"), "image/webp");
    }

    #[test]
    fn mime_for_path_ico() {
        assert_eq!(svc::mime_for_path("favicon.ico"), "image/x-icon");
    }

    #[test]
    fn mime_for_path_woff() {
        assert_eq!(svc::mime_for_path("font.woff"), "font/woff");
    }

    #[test]
    fn mime_for_path_woff2() {
        assert_eq!(svc::mime_for_path("font.woff2"), "font/woff2");
    }

    #[test]
    fn mime_for_path_ttf() {
        assert_eq!(svc::mime_for_path("font.ttf"), "font/ttf");
    }

    #[test]
    fn mime_for_path_unknown() {
        assert_eq!(
            svc::mime_for_path("archive.zip"),
            "application/octet-stream"
        );
    }

    // ── extract_zip ────────────────────────────────────────────────

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
        let entries: Vec<(&str, &[u8])> = (0..501)
            .map(|i| {
                let name = Box::leak(format!("file{i}.html").into_boxed_str());
                (name as &str, b"x" as &[u8])
            })
            .collect();
        let data = build_zip(entries);
        let result = svc::extract_zip(&data);
        match result {
            Err(ServiceError::BadRequest(msg)) => {
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
        let long_name = "a".repeat(256);
        let entries: Vec<(&str, &[u8])> = vec![(&long_name, b"data" as &[u8])];
        let data = build_zip(entries);
        let result = svc::extract_zip(&data);
        match result {
            Err(ServiceError::BadRequest(msg)) => {
                assert!(msg.contains("name too long"), "unexpected msg: {msg}");
            }
            other => panic!("expected BadRequest, got: {other:?}"),
        }
    }

    #[test]
    fn extract_zip_too_large() {
        let big_data = vec![0u8; 51 * 1024 * 1024];
        let entries: Vec<(&str, &[u8])> = vec![("big.html", &big_data)];
        let data = build_zip(entries);
        let result = svc::extract_zip(&data);
        match result {
            Err(ServiceError::BadRequest(msg)) => {
                assert!(msg.contains("zip too large"), "unexpected msg: {msg}");
            }
            other => panic!("expected BadRequest, got: {other:?}"),
        }
    }
}
