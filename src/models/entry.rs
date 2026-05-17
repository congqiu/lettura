use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

use super::error::ModelError;

/// Opaque pagination cursor: encodes the (created_at, id) tuple of the last
/// entry on the current page. Format: "<unix_micros>:<uuid>". Plain text so it's
/// URL-safe and debuggable; not a security token.
pub mod cursor {
    use chrono::{DateTime, TimeZone, Utc};
    use uuid::Uuid;

    pub fn encode(ts: DateTime<Utc>, id: Uuid) -> String {
        format!("{}:{}", ts.timestamp_micros(), id)
    }

    pub fn decode(s: &str) -> Option<(DateTime<Utc>, Uuid)> {
        let (ts_str, id_str) = s.split_once(':')?;
        let micros: i64 = ts_str.parse().ok()?;
        let ts = Utc.timestamp_micros(micros).single()?;
        let id: Uuid = id_str.parse().ok()?;
        Some((ts, id))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn round_trip() {
            let ts = Utc.timestamp_micros(1_714_086_456_123_456).unwrap();
            let id = Uuid::new_v4();
            let s = encode(ts, id);
            let (ts2, id2) = decode(&s).expect("round-trip");
            assert_eq!(ts.timestamp_micros(), ts2.timestamp_micros());
            assert_eq!(id, id2);
        }

        #[test]
        fn decode_rejects_garbage() {
            assert!(decode("not-a-cursor").is_none());
            assert!(decode("123:not-a-uuid").is_none());
            assert!(decode(":550e8400-e29b-41d4-a716-446655440000").is_none());
            assert!(decode("").is_none());
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Entry {
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

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct EntryTagLink {
    pub entry_id: Uuid,
    pub tag_id: Uuid,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct EntrySummary {
    pub id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub title: Option<String>,
    pub content_type: String,
    pub extract_method: String,
    pub language: Option<String>,
    pub reading_time: Option<i32>,
    pub preview_picture: Option<String>,
    pub domain_name: Option<String>,
    pub published_by: Option<String>,
    pub is_archived: bool,
    pub is_starred: bool,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    #[sqlx(skip)]
    pub tags: Vec<crate::models::tag::TagLabel>,
}

pub fn hash_url(url: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(url.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn extract_domain(url_str: &str) -> Option<String> {
    Url::parse(url_str)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
}

pub struct CreateEntryResult {
    pub entry: Entry,
    pub already_existed: bool,
}

/// Idempotent entry creation: returns existing entry on URL conflict.
/// Uses ON CONFLICT (user_id, hashed_url) DO NOTHING + fallback SELECT.
pub async fn create_or_get_entry(
    pool: &PgPool,
    user_id: Uuid,
    given_url: &str,
) -> Result<CreateEntryResult, ModelError> {
    let hashed_url = hash_url(given_url);
    let hashed_given_url = hash_url(given_url);
    let domain_name = extract_domain(given_url);

    // Try insert; ON CONFLICT return None
    let inserted: Option<Entry> = sqlx::query_as(
        "INSERT INTO entries (user_id, url, given_url, hashed_url, hashed_given_url, domain_name) \
         VALUES ($1,$2,$3,$4,$5,$6) \
         ON CONFLICT (user_id, hashed_url) DO NOTHING \
         RETURNING *",
    )
    .bind(user_id)
    .bind(given_url)
    .bind(given_url)
    .bind(&hashed_url)
    .bind(&hashed_given_url)
    .bind(&domain_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;

    if let Some(entry) = inserted {
        return Ok(CreateEntryResult {
            entry,
            already_existed: false,
        });
    }

    // Fallback: look up the existing entry
    let existing: Entry = sqlx::query_as(
        "SELECT * FROM entries WHERE user_id = $1 AND hashed_url = $2 AND deleted_at IS NULL",
    )
    .bind(user_id)
    .bind(&hashed_url)
    .fetch_one(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;

    Ok(CreateEntryResult {
        entry: existing,
        already_existed: true,
    })
}

pub async fn create_entry(
    pool: &PgPool,
    user_id: Uuid,
    given_url: &str,
) -> Result<Entry, ModelError> {
    create_or_get_entry(pool, user_id, given_url)
        .await
        .map(|r| r.entry)
}

pub async fn find_entry_by_id(
    pool: &PgPool,
    user_id: Uuid,
    entry_id: Uuid,
) -> Result<Option<Entry>, ModelError> {
    sqlx::query_as::<_, Entry>(
        "SELECT * FROM entries WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(entry_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    #[serde(
        default,
        deserialize_with = "crate::models::serde_helpers::deserialize_i64_from_string"
    )]
    pub page: Option<i64>,
    #[serde(
        default,
        deserialize_with = "crate::models::serde_helpers::deserialize_i64_from_string"
    )]
    pub per_page: Option<i64>,
    #[serde(
        default,
        deserialize_with = "crate::models::serde_helpers::deserialize_bool_from_string"
    )]
    pub is_archived: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::models::serde_helpers::deserialize_bool_from_string"
    )]
    pub is_starred: Option<bool>,
    /// Alias for `is_archived` (CLI Filter DSL compatibility).
    /// `is_read=true` → `is_archived = true`, `is_read=false` → `is_archived = false`.
    #[serde(
        default,
        deserialize_with = "crate::models::serde_helpers::deserialize_bool_from_string"
    )]
    pub is_read: Option<bool>,
    pub domain: Option<String>,
    /// Comma-separated tag labels; AND semantics (entry must have ALL listed tags).
    pub tag: Option<String>,
    /// Comma-separated tag labels to exclude; entry must NOT have any of these tags.
    pub exclude_tag: Option<String>,
    /// When true, return only entries with no tags.
    #[serde(
        default,
        deserialize_with = "crate::models::serde_helpers::deserialize_bool_from_string"
    )]
    pub untagged: Option<bool>,
    /// Return entries created at or after this timestamp.
    pub since: Option<DateTime<Utc>>,
    /// Return entries created strictly before this timestamp.
    pub before: Option<DateTime<Utc>>,
    /// Search query. Handled by tantivy in the API handler; unused in list_entries itself.
    pub search: Option<String>,
    /// Field-projection hint — placeholder for Task 15, not used in query construction.
    pub fields: Option<String>,
    /// Opaque cursor for keyset pagination. When present, page/OFFSET is ignored
    /// and the result is keyed on `(created_at, id) < cursor` ordering.
    pub cursor: Option<String>,
}

