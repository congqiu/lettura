# Lettura - Design Spec

## Overview

Lettura 是一个受 wallabag 启发的 read-it-later 应用，使用 Rust 构建核心服务，React SPA 作为前端。目标是提供极低资源占用的自托管体验，同时解决 wallabag 的几个核心痛点：部署复杂、内容提取质量差、缺少快速捕获能力、不支持内容编辑。

**项目定位**：先满足个人需求，成熟后开源面向自托管社区。

## Architecture

单体 Rust 服务 + 内嵌 SPA 静态文件，容器优先部署。

```
Docker Image (~50-80MB)
┌─────────────────────────────────┐
│  Axum HTTP Server               │
│  ├─ REST API (/api/*)           │
│  ├─ 内嵌 SPA 静态文件 (/)       │
│  └─ RSS Feed (/feed/*)         │
│                                 │
│  后台任务 (tokio tasks)          │
│  ├─ 内容抓取 + 提取引擎          │
│  ├─ 自动打标签规则引擎           │
│  └─ 全文索引更新 (tantivy)      │
│                                 │
│  PostgreSQL ←── SQLx            │
│  tantivy 索引 ←── Docker volume │
└─────────────────────────────────┘
```

docker-compose 包含两个服务：`lettura` (Rust 应用) + `postgres`。

Volumes:
- `lettura_data` → `/data/tantivy` — tantivy 全文索引持久化
- `postgres_data` → PG 数据目录

### Tech Stack

**后端：**
- Rust (edition 2024)
- Axum — HTTP 框架
- SQLx — 异步数据库，编译期 SQL 检查
- PostgreSQL — 数据库
- tantivy — 全文搜索引擎（内嵌）
- reqwest — HTTP 客户端
- scraper — HTML 解析与内容提取
- ammonia — HTML 清洗/白名单过滤
- rust-embed — 内嵌 SPA 静态文件
- argon2 — 密码哈希
- jsonwebtoken — JWT 认证

**前端：**
- React 18+ (TypeScript)
- Vite — 构建工具
- Tiptap — 富文本编辑器（基于 ProseMirror）
- TanStack Query — 数据请求
- Zustand — 状态管理
- Tailwind CSS — 样式

## Data Model

### users

| Column | Type | Note |
|--------|------|------|
| id | UUID | PK |
| username | VARCHAR(50) | unique |
| email | VARCHAR(255) | unique |
| password_hash | TEXT | argon2 |
| is_admin | BOOL | default false |
| feed_token | VARCHAR(64) | RSS feed 专用 token, 可重新生成 |
| created_at | TIMESTAMPTZ | |
| updated_at | TIMESTAMPTZ | |

### refresh_tokens

| Column | Type | Note |
|--------|------|------|
| id | UUID | PK |
| user_id | UUID | FK → users.id |
| token_hash | VARCHAR(64) | SHA256(token), 不存明文 |
| expires_at | TIMESTAMPTZ | 过期时间 |
| created_at | TIMESTAMPTZ | |

### entries

| Column | Type | Note |
|--------|------|------|
| id | UUID | PK |
| user_id | UUID | FK → users.id |
| url | TEXT | 最终 URL（重定向后） |
| given_url | TEXT | 用户输入的原始 URL |
| hashed_url | VARCHAR(40) | SHA1(url), 用于去重 |
| hashed_given_url | VARCHAR(40) | SHA1(given_url), 用于短链接去重 |
| title | TEXT | 可编辑 |
| content | TEXT | 提取后的正文 HTML, 可编辑 |
| content_type | VARCHAR(20) | "article" / "github_repo" / "memo_promoted" 等 |
| extract_method | VARCHAR(20) | "readability" / "site_rule" / "manual" / "failed" |
| is_content_edited | BOOL | default false, 编辑过则不自动覆盖 |
| language | VARCHAR(20) | 文章语言 (en, zh, ja 等), 影响阅读渲染 |
| http_status | SMALLINT | 抓取时的 HTTP 状态码, 用于调试和重抓决策 |
| reading_time | INT | 分钟 |
| preview_picture | TEXT | 封面图 URL |
| domain_name | VARCHAR(255) | 来源域名 |
| published_by | TEXT | 原文作者 |
| metadata | JSONB | 类型特有扩展数据 (stars, language 等) |
| is_archived | BOOL | default false |
| archived_at | TIMESTAMPTZ | |
| is_starred | BOOL | default false |
| starred_at | TIMESTAMPTZ | |
| published_at | TIMESTAMPTZ | 原文发布时间 |
| created_at | TIMESTAMPTZ | |
| updated_at | TIMESTAMPTZ | |

