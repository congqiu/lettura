//! OpenAPI schema collection.
//!
//! Adding a new endpoint to the public schema is two steps:
//!   1. Add `#[utoipa::path(...)]` above the handler
//!   2. List the handler in the `paths(...)` block on `ApiDoc` below, and
//!      list any new request/response types in `components(schemas(...))`
//!
//! The schema is served at `GET /api/openapi.json`. The frontend regenerates
//! `web/src/api/schema.ts` from it via `pnpm codegen`.
//!
//! Incremental adoption is fine — handlers without `#[utoipa::path]` still
//! work, they just don't appear in the schema. Aim to annotate any handler
//! you're already editing for another reason.

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Lettura API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Lettura is a wallabag-inspired read-it-later service. \
                       Endpoints below are stable; unannotated routes exist \
                       but their schema is not yet machine-readable."
    ),
    paths(
        crate::api::health::health_check,
        crate::api::auth::register,
        crate::api::auth::login,
        crate::api::auth::refresh,
        crate::api::auth::logout,
        crate::api::auth::me,
        crate::api::auth::change_password,
        crate::api::auth::regenerate_feed_token,
        crate::api::tokens::list_tokens,
        crate::api::tokens::create_token,
        crate::api::tokens::delete_token,
        crate::api::audit_logs::list_audit_logs,
        crate::api::tags::list_tags,
        crate::api::tags::tags_stats,
        crate::api::tags::rename_tag_handler,
        crate::api::tags::list_tags_for_entry,
        crate::api::tags::add_tag_to_entry,
        crate::api::tags::remove_tag_from_entry,
        crate::api::tags::remove_tag_from_entry_by_label,
        crate::api::tags::delete_tag,
        crate::api::tagging_rules::list_rules,
        crate::api::tagging_rules::create_rule,
        crate::api::tagging_rules::update_rule,
        crate::api::tagging_rules::delete_rule,
        crate::api::site_rules::list_rules,
        crate::api::site_rules::create_rule,
        crate::api::site_rules::update_rule,
        crate::api::site_rules::delete_rule,
        crate::api::entries::create_entry,
        crate::api::entries::get_entry,
        crate::api::entries::list_entries,
        crate::api::entries::update_entry,
        crate::api::entries::delete_entry,
        crate::api::entries::restore_entry,
        crate::api::entries::permanently_delete_entry,
        crate::api::entries::refetch_entry,
        crate::api::annotations::list_annotations,
        crate::api::annotations::create_annotation,
        crate::api::annotations::update_annotation,
        crate::api::annotations::delete_annotation,
        crate::api::memos::list_memos,
        crate::api::memos::create_memo,
        crate::api::memos::delete_memo,
        crate::api::memos::promote_memo,
        crate::api::bulk::bulk_tag_add,
        crate::api::bulk::bulk_untag,
        crate::api::bulk::bulk_archive,
        crate::api::bulk::bulk_star,
        crate::api::bulk::bulk_tag_by_ids,
        crate::api::bulk::bulk_untag_by_ids,
        crate::api::bulk::bulk_delete_by_ids,
        crate::api::bulk::bulk_archive_by_ids,
        crate::api::import::import_wallabag,
        crate::api::import::import_browser,
        crate::api::import::import_lettura,
        crate::api::export::export_all,
        crate::api::admin::list_users,
        crate::api::admin::reindex,
        crate::api::fetch_jobs::list,
        crate::api::fetch_jobs::get,
        crate::api::fetch_jobs::delete,
        crate::api::fetch_jobs::retry,
        crate::api::fetch_jobs::retry_all_dead,
        crate::api::pages::list_pages_handler,
        crate::api::pages::create_page_handler,
        crate::api::pages::update_page_handler,
        crate::api::pages::delete_page_handler,
        crate::api::pages::restore_page_handler,
        crate::api::pages::get_share_url_handler,
        crate::api::backup::backup,
    ),
    components(schemas(
        crate::api::health::HealthResponse,
        crate::api::auth::RegisterRequest,
        crate::api::auth::LoginRequest,
        crate::api::auth::RefreshRequest,
        crate::api::auth::AuthResponse,
        crate::api::auth::MessageResponse,
        crate::api::auth::MeResponse,
        crate::api::auth::FeedTokenResponse,
        crate::api::auth::ChangePasswordRequest,
        crate::api::tokens::CreateTokenRequest,
        crate::api::tokens::CreateTokenResponse,
        crate::models::pat::PersonalAccessToken,
        crate::models::pat::Scope,
        crate::api::audit_logs::ListAuditLogsResponse,
        crate::models::audit_log::AuditLog,
        crate::models::audit_log::AuditAction,
        crate::models::audit_log::AuditResourceType,
        crate::models::audit_log::AuditDetails,
        crate::models::tag::Tag,
        crate::models::tag::TagStats,
        crate::models::tag::TagLabel,
        crate::models::entry::Entry,
        crate::models::entry::EntrySummary,
        crate::models::entry::ListParams,
        crate::models::entry::UpdateEntryParams,
        crate::api::entries::CreateEntryRequest,
        crate::api::entries::CreateEntryResponse,
        crate::api::tags::RenameTagRequest,
        crate::api::tags::AddTagRequest,
        crate::models::tagging_rule::TaggingRule,
        crate::models::tagging_rule::CreateTaggingRule,
        crate::models::tagging_rule::UpdateTaggingRule,
        crate::models::site_rule::SiteRule,
        crate::models::site_rule::CreateSiteRule,
        crate::models::site_rule::UpdateSiteRule,
        crate::models::annotation::Annotation,
        crate::models::annotation::CreateAnnotation,
        crate::models::annotation::UpdateAnnotation,
        crate::models::memo::Memo,
        crate::models::memo::CreateMemo,
        crate::models::entry::EntryTagLink,
        crate::api::bulk::BulkTagRequest,
        crate::api::bulk::BulkUntagRequest,
        crate::api::bulk::BulkStateRequest,
        crate::api::bulk::BulkResult,
        crate::api::bulk::BulkTagByIdsRequest,
        crate::api::bulk::BulkUntagByIdsRequest,
        crate::api::bulk::BulkDeleteByIdsRequest,
        crate::api::bulk::BulkArchiveByIdsRequest,
        crate::api::import::WallabagEntry,
        crate::api::import::LetturaExport,
        crate::api::export::ExportScope,
        crate::api::admin::UserSummary,
        crate::api::admin::ReindexResponse,
        crate::api::fetch_jobs::ListResponse,
        crate::api::fetch_jobs::RetryAllResponse,
        crate::models::fetch_job::FetchJobStatus,
        crate::models::fetch_job::FetchJobRow,
        crate::api::pages::UploadResponse,
        crate::api::pages::CreatePageRequest,
        crate::api::pages::UpdatePageRequest,
        crate::api::pages::ShareUrlResponse,
        crate::api::pages::PageListResponse,
        crate::api::pages::PageDeleteResponse,
        crate::api::pages::PageRestoreResponse,
        crate::models::page::PageResponse,
        crate::models::page::PageSummaryResponse,
        crate::api::backup::BackupUser,
        crate::api::backup::BackupEntry,
        crate::api::backup::BackupTag,
        crate::api::backup::BackupEntryTag,
        crate::api::backup::BackupAnnotation,
        crate::api::backup::BackupMemo,
        crate::api::backup::BackupTaggingRule,
        crate::api::backup::BackupSiteRule,
    )),
    tags(
        (name = "health", description = "Service liveness and readiness"),
        (name = "auth", description = "Authentication and user management"),
        (name = "tokens", description = "Personal access token management"),
        (name = "audit-logs", description = "Audit log queries"),
        (name = "tags", description = "User-defined tags applied to entries"),
        (name = "entries", description = "Saved entries (articles, bookmarks)"),
        (name = "tagging-rules", description = "Auto-tagging rules for new entries"),
        (name = "site-rules", description = "Per-domain extraction rules"),
        (name = "annotations", description = "Highlights and notes on entries"),
        (name = "memos", description = "Quick notes that can be promoted to entries"),
        (name = "bulk", description = "Bulk operations on entries"),
        (name = "import", description = "Import data from external sources"),
        (name = "export", description = "Export all user data"),
        (name = "admin", description = "Admin-only operations (backup, restore, reindex)"),
    ),
)]
pub struct ApiDoc;

