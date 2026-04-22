# PWA Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给 Lettura 增加最小可安装 PWA 支持（manifest + 图标 + App Shell precache），并确保前端资源版本更新能在用户侧平滑生效。

**Architecture:** 前端用 `vite-plugin-pwa`（`generateSW` 模式）生成 manifest + Service Worker；`skipWaiting + !clientsClaim` 实现「静默更新，下次导航生效」；后端 `src/spa.rs` 分路径差异化 `Cache-Control`（`sw.js` / `index.html` / `manifest` → `no-cache` + ETag，hashed assets → 1 年 immutable），并对非 HTML 资源后缀的 404 路径**禁用** SPA fallback 防止 SW 注册静默失败。

**Tech Stack:** vite-plugin-pwa 1.x，@vite-pwa/assets-generator 1.x，workbox-window 7.x，rust-embed 8（已有），axum 0.8（已有），Vitest 4（已有）。

**Spec reference:** `docs/specs/2026-04-21-pwa-support-design.md`

---

## File Structure

**新增文件**
- `web/pwa-assets.config.ts` — 图标生成配置
- `web/src/pwa/register.ts` — Service Worker 注册逻辑
- `web/src/pwa/register.test.ts` — 注册逻辑单元测试
- `web/public/pwa-64x64.png`, `pwa-192x192.png`, `pwa-512x512.png`, `maskable-icon-512x512.png`, `apple-touch-icon-180x180.png`, `favicon.ico` — 生成并提交
- `tests/integration_spa.rs` — `spa_handler` 集成测试

**修改文件**
- `web/package.json` — 三个 devDependency + `generate-pwa-assets` script
- `web/vite.config.ts` — 接入 `VitePWA` 插件
- `web/index.html` — `<link rel="manifest">` 与 `<link rel="apple-touch-icon">`
- `web/src/main.tsx` — 调用 `registerServiceWorker()`
- `web/src/env.d.ts` — `virtual:pwa-register` 模块类型声明（如缺）
- `src/spa.rs` — `cache_control_for`、`is_static_asset_path` 纯函数 + `spa_handler` 重构（ETag、304、404-for-static）

---

## Task 1: 安装前端 PWA 依赖

**Files:**
- Modify: `web/package.json`

- [ ] **Step 1: 在 `web/` 目录添加三个 devDependency**

Run：
```bash
cd web && pnpm add -D vite-plugin-pwa@^1.0.0 @vite-pwa/assets-generator@^1.0.0 workbox-window@^7.3.0
```

- [ ] **Step 2: 验证安装成功**

Run：
```bash
cd web && pnpm list vite-plugin-pwa @vite-pwa/assets-generator workbox-window
```
Expected：三个包都显示版本号，无报错。

- [ ] **Step 3: 在 `web/package.json` 的 `scripts` 段新增 `generate-pwa-assets`**

```jsonc
"scripts": {
  "dev": "vite",
  "build": "tsc -b && vite build",
  "lint": "eslint .",
  "preview": "vite preview",
  "test": "vitest run",
  "test:watch": "vitest",
  "generate-pwa-assets": "pwa-assets-generator"
}
```

- [ ] **Step 4: 确认 TypeScript/编译未被破坏**

Run：
```bash
cd web && pnpm run build
```
Expected：构建成功（PWA 插件尚未接入，build 产物和之前一致）。

- [ ] **Step 5: Commit**

```bash
git add web/package.json web/pnpm-lock.yaml
git commit -m "chore(web): add vite-plugin-pwa + assets-generator deps"
```

---

## Task 2: 生成 PWA 图标资产

**Files:**
- Create: `web/pwa-assets.config.ts`
- Create: `web/public/pwa-64x64.png`, `pwa-192x192.png`, `pwa-512x512.png`, `maskable-icon-512x512.png`, `apple-touch-icon-180x180.png`, `favicon.ico`

- [ ] **Step 1: 创建 `web/pwa-assets.config.ts`**