**Indexes:**
- (user_id, hashed_url) UNIQUE — 去重
- (user_id, hashed_given_url) — 短链接去重
- (user_id, created_at) — 列表排序
- (user_id, is_archived, archived_at) — 归档列表
- (user_id, is_starred, starred_at) — 收藏列表
- (domain_name, user_id) — 按域名筛选
- (user_id, language) — 按语言筛选

### tags

| Column | Type | Note |
|--------|------|------|
| id | UUID | PK |
| user_id | UUID | FK → users.id |
| label | VARCHAR(100) | |
| slug | VARCHAR(100) | |

**Constraint:** UNIQUE (user_id, slug)

### entry_tags

| Column | Type | Note |
|--------|------|------|
| entry_id | UUID | FK → entries.id |
| tag_id | UUID | FK → tags.id |

**Constraint:** PK (entry_id, tag_id)

### annotations

| Column | Type | Note |
|--------|------|------|
| id | UUID | PK |
| entry_id | UUID | FK → entries.id |
| user_id | UUID | FK → users.id |
| quote | TEXT | 选中的原文 |
| text | TEXT | 用户批注 |
| ranges | JSONB | W3C Web Annotation 定位信息 |
| is_orphaned | BOOL | default false, 内容编辑后定位失效时标记为 true |
| created_at | TIMESTAMPTZ | |
| updated_at | TIMESTAMPTZ | |

**注意**: 当用户编辑 entry content 后, 已有 annotation 的 DOM 定位可能失效。策略: 编辑保存时将该 entry 的所有 annotation 标记为 `is_orphaned=true`, 前端展示时降级为仅显示 quote 文本（不高亮定位）, 用户可手动重新选中文本来 re-anchor。

### memos

| Column | Type | Note |
|--------|------|------|
| id | UUID | PK |
| user_id | UUID | FK → users.id |
| content | TEXT | 文本/URL/关键词 |
| source_url | TEXT | 可选, 来源页面 |
| promoted_entry_id | UUID | FK → entries.id, nullable (非 null 表示已转为 entry) |
| created_at | TIMESTAMPTZ | |

### tagging_rules

| Column | Type | Note |
|--------|------|------|
| id | UUID | PK |
| user_id | UUID | FK → users.id |
| rule | JSONB | 结构化条件, 见下方规则引擎说明 |
| tags | TEXT[] | 匹配时自动添加的标签 |
| priority | INT | 执行顺序 |
| created_at | TIMESTAMPTZ | |

**规则引擎设计 (MVP)**:

MVP 阶段使用结构化 JSON 条件而非自由表达式, 避免实现表达式解析器。

规则格式:
```json
{
  "operator": "AND",
  "conditions": [
    {"field": "domainName", "op": "eq", "value": "github.com"},
    {"field": "readingTime", "op": "gt", "value": 5}
  ]
}
```

支持的字段: `title`, `url`, `domainName`, `language`, `readingTime`, `contentType`
支持的操作符: `eq`, `neq`, `contains`, `not_contains`, `gt`, `lt`, `matches` (正则)
组合逻辑: `AND` / `OR`

**嵌套扩展预留**: `conditions` 数组中的元素可以是条件对象, 也可以是嵌套的 `{operator, conditions}` 分组。MVP 前端 UI 只支持单层条件, 但 JSONB schema 和后端解析器从一开始就支持递归嵌套, 避免未来数据迁移。

后续可引入 `evalexpr` crate 支持自由表达式语法, 结构化 JSON 作为前端 UI 的中间表示。

### site_rules

| Column | Type | Note |
|--------|------|------|
| id | UUID | PK |
| user_id | UUID | FK → users.id, nullable (null = 全局规则) |
| domain | VARCHAR(255) | 目标域名 |
| content_selector | TEXT | CSS 选择器, 指定正文区域 |
| title_selector | TEXT | CSS 选择器, 指定标题 |
| strip_selectors | TEXT[] | 需要移除的元素选择器 |
| created_at | TIMESTAMPTZ | |