pub async fn list_entries(
    pool: &PgPool,
    user_id: Uuid,
    params: &ListParams,
) -> Result<Vec<EntrySummary>, ModelError> {
    let per_page = params.per_page.unwrap_or(20).min(100);

    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT id, user_id, url, title, content_type, extract_method, language, reading_time, \
         preview_picture, domain_name, published_by, is_archived, is_starred, created_at, deleted_at \
         FROM entries WHERE user_id = ",
    );
    build_where_clause(&mut qb, user_id, params);

    // Keyset pagination wins over page+OFFSET when cursor is provided.
    if let Some(cursor_str) = params.cursor.as_deref() {
        if let Some((cur_ts, cur_id)) = cursor::decode(cursor_str) {
            qb.push(" AND (created_at, id) < (");
            qb.push_bind(cur_ts);
            qb.push(", ");
            qb.push_bind(cur_id);
            qb.push(")");
        } else {
            return Err(ModelError::Database("invalid cursor".to_string()));
        }
    }

    qb.push(" ORDER BY created_at DESC, id DESC LIMIT ");
    qb.push_bind(per_page);

    if params.cursor.is_none() {
        let offset = (params.page.unwrap_or(1) - 1).max(0) * per_page;
        qb.push(" OFFSET ");
        qb.push_bind(offset);
    }

    let mut entries = qb
        .build_query_as::<EntrySummary>()
        .fetch_all(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))?;

    attach_tags(pool, &mut entries).await?;
    Ok(entries)
}

/// Compute the next cursor from a freshly-fetched result page. Returns Some
/// when the page is "full" (likely more rows exist) and the last item has a
/// `created_at`. The caller should emit this as `X-Next-Cursor`.
pub fn next_cursor_from(items: &[EntrySummary], per_page: i64) -> Option<String> {
    if (items.len() as i64) < per_page {
        return None;
    }
    let last = items.last()?;
    Some(cursor::encode(last.created_at, last.id))
}

