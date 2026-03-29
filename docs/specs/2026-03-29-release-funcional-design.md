# Spec B: 功能补全（CORS + 备份/恢复 + 日志）

> 日期: 2026-03-29
> 状态: 已批准
> 范围: CORS 配置、管理员备份/恢复 API、业务日志增强

## 概述

补全发布前的 3 项功能缺口，确保浏览器扩展可跨域访问、管理员可备份/恢复数据、生产环境有足够的日志可观测性。

---

## B1. CORS 配置

**问题**: 浏览器扩展向 Lettura 服务发起跨域请求时会被浏览器拦截。

**变更**:
- `src/config.rs` 新增 `cors_origins: String`（环境变量 `CORS_ORIGINS`，默认 `*`）
- `src/api/mod.rs` 在路由上添加 `tower_http::cors::CorsLayer`：
  - origins: 从配置读取，`*` 表示 `Any`，否则按逗号分割为具体 origins
  - methods: GET, POST, PATCH, DELETE, OPTIONS
  - headers: Authorization, Content-Type
  - 放在 security headers layer 旁边
- `tests/common/mod.rs` Config 添加 `cors_origins` 字段
- `.env.example` 添加 `CORS_ORIGINS` 说明

**文件**: `src/config.rs`, `src/api/mod.rs`, `tests/common/mod.rs`, `.env.example`

---

## B2. 管理员备份/恢复 API

**问题**: 无数据备份手段，用户只能手动 pg_dump。

### 备份 API

`GET /api/v1/admin/backup` — 需 admin 权限

导出所有表数据为 JSON，格式：
```json
{
  "version": "1.0",
  "created_at": "2026-03-29T12:00:00Z",
  "users": [{ "id": "...", "username": "...", "email": "...", "is_admin": true, "feed_token": "...", "created_at": "...", "updated_at": "..." }],
  "entries": [{ ... }],
  "tags": [{ ... }],
  "entry_tags": [{ "entry_id": "...", "tag_id": "..." }],
  "annotations": [{ ... }],
  "memos": [{ ... }],
  "tagging_rules": [{ ... }],
  "site_rules": [{ ... }]
}
```

注意：
- 不导出 `password_hash`（安全）
- 不导出 `refresh_tokens`（会话数据）
- 包含软删除的 entries（`deleted_at IS NOT NULL`），备份应完整
- 返回 `Content-Disposition: attachment; filename="lettura-backup-YYYY-MM-DD.json"`

### 恢复 API

`POST /api/v1/admin/restore` — 需 admin 权限

接受备份 JSON body，在单个数据库事务中：
1. 验证 JSON 格式和 `version` 字段
2. 要求 query parameter `confirm=true`，否则返回 400
3. 清空所有业务表（CASCADE）
4. 按依赖顺序插入：users → entries → tags → entry_tags → annotations → memos → tagging_rules → site_rules
5. 重建 tantivy 搜索索引
6. 返回各表恢复的记录数

错误处理：事务失败时自动回滚，返回 500 + 错误信息。

**文件**:
- Create: `src/api/backup.rs`
- Modify: `src/api/mod.rs`（注册路由 + 模块）

---

## B3. 日志增强

**问题**: 业务代码中几乎没有日志，生产环境难以排查问题。

**变更**: 在以下位置添加 tracing 日志（不改变任何业务逻辑）：

### info! 级别（业务关键事件）
| 位置 | 日志内容 |
|------|---------|
| `api/auth.rs` register | `tracing::info!(user_id = %new_user.id, "user registered")` |
| `api/auth.rs` login | `tracing::info!(user_id = %found.id, "user logged in")` |
| `api/entries.rs` create_entry | `tracing::info!(entry_id = %new_entry.id, url = %req.url, "entry created")` |
| `api/entries.rs` delete_entry | `tracing::info!(entry_id = %entry_id, "entry soft-deleted")` |
| `api/entries.rs` restore_entry | `tracing::info!(entry_id = %entry_id, "entry restored")` |
| `api/entries.rs` permanently_delete | `tracing::info!(entry_id = %entry_id, "entry permanently deleted")` |
| `api/import.rs` import_wallabag | `tracing::info!(count = imported, "wallabag import completed")` |
| `api/import.rs` import_browser | `tracing::info!(count = imported, "browser import completed")` |
| `api/backup.rs` backup | `tracing::info!("admin backup created")` |
| `api/backup.rs` restore | `tracing::info!("admin restore completed")` |

### warn! 级别（异常但可恢复）
| 位置 | 日志内容 |
|------|---------|
| `rate_limit.rs` | `tracing::warn!("rate limit exceeded")` |
| `tasks/fetcher.rs` 提取失败 | `tracing::warn!(entry_id = %job.entry_id, "content extraction failed")` |
| `tasks/fetcher.rs` HTTP 错误 | `tracing::warn!(entry_id = %job.entry_id, status = %status, "fetch HTTP error")` |

### debug! 级别（开发调试）
| 位置 | 日志内容 |
|------|---------|
| `tasks/fetcher.rs` 开始 | `tracing::debug!(entry_id = %job.entry_id, url = %job.url, "fetch job started")` |
| `tasks/fetcher.rs` 完成 | `tracing::debug!(entry_id = %job.entry_id, "fetch job completed")` |
| `tasks/fetcher.rs` 限速等待 | `tracing::debug!(domain = %domain, "rate limiting domain")` |

**文件**: `src/api/auth.rs`, `src/api/entries.rs`, `src/api/import.rs`, `src/api/backup.rs`, `src/rate_limit.rs`, `src/tasks/fetcher.rs`

---

## 依赖关系

```
B1 (CORS) ─── 无依赖
B2 (备份) ─── 无依赖
B3 (日志) ─── 依赖 B2（backup.rs 中的日志）
```

B1 和 B2 可并行，B3 最后做。

## 测试策略

- B1: 集成测试验证 CORS 响应头
- B2: 集成测试验证备份导出格式 + 恢复后数据一致性
- B3: 无需测试（日志不改变行为）
- 最终: `cargo test` 全量通过
