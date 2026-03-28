# Lettura 项目全面优化设计

> 日期: 2026-03-29
> 状态: 待审核
> 范围: 后端、前端、数据库、安全、运维、开发者体验

## 概述

基于对当前项目（Plan 1-3b 已完成）的全面架构审查，识别出 20 项优化机会。按优先级分为两个 Plan 分批实施：

- **Plan A（P0+P1）**: 8 项关键优化 — 安全、性能、稳定性
- **Plan B（P2+P3）**: 12 项改进优化 — 架构演进、可观测性、开发者体验

两个 Plan 之间跑完整测试确认无回归，再继续。

## 不在范围内

以下不在本次优化中：

- 浏览器扩展 TypeScript 迁移（独立重构，工作量大）
- 浏览器扩展离线队列（依赖 TypeScript 迁移）
- CORS 配置（SPA 内嵌同源，Vite proxy 处理开发模式，无实际需求）
- CSP / HSTS 响应头（依赖用户部署环境，无法通用配置）
- 数据库审计日志（当前单用户场景不需要）

---

## Plan A: P0+P1 关键优化（8 项）

### A1. 更新 CLAUDE.md 项目状态 [P0, DevX]

**问题**: CLAUDE.md 写的是 "Plan 1 阶段，尚未开始编码"，与实际严重不符，误导 AI agent。

**变更**:
- 更新"当前状态"段落，反映 Plan 1-3b 均已完成
- 更新路线图表格中各 Plan 的状态列
- 标注下一步是 Plan 4（前端优化）或本优化 Plan

**文件**: `CLAUDE.md`

---

### A2. JWT Secret 启动校验 [P0, 安全]

**问题**: 如果 JWT_SECRET 太短或使用 docker-compose 默认值，系统会静默运行，存在安全隐患。

**变更**:
- `src/config.rs` 中 `Config` 构建时加入校验逻辑
- 校验规则：
  - 长度 >= 32 字符，否则 panic 并提示
  - 值等于 `change-me-in-production` 时 panic 并提示
- 使用 `panic!` 而非 `tracing::warn`（安全问题不应该只是警告）

**文件**: `src/config.rs`

---

### A3. 安全响应头 [P1, 安全]

**问题**: HTTP 响应没有安全头，浏览器无法启用内置安全保护。

**变更**:
- 在 Axum 路由上添加全局中间件层
- 使用 `tower-http::set_header::SetResponseHeaderLayer` 或在自定义 middleware 中设置
- 添加的头：
  - `X-Content-Type-Options: nosniff`
  - `X-Frame-Options: DENY`
  - `X-XSS-Protection: 1; mode=block`
  - `Referrer-Policy: strict-origin-when-cross-origin`
- 不添加 CSP 和 HSTS（依赖部署环境）

**依赖**: 检查 `tower-http` 已有 feature 是否覆盖，可能需要加 feature flag

**文件**: `src/main.rs`（路由构建处）

---

### A4. Token 刷新竞态修复 [P1, 前端]

**问题**: 并发请求同时收到 401 时，会触发多次 refresh token 请求，可能导致 token 被多次轮换使第一次之后的全部失败。

**变更**:
- `web/src/api/client.ts` 中引入模块级变量 `let refreshPromise: Promise<string> | null = null`
- 401 拦截器逻辑改为：
  1. 如果 `refreshPromise` 不为 null，await 它并用新 token 重试
  2. 如果为 null，创建 refresh Promise 赋值给 `refreshPromise`
  3. refresh 完成后（成功或失败）将 `refreshPromise` 重置为 null
  4. 成功：所有等待的请求用新 token 重试
  5. 失败：所有等待的请求统一跳转 login

**文件**: `web/src/api/client.ts`

---

### A5. Entry 列表查询优化 [P1, 性能]

**问题**: `list_entries` SQL 用 `SELECT *` 返回完整 content 和 text_content，列表页不需要这些大字段。

