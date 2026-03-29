# 极简页面展示模块 — 设计规格

> 日期: 2026-04-17
> 状态: Draft

## 1. 概述

为 Lettura 新增一个轻量级 HTML 页面展示模块。用户可以上传 HTML 文件（含关联的 JS/CSS/图片），系统为其生成一个公开可访问的 URL，类似 GitHub Pages 的体验。适用于 AI 生成的 HTML demo、快速原型展示等场景。

### 核心特性

- 拖拽上传 HTML / CSS / JS / 图片 / ZIP 文件
- 自动生成短 ID 作为公开访问路径（`/p/{slug}`）
- 从 `<title>` 自动提取页面标题，可二次编辑
- 可选的页面级密码保护
- 软删除 / 禁用页面
- 一键复制分享链接
- 多用户支持，每个用户管理自己的页面

### 设计决策

- **方案选择**: 独立路由树 + 中间件密码校验（方案 A）
- **访问模式**: 公开为主，密码保护为可选
- **URL 结构**: 自动短 ID（12 字符）
- **文件存储**: 复用现有 storage 模块（local / OSS）
- **入口文件**: 单文件自动确定；多文件/ZIP 自动选中 `index.html` 或第一个 `.html`，用户可更改
- **管理界面**: 集成到 Lettura 主导航，弹窗式上传

## 2. 数据模型

### pages 表

```sql
CREATE TABLE pages (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        VARCHAR(12) NOT NULL UNIQUE,
    user_id     UUID NOT NULL REFERENCES users(id),
    title       VARCHAR(500) NOT NULL,
    description TEXT,
    entry_file  VARCHAR(500) NOT NULL,
    password    VARCHAR(255),
    status      VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    file_count  INTEGER NOT NULL DEFAULT 0,
    deleted_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_pages_slug ON pages(slug) WHERE deleted_at IS NULL;
CREATE INDEX idx_pages_user ON pages(user_id) WHERE deleted_at IS NULL;
```

**字段说明**:

| 字段 | 类型 | 说明 |
|------|------|------|
| id | UUID | 主键 |
| slug | VARCHAR(12) | 自动生成的短 ID，作为公开 URL 路径 |
| user_id | UUID | 所属用户 |
| title | VARCHAR(500) | 页面标题，从 `<title>` 自动填充，可编辑 |
| description | TEXT | 可选描述 |
| entry_file | VARCHAR(500) | 入口 HTML 文件名，如 "demo.html" |
| password | VARCHAR(255) | argon2 哈希密码，NULL 表示无密码 |
| status | VARCHAR(20) | `active` 或 `disabled`，CHECK 约束 |
| file_count | INTEGER | 页面包含的文件数量 |
| deleted_at | TIMESTAMPTZ | 软删除时间，NULL 表示未删除（与 entries 表风格一致） |
| created_at | TIMESTAMPTZ | 创建时间 |
| updated_at | TIMESTAMPTZ | 更新时间 |

### slug 生成规则

- 12 字符，字符集: `[a-z0-9]`
- 使用 `rand` crate 生成
- 依赖 DB UNIQUE 约束检测冲突：INSERT 失败时重新生成 slug 重试（最多 5 次），避免 SELECT+INSERT 的竞态条件
- 示例: `/p/a3k9x2m7bp01`

## 3. API 设计

### 3.1 管理 API（需登录）

| Method | Path | 说明 |
|--------|------|------|
| POST | `/api/v1/pages/upload` | 上传文件（multipart），解析后返回 HTML 文件列表 + 预填标题 |
| POST | `/api/v1/pages` | 确认创建页面 |
| GET | `/api/v1/pages` | 列出当前用户的页面 |
| PATCH | `/api/v1/pages/{id}` | 更新页面（标题、密码、状态等） |
| DELETE | `/api/v1/pages/{id}` | 软删除页面 |
| POST | `/api/v1/pages/{id}/restore` | 恢复已删除页面 |

#### POST /api/v1/pages/upload

请求: `multipart/form-data`，字段名 `files`，支持多个文件。

响应:
```json
{
  "upload_id": "uuid-temp-id",
  "html_files": ["index.html", "demo.html"],
  "default_entry": "index.html",
  "suggested_title": "My Demo Page",
  "file_count": 3
}
```

- `html_files`: 在上传文件中找到的所有 `.html` 文件
- `default_entry`: 自动选中的入口文件（优先 `index.html`，否则第一个 `.html`）
- `suggested_title`: 从 default_entry 的 `<title>` 解析，无则用文件名
- 服务端将上传文件保存到本地临时目录（`{STORAGE_LOCAL_PATH}/tmp/{upload_id}/`）。清理机制：
  - 确认创建成功后立即删除对应临时目录
  - 未确认的上传：`tokio::spawn(tokio::time::sleep(Duration::from_secs(1800)))` 30 分钟后自动清理
  - 临时文件始终存本地，不经过 OSS

