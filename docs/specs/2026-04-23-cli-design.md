# Lettura CLI（AI-first）设计规格

- 作者: qiu
- 日期: 2026-04-23
- 状态: 草案

## 背景

Lettura 当前只有 Web UI / 浏览器扩展 / RSS feed 三种前端。随着 AI agent（Claude Code、Cursor、Claude Desktop 等）普及，出现三个典型使用场景，Web UI 都不合适：

1. **AI 获取某个收藏页面的 markdown**：AI 把用户已收藏的文章拉到自己的 context 里继续处理（总结、翻译、问答）
2. **AI 整理收藏内容**：AI 帮用户对未分类的文章自动打标签、归档
3. **AI 自动收藏**：AI 在工作流中遇到有价值的链接时主动收藏

本设计新增一个 **shell CLI（`lettura-cli`）+ 一份 skill 文件**，面向 AI agent 作为一等公民消费。人类使用者不是目标；普通的阅读、整理仍用 Web UI 完成。

## 目标 & 非目标

### 目标

- AI agent 通过简单、结构化、幂等的命令完成上述三场景
- CLI 与 Lettura server 解耦，走 HTTP API，支持远程拓扑（server 在 NAS/VPS，CLI 在用户笔记本）
- 凭证长期可用、可撤销、可审计（Personal Access Token）
- Skill 文件是 AI 的唯一真相来源（命令、filter DSL、输出 schema、错误码、安全规则），且版本能跟 server 对齐

### 非目标

- 不做 MCP server（可以以后在此 CLI 之上加一层）
- 不做人类友好的交互式 TUI
- 不为 AI 开放 `delete`（风险过高；删除仍由 Web UI 手工执行）
- 不做 annotation / memo / import / export 的 CLI 入口（v1 范围外）
- 不做细粒度 scope（只有 `read` / `write` 两级）
- 不发 crates.io（大多数用户无 Rust 工具链）

## 架构总览

```
┌──────────────────────┐       HTTP + Bearer PAT        ┌──────────────────┐
│ AI agent host        │ ─────────────────────────────> │ Lettura server   │
│ (user laptop / cloud)│                                │ (Docker)         │
│                      │                                │                  │
│  ┌────────────────┐  │                                │  ┌────────────┐  │
│  │ lettura-cli    │  │                                │  │ axum API   │  │
│  │ (Rust binary)  │  │                                │  │  + PAT mw  │  │
│  └───────┬────────┘  │                                │  └──────┬─────┘  │
│          │           │                                │         │        │
│  ┌───────▼────────┐  │                                │  ┌──────▼─────┐  │
│  │ config.toml    │  │                                │  │ PostgreSQL │  │
│  │ (~/.config/    │  │                                │  └────────────┘  │
│  │  lettura/)     │  │                                │                  │
│  └────────────────┘  │                                │                  │
│                      │                                │                  │
│  ┌────────────────┐  │                                │                  │
│  │ skill file     │  │                                │                  │
│  │ (.claude/      │  │                                │                  │
│  │  skills/)      │  │                                │                  │
│  └────────────────┘  │                                │                  │
└──────────────────────┘                                └──────────────────┘
```

核心原则：
- **CLI 是纯 HTTP 客户端**，不直连数据库、不与容器绑定
- **Skill 约束 AI 行为**，CLI 只负责把 API 包成 shell 友好形态
- **API 是权威**，CLI 侧的响应类型重新声明（取字段子集），通过集成测试钉住 wire contract

## Personal Access Token (PAT)

### 数据模型

新增表 `personal_access_tokens`：

```sql
CREATE TABLE personal_access_tokens (
  id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,                     -- 用户命名，如 "Claude on laptop"
  token_hash   TEXT NOT NULL UNIQUE,              -- SHA256(token) hex
  token_prefix TEXT NOT NULL,                     -- 前 12 字节明文，UI / 日志识别
  scope        TEXT NOT NULL CHECK (scope IN ('read', 'write')),
  last_used_at TIMESTAMPTZ,
  expires_at   TIMESTAMPTZ,                       -- NULL = 永不过期（默认）
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_pat_user ON personal_access_tokens(user_id);
CREATE INDEX idx_pat_hash ON personal_access_tokens(token_hash);
```

Scope 语义：
- `read`：允许 list / get / search 类 API
- `write`：允许所有 API（含 read）

细粒度 scope 不做（YAGNI）。

### Token 格式

`lta_<40 chars base62>`

- `lta_` 前缀让日志、grep、截图里能一眼识别出这是 Lettura 的 token（借鉴 GitHub `ghp_`、Slack `xoxp-`）
- 随机熵 40 × log2(62) ≈ 238 bit，足以抗暴力
- `token_prefix` 存前 12 字节（包含 `lta_` 前缀），用于 UI / 日志展示：`lta_abc12345...`
- 完整 token 仅 SHA256 hash 存库，服务端任何时刻都不可逆

