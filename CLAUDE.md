# Lettura - AI Agent Context

## 项目简介

Lettura（意大利语"阅读"）是一个受 wallabag 启发的 read-it-later 应用。Rust 后端 + React SPA 前端，容器优先部署。

## 关键文档

- 设计规格: `docs/specs/2026-03-28-lettura-design.md` — **必读**，包含所有数据模型、API、架构决策
- 实施计划: `docs/plans/` — 包含 Plan 1 至 Plan 4b 的所有实施计划
- 优化设计: `docs/specs/2026-03-29-optimization-design.md` — Plan 4 优化设计规格
- 站点配置设计: `docs/specs/2026-04-18-site-config-design.md` — 站点配置系统设计规格
- PoC 评估: `docs/poc-evaluation.md` — 内容提取 PoC 评估结果
- 参考项目: `/home/cc/workspace/github/wallabag` — 原始 PHP 项目，可参考其设计但不要复制代码

## 当前状态

Plan 1（内容提取 PoC）、Plan 2a（DB + Auth）、Plan 2b（Entry CRUD + 抓取队列）、Plan 3a（Tags, Annotations, Memos）、Plan 3b（全文搜索）、Plan 5（浏览器扩展 + Docker 部署）均已完成。Plan 4a（关键优化）已完成，Plan 4b（改进优化）执行中。

## 实施计划路线图

| 计划 | 内容 | 状态 |
|------|------|------|
| Plan 1 | 项目脚手架 + 内容提取 PoC | ✅ 已完成 |
| Plan 2a | 核心后端 — DB + Auth | ✅ 已完成 |
| Plan 2b | 核心后端 — Entry CRUD + 抓取队列 | ✅ 已完成 |
| Plan 3a | 高级功能 — Tags, Annotations, Memos | ✅ 已完成 |
| Plan 3b | 高级功能 — 全文搜索 | ✅ 已完成 |
| Plan 4a | 关键优化（安全、稳定性、运维） | ✅ 已完成 |
| Plan 4b | 改进优化（性能、体验、可维护性） | 🔄 执行中 |
| Plan 5 | 浏览器扩展 + Docker 部署 | ✅ 已完成 |

## 技术栈

**后端:** Rust 2024, Axum, SQLx, PostgreSQL, tantivy, scraper, ammonia, reqwest, argon2, jsonwebtoken
**前端:** React 19 (TypeScript), Vite, Tiptap, TanStack Query, Zustand, Tailwind CSS

## 关键设计决策（不可随意更改）

1. **单体架构** — 一个 Rust 二进制 + 内嵌 SPA 静态文件，不要拆微服务
2. **PostgreSQL only** — 不支持 SQLite，不做多数据库兼容
3. **JWT 认证** — 无 OAuth2，access token (15min) + refresh token (30 days in DB)
4. **内容提取优先** — 这是产品核心，必须先验证再建其他功能
5. **UUID 主键** — 所有表用 UUID，不用自增 ID
6. **不做 i18n** — 界面不做多语言
7. **不做 2FA** — 不做两步验证
8. **前后端分离但单容器** — SPA 编译后用 rust-embed 嵌入二进制

## 编码规范

- Rust: 每个模块严格 TDD（先写测试 → 确认失败 → 实现 → 确认通过 → 提交）
- 每个 commit 应该是一个原子性的、可编译的变更
- 测试命令: `cargo test`
- 不要跳过 Plan 中的 TDD 步骤，即使看起来很简单
- 代码注释用英文，文档用中文

## 编译和运行

项目使用 Docker Compose 进行编译和运行，本地不需要安装 Rust 工具链。

### 开发脚本 (`dev.sh`)

```bash
./dev.sh build    # 重新构建镜像并启动（默认命令）
./dev.sh up       # 启动服务（不重新构建）
./dev.sh down     # 停止并移除容器
./dev.sh restart  # 重启应用容器（不重新构建）
./dev.sh logs     # 查看所有服务日志
./dev.sh status   # 查看容器状态
./dev.sh psql     # 打开 PostgreSQL 命令行
./dev.sh clean    # 清理容器、镜像和卷
```

### 直接使用 Docker Compose

```bash
docker compose build lettura     # 编译后端
docker compose up -d             # 启动所有服务
docker compose logs -f lettura   # 查看应用日志
```

### 配置

在项目根目录创建 `.env` 文件：

```
JWT_SECRET=your-secret-at-least-32-characters-long
```

### 可选环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `LETTURA_USER_AGENT` | Chrome UA | HTTP 请求 User-Agent |
| `LETTURA_FETCH_TIMEOUT` | 30 | 抓取超时（秒） |
| `LETTURA_FETCH_MAX_RETRIES` | 3 | 抓取失败最大重试次数 |
| `LETTURA_PROXY` | 无 | HTTP/SOCKS5 代理地址 |
| `LETTURA_RENDERING_URL` | 无 | JS 渲染服务地址（如 `http://browserless:3000`） |
| `LETTURA_SITE_CONFIGS_PATH` | 无 | 用户自定义站点配置文件目录 |

### 站点配置系统

内置配置文件在 `site-configs/` 目录下（编译时嵌入二进制），格式为 FTR 简洁文本：

```
# site-configs/example.com.txt
render: true
title: h1.article-title
body: div.content, article
strip: div.ads, div.sidebar
author: span.author
match: /article/
exclude: /video/
```

用户可通过 `site-configs-local/` 目录添加本地覆盖配置（通过 docker-compose volume 挂载到 `/data/site-configs`）。

规则优先级：本地覆盖文件 → 内置配置库 → 数据库 site_rules → readability 自动提取

### 服务端口

- 应用: `http://localhost:3001`
- PostgreSQL: `localhost:5432`（用户名/密码: lettura/lettura）

## 不要做的事

- 不要引入 boa_engine（JS 引擎），已明确放弃此方案
- 不要添加 OAuth2 / i18n / 2FA
- 不要把前端拆成独立容器
- 不要用 SQLite
- 不要修改已通过评审的设计文档中的核心架构决策，除非有充分理由并记录