#### POST /api/v1/pages

请求:
```json
{
  "upload_id": "uuid-temp-id",
  "entry_file": "index.html",
  "title": "My Demo Page",
  "description": "Optional description",
  "password": "optional-plain-text"
}
```

响应:
```json
{
  "id": "uuid",
  "slug": "a3k9x2m7bp01",
  "title": "My Demo Page",
  "url": "/p/a3k9x2m7bp01",
  "created_at": "2026-04-17T00:00:00Z"
}
```

处理流程:
1. 根据 `upload_id` 取出临时文件
2. 生成唯一 slug
3. 如有 password，argon2 哈希后存入
4. 将文件写入 storage（`pages/{slug}/xxx`）
5. 插入 DB 记录
6. 清理临时文件

#### GET /api/v1/pages

查询参数: `?status=active|disabled|deleted&page=1&limit=20`

响应:
```json
{
  "items": [
    {
      "id": "uuid",
      "slug": "a3k9x2m7bp01",
      "title": "My Demo Page",
      "description": "...",
      "has_password": true,
      "status": "active",
      "file_count": 3,
      "created_at": "...",
      "updated_at": "..."
    }
  ],
  "total": 42,
  "page": 1,
  "limit": 20
}
```

#### PATCH /api/v1/pages/{id}

请求（部分更新）:
```json
{
  "title": "New Title",
  "description": "New description",
  "password": "new-password-or-null-to-remove",
  "status": "disabled"
}
```

- `password`: 传 `null` 移除密码保护；传字符串设置新密码
- `status`: `active` 或 `disabled`

### 3.2 公开访问

| Method | Path | 说明 |
|--------|------|------|
| GET | `/p/{slug}` | 访问页面入口 HTML |
| GET | `/p/{slug}/*file` | 访问页面关联文件 |
| POST | `/p/{slug}/auth` | 提交密码验证 |

> **注意**: `/p/` 路径由后端精确路由注册，不走 SPA fallback。该路径已保留给展示模块，前端路由不可使用 `/p/` 前缀。

#### 访问流程

```
GET /p/{slug}
  → 查 DB: slug 存在 + deleted_at IS NULL + status='active'
  → 不存在/已删除/已禁用 → 404
  → 有 password 且无有效 cookie → 返回密码输入页（服务端渲染 HTML）
  → 无 password 或 cookie 有效 → 从 storage 读取 entry_file，返回 HTML

GET /p/{slug}/assets/style.css
  → 同上鉴权（复用中间件）
  → 从 storage 读取 pages/{slug}/assets/style.css 返回

POST /p/{slug}/auth
  → body: { "password": "xxx" }
  → 验证 argon2 哈希
  → 成功 → 设 cookie `page_auth_{slug}`（签名，24h 有效）+ 302 重定向到 /p/{slug}
  → 失败 → 重新返回密码输入页（带错误提示）
```

#### 密码输入页

服务端硬编码的极简 HTML 页面，特性:
- 单个密码输入框 + 提交按钮
- 支持 Enter 键提交
- 错误时显示 "密码错误" 提示
- 简洁的居中布局，响应式
- 无外部 JS/CSS 依赖

#### Cookie 设计

- 名称: `page_auth_{slug}`
- 值: HMAC-SHA256 签名的 token（使用 JWT_SECRET 作为密钥）
- 有效期: 24 小时
- HttpOnly, SameSite=Lax
- 每个页面独立 cookie

## 4. 文件存储

### 存储路径

所有页面文件使用独立存储路径，与 `/storage/` 公开路由隔离：

```
{STORAGE_LOCAL_PATH}/
  storage/           ← 现有 /storage/ 路由映射范围（图片等）
  pages/             ← 展示模块专用，不暴露给 /storage/ 路由
    a3k9x2m7bp01/
      index.html
      style.css
      script.js
      images/
        photo.png
  tmp/               ← 临时上传目录
    {upload_id}/
```

配置项：新增 `PAGES_STORAGE_PATH` 环境变量（默认 `{STORAGE_LOCAL_PATH}/../pages`），local 模式专用。

复用现有 `ImageStorage` trait 存储文件（接口通用，不仅限于图片）。需扩展 trait 新增 `get` 方法用于读取文件：

```rust
async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;
```

- `store` / `delete`：复用现有实现，key 为 `pages/{slug}/xxx`
- `get`：local 模式用 `tokio::fs::read` 读取，OSS 模式用 `get_object` 下载
- local 模式文件存储在 `PAGES_STORAGE_PATH/{slug}/`（独立于 `/storage/` 路由的 `STORAGE_LOCAL_PATH`）
- OSS 模式存储在 `pages/{slug}/` 前缀，通过 presigned URL 或 `get_object` 读取（不经过公开 `/storage/` 路由）

