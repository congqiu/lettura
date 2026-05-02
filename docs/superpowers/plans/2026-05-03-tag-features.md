# 标签功能完整改进 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让标签从"形同虚设"变为可导航、可筛选、可管理的核心功能

**Architecture:** 后端新增 tags/stats API 和 bulk-by-ids API，修改 list_entries 返回标签；前端侧边栏添加标签导航、文章卡片显示标签、标签筛选 UI、EntryTags 组件修复+自动补全、设置页标签管理、独立标签页、批量操作、标签规则管理

**Tech Stack:** Rust/Axum/SQLx (后端), React 19/TypeScript/TanStack Query/shadcn (前端)

---

## 文件结构

### 后端新增/修改

| 文件 | 操作 | 职责 |
|------|------|------|
| `src/models/tag.rs` | 修改 | 新增 `TagStats`、`TagLabel` 结构体，`list_stats()`、`list_stats_cached()`、`rename()` 函数 |
| `src/api/tags.rs` | 修改 | 新增 `tags_stats`、`rename_tag` 路由 |
| `src/models/entry.rs` | 修改 | `EntrySummary` 新增 `tags` 字段（`#[sqlx(skip)]`），`list_entries` 批量查询标签 |
| `src/api/entries.rs` | 修改 | `list_entries` 响应包含标签 |
| `src/api/bulk.rs` | 修改 | 新增 `bulk_tag_by_ids`、`bulk_untag_by_ids`、`bulk_delete_by_ids` |
| `src/api/import.rs` | 修改 | Wallabag 导入时关联标签 |
| `src/cache/mod.rs` | 修改 | 新增 `TAG_STATS_CACHE` |
| `src/models/audit_log.rs` | 修改 | 新增 `RenameTag`、`BulkSoftDelete` 审计操作 |
| `src/api/mod.rs` | 修改 | 注册新路由 |

### 前端新增/修改

| 文件 | 操作 | 职责 |
|------|------|------|
| `web/src/api/tags.ts` | 修改 | 新增 `fetchTagStats`、`renameTag`、`bulkTagByIds` 等 |
| `web/src/api/taggingRules.ts` | 新建 | 标签规则 CRUD API |
| `web/src/api/entries.ts` | 修改 | 修复 `ListParams`（tags→tag），类型添加 tags 字段 |
| `web/src/components/TagBadge.tsx` | 新建 | 标签 badge 组件（可点击跳转筛选） |
| `web/src/components/EntryTags.tsx` | 修改 | 修复 bug + 自动补全 |
| `web/src/components/layout/AppSidebar.tsx` | 修改 | 添加标签分组 |
| `web/src/components/EntryCard.tsx` | 修改 | 卡片底部显示标签 |
| `web/src/pages/EntryListPage.tsx` | 修改 | 标签筛选 UI + 多选模式 |
| `web/src/pages/SettingsPage.tsx` | 修改 | 标签管理 + 标签规则管理 |
| `web/src/pages/TagsPage.tsx` | 新建 | 独立标签页 |
| `web/src/App.tsx` | 修改 | 添加 `/tags` 路由 |

---

## Task 1: 后端 — TagStats 结构体和 list_stats 函数

**Files:**
- Modify: `src/models/tag.rs`
- Modify: `src/cache/mod.rs`

- [ ] **Step 1: 添加 TagStats 结构体**

