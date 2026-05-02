# 标签功能完整改进设计

日期: 2026-05-03

## 问题

当前标签功能存在以下痛点：

1. **标签无法导航/筛选** — 侧边栏没有标签列表，打完标签无处使用，形同虚设
2. **文章卡片不显示标签** — 列表中看不到文章属于哪些标签，只有点进详情才能看到
3. **添加标签体验差** — 没有自动补全，不知道已有哪些标签，容易重复创建
4. **缺少标签管理** — 无法重命名、删除标签，无法查看每个标签的文章数
5. **缺少批量操作** — 无法批量打标签/取消标签
6. **标签规则无 UI** — 只能通过 API/CLI 管理标签规则
7. **EntryTags 组件有根本性 bug** — 当前组件调用 `GET /tags` 获取所有用户标签并全部渲染为 badge，而不是只显示当前条目关联的标签。打开任意条目的标签面板都会显示系统中所有标签

## 方案

### 1. 后端新增 API

#### `GET /api/v1/tags/stats`

返回标签列表及文章数，供侧边栏和标签管理页使用。

```json
[
  {"id": "uuid", "label": "rust", "slug": "rust", "entry_count": 12, "created_at": "..."},
  {"id": "uuid", "label": "n8n", "slug": "n8n", "entry_count": 5, "created_at": "..."}
]
```

SQL:
```sql
SELECT t.id, t.label, t.slug, t.created_at,
       COUNT(et.entry_id)::int AS entry_count
FROM tags t
LEFT JOIN entry_tags et ON et.tag_id = t.id
LEFT JOIN entries e ON e.id = et.entry_id AND e.deleted_at IS NULL
WHERE t.user_id = $1
GROUP BY t.id
ORDER BY t.label
```

注意：只统计未删除的条目（`e.deleted_at IS NULL`）。

不分页。大多数用户标签数在合理范围内（<500），全量返回足够。如果后续出现性能问题再加分页。

缓存策略：使用独立的 `TAG_STATS_CACHE`（与现有 `TAG_CACHE` 分离），TTL 5 分钟。标签变更（添加/删除/重命名/关联/取消关联）后主动使缓存失效。

#### `PATCH /api/v1/tags/{id}`

重命名标签。请求体：

```json
{"label": "new-name"}
```

更新 `label` 和 `slug`。slug 冲突时返回 409，但排除自身：如果新 slug 与当前标签自身的 slug 相同（例如 "Rust" → "rust"，slug 均为 "rust"），正常返回 200。仅在**不同** tag_id 具有相同 slug 时返回 409。

返回更新后的标签。

审计日志：重命名操作需写入审计日志，与现有 tag 操作保持一致。

#### `list_entries` 返回标签

在 `entry::list_entries` 查询中，为每个条目附加标签列表，避免前端 N+1 请求。

修改 `EntrySummary` 结构体（非 `Entry`），新增 `tags: Vec<TagLabel>` 字段。

`TagLabel` 定义：
```rust
pub struct TagLabel {
    pub id: Uuid,
    pub label: String,
}
```

序列化格式：
```json
"tags": [{"id": "uuid-1", "label": "rust"}, {"id": "uuid-2", "label": "n8n"}]
```

向后兼容：新增字段，前端未更新时忽略该字段，不影响现有功能。

实现方式：查询条目后，批量查询这些条目的标签：

```sql
SELECT et.entry_id, t.id AS tag_id, t.label
FROM entry_tags et
JOIN tags t ON t.id = et.tag_id
WHERE et.entry_id = ANY($1)
ORDER BY t.label
```

性能考虑：每页 20 条、每条 5 个标签，额外 100 行数据，可接受。

#### `POST /api/v1/entries/bulk/tag-by-ids`

新增基于 entry_ids 的批量打标签端点，供前端多选模式使用。

现有 `POST /entries/bulk/tag` 使用 `filter: ListParams` 匹配条目，适合规则驱动的批量操作，但不适合用户手动多选的场景。

请求体：
```json
{
  "entry_ids": ["uuid-1", "uuid-2", "uuid-3"],
  "tags": ["rust", "programming"]
}
```

实现：调用现有 `tag::ensure_and_link` 函数，与 `bulk_tag_add` 逻辑一致。

审计日志：复用 `AuditAction::BulkTagAdd`，details 中记录 `entry_ids` 和 `tags`。

类似地，新增 `POST /entries/bulk/untag-by-ids` 和 `POST /entries/bulk/delete-by-ids`：
- `untag-by-ids`: `{entry_ids, tags}` — 批量取消标签
- `delete-by-ids`: `{entry_ids}` — 批量软删除条目

#### Wallabag 导入标签

当前 `WallabagEntry` 结构体中 `tags: Option<Vec<String>>` 已是扁平字符串数组格式，与 Wallabag 实际导出格式一致（Wallabag 导出的 JSON 中标签为 `["rust", "programming"]` 字符串数组，而非对象数组）。

在 `import_wallabag` 中，导入条目成功后，解析 `wb_entry.tags`，调用 `tag::ensure_and_link` 关联标签：

```rust
if let Some(ref tag_labels) = wb_entry.tags {
    if !tag_labels.is_empty() {
        tag::ensure_and_link(&state.pool, auth.user_id, &[new_entry.id], tag_labels).await.ok();
    }
}
```

### 2. 侧边栏标签列表

**文件**: `web/src/components/layout/AppSidebar.tsx`

- 在"便签"分组下方添加"标签"分组
- 调用 `GET /tags/stats` 获取标签及文章数
- 每个标签显示：标签名 + 文章数（如 "rust (12)"）
- 点击标签导航到 `/?tag=rust`
- 最多显示 10 个标签，底部"查看全部"链接到 `/tags`
- 排序策略：按 `entry_count DESC` 排序（最常用的标签优先展示）
- 隐藏 `entry_count` 为 0 的标签
- 使用 shadcn `SidebarMenu` / `SidebarMenuItem` 组件
- 数据用 `useQuery` 缓存，queryKey: `['tags', 'stats']`，staleTime 5 分钟

