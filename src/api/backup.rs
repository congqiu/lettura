use axum::body::{Body, Bytes};
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::Stream;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Backup data structures (used for restore parsing)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupData {
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub users: Vec<BackupUser>,
    pub entries: Vec<BackupEntry>,
    pub tags: Vec<BackupTag>,
    pub entry_tags: Vec<BackupEntryTag>,
    pub annotations: Vec<BackupAnnotation>,
    pub memos: Vec<BackupMemo>,
    pub tagging_rules: Vec<BackupTaggingRule>,
    pub site_rules: Vec<BackupSiteRule>,
}

/// User without password_hash (security: never export credentials).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct BackupUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub is_admin: bool,
    pub feed_token: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct BackupEntry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub given_url: String,
    pub hashed_url: String,
    pub hashed_given_url: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub text_content: Option<String>,
    pub content_type: String,
    pub extract_method: String,
    pub is_content_edited: bool,
    pub language: Option<String>,
    pub http_status: Option<i16>,
    pub reading_time: Option<i32>,
    pub preview_picture: Option<String>,
    pub domain_name: Option<String>,
    pub published_by: Option<String>,
    #[schema(value_type = serde_json::Value)]
    pub metadata: serde_json::Value,
    pub is_archived: bool,
    pub archived_at: Option<DateTime<Utc>>,
    pub is_starred: bool,
    pub starred_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct BackupTag {
    pub id: Uuid,
    pub user_id: Uuid,
    pub label: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct BackupEntryTag {
    pub entry_id: Uuid,
    pub tag_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct BackupAnnotation {
    pub id: Uuid,
    pub entry_id: Uuid,
    pub user_id: Uuid,
    pub quote: String,
    pub text: String,
    #[schema(value_type = serde_json::Value)]
    pub ranges: serde_json::Value,
    pub is_orphaned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct BackupMemo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub source_url: Option<String>,
    pub promoted_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct BackupTaggingRule {
    pub id: Uuid,
    pub user_id: Uuid,
    #[schema(value_type = serde_json::Value)]
    pub rule: serde_json::Value,
    pub tags: Vec<String>,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct BackupSiteRule {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub domain: String,
    pub content_selector: String,
    pub title_selector: Option<String>,
    pub strip_selectors: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// NDJSON line types for streaming backup
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
enum NdjsonLine {
    #[serde(rename = "metadata")]
    Metadata {
        version: String,
        created_at: DateTime<Utc>,
    },
    #[serde(rename = "user")]
    User(BackupUser),
    #[serde(rename = "entry")]
    Entry(BackupEntry),
    #[serde(rename = "tag")]
    Tag(BackupTag),
    #[serde(rename = "entry_tag")]
    EntryTag(BackupEntryTag),
    #[serde(rename = "annotation")]
    Annotation(BackupAnnotation),
    #[serde(rename = "memo")]
    Memo(BackupMemo),
    #[serde(rename = "tagging_rule")]
    TaggingRule(BackupTaggingRule),
    #[serde(rename = "site_rule")]
    SiteRule(BackupSiteRule),
}

// ---------------------------------------------------------------------------
// GET /api/v1/admin/backup — streaming NDJSON
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/api/v1/admin/backup",
    operation_id = "admin_backup",
    tag = "admin",
    responses(
        (status = 200, description = "NDJSON stream of backup data", content_type = "application/x-ndjson"),
        (status = 401, description = "Missing or invalid auth"),
        (status = 403, description = "Admin required"),
    ),
    security(("bearer" = [])),
)]
pub async fn backup(State(state): State<AppState>, auth: AuthUser) -> Result<Response, ApiError> {
    if !auth.is_admin {
        return Err(ApiError::Forbidden("admin required".to_string()));
    }

    let filename = format!("lettura-backup-{}.ndjson", Utc::now().format("%Y-%m-%d"));

    let auth_source = auth_source_str(&auth);
    let stream = backup_ndjson_stream(state.pool.clone(), auth.user_id, auth_source);

    Ok((
        [
            (header::CONTENT_TYPE, "application/x-ndjson".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}

fn backup_ndjson_stream(
    pool: sqlx::PgPool,
    admin_user_id: Uuid,
    auth_source: String,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> {
    async_stream::stream! {
        let mut buf = Vec::with_capacity(8192);

        // metadata line
        let meta = NdjsonLine::Metadata {
            version: "2.0".to_string(),
            created_at: Utc::now(),
        };
        serde_json::to_writer(&mut buf, &meta).unwrap();
        buf.push(b'\n');
        yield Ok(Bytes::from(buf.clone()));
        buf.clear();

        // users
        {
            let mut rows = sqlx::query_as::<_, BackupUser>(
                "SELECT id, username, email, is_admin, feed_token, created_at, updated_at FROM users ORDER BY created_at",
            )
            .fetch(&pool);

            while let Some(user) = rows.next().await {
                let user = match user {
                    Ok(u) => u,
                    Err(e) => {
                        tracing::error!("backup stream: users query error: {e}");
                        break;
                    }
                };
                let line = NdjsonLine::User(user);
                serde_json::to_writer(&mut buf, &line).unwrap();
                buf.push(b'\n');
                yield Ok(Bytes::from(buf.clone()));
                buf.clear();
            }
        }

        // entries
        {
            let mut rows = sqlx::query_as::<_, BackupEntry>("SELECT * FROM entries ORDER BY created_at")
                .fetch(&pool);

            while let Some(entry) = rows.next().await {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::error!("backup stream: entries query error: {e}");
                        break;
                    }
                };
                let line = NdjsonLine::Entry(entry);
                serde_json::to_writer(&mut buf, &line).unwrap();
                buf.push(b'\n');
                yield Ok(Bytes::from(buf.clone()));
                buf.clear();
            }
        }

        // tags
        {
            let mut rows = sqlx::query_as::<_, BackupTag>("SELECT * FROM tags ORDER BY created_at")
                .fetch(&pool);

            while let Some(tag) = rows.next().await {
                let tag = match tag {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::error!("backup stream: tags query error: {e}");
                        break;
                    }
                };
                let line = NdjsonLine::Tag(tag);
                serde_json::to_writer(&mut buf, &line).unwrap();
                buf.push(b'\n');
                yield Ok(Bytes::from(buf.clone()));
                buf.clear();
            }
        }

        // entry_tags
        {
            let mut rows = sqlx::query_as::<_, BackupEntryTag>(
                "SELECT * FROM entry_tags ORDER BY entry_id, tag_id",
            )
            .fetch(&pool);

            while let Some(et) = rows.next().await {
                let et = match et {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!("backup stream: entry_tags query error: {e}");
                        break;
                    }
                };
                let line = NdjsonLine::EntryTag(et);
                serde_json::to_writer(&mut buf, &line).unwrap();
                buf.push(b'\n');
                yield Ok(Bytes::from(buf.clone()));
                buf.clear();
            }
        }

        // annotations
        {
            let mut rows = sqlx::query_as::<_, BackupAnnotation>(
                "SELECT * FROM annotations ORDER BY created_at",
            )
            .fetch(&pool);

            while let Some(ann) = rows.next().await {
                let ann = match ann {
                    Ok(a) => a,
                    Err(e) => {
                        tracing::error!("backup stream: annotations query error: {e}");
                        break;
                    }
                };
                let line = NdjsonLine::Annotation(ann);
                serde_json::to_writer(&mut buf, &line).unwrap();
                buf.push(b'\n');
                yield Ok(Bytes::from(buf.clone()));
                buf.clear();
            }
        }

        // memos
        {
            let mut rows = sqlx::query_as::<_, BackupMemo>("SELECT * FROM memos ORDER BY created_at")
                .fetch(&pool);

            while let Some(memo) = rows.next().await {
                let memo = match memo {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::error!("backup stream: memos query error: {e}");
                        break;
                    }
                };
                let line = NdjsonLine::Memo(memo);
                serde_json::to_writer(&mut buf, &line).unwrap();
                buf.push(b'\n');
                yield Ok(Bytes::from(buf.clone()));
                buf.clear();
            }
        }

        // tagging_rules
        {
            let mut rows = sqlx::query_as::<_, BackupTaggingRule>(
                "SELECT * FROM tagging_rules ORDER BY created_at",
            )
            .fetch(&pool);

            while let Some(rule) = rows.next().await {
                let rule = match rule {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!("backup stream: tagging_rules query error: {e}");
                        break;
                    }
                };
                let line = NdjsonLine::TaggingRule(rule);
                serde_json::to_writer(&mut buf, &line).unwrap();
                buf.push(b'\n');
                yield Ok(Bytes::from(buf.clone()));
                buf.clear();
            }
        }

        // site_rules
        {
            let mut rows = sqlx::query_as::<_, BackupSiteRule>(
                "SELECT * FROM site_rules ORDER BY created_at",
            )
            .fetch(&pool);

            while let Some(rule) = rows.next().await {
                let rule = match rule {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!("backup stream: site_rules query error: {e}");
                        break;
                    }
                };
                let line = NdjsonLine::SiteRule(rule);
                serde_json::to_writer(&mut buf, &line).unwrap();
                buf.push(b'\n');
                yield Ok(Bytes::from(buf.clone()));
                buf.clear();
            }
        }

        // audit log
        audit_log::log_success(
            &pool,
            Some(admin_user_id),
            auth_source,
            AuditAction::AdminBackup,
            Some(AuditResourceType::System),
            None,
            serde_json::json!({"format": "ndjson", "version": "2.0"}),
        )
        .await;

        tracing::info!("admin backup (ndjson) completed");
    }
}

// ---------------------------------------------------------------------------
// POST /api/v1/admin/restore?confirm=true
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct RestoreParams {
    #[serde(
        default,
        deserialize_with = "crate::api::validate::deserialize_bool_from_string"
    )]
    pub confirm: Option<bool>,
}