## Content Extraction

多层兜底策略。**前置要求**: 在实现其他任何功能之前, 必须先完成内容提取的 PoC 验证。

### 提取引擎选型

**首选方案: 纯 Rust 实现 (scraper crate)**

基于 Readability 算法（评分 + DOM 裁剪）用 `scraper` crate 实现纯 Rust 提取引擎。核心逻辑:
1. 解析 HTML DOM
2. 对每个节点计算内容评分（文本密度、段落长度、链接比率等）
3. 选取最高分节点作为正文区域
4. 清理非内容元素（nav, sidebar, footer, ads 等）

参考实现广泛存在（Python readability-lxml、Go go-readability），但考虑到 CJK 文本适配、边界处理、启发式规则等, 预估核心代码量 1500-2500 行。PoC 阶段设 **2 周时间盒**，超时则切换到备选方案。

**备选方案 (如果纯 Rust 提取质量不达标或 PoC 超时)**:

通过 `std::process::Command` 调用 Node.js 子进程运行 Readability.js。Docker 镜像中内置轻量 Node 环境 + Readability.js 脚本。不优雅但 100% 可行, 提取质量有保证。

**镜像体积影响**: 纯 Rust 方案镜像 ~50-80MB; 如果切换到 Node.js 备选方案, 需额外打包 Node alpine 运行时, 镜像预计 ~120-150MB。

**放弃 boa_engine 方案**: boa_engine 不提供 DOM API, 需要自行实现 DOM polyfill, 工作量等同于一个中等规模独立项目, 风险过高。

### L1: 纯 Rust Readability (默认)

流程: `reqwest 抓取 HTML` → `scraper 解析 DOM` → `评分算法提取正文` → `ammonia 清洗 HTML`

### L2: 站点特定规则

当 L1 提取失败或质量差时，查询 site_rules 表，用 CSS 选择器手动指定正文区域。

流程: `匹配 domain` → `scraper 解析 HTML` → `按 CSS 选择器提取`

用户可在设置中提交站点规则，指定某个域名的正文选择器。

### L3: 无头浏览器渲染 (后续)

针对 SPA/动态加载页面。初期不实现，标记为 future。

### L4: 手动编辑 (兜底)

抓取失败时保留原始 HTML 的简化版（strip scripts/styles），用户可手动编辑修正。

### 特殊内容适配

**GitHub 仓库** (`github.com/{owner}/{repo}`):
- 通过 GitHub API 获取 repo 信息 (description, stars, language, topics)
- 抓取 README 作为 content
- content_type 设为 "github_repo"
- metadata 存储结构化信息

未来可扩展: Twitter/X, YouTube, PDF 等。

### 抓取队列与速率限制

所有抓取请求（单篇保存、批量导入）统一进入内存抓取队列, 由后台 worker 消费:

- **全局并发上限**: 最多 10 个并发抓取 task (可通过环境变量 `LETTURA_FETCH_CONCURRENCY` 配置)
- **Per-domain 速率限制**: 同一域名每秒最多 1 个请求, 使用令牌桶算法
- **令牌桶管理**: 使用 LRU 缓存, 最多维护 500 个 domain 的令牌桶, 超出时淘汰最久未访问的
- **队列策略**: FIFO, 队列最大深度 5000, 超出时拒绝新请求并返回 429
- **批量导入**: 导入的 URL 逐条入队而非即时抓取, 提供 `GET /api/entries/queue` 查看队列状态 (pending/total)
- 可通过环境变量配置全局默认速率

### PoC 验证计划

在项目 Phase 1 首先完成内容提取 PoC:
1. 用 scraper crate 实现核心评分算法
2. 用 30+ 真实 URL 的 HTML 快照做测试 (覆盖新闻、博客、技术文章、中英文等)
3. 与 Readability.js 的提取结果做对比, 达到 80%+ 匹配度即为合格
4. 如果不达标, 切换到 Node.js 子进程方案

## API Design

认证方式: JWT (access token + refresh token), 无 OAuth2。

- Access token: 短期有效 (15 分钟), 无状态, 不存 DB
- Refresh token: 长期有效 (30 天), 存储在 DB 的 `refresh_tokens` 表中, 支持撤销 (用户改密码/主动登出时删除该用户所有 refresh token)

