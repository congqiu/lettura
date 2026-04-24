# 抓取管线重设计：规则优先 + 渲染兜底

## 背景

当前的抓取系统依赖外部 `browserless` 服务作为 JS 渲染兜底，但：

1. **Browserless 已从运行时移除**（`docker-compose.yml` 不再定义该 service），但代码、`.env`、site-configs 中仍有大量残留——`LETTURA_RENDERING_URL` 配置项、`fetcher.rs` 里约 100 行渲染相关代码、6 个 site-config 文件里的 `render: true`——指向一个不存在的服务。
2. **表达力不足**：FTR 文本格式只能描述 CSS 选择器和少量 HTTP 头，无法应对"SPA 前端但后端有开放 API"这类场景（如知乎 `/p/{id}` 背后对应 `/api/articles/{id}` 返回 JSON）。
3. **外部依赖不可控**：browserless 官方镜像的行为无法自定义——不能注入额外请求头、不能做启动参数调优、不能精细控制等待条件。

本设计用"**规则优先 + 内嵌 Chromium 兜底**"的混合架构替代现有实现：

- 规则引擎支持 URL 重写、自定义 header/Cookie、API 直调 + JSON 路径提取（能力上限 L2）
- 渲染兜底通过 `chromiumoxide` 内嵌到主二进制，受 Cargo feature 控制，可构建精简版镜像
- 配置文件从 FTR 文本改为 YAML，表达力更强

## 设计目标

- **彻底移除 browserless 残留**：代码、配置、文档一次清理干净
- **规则表达力达到 L2**：headers/Cookie/UA + URL 重写 + API/JSON 响应提取
- **渲染可选**：`chromiumoxide` 放在 `rendering` Cargo feature 后，构建时可去除（镜像大小 ~100MB vs ~350MB 两档）
- **向后兼容不做**：YAML 统一重设，删除现有 `site-configs/*.txt`（用户选择）
- **单体架构不变**：一个 Rust 二进制 + postgres，Chromium 作为同镜像内的系统依赖

## 整体架构

### 数据流

```
FetchJob(url)
   │
   ▼
[1] 规则查找
     - 按 domain + URL 路径从 site-configs-local/*.yaml 查 SiteConfig
     - 未匹配 → 跳到 [3]（无规则走默认 HTTP + readability）
   │
   ▼
[2] URL 重写（可选）
     - 匹配 rewrite.from 的第一条规则，替换为 rewrite.to
     - render.mode == force → 跳到 [5]
   │
   ▼
[3] HTTP 抓取
     - 带 request.headers / cookies / user_agent
     - 失败（5xx / timeout）按原有指数退避重试
     - 仍失败 → render.mode != never 则跳到 [5]，否则跳到 [7]
   │
   ▼
[4] 提取（按 response.type 分支）
     - type=json → 用 JSON Pointer 提取 title/content/author/language
     - type=html（或默认）→ 用 selectors 提取；失败回落 readability；再失败回落 body
   │
   ▼
[5] 渲染兜底（仅 feature=rendering 且 RENDERING_ENABLED=true）
     触发条件（任一）：
       - render.mode == force
       - 静态 HTTP 抓取失败
       - [4] 提取出的 text_content 长度 < 100 且 render.mode != never
     - 复用单例 Chromium，开 Page，navigate + 等待策略
     - 拿渲染后 HTML → 回走 [4] 的 HTML 分支
   │
   ▼
[6] 图片代理化 + 入库 + 索引
     - storage::process_images
     - entry::update_entry_content
     - SearchIndex::upsert
     - apply_tagging_rules
   │
   ▼
[7] 最终降级
     - 若有部分内容则保留
     - 否则 fetch_status = "failed"
```

### 顶层模块划分

| 模块 | 责任 | 状态 |
|------|------|------|
| `src/site_config/` | YAML 解析、按 domain+URL 查找规则 | 重写 |
| `src/fetch/mod.rs` | 对外公开 `process(job)` | 新建 |
| `src/fetch/pipeline.rs` | 编排数据流 [1]-[7]，调用下面各子模块 | 新建 |
| `src/fetch/http.rs` | 带 header/cookie/UA 的静态抓取 + 重试 | 新建 |
| `src/fetch/rewrite.rs` | URL 正则重写 | 新建 |
| `src/fetch/json_extract.rs` | JSON Pointer 提取 → `ExtractResult` | 新建 |
| `src/fetch/render/` | `chromiumoxide` 封装（feature-gated） | 新建 |
| `src/tasks/fetcher.rs` | 仅保留 mpsc 队列 + worker 协程，调用 `fetch::process` | 瘦身 |

