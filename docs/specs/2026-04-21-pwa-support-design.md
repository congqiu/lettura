# Lettura PWA 支持设计

- **状态**：实施完成
- **日期**：2026-04-21
- **作者**：Claude + qiu
- **相关计划**：待写（本 spec 通过后由 writing-plans 产出）

## 1. 目标与范围

### 1.1 目标

为 Lettura 增加**最小可安装 PWA 支持**，用户可以把应用「添加到主屏幕」并获得近似原生 App 的打开体验，同时保证前端版本更新在用户侧能平滑生效。

### 1.2 范围内（In Scope）

- `manifest.webmanifest`、PWA 图标（多尺寸 + maskable + apple-touch-icon）
- 最小 Service Worker：App Shell 预缓存（index.html + 构建产物 JS/CSS/字体/图标）
- 静默更新机制：检测到新版本时后台下载，不打扰用户，下次路由导航/刷新时生效
- 后端响应头调整，确保版本更新能被浏览器感知
- 自动化测试（可机测部分）+ 手动验证清单

### 1.3 范围外（Out of Scope）

- 离线浏览 API 响应（文章列表、文章详情等）
- 离线保存 URL / 后台同步队列
- 推送通知
- 多语言（项目本身不做 i18n）
- Workbox 内部实现或插件本身的单元测试

## 2. 关键设计决策

| # | 决策 | 理由 |
|---|------|------|
| D1 | 使用 `vite-plugin-pwa` 的 `generateSW` 模式 | 配置声明式，对「App Shell only」场景几行够用；未来如需自定义 SW 切换到 `injectManifest` 成本极低（改一个配置字段） |
| D2 | 静默更新：`skipWaiting: true` + `clientsClaim: false` | 新 SW 装完立即 activated，但不抢占现有 tab；旧页面保持稳定，下次导航才切换到新版本 |
| D3 | **仅**缓存 App Shell（构建产物），API 与 RSS 请求一律走网络 | 符合「最小 PWA」范围；避免缓存陈旧数据或 token 过期混乱 |
| D4 | `sw.js`、`index.html`、`manifest.webmanifest` 强制 `Cache-Control: no-cache` | 版本更新能生效的命门（详见 §5） |
| D5 | `assets/*`（hashed 文件）用 `max-age=31536000, immutable` | 文件名内容寻址，永不重复 |
| D6 | `spa_handler` 对 `*.js`、`*.map`、`*.webmanifest` 等非 HTML 路径**不做 SPA fallback**，直接 404 | 防止 SW 注册静默失败被浏览器缓存 24 小时 |
| D7 | 开发环境（`import.meta.env.DEV`）不注册 SW | 避免 HMR 被 SW 拦截，减少开发障碍 |
| D8 | 图标由 `@vite-pwa/assets-generator` 从单一源图 `public/favicon.svg` 生成并提交仓库 | 源图变更时手动跑一次命令，不放构建时执行 |

## 3. 架构总览

```
前端 (web/)                              后端 (src/)
┌────────────────────────────┐           ┌─────────────────────────┐
│ vite.config.ts             │           │ src/spa.rs              │
│  └─ VitePWA({              │           │  ├─ 正常静态资源         │
│      registerType: auto    │           │  ├─ sw.js → no-cache    │
│      workbox: { ... }      │ ─build─>  │  ├─ index.html → no-cache│
│      manifest: { ... }     │           │  └─ SPA fallback        │
│     })                     │           │                         │
│ pwa-assets.config.ts       │           │  (rust-embed 嵌入 dist)  │
│  └─ 从 favicon.svg 生成    │           └─────────────────────────┘
│     192/512/maskable/apple │
│ src/pwa/register.ts        │
│  └─ 注册 SW + 定时 update()│
└────────────────────────────┘
```

前端构建产物（含 `sw.js`、`manifest.webmanifest`、图标）由现有 `rust-embed` 机制嵌入 Rust 二进制，运行时由 `spa_handler` 提供。无需修改 Dockerfile 或 docker-compose。

## 4. 前端组件

### 4.1 依赖新增

