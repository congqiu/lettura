# Lettura API Reference

所有 API 端点以 `/api/v1/` 为前缀（除 Health 和 RSS Feed 外）。需认证的端点使用 JWT Bearer token，在 `Authorization: Bearer <token>` 头中传递。

旧路径 `/api/{path}` 会被 301 重定向到 `/api/v1/{path}`。

---

## Auth

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| POST | `/api/v1/auth/register` | 否 | 注册新用户（第一个用户自动成为管理员）。Body: `{ username, email, password }` |
| POST | `/api/v1/auth/login` | 否 | 登录，返回 access_token + refresh_token。Body: `{ email, password }` |
| POST | `/api/v1/auth/refresh` | 否 | 刷新 access token。Body: `{ refresh_token }` |
| POST | `/api/v1/auth/logout` | 是 | 登出，撤销 refresh token |
| POST | `/api/v1/auth/regenerate-feed-token` | 是 | 重新生成 RSS feed token |

> 注册和登录端点有严格速率限制（10 次/分钟），防止暴力破解。

**响应格式（登录/注册/刷新）：**
```json
{
  "access_token": "eyJ...",
  "refresh_token": "uuid-string",
  "token_type": "Bearer",
  "expires_in": 900
}
```

---

## Entries

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| GET | `/api/v1/entries` | 是 | 获取文章列表 |
| POST | `/api/v1/entries` | 是 | 创建文章（提交 URL 后异步抓取）。Body: `{ url }` |
| GET | `/api/v1/entries/{id}` | 是 | 获取单篇文章详情 |
| PATCH | `/api/v1/entries/{id}` | 是 | 更新文章。Body: `{ title?, content?, is_archived?, is_starred? }` |
| DELETE | `/api/v1/entries/{id}` | 是 | 软删除文章 |
| POST | `/api/v1/entries/{id}/refetch` | 是 | 重新抓取文章内容 |
| POST | `/api/v1/entries/{id}/restore` | 是 | 恢复已删除的文章 |
| DELETE | `/api/v1/entries/{id}/permanent` | 是 | 永久删除文章 |

**列表查询参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `page` | int | 页码（默认 1） |
| `per_page` | int | 每页数量（默认 20，最大 100） |
| `is_archived` | bool | 筛选已归档 |
| `is_starred` | bool | 筛选已收藏 |
| `domain` | string | 按域名筛选 |
| `search` | string | 全文搜索（tantivy） |
| `deleted` | bool | 设为 true 查看已删除文章 |

---

## Tags

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| GET | `/api/v1/tags` | 是 | 获取当前用户所有标签 |
| POST | `/api/v1/entries/{id}/tags` | 是 | 为文章添加标签。Body: `{ label }` |
| DELETE | `/api/v1/entries/{entry_id}/tags/{tag_id}` | 是 | 移除文章的某个标签 |
| DELETE | `/api/v1/tags/{id}` | 是 | 删除标签 |

---

## Annotations

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| GET | `/api/v1/entries/{id}/annotations` | 是 | 获取文章的所有标注 |
| POST | `/api/v1/entries/{id}/annotations` | 是 | 创建标注。Body: `{ quote, text?, ranges }` |
| PATCH | `/api/v1/annotations/{id}` | 是 | 更新标注。Body: `{ text?, ranges? }` |
| DELETE | `/api/v1/annotations/{id}` | 是 | 删除标注 |

---

## Memos

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| GET | `/api/v1/memos` | 是 | 获取所有 Memo |
| POST | `/api/v1/memos` | 是 | 创建 Memo。Body: `{ content, source_url? }` |
| DELETE | `/api/v1/memos/{id}` | 是 | 删除 Memo |
| POST | `/api/v1/memos/{id}/promote` | 是 | 将 Memo 提升为文章（若有 source_url 则抓取） |

---

## Tagging Rules

自动标签规则：当新文章匹配条件时自动打标签。

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| GET | `/api/v1/tagging-rules` | 是 | 获取所有规则 |
| POST | `/api/v1/tagging-rules` | 是 | 创建规则 |
| PATCH | `/api/v1/tagging-rules/{id}` | 是 | 更新规则 |
| DELETE | `/api/v1/tagging-rules/{id}` | 是 | 删除规则 |

---

## Site Rules

自定义站点抓取规则（CSS 选择器等）。

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| GET | `/api/v1/site-rules` | 是 | 获取所有站点规则 |
| POST | `/api/v1/site-rules` | 是 | 创建站点规则 |
| PATCH | `/api/v1/site-rules/{id}` | 是 | 更新站点规则 |
| DELETE | `/api/v1/site-rules/{id}` | 是 | 删除站点规则 |

---

## Import / Export

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| POST | `/api/v1/import/wallabag` | 是 | 导入 Wallabag JSON 导出文件。Body: Wallabag 条目数组 |
| POST | `/api/v1/import/browser` | 是 | 导入浏览器书签 HTML 文件。Body: `{ html }` |
| GET | `/api/v1/export` | 是 | 导出当前用户所有数据（JSON） |

---

## Admin

需要管理员权限（`is_admin = true`）。

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| GET | `/api/v1/admin/users` | 是 (admin) | 获取所有用户列表 |
| POST | `/api/v1/admin/reindex` | 是 (admin) | 重建全文搜索索引 |
| GET | `/api/v1/admin/backup` | 是 (admin) | 下载完整数据库备份（JSON） |
| POST | `/api/v1/admin/restore` | 是 (admin) | 从备份 JSON 恢复数据 |

---

## RSS Feed

RSS feed 通过用户专属 token 访问，无需 JWT 认证。

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| GET | `/feed/{user_token}/unread` | token | 未读文章 RSS feed（最近 50 条） |
| GET | `/feed/{user_token}/starred` | token | 收藏文章 RSS feed（最近 50 条） |
| GET | `/feed/{user_token}/archive` | token | 归档文章 RSS feed（最近 50 条） |

> 使用 `POST /api/v1/auth/regenerate-feed-token` 重新生成 feed token。

---

## Health

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| GET | `/api/health` | 否 | 健康检查（数据库 + 搜索引擎状态） |

**响应：**
```json
{
  "status": "healthy",
  "db": "ok",
  "search": "ok (42 docs)"
}
```

---

## 通用错误格式

所有错误返回统一 JSON 格式：

```json
{
  "error": "error message here"
}
```

常见 HTTP 状态码：

| 状态码 | 说明 |
|--------|------|
| 400 | 请求参数错误 |
| 401 | 未认证或 token 过期 |
| 403 | 权限不足 |
| 404 | 资源不存在 |
| 409 | 资源冲突（如 URL 已存在） |
| 429 | 请求过于频繁 |
| 500 | 服务器内部错误 |

## 速率限制

- 全局限制：100 次/分钟
- 认证端点（登录/注册）：10 次/分钟