**变更**:
- 后端:
  - `src/models/entry.rs` 新增 `EntrySummary` 结构体，只含列表所需字段：`id, url, title, domain_name, reading_time, language, published_by, preview_picture, is_archived, is_starred, extract_method, http_status, created_at, archived_at, starred_at`
  - `list_entries()` SQL 改为显式 SELECT 这些字段
  - `src/api/entries.rs` 的列表 handler 返回类型改为 `Vec<EntrySummary>`
- 前端: 无需变更（`EntryCard` 已只用摘要字段）
- 详情 API `get_entry` 保持返回完整 `Entry`

**文件**: `src/models/entry.rs`, `src/api/entries.rs`

---

### A6. 前端 ErrorBoundary [P1, 前端]

**问题**: 任何组件的渲染错误会导致整个应用白屏，没有恢复手段。

**变更**:
- 新建 `web/src/components/ErrorBoundary.tsx`
  - React class component（getDerivedStateFromError + componentDidCatch）
  - 渲染：错误信息 + "重新加载页面" 按钮（调用 `window.location.reload()`）
  - 支持 dark mode 样式
- `App.tsx` 中：
  - 在 `<BrowserRouter>` 外层包裹顶级 ErrorBoundary（捕获路由级崩溃）
  - 在 `<Layout>` 的 `<Outlet />` 外层包裹页面级 ErrorBoundary（隔离单页面崩溃，不影响导航）

**文件**: `web/src/components/ErrorBoundary.tsx`（新建）, `web/src/App.tsx`, `web/src/components/Layout.tsx`

---

### A7. 健康检查端点 [P1, 运维]

**问题**: 没有专门的健康检查 API，docker-compose healthcheck 无法可靠判断服务状态。

**变更**:
- 新建 `src/api/health.rs`
  - `GET /api/health` — 无需认证
  - 检查项：
    - DB: 执行 `SELECT 1` 验证连接
    - Search: 尝试获取 tantivy reader（`index.reader()` 成功即正常）
  - 响应格式：`{"status": "ok"|"degraded"|"error", "db": "ok"|"error: ...", "search": "ok"|"error: ..."}`
  - DB 或 search 任一失败返回 503，全部正常返回 200
- `docker-compose.yml` 的 healthcheck 改为 `curl -f http://localhost:3000/api/health`
- 路由注册在认证中间件之外

**文件**: `src/api/health.rs`（新建）, `src/api/mod.rs`, `src/main.rs`, `docker-compose.yml`

---

### A8. RSS Feed Token 轮换 [P1, 安全]

**问题**: feed_token 一旦生成无法更换，如果泄露只能注销账号。

**变更**:
- `src/models/user.rs` 新增 `regenerate_feed_token(pool, user_id)` 函数
  - 生成新的 32 字节随机 hex token
  - `UPDATE users SET feed_token = $1 WHERE id = $2 RETURNING feed_token`
- `src/api/auth.rs` 新增 handler: `POST /api/auth/regenerate-feed-token`
  - 需要认证
  - 调用 model 函数，返回 `{"feed_token": "new_token"}`
- 路由注册在认证路由组内

**文件**: `src/models/user.rs`, `src/api/auth.rs`, `src/api/mod.rs`

---

## Plan B: P2+P3 改进优化（12 项）

> Plan B 在 Plan A 完成并通过全部测试后开始。

### B1. API 版本前缀 [P2, 后端]

**问题**: API 路由无版本号，未来 breaking change 没有退路。

**变更**:
- 所有 `/api/*` 路由迁移到 `/api/v1/*`
- 例外：`/api/health` 不加版本（运维端点）
- `/feed/*` 路径不变（非 REST API）
- `/storage/*` 路径不变（静态资源）
- 前端 `client.ts` baseURL 从 `/api` 改为 `/api/v1`
- 浏览器扩展中所有 API 路径加 `/v1` 前缀
- Vite proxy 规则无需改动（`/api` 前缀不变）

**文件**: `src/main.rs`（或 `src/api/mod.rs`，视路由组织方式）, `web/src/api/client.ts`, `extension/background.js`, `extension/popup.js`

---

