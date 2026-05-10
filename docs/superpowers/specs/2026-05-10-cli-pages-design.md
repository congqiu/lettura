# CLI Pages 命令设计规格

## 概述

为 lettura-cli 添加 `pages` 嵌套子命令，支持页面的发布、列表、更新、删除、恢复和分享操作。upload + create 合并为 `publish` 命令，支持本地文件、目录和 URL 三种输入方式。

## 命令结构

```
lettura pages <subcommand>

子命令：
  publish <path|url>  发布页面（自动 upload + create）
  list                列出页面
  update <id>         更新页面
  delete <id>         删除页面
  restore <id>        恢复已删除页面
  share <id>          获取分享链接
```

## 子命令详细设计

### `pages publish`

```
lettura pages publish <path|url> [options]
```

**参数：**

| 参数 | 必填 | 说明 |
|------|------|------|
| `<path\|url>` | 是 | 本地文件/目录路径，或远程 URL |

**选项：**

| 选项 | 说明 |
|------|------|
| `--title <string>` | 页面标题（默认：从 HTML `<title>` 提取，或文件名） |
| `--entry-file <string>` | 入口 HTML 文件（默认：index.html） |
| `--password <string>` | 设置访问密码 |
| `--expires-at <datetime>` | 过期时间（RFC 3339，如 2026-12-31T23:59:59Z） |

**输入类型自动检测：**

- **本地文件**（.html/.zip）：直接上传
- **本地目录**：递归打包为临时 ZIP 后上传
- **URL**（http/https）：先 HTTP 抓取内容，保存为临时 HTML 文件，再上传

**流程：**

```
输入检测
  ├─ URL → HTTP 抓取 → 保存为临时 HTML → 走文件上传流程
  ├─ 目录 → 递归打包为临时 ZIP → 走文件上传流程
  └─ 文件（.html/.zip）→ 直接上传

上传流程：
  1. POST /api/v1/pages/upload (multipart/form-data, field: files)
     ← 返回 upload_id, html_files, default_entry, suggested_title
  2. POST /api/v1/pages (JSON)
     body: { upload_id, entry_file, title, description, password, expires_at }
     ← 返回 id, slug, title, url, has_password, created_at
  3. 输出结果
```

### `pages list`

```
lettura pages list [options]
```

**选项：**

| 选项 | 说明 |
|------|------|
| `--status <active\|disabled\|deleted\|expired\|all>` | 状态过滤（默认：active） |
| `--limit <number>` | 每页数量（默认：20） |
| `--output <json\|ids\|human>` | 输出格式（默认：json） |

**human 格式显示：** slug | title | status | share-url | created_at

### `pages update`

```
lettura pages update <id> [options]
```

**选项：**

| 选项 | 说明 |
|------|------|
| `--title <string>` | 修改标题 |
| `--description <string>` | 修改描述 |
| `--password <string>` | 设置/修改密码（空字符串清除密码） |
| `--status <active\|disabled>` | 修改状态 |
| `--expires-at <datetime>` | 修改过期时间（"none" 清除过期） |
| `--files <path\|url>` | 替换文件（自动 upload + 更新） |
| `--entry-file <string>` | 修改入口文件（需配合 --files） |

**`--files` 流程：**

1. 同 publish 的上传流程，获取新 upload_id
2. PATCH /api/v1/pages/{id}，body 包含 upload_id + entry_file + 其他元数据

### `pages delete`

```
lettura pages delete <id>
```

软删除页面。

### `pages restore`

```
lettura pages restore <id>
```

恢复已删除页面。

### `pages share`

```
lettura pages share <id>
```

获取分享 URL。

## 输出格式

| 命令 | JSON 输出 | human 输出 |
|------|-----------|------------|
| publish | 创建响应 JSON | `Published: {title} → {url}` |
| list | `{ items, total, page, limit }` | 表格：slug \| title \| status \| share-url \| created_at |
| update | 完整 page JSON | `Updated: {title}` |
| delete | `{ success: true }` | `Deleted: {id}` |
| restore | `{ success: true }` | `Restored: {id}` |
| share | `{ url, has_password }` | `{url}` |

## ApiClient 扩展

现有 `ApiClient` 只有 `get`/`post`/`delete`/`http_patch`，需新增：

- `upload_files(path: &Path) -> Result<UploadResponse>` — 发送 multipart/form-data 请求
- 使用 `reqwest::multipart` 构建表单

## 错误处理

复用现有 `CliError` 体系，新增变体：

- `UploadFailed(String)` — 上传失败（文件过大、格式错误等）

## 文件变更

### 新增文件

| 文件 | 说明 |
|------|------|
| `cli/src/commands/pages.rs` | pages 命令实现 |

### 修改文件

| 文件 | 变更 |
|------|------|
| `cli/src/cli.rs` | Command 枚举新增 `Pages { cmd: PagesCmd }`，新增 `PagesCmd` 和各子命令 Args 结构体 |
| `cli/src/commands/mod.rs` | 新增 `pub mod pages;` |
| `cli/src/main.rs` | match 分支新增 `Pages` 处理 |
| `cli/src/client.rs` | 新增 `upload_files` 方法 |
| `cli/src/error.rs` | CliError 新增 `UploadFailed` 变体 |
| `skills/lettura.md` | 新增 pages 命令文档和 cheatsheet 条目 |

## 测试策略

### 单元测试（`cli/src/commands/pages.rs` 内）

- 输入类型检测逻辑（URL vs 目录 vs 文件）
- 目录打包为 ZIP 的逻辑

### 契约测试（`tests/cli_contract.rs`）

- `pages publish <html文件>` → 验证返回 slug
- `pages list` → 验证包含已发布页面
- `pages update <id> --title "new"` → 验证标题更新
- `pages share <id>` → 验证返回 URL
- `pages delete <id>` → 验证删除成功
- `pages restore <id>` → 验证恢复成功

### Skill lint（`cli/tests/skill_lint.rs`）

- 新增 pages 命令示例的 clap 校验

## 不做的事

- 不做 pages 的 `get` 单条查询命令（API 没有单独的 GET /pages/{id} 端点）
- 不做目录打包时的排除规则（如 .gitignore）——YAGNI
- 不做 URL 抓取时的 JS 渲染——复用基础 HTTP 抓取即可