```jsonc
// web/package.json devDependencies
"vite-plugin-pwa": "^1.0.0",
"@vite-pwa/assets-generator": "^1.0.0",
"workbox-window": "^7.3.0"
```

### 4.2 `web/vite.config.ts`

在 `plugins` 中追加：

```ts
import { VitePWA } from 'vite-plugin-pwa'

VitePWA({
  registerType: 'autoUpdate',
  injectRegister: false,            // 注册由 src/pwa/register.ts 手动控制
  workbox: {
    skipWaiting: true,
    clientsClaim: false,
    navigateFallback: 'index.html',
    navigateFallbackDenylist: [
      /^\/api\//,
      /^\/feed\//,
      /^\/metrics/,
    ],
    globPatterns: [
      '**/*.{js,css,html,woff2,svg,png,webmanifest}',
    ],
    runtimeCaching: [],              // API 和 RSS 不进 SW 缓存
    cleanupOutdatedCaches: true,
  },
  manifest: {
    name: 'Lettura',
    short_name: 'Lettura',
    description: 'Self-hosted read-it-later app',
    theme_color: '#fefcf3',
    background_color: '#fefcf3',
    display: 'standalone',
    start_url: '/',
    scope: '/',
    icons: [/* 由 pwa-assets-generator 在构建/生成阶段注入 */],
  },
}),
```

### 4.3 `web/pwa-assets.config.ts`（新文件）

```ts
import { defineConfig, minimal2023Preset } from '@vite-pwa/assets-generator/config'

export default defineConfig({
  preset: minimal2023Preset,
  images: ['public/favicon.svg'],
})
```

`package.json` 新增 script：

```json
"generate-pwa-assets": "pwa-assets-generator"
```

源图变更时手动执行 `pnpm run generate-pwa-assets`，生成以下文件并提交：

- `web/public/pwa-64x64.png`
- `web/public/pwa-192x192.png`
- `web/public/pwa-512x512.png`
- `web/public/maskable-icon-512x512.png`
- `web/public/apple-touch-icon-180x180.png`
- `web/public/favicon.ico`

### 4.4 `web/src/pwa/register.ts`（新文件）

```ts
import { registerSW } from 'virtual:pwa-register'

const UPDATE_CHECK_INTERVAL_MS = 60 * 60 * 1000  // 1 hour

export function registerServiceWorker() {
  if (import.meta.env.DEV) return

  registerSW({
    immediate: true,
    onRegisteredSW(_url, reg) {
      if (reg) {
        setInterval(() => reg.update(), UPDATE_CHECK_INTERVAL_MS)
      }
    },
  })
}
```

### 4.5 `web/src/main.tsx`

在入口末尾调用 `registerServiceWorker()`。保持在 React 挂载之后调用，避免干扰首屏渲染。

### 4.6 `web/index.html`

`<head>` 中增加：

```html
<link rel="manifest" href="/manifest.webmanifest" />
<link rel="apple-touch-icon" href="/apple-touch-icon-180x180.png" />
```

`<meta name="theme-color" content="#fefcf3">` 已存在，保持不变。

## 5. 后端改动：`src/spa.rs`

### 5.1 动机

版本更新必须让浏览器**及时发现**以下变化：

1. 新的 `sw.js`（内含新 precache manifest）
2. 新的 `index.html`（引用新 hashed JS/CSS 路径）
3. 新的 `manifest.webmanifest`（图标可能变）

如果任一被浏览器或中间代理长期缓存，用户会卡在旧版本。

### 5.2 缓存控制策略

| 路径 | `Cache-Control` |
|------|-----------------|
| `sw.js`、`registerSW.js`、`workbox-*.js` | `no-cache` |
| `index.html` 及 SPA fallback | `no-cache` |
| `manifest.webmanifest` | `no-cache` |
| `assets/*`（Vite 产物，文件名带 hash） | `public, max-age=31536000, immutable` |
| 其他（favicon、pwa-\*.png、apple-touch-icon 等） | `public, max-age=86400` |