在 `src/models/tag.rs` 中添加（注意 derive `Clone` 以满足 `UserCache<T>` 约束）：

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct TagStats {
    pub id: Uuid,
    pub label: String,
    pub slug: String,
    pub entry_count: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

- [ ] **Step 2: 实现 list_stats 函数**

使用 `sqlx::query_as::<_, TagStats>` 模式（与代码库其他查询一致，不用 `query_as!` 宏）：

```rust
impl TagStats {
    pub async fn list(pool: &PgPool, user_id: Uuid) -> Result<Vec<Self>, ModelError> {
        sqlx::query_as::<_, TagStats>(
            "SELECT t.id, t.label, t.slug, t.created_at,
                    COUNT(et.entry_id)::int AS entry_count
             FROM tags t
             LEFT JOIN entry_tags et ON et.tag_id = t.id
             LEFT JOIN entries e ON e.id = et.entry_id AND e.deleted_at IS NULL
             WHERE t.user_id = $1
             GROUP BY t.id
             ORDER BY t.label"
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))
    }

    pub async fn list_cached(pool: &PgPool, user_id: Uuid) -> Result<Vec<Self>, ModelError> {
        if let Some(cached) = crate::cache::TAG_STATS_CACHE.get(user_id).await {
            return Ok(cached);
        }
        let stats = Self::list(pool, user_id).await?;
        crate::cache::TAG_STATS_CACHE.insert(user_id, stats.clone()).await;
        Ok(stats)
    }
}
```

- [ ] **Step 3: 添加 TAG_STATS_CACHE**

在 `src/cache/mod.rs` 中添加：

```rust
use crate::models::tag::TagStats;

/// Cache for tag stats (5 minute TTL, 1000 users max).
pub static TAG_STATS_CACHE: once_cell::sync::Lazy<Arc<UserCache<TagStats>>> =
    once_cell::sync::Lazy::new(|| {
        Arc::new(UserCache::new(1000, Duration::from_secs(300)))
    });
```

- [ ] **Step 4: 添加 TagLabel 结构体**

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct TagLabel {
    pub id: Uuid,
    pub label: String,
}
```

- [ ] **Step 5: 编译验证**

Run: `docker build --target test --build-arg "TEST_ARGS=--lib tag" -t lettura-test .`
Expected: PASS

- [ ] **Step 6: 提交**

```bash
git add src/models/tag.rs src/cache/mod.rs
git commit -m "feat(tag): add TagStats, TagLabel structs and list_stats query with cache"
```

---

## Task 2: 后端 — rename 函数

**Files:**
- Modify: `src/models/tag.rs`

- [ ] **Step 1: 实现 rename 函数**

使用代码库的自由函数模式（非 impl 块），使用 `sqlx::query_as::<_, Tag>` 模式，使用 `slugify` 而非 `slug::slugify`：

```rust
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

    // Check slug conflict (excluding self)
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tags WHERE user_id = $1 AND slug = $2 AND id != $3"
    )
    .bind(user_id)
    .bind(&new_slug)
    .bind(tag_id)
    .fetch_one(pool)
    .await
    .map_err(|e| RenameError::Database(e.to_string()))?;

    if count.0 > 0 {
        return Err(RenameError::Conflict);
    }

    let tag = sqlx::query_as::<_, Tag>(
        "UPDATE tags SET label = $1, slug = $2 WHERE id = $3 AND user_id = $4
         RETURNING id, user_id, label, slug, created_at"
    )
    .bind(new_label)
    .bind(&new_slug)
    .bind(tag_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| RenameError::Database(e.to_string()))?;

    // Invalidate caches
    crate::cache::TAG_CACHE.invalidate(user_id).await;
    crate::cache::TAG_STATS_CACHE.invalidate(user_id).await;

    Ok(tag)
}
```

- [ ] **Step 2: 编译验证**

Run: `docker build --target test --build-arg "TEST_ARGS=--lib tag" -t lettura-test .`
Expected: PASS

- [ ] **Step 3: 提交**

```bash
git add src/models/tag.rs
git commit -m "feat(tag): add rename_tag function with slug conflict check"
```

---

## Task 3: 后端 — 审计日志新增操作

**Files:**
- Modify: `src/models/audit_log.rs`

- [ ] **Step 1: 添加 RenameTag 和 BulkSoftDelete 审计操作**

在 `AuditAction` 枚举中添加两个变体（在 `DeleteTag` 之后和 `AddTagToEntry` 之前添加 `RenameTag`，在 `BulkStar` 之后添加 `BulkSoftDelete`）：

```rust
pub enum AuditAction {
    // ...existing...
    DeleteTag,
    RenameTag,        // 新增
    AddTagToEntry,
    // ...existing...
    BulkStar,
    BulkSoftDelete,   // 新增
    UploadPageFiles,
}
```

- [ ] **Step 2: 添加数据库迁移**

创建 `migrations/012_add_audit_actions_rename_delete.sql`：

```sql
ALTER TYPE audit_action ADD VALUE IF NOT EXISTS 'rename_tag';
ALTER TYPE audit_action ADD VALUE IF NOT EXISTS 'bulk_soft_delete';
```

- [ ] **Step 3: 编译验证**

Run: `docker build --target test --build-arg "TEST_ARGS=--lib audit_log" -t lettura-test .`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add src/models/audit_log.rs migrations/012_add_audit_actions_rename_delete.sql
git commit -m "feat(audit): add RenameTag and BulkSoftDelete audit actions"
```

---

## Task 4: 后端 — tags/stats 和 PATCH tags/{id} API 路由