### B2. 请求验证集中化 [P2, 后端]

**问题**: 验证逻辑散落在各 handler 中，缺少统一的错误格式。

**变更**:
- 新增依赖：`validator` (Cargo.toml，使用最新稳定版，需启用 `derive` feature)
- 在请求 DTO 结构体上 `#[derive(Validate)]`:
  - `RegisterRequest`: email 格式、password 长度 >= 8、username 非空
  - `CreateEntryRequest`: url 格式校验
  - `CreateAnnotationRequest`: quote 非空
  - `CreateMemoRequest`: content 非空
  - 等其他需要验证的 DTO
- 新建 `src/api/validate.rs`：实现 `ValidatedJson<T>` Axum extractor
  - 先反序列化 JSON，再调用 `validate()`
  - 验证失败返回 400: `{"error": "validation", "fields": {"field_name": ["error message"]}}`
- handler 中将 `Json<T>` 替换为 `ValidatedJson<T>`，移除手动验证代码

**文件**: `Cargo.toml`, `src/api/validate.rs`（新建）, 各 API handler 文件

---

### B3. 用户级 API 限流 [P2, 后端]

**问题**: 没有 API 层面限流，单用户可能大量请求导致资源耗尽。

**变更**:
- 新增依赖：`tower-governor` (Cargo.toml，使用最新稳定版)
- 全局限流：100 req/min per IP
- 认证路由组（register/login）单独限流：10 req/min per IP（防暴力破解）
- 内存存储（单实例部署够用）
- 超限返回 429 + `Retry-After` 头
- 应用为 Axum layer

**文件**: `Cargo.toml`, `src/main.rs`

---

### B4. DB 连接池可配置 [P2, 后端]

**问题**: 连接池参数写死 10 个连接，无法根据部署环境调整。

**变更**:
- `src/config.rs` 新增字段：
  - `db_max_connections: u32`（默认 10，环境变量 `DB_MAX_CONNECTIONS`）
  - `db_min_connections: u32`（默认 2，环境变量 `DB_MIN_CONNECTIONS`）
  - `db_acquire_timeout_secs: u64`（默认 30，环境变量 `DB_ACQUIRE_TIMEOUT`）
- `src/db.rs` 使用这些配置：
  ```
  PgPoolOptions::new()
      .max_connections(config.db_max_connections)
      .min_connections(config.db_min_connections)
      .acquire_timeout(Duration::from_secs(config.db_acquire_timeout_secs))
  ```

**文件**: `src/config.rs`, `src/db.rs`

---

### B5. 错误处理增强 [P2, 后端]

**问题**: SQLx 错误全部映射为 Internal(500)，丢失有用上下文。

**变更**:
- `src/api/error.rs` 中 `From<sqlx::Error>` 实现改进：
  - 匹配 `sqlx::Error::Database(e)` 并检查 `e.constraint()`
  - 已知约束映射表：
    - `users_email_key` → `Conflict("email already exists")`
    - `users_username_key` → `Conflict("username already exists")`
    - `entries_user_id_hashed_url_key` → `Conflict("URL already saved")`
    - 其他约束 → `Conflict("duplicate record")`
  - 非约束 DB 错误仍为 `Internal`，但用 `tracing::error!` 记录完整错误链
- 给高频 handler 添加 `#[tracing::instrument(skip(state), err)]`：
  - `create_entry`, `register`, `login`, `list_entries`
  - 自动记录函数参数和返回的错误

**文件**: `src/api/error.rs`, 各 API handler 文件

---

### B6. 前端代码分割 [P2, 前端]

**问题**: 所有页面打包在单个 bundle 中，首屏加载不必要的代码。

**变更**:
- `App.tsx` 中对低频页面用 `React.lazy()`:
  - `const SettingsPage = lazy(() => import('./pages/SettingsPage'))`
  - `const MemosPage = lazy(() => import('./pages/MemosPage'))`
  - `const EntryDetailPage = lazy(() => import('./pages/EntryDetailPage'))`