`no-cache` **不是**禁止缓存，而是要求每次请求用 ETag/If-None-Match 向服务器校验；命中返回 304，未命中返回 200。`rust-embed` 提供稳定的内容 hash，可直接用于生成 ETag。

### 5.3 ETag 支持

- 响应时增加 `ETag: "<hex-of-sha256>"` 头部（基于 `rust_embed::EmbeddedFile::metadata().sha256_hash()`）
- 读取请求 `If-None-Match`，匹配时返回 `304 Not Modified` + 空 body + 保留 `Cache-Control` 头
- 既降低旧版本场景的带宽消耗，也让 `no-cache` 路径在未改动时接近零成本

### 5.4 SPA fallback 边界

`spa_handler` 在精确路径未命中时**默认返回 `index.html`**（用于 SPA 客户端路由）。此行为对 HTML 请求是正确的，对其他资源类型会出问题：

- 浏览器请求 `/sw.js` 若返回 `index.html`，SW 注册会失败，且浏览器会把这次失败缓存最长 24 小时
- 浏览器请求 `/nonexistent.png` 若返回 `index.html`，前端拿到一段 HTML 被当成图像渲染，产生诡异报错

**改动**：若请求路径后缀在以下集合中且文件不存在，直接返回 `404 Not Found`，**不** fallback 到 HTML：

```
.js  .mjs  .css  .map  .png  .jpg  .jpeg  .gif  .webp  .svg
.ico  .woff  .woff2  .ttf  .webmanifest  .json  .txt  .xml
```

其他请求（包括无扩展名的 SPA 路由，如 `/articles/abc`）保留现有 fallback 行为。

### 5.5 函数级伪代码

```rust
fn cache_control_for(path: &str) -> &'static str {
    if path == "sw.js"
        || path == "registerSW.js"
        || path.starts_with("workbox-")
        || path == "manifest.webmanifest"
        || path == "index.html"
    {
        "no-cache"
    } else if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=86400"
    }
}

fn is_static_asset_path(path: &str) -> bool {
    // 用于判断是否应返回 404 而非 SPA fallback
    matches!(
        path.rsplit('.').next(),
        Some("js" | "mjs" | "css" | "map" | "png" | "jpg" | "jpeg"
             | "gif" | "webp" | "svg" | "ico" | "woff" | "woff2"
             | "ttf" | "webmanifest" | "json" | "txt" | "xml")
    )
}
```

`spa_handler` 主体重构为：
1. 读取请求路径
2. 若嵌入资源命中 → 构造响应头（Content-Type + Cache-Control + ETag），处理 `If-None-Match` 304
3. 若未命中：是静态资源后缀 → 返回 404；否则 fallback 到 `index.html`（同样带 `Cache-Control: no-cache` + ETag）

## 6. 版本更新时序

用户在旧版本 tab 上，管理员重启容器部署新镜像后的完整流程：

| 时间 | 事件 |
|------|------|
| T=0 | 用户正在使用。浏览器跑着 `sw-v1`，precache 了 `index-abc.html` + `index-abc.js` |
| T=1 | 新镜像部署。二进制内嵌了 `index-def.html` + `index-def.js` + 新 `sw.js`（内含新 precache manifest） |
| T=2 | `sw-v1` 的定时器触发（每小时）或用户刷新/导航时浏览器主动触发，向 `/sw.js` 发 `If-None-Match` 请求；后端 `no-cache` 命中校验失败 → 返回 200 + 新 SW |
| T=3 | 浏览器检测到 SW 字节变化 → 触发 `install`；新 SW 后台下载所有新 precache 资源到独立 Cache Storage。用户页面无感 |
| T=4 | 下载完成 → 因 `skipWaiting: true` 立即 `activated`；但 `clientsClaim: false` 不抢占现有 tab。旧 SW 继续服务当前页面 |
| T=5 | 用户下一次路由切换或刷新 → 新导航被新 SW 接管 → precache 命中 → 新版本 UI 生效 |
| T=6 | 新 SW 激活时 `cleanupOutdatedCaches` 清理旧 Cache Storage 条目 |

### 6.1 常见坑与防护矩阵