### 认证中间件

新增 `PatAuthLayer`，与现有 `JwtAuthLayer` 并列。

请求头 `Authorization: Bearer lta_...`：

1. 识别 `lta_` 前缀 → 走 PAT 分支（`Bearer ey...` 走 JWT 分支）
2. SHA256 hash → 查 `personal_access_tokens`
3. 校验 `expires_at` 未过期
4. 按请求 method + path 校验 scope（写请求要 `write`）
5. 通过：注入 `AuthenticatedUser { id, scope: Pat }`；失败：返回 401 / 403
6. 异步更新 `last_used_at`（60s 去重，避免每次请求都写库）

### 管理 API

所有 token 管理端点 **只接受 JWT 鉴权**，禁止用 PAT 管 PAT（防权限放大循环）。

```
POST   /api/v1/tokens
  body: { name: string, scope: "read" | "write", expires_in_days?: number }
  201:  { id, name, scope, token: "lta_..." }   -- 仅此一次返回明文
  
GET    /api/v1/tokens
  200:  [{ id, name, prefix, scope, last_used_at, expires_at, created_at }]

DELETE /api/v1/tokens/:id
  204
```

### Web UI

设置页新增 "API Tokens" tab：

- 现有 token 表格：name / prefix（`lta_abc12345…`）/ scope / last used / expires / 删除按钮
- "Generate token" 按钮 → 弹窗：name 输入框 + scope 下拉 + expiration 下拉（30d / 90d / 1y / **Never**，默认 Never）
- 创建成功后在同一弹窗内用高亮块展示明文 token + Copy 按钮 + 醒目警告 "This is the only time you'll see this token"
- 空状态：展示 `lettura-cli login` 快速指引

## CLI

### 项目布局

现有 repo 升级为 **Cargo workspace**，成员：

- `.`（根）→ 保留，继续是 server crate `lettura`
- `cli/` → 新成员 crate `lettura-cli`

`lettura-cli` 依赖最小化：`clap`、`reqwest`、`serde`、`serde_json`、`tokio`、`directories`（查 config 路径）、`toml`。**不**依赖 server crate，响应类型在 CLI 侧独立声明，取字段子集。

一个 `tests/cli_contract.rs` 集成测试：启动 server 的 in-memory 配置 + 真实 CLI 打一轮，断言 wire contract，防止 server-CLI 两侧字段漂移。

### 命令表面

```
# 认证 / 配置
lettura-cli login                                  # 交互：输 URL、粘贴 token、写 config
lettura-cli whoami                                 # 当前 profile + 用户名 + scope
lettura-cli config get <key>
lettura-cli config set <key> <value>
lettura-cli config list

# 读取
lettura-cli list   [--filter EXPR] [--limit N] [--output json|ids] [--fields f1,f2]
lettura-cli search <query> [--limit N] [--output json|ids]
lettura-cli get    <id> [--format markdown|json|html|text]    # 默认 markdown

# 写入
lettura-cli save   <url> [--title X] [--tag a,b] [--wait]
lettura-cli tag    <id> <name>... | --add a,b --filter EXPR [--dry-run|--yes]
lettura-cli untag  <id> <name>... | --remove a,b --filter EXPR [--dry-run|--yes]
lettura-cli archive <id> | --filter EXPR [--dry-run|--yes]
lettura-cli star    <id> | --filter EXPR [--dry-run|--yes]

# 杂项
lettura-cli tags                                   # 列全部 tag
lettura-cli skill print                            # 把 bundled skill 打到 stdout
lettura-cli skill install [--path PATH]            # 写 skill 到 ~/.claude/skills/lettura.md（可改路径）
```

保留的全局 flag：`--profile`、`--url`、`--token`、`--output`（json / ids / human）、`--quiet`、`--pretty`。

### Filter DSL

统一用于 `list` 和所有支持批量的命令。语法：**逗号分隔的 AND 条件**，不支持 OR 和括号。

| Key | 示例 | 含义 |
|-----|------|------|
| `tag:<name>` | `tag:golang` | 带指定 tag |
| `!tag:<name>` | `!tag:archive` | 不带指定 tag |
| `untagged` | `untagged` | 无任何 tag |
| `domain:<host>` | `domain:medium.com` | 按域名 |
| `since:<rel|abs>` | `since:7d` / `since:2026-01-01` | 自某时间起 |
| `older-than:<rel>` | `older-than:90d` | 比相对时间更老 |
| `starred` / `!starred` | | 是否加星 |
| `archived` / `!archived` | | 是否归档（默认排除归档） |
| `unread` / `read` | | 阅读状态 |
| `search:<query>` | `search:rust async` | 嵌套全文搜索（走 tantivy） |

