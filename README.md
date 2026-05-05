# Lettura

> 一个轻量级、自托管的 read-it-later 应用，受 wallabag 启发。Rust 后端 + React 前端，单容器部署。

## Features

- 保存网页文章，自动提取正文内容
- 全文搜索（tantivy 引擎）
- 标签管理和自动标签规则
- 文章高亮标注
- 快速收集（Memo）并可提升为文章
- 浏览器扩展一键保存
- Wallabag/浏览器书签导入
- RSS 订阅输出
- HTML 页面分享（支持密码保护、有效期、文件替换）
- 响应式 Web 界面 + 内容编辑器
- 管理员备份/恢复
- Prometheus 指标（可选）

## Quick Start

### Docker Compose (推荐)

1. 创建 `.env` 文件：
```bash
JWT_SECRET=your-secret-key-at-least-32-characters-long
```

2. 启动：
```bash
docker compose up -d
```

3. 访问 http://localhost:3330，注册第一个用户（自动成为管理员）

### 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `DATABASE_URL` | 必填 | PostgreSQL 连接字符串 |
| `JWT_SECRET` | 必填 | JWT 签名密钥（至少 32 字符） |
| `LISTEN_ADDR` | `0.0.0.0:3330` | HTTP 监听地址 |
| `STORAGE_TYPE` | `local` | 图片存储类型（local/oss） |
| `CORS_ORIGINS` | `*` | CORS 允许来源（逗号分隔或 *） |
| `METRICS_ENABLED` | `false` | 启用 Prometheus 指标 |
| `DB_MAX_CONNECTIONS` | `10` | 数据库最大连接数 |

更多配置见 `.env.example`。

## Browser Extension

支持 Chrome 和 Firefox（Manifest V3）。在 `extension/` 目录中加载为开发扩展，配置服务器地址后即可使用。

## API

所有 API 路径以 `/api/v1/` 为前缀，使用 JWT Bearer 认证。

详细 API 文档见 [docs/api.md](docs/api.md)。

## CLI (`lettura-cli`)

AI-first 命令行工具，通过 Personal Access Token 与 Lettura HTTP API 通信。

### 安装

```sh
curl -sSL https://raw.githubusercontent.com/congqiu/lettura/main/scripts/install-cli.sh | sh
```

或指定版本 / Fork：

```sh
LETTURA_CLI_VERSION=v0.1.0 LETTURA_REPO=my-org/lettura ./scripts/install-cli.sh
```

默认安装到 `~/.local/bin/lettura-cli`（可通过 `LETTURA_INSTALL_DIR` 覆盖）。

### 认证

1. 打开 Lettura Web UI → Settings → API Tokens → Generate token
2. 复制令牌（仅显示一次）
3. `lettura-cli login` — 输入服务器地址并粘贴令牌

### 常用命令

```sh
lettura-cli save https://example.com/post --tag rust
lettura-cli list --filter "tag:rust,since:7d" --limit 10
lettura-cli get <id> --format markdown
lettura-cli tag <id> ai research
lettura-cli tag --add ai --filter "domain:example.com,untagged" --dry-run
lettura-cli tag --add ai --filter "domain:example.com,untagged" --yes
```

完整命令参考可通过 `lettura-cli --help` 查看，或安装 AI skill：

```sh
lettura-cli skill install    # 将 skill 写入 ~/.claude/skills/lettura.md
```

### 配置

`~/.config/lettura/config.toml`（权限 0600）：

```toml
default_profile = "default"

[profiles.default]
url   = "https://lettura.example.com"
token = "lta_..."
```

通过 `--profile <name>`、`--url <url>` 或环境变量 `LETTURA_PROFILE`、`LETTURA_URL`、`LETTURA_TOKEN` 可覆盖配置。

## Development

```bash
# 后端编译/测试（Docker）
docker compose build lettura
docker compose up -d postgres lettura
docker compose exec lettura cargo test --workspace

# 前端
cd web && pnpm install && pnpm run dev

# 生产镜像构建
DOCKER_BUILDKIT=1 docker compose build lettura
```

## Tech Stack

**后端:** Rust, Axum, SQLx, PostgreSQL, tantivy, scraper, ammonia
**前端:** React 19, TypeScript, Vite, Tailwind CSS, Tiptap, TanStack Query

## License

[MIT](LICENSE)