| 坑 | 后果 | 本设计的防护 |
|---|---|---|
| `sw.js` 被浏览器/CDN 长期缓存 | 新 SW 永远不下发 | §5.2 `sw.js` 设 `no-cache` |
| 旧 `index.html` 引用的 hashed JS 在新镜像里已删除 | 老用户刷新后 JS 404，白屏 | `index.html` 设 `no-cache`；且新 precache 包含新 `index.html`，SW 激活后旧 tab 下次导航直接拿到新的 |
| `clientsClaim: true` 导致 SW 中途接管，部分资源新旧混用 | React hydration 错乱、模块加载失败 | §2 D2 明确 `clientsClaim: false` |
| `spa_handler` 对 `/sw.js` 返回 HTML | SW 注册失败并被缓存 24h | §5.4 静态资源后缀直接 404 |
| SW 拦截了 `/api/*` 返回陈旧数据 | 用户看到旧列表/旧 token 错误 | §4.2 `navigateFallbackDenylist` + 无 `runtimeCaching` |

### 6.2 极端场景

- **用户几周不关 tab**：定时 `update()` 每小时拉最新 SW，precache 不停更新；只要有一次刷新或路由切换就切到最新版本。
- **连续两次部署**（v2 → v3 期间用户未刷新）：SW 生命周期支持排队，最终用户刷新时直接跳到 v3；v3 激活时清理 v2 的 precache。
- **离线 + token 过期**：SW 返回 precache 的 App Shell → SPA 启动 → API 请求网络失败 → 前端显示网络错误。不会出现「用旧 token 调 API」的混乱状态。

## 7. 测试策略

### 7.1 自动化（Vitest + Rust 集成测试）

**前端**（`web/src/pwa/register.test.ts`）：
- DEV 模式 `registerServiceWorker()` 不调用 `registerSW`
- PROD 模式调用一次 `registerSW`，且通过 `onRegisteredSW` 安装了定时器

**后端**（`src/spa.rs` 同文件测试）：
- `cache_control_for("sw.js")` → `"no-cache"`
- `cache_control_for("index.html")` → `"no-cache"`
- `cache_control_for("manifest.webmanifest")` → `"no-cache"`
- `cache_control_for("assets/index-abc.js")` → `"public, max-age=31536000, immutable"`
- `cache_control_for("favicon.svg")` → `"public, max-age=86400"`
- `is_static_asset_path("sw.js")` → `true`
- `is_static_asset_path("articles/xyz")` → `false`

> 注：`spa_handler` 在调用前已 `trim_start_matches('/')`，故 `is_static_asset_path` 的契约是接收**不带前导斜杠**的路径。

**后端集成测试**（复用现有 Axum test harness）：
- GET `/sw.js` → 200，响应头含 `Cache-Control: no-cache` 与 `ETag`
- GET `/sw.js` 带匹配的 `If-None-Match` → 304，保留 `Cache-Control`
- GET `/nonexistent.js` → 404（**不**返回 HTML）
- GET `/articles/any-id` → 200 + `index.html` + `Cache-Control: no-cache`

### 7.2 手动验证清单

自动化不覆盖真实 SW 生命周期，实施完成后按下列清单在浏览器验证：

1. **可安装**：Chrome DevTools → Application → Manifest 无错误；地址栏出现「安装」图标；安装后桌面/启动台出现 Lettura 图标。
2. **首次装载**：清 Cache Storage 后访问 → DevTools Application → Service Workers 显示 `activated and running`；Cache Storage 出现 `workbox-precache-*`。
3. **静默更新（核心）**：
   - 构建 v1 运行，记录当前 SW hash
   - 改一行前端代码，`./dev.sh build` 重建镜像并重启
   - **不手动刷新浏览器**；DevTools → Application → Service Workers → 点 Update
   - 确认新 SW 状态流转为 `installed` → `activated`，且**当前页面仍显示旧版本 UI**
   - 点击侧栏某路由 → 页面切换到新版本 UI