组合示例：`"domain:medium.com,untagged,since:7d,!archived"`

相对时间单位：`h`（小时）/ `d`（天）/ `w`（周）。绝对时间 ISO-8601。

### 输出格式

默认 JSON（或 JSON Lines，多条记录时），所有时间字段 RFC3339 UTC。

`list` 单条示例：
```json
{
  "id": "e8a3…",
  "url": "https://example.com/post",
  "title": "…",
  "domain": "example.com",
  "tags": ["golang", "async"],
  "starred": false,
  "archived": false,
  "unread": true,
  "saved_at": "2026-04-20T10:00:00Z",
  "reading_time_minutes": 8,
  "word_count": 1840
}
```

`get --format markdown` 输出：
```markdown
---
id: e8a3…
url: https://example.com/post
title: …
tags: [golang, async]
saved_at: 2026-04-20T10:00:00Z
---

<content_markdown 原文>
```

content_markdown 取 DB 中已存字段；若缺失（旧数据）则用 `content_html` 现场转一次 markdown。

`--output ids`：一行一个 UUID，方便 `xargs` / shell loop。
`--fields id,url,title`：只输出指定字段，给 AI 省 token。

### 错误格式 & 退出码

失败时 stderr 输出 JSON：
```json
{"error": {"code": "not_found", "message": "Entry not found: abc-123", "hint": "Use `lettura-cli list` to find entry ids"}}
```

| Exit | Code | 含义 |
|------|------|------|
| 0 | — | 成功 |
| 2 | `not_found` | 资源不存在 |
| 3 | `unauthorized` / `forbidden` | 认证失败 / scope 不足 |
| 4 | `bad_args` | CLI 参数错或 server 400 |
| 5 | `server_error` | 5xx |
| 6 | `rate_limited` | 429（`hint` 带 retry 建议） |
| 7 | `conflict` | 其他业务冲突 |

`hint` 字段给 AI 自恢复用——比如 rate limit 命中时写明 retry 时间。

### 批量安全阀

任何带 `--filter` 的写类命令（`tag --filter` / `archive --filter` / `star --filter`）必须显式指定 `--dry-run` 或 `--yes` 其一，否则命令退出码 4 报错。Skill 明确要求 AI "先 dry-run → 检查 matched 数量 → 再 --yes"。

- `--dry-run`：返回 `{"matched": 42, "would_update": 42, "ids": [...]}`，不改 DB
- `--yes`：执行，返回 `{"matched": 42, "updated": 42, "failed": [], "ids": [...]}`
- `--max N`（可选）：超出 N 条时 server 端直接 422 拒绝，避免误扫全库

`list` 默认 `--limit 20`，避免 AI 上下文被一次性冲垮；skill 指引 AI 必要时显式加大或分页。

### `save` 行为

- **默认异步**：`lettura-cli save <url>` 立即返回 `{id, already_existed: false, status: "queued"}`，entry 进入抓取队列
- **`--wait`**：阻塞最长 `LETTURA_FETCH_TIMEOUT`（默认 30s），返回 `{id, status: "ready"|"failed", title, tags, reading_time_minutes}` 等完整字段。失败时 stderr 写标准 error JSON，退出码 5
- **超时**：`--wait` 超时退出码 5、`hint` 提示可以稍后 `lettura-cli get <id>` 查状态

### 幂等性

- **`save <url>`**：同一用户的同一 URL 再 save → 定位已有 entry → **tag 集合取并集**（已有 + 本次传入）→ 返回 `{id, already_existed: true, tags: [...]}`。这种行为让 AI 反复跑"整理"流程不会丢失已有 tag，也不会创建重复记录
- **`tag <id> <name>`**：已存在不报错，直接 200
- **`untag <id> <name>`**：不存在也不报错，200
- **`archive` / `star`**：重复操作等幂等

### Config 文件

`~/.config/lettura/config.toml`（遵循 XDG），权限 0600：

```toml
default_profile = "default"

[profiles.default]
url   = "https://lettura.example.com"
token = "lta_..."

[profiles.work]
url   = "https://lettura-work.internal"
token = "lta_..."
```

优先级（高 → 低）：
1. 命令行 `--url` / `--token` / `--profile`
2. 环境变量 `LETTURA_URL` / `LETTURA_TOKEN` / `LETTURA_PROFILE`
3. `default_profile` 指向的 profile 字段
4. 无：报 `bad_args`，提示 `lettura-cli login`

`lettura-cli login` 流程：
1. 提示 server URL（默认从现有 profile 读）
2. 检测 `$SERVER/api/health` 可达
3. 提示浏览器打开 `$SERVER/settings/tokens` 生成 token
4. 粘贴 token → 打一次 `GET /api/v1/auth/me`（或 `/whoami`）验证
5. 成功写入 config（首次建文件并设 0600）