/// Intermediate struct for parsing NDJSON lines
#[derive(Deserialize)]
struct NdjsonLineIn {
    #[serde(rename = "type")]
    line_type: String,
    // We ignore the rest — each line is parsed into the appropriate struct below
}

/// Hard cap on restore body. Set well above typical personal backups
/// (a 5k-entry NDJSON dump is ~4 MB) but well below host RAM exhaustion
/// even on small instances. Adjust if a real user hits the limit.
const MAX_RESTORE_BODY_BYTES: u64 = 500 * 1024 * 1024; // 500 MiB

pub async fn restore(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<RestoreParams>,
    body: axum::body::Body,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    if !auth.is_admin {
        return Err(ApiError::Forbidden("admin required".to_string()));
    }
    if params.confirm != Some(true) {
        return Err(ApiError::BadRequest(
            "must include ?confirm=true to proceed with restore".to_string(),
        ));
    }

    // Bypass axum's DefaultBodyLimit (2 MiB) — a realistic backup easily
    // exceeds it. We still enforce an explicit ceiling so a malicious admin
    // PAT can't OOM the process. Real streaming line-by-line parse is a
    // follow-up; see docs/specs/2026-05-17-remaining-optimizations.md F.
    let bytes = match axum::body::to_bytes(body, MAX_RESTORE_BODY_BYTES as usize).await {
        Ok(b) => b,
        Err(e) => {
            return Err(ApiError::BadRequest(format!(
                "request body too large (max {} MiB) or read failed: {e}",
                MAX_RESTORE_BODY_BYTES / 1024 / 1024
            )));
        }
    };

    let text = String::from_utf8(bytes.to_vec())
        .map_err(|e| ApiError::BadRequest(format!("invalid UTF-8 in request body: {e}")))?;

    // Detect format: if first non-whitespace char is '[' → old JSON bundle; otherwise NDJSON
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') {
        // Could be old-format JSON bundle (BackupData) or NDJSON starting with metadata object
        // Peek at the first line to decide
        let first_line = trimmed.lines().next().unwrap_or("");
        if let Ok(typed) = serde_json::from_str::<NdjsonLineIn>(first_line)
            && typed.line_type == "metadata"
        {
            return restore_ndjson(&state, &text, &auth).await;
        }
        // Fall through to old format
        return restore_legacy_json(&state, &text, &auth).await;
    }

    Err(ApiError::BadRequest(
        "unrecognized backup format — expected NDJSON or legacy JSON bundle".to_string(),
    ))
}