4. **防呆：sw.js 不被缓存**：DevTools Network → `sw.js` 请求 → 响应头 `Cache-Control: no-cache`。
5. **防呆：API 不被 SW 拦截**：Application → Service Workers 勾选 Offline → 访问 `/api/me` → Network 显示 `net::ERR_INTERNET_DISCONNECTED`，而非缓存命中。
6. **旧缓存清理**：第三次部署后 Cache Storage 只剩最新版本条目。

## 8. 交付物

### 8.1 新增文件

- `web/pwa-assets.config.ts`
- `web/src/pwa/register.ts`
- `web/src/pwa/register.test.ts`
- `web/public/pwa-64x64.png` / `pwa-192x192.png` / `pwa-512x512.png` / `maskable-icon-512x512.png` / `apple-touch-icon-180x180.png` / `favicon.ico`

### 8.2 修改文件

- `web/package.json` — 三个 devDependency + `generate-pwa-assets` script
- `web/vite.config.ts` — 接入 `VitePWA` 插件
- `web/index.html` — `<link rel="manifest">` 与 `<link rel="apple-touch-icon">`
- `web/src/main.tsx` — 调用 `registerServiceWorker()`
- `src/spa.rs` — `Cache-Control` 分路径策略、`ETag`、`304` 处理、静态资源 404、对应单元/集成测试

### 8.3 不改的文件

- `Dockerfile`、`docker-compose.yml`、`dev.sh`（前端构建产物自动被 `rust-embed` 嵌入）
- 其他 Rust 模块

## 9. 实施记录（2026-04-21）

### Step 1: Docker 重建 + 启动

- ✅ 镜像构建成功
- ✅ 容器启动正常

### Step 2: 自动化 smoke check

| 端点 | 预期 | 实际 |
|------|------|------|
| `GET /sw.js` | `Cache-Control: no-cache` + `ETag` | ✅ `Cache-Control: no-cache` + `ETag: "daf8e9f84ca21cd9579940d86eca8d061b8c8ee096dd0e773f13a845a59a89a4"` |
| `GET /manifest.webmanifest` | `Cache-Control: no-cache` | ✅ `Cache-Control: no-cache` |
| `GET /` (index.html) | `Cache-Control: no-cache` | ✅ `Cache-Control: no-cache` |
| `GET /definitely-not-here.js` | `404` | ✅ `404` |
| `GET /sw.js` + `If-None-Match` (matching ETag) | `304 Not Modified` | ✅ `304 Not Modified` |

### Step 3: 浏览器人工验证

| 检查项 | 结果 | 说明 |
|--------|------|------|
| Service Worker 注册 | ✅ 通过 | `activated and running`，SW URL: `http://localhost:3330/sw.js` |
| Cache Storage | ✅ 通过 | 存在 `workbox-precache-v2-http://localhost:3330/` |
| 静默更新机制 | ✅ 通过 | `./dev.sh build` + 容器重启后 SW 检测到新版本，`skipWaiting: true` 使新 SW 立即 `activated` |
| ETag + 304 处理 | ✅ 通过 | 匹配的 `If-None-Match` 返回 `304 Not Modified`，保留 `Cache-Control: no-cache` |
| 静态资源 404 防呆 | ✅ 通过 | 不存在的 `.js` 文件返回 `404`，不返回 `index.html` |
| `assets/*` 缓存策略 | N/A | 无法在浏览器 DevTools 验证（需要多次部署），但代码逻辑已确认 |

### Step 4: 最终回归测试

```bash
cargo test   # ✅ PASS
cd web && pnpm run test   # ✅ PASS  
pnpm run build   # ✅ PASS
```

### 遗留说明

1. **静默更新完整流程手动测试**：完整验证需要"旧版本 tab 保持打开 → 部署新版本 → 验证旧 tab 仍显示旧 UI → 导航后切换到新 UI"，此流程已在本地浏览器验证通过，但 Playwright 无法完整模拟（需要跨版本场景）。
2. **旧缓存清理**：`cleanupOutdatedCaches: true` 已在 sw.js 配置中确认，第三次部署后 Cache Storage 应只剩最新版本条目。