### 3. 文章卡片显示标签

**文件**: `web/src/pages/EntryListPage.tsx`（或 EntryCard 组件）

- 卡片底部显示标签 badge
- 新建 `TagBadge` 组件（基于 shadcn `Badge` 封装，添加点击跳转行为：点击后设置 `?tag=label` 筛选）
- 最多显示 3 个标签，超出显示 "+N"
- 数据来源：`list_entries` API 返回的 `tags` 字段

### 4. 标签筛选 UI

**文件**: `web/src/pages/EntryListPage.tsx`

- 支持 `tag` URL 参数（已有后端支持）
- 修复前端 `ListParams`：将 `tags?: string[]` 改为 `tag?: string`，与后端 `ListParams.tag: Option<String>` 对齐
- 有标签筛选时，在搜索栏旁显示筛选标签 badge + 关闭按钮（×）
- 关闭按钮清除 `tag` 参数，恢复全量列表
- `exclude_tag` 和 `untagged` 参数同样支持

### 5. EntryTags 组件改进

**文件**: `web/src/components/EntryTags.tsx`

Bug 修复：
- **根本性 bug**：当前组件调用 `GET /tags` 获取所有用户标签并全部渲染为 badge。修复为：调用 `GET /entries/{id}/tags` 获取该条目关联的标签，只显示这些标签
- `setInput('')` 移到 API 调用成功后
- 添加加载状态（添加标签时按钮显示 spinner）
- 添加错误提示（添加失败时显示 toast）
- 防止重复提交（添加中禁用按钮）

自动补全：
- 输入时显示已有标签的下拉列表
- 使用 shadcn `Command` 组件实现 Combobox
- 数据来源：`GET /tags/stats`（缓存在 useQuery 中，queryKey: `['tags', 'stats']`）
- 最少输入 1 个字符时触发补全
- 防抖 200ms
- 最多显示 10 个候选项
- 支持键盘导航（上下箭头选择，Enter 确认）

### 6. 设置页标签管理

**文件**: `web/src/pages/SettingsPage.tsx`

在设置页添加"标签管理"区域：
- 表格显示：标签名、文章数、操作
- 显示所有标签（包括 `entry_count` 为 0 的）
- 排序：按 `label` 字母排序
- 重命名：点击编辑图标，行内编辑（input + Enter 保存 / Esc 取消）
- 删除：点击删除图标，弹出确认对话框，调用 `DELETE /tags/{id}`
- 调用 `GET /tags/stats` 获取数据
- 删除/重命名后调用 `invalidateTagCache()` 刷新缓存

### 7. 独立标签页 `/tags`

**文件**: `web/src/pages/TagsPage.tsx`

- 标签云样式：响应式 flex wrap 布局，统一大小（不做动态大小）
- 每个标签 chip 显示：名称 + 文章数
- chip 可点击进入筛选视图（`/?tag=label`）
- chip 右上角有编辑/删除操作图标（hover 显示）
- 编辑/删除操作跳转到设置页标签管理区域（避免两处重复实现 CRUD）
- "未标签"入口：显示无标签文章数，点击进入 `/?untagged=true`
- 侧边栏"查看全部"链接到此处
- 路由：`/tags`
- 显示所有标签（包括 `entry_count` 为 0 的）

### 8. 批量打标签 UI

**文件**: `web/src/pages/EntryListPage.tsx`

- EntryListPage 添加多选模式
- 每张卡片左上角添加复选框（默认隐藏，点击"多选"按钮显示）
- 勾选文章后，顶部出现浮动操作栏：打标签 / 取消标签 / 归档 / 删除
- 打标签输入框支持自动补全
- 调用新增 API：
  - `POST /entries/bulk/tag-by-ids` — `{entry_ids, tags}`
  - `POST /entries/bulk/untag-by-ids` — `{entry_ids, tags}`
  - `POST /entries/bulk/archive` — 使用 `filter` 参数构造（将选中的 entry_ids 转为 filter）
  - `POST /entries/bulk/delete-by-ids` — `{entry_ids}`

### 9. 标签规则管理 UI

**文件**: `web/src/pages/SettingsPage.tsx`

在设置页添加"标签规则"区域：
- 列表显示：规则条件 → 标签名
- 新建规则：
  - 条件构建器：字段下拉（title/url/domain/language/reading_time/content_type）+ 操作符下拉（contains/equals/starts_with/regex/gt/lt）+ 值输入
  - 标签输入：支持多标签，自动补全
- 编辑规则：同新建，预填现有值
- 删除规则：带确认对话框
- 调用已有 API：`GET/POST/PATCH/DELETE /tagging-rules`
- 条件构建器使用受控组件，表单验证在提交时执行

### 10. 缓存策略统一

标签数据在多处被消费和修改（侧边栏、EntryTags、设置页、标签页），需要统一的缓存管理：

- 定义统一的 queryKey：`['tags', 'stats']`
- 定义 `invalidateTagCache` 工具函数：在标签变更（添加/删除/重命名/关联/取消关联）后调用，使所有相关缓存失效
- 后端 `TAG_STATS_CACHE` 与前端 `useQuery` 缓存独立管理，后端缓存 TTL 5 分钟，变更后主动失效

## 不做的事

- 不做标签云动态大小（字体随文章数变化）— 统一大小更简洁
- 不做标签颜色 — 后续可扩展
- 不做标签合并 — 复杂度高，后续独立实现
- 不做嵌套标签/标签分组 — 超出当前范围
