use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::error::ModelError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Tag {
    pub id: Uuid,
    pub user_id: Uuid,
    pub label: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TagStats {
    pub id: Uuid,
    pub label: String,
    pub slug: String,
    pub entry_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TagLabel {
    pub id: Uuid,
    pub label: String,
}

pub fn slugify(label: &str) -> String {
    label
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub async fn list_tags(pool: &PgPool, user_id: Uuid) -> Result<Vec<Tag>, ModelError> {
    sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE user_id = $1 ORDER BY label")
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))
}

/// List tags with caching. Use this for read-heavy paths like the tag list API.
pub async fn list_tags_cached(pool: &PgPool, user_id: Uuid) -> Result<Vec<Tag>, ModelError> {
    // Try cache first
    if let Some(cached) = crate::cache::TAG_CACHE.get(user_id).await {
        return Ok(cached);
    }

    // Query database
    let tags = list_tags(pool, user_id).await?;

    // Update cache
    crate::cache::TAG_CACHE.insert(user_id, tags.clone()).await;

    Ok(tags)
}

pub async fn find_or_create_tag(pool: &PgPool, user_id: Uuid, label: &str) -> Result<Tag, ModelError> {
    let slug = slugify(label);
    if let Some(tag) = sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE user_id = $1 AND slug = $2")
        .bind(user_id).bind(&slug).fetch_optional(pool).await.map_err(|e| ModelError::Database(e.to_string()))? {
        return Ok(tag);
    }
    let tag = sqlx::query_as::<_, Tag>("INSERT INTO tags (user_id, label, slug) VALUES ($1, $2, $3) RETURNING *")
        .bind(user_id).bind(label).bind(&slug).fetch_one(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?;

    // Invalidate cache since we added a new tag
    crate::cache::TAG_CACHE.invalidate(user_id).await;
    crate::cache::TAG_STATS_CACHE.invalidate(user_id).await;

    Ok(tag)
}

pub async fn add_tag_to_entry(pool: &PgPool, user_id: Uuid, entry_id: Uuid, tag_id: Uuid) -> Result<(), ModelError> {
    sqlx::query("INSERT INTO entry_tags (entry_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(entry_id).bind(tag_id).execute(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    crate::cache::TAG_STATS_CACHE.invalidate(user_id).await;
    Ok(())
}

pub async fn remove_tag_from_entry(pool: &PgPool, user_id: Uuid, entry_id: Uuid, tag_id: Uuid) -> Result<bool, ModelError> {
    let result = sqlx::query("DELETE FROM entry_tags WHERE entry_id = $1 AND tag_id = $2")
        .bind(entry_id).bind(tag_id).execute(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?;
    if result.rows_affected() > 0 {
        crate::cache::TAG_STATS_CACHE.invalidate(user_id).await;
    }
    Ok(result.rows_affected() > 0)
}

pub async fn delete_tag(pool: &PgPool, user_id: Uuid, tag_id: Uuid) -> Result<bool, ModelError> {
    let result = sqlx::query("DELETE FROM tags WHERE id = $1 AND user_id = $2")
        .bind(tag_id).bind(user_id).execute(pool).await
        .map_err(|e| ModelError::Database(e.to_string()))?;

    if result.rows_affected() > 0 {
        // Invalidate cache since we deleted a tag
        crate::cache::TAG_CACHE.invalidate(user_id).await;
        crate::cache::TAG_STATS_CACHE.invalidate(user_id).await;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[derive(Debug)]
pub enum RenameError {
    Conflict,
    Database(String),
}

pub async fn rename_tag(
    pool: &PgPool,
    tag_id: Uuid,
    user_id: Uuid,
    new_label: &str,
) -> Result<Tag, RenameError> {
    let new_slug = slugify(new_label);

    // Check slug conflict excluding self
    let conflict: bool = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM tags WHERE user_id = $1 AND slug = $2 AND id != $3",
    )
    .bind(user_id)
    .bind(&new_slug)
    .bind(tag_id)
    .fetch_one(pool)
    .await
    .map_err(|e| RenameError::Database(e.to_string()))?
        > 0;

    if conflict {
        return Err(RenameError::Conflict);
    }

    let tag = sqlx::query_as::<_, Tag>(
        "UPDATE tags SET label = $1, slug = $2 WHERE id = $3 AND user_id = $4 RETURNING *",
    )
    .bind(new_label)
    .bind(&new_slug)
    .bind(tag_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| RenameError::Database(e.to_string()))?
    .ok_or_else(|| RenameError::Database("tag not found".to_string()))?;

    // Invalidate caches
    crate::cache::TAG_CACHE.invalidate(user_id).await;
    crate::cache::TAG_STATS_CACHE.invalidate(user_id).await;

    Ok(tag)
}

pub async fn list_tags_for_entry(pool: &PgPool, entry_id: Uuid) -> Result<Vec<Tag>, ModelError> {
    sqlx::query_as::<_, Tag>(
        "SELECT t.* FROM tags t JOIN entry_tags et ON t.id = et.tag_id WHERE et.entry_id = $1 ORDER BY t.label"
    )
    .bind(entry_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))
}

/// Ensure each label exists as a tag for `user_id`, then link every (entry, tag)
/// pair in a single transaction. Idempotent: re-linking an existing pair is a
/// no-op (ON CONFLICT DO NOTHING).
///
/// Returns once all rows are committed. Empty `entry_ids` or empty `labels`
/// is a no-op.
pub async fn ensure_and_link(
    pool: &PgPool,
    user_id: Uuid,
    entry_ids: &[Uuid],
    labels: &[String],
) -> Result<(), ModelError> {
    if entry_ids.is_empty() || labels.is_empty() {
        return Ok(());
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ModelError::Database(e.to_string()))?;

    // Dedup labels by slug, then ensure each tag exists. ON CONFLICT DO NOTHING
    // means a RETURNING clause would only emit newly-inserted rows, so we use
    // a separate SELECT to fetch the full id set (new + pre-existing).
    let mut seen = std::collections::HashSet::new();
    let mut unique_labels: Vec<&str> = Vec::new();
    let mut unique_slugs: Vec<String> = Vec::new();
    for label in labels {
        let slug = slugify(label);
        if seen.insert(slug.clone()) {
            unique_labels.push(label.as_str());
            unique_slugs.push(slug);
        }
    }

    sqlx::query(
        "INSERT INTO tags (user_id, label, slug) \
         SELECT $1, l, s FROM UNNEST($2::text[], $3::text[]) AS t(l, s) \
         ON CONFLICT (user_id, slug) DO NOTHING",
    )
    .bind(user_id)
    .bind(&unique_labels)
    .bind(&unique_slugs)
    .execute(&mut *tx)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;

    let tag_id_vec: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM tags WHERE user_id = $1 AND slug = ANY($2)",
    )
    .bind(user_id)
    .bind(&unique_slugs)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;

    // Cross-product link, single statement. CROSS JOIN UNNEST builds the
    // cartesian product; ON CONFLICT skips already-linked pairs.
    sqlx::query(
        "INSERT INTO entry_tags (entry_id, tag_id) \
         SELECT e, t \
         FROM UNNEST($1::uuid[]) AS e \
         CROSS JOIN UNNEST($2::uuid[]) AS t \
         ON CONFLICT DO NOTHING",
    )
    .bind(entry_ids)
    .bind(&tag_id_vec)
    .execute(&mut *tx)
    .await
    .map_err(|e| ModelError::Database(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ModelError::Database(e.to_string()))?;

    // Invalidate cache since we may have created new tags
    crate::cache::TAG_CACHE.invalidate(user_id).await;
    crate::cache::TAG_STATS_CACHE.invalidate(user_id).await;

    Ok(())
}

impl TagStats {
    /// List tags with entry counts for a user, only counting non-deleted entries.
    pub async fn list(pool: &PgPool, user_id: Uuid) -> Result<Vec<TagStats>, ModelError> {
        sqlx::query_as::<_, TagStats>(
            r#"
            SELECT t.id, t.label, t.slug, t.created_at,
                   COUNT(et.entry_id)::int AS entry_count
            FROM tags t
            LEFT JOIN entry_tags et ON et.tag_id = t.id
            LEFT JOIN entries e ON e.id = et.entry_id AND e.deleted_at IS NULL
            WHERE t.user_id = $1
            GROUP BY t.id, t.label, t.slug, t.created_at
            ORDER BY t.label
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))
    }

    /// Cache-first wrapper for list.
    pub async fn list_cached(pool: &PgPool, user_id: Uuid) -> Result<Vec<TagStats>, ModelError> {
        if let Some(cached) = crate::cache::TAG_STATS_CACHE.get(user_id).await {
            return Ok(cached);
        }

        let stats = Self::list(pool, user_id).await?;
        crate::cache::TAG_STATS_CACHE.insert(user_id, stats.clone()).await;
        Ok(stats)
    }
}