### ZIP 文件处理

- 使用 `zip` crate 解压
- 解压后保持目录结构
- 过滤隐藏文件（以 `.` 开头）和 macOS 元数据（`__MACOSX`）
- 安全检查：禁止解压含 `..` 路径的文件

### Content-Type 映射

| 扩展名 | Content-Type |
|--------|-------------|
| .html | text/html; charset=utf-8 |
| .css | text/css; charset=utf-8 |
| .js | application/javascript |
| .json | application/json |
| .svg | image/svg+xml |
| .png | image/png |
| .jpg/.jpeg | image/jpeg |
| .gif | image/gif |
| .webp | image/webp |
| .ico | image/x-icon |
| .woff/.woff2 | font/woff, font/woff2 |
| 其他 | application/octet-stream |

## 5. 前端设计

### 路由

在 `App.tsx` 新增:

```
/pages  →  PagesPage（页面列表 + 上传弹窗）
```

主导航新增"展示"入口。

### PagesPage

- 页面卡片列表，显示：标题、slug、创建时间、状态标签、文件数
- 操作栏: 新窗口打开 `/p/{slug}` | 复制分享链接 | 编辑 | 禁用/启用 | 删除
- 右上角"上传"按钮 → 打开 PageUploadModal
- 支持筛选: active / disabled / 已删除

### PageUploadModal

1. 拖拽区域，支持 HTML / CSS / JS / 图片 / ZIP
2. 上传后（调 `/api/v1/pages/upload`，使用 `FormData` 而非 JSON）显示:
   - HTML 文件下拉选择器（默认选中 index.html 或第一个）
   - 标题输入框（预填充 `<title>`）
   - 描述输入框（可选）
   - 密码输入框（可选）+ "自动生成"按钮（生成 8 位随机字母数字，显示明文 + 复制按钮）
3. "发布"按钮 → 调 POST `/api/v1/pages` → 关闭弹窗 → 列表刷新 → toast 显示分享链接

### 文件结构

```
web/src/
  api/
    pages.ts              -- API client
  pages/
    PagesPage.tsx          -- 列表页
  components/
    PageUploadModal.tsx    -- 上传弹窗
    PageCard.tsx           -- 列表卡片
```

## 6. 后端模块划分

```
src/
  models/
    page.rs               -- Page struct, CRUD queries
  api/
    pages.rs              -- 管理 API (upload/create/list/update/delete)
    pages_public.rs       -- 公开访问 (/p/{slug} 服务 + 密码验证)

migrations/
  012_create_pages.sql    -- 建表语句
```

## 7. 安全考虑

- **密码哈希**: 使用 argon2，与现有用户密码相同的哈希方案
- **Cookie 签名**: HMAC-SHA256，使用 JWT_SECRET 作为密钥
- **ZIP 解压**: 禁止路径遍历（`..`），过滤隐藏文件和系统元数据
- **文件大小限制**: 单次上传总大小限制（建议 10MB）- **XSS 防护**: 展示页面在 `/p/` 路由下，与主应用路径隔离。**已知限制**：展示页面与主应用同源同端口，恶意上传的 HTML 中的 JS 理论上可访问 localStorage 中的 auth token。后续可考虑为展示页面添加 CSP `sandbox` 策略
- **X-Frame-Options**: 全局安全头使用 `SetResponseHeaderLayer::if_not_present` 模式，`/p/` 路由在独立的 Axum Nest 中使用 `SetResponseHeaderLayer::overriding` 设置为 `SAMEORIGIN`。这样：
  - 普通路由：无内层设置 → 外层 `if_not_present` 生效 → DENY
  - 展示路由：内层 SAMEORIGIN → 外层检测到已存在不覆盖 → SAMEORIGIN
- **存储隔离**: 页面文件使用独立存储路径 `PAGES_STORAGE_PATH`，不在 `/storage/` 路由映射范围内，防止通过 `/storage/pages/{slug}/xxx` 绕过密码保护
- **访问频率**: 公开路由 `/p/` 复用全局 rate limit (100 req/min)
- **新增依赖**: `zip` crate（ZIP 解压）、`tempfile` crate（临时目录管理，可选）

## 8. 运维考虑

- **临时文件清理**: `tokio::spawn(sleep)` 在服务重启后丢失，建议服务启动时扫描 `tmp/` 目录清理超过 30 分钟的临时文件
- **永久删除**: 当前仅支持软删除。后续可添加永久删除端点（同时清理 storage 中的文件），避免软删除的页面文件永久占用存储

## 9. 不做的事情

- 不做自定义域名绑定
- 不做页面访问统计/分析
- 不做版本控制/历史回滚
- 不做协作编辑
- 不做页面模板
- 不做 SEO 优化
- 不做评论系统