**Files:**
- Modify: `src/api/tags.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: 添加 tags_stats handler**

在 `src/api/tags.rs` 中添加。使用模型层缓存函数 `TagStats::list_cached`（与现有 `list_tags` 使用 `list_tags_cached` 的模式一致）：

```rust
pub async fn tags_stats(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<TagStats>>, ApiError> {
    let stats = tag::TagStats::list_cached(&state.pool, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(stats))
}
```

- [ ] **Step 2: 添加 rename_tag handler**

```rust
#[derive(serde::Deserialize)]
struct RenameTagRequest {
    label: String,
}

pub async fn rename_tag(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(tag_id): Path<Uuid>,
    Json(body): Json<RenameTagRequest>,
) -> Result<Json<Tag>, ApiError> {
    let tag = tag::rename_tag(&state.pool, tag_id, auth.user_id, &body.label)
        .await
        .map_err(|e| match e {
            tag::RenameError::Conflict => ApiError::Conflict("Tag with this name already exists".into()),
            tag::RenameError::Database(msg) => ApiError::Internal(msg),
        })?;

    // Audit log
    let auth_source = match auth.source {
        AuthSource::Jwt => "jwt".to_string(),
        AuthSource::Pat { .. } => "pat".to_string(),
    };
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source,
            action: AuditAction::RenameTag,
            resource_type: Some(AuditResourceType::Tag),
            resource_id: Some(tag_id),
            status: "success".to_string(),
            details: serde_json::json!({"new_label": body.label}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;

    Ok(Json(tag))
}
```

- [ ] **Step 3: 注册路由**

在 `src/api/mod.rs` 的 tag router 中添加：

```rust
// 在现有 tag 路由中添加
.route("/stats", get(tags::tags_stats))
// 修改 tags/{id} 路由，添加 patch
.route("/{id}", delete(tags::delete_tag).patch(tags::rename_tag))
```

- [ ] **Step 4: 编译验证**

Run: `docker build --target test --build-arg "TEST_ARGS=--lib" -t lettura-test .`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/api/tags.rs src/api/mod.rs
git commit -m "feat(api): add GET /tags/stats and PATCH /tags/{id} endpoints"
```

---

## Task 5: 后端 — list_entries 返回标签

**Files:**
- Modify: `src/models/entry.rs`

- [ ] **Step 1: 添加 tags 字段到 EntrySummary**

在 `EntrySummary` 结构体中添加 `tags` 字段，使用 `#[sqlx(skip)]` 避免从 SQL 行自动填充：

```rust
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct EntrySummary {
    // ...existing fields...
    pub deleted_at: Option<DateTime<Utc>>,
    #[sqlx(skip)]
    pub tags: Vec<crate::models::tag::TagLabel>,
}
```

- [ ] **Step 2: 修改 list_entries 函数**

在 `list_entries` 函数中，查询条目后批量查询标签并附加：

```rust
pub async fn list_entries(
    pool: &PgPool,
    user_id: Uuid,
    params: &ListParams,
) -> Result<Vec<EntrySummary>, ModelError> {
    // ...existing query logic...
    let mut entries: Vec<EntrySummary> = /* existing query result */;

    // Batch-fetch tags for all entries
    if !entries.is_empty() {
        let entry_ids: Vec<Uuid> = entries.iter().map(|e| e.id).collect();
        let tag_rows: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
            "SELECT et.entry_id, t.id AS tag_id, t.label
             FROM entry_tags et
             JOIN tags t ON t.id = et.tag_id
             WHERE et.entry_id = ANY($1)
             ORDER BY t.label"
        )
        .bind(&entry_ids)
        .fetch_all(pool)
        .await
        .map_err(|e| ModelError::Database(e.to_string()))?;

        let mut tags_by_entry: std::collections::HashMap<Uuid, Vec<crate::models::tag::TagLabel>> =
            std::collections::HashMap::new();
        for (entry_id, tag_id, label) in tag_rows {
            tags_by_entry
                .entry(entry_id)
                .or_default()
                .push(crate::models::tag::TagLabel { id: tag_id, label });
        }

        for entry in &mut entries {
            entry.tags = tags_by_entry.remove(&entry.id).unwrap_or_default();
        }
    }

    Ok(entries)
}
```

- [ ] **Step 3: 处理 list_entries_by_ids 和 list_deleted_entries**

这两个函数也返回 `Vec<EntrySummary>`。添加相同的批量标签查询逻辑，或提取为辅助函数 `attach_tags(pool, &mut entries)`。

- [ ] **Step 4: 编译验证**

Run: `docker build --target test --build-arg "TEST_ARGS=--lib entry" -t lettura-test .`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/models/entry.rs
git commit -m "feat(entry): include tags in list_entries response"
```

---

## Task 6: 后端 — bulk-by-ids API 端点

**Files:**
- Modify: `src/api/bulk.rs`

- [ ] **Step 1: 添加请求结构体和 handlers**

使用现有的 `BulkResult` 结构体（`{ matched, updated, ids }`）：

```rust
#[derive(serde::Deserialize)]
struct BulkTagByIdsRequest {
    entry_ids: Vec<Uuid>,
    tags: Vec<String>,
}

#[derive(serde::Deserialize)]
struct BulkUntagByIdsRequest {
    entry_ids: Vec<Uuid>,
    tags: Vec<String>,
}

#[derive(serde::Deserialize)]
struct BulkDeleteByIdsRequest {
    entry_ids: Vec<Uuid>,
}
```

`bulk_tag_by_ids` handler：

```rust
pub async fn bulk_tag_by_ids(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<BulkTagByIdsRequest>,
) -> Result<Json<BulkResult>, ApiError> {
    if body.entry_ids.is_empty() || body.tags.is_empty() {
        return Ok(Json(BulkResult { matched: 0, updated: 0, ids: vec![] }));
    }
    tag::ensure_and_link(&state.pool, auth.user_id, &body.entry_ids, &body.tags)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let count = body.entry_ids.len();
    // Audit log + cache invalidation
    crate::cache::TAG_STATS_CACHE.invalidate(auth.user_id).await;
    Ok(Json(BulkResult { matched: count, updated: count, ids: body.entry_ids.clone() }))
}
```

类似地实现 `bulk_untag_by_ids` 和 `bulk_delete_by_ids`（软删除：`UPDATE entries SET deleted_at = NOW() WHERE id = ANY($1) AND user_id = $2`）。

- [ ] **Step 2: 注册路由**

在 `src/api/mod.rs` 的 bulk router 中添加：

```rust
.route("/bulk/tag-by-ids", post(bulk::bulk_tag_by_ids))
.route("/bulk/untag-by-ids", post(bulk::bulk_untag_by_ids))
.route("/bulk/delete-by-ids", post(bulk::bulk_delete_by_ids))
.route("/bulk/archive-by-ids", post(bulk::bulk_archive_by_ids))
```

- [ ] **Step 3: 编译验证**

Run: `docker build --target test --build-arg "TEST_ARGS=--lib" -t lettura-test .`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add src/api/bulk.rs src/api/mod.rs
git commit -m "feat(api): add bulk-by-ids endpoints for tag/untag/delete/archive"
```

---

## Task 7: 后端 — Wallabag 导入标签

**Files:**
- Modify: `src/api/import.rs`

- [ ] **Step 1: 在 import_wallabag 中添加标签关联**

在 `import_wallabag` 函数中，`update_entry_content` 调用之后、`imported += 1` 之前，添加：

```rust
// Import tags from Wallabag
if let Some(ref tag_labels) = wb_entry.tags {
    if !tag_labels.is_empty() {
        if let Err(e) = tag::ensure_and_link(
            &state.pool,
            auth.user_id,
            &[new_entry.id],
            tag_labels,
        )
        .await
        {
            tracing::warn!(entry_id = %new_entry.id, "failed to import tags: {e}");
        }
    }
}
```

确保文件顶部有 `use crate::models::tag;`。

- [ ] **Step 2: 编译验证**

Run: `docker build --target test --build-arg "TEST_ARGS=--lib" -t lettura-test .`
Expected: PASS

- [ ] **Step 3: 提交**

```bash
git add src/api/import.rs
git commit -m "feat(import): import Wallabag tags during entry import"
```

---

## Task 8: 前端 — TagBadge 组件和 API 更新

**Files:**
- Create: `web/src/components/TagBadge.tsx`
- Modify: `web/src/api/tags.ts`
- Modify: `web/src/api/entries.ts`
- Create: `web/src/api/taggingRules.ts`

- [ ] **Step 1: 创建 TagBadge 组件**

```tsx
import { useNavigate } from "react-router-dom";
import { Badge } from "@/components/ui/badge";

interface TagBadgeProps {
  label: string;
  clickable?: boolean;
  onRemove?: () => void;
}

export function TagBadge({ label, clickable = true, onRemove }: TagBadgeProps) {
  const navigate = useNavigate();

  const handleClick = () => {
    if (!clickable) return;
    navigate(`/?tag=${encodeURIComponent(label)}`);
  };

  return (
    <Badge
      variant="secondary"
      className={`text-xs ${clickable ? "cursor-pointer hover:bg-secondary/80" : ""}`}
      onClick={handleClick}
    >
      {label}
      {onRemove && (
        <button
          className="ml-1 hover:text-destructive"
          onClick={(e) => { e.stopPropagation(); onRemove(); }}
        >
          ×
        </button>
      )}
    </Badge>
  );
}
```

- [ ] **Step 2: 更新 tags.ts API**

在 `web/src/api/tags.ts` 中添加：

```typescript
export interface TagStats {
  id: string;
  label: string;
  slug: string;
  entry_count: number;
  created_at: string;
}

export async function fetchTagStats(): Promise<TagStats[]> {
  const res = await api.get('/tags/stats');
  return res.data;
}

export async function renameTag(id: string, label: string): Promise<Tag> {
  const res = await api.patch(`/tags/${id}`, { label });
  return res.data;
}

export async function bulkTagByIds(entryIds: string[], tags: string[]): Promise<void> {
  await api.post('/entries/bulk/tag-by-ids', { entry_ids: entryIds, tags });
}

export async function bulkUntagByIds(entryIds: string[], tags: string[]): Promise<void> {
  await api.post('/entries/bulk/untag-by-ids', { entry_ids: entryIds, tags });
}

export async function bulkDeleteByIds(entryIds: string[]): Promise<void> {
  await api.post('/entries/bulk/delete-by-ids', { entry_ids: entryIds });
}

export async function bulkArchiveByIds(entryIds: string[]): Promise<void> {
  await api.post('/entries/bulk/archive-by-ids', { entry_ids: entryIds });
}
```

- [ ] **Step 3: 创建 taggingRules.ts API**

```typescript
import api from './client';

export interface TaggingRule {
  id: string;
  rule: unknown;
  tags: string[];
  priority: number;
  created_at: string;
}

export async function listRules(): Promise<TaggingRule[]> {
  const res = await api.get('/tagging-rules');
  return res.data;
}

export async function createRule(data: { rule: unknown; tags: string[]; priority?: number }): Promise<TaggingRule> {
  const res = await api.post('/tagging-rules', data);
  return res.data;
}

export async function updateRule(id: string, data: { rule?: unknown; tags?: string[]; priority?: number }): Promise<TaggingRule> {
  const res = await api.patch(`/tagging-rules/${id}`, data);
  return res.data;
}

export async function deleteRule(id: string): Promise<void> {
  await api.delete(`/tagging-rules/${id}`);
}
```

- [ ] **Step 4: 修复 entries.ts ListParams**

在 `web/src/api/entries.ts` 中，将 `ListParams` 的 `tags?: string[]` 改为 `tag?: string`，与后端对齐。同时给 `EntrySummary` 类型添加 `tags` 字段：

```typescript
export interface EntrySummary {
  // ...existing fields...
  tags: { id: string; label: string }[];
}

export interface ListParams {
  cursor?: string;
  page?: number;
  per_page?: number;
  is_archived?: boolean;
  is_starred?: boolean;
  search?: string;
  domain?: string;
  tag?: string;          // 替换 tags?: string[]
  exclude_tag?: string;  // 新增
  untagged?: boolean;    // 新增
}
```

- [ ] **Step 5: 验证前端编译**

Run: `cd web && pnpm tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 6: 提交**

```bash
git add web/src/components/TagBadge.tsx web/src/api/tags.ts web/src/api/taggingRules.ts web/src/api/entries.ts
git commit -m "feat(web): add TagBadge, tag stats/rename/bulk APIs, taggingRules API, fix ListParams"
```

---

## Task 9: 前端 — 侧边栏标签列表

**Files:**
- Modify: `web/src/components/layout/AppSidebar.tsx`

- [ ] **Step 1: 在 AppSidebar 中添加标签分组**

在"便签"分组下方（`<Separator>` 之后）添加标签区域：

```tsx
import { useQuery } from "@tanstack/react-query";
import { Tag as TagIcon } from "lucide-react";
import { fetchTagStats } from "@/api/tags";

// 在 AppSidebar 组件内部
const { data: tagStats } = useQuery({
  queryKey: ["tags", "stats"],
  queryFn: fetchTagStats,
  staleTime: 5 * 60 * 1000,
});

const topTags = (tagStats ?? [])
  .filter((t) => t.entry_count > 0)
  .sort((a, b) => b.entry_count - a.entry_count)
  .slice(0, 10);
```

渲染标签列表（使用 `NavLink`，与现有 navItems 模式一致）：

```tsx
<SidebarGroup>
  <SidebarGroupContent>
    <SidebarMenu>
      {topTags.map((tag) => (
        <SidebarMenuItem key={tag.id}>
          <SidebarMenuButton asChild>
            <NavLink
              to={`/?tag=${encodeURIComponent(tag.label)}`}
              className={({ isActive }) =>
                isActive ? "bg-accent text-accent-foreground font-medium" : "text-muted-foreground"
              }
            >
              <TagIcon size={18} />
              <span className="truncate">{tag.label}</span>
              <span className="ml-auto text-xs text-muted-foreground">{tag.entry_count}</span>
            </NavLink>
          </SidebarMenuButton>
        </SidebarMenuItem>
      ))}
      {(tagStats ?? []).length > 10 && (
        <SidebarMenuItem>
          <SidebarMenuButton asChild>
            <NavLink to="/tags" className="text-muted-foreground">
              <span>查看全部</span>
            </NavLink>
          </SidebarMenuButton>
        </SidebarMenuItem>
      )}
    </SidebarMenu>
  </SidebarGroupContent>
</SidebarGroup>
```

- [ ] **Step 2: 验证前端编译**

Run: `cd web && pnpm tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 3: 提交**

```bash
git add web/src/components/layout/AppSidebar.tsx
git commit -m "feat(sidebar): add tag list with entry counts sorted by usage"
```

---

## Task 10: 前端 — 文章卡片显示标签

**Files:**
- Modify: `web/src/components/EntryCard.tsx`

- [ ] **Step 1: 在 EntryCard 底部添加标签显示**

在卡片底部区域（star/archive 按钮下方）添加标签 badges：

```tsx
import { TagBadge } from "@/components/TagBadge";

// 在 star/archive 按钮的 div 之后
{entry.tags && entry.tags.length > 0 && (
  <div className="flex flex-wrap gap-1 mt-2">
    {entry.tags.slice(0, 3).map((tag) => (
      <TagBadge key={tag.id} label={tag.label} />
    ))}
    {entry.tags.length > 3 && (
      <span className="text-xs text-muted-foreground">+{entry.tags.length - 3}</span>
    )}
  </div>
)}
```

- [ ] **Step 2: 验证前端编译**

Run: `cd web && pnpm tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 3: 提交**

```bash
git add web/src/components/EntryCard.tsx
git commit -m "feat(entry-card): display tags with TagBadge component"
```

---

## Task 11: 前端 — 标签筛选 UI

**Files:**
- Modify: `web/src/pages/EntryListPage.tsx`
- Modify: `web/src/hooks/useInfiniteEntries.ts`

- [ ] **Step 1: 修改 useInfiniteEntries 支持 tag 参数**

确保 `useInfiniteEntries` 的 `ListParams` 正确传递 `tag`/`exclude_tag`/`untagged` 参数。当前 `ListParams` 已有这些字段（Task 8 添加），只需确保 `useInfiniteEntries` 透传它们。

- [ ] **Step 2: 添加标签筛选指示器**

在 `EntryListPage` 中，从 URL 读取 tag 参数并传递给 `useInfiniteEntries`：

```tsx
import { useSearchParams } from "react-router-dom";
import { TagBadge } from "@/components/TagBadge";

// 在组件内部
const [searchParams, setSearchParams] = useSearchParams();
const activeTag = searchParams.get("tag");
const excludeTag = searchParams.get("exclude_tag");
const untagged = searchParams.get("untagged");

// 修改 params 构建
const params: Omit<ListParams, "cursor"> = {};
// ...existing filter logic...
if (activeTag) params.tag = activeTag;
if (excludeTag) params.exclude_tag = excludeTag;
if (untagged) params.untagged = true;
```

筛选指示器（在标题区域，domain badge 旁边）：

```tsx
{activeTag && (
  <TagBadge
    label={activeTag}
    clickable={false}
    onRemove={() => {
      setSearchParams((prev) => { prev.delete("tag"); return prev; });
    }}
  />
)}
{untagged && (
  <TagBadge
    label="未标签"
    clickable={false}
    onRemove={() => {
      setSearchParams((prev) => { prev.delete("untagged"); return prev; });
    }}
  />
)}
```

- [ ] **Step 3: 验证前端编译**

Run: `cd web && pnpm tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 4: 提交**

```bash
git add web/src/pages/EntryListPage.tsx web/src/hooks/useInfiniteEntries.ts
git commit -m "feat(entry-list): add tag filter indicator with remove button"
```

---

## Task 12: 前端 — EntryTags 组件修复和自动补全

**Files:**
- Modify: `web/src/components/EntryTags.tsx`

- [ ] **Step 1: 修复根本性 bug — 只显示条目关联的标签**

当前组件调用 `GET /tags` 获取所有用户标签。修复为调用 `GET /entries/{id}/tags`（后端已有 `list_tags_for_entry` 路由）：

```tsx
const { data: entryTags = [], refetch: refetchTags } = useQuery({
  queryKey: ['entry-tags', entryId],
  queryFn: async () => {
    const res = await api.get(`/entries/${entryId}/tags`);
    return res.data as Tag[];
  },
});
```

渲染 `entryTags` 而非 `allTags`。

- [ ] **Step 2: 修复 setNewTag 时机和防重复提交**

```tsx
const [isAdding, setIsAdding] = useState(false);

const handleAddTag = async (e: React.FormEvent) => {
  e.preventDefault();
  const label = input.trim();
  if (!label || isAdding) return;
  setIsAdding(true);
  try {
    await addTagToEntry(entryId, label);
    setInput("");  // 成功后才清空
    refetchTags();
    qc.invalidateQueries({ queryKey: ['tags', 'stats'] });
  } catch {
    toast.error('添加标签失败');
  } finally {
    setIsAdding(false);
  }
};
```

- [ ] **Step 3: 添加自动补全**

使用 `useQuery` 缓存的 `fetchTagStats` 数据，输入时过滤匹配标签：

```tsx
const { data: tagStats } = useQuery({
  queryKey: ['tags', 'stats'],
  queryFn: fetchTagStats,
  staleTime: 5 * 60 * 1000,
});

const [showSuggestions, setShowSuggestions] = useState(false);
const [selectedIndex, setSelectedIndex] = useState(-1);

const suggestions = (tagStats ?? [])
  .filter((t) => t.label.toLowerCase().includes(input.toLowerCase()))
  .filter((t) => !entryTags.some((et) => et.label === t.label))
  .slice(0, 10);
```

渲染下拉列表（使用 shadcn `Command` 组件），支持键盘导航（上下箭头 + Enter）。

- [ ] **Step 4: 验证前端编译**

Run: `cd web && pnpm tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 5: 提交**

```bash
git add web/src/components/EntryTags.tsx
git commit -m "fix(entry-tags): show only entry tags, add autocomplete, fix submit bugs"
```

---

## Task 13: 前端 — 设置页标签管理

**Files:**
- Modify: `web/src/pages/SettingsPage.tsx`

- [ ] **Step 1: 在设置页添加标签管理区域**

使用 `fetchTagStats` 获取数据，渲染表格：

- 列：标签名、文章数、操作（重命名/删除）
- 重命名：行内编辑（点击编辑图标 → input → Enter 保存 / Esc 取消）
- 删除：确认对话框 → `deleteTag(id)` → 刷新缓存

```tsx
const { data: tagStats, refetch: refetchTagStats } = useQuery({
  queryKey: ["tags", "stats"],
  queryFn: fetchTagStats,
});

const handleRename = async (id: string, newLabel: string) => {
  await renameTag(id, newLabel);
  await queryClient.invalidateQueries({ queryKey: ["tags", "stats"] });
};

const handleDelete = async (id: string) => {
  await deleteTag(id);
  await queryClient.invalidateQueries({ queryKey: ["tags", "stats"] });
};
```

- [ ] **Step 2: 验证前端编译**

Run: `cd web && pnpm tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 3: 提交**

```bash
git add web/src/pages/SettingsPage.tsx
git commit -m "feat(settings): add tag management with rename and delete"
```

---

## Task 14: 前端 — 独立标签页 /tags

**Files:**
- Create: `web/src/pages/TagsPage.tsx`
- Modify: `web/src/App.tsx`

- [ ] **Step 1: 创建 TagsPage**

标签云样式：flex wrap 布局，每个 chip 显示名称 + 文章数，可点击跳转筛选，hover 显示编辑/删除图标（使用 lucide-react `Pencil` 和 `Trash2` 图标）。

```tsx
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "react-router-dom";
import { Pencil, Trash2 } from "lucide-react";
import { fetchTagStats, deleteTag, type TagStats } from "@/api/tags";
import { AlertDialog, AlertDialogTrigger, AlertDialogContent, AlertDialogHeader, AlertDialogTitle, AlertDialogDescription, AlertDialogCancel, AlertDialogAction } from "@/components/ui/alert-dialog";

export default function TagsPage() {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const { data: tagStats } = useQuery({
    queryKey: ["tags", "stats"],
    queryFn: fetchTagStats,
  });

  const handleDelete = async (id: string) => {
    await deleteTag(id);
    await qc.invalidateQueries({ queryKey: ["tags", "stats"] });
  };

  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-6">标签</h1>
      <div className="flex flex-wrap gap-3">
        {tagStats?.map((tag) => (
          <Link
            key={tag.id}
            to={`/?tag=${encodeURIComponent(tag.label)}`}
            className="group flex items-center gap-1 rounded-full bg-secondary px-3 py-1.5 text-sm hover:bg-secondary/80"
          >
            {tag.label}
            <span className="text-muted-foreground">({tag.entry_count})</span>
            <button
              className="ml-1 hidden group-hover:inline text-muted-foreground hover:text-foreground"
              onClick={(e) => { e.preventDefault(); navigate("/settings"); }}
            >
              <Pencil size={14} />
            </button>
          </Link>
        ))}
      </div>
      <div className="mt-8">
        <Link to="/?untagged=true" className="text-sm text-muted-foreground hover:text-foreground">
          查看未标签文章
        </Link>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 在 App.tsx 中添加路由**

```tsx
const TagsPage = lazy(() => import('./pages/TagsPage'));

// 在路由配置中添加
<Route path="/tags" element={<TagsPage />} />
```

- [ ] **Step 3: 验证前端编译**

Run: `cd web && pnpm tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 4: 提交**

```bash
git add web/src/pages/TagsPage.tsx web/src/App.tsx
git commit -m "feat(tags): add /tags page with tag cloud layout"
```

---

## Task 15: 前端 — 批量打标签 UI

**Files:**
- Modify: `web/src/pages/EntryListPage.tsx`

- [ ] **Step 1: 添加多选模式**

- 添加"多选"按钮到列表工具栏
- 每张卡片左上角添加复选框（多选模式激活时显示）
- 勾选文章后，顶部出现浮动操作栏

```tsx
const [selectionMode, setSelectionMode] = useState(false);
const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

const toggleSelect = (id: string) => {
  setSelectedIds((prev) => {
    const next = new Set(prev);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    return next;
  });
};
```

- [ ] **Step 2: 浮动操作栏**

```tsx
{selectionMode && selectedIds.size > 0 && (
  <div className="fixed bottom-4 left-1/2 -translate-x-1/2 flex items-center gap-2 bg-card border rounded-lg shadow-lg p-3 z-50">
    <span className="text-sm">已选 {selectedIds.size} 篇</span>
    <Button size="sm" onClick={handleBulkTag}>打标签</Button>
    <Button size="sm" variant="outline" onClick={handleBulkUntag}>取消标签</Button>
    <Button size="sm" variant="outline" onClick={handleBulkArchive}>归档</Button>
    <Button size="sm" variant="destructive" onClick={handleBulkDelete}>删除</Button>
  </div>
)}
```

打标签/取消标签使用自动补全输入框，调用 `bulkTagByIds` / `bulkUntagByIds` / `bulkDeleteByIds` / `bulkArchiveByIds`。

- [ ] **Step 3: 验证前端编译**

Run: `cd web && pnpm tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 4: 提交**

```bash
git add web/src/pages/EntryListPage.tsx
git commit -m "feat(entry-list): add multi-select mode with bulk tag/untag/delete"
```

---

## Task 16: 前端 — 标签规则管理 UI

**Files:**
- Modify: `web/src/pages/SettingsPage.tsx`

- [ ] **Step 1: 添加标签规则管理区域**

在设置页标签管理下方添加"标签规则"区域：

- 列表显示规则：条件 → 标签名
- 新建/编辑规则：条件构建器 + 标签输入（自动补全）
- 删除规则：确认对话框

条件构建器字段映射（UI 显示名 → API JSON 字段名）：

| UI 显示 | API 字段 | 类型 |
|---------|----------|------|
| 标题 | title | string |
| URL | url | string |
| 域名 | domainName | string |
| 语言 | language | string |
| 阅读时间 | readingTime | number |
| 内容类型 | contentType | string |

操作符映射（UI 显示 → API 值）：

| UI 显示 | API 值 | 适用类型 |
|---------|--------|----------|
| 等于 | eq | string/number |
| 不等于 | neq | string/number |
| 包含 | contains | string |
| 不包含 | not_contains | string |
| 匹配正则 | matches | string |
| 大于 | gt | number |
| 小于 | lt | number |

调用已有 API：`listRules`、`createRule`、`updateRule`、`deleteRule`（来自 `web/src/api/taggingRules.ts`）。

- [ ] **Step 2: 验证前端编译**

Run: `cd web && pnpm tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 3: 提交**

```bash
git add web/src/pages/SettingsPage.tsx
git commit -m "feat(settings): add tagging rules management UI"
```

---

## Task 17: 集成测试和缓存失效验证

**Files:** 无新增，可能修改 `src/api/import.rs` 和 `src/api/tags.rs`

- [ ] **Step 1: 验证所有标签变更操作都使缓存失效**

检查以下操作是否都调用了 `TAG_STATS_CACHE.invalidate(user_id)` 和 `TAG_CACHE.invalidate(user_id)`：
- `tags_stats` — 只读，不失效
- `add_tag_to_entry` — 添加标签到条目 → 失效 TAG_STATS_CACHE
- `remove_tag_from_entry` — 从条目移除标签 → 失效 TAG_STATS_CACHE
- `delete_tag` — 删除标签 → 失效 TAG_STATS_CACHE（已有 TAG_CACHE 失效）
- `rename_tag` — 重命名标签 → 失效 TAG_STATS_CACHE（已在 rename 函数中）
- `bulk_tag_by_ids` — 批量打标签 → 失效 TAG_STATS_CACHE
- `bulk_untag_by_ids` — 批量取消标签 → 失效 TAG_STATS_CACHE
- `import_wallabag` — 导入标签 → 失效 TAG_STATS_CACHE（ensure_and_link 已失效 TAG_CACHE，需额外失效 TAG_STATS_CACHE）

- [ ] **Step 2: 运行完整测试套件**

Run: `docker build --target test -t lettura-test .`
Expected: PASS

- [ ] **Step 3: 提交**

```bash
git add -A
git commit -m "fix: ensure tag stats cache invalidation on all mutation operations"
```

---

## Task 18: 构建部署和端到端验证

**Files:** 无代码变更

- [ ] **Step 1: 构建并部署**

Run: `./dev.sh build`

- [ ] **Step 2: 验证侧边栏标签列表**

- 打开应用，确认侧边栏显示标签分组
- 确认标签按文章数排序
- 点击标签，确认跳转到筛选视图

- [ ] **Step 3: 验证文章卡片标签**

- 确认列表中卡片底部显示标签
- 点击标签 badge，确认筛选生效

- [ ] **Step 4: 验证标签筛选 UI**

- 确认筛选指示器显示当前标签
- 点击 × 关闭筛选

- [ ] **Step 5: 验证 EntryTags 修复**

- 打开条目详情，确认只显示该条目的标签
- 添加标签，确认自动补全工作
- 确认不会重复提交

- [ ] **Step 6: 验证设置页标签管理**

- 重命名标签，确认成功
- 删除标签，确认确认对话框和成功

- [ ] **Step 7: 验证 /tags 页**

- 访问 /tags，确认标签云显示
- 点击标签 chip，确认跳转筛选

- [ ] **Step 8: 验证批量操作**

- 进入多选模式
- 选择多篇文章
- 批量打标签/取消标签/删除

- [ ] **Step 9: 验证标签规则管理**

- 创建新规则
- 编辑规则
- 删除规则