```ts
import { defineConfig, minimal2023Preset } from '@vite-pwa/assets-generator/config'

export default defineConfig({
  preset: minimal2023Preset,
  images: ['public/favicon.svg'],
})
```

- [ ] **Step 2: 运行图标生成脚本**

Run：
```bash
cd web && pnpm run generate-pwa-assets
```
Expected：在 `web/public/` 生成六个图标文件，控制台打印每个文件的路径和尺寸。

- [ ] **Step 3: 验证生成文件**

Run：
```bash
ls -la web/public/
```
Expected 文件存在：
- `pwa-64x64.png`
- `pwa-192x192.png`
- `pwa-512x512.png`
- `maskable-icon-512x512.png`
- `apple-touch-icon-180x180.png`
- `favicon.ico`

- [ ] **Step 4: Commit**

```bash
git add web/pwa-assets.config.ts web/public/pwa-*.png web/public/maskable-icon-*.png web/public/apple-touch-icon-*.png web/public/favicon.ico
git commit -m "feat(web): generate PWA icon assets from favicon.svg"
```

---

## Task 3: Service Worker 注册模块（TDD）

**Files:**
- Create: `web/src/pwa/register.ts`
- Create: `web/src/pwa/register.test.ts`
- Modify: `web/src/env.d.ts`（补虚拟模块声明）

- [ ] **Step 1: 写失败测试 `web/src/pwa/register.test.ts`**

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const registerSW = vi.fn()

vi.mock('virtual:pwa-register', () => ({
  registerSW: (options?: unknown) => registerSW(options),
}))

describe('registerServiceWorker', () => {
  beforeEach(() => {
    registerSW.mockReset()
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllEnvs()
    vi.resetModules()
  })

  it('does not register SW in DEV mode', async () => {
    vi.stubEnv('DEV', true)
    const { registerServiceWorker } = await import('./register')
    registerServiceWorker()
    expect(registerSW).not.toHaveBeenCalled()
  })

  it('registers SW in PROD mode with immediate: true', async () => {
    vi.stubEnv('DEV', false)
    const { registerServiceWorker } = await import('./register')
    registerServiceWorker()
    expect(registerSW).toHaveBeenCalledTimes(1)
    const options = registerSW.mock.calls[0][0] as { immediate: boolean }
    expect(options.immediate).toBe(true)
  })

  it('installs hourly update timer via onRegisteredSW', async () => {
    vi.stubEnv('DEV', false)
    const { registerServiceWorker } = await import('./register')
    registerServiceWorker()

    const options = registerSW.mock.calls[0][0] as {
      onRegisteredSW: (
        url: string,
        reg: { update: () => Promise<void> } | undefined,
      ) => void
    }
    const update = vi.fn()
    options.onRegisteredSW('/sw.js', { update })

    vi.advanceTimersByTime(60 * 60 * 1000 - 1)
    expect(update).not.toHaveBeenCalled()
    vi.advanceTimersByTime(1)
    expect(update).toHaveBeenCalledTimes(1)
  })

  it('tolerates missing registration in onRegisteredSW', async () => {
    vi.stubEnv('DEV', false)
    const { registerServiceWorker } = await import('./register')
    registerServiceWorker()

    const options = registerSW.mock.calls[0][0] as {
      onRegisteredSW: (url: string, reg: undefined) => void
    }
    expect(() => options.onRegisteredSW('/sw.js', undefined)).not.toThrow()
  })
})
```

- [ ] **Step 2: 运行测试确认失败**

Run：
```bash
cd web && pnpm run test -- src/pwa/register.test.ts
```
Expected：FAIL，报错类似 `Cannot find module './register'` 或 `Failed to resolve import 'virtual:pwa-register'`（后者需要先确保 Vitest 能解析 mock；由 `vi.mock` 覆盖，不需要真实模块存在）。

- [ ] **Step 3: 若 `virtual:pwa-register` 类型缺失，补 `web/src/env.d.ts`**

先读现有 `web/src/env.d.ts`，在末尾追加：

```ts
/// <reference types="vite-plugin-pwa/client" />
```

- [ ] **Step 4: 实现 `web/src/pwa/register.ts`**

```ts
import { registerSW } from 'virtual:pwa-register'