/// Serve the OpenAPI document. No auth — the schema itself does not leak
/// data, only describes shapes.
pub async fn openapi_json() -> axum::Json<utoipa::openapi::OpenApi> {
    axum::Json(ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The schema must include the handlers we registered. If somebody adds
    /// a new utoipa::path but forgets to list it in ApiDoc, this catches it.
    #[test]
    fn schema_includes_registered_paths() {
        let doc = ApiDoc::openapi();
        let paths = doc.paths.paths.keys().cloned().collect::<Vec<_>>();
        assert!(
            paths.contains(&"/api/health".to_string()),
            "/api/health missing from OpenAPI schema, got: {paths:?}"
        );
        assert!(
            paths.contains(&"/api/v1/tags".to_string()),
            "/api/v1/tags missing from OpenAPI schema, got: {paths:?}"
        );
        assert!(
            paths.contains(&"/api/v1/entries".to_string()),
            "/api/v1/entries missing from OpenAPI schema, got: {paths:?}"
        );
        assert!(
            paths.contains(&"/api/v1/entries/{id}".to_string()),
            "/api/v1/entries/{{id}} missing from OpenAPI schema, got: {paths:?}"
        );
        assert!(
            paths.contains(&"/api/v1/entries/{id}/refetch".to_string()),
            "/api/v1/entries/{{id}}/refetch missing from OpenAPI schema, got: {paths:?}"
        );
        assert!(
            paths.contains(&"/api/v1/entries/{id}/restore".to_string()),
            "/api/v1/entries/{{id}}/restore missing from OpenAPI schema, got: {paths:?}"
        );
        assert!(
            paths.contains(&"/api/v1/entries/{id}/permanent".to_string()),
            "/api/v1/entries/{{id}}/permanent missing from OpenAPI schema, got: {paths:?}"
        );
    }

    /// Schema serializes to valid JSON — what the /api/openapi.json endpoint
    /// will hand to openapi-typescript on the frontend.
    #[test]
    fn schema_serializes_to_json() {
        let doc = ApiDoc::openapi();
        let json = serde_json::to_string(&doc).expect("OpenAPI schema must be JSON-serializable");
        assert!(json.contains("\"HealthResponse\""));
        assert!(json.contains("\"Tag\""));
        assert!(json.contains("\"Entry\""));
        assert!(json.contains("\"EntrySummary\""));
        assert!(json.contains("\"CreateEntryResponse\""));
    }
}