- 在路由外层包裹 `<Suspense fallback={<div className="p-8 text-center">Loading...</div>}>`
- 保持同步加载：`EntryListPage`, `LoginPage`, `RegisterPage`（首屏核心路径）

**文件**: `web/src/App.tsx`

---

### B7. 前端测试框架搭建 [P2, 前端]

**问题**: 前端零测试覆盖，没有配置测试框架。

**变更**:
- 安装依赖：`vitest`, `@testing-library/react`, `@testing-library/jest-dom`, `@testing-library/user-event`, `jsdom`
- `vite.config.ts` 加 test 配置：`test: { environment: 'jsdom', globals: true, setupFiles: './src/test-setup.ts' }`
- 新建 `web/src/test-setup.ts`：导入 `@testing-library/jest-dom`
- `package.json` 加 `"test": "vitest run"`, `"test:watch": "vitest"`
- 编写 3 个基础测试：
  - `web/src/components/__tests__/ProtectedRoute.test.tsx` — 未认证时渲染 Navigate 到 /login
  - `web/src/components/__tests__/EntryCard.test.tsx` — 渲染标题、域名、阅读时间
  - `web/src/api/__tests__/client.test.ts` — refresh lock 去重验证

**文件**: `vite.config.ts`, `package.json`, 新建测试文件

---

### B8. 离线/网络状态提示 [P2, 前端]

**问题**: 用户断网时没有任何提示，操作静默失败。

**变更**:
- 新建 `web/src/components/NetworkStatus.tsx`
  - 监听 `window` 的 `online`/`offline` 事件
  - 离线时渲染顶部红色提示条："网络连接已断开，部分功能可能不可用"
  - 恢复时自动隐藏（加 1 秒延迟显示"已恢复"绿色提示后消失）
  - 使用 fixed 定位，不影响页面布局
- `Layout.tsx` 中在最顶层添加 `<NetworkStatus />`

**文件**: `web/src/components/NetworkStatus.tsx`（新建）, `web/src/components/Layout.tsx`

---

### B9. 软删除机制 [P3, DB]

**问题**: Entry 删除是硬删除，误删无法恢复。

**变更**:
- 新增 migration `009_soft_delete.sql`：
  - `ALTER TABLE entries ADD COLUMN deleted_at TIMESTAMPTZ`
  - `CREATE INDEX idx_entries_deleted ON entries (deleted_at) WHERE deleted_at IS NOT NULL`
- `src/models/entry.rs`:
  - `list_entries()` 加 `WHERE deleted_at IS NULL`（默认不显示已删除）
  - `delete_entry()` 改为 `UPDATE entries SET deleted_at = now()`
  - 新增 `list_deleted_entries(pool, user_id)` — 回收站列表
  - 新增 `restore_entry(pool, entry_id, user_id)` — `SET deleted_at = NULL`
  - 新增 `permanently_delete_entry(pool, entry_id, user_id)` — 真正 DELETE
- `src/api/entries.rs`:
  - `DELETE /api/v1/entries/{id}` — 软删除
  - `GET /api/v1/entries?deleted=true` — 查看回收站
  - `POST /api/v1/entries/{id}/restore` — 恢复
  - `DELETE /api/v1/entries/{id}/permanent` — 永久删除
- tantivy 索引：软删除时从索引中删除，恢复时重新索引
- 不实现自动过期清理（YAGNI，用户手动永久删除即可）

**文件**: `migrations/009_soft_delete.sql`（新建）, `src/models/entry.rs`, `src/api/entries.rs`

---

### B10. JSONB GIN 索引 [P3, DB]

**问题**: `metadata` 和 `conditions` JSONB 字段无索引，将来按 metadata 查询会全表扫描。

**变更**:
- 新增 migration `010_gin_indexes.sql`：
  ```sql
  CREATE INDEX idx_entries_metadata ON entries USING GIN (metadata);
  CREATE INDEX idx_tagging_rules_conditions ON tagging_rules USING GIN (conditions);
  ```
- 纯 DDL 变更，不影响应用代码