### Auth

注册策略: 首个注册用户自动成为 admin, 后续注册需 admin 审批或关闭开放注册。

```
POST   /api/auth/register     — 注册
POST   /api/auth/login        — 登录, 返回 JWT
POST   /api/auth/refresh      — 刷新 token
POST   /api/auth/logout       — 登出 (撤销当前 refresh token)
```

### Admin

需要 admin 权限的操作:

```
POST   /api/admin/reindex            — 全量重建搜索索引
GET    /api/admin/users              — 用户列表
PATCH  /api/admin/users/:id          — 管理用户 (启用/禁用/设为admin)
PATCH  /api/admin/settings           — 全局设置 (开放注册开关等)
POST   /api/admin/backup             — 触发数据库备份 (pg_dump), 返回下载链接
```

CLI 也提供备份命令: `lettura backup --output /path/to/backup.sql`

### Entries

```
GET    /api/entries            — 列表 (分页, 筛选: archived/starred/tag/domain/search), 不返回 content 字段
POST   /api/entries            — 保存新文章 (传入 URL, 进入抓取队列)
GET    /api/entries/:id        — 详情 (包含 content)
PATCH  /api/entries/:id        — 更新 (title, content, is_archived, is_starred 等)
DELETE /api/entries/:id        — 删除
POST   /api/entries/:id/refetch — 重新抓取内容 (仅 is_content_edited=false 时)
GET    /api/entries/:id/export  — 导出单篇
```

### Tags

```
GET    /api/tags               — 当前用户所有标签
POST   /api/entries/:id/tags   — 给文章添加标签
DELETE /api/entries/:id/tags/:tag_id — 移除标签
DELETE /api/tags/:id           — 删除标签 (同时移除关联)
```

### Annotations

```
GET    /api/entries/:id/annotations  — 文章的所有注释
POST   /api/entries/:id/annotations  — 创建注释
PATCH  /api/annotations/:id          — 更新注释
DELETE /api/annotations/:id          — 删除注释
```

### Memos

```
GET    /api/memos              — 列表
POST   /api/memos              — 快速捕获
DELETE /api/memos/:id          — 删除
POST   /api/memos/:id/promote  — 转为 entry (如果包含 URL 则抓取)
```

### Tagging Rules

```
GET    /api/tagging-rules      — 列表
POST   /api/tagging-rules      — 创建
PATCH  /api/tagging-rules/:id  — 更新
DELETE /api/tagging-rules/:id  — 删除
```

### Site Rules

```
GET    /api/site-rules         — 列表
POST   /api/site-rules         — 创建
PATCH  /api/site-rules/:id     — 更新
DELETE /api/site-rules/:id     — 删除
```

### Import/Export

```
POST   /api/import/wallabag    — 导入 wallabag JSON
POST   /api/import/browser     — 导入浏览器书签 HTML (Chrome/Firefox 通用格式)
GET    /api/export             — 导出全部 (JSON)
```

MVP 只支持 wallabag JSON 导入 + 浏览器书签导入。Pocket 等第三方导入需要 OAuth 集成, 复杂度高, 列为后续功能。

### RSS Feed

```
GET    /feed/:user_token/unread   — 未读文章 RSS
GET    /feed/:user_token/starred  — 收藏文章 RSS
GET    /feed/:user_token/archive  — 归档文章 RSS
GET    /feed/:user_token/tag/:slug — 按标签 RSS
```

RSS feed 使用 per-user token (非 JWT), 存储在 users 表的 feed_token 字段中。
- 可在设置页重新生成 (旧 token 立即失效)
- 无自动过期, 用户自行管理 (自托管场景下足够安全)
- RSS 响应设置 `Referrer-Policy: no-referrer` header, 防止 token 通过 Referer 泄露

## Frontend

React SPA, 响应式设计覆盖桌面和移动端。

### 核心页面

- **登录/注册页**
- **文章列表** — 未读/归档/收藏三个视图, 支持搜索、标签筛选、域名筛选
- **文章详情/阅读页** — 正文渲染 + Tiptap 编辑模式 + 注释/高亮侧边栏
- **Memo 收集箱** — 快速输入 + 列表 + 转化操作
- **标签管理**
- **设置页** — 账户信息、打标签规则、站点规则、导入/导出、RSS token