**为什么这么分**：现有 `fetcher.rs` 675 行承担了队列、重试、渲染、入库、tagging、rate-limit 全部职责，单测难度高。拆分后每块职责单一，pipeline 作为纯编排，可 mock 各步骤。

## 配置格式（YAML）

### Schema

```yaml
# 匹配作用域
domain: zhihu.com              # 必填，查找键
match: ["^/p/"]                # 可选，URL path 部分白名单正则（数组；默认全匹配）
exclude: ["^/video/"]          # 可选，URL path 部分黑名单正则（优先级高于 match）

# URL 重写（可选）
rewrite:
  - from: "^/p/(\\d+)"         # 作用于 URL path，正则
    to: "/api/articles/$1"     # 仅替换 path；scheme/host/query 保持不变；支持捕获组 $1-$9

# HTTP 请求参数（可选）
request:
  headers:
    Referer: "https://zhuanlan.zhihu.com/"
    X-Requested-With: "XMLHttpRequest"
  cookies:
    z_c0: "${ENV_ZHIHU_TOKEN}"   # ${ENV_*} 占位符，从进程 env 替换
  user_agent: "Mozilla/5.0 ..."  # 覆盖全局 UA

# 响应解析
response:
  type: json | html              # 默认 html
  # type=json
  json:
    title: "/data/title"         # JSON Pointer (RFC 6901)
    content: "/data/content"
    content_is_html: true        # true = 作为 HTML 继续清洗；false = 纯文本
    author: "/data/author/name"
    language: "/data/lang"
    published_at: "/data/publishedAt"
  # type=html
  html:
    title: "h1.article-title"    # CSS 选择器
    body: ["article.post", "div.content"]  # 按序尝试
    strip: ["div.ads", ".share-widget"]
    author: "span.author"

# 渲染兜底策略
render:
  mode: never | auto | force     # 默认 auto
  # never = 不渲染，抓不到就失败
  # auto  = 静态抓取失败或内容过短才渲染
  # force = 跳过静态抓取直接渲染
  wait_for: "div.article-body"   # 可选，等待选择器出现
  timeout_ms: 15000              # 可选，覆盖全局默认
```

### 三个典型例子

**github.com.yaml**（静态 HTML + 选择器）
```yaml
domain: github.com
response:
  type: html
  html:
    title: "title"
    body: ["article.markdown-body", "#readme"]
    strip: [".anchor"]
```

**zhuanlan.zhihu.com.yaml**（SPA 走 API）
```yaml
domain: zhuanlan.zhihu.com
rewrite:
  - from: "^/p/(\\d+)"
    to: "/api/articles/$1"
request:
  headers:
    Referer: "https://zhuanlan.zhihu.com/"
response:
  type: json
  json:
    title: "/title"
    content: "/content"
    content_is_html: true
    author: "/author/name"
render:
  mode: auto
```

**medium.com.yaml**（强制渲染）
```yaml
domain: medium.com
render:
  mode: force
  wait_for: "article"
response:
  type: html
  html:
    title: "article h1"
    body: ["article"]
    strip: ["[data-testid='footer']", "nav"]
```

### 关键决策

- **JSON 提取用 JSON Pointer**：标准 RFC 6901，`serde_json::Value::pointer` 零依赖、语义明确；不引入 jsonpath（多方言、依赖重）。
- **不保留编译期内置规则**：现有 `site-configs/*.txt` 12 个文件全部删除；用户在 `site-configs-local/*.yaml` 自行配置。
- **环境变量占位**：`${ENV_XXX}` 形式，在规则加载时从 `std::env::var` 替换；找不到则保留原文（不抛错）。
- **匹配优先级**：本地 `site-configs-local/*.yaml` → 数据库 `site_rules`（现有表保留，仅提供 CSS 选择器回退）→ readability 自动提取。数据库 `site_rules` 只参与 [4] HTML 提取步骤（当 YAML 规则未匹配时），不参与 URL 重写、header 注入、渲染控制——这些 L2 能力仅通过 YAML 提供。

## 渲染服务（chromiumoxide）

### 生命周期

- 全局一个 `chromiumoxide::Browser` 单例，在 `start_fetch_worker` 时 spawn handler
- 并发渲染通过 `tokio::sync::Semaphore` 限制（默认 2）
- 每次渲染 `browser.new_page(url)` → 等待策略 → `page.content()` → `page.close()`
- 不复用 Page（避免 cookie / storage 污染）

### 崩溃恢复