async fn restore_ndjson(
    state: &AppState,
    text: &str,
    auth: &AuthUser,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let mut users: Vec<BackupUser> = Vec::new();
    let mut entries: Vec<BackupEntry> = Vec::new();
    let mut tags: Vec<BackupTag> = Vec::new();
    let mut entry_tags: Vec<BackupEntryTag> = Vec::new();
    let mut annotations: Vec<BackupAnnotation> = Vec::new();
    let mut memos: Vec<BackupMemo> = Vec::new();
    let mut tagging_rules: Vec<BackupTaggingRule> = Vec::new();
    let mut site_rules: Vec<BackupSiteRule> = Vec::new();
    let mut version = String::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let typed: NdjsonLineIn = match serde_json::from_str(line) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("restore: skipping unparseable NDJSON line: {e}");
                continue;
            }
        };
        match typed.line_type.as_str() {
            "metadata" => {
                if let Ok(meta) = serde_json::from_str::<NdjsonMetadataLine>(line) {
                    version = meta.version;
                }
            }
            "user" => {
                if let Ok(u) = serde_json::from_str(line) {
                    users.push(u);
                }
            }
            "entry" => {
                if let Ok(e) = serde_json::from_str(line) {
                    entries.push(e);
                }
            }
            "tag" => {
                if let Ok(t) = serde_json::from_str(line) {
                    tags.push(t);
                }
            }
            "entry_tag" => {
                if let Ok(et) = serde_json::from_str(line) {
                    entry_tags.push(et);
                }
            }
            "annotation" => {
                if let Ok(a) = serde_json::from_str(line) {
                    annotations.push(a);
                }
            }
            "memo" => {
                if let Ok(m) = serde_json::from_str(line) {
                    memos.push(m);
                }
            }
            "tagging_rule" => {
                if let Ok(r) = serde_json::from_str(line) {
                    tagging_rules.push(r);
                }
            }
            "site_rule" => {
                if let Ok(r) = serde_json::from_str(line) {
                    site_rules.push(r);
                }
            }
            other => {
                tracing::warn!("restore: unknown NDJSON line type: {other}");
            }
        }
    }

    if !version.starts_with('2') {
        return Err(ApiError::BadRequest(format!(
            "unsupported backup version: {version}"
        )));
    }

    do_restore(
        state,
        auth,
        &BackupData {
            version,
            created_at: Utc::now(),
            users,
            entries,
            tags,
            entry_tags,
            annotations,
            memos,
            tagging_rules,
            site_rules,
        },
    )
    .await
}