### 浏览器扩展

Chrome/Firefox 扩展, 功能:
- 一键保存当前页面为 Entry
- 快速创建 Memo (选中文本 + 右键)
- 弹窗显示保存状态

扩展调用 REST API, 用 JWT 认证:
- Access token 存储在 `chrome.storage.session` (浏览器关闭后清除)
- Refresh token 存储在 `chrome.storage.local` (持久化, 支持自动续期)
- 扩展首次使用时需要输入服务器地址 + 登录凭据

### 移动端适配

不做原生 App, SPA 做响应式:
- 底部导航栏 (文章/收集箱/搜索/设置)
- 手机端 Memo 输入优化 (打开直接聚焦输入框)
- 支持 PWA (可添加到主屏幕)
- 手机浏览器 Share Sheet 通过 PWA Web Share Target API 接收分享

## Error Handling

- 抓取失败: 保留 entry 记录, content 为空, 标记 extract_method="failed", 前端提示用户可手动编辑或重试
- 抓取超时: 默认 30 秒超时, 可在设置中调整
- 重复 URL: 返回已存在的 entry, 不创建新记录
- 无效 URL: 返回 400, 前端提示

## Full-Text Search

使用 tantivy 内嵌全文搜索引擎, 索引存储在 Docker volume (`/data/tantivy`)。

**索引一致性策略 (DB 为主, 索引为从):**
- 增量更新: entry 创建/更新/删除时, 先写 DB (事务提交后), 再异步更新 tantivy 索引
- 索引写入失败: 记录到 DB 的 `index_pending` 队列表 (entry_id + operation), 后台 task 定期重试
- 全量重建: 提供 CLI 命令 `lettura reindex` 和 API `POST /api/admin/reindex`, 从 DB 全量重建索引
- 启动校验: 应用启动时对比 DB entries 总数与索引文档数, 差异超过阈值时自动触发全量重建
- 容器重启: 索引通过 volume 持久化, 正常重启不丢失
- 索引损坏: 检测到索引不可读时自动触发全量重建, 记录日志告警

**索引字段:** title, content (strip HTML tags), url, domain_name, tags

## Testing Strategy

- **后端**: 单元测试 (提取逻辑、规则引擎) + 集成测试 (API 端到端, 用 testcontainers 启动 PG)
- **前端**: Vitest 组件测试 + Playwright E2E 测试
- **内容提取**: 维护一组真实 URL 的快照测试, 确保提取质量不退化

## Project Structure

```
lettura/
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
├── migrations/           — SQLx 数据库迁移
├── src/
│   ├── main.rs
│   ├── config.rs         — 环境变量配置
│   ├── db/               — 数据库层
│   │   ├── mod.rs
│   │   └── models/       — 数据模型 (User, Entry, Tag, Memo...)
│   ├── api/              — Axum handler 层
│   │   ├── mod.rs
│   │   ├── auth.rs
│   │   ├── entries.rs
│   │   ├── tags.rs
│   │   ├── annotations.rs
│   │   ├── memos.rs
│   │   ├── tagging_rules.rs
│   │   ├── site_rules.rs
│   │   ├── import.rs
│   │   ├── export.rs
│   │   └── feed.rs
│   ├── extract/          — 内容提取
│   │   ├── mod.rs
│   │   ├── readability.rs  — 纯 Rust Readability 评分算法
│   │   ├── site_rule.rs    — CSS 选择器提取
│   │   ├── github.rs       — GitHub 仓库适配
│   │   └── sanitize.rs     — HTML 清洗
│   ├── search/           — tantivy 全文搜索 (索引持久化到 /data/tantivy, 提供全量重建命令)
│   ├── rules/            — 自动打标签规则引擎
│   ├── tasks/            — 后台异步任务
│   └── auth/             — JWT 认证中间件
├── web/                  — React SPA
│   ├── package.json
│   ├── vite.config.ts
│   ├── src/
│   │   ├── pages/
│   │   ├── components/
│   │   ├── hooks/
│   │   ├── api/          — API client
│   │   └── store/
│   └── public/
├── extension/            — 浏览器扩展
│   ├── manifest.json
│   ├── popup/
│   └── background/
└── docs/
    └── specs/
```