- 封装为 `RenderService`，内部 `Arc<RwLock<Option<Browser>>>`
- 每次取 Browser 前检测 handler task 状态；异常时重启
- **Circuit breaker**：连续 5 次渲染失败 → 冷却 60s 不再尝试（仅影响渲染路径）
- 进程收到 SIGTERM 时显式 `browser.close()`

### Chromium 可执行文件

- 不让 chromiumoxide 自动下载
- 从 PATH 查 `chromium` / `google-chrome` / `chrome`
- 环境变量 `LETTURA_CHROMIUM_PATH` 显式指定时优先使用

### 启动参数

```
--no-sandbox                              # Docker 必需
--disable-gpu
--disable-dev-shm-usage                   # Docker 环境 /dev/shm 太小
--headless=new
--hide-scrollbars
--disable-blink-features=AutomationControlled  # 减弱自动化特征
```

### 等待策略

通过 `LETTURA_RENDER_WAIT_STRATEGY` 全局配置，规则里 `render.wait_for` 可覆盖：

- `networkidle`（默认）：500ms 无网络请求
- `domcontent`：DOMContentLoaded 触发
- `load`：load 事件触发
- 规则里配 `wait_for: "selector"` → 等该 CSS 选择器匹配到元素

## Cargo feature + Dockerfile

### Cargo.toml

```toml
[features]
default = ["rendering"]
rendering = ["dep:chromiumoxide", "dep:futures-util"]

[dependencies]
# 版本号在实施时确认 crates.io 当前稳定版
chromiumoxide = { version = "<latest>", features = ["tokio-runtime"], optional = true }
futures-util  = { version = "<latest>", optional = true }
```

整个 `src/fetch/render/` 模块 `#[cfg(feature = "rendering")]`。pipeline 里通过 feature gate 的分支调用。

### Dockerfile（在现有多阶段基础上增加 RENDERING 控制）

现有 Dockerfile 是 `frontend-builder` (node) + `backend-builder` (rust:latest) + runtime 的三阶段结构。修改点：

```dockerfile
# 顶层增加 ARG
ARG RENDERING=1

# backend-builder 阶段改 cargo build 命令
FROM rust:latest AS backend-builder
ARG RENDERING
...
RUN if [ "$RENDERING" = "1" ]; then \
      cargo build --release; \
    else \
      cargo build --release --no-default-features; \
    fi

# runtime 阶段条件安装 chromium
FROM debian:bookworm-slim
ARG RENDERING
RUN if [ "$RENDERING" = "1" ]; then \
      apt-get update && apt-get install -y --no-install-recommends \
        chromium fonts-noto-cjk fonts-noto-color-emoji ca-certificates && \
      rm -rf /var/lib/apt/lists/*; \
    fi
COPY --from=backend-builder /app/target/release/lettura /usr/local/bin/
```

docker-compose.yml：
```yaml
build:
  context: .
  args:
    RENDERING: ${RENDERING:-1}
```

使用：`RENDERING=0 docker compose build lettura` 得到精简版（~100MB），默认得到完整版（~350MB）。

## 环境变量变更

### 新增

| 变量 | 默认 | 说明 |
|------|------|------|
| `LETTURA_RENDERING_ENABLED` | `auto` | `auto`（feature 开启即启用）/ `true` / `false`。当二进制以 `--no-default-features` 构建时，此变量被忽略（渲染模块未编译进来） |
| `LETTURA_CHROMIUM_PATH` | 自动搜索 PATH | Chromium 可执行文件绝对路径 |
| `LETTURA_RENDER_CONCURRENCY` | `2` | 并发渲染上限 |
| `LETTURA_RENDER_TIMEOUT_MS` | `15000` | 单次渲染总超时 |
| `LETTURA_RENDER_WAIT_STRATEGY` | `networkidle` | `networkidle` / `domcontent` / `load` |

### 删除

- `LETTURA_RENDERING_URL`（原 browserless 地址）

## 错误处理

四级降级：

1. 静态抓取失败 → 按 `render.mode` 决定是否兜底渲染
2. 渲染失败 / 禁用 → 若已拿到部分静态 HTML 就用（过短也比没有好）
3. 全部失败 → `fetch_status = "failed"`，保留 URL+域名供用户手动重试
4. Circuit breaker → 冷却期内跳过渲染路径，静态路径不受影响

所有错误 `tracing::warn!` / `error!` 记录，含 `entry_id`、`url`、失败阶段（`rewrite` / `http` / `parse` / `render`）。

## 测试策略

### 单元测试

