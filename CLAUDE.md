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

Plan 1（内容提取 PoC）、Plan 2a（DB + Auth）、Plan 2b（Entry CRUD + 抓取队列）、Plan 3a（Tags, Annotations, Memos）、Plan 3b（全文搜索）、Plan 4a（关键优化）、Plan 4b（改进优化）、Plan 5（浏览器扩展 + Docker 部署）均已完成。

## 实施计划路线图

| 计划 | 内容 | 状态 |
|------|------|------|
| Plan 1 | 项目脚手架 + 内容提取 PoC | ✅ 已完成 |
| Plan 2a | 核心后端 — DB + Auth | ✅ 已完成 |
| Plan 2b | 核心后端 — Entry CRUD + 抓取队列 | ✅ 已完成 |
| Plan 3a | 高级功能 — Tags, Annotations, Memos | ✅ 已完成 |
| Plan 3b | 高级功能 — 全文搜索 | ✅ 已完成 |
| Plan 4a | 关键优化（安全、稳定性、运维） | ✅ 已完成 |
| Plan 4b | 改进优化（性能、体验、可维护性） | ✅ 已完成 |
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
- 后端编译/测试一律在 Docker 中执行：优先用 `docker compose exec lettura cargo test ...`，未启动服务时用 `docker compose run --rm lettura cargo test ...`
- 不要跳过 Plan 中的 TDD 步骤，即使看起来很简单
- 代码注释用英文，文档用中文

## 问题修复原则

- **追根溯源**：修复 bug 时不要只解决表面症状，必须定位问题产生的根本原因并从源头修复。例如：搜索无结果时不应该只加一个 fallback，还要找到索引数据缺失的原因并修复数据写入路径。
- **完整排查**：当一个修复不够时，继续深挖。表面问题可能由多个独立原因共同导致（如分页失效 = 后端逻辑错误 + CORS 头未暴露），每个原因都需要逐一修复。
- **验证闭环**：修复后要在实际环境中验证，确认症状消失且根因已消除，而不是仅凭代码推理判断修复成功。

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

### 单元测试

```bash
# 运行全部单元测试（利用 BuildKit 缓存，增量编译很快）
docker build --target test -t lettura-test .

# 只运行特定模块的测试（更快）
docker build --target test --build-arg "TEST_ARGS=--lib search" -t lettura-test .

# 集成测试（使用主数据库）
docker compose exec lettura cargo test --test '*'
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
| `LETTURA_SITE_CONFIGS_PATH` | `/data/site-configs` | 用户站点规则 YAML 目录 |
| `LETTURA_RENDERING_ENABLED` | `auto` | 渲染兜底开关：`auto` / `true` / `false` |
| `LETTURA_CHROMIUM_PATH` | 自动搜索 PATH | Chromium 可执行文件绝对路径 |
| `LETTURA_RENDER_CONCURRENCY` | 2 | 并发渲染上限 |
| `LETTURA_RENDER_TIMEOUT_MS` | 15000 | 单次渲染总超时（毫秒） |
| `LETTURA_FETCH_CONCURRENCY` | 5 | 抓取 worker 并发数 |
| `LETTURA_FETCH_MAX_ATTEMPTS` | 5 | 单 job 最大重试（超过进死信） |
| `LETTURA_FETCH_LEASE_SECS` | 300 | job 租约初始秒数 |
| `LETTURA_FETCH_DEAD_TTL_DAYS` | 30 | 死信保留天数 |

### 站点配置系统

详见 `docs/specs/2026-04-23-fetch-pipeline-redesign.md`。

规则文件放 `site-configs-local/<domain>.yaml`（docker-compose 会把该目录挂到容器内 `/data/site-configs`）。YAML 字段：`match` / `exclude`（URL path 正则）、`rewrite`（path 重写）、`request.headers` / `request.cookies` / `request.user_agent`、`response.type: html|json` + `response.html|json` 提取规则、`render.mode: never|auto|force` + `wait_for` / `timeout_ms`。

规则优先级：本地 YAML → 数据库 `site_rules` → readability 自动提取。

### 构建选项

| 构建命令 | 说明 | 镜像大小 |
|----------|------|----------|
| `docker compose build lettura` | 完整版，带 chromiumoxide + chromium | ~350MB |
| `RENDERING=0 docker compose build lettura` | 精简版，不含 Chromium | ~100MB |

### 服务端口

- 应用: `http://localhost:3330`
- PostgreSQL: `localhost:5436`（用户名/密码: lettura/lettura）

## 抓取队列

