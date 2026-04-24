# 抓取管线重设计实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 彻底替换 browserless 依赖，落地"YAML 规则优先 + 可选 chromiumoxide 渲染兜底"的混合抓取架构。

**Architecture:** YAML 配置（L2: URL 重写 + headers/cookies/UA + JSON Pointer 提取）→ 静态 HTTP → 渲染兜底（feature-gated）。将现有 `src/tasks/fetcher.rs` 拆成 `src/fetch/` 子模块。

**Tech Stack:** Rust 2024, Axum, serde_yaml, reqwest, chromiumoxide (optional feature), JSON Pointer (serde_json 自带)。

**参考 spec:** `docs/specs/2026-04-23-fetch-pipeline-redesign.md`

---

## Task 1: 重写 site_config 为 YAML

**Files:**
- Modify: `Cargo.toml`（加 `serde_yaml`）
- Modify: `src/site_config/mod.rs`（新 types：`SiteConfig`, `RequestConfig`, `ResponseConfig`, `Rewrite`, `RenderMode`）
- Modify: `src/site_config/parser.rs`（YAML 解析 + env 占位替换）
- Modify: `src/site_config/store.rs`（只扫 `site-configs-local/*.yaml`）

**TDD 要点：**
- parser 单测覆盖：最小规则、完整规则、env 占位 `${ENV_XXX}`、非法 YAML 报错
- `matches_url` 改用 regex 而非 starts_with
- store 测试用 `tempdir` 造 yaml 文件

## Task 2: 删除旧 FTR 配置

**Files:**
- Delete: `site-configs/*.txt`（12 个）
- Modify: `src/site_config/store.rs`（删 rust-embed 引用）
- Modify: `Cargo.toml`（rust-embed 还用于 SPA，保留）

旧的 `BuiltInConfigs` / `load_builtin` 代码移除，store 改为单一 "扫描 local dir" 模式。

## Task 3: 新建 src/fetch/ 骨架 + 纯函数模块

**Files:**
- Create: `src/fetch/mod.rs`
- Create: `src/fetch/rewrite.rs` — URL 路径正则重写
- Create: `src/fetch/json_extract.rs` — 按 JSON Pointer 抽 title/content/author/language
- Modify: `src/lib.rs`（`pub mod fetch;`）

每个文件有单测。

## Task 4: fetch/http.rs — 带 header 的抓取

**Files:**
- Create: `src/fetch/http.rs`
- 搬移 `tasks/fetcher.rs` 里的：`build_http_client`, `fetch_with_retry`, `fetch_with_retry_from_builder`, `rand_simple`, `DomainRateLimiter`
- 新增：把 `SiteConfig.request`（headers / cookies / user_agent）应用到请求

## Task 5: fetch/pipeline.rs — 编排静态路径

**Files:**
- Create: `src/fetch/pipeline.rs`
- 提供 `pub async fn process(ctx: &FetchContext, job: &FetchJob)`
- 整合 rewrite → http → json_extract OR html extract → save

`FetchContext` 封装：`PgPool`, `Arc<dyn ImageStorage>`, `SearchIndex`, `reqwest::Client`, `max_retries`。

## Task 6: tasks/fetcher.rs 瘦身

**Files:**
- Modify: `src/tasks/fetcher.rs`（删除 ~500 行，只留 `FetchJob`, `FetchQueue`, `start_fetch_worker`）
- worker 改为 `fetch::pipeline::process(&ctx, &job).await`

## Task 7: Cargo feature `rendering`

**Files:**
- Modify: `Cargo.toml`
- Create: `src/fetch/render/mod.rs`（Browser 单例 + 启动参数）

```toml
[features]
default = ["rendering"]
rendering = ["dep:chromiumoxide", "dep:futures-util"]
```

## Task 8: render service — Semaphore + circuit breaker

**Files:**
- Modify: `src/fetch/render/mod.rs`

```rust
pub struct RenderService {
    sem: Arc<Semaphore>,
    browser: Arc<RwLock<Option<Browser>>>,
    failures: Arc<AtomicUsize>,
    cooldown_until: Arc<RwLock<Option<Instant>>>,
}
```

## Task 9: pipeline 集成渲染分支

**Files:**
- Modify: `src/fetch/pipeline.rs`（feature-gated 渲染触发）
- Modify: `src/fetch/mod.rs`（`RenderService` 注入）

## Task 10: config.rs 更新

**Files:**
- Modify: `src/config.rs`

删 `rendering_url`，加 `rendering_enabled`, `chromium_path`, `render_concurrency`, `render_timeout_ms`, `render_wait_strategy`。

## Task 11: 清理 Browserless 残留

**Files:**
- Modify: `.env`
- Modify: `.env.example`
- Modify: `docker-compose.yml`
- Modify: `CLAUDE.md`

删所有 `LETTURA_RENDERING_URL` 引用，加新环境变量。

## Task 12: Dockerfile 支持 RENDERING ARG

**Files:**
- Modify: `Dockerfile`

`ARG RENDERING=1`，条件 `cargo build --no-default-features`，条件 `apt install chromium`。

## Task 13: 示例规则

**Files:**
- Create: `site-configs-local/github.com.yaml`
- Create: `site-configs-local/zhuanlan.zhihu.com.yaml`
- Create: `site-configs-local/medium.com.yaml`
- Create: `site-configs-local/.gitkeep`（保留目录）

## Task 14: 集成测试

**Files:**
- Create: `tests/fetch_pipeline_test.rs` — wiremock，覆盖 HTML / JSON / rewrite

## Task 15: Docker 端到端验收

运行 `RENDERING=0 docker compose build lettura` 跑通精简版。
运行 `docker compose build lettura` 完整版 + `up` + 抓取真实 URL（HTML / JSON / 渲染）。

---

## 关键失败降级

- YAML 解析失败 → tracing warn，跳过该文件，不影响其他
- URL 重写正则非法 → tracing warn，该规则跳过
- 渲染 service 启动失败 → 启动时降级为"只静态"，不 panic
- render.mode: force 但 feature 未编译 → 走静态
