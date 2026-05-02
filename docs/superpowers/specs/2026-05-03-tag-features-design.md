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

#### `PATCH /api/v1/tags/{id}`

重命名标签。请求体：

```json
{"label": "new-name"}
```

更新 `label` 和 `slug`。slug 冲突时返回 409。返回更新后的标签。

#### `list_entries` 返回标签

在 `entry::list_entries` 查询中，为每个条目附加标签列表，避免前端 N+1 请求。

`Entry` 结构体新增 `tags: Vec<TagLabel>` 字段（`TagLabel { id, label }`）。

实现方式：查询条目后，批量查询这些条目的标签：

```sql
SELECT et.entry_id, t.id AS tag_id, t.label
FROM entry_tags et
JOIN tags t ON t.id = et.tag_id
WHERE et.entry_id = ANY($1)
ORDER BY t.label
```

#### `import_wallabag` 导入标签

解析 Wallabag JSON 中的 `tags` 字段，调用 `tag::ensure_and_link` 关联标签。

### 2. 侧边栏标签列表

**文件**: `web/src/components/layout/AppSidebar.tsx`

- 在"便签"分组下方添加"标签"分组
- 调用 `GET /tags/stats` 获取标签及文章数
- 每个标签显示：标签名 + 文章数（如 "rust (12)"）
- 点击标签导航到 `/?tag=rust`
- 最多显示 10 个标签，底部"查看全部"链接到 `/tags`
- 使用 shadcn `SidebarMenu` / `SidebarMenuItem` 组件
- 数据用 `useQuery` 缓存，staleTime 5 分钟

### 3. 文章卡片显示标签

**文件**: `web/src/pages/EntryListPage.tsx`（或 EntryCard 组件）

- 卡片底部显示标签 badge（使用现有 `TagBadge` 组件）
- 最多显示 3 个标签，超出显示 "+N"
- 标签可点击，点击后设置 `?tag=label` 筛选
- 数据来源：`list_entries` API 返回的 `tags` 字段

### 4. 标签筛选 UI

**文件**: `web/src/pages/EntryListPage.tsx`

- 支持 `tag` URL 参数（已有后端支持）
- 有标签筛选时，在搜索栏旁显示筛选标签 badge + 关闭按钮（×）
- 关闭按钮清除 `tag` 参数，恢复全量列表
- `exclude_tag` 和 `untagged` 参数同样支持

### 5. EntryTags 组件改进

**文件**: `web/src/components/EntryTags.tsx`

Bug 修复：
- `setNewTag('')` 移到 API 调用成功后
- 添加加载状态（添加标签时按钮显示 spinner）
- 添加错误提示（添加失败时显示 toast）
- 防止重复提交（添加中禁用按钮）

自动补全：
- 输入时显示已有标签的下拉列表
- 使用 shadcn `Command` 组件实现 Combobox
- 数据来源：`GET /tags/stats`（缓存在 useQuery 中）
- 支持键盘导航（上下箭头选择，Enter 确认）

### 6. 设置页标签管理

**文件**: `web/src/pages/SettingsPage.tsx`

在设置页添加"标签管理"区域：
- 表格显示：标签名、文章数、操作
- 重命名：点击编辑图标，行内编辑（input + Enter 保存 / Esc 取消）
- 删除：点击删除图标，弹出确认对话框，调用 `DELETE /tags/{id}`
- 调用 `GET /tags/stats` 获取数据
- 删除/重命名后刷新缓存

### 7. 独立标签页 `/tags`

**文件**: `web/src/pages/TagsPage.tsx`

- 标签云样式：响应式 flex wrap 布局
- 每个标签 chip 显示：名称 + 文章数
- chip 大小根据文章数变化（可选，简单方案：统一大小）
- chip 可点击进入筛选视图（`/?tag=label`）
- chip 右上角有编辑/删除操作图标（hover 显示）
- "未标签"入口：显示无标签文章数，点击进入 `/?untagged=true`
- 侧边栏"查看全部"链接到此处
- 路由：`/tags`

### 8. 批量打标签 UI

**文件**: `web/src/pages/EntryListPage.tsx`

- EntryListPage 添加多选模式
- 每张卡片左上角添加复选框（默认隐藏，点击"多选"按钮显示）
- 勾选文章后，顶部出现浮动操作栏：打标签 / 取消标签 / 归档 / 删除
- 打标签输入框支持自动补全
- 调用已有 API：
  - `POST /entries/bulk/tag` — `{entry_ids, tags}`
  - `POST /entries/bulk/untag` — `{entry_ids, tags}`
  - `POST /entries/bulk/archive` — `{entry_ids}`
  - `DELETE /entries/{id}` — 逐个调用（或新增批量删除 API）

### 9. 标签规则管理 UI

**文件**: `web/src/pages/SettingsPage.tsx`（或独立 `TaggingRulesPage.tsx`）

在设置页添加"标签规则"区域：
- 列表显示：规则条件 → 标签名
- 新建规则：
  - 条件构建器：字段下拉（title/url/domain/language/reading_time/content_type）+ 操作符下拉（contains/equals/starts_with/regex/gt/lt）+ 值输入
  - 标签输入：支持多标签，自动补全
- 编辑规则：同新建，预填现有值
- 删除规则：带确认对话框
- 调用已有 API：`GET/POST/PATCH/DELETE /tagging-rules`

### 10. Wallabag 导入标签

**文件**: `src/api/import.rs`

在 `import_wallabag` 中解析 `wb_entry.tags`（Wallabag JSON 中的标签数组），调用 `tag::ensure_and_link` 关联标签到导入的条目。

Wallabag 标签格式：
```json
{"tags": [{"label": "rust"}, {"label": "programming"}]}
```

## 不做的事

- 不做标签云动态大小（字体随文章数变化）— 统一大小更简洁
- 不做标签颜色 — 后续可扩展
- 不做标签合并 — 复杂度高，后续独立实现
- 不做嵌套标签/标签分组 — 超出当前范围
