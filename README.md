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

## Development

```bash
# 后端
cargo test

# 前端
cd web && pnpm install && pnpm run dev

# Docker 构建
docker build -t lettura .
```

## Tech Stack

**后端:** Rust, Axum, SQLx, PostgreSQL, tantivy, scraper, ammonia
**前端:** React 19, TypeScript, Vite, Tailwind CSS, Tiptap, TanStack Query

## License

[MIT](LICENSE)