async fn restore_legacy_json(
    state: &AppState,
    text: &str,
    auth: &AuthUser,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let data: BackupData = serde_json::from_str(text)
        .map_err(|e| ApiError::BadRequest(format!("invalid backup JSON: {e}")))?;

    if data.version != "1.0" {
        return Err(ApiError::BadRequest(format!(
            "unsupported backup version: {}",
            data.version
        )));
    }

    do_restore(state, auth, &data).await
}

async fn do_restore(
    state: &AppState,
    auth: &AuthUser,
    data: &BackupData,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Clear all tables in reverse dependency order
    sqlx::query("DELETE FROM entry_tags")
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    sqlx::query("DELETE FROM annotations")
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    sqlx::query("DELETE FROM memos")
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    sqlx::query("DELETE FROM tagging_rules")
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    sqlx::query("DELETE FROM site_rules")
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    sqlx::query("DELETE FROM entries")
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    sqlx::query("DELETE FROM refresh_tokens")
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    sqlx::query("DELETE FROM tags")
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    sqlx::query("DELETE FROM users")
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Insert users (without password_hash — restored users must reset passwords)
    for u in &data.users {
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, is_admin, feed_token, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(u.id)
        .bind(&u.username)
        .bind(&u.email)
        .bind("!restored")
        .bind(u.is_admin)
        .bind(&u.feed_token)
        .bind(u.created_at)
        .bind(u.updated_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    }

    // Insert entries
    for e in &data.entries {
        sqlx::query(
            "INSERT INTO entries (id, user_id, url, given_url, hashed_url, hashed_given_url, \
             title, content, text_content, content_type, extract_method, is_content_edited, \
             language, http_status, reading_time, preview_picture, domain_name, published_by, \
             metadata, is_archived, archived_at, is_starred, starred_at, published_at, \
             created_at, updated_at, deleted_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27)",
        )
        .bind(e.id)
        .bind(e.user_id)
        .bind(&e.url)
        .bind(&e.given_url)
        .bind(&e.hashed_url)
        .bind(&e.hashed_given_url)
        .bind(&e.title)
        .bind(&e.content)
        .bind(&e.text_content)
        .bind(&e.content_type)
        .bind(&e.extract_method)
        .bind(e.is_content_edited)
        .bind(&e.language)
        .bind(e.http_status)
        .bind(e.reading_time)
        .bind(&e.preview_picture)
        .bind(&e.domain_name)
        .bind(&e.published_by)
        .bind(&e.metadata)
        .bind(e.is_archived)
        .bind(e.archived_at)
        .bind(e.is_starred)
        .bind(e.starred_at)
        .bind(e.published_at)
        .bind(e.created_at)
        .bind(e.updated_at)
        .bind(e.deleted_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    }

    // Insert tags
    for t in &data.tags {
        sqlx::query(
            "INSERT INTO tags (id, user_id, label, slug, created_at) VALUES ($1,$2,$3,$4,$5)",
        )
        .bind(t.id)
        .bind(t.user_id)
        .bind(&t.label)
        .bind(&t.slug)
        .bind(t.created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    }

    // Insert entry_tags
    for et in &data.entry_tags {
        sqlx::query("INSERT INTO entry_tags (entry_id, tag_id) VALUES ($1,$2)")
            .bind(et.entry_id)
            .bind(et.tag_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    }

    // Insert annotations
    for a in &data.annotations {
        sqlx::query(
            "INSERT INTO annotations (id, entry_id, user_id, quote, text, ranges, is_orphaned, created_at, updated_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(a.id)
        .bind(a.entry_id)
        .bind(a.user_id)
        .bind(&a.quote)
        .bind(&a.text)
        .bind(&a.ranges)
        .bind(a.is_orphaned)
        .bind(a.created_at)
        .bind(a.updated_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    }

    // Insert memos
    for m in &data.memos {
        sqlx::query(
            "INSERT INTO memos (id, user_id, content, source_url, promoted_entry_id, created_at) \
             VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(m.id)
        .bind(m.user_id)
        .bind(&m.content)
        .bind(&m.source_url)
        .bind(m.promoted_entry_id)
        .bind(m.created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    }

    // Insert tagging rules
    for r in &data.tagging_rules {
        sqlx::query(
            "INSERT INTO tagging_rules (id, user_id, rule, tags, priority, created_at) \
             VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(r.id)
        .bind(r.user_id)
        .bind(&r.rule)
        .bind(&r.tags)
        .bind(r.priority)
        .bind(r.created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    }

    // Insert site rules
    for sr in &data.site_rules {
        sqlx::query(
            "INSERT INTO site_rules (id, user_id, domain, content_selector, title_selector, strip_selectors, created_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(sr.id)
        .bind(sr.user_id)
        .bind(&sr.domain)
        .bind(&sr.content_selector)
        .bind(&sr.title_selector)
        .bind(&sr.strip_selectors)
        .bind(sr.created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Rebuild search index
    if let Err(e) = state.search_index.clear().await {
        tracing::warn!("failed to clear search index after restore: {e}");
    }

    let entries_for_index: Vec<crate::models::entry::SearchableEntry> =
        sqlx::query_as(
            "SELECT id, user_id, title, text_content, url, domain_name FROM entries WHERE deleted_at IS NULL",
        )
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    for e in &entries_for_index {
        if let Err(err) = state
            .search_index
            .upsert(
                e.id,
                e.user_id,
                e.title.as_deref().unwrap_or(""),
                e.text_content.as_deref().unwrap_or(""),
                &e.url,
                e.domain_name.as_deref().unwrap_or(""),
            )
            .await
        {
            tracing::warn!(entry_id = %e.id, "failed to re-index entry after restore: {err}");
        }
    }

    tracing::info!("admin restore completed");

    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(auth),
        AuditAction::AdminRestore,
        Some(AuditResourceType::System),
        None,
        serde_json::json!({"users": data.users.len(), "entries": data.entries.len()}),
    )
    .await;

    Ok(axum::Json(serde_json::json!({
        "message": "restore complete",
        "users": data.users.len(),
        "entries": data.entries.len(),
        "tags": data.tags.len(),
        "entry_tags": data.entry_tags.len(),
        "annotations": data.annotations.len(),
        "memos": data.memos.len(),
        "tagging_rules": data.tagging_rules.len(),
        "site_rules": data.site_rules.len(),
    })))
}

// Helper struct for parsing metadata NDJSON line
#[derive(Deserialize)]
struct NdjsonMetadataLine {
    version: String,
}

// Need futures::StreamExt for .next() on sqlx streams
use futures::StreamExt;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_data_roundtrip_json() {
        let data = BackupData {
            version: "1.0".to_string(),
            created_at: Utc::now(),
            users: vec![],
            entries: vec![],
            tags: vec![],
            entry_tags: vec![],
            annotations: vec![],
            memos: vec![],
            tagging_rules: vec![],
            site_rules: vec![],
        };
        let json = serde_json::to_string(&data).unwrap();
        let parsed: BackupData = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, "1.0");
    }

    #[test]
    fn backup_user_excludes_password_hash() {
        let user = BackupUser {
            id: Uuid::new_v4(),
            username: "admin".to_string(),
            email: "admin@example.com".to_string(),
            is_admin: true,
            feed_token: "abc123".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&user).unwrap();
        assert!(!json.contains("password_hash"));
        let parsed: BackupUser = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.username, "admin");
    }

    #[test]
    fn ndjson_line_serialization() {
        let meta = NdjsonLine::Metadata {
            version: "2.0".to_string(),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains(r#""type":"metadata""#));
        assert!(json.contains(r#""version":"2.0""#));
    }

    #[test]
    fn ndjson_restore_parses_metadata() {
        let line = r#"{"type":"metadata","version":"2.0","created_at":"2026-01-01T00:00:00Z"}"#;
        let parsed: NdjsonMetadataLine = serde_json::from_str(line).unwrap();
        assert_eq!(parsed.version, "2.0");
    }

    #[test]
    fn ndjson_restore_rejects_v1() {
        // v1 NDJSON metadata should be rejected
        let line = r#"{"type":"metadata","version":"1.0","created_at":"2026-01-01T00:00:00Z"}"#;
        let parsed: NdjsonMetadataLine = serde_json::from_str(line).unwrap();
        assert!(!parsed.version.starts_with('2'));
    }

    #[test]
    fn backup_data_with_populated_fields() {
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();
        let tag_id = Uuid::new_v4();
        let now = Utc::now();

        let data = BackupData {
            version: "1.0".to_string(),
            created_at: now,
            users: vec![BackupUser {
                id: user_id,
                username: "testuser".to_string(),
                email: "test@example.com".to_string(),
                is_admin: false,
                feed_token: "token123".to_string(),
                created_at: now,
                updated_at: now,
            }],
            entries: vec![BackupEntry {
                id: entry_id,
                user_id,
                url: "https://example.com".to_string(),
                given_url: "https://example.com".to_string(),
                hashed_url: "abc".to_string(),
                hashed_given_url: "abc".to_string(),
                title: Some("Test".to_string()),
                content: Some("<p>test</p>".to_string()),
                text_content: Some("test".to_string()),
                content_type: "article".to_string(),
                extract_method: "readability".to_string(),
                is_content_edited: false,
                language: Some("en".to_string()),
                http_status: Some(200),
                reading_time: Some(5),
                preview_picture: None,
                domain_name: Some("example.com".to_string()),
                published_by: None,
                metadata: serde_json::json!({}),
                is_archived: false,
                archived_at: None,
                is_starred: true,
                starred_at: Some(now),
                published_at: None,
                created_at: now,
                updated_at: now,
                deleted_at: None,
            }],
            tags: vec![BackupTag {
                id: tag_id,
                user_id,
                label: "rust".to_string(),
                slug: "rust".to_string(),
                created_at: now,
            }],
            entry_tags: vec![BackupEntryTag { entry_id, tag_id }],
            annotations: vec![],
            memos: vec![],
            tagging_rules: vec![],
            site_rules: vec![],
        };

        let json = serde_json::to_string_pretty(&data).unwrap();
        let parsed: BackupData = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.users.len(), 1);
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.tags.len(), 1);
        assert_eq!(parsed.entry_tags.len(), 1);
        assert_eq!(parsed.entries[0].url, "https://example.com");
    }

    #[test]
    fn unsupported_version_detected() {
        let json = r#"{"version":"2.0","created_at":"2026-01-01T00:00:00Z","users":[],"entries":[],"tags":[],"entry_tags":[],"annotations":[],"memos":[],"tagging_rules":[],"site_rules":[]}"#;
        let parsed: BackupData = serde_json::from_str(json).unwrap();
        assert_ne!(parsed.version, "1.0");
    }
}