- `site_config/parser.rs`：YAML 字段、非法字段、环境变量替换、缺失 domain 报错
- `fetch/rewrite.rs`：正则重写、多规则顺序、捕获组
- `fetch/json_extract.rs`：Pointer 路径、缺失字段降级、`content_is_html` 两分支
- `fetch/http.rs`：header / cookie / UA 注入、5xx 重试、非法 header 忽略
- `fetch/render/`：仅 `rendering` feature 开启时编译测试；CI 默认跳过（`#[ignore]`）

### 集成测试

`tests/fetch_pipeline_test.rs` 用 `wiremock`：

- 静态 HTML + 选择器提取
- JSON 响应 + JSON Pointer 提取
- URL 重写后二次请求
- 内容过短触发渲染（feature 关闭时走降级）

### Feature 矩阵

- `cargo test` → 默认全开
- `cargo test --no-default-features` → 精简版编译 + 测试通过

## 迁移步骤

| 步骤 | 内容 | 原子提交 |
|------|------|----------|
| 1 | 新建 `src/fetch/{mod,pipeline,http,rewrite,json_extract}.rs` 骨架 + 类型 | ✓ |
| 2 | 重写 `src/site_config/` 为 YAML（删 FTR parser，建新类型） | ✓ |
| 3 | 删除 `site-configs/*.txt` 12 个文件 + `include_str!` 逻辑 | ✓ |
| 4 | 实现 URL rewrite、HTTP + headers、JSON 提取，`pipeline::process` 跑通静态路径 | ✓ |
| 5 | 加 `rendering` feature + `src/fetch/render/`（Browser 单例 + Semaphore + circuit breaker） | ✓ |
| 6 | `src/tasks/fetcher.rs` 瘦身到 worker 骨架 + queue，其余搬 pipeline | ✓ |
| 7 | Dockerfile 加 `RENDERING` ARG，docker-compose 透传 | ✓ |
| 8 | 清理 `.env` / `.env.example` / `docker-compose.yml` / `CLAUDE.md` 的 `LETTURA_RENDERING_URL`，加新变量 | ✓ |
| 9 | 在 `site-configs-local/` 放 2-3 个示例规则（github/zhuanlan.zhihu/medium） | ✓ |
| 10 | 本地 `docker compose build + up`，用真实 URL 验收（HTML / JSON / 渲染各一） | ✓ |

每步独立可编译、能通过单测，符合 `CLAUDE.md` 的 TDD + 原子提交规范。

## 清理清单

**删除**
- `site-configs/*.txt`（12 个文件）
- `src/site_config/parser.rs` 中 FTR 解析代码
- `src/tasks/fetcher.rs` 中 `process_rendered` / `fetch_rendered` / `build_http_client`（搬移到 `fetch/`）
- `.env` 中 `LETTURA_RENDERING_URL` 行
- `.env.example` 中相关示例（若有）
- `docker-compose.yml` 中 `LETTURA_RENDERING_URL` 透传
- `CLAUDE.md` 中 Browserless 相关说明

**重写**
- `src/site_config/mod.rs`（类型定义）
- `src/site_config/parser.rs`（改为 `serde_yaml` 解析）
- `src/site_config/store.rs`（只扫描 `site-configs-local/*.yaml`，不再从内置库加载）

## 非目标

- **不做向后兼容的 FTR 文本格式解析**：用户选择"YAML 统一重设"，不再支持 `.txt` 格式
- **不支持多步骤请求**（L3 能力）：一条规则只发一次 HTTP；需要链式请求的场景走渲染兜底
- **不做反爬 stealth 增强**：除了上面列的基础启动参数，不引入 `undetected-chromedriver` 类的高级反检测
- **不改现有图片代理、入库、tagging、搜索索引的接口**：这些模块不动
- **不影响 RSS feed、annotations、memos 等其他功能**

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| 镜像变大到 350MB | 提供 `RENDERING=0` 精简构建；README 明确两档选择 |
| Chromium 启动失败 | 启动时健康检查，失败降级为"只静态抓取"；日志明确指出原因 |
| 渲染内存爆炸 | Semaphore 并发上限 + 单页超时 + 进程监控（后续可加） |
| chromiumoxide 协议漂移 | 锁定 crate 版本；Chromium 在 Dockerfile 里也锁定 apt 版本 |
| 用户现有 `.txt` 规则失效 | 文档提示迁移方案；提供示例 YAML |

## 参考

- chromiumoxide：https://github.com/mattsse/chromiumoxide
- JSON Pointer RFC 6901：https://datatracker.ietf.org/doc/html/rfc6901
- 现行站点配置设计：`docs/specs/2026-04-18-site-config-design.md`
- 现行抓取实现：`src/tasks/fetcher.rs`