const UPDATE_CHECK_INTERVAL_MS = 60 * 60 * 1000

export function registerServiceWorker(): void {
  if (import.meta.env.DEV) return

  registerSW({
    immediate: true,
    onRegisteredSW(_swUrl, registration) {
      if (registration) {
        setInterval(() => {
          void registration.update()
        }, UPDATE_CHECK_INTERVAL_MS)
      }
    },
  })
}
```

- [ ] **Step 5: 运行测试确认通过**

Run：
```bash
cd web && pnpm run test -- src/pwa/register.test.ts
```
Expected：所有 4 个测试 PASS。

- [ ] **Step 6: Commit**

```bash
git add web/src/pwa/register.ts web/src/pwa/register.test.ts web/src/env.d.ts
git commit -m "feat(web): add service worker registration module with silent update"
```

---

## Task 4: 接入 VitePWA 插件

**Files:**
- Modify: `web/vite.config.ts`

- [ ] **Step 1: 读取当前 `web/vite.config.ts`**

当前内容（供参考）：
```ts
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  // ...
})
```

- [ ] **Step 2: 改写 `web/vite.config.ts`，在 `plugins` 中追加 `VitePWA`**

完整新内容：
```ts
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { VitePWA } from 'vite-plugin-pwa'
import path from 'path'

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    VitePWA({
      registerType: 'autoUpdate',
      injectRegister: false,
      workbox: {
        skipWaiting: true,
        clientsClaim: false,
        navigateFallback: 'index.html',
        navigateFallbackDenylist: [/^\/api\//, /^\/feed\//, /^\/metrics/],
        globPatterns: ['**/*.{js,css,html,woff2,svg,png,ico,webmanifest}'],
        runtimeCaching: [],
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
        icons: [
          { src: 'pwa-64x64.png', sizes: '64x64', type: 'image/png' },
          { src: 'pwa-192x192.png', sizes: '192x192', type: 'image/png' },
          { src: 'pwa-512x512.png', sizes: '512x512', type: 'image/png' },
          {
            src: 'maskable-icon-512x512.png',
            sizes: '512x512',
            type: 'image/png',
            purpose: 'maskable',
          },
        ],
      },
    }),
  ],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    proxy: {
      '/api': 'http://localhost:3330',
      '/feed': 'http://localhost:3330',
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: './src/test-setup.ts',
  },
})
```

- [ ] **Step 3: 运行前端 build 确认产物正确**

Run：
```bash
cd web && pnpm run build
```
Expected：构建成功；`web/dist/` 下出现：
- `sw.js`
- `workbox-*.js`
- `manifest.webmanifest`
- `registerSW.js`（由插件生成，即便我们 `injectRegister: false` 也会产出给 virtual 模块使用）
- 所有图标

检查：
```bash
ls web/dist/*.js web/dist/*.webmanifest
```

- [ ] **Step 4: 运行现有测试确保未回归**

Run：
```bash
cd web && pnpm run test
```
Expected：所有测试（含 Task 3 的新测试）PASS。

- [ ] **Step 5: Commit**

```bash
git add web/vite.config.ts
git commit -m "feat(web): configure VitePWA plugin with app shell precache"
```

---

## Task 5: 接线 HTML 与入口文件

**Files:**
- Modify: `web/index.html`
- Modify: `web/src/main.tsx`

- [ ] **Step 1: 修改 `web/index.html`，在 `<head>` 的 `<meta name="theme-color">` 之后新增两行**

原段落：
```html
<meta name="theme-color" content="#fefcf3" />
<link rel="preconnect" href="https://fonts.googleapis.com">
```

改为：
```html
<meta name="theme-color" content="#fefcf3" />
<link rel="manifest" href="/manifest.webmanifest" />
<link rel="apple-touch-icon" href="/apple-touch-icon-180x180.png" />
<link rel="preconnect" href="https://fonts.googleapis.com">
```

（保留 `<link rel="apple-touch-icon" href="/favicon.svg" />` 的同名 link 会冲突；此处 Vite PWA 插件会自动 inject apple-touch-icon，但我们显式控制以保证指向 PNG。**删除** `index.html` 中原有的 `<link rel="apple-touch-icon" href="/favicon.svg" />`。）

- [ ] **Step 2: 修改 `web/src/main.tsx`，在渲染之后调用 `registerServiceWorker()`**

完整新内容：
```tsx
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
import { registerServiceWorker } from './pwa/register'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)

registerServiceWorker()
```

- [ ] **Step 3: 运行 build 验证产物**

Run：
```bash
cd web && pnpm run build
```
Expected：构建成功；`web/dist/index.html` 含 `<link rel="manifest">`；产物里包含 `sw.js` 注册逻辑。

- [ ] **Step 4: Commit**

```bash
git add web/index.html web/src/main.tsx
git commit -m "feat(web): wire PWA manifest and SW registration into app shell"
```

---

## Task 6: 后端纯函数 `cache_control_for`（TDD）

**Files:**
- Modify: `src/spa.rs`

- [ ] **Step 1: 在 `src/spa.rs` 末尾新增失败的单元测试模块**

追加到 `src/spa.rs` 末尾：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_control_for_sw_js_is_no_cache() {
        assert_eq!(cache_control_for("sw.js"), "no-cache");
    }

    #[test]
    fn cache_control_for_register_sw_js_is_no_cache() {
        assert_eq!(cache_control_for("registerSW.js"), "no-cache");
    }

    #[test]
    fn cache_control_for_workbox_runtime_is_no_cache() {
        assert_eq!(cache_control_for("workbox-abc123.js"), "no-cache");
    }

    #[test]
    fn cache_control_for_manifest_is_no_cache() {
        assert_eq!(cache_control_for("manifest.webmanifest"), "no-cache");
    }

    #[test]
    fn cache_control_for_index_html_is_no_cache() {
        assert_eq!(cache_control_for("index.html"), "no-cache");
    }

    #[test]
    fn cache_control_for_hashed_asset_is_immutable() {
        assert_eq!(
            cache_control_for("assets/index-abc123.js"),
            "public, max-age=31536000, immutable"
        );
    }

    #[test]
    fn cache_control_for_favicon_is_one_day() {
        assert_eq!(cache_control_for("favicon.svg"), "public, max-age=86400");
    }

    #[test]
    fn cache_control_for_pwa_png_is_one_day() {
        assert_eq!(cache_control_for("pwa-512x512.png"), "public, max-age=86400");
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run：
```bash
cargo test --lib spa::tests
```
Expected：FAIL，编译错误 `cannot find function \`cache_control_for\` in this scope`。

- [ ] **Step 3: 实现 `cache_control_for` 纯函数**

在 `src/spa.rs` 中 `Asset` 结构体定义之后（`spa_handler` 之前）追加：

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
```

- [ ] **Step 4: 运行测试确认通过**

Run：
```bash
cargo test --lib spa::tests
```
Expected：8 个测试全部 PASS。

- [ ] **Step 5: Commit**

```bash
git add src/spa.rs
git commit -m "feat(spa): add cache_control_for path-based policy helper"
```

---

## Task 7: 后端纯函数 `is_static_asset_path`（TDD）

**Files:**
- Modify: `src/spa.rs`

- [ ] **Step 1: 在 `src/spa.rs` 的 `tests` 模块内追加失败测试**

```rust
    #[test]
    fn is_static_asset_js() {
        assert!(is_static_asset_path("sw.js"));
        assert!(is_static_asset_path("assets/index-abc.js"));
    }

    #[test]
    fn is_static_asset_mjs() {
        assert!(is_static_asset_path("assets/foo.mjs"));
    }

    #[test]
    fn is_static_asset_css() {
        assert!(is_static_asset_path("assets/style-xyz.css"));
    }

    #[test]
    fn is_static_asset_webmanifest() {
        assert!(is_static_asset_path("manifest.webmanifest"));
    }

    #[test]
    fn is_static_asset_source_map() {
        assert!(is_static_asset_path("sw.js.map"));
    }

    #[test]
    fn is_static_asset_images_and_fonts() {
        for p in [
            "pwa-512x512.png",
            "icon.jpg",
            "pic.jpeg",
            "anim.gif",
            "modern.webp",
            "favicon.svg",
            "favicon.ico",
            "Inter.woff",
            "Inter.woff2",
            "Noto.ttf",
        ] {
            assert!(is_static_asset_path(p), "expected {p} to be static asset");
        }
    }

    #[test]
    fn is_static_asset_json_and_txt_and_xml() {
        assert!(is_static_asset_path("data.json"));
        assert!(is_static_asset_path("robots.txt"));
        assert!(is_static_asset_path("sitemap.xml"));
    }

    #[test]
    fn is_not_static_asset_bare_route() {
        assert!(!is_static_asset_path("articles/xyz"));
        assert!(!is_static_asset_path("login"));
        assert!(!is_static_asset_path(""));
    }

    #[test]
    fn is_not_static_asset_unknown_extension() {
        assert!(!is_static_asset_path("foo.bar"));
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run：
```bash
cargo test --lib spa::tests
```
Expected：FAIL，`cannot find function \`is_static_asset_path\``。

- [ ] **Step 3: 实现函数**

在 `cache_control_for` 之后追加：

```rust
fn is_static_asset_path(path: &str) -> bool {
    let ext = match path.rsplit('.').next() {
        Some(ext) if ext != path => ext, // 有 `.` 才算后缀
        _ => return false,
    };
    matches!(
        ext,
        "js" | "mjs"
            | "css"
            | "map"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "svg"
            | "ico"
            | "woff"
            | "woff2"
            | "ttf"
            | "webmanifest"
            | "json"
            | "txt"
            | "xml"
    )
}
```

- [ ] **Step 4: 运行测试确认通过**

Run：
```bash
cargo test --lib spa::tests
```
Expected：全部 PASS（含前一步的 8 个 + 本任务的 9 个 = 17 个）。

- [ ] **Step 5: Commit**

```bash
git add src/spa.rs
git commit -m "feat(spa): add is_static_asset_path extension classifier"
```

---

## Task 8: 重构 `spa_handler` 支持 ETag / 304 / 分路径 Cache-Control / 静态资源 404

**Files:**
- Modify: `src/spa.rs`

- [ ] **Step 1: 读取当前 `src/spa.rs`（供 diff 参考）**

Run：
```bash
cat src/spa.rs
```

- [ ] **Step 2: 重写 `spa_handler` 主体（同时保留 Task 6/7 的辅助函数与 tests 模块）**

完整新文件内容：

```rust
use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::{Embed, EmbeddedFile};

#[derive(Embed)]
#[folder = "web/dist"]
struct Asset;

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
    let ext = match path.rsplit('.').next() {
        Some(ext) if ext != path => ext,
        _ => return false,
    };
    matches!(
        ext,
        "js" | "mjs"
            | "css"
            | "map"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "svg"
            | "ico"
            | "woff"
            | "woff2"
            | "ttf"
            | "webmanifest"
            | "json"
            | "txt"
            | "xml"
    )
}

fn etag_of(file: &EmbeddedFile) -> String {
    let hash = file.metadata.sha256_hash();
    format!("\"{}\"", hex::encode(hash))
}

fn build_asset_response(
    path: &str,
    file: EmbeddedFile,
    req_headers: &HeaderMap,
) -> Response {
    let etag = etag_of(&file);
    let cache_control = cache_control_for(path);

    // 304 short-circuit
    if let Some(if_none_match) = req_headers.get(header::IF_NONE_MATCH) {
        if if_none_match.as_bytes() == etag.as_bytes() {
            let mut resp = Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .body(Body::empty())
                .unwrap();
            let h = resp.headers_mut();
            h.insert(header::ETAG, HeaderValue::from_str(&etag).unwrap());
            h.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static(cache_control),
            );
            return resp;
        }
    }

    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(file.data.into_owned()))
        .unwrap();
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref()).unwrap(),
    );
    h.insert(header::ETAG, HeaderValue::from_str(&etag).unwrap());
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control),
    );
    resp
}

pub async fn spa_handler(uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');

    // 精确命中
    if let Some(file) = Asset::get(path) {
        return build_asset_response(path, file, &headers);
    }

    // 静态资源后缀但文件不存在：404，不 fallback 到 HTML
    if is_static_asset_path(path) {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }

    // SPA fallback：index.html（同样带 no-cache + ETag）
    match Asset::get("index.html") {
        Some(file) => build_asset_response("index.html", file, &headers),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Task 6 的 cache_control_for 测试（保留原样）
    #[test]
    fn cache_control_for_sw_js_is_no_cache() {
        assert_eq!(cache_control_for("sw.js"), "no-cache");
    }

    #[test]
    fn cache_control_for_register_sw_js_is_no_cache() {
        assert_eq!(cache_control_for("registerSW.js"), "no-cache");
    }

    #[test]
    fn cache_control_for_workbox_runtime_is_no_cache() {
        assert_eq!(cache_control_for("workbox-abc123.js"), "no-cache");
    }

    #[test]
    fn cache_control_for_manifest_is_no_cache() {
        assert_eq!(cache_control_for("manifest.webmanifest"), "no-cache");
    }

    #[test]
    fn cache_control_for_index_html_is_no_cache() {
        assert_eq!(cache_control_for("index.html"), "no-cache");
    }

    #[test]
    fn cache_control_for_hashed_asset_is_immutable() {
        assert_eq!(
            cache_control_for("assets/index-abc123.js"),
            "public, max-age=31536000, immutable"
        );
    }

    #[test]
    fn cache_control_for_favicon_is_one_day() {
        assert_eq!(cache_control_for("favicon.svg"), "public, max-age=86400");
    }

    #[test]
    fn cache_control_for_pwa_png_is_one_day() {
        assert_eq!(
            cache_control_for("pwa-512x512.png"),
            "public, max-age=86400"
        );
    }

    // Task 7 的 is_static_asset_path 测试
    #[test]
    fn is_static_asset_js() {
        assert!(is_static_asset_path("sw.js"));
        assert!(is_static_asset_path("assets/index-abc.js"));
    }

    #[test]
    fn is_static_asset_mjs() {
        assert!(is_static_asset_path("assets/foo.mjs"));
    }

    #[test]
    fn is_static_asset_css() {
        assert!(is_static_asset_path("assets/style-xyz.css"));
    }

    #[test]
    fn is_static_asset_webmanifest() {
        assert!(is_static_asset_path("manifest.webmanifest"));
    }

    #[test]
    fn is_static_asset_source_map() {
        assert!(is_static_asset_path("sw.js.map"));
    }

    #[test]
    fn is_static_asset_images_and_fonts() {
        for p in [
            "pwa-512x512.png",
            "icon.jpg",
            "pic.jpeg",
            "anim.gif",
            "modern.webp",
            "favicon.svg",
            "favicon.ico",
            "Inter.woff",
            "Inter.woff2",
            "Noto.ttf",
        ] {
            assert!(is_static_asset_path(p), "expected {p} to be static asset");
        }
    }

    #[test]
    fn is_static_asset_json_and_txt_and_xml() {
        assert!(is_static_asset_path("data.json"));
        assert!(is_static_asset_path("robots.txt"));
        assert!(is_static_asset_path("sitemap.xml"));
    }

    #[test]
    fn is_not_static_asset_bare_route() {
        assert!(!is_static_asset_path("articles/xyz"));
        assert!(!is_static_asset_path("login"));
        assert!(!is_static_asset_path(""));
    }

    #[test]
    fn is_not_static_asset_unknown_extension() {
        assert!(!is_static_asset_path("foo.bar"));
    }
}
```

- [ ] **Step 3: 确保 `Cargo.toml` 中 `hex` 在 `[dependencies]` 中（已有，无需改动）**

Run：
```bash
grep '^hex' Cargo.toml
```
Expected：`hex = "0.4"`。

- [ ] **Step 4: 编译并运行单元测试**

Run：
```bash
cargo build
cargo test --lib spa::tests
```
Expected：编译通过；17 个单元测试全部 PASS。

- [ ] **Step 5: 运行整个 lib 测试套件确认无回归**

Run：
```bash
cargo test --lib
```
Expected：全部 PASS。

- [ ] **Step 6: Commit**

```bash
git add src/spa.rs
git commit -m "feat(spa): add ETag/304 + path-based Cache-Control + 404 for missing static assets"
```

---

## Task 9: 集成测试 `spa_handler` HTTP 行为

**Files:**
- Create: `tests/integration_spa.rs`

- [ ] **Step 1: 查看 Docker 容器里跑前端 build 的路径，确认 `web/dist/` 在本次 task 执行时存在**

Run：
```bash
ls web/dist/sw.js web/dist/index.html web/dist/manifest.webmanifest
```
Expected：三个文件都存在（来自 Task 5 的 build 产物）。若缺失，先 `cd web && pnpm run build`。

- [ ] **Step 2: 创建 `tests/integration_spa.rs`**

```rust
mod common;

#[tokio::test]
async fn sw_js_served_with_no_cache_and_etag() {
    let app = common::TestApp::new().await;
    let res = app.client.get(app.url("/sw.js")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers()
            .get("cache-control")
            .unwrap()
            .to_str()
            .unwrap(),
        "no-cache"
    );
    assert!(
        res.headers().get("etag").is_some(),
        "sw.js response must include an ETag header"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn sw_js_returns_304_on_if_none_match() {
    let app = common::TestApp::new().await;
    let first = app.client.get(app.url("/sw.js")).send().await.unwrap();
    let etag = first.headers().get("etag").unwrap().to_str().unwrap().to_string();

    let second = app
        .client
        .get(app.url("/sw.js"))
        .header("If-None-Match", &etag)
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), 304);
    assert_eq!(
        second.headers().get("cache-control").unwrap().to_str().unwrap(),
        "no-cache"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn index_html_served_with_no_cache() {
    let app = common::TestApp::new().await;
    let res = app.client.get(app.url("/")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers().get("cache-control").unwrap().to_str().unwrap(),
        "no-cache"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn manifest_served_with_no_cache() {
    let app = common::TestApp::new().await;
    let res = app
        .client
        .get(app.url("/manifest.webmanifest"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers().get("cache-control").unwrap().to_str().unwrap(),
        "no-cache"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn missing_js_returns_404_not_spa_fallback() {
    let app = common::TestApp::new().await;
    let res = app
        .client
        .get(app.url("/definitely-not-here.js"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
    let content_type = res
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    assert!(
        !content_type.contains("text/html"),
        "missing JS must not fall back to HTML"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn spa_route_falls_back_to_index_html() {
    let app = common::TestApp::new().await;
    let res = app
        .client
        .get(app.url("/articles/some-spa-route"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    assert!(
        body.contains("<div id=\"root\">") || body.contains("<div id='root'>"),
        "SPA fallback should return index.html shell"
    );
    app.cleanup().await;
}
```

- [ ] **Step 3: 运行集成测试**

Run：
```bash
cargo test --test integration_spa
```
Expected：6 个测试全部 PASS。（前提：`web/dist/` 已 build，`rust-embed` 嵌入成功。）

- [ ] **Step 4: 运行完整测试套件确保无回归**

Run：
```bash
cargo test
```
Expected：所有已有测试 + 新测试 PASS。

- [ ] **Step 5: Commit**

```bash
git add tests/integration_spa.rs
git commit -m "test(spa): cover SW/manifest/HTML cache headers and 404-for-static"
```

---

## Task 10: Docker 构建端到端验证 + 手动验证清单归档

**Files:**
- Modify: `docs/specs/2026-04-21-pwa-support-design.md`（补 "实施完成" 状态）

- [ ] **Step 1: Docker 重建 + 启动**

Run：
```bash
./dev.sh build
```
Expected：镜像构建成功；容器启动。

- [ ] **Step 2: 自动化 smoke check**

Run（容器启动后）：
```bash
curl -sI http://localhost:3330/sw.js | grep -i 'cache-control\|etag'
curl -sI http://localhost:3330/manifest.webmanifest | grep -i 'cache-control'
curl -sI http://localhost:3330/ | grep -i 'cache-control'
curl -s -o /dev/null -w '%{http_code}\n' http://localhost:3330/definitely-not-here.js
```
Expected：
- `/sw.js` → `Cache-Control: no-cache` + `ETag: "<hex>"`
- `/manifest.webmanifest` → `Cache-Control: no-cache`
- `/` → `Cache-Control: no-cache`
- `/definitely-not-here.js` → `404`

- [ ] **Step 3: 浏览器人工验证**

在 Chrome 打开 `http://localhost:3330`，执行 spec 第 7.2 节的手动验证清单：

1. DevTools → Application → Manifest 无报错；地址栏右侧出现安装按钮。
2. DevTools → Application → Service Workers 显示 `activated and running`，Cache Storage 出现 `workbox-precache-*`。
3. **静默更新测试**：
   - 当前页面不刷新
   - 修改 `web/src/App.tsx` 改一行可见文案（如页面标题）
   - `./dev.sh build` 重建镜像（会重启容器）
   - 回到浏览器，DevTools → Application → Service Workers 点 **Update**
   - 确认新 SW 状态流转：`installed` → `activated`，且**当前页面仍显示旧文案**
   - 手动点击侧栏其他路由 → 页面切换后显示**新文案**
4. Network 面板确认：
   - `sw.js` 响应头 `Cache-Control: no-cache`
   - Application → Service Workers 勾选 Offline → 访问 `/api/v1/me` → Network 显示网络失败（不是从缓存返回）
5. 恢复前一步改动：再改一次文案，再重建，验证第三次部署后 Cache Storage 只剩最新版本条目

将验证结果（通过/失败 + 截图或简述）追加到 `docs/specs/2026-04-21-pwa-support-design.md` 末尾的新章节 `## 9. 实施记录（2026-04-21）`。

- [ ] **Step 4: Commit 验证记录**

```bash
git add docs/specs/2026-04-21-pwa-support-design.md
git commit -m "docs(pwa): record manual verification results"
```

- [ ] **Step 5: 最终完整回归**

Run：
```bash
cargo test
cd web && pnpm run test && pnpm run build
```
Expected：全部 PASS，无警告回归。

---

## Self-Review Notes

- **Spec 覆盖**：Task 1-5 覆盖 spec §4（前端组件）+ §2（关键决策 D1/D2/D3/D7/D8）；Task 6-8 覆盖 §5（后端缓存头 + ETag + 404-for-static），即 D4/D5/D6；Task 9 覆盖 §7.1 自动化测试；Task 10 覆盖 §7.2 手动验证。范围外条目（离线 API、推送）在 Task 里未出现，符合预期。
- **类型一致性**：`cache_control_for` / `is_static_asset_path` / `etag_of` / `build_asset_response` 签名在 Task 6/7/8 之间一致。`EmbeddedFile::metadata.sha256_hash()` 是 rust-embed 8 的已知 API，返回 `[u8; 32]`，用 `hex::encode` 转字符串。
- **TDD 顺序**：Tasks 3、6、7 严格 Red → Green → Commit；Task 8 因为要把两个纯函数搬进同一个 handler，单独跑 Task 8 Step 2 的 cargo build 可能报错——已用 Step 4 的 `cargo build` 保证每次 commit 可编译。
- **提交原子性**：每个 Task 一次 commit，代码自含可编译。