/// Build the shared WHERE clause for entry list queries.
fn build_where_clause(
    qb: &mut sqlx::QueryBuilder<'static, sqlx::Postgres>,
    user_id: Uuid,
    params: &ListParams,
) {
    qb.push_bind(user_id);
    qb.push(" AND deleted_at IS NULL");

    if let Some(b) = params.is_archived {
        qb.push(" AND is_archived = ");
        qb.push_bind(b);
    }
    if let Some(b) = params.is_read {
        qb.push(" AND is_archived = ");
        qb.push_bind(b);
    }
    if let Some(b) = params.is_starred {
        qb.push(" AND is_starred = ");
        qb.push_bind(b);
    }
    if let Some(d) = &params.domain {
        qb.push(" AND domain_name = ");
        qb.push_bind(d.clone());
    }
    if let Some(t) = params.since {
        qb.push(" AND created_at >= ");
        qb.push_bind(t);
    }
    if let Some(t) = params.before {
        qb.push(" AND created_at < ");
        qb.push_bind(t);
    }
    if let Some(true) = params.untagged {
        qb.push(" AND NOT EXISTS (SELECT 1 FROM entry_tags et WHERE et.entry_id = entries.id)");
    }
    if let Some(tags_csv) = &params.tag {
        for t in tags_csv.split(',').filter(|s| !s.trim().is_empty()) {
            qb.push(
                " AND EXISTS (SELECT 1 FROM entry_tags et JOIN tags tg ON tg.id = et.tag_id \
                 WHERE et.entry_id = entries.id AND tg.user_id = ",
            );
            qb.push_bind(user_id);
            qb.push(" AND tg.label = ");
            qb.push_bind(t.trim().to_string());
            qb.push(")");
        }
    }
    if let Some(tags_csv) = &params.exclude_tag {
        for t in tags_csv.split(',').filter(|s| !s.trim().is_empty()) {
            qb.push(
                " AND NOT EXISTS (SELECT 1 FROM entry_tags et JOIN tags tg ON tg.id = et.tag_id \
                 WHERE et.entry_id = entries.id AND tg.user_id = ",
            );
            qb.push_bind(user_id);
            qb.push(" AND tg.label = ");
            qb.push_bind(t.trim().to_string());
            qb.push(")");
        }
    }
    if let Some(s) = &params.search
        && !s.is_empty()
    {
        qb.push(" AND (title ILIKE ");
        qb.push_bind(format!("%{s}%"));
        qb.push(" OR content ILIKE ");
        qb.push_bind(format!("%{s}%"));
        qb.push(")");
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateEntryParams {
    pub title: Option<String>,
    pub content: Option<String>,
    pub is_archived: Option<bool>,
    pub is_starred: Option<bool>,
}

pub async fn update_entry(
    pool: &PgPool,
    user_id: Uuid,
    entry_id: Uuid,
    params: &UpdateEntryParams,
) -> Result<Entry, ModelError> {
    let existing = find_entry_by_id(pool, user_id, entry_id)
        .await?
        .ok_or_else(|| ModelError::NotFound("entry not found".to_string()))?;

    let title = params
        .title
        .as_deref()
        .unwrap_or(existing.title.as_deref().unwrap_or(""));
    let content = params.content.as_deref().or(existing.content.as_deref());
    let is_content_edited = if params.content.is_some() {
        true
    } else {
        existing.is_content_edited
    };
    let is_archived = params.is_archived.unwrap_or(existing.is_archived);
    let archived_at = if params.is_archived == Some(true) && !existing.is_archived {
        Some(Utc::now())
    } else if params.is_archived == Some(false) {
        None
    } else {
        existing.archived_at
    };
    let is_starred = params.is_starred.unwrap_or(existing.is_starred);
    let starred_at = if params.is_starred == Some(true) && !existing.is_starred {
        Some(Utc::now())
    } else if params.is_starred == Some(false) {
        None
    } else {
        existing.starred_at
    };

    sqlx::query_as::<_, Entry>(
        "UPDATE entries SET title=$3, content=$4, is_content_edited=$5, is_archived=$6, archived_at=$7, is_starred=$8, starred_at=$9, updated_at=now() WHERE id=$1 AND user_id=$2 RETURNING *"
    )
    .bind(entry_id).bind(user_id).bind(title).bind(content).bind(is_content_edited)
    .bind(is_archived).bind(archived_at).bind(is_starred).bind(starred_at)
    .fetch_one(pool).await.map_err(|e| ModelError::Database(e.to_string()))
}

/// Restore archived/starred flags + their original timestamps verbatim.
/// Used by import paths where preserving the original moment matters; the
/// regular `update_entry` would stamp `now()` and lose backup fidelity.
pub async fn restore_import_status(
    pool: &PgPool,
    user_id: Uuid,
    entry_id: Uuid,
    is_archived: bool,
    archived_at: Option<DateTime<Utc>>,
    is_starred: bool,
    starred_at: Option<DateTime<Utc>>,
) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE entries SET is_archived = $3, archived_at = $4, is_starred = $5, starred_at = $6, updated_at = now() \
         WHERE id = $1 AND user_id = $2",
    )
    .bind(entry_id)
    .bind(user_id)
    .bind(is_archived)
    .bind(archived_at)
    .bind(is_starred)
    .bind(starred_at)
    .execute(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(())
}

pub async fn list_entries_by_ids(
    pool: &PgPool,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<Vec<EntrySummary>, ModelError> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let mut entries = sqlx::query_as::<_, EntrySummary>(
        "SELECT id, user_id, url, title, content_type, extract_method, language, reading_time, preview_picture, domain_name, published_by, is_archived, is_starred, created_at, deleted_at FROM entries WHERE user_id = $1 AND id = ANY($2) AND deleted_at IS NULL ORDER BY created_at DESC"
    )
    .bind(user_id)
    .bind(ids)
    .fetch_all(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;
    attach_tags(pool, &mut entries).await?;
    Ok(entries)
}

pub async fn delete_entry(
    pool: &PgPool,
    user_id: Uuid,
    entry_id: Uuid,
) -> Result<bool, ModelError> {
    let result = sqlx::query("UPDATE entries SET deleted_at = now() WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL")
        .bind(entry_id).bind(user_id).execute(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_deleted_entries(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<EntrySummary>, ModelError> {
    let mut entries = sqlx::query_as::<_, EntrySummary>(
        "SELECT id, user_id, url, title, content_type, extract_method, language, reading_time, preview_picture, domain_name, published_by, is_archived, is_starred, created_at, deleted_at FROM entries WHERE user_id = $1 AND deleted_at IS NOT NULL ORDER BY deleted_at DESC"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;
    attach_tags(pool, &mut entries).await?;
    Ok(entries)
}

pub async fn restore_entry(pool: &PgPool, entry_id: Uuid, user_id: Uuid) -> Result<(), ModelError> {
    let result = sqlx::query("UPDATE entries SET deleted_at = NULL WHERE id = $1 AND user_id = $2 AND deleted_at IS NOT NULL")
        .bind(entry_id).bind(user_id).execute(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    if result.rows_affected() == 0 {
        return Err(ModelError::NotFound(
            "entry not found or not deleted".to_string(),
        ));
    }
    Ok(())
}

pub async fn permanently_delete_entry(
    pool: &PgPool,
    entry_id: Uuid,
    user_id: Uuid,
) -> Result<(), ModelError> {
    let result = sqlx::query(
        "DELETE FROM entries WHERE id = $1 AND user_id = $2 AND deleted_at IS NOT NULL",
    )
    .bind(entry_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;
    if result.rows_affected() == 0 {
        return Err(ModelError::NotFound("entry not found".to_string()));
    }
    Ok(())
}

/// Find all entry IDs matching the given filter params (no pagination).
pub async fn find_ids_matching(
    pool: &PgPool,
    user_id: Uuid,
    params: &ListParams,
) -> Result<Vec<Uuid>, ModelError> {
    let mut qb =
        sqlx::QueryBuilder::<sqlx::Postgres>::new("SELECT id FROM entries WHERE user_id = ");
    build_where_clause(&mut qb, user_id, params);

    let rows: Vec<(Uuid,)> = qb
        .build_query_as()
        .fetch_all(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

#[derive(Default)]
pub struct ExtractedContent {
    pub title: Option<String>,
    pub content: Option<String>,
    pub text_content: Option<String>,
    pub language: Option<String>,
    pub preview_picture: Option<String>,
    pub published_by: Option<String>,
    pub reading_time: Option<i32>,
    pub http_status: i16,
    pub extract_method: String,
}

pub async fn update_entry_content(
    pool: &PgPool,
    entry_id: Uuid,
    ec: &ExtractedContent,
) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE entries SET title=COALESCE($2,title), content=$3, text_content=$4, language=$5, preview_picture=$6, published_by=$7, reading_time=$8, http_status=$9, extract_method=$10, updated_at=now() WHERE id=$1 AND is_content_edited=false"
    )
    .bind(entry_id)
    .bind(&ec.title)
    .bind(&ec.content)
    .bind(&ec.text_content)
    .bind(&ec.language)
    .bind(&ec.preview_picture)
    .bind(&ec.published_by)
    .bind(ec.reading_time)
    .bind(ec.http_status)
    .bind(&ec.extract_method)
    .execute(pool).await.map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(())
}

/// Update only the content field (used by async image processor).
/// Lightweight row type for search index rebuild queries.
#[derive(sqlx::FromRow)]
pub struct SearchableEntry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: Option<String>,
    pub text_content: Option<String>,
    pub url: String,
    pub domain_name: Option<String>,
}

pub async fn update_content_only(
    pool: &PgPool,
    entry_id: Uuid,
    content: &str,
) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE entries SET content = $2, updated_at = NOW() WHERE id = $1 AND is_content_edited = false"
    )
    .bind(entry_id)
    .bind(content)
    .execute(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;
    Ok(())
}

/// Attach tag labels to a list of entry summaries in-place.
/// Issues a single batch query for all entry IDs.
async fn attach_tags(pool: &PgPool, entries: &mut [EntrySummary]) -> Result<(), ModelError> {
    if entries.is_empty() {
        return Ok(());
    }

    let entry_ids: Vec<Uuid> = entries.iter().map(|e| e.id).collect();

    let rows: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT et.entry_id, t.id AS tag_id, t.label \
         FROM entry_tags et \
         JOIN tags t ON t.id = et.tag_id \
         WHERE et.entry_id = ANY($1) \
         ORDER BY t.label",
    )
    .bind(&entry_ids)
    .fetch_all(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;

    let mut tag_map: std::collections::HashMap<Uuid, Vec<crate::models::tag::TagLabel>> =
        std::collections::HashMap::new();
    for (entry_id, tag_id, label) in rows {
        tag_map
            .entry(entry_id)
            .or_default()
            .push(crate::models::tag::TagLabel { id: tag_id, label });
    }

    for entry in entries.iter_mut() {
        entry.tags = tag_map.remove(&entry.id).unwrap_or_default();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// Helper to build an EntrySummary with minimal, deterministic fields.
    fn mock_summary(id: Uuid, created_at: DateTime<Utc>) -> EntrySummary {
        EntrySummary {
            id,
            user_id: Uuid::nil(),
            url: "https://example.com".to_string(),
            title: None,
            content_type: "text/html".to_string(),
            extract_method: "readability".to_string(),
            language: None,
            reading_time: None,
            preview_picture: None,
            domain_name: Some("example.com".to_string()),
            published_by: None,
            is_archived: false,
            is_starred: false,
            created_at,
            deleted_at: None,
            tags: vec![],
        }
    }

    // --- hash_url ---

    #[test]
    fn hash_url_deterministic() {
        let h1 = hash_url("https://example.com/article");
        let h2 = hash_url("https://example.com/article");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_url_different_urls() {
        let h1 = hash_url("https://example.com/a");
        let h2 = hash_url("https://example.com/b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_url_empty() {
        let h = hash_url("");
        assert!(
            !h.is_empty(),
            "empty input should still produce a non-empty hash"
        );
    }

    // --- extract_domain ---

    #[test]
    fn extract_domain_common() {
        assert_eq!(
            extract_domain("https://example.com/path"),
            Some("example.com".to_string())
        );
    }

    #[test]
    fn extract_domain_with_port() {
        // Url::host_str() does NOT include the port number
        assert_eq!(
            extract_domain("http://localhost:3000/api"),
            Some("localhost".to_string())
        );
    }

    #[test]
    fn extract_domain_subdomain() {
        assert_eq!(
            extract_domain("https://blog.example.com/post"),
            Some("blog.example.com".to_string())
        );
    }

    #[test]
    fn extract_domain_invalid() {
        assert_eq!(extract_domain("not-a-url"), None);
    }

    #[test]
    fn extract_domain_ip() {
        assert_eq!(
            extract_domain("http://192.168.1.1/path"),
            Some("192.168.1.1".to_string())
        );
    }

    // --- next_cursor_from ---

    #[test]
    fn next_cursor_full_page() {
        let per_page: i64 = 3;
        let items: Vec<EntrySummary> = (0..3)
            .map(|i| {
                let ts = Utc
                    .timestamp_micros(1_700_000_000_000_000 + i as i64)
                    .unwrap();
                mock_summary(Uuid::new_v4(), ts)
            })
            .collect();

        let cursor = next_cursor_from(&items, per_page);
        assert!(cursor.is_some(), "full page should produce a cursor");

        // Verify cursor is decodable and matches last item
        let last = items.last().unwrap();
        let (ts, id) = cursor::decode(&cursor.unwrap()).expect("cursor should decode");
        assert_eq!(ts, last.created_at);
        assert_eq!(id, last.id);
    }

    #[test]
    fn next_cursor_partial_page() {
        let per_page: i64 = 10;
        let items: Vec<EntrySummary> = (0..3)
            .map(|i| {
                let ts = Utc
                    .timestamp_micros(1_700_000_000_000_000 + i as i64)
                    .unwrap();
                mock_summary(Uuid::new_v4(), ts)
            })
            .collect();

        assert!(
            next_cursor_from(&items, per_page).is_none(),
            "partial page should return None"
        );
    }

    #[test]
    fn next_cursor_empty() {
        let items: Vec<EntrySummary> = vec![];
        assert!(
            next_cursor_from(&items, 10).is_none(),
            "empty list should return None"
        );
    }
}