## Skill

### 分发

同一份源文件 `skills/lettura.md` 走两条渠道：

1. **CLI 自带**：`lettura-cli skill install` 把 embedded skill 写到 `~/.claude/skills/lettura.md`。解决离线 bootstrap 和版本与 CLI 绑定
2. **Server 动态下发**：`GET /skills/lettura.md`（无需 auth）由 server 渲染，把实例专属信息注入模板（base URL、server 版本、支持的 filter key、当前用户 tag 库提示样例）。用户 `curl $SERVER/skills/lettura.md > ~/.claude/skills/lettura.md` 即可与 server 对齐

两者用 `<!-- lettura-skill-version: X.Y.Z -->` 注释标注版本，AI 在怀疑行为不一致时可自检。

### 内容结构

```markdown
# Lettura CLI Skill

## 何时触发
关键词：收藏、稍后读、wallabag、"帮我保存"、"整理我收藏的"、"上周存的那个"

## 认证
lettura-cli login → 按提示生成 PAT、粘贴。
lettura-cli whoami 验证。

## 三大任务 Cookbook

### 1. 读取某篇已收藏文章的 markdown
（完整命令序列示例）

### 2. 整理未分类收藏
（list --filter untagged → get 逐条 → tag；强调安全阀）

### 3. 保存新链接
（save 用法；多 tag；--wait 使用时机）

## 命令速查

## Filter DSL 参考

## JSON 输出 Schema

## 安全规则
- 批量写操作必须先 --dry-run 确认 matched 数量
- list 默认 limit 20，超过请显式指定
- 不做 delete
- 用户让 AI "整理"时，先跑 dry-run 把拟议打 tag 方案呈现给用户确认

## 错误码 + 自恢复

## 常见陷阱
- URL 重复 save 不会报错（并集更新 tag），利用此特性放心重跑
```

Server 渲染版本会额外注入：
- `Base URL: <server_url>`
- `Server version: <version>`
- 用户 tag 库摘要（前 20 个高频 tag，帮 AI 复用既有分类体系）

## 测试策略

- **server 端**：`tests/pat_auth.rs`、`tests/tokens_api.rs` —— PAT 鉴权中间件、scope 校验、token CRUD
- **CLI 端**：`cli/tests/cli.rs` —— 基于 `assert_cmd` 起一个 mock HTTP server，每个命令打全 happy / 错误 path
- **契约测试**：`tests/cli_contract.rs` —— 启真实 server（测试 DB）+ 真实 CLI 二进制，打完整场景（save → list → tag → get markdown），断言 schema。CI 必跑
- **skill lint**：`tests/skill_lint.rs` —— 验证 `skills/lettura.md` 中示例命令都能 parse（用 clap 的 try_parse），防止 skill 和命令漂移

## 分发

- **CI**（github actions matrix）产物 `lettura-cli-<version>-<target>.tar.gz`：
  - `x86_64-unknown-linux-gnu`
  - `aarch64-unknown-linux-gnu`
  - `x86_64-apple-darwin`
  - `aarch64-apple-darwin`
- `scripts/install.sh`：`curl -sSL https://raw.githubusercontent.com/.../install.sh | sh`，检测 arch，装到 `~/.local/bin/lettura-cli`
- Homebrew / Scoop：有需求再加
- 不发 crates.io

## 开放问题 / 后续

- **MCP server**：在此 CLI 之上加 `lettura-cli mcp` 子命令启动 MCP server，复用 HTTP 客户端。等 MCP 生态稳定再做
- **细粒度 scope**：如果有"只给某标签的读权限"需求再加
- **审计日志**：PAT 的调用历史暂不记录，只维护 `last_used_at`。真有审计诉求再加单独表
- **Token rotation**：v1 不做自动轮换；用户手动创建新 token → 更新 config → 删除老 token
- **速率限制**：复用现有 governor，对 PAT 设独立 bucket（比 JWT 宽松，因为 AI 流量特征不同）。v1 直接复用现有策略，上线后视情况调

## 里程碑

此设计交付一个独立的实施计划，由 `writing-plans` 拆分。核心增量：

1. PAT 表 + 迁移 + 鉴权中间件 + 管理 API
2. Web UI tokens 设置页
3. Workspace 拆分 + `lettura-cli` crate 骨架 + config / login / whoami
4. 读取类命令（list / search / get）+ filter DSL 解析
5. 写入类命令（save / tag / untag / archive / star）+ 幂等 + 批量安全阀
6. Skill 源文件 + CLI `skill install` + server `/skills/lettura.md` endpoint
7. CI 发布流水线 + install.sh
8. 契约测试 + skill lint