**文件**: `migrations/010_gin_indexes.sql`（新建）

---

### B11. 复合索引优化 [P3, DB]

**问题**: 高频列表查询缺少复合索引，查询计划可能使用不到最优索引。

**变更**:
- 新增 migration `011_composite_indexes.sql`：
  ```sql
  -- 未读列表（最高频查询）
  CREATE INDEX idx_entries_user_unread ON entries (user_id, created_at DESC)
      WHERE deleted_at IS NULL AND is_archived = false;

  -- 归档列表
  CREATE INDEX idx_entries_user_archived ON entries (user_id, archived_at DESC)
      WHERE deleted_at IS NULL AND is_archived = true;

  -- 收藏列表
  CREATE INDEX idx_entries_user_starred ON entries (user_id, starred_at DESC)
      WHERE deleted_at IS NULL AND is_starred = true;
  ```
- 使用 partial index（WHERE 条件）减少索引大小
- 依赖 B9（deleted_at 字段），必须在 B9 之后

**文件**: `migrations/011_composite_indexes.sql`（新建）

---

### B12. 可观测性 Prometheus Metrics [P3, 后端]

**问题**: 没有生产环境指标采集，无法监控和告警。

**变更**:
- 新增依赖：`metrics`, `metrics-exporter-prometheus` (Cargo.toml，使用最新稳定版)
- `src/main.rs`:
  - 初始化 Prometheus recorder
  - 注册 `GET /metrics` 端点（无需认证，建议生产环境网络限制）
- 新建 `src/middleware/metrics.rs`（或内联在 main.rs）:
  - Axum middleware layer，记录每个请求的：
    - `http_requests_total{method, path, status}` — counter
    - `http_request_duration_seconds{method, path}` — histogram
- `src/tasks/fetcher.rs`:
  - `fetch_queue_depth` — gauge（队列当前深度）
- `src/search.rs`:
  - `search_index_documents` — gauge（索引文档数，reindex 时更新）
- 路径标准化：将 UUID 参数替换为 `{id}` 防止高基数

**文件**: `Cargo.toml`, `src/main.rs`, `src/tasks/fetcher.rs`, `src/search.rs`

---

## 依赖关系

```
Plan A（全部独立，可并行开发）:
  A1 ─── 无依赖
  A2 ─── 无依赖
  A3 ─── 无依赖
  A4 ─── 无依赖
  A5 ─── 无依赖
  A6 ─── 无依赖
  A7 ─── 无依赖
  A8 ─── 无依赖

Plan B:
  B1 ─── 依赖 Plan A 完成（路由结构变更）
  B2 ─── 依赖 B1（路由已迁移后再改 handler）
  B3 ─── 依赖 B1
  B4 ─── 无依赖
  B5 ─── 无依赖
  B6 ─── 无依赖
  B7 ─── 无依赖
  B8 ─── 无依赖
  B9 ─── 依赖 B1（API 路径已迁移）
  B10 ── 无依赖
  B11 ── 依赖 B9（需要 deleted_at 字段）
  B12 ── 无依赖
```

## 测试策略

- Plan A 完成后：运行 `cargo test` 全量 + 前端手动验证
- Plan B 每个 migration 后：运行 `cargo test` + `EXPLAIN ANALYZE` 验证索引生效
- B7 完成后：运行 `npm test` 验证前端测试基础设施
- 最终：全量 `cargo test` + `npm test` + Docker build 验证

## 风险点

| 风险 | 影响 | 缓解 |
|------|------|------|
| B1 API 版本迁移遗漏路径 | 前端/扩展 404 | 全文搜索 `/api/` 确认无遗漏 |
| B9 软删除影响现有查询 | 已删除数据仍出现在列表 | 所有 entry 查询加 `WHERE deleted_at IS NULL` |
| B2 validator 与现有 handler 签名冲突 | 编译错误 | 逐个 handler 替换，每次替换后 `cargo check` |
| B12 metrics 高基数路径 | Prometheus 内存膨胀 | UUID 参数标准化为 `{id}` |