抓取任务存在 PostgreSQL `fetch_jobs` 表（持久化队列），进程崩溃零丢失、多副本可水平扩展。

- worker 用 `SELECT FOR UPDATE SKIP LOCKED` 抢占任务，租约 5 分钟，崩溃后其他 worker 自动接管
- 失败按 60s → 2min → 4min → 8min 指数 backoff，最多 5 次后进 dead 死信
- admin endpoint:
  - `GET /api/v1/admin/fetch-jobs?status=failed|dead|pending|running&limit=N` — 查看队列状态
  - `POST /api/v1/admin/fetch-jobs/{id}/retry` — 单条重试
  - `POST /api/v1/admin/fetch-jobs/retry-all-dead` — 一次复活最多 100 条死信
- **PAT 不支持** admin endpoints（middleware 固定 `is_admin=false`），CLI 复活死信请人工通过 web UI
- 详见 `docs/specs/2026-05-16-fetch-queue-persistence.md`

### 回退

如需回到 mpsc 实现（不推荐 — mpsc 本身有"重启丢任务"bug），revert 引入这个 feature 的 merge commit（或参考对应 PR 列出的 commit 范围），重新 build + redeploy：

```bash
git revert -m 1 <merge-commit-hash>
docker compose build lettura
docker compose up -d
```

`fetch_jobs` 表保留，下次再上线时未消费的 job 继续被处理（schema 完全向下兼容）。

## API Codegen

后端用 `utoipa` 生成 OpenAPI 3.1 schema，前端用 `openapi-typescript` 把 schema 编译成 `web/src/api/schema.ts`。新写或改 handler 时按以下流程同步：

1. handler 上加 `#[utoipa::path(get, path = "...", responses(...), tag = "...")]`
2. handler 用到的 request/response 类型加 `#[derive(utoipa::ToSchema)]`
3. 在 `src/api/openapi.rs::ApiDoc` 的 `paths(...)` 列出 handler、`components(schemas(...))` 列出新类型
4. 跑 `./dev.sh codegen` 重新生成 `web/src/api/openapi.json` + `web/src/api/schema.ts`
5. commit 这两个生成文件（schema.ts 进 git 让 PR 能看到 API 变更）

前端用法：
```ts
import type { paths, components } from '@/api/schema';
type HealthResponse = components['schemas']['HealthResponse'];
type ListTagsResp = paths['/api/v1/tags']['get']['responses']['200']['content']['application/json'];
```

**Incremental adoption**：未加 `#[utoipa::path]` 注解的 handler 不出现在 schema，但 server 正常 work。修动旧 handler 时顺手补注解。Schema 也通过 `GET /api/openapi.json` 在线暴露。

## CLI (`lettura-cli`)

新增的 `lettura-cli` 面向 AI agent，位于 `cli/` 子 crate 中。

- **Workspace**: 项目已拆为 Cargo workspace（根 crate 为 server，`cli/` 为 CLI）。后端测试通过 Docker 容器执行；server 默认用 `docker compose exec lettura cargo test`，CLI 测试用 `docker compose exec lettura cargo test -p lettura-cli`，整套用 `docker compose exec lettura cargo test --workspace`。
- **认证**: 通过 Personal Access Token (PAT) 认证。明文令牌以 `lta_` 前缀识别；数据库只存 SHA-256。PAT 与 JWT 通过同一 `Authorization: Bearer` 头路由，`src/auth/middleware.rs` 根据前缀分流。
- **Skill**: AI 指令位于 `skills/lettura.md`（源文件，含 `{{BASE_URL}}` 和 `{{SERVER_VERSION}}` 占位符）。服务器通过 `GET /skills/lettura.md` 动态渲染，CLI 通过 `lettura-cli skill install` 分发已绑定的版本。
- **Release**: 打 `v*` tag 触发 `.github/workflows/release.yml` 构建 3 平台 binaries（linux-x86_64, darwin-x86_64, darwin-aarch64）。
- **契约测试**: `tests/cli_contract.rs` 用真实 CLI binary 打真实 server，跑 save/list/tag/markdown/bulk 全链路。维护 CLI 或服务器 API 时注意保持兼容。
- **Skill lint**: `cli/tests/skill_lint.rs` 会对 `skills/lettura.md` 中的每个命令示例用 clap 校验一遍，防止 skill 漂移。

## 不要做的事

- 不要引入 boa_engine（JS 引擎），已明确放弃此方案
- 不要添加 OAuth2 / i18n / 2FA
- 不要把前端拆成独立容器
- 不要用 SQLite
- 不要修改已通过评审的设计文档中的核心架构决策，除非有充分理由并记录
