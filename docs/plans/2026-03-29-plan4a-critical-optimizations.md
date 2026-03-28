# Plan 4a: 关键优化（安全、稳定性、运维）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实施 7 项 P0/P1 优化，覆盖安全校验、安全响应头、Token 竞态修复、ErrorBoundary、健康检查端点、Feed Token 轮换、文档更新。

**Architecture:** 所有 7 个 Task 互相独立，无依赖关系，可并行执行。后端改动涉及 `config.rs`、`main.rs`、`api/mod.rs`、`api/auth.rs`、`api/health.rs`（新建）、`models/user.rs`。前端改动涉及 `client.ts`、`ErrorBoundary.tsx`（新建）、`App.tsx`、`Layout.tsx`。

**Tech Stack:** Rust (Axum, tower-http, SQLx), React 19, TypeScript, Axios

**Spec:** `docs/specs/2026-03-29-optimization-design.md` A1-A7

---

## File Structure

| Action | Path | Responsibility |
|--------|------|---------------|
| Modify | `CLAUDE.md` | 更新项目状态文档 |
| Modify | `src/config.rs` | JWT secret 校验，返回 Result |
| Modify | `src/main.rs` | 处理 Config Result + 安全头中间件 |
| Modify | `Cargo.toml` | tower-http 添加 set-header feature |
| Modify | `src/api/mod.rs` | 注册 health + feed-token 路由 |
| Create | `src/api/health.rs` | 健康检查端点 |
| Modify | `src/api/auth.rs` | regenerate-feed-token handler |
| Modify | `src/models/user.rs` | regenerate_feed_token 函数 |
| Modify | `docker-compose.yml` | 更新 healthcheck |
| Modify | `web/src/api/client.ts` | Token refresh 竞态修复 |
| Create | `web/src/components/ErrorBoundary.tsx` | 错误边界组件 |
| Modify | `web/src/App.tsx` | 顶级 ErrorBoundary + lazy import |
| Modify | `web/src/components/Layout.tsx` | 页面级 ErrorBoundary |
| Modify | `tests/common/mod.rs` | TestApp Config 适配 |

---

### Task 1: 更新 CLAUDE.md 项目状态 [A1]

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: 更新当前状态段落**

```markdown
## 当前状态

项目 Plan 1（内容提取 PoC）至 Plan 3b（全文搜索）已全部完成。当前正在执行 **Plan 4: 项目优化**，参见 `docs/specs/2026-03-29-optimization-design.md`。
```

- [ ] **Step 2: 更新路线图表格**

```markdown
## 实施计划路线图

| 计划 | 内容 | 状态 |
|------|------|------|
| Plan 1 | 项目脚手架 + 内容提取 PoC | ✅ 已完成 |
| Plan 2a | 数据库 + 认证系统 | ✅ 已完成 |
| Plan 2b | Entry CRUD + 抓取队列 | ✅ 已完成 |
| Plan 3a | Tags, Annotations, Memos | ✅ 已完成 |
| Plan 3b | 全文搜索 (tantivy) | ✅ 已完成 |
| Plan 4 | 项目优化（安全、性能、可观测性） | 🔄 执行中 |
| Plan 5 | 前端 SPA 优化 + 浏览器扩展改进 | 待编写 |
```

- [ ] **Step 3: 确认无其他过时内容**

全文检查 `CLAUDE.md`，确保没有其他引用 "尚未开始编码" 或 "待执行" 的段落。

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md to reflect current project status (Plan 1-3b complete)"
```

---

### Task 2: JWT Secret 启动校验 [A2]

**Files:**
- Modify: `src/config.rs`
- Modify: `src/main.rs`
- Modify: `tests/common/mod.rs`

- [ ] **Step 1: 写 Config::from_env 返回 Result 的测试**

在 `src/config.rs` 底部添加测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_jwt_secret() {
        env::set_var("DATABASE_URL", "postgres://test");
        env::set_var("JWT_SECRET", "too-short");
        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 32 characters"));
        env::remove_var("JWT_SECRET");
        env::remove_var("DATABASE_URL");
    }

    #[test]
    fn rejects_default_jwt_secret() {
        env::set_var("DATABASE_URL", "postgres://test");
        env::set_var("JWT_SECRET", "change-me-in-production");
        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("default"));
        env::remove_var("JWT_SECRET");
        env::remove_var("DATABASE_URL");
    }

    #[test]
    fn accepts_valid_jwt_secret() {
        env::set_var("DATABASE_URL", "postgres://test");
        env::set_var("JWT_SECRET", "a]3kf9$mP!qR7vLx2Yw8Hn5Bc6Tj4Ud0Ze");
        let result = Config::from_env();
        assert!(result.is_ok());
        env::remove_var("JWT_SECRET");
        env::remove_var("DATABASE_URL");
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test config::tests -- --test-threads=1`
Expected: FAIL — `from_env()` 当前返回 `Self` 不是 `Result`

- [ ] **Step 3: 修改 Config::from_env 返回 Result**

将 `src/config.rs` 的 `from_env` 改为：

```rust
impl Config {
    pub fn from_env() -> Result<Self, String> {
        let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");

        // Validate JWT secret
        if jwt_secret.len() < 32 {
            return Err(format!(
                "JWT_SECRET must be at least 32 characters (got {})",
                jwt_secret.len()
            ));
        }
        if jwt_secret == "change-me-in-production" {
            return Err("JWT_SECRET is still the default value — change it before running in production".to_string());
        }

        Ok(Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            jwt_secret,
            listen_addr: env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            index_path: env::var("INDEX_PATH").unwrap_or_else(|_| "/data/tantivy".to_string()),
            storage_type: env::var("STORAGE_TYPE").unwrap_or_else(|_| "local".to_string()),
            storage_local_path: env::var("STORAGE_LOCAL_PATH").unwrap_or_else(|_| "/data/storage".to_string()),
            oss_endpoint: env::var("OSS_ENDPOINT").unwrap_or_default(),
            oss_region: env::var("OSS_REGION").unwrap_or_else(|_| "auto".to_string()),
            oss_bucket: env::var("OSS_BUCKET").unwrap_or_default(),
            oss_access_key: env::var("OSS_ACCESS_KEY").unwrap_or_default(),
            oss_secret_key: env::var("OSS_SECRET_KEY").unwrap_or_default(),
            oss_public_url: env::var("OSS_PUBLIC_URL").unwrap_or_default(),
        })
    }
}
```

- [ ] **Step 4: 更新 main.rs 处理 Result**

将 `src/main.rs` 第 10 行从：
```rust
let config = lettura::config::Config::from_env();
```
改为：
```rust
let config = lettura::config::Config::from_env().unwrap_or_else(|e| {
    eprintln!("Configuration error: {e}");
    std::process::exit(1);
});
```

- [ ] **Step 5: 更新 TestApp 的 Config 构造**

`tests/common/mod.rs` 中 TestApp::new() 直接构造 `Config { .. }`，不走 `from_env()`，因此不受影响。确认编译通过即可。

- [ ] **Step 6: 运行测试确认通过**

Run: `cargo test config::tests -- --test-threads=1`
Expected: 3 tests PASS

Run: `cargo test`
Expected: 全部 PASS

- [ ] **Step 7: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: validate JWT_SECRET on startup (reject short or default values)"
```

---

### Task 3: 安全响应头 [A3]

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs` (通过 `src/api/mod.rs` 路由层)

- [ ] **Step 1: 写集成测试验证安全头**

在 `tests/integration_security.rs` 中：

```rust
mod common;

#[tokio::test]
async fn responses_include_security_headers() {
    let app = common::TestApp::new().await;

    let res = app.client.get(app.url("/api/auth/login")).send().await.unwrap();

    // The endpoint returns 405 for GET but headers should still be present
    assert_eq!(
        res.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert_eq!(
        res.headers().get("x-frame-options").unwrap(),
        "DENY"
    );
    assert_eq!(
        res.headers().get("referrer-policy").unwrap(),
        "strict-origin-when-cross-origin"
    );

    app.cleanup().await;
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test responses_include_security_headers`
Expected: FAIL — headers not present

- [ ] **Step 3: 添加 tower-http set-header feature**

在 `Cargo.toml` 中修改：
```toml
tower-http = { version = "0.6", features = ["cors", "trace", "set-header"] }
```

- [ ] **Step 4: 在路由上添加安全头中间件**

在 `src/api/mod.rs` 的 `router_with_search` 函数中，在 `.with_state(state)` 之后添加安全头层。在文件顶部添加 import：

```rust
use axum::http::HeaderValue;
use tower_http::set_header::SetResponseHeaderLayer;
```

在 `Router::new()` 链的 `.with_state(state)` 之后，添加：

```rust
        .with_state(state)
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("x-xss-protection"),
            HeaderValue::from_static("1; mode=block"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test responses_include_security_headers`
Expected: PASS

Run: `cargo test`
Expected: 全部 PASS

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/api/mod.rs tests/integration_security.rs
git commit -m "feat: add security response headers (X-Content-Type-Options, X-Frame-Options, etc.)"
```

---

### Task 4: Token 刷新竞态修复 [A4]

**Files:**
- Modify: `web/src/api/client.ts`

- [ ] **Step 1: 替换 client.ts 的 response interceptor**

将 `web/src/api/client.ts` 全部内容替换为：

```typescript
import axios from 'axios';

const api = axios.create({
  baseURL: '/api',
});

api.interceptors.request.use((config) => {
  const token = localStorage.getItem('access_token');
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// Refresh lock: only one refresh request at a time
let refreshPromise: Promise<string> | null = null;
let refreshFailedAt: number = 0;
const REFRESH_COOLDOWN_MS = 5000;

async function doRefresh(): Promise<string> {
  const refreshToken = localStorage.getItem('refresh_token');
  if (!refreshToken) {
    throw new Error('no refresh token');
  }
  const res = await axios.post('/api/auth/refresh', {
    refresh_token: refreshToken,
  });
  const { access_token, refresh_token } = res.data;
  localStorage.setItem('access_token', access_token);
  localStorage.setItem('refresh_token', refresh_token);
  return access_token;
}

api.interceptors.response.use(
  (response) => response,
  async (error) => {
    const originalRequest = error.config;
    if (error.response?.status === 401 && !originalRequest._retry) {
      originalRequest._retry = true;

      // Cooldown check: if refresh failed recently, skip straight to login
      if (Date.now() - refreshFailedAt < REFRESH_COOLDOWN_MS) {
        localStorage.removeItem('access_token');
        localStorage.removeItem('refresh_token');
        window.location.href = '/login';
        return Promise.reject(error);
      }

      try {
        // If a refresh is already in progress, wait for it
        if (!refreshPromise) {
          refreshPromise = doRefresh().finally(() => {
            refreshPromise = null;
          });
        }
        const newToken = await refreshPromise;
        originalRequest.headers.Authorization = `Bearer ${newToken}`;
        return api(originalRequest);
      } catch {
        refreshFailedAt = Date.now();
        localStorage.removeItem('access_token');
        localStorage.removeItem('refresh_token');
        window.location.href = '/login';
      }
    }
    return Promise.reject(error);
  }
);

export default api;
```

- [ ] **Step 2: 前端编译验证**

Run: `cd web && npm run build`
Expected: BUILD SUCCESS（无 TypeScript 错误）

- [ ] **Step 3: Commit**

```bash
git add web/src/api/client.ts
git commit -m "fix: prevent concurrent token refresh race condition with refresh lock"
```

---

### Task 5: 前端 ErrorBoundary [A5]

**Files:**
- Create: `web/src/components/ErrorBoundary.tsx`
- Modify: `web/src/App.tsx`
- Modify: `web/src/components/Layout.tsx`

- [ ] **Step 1: 创建 ErrorBoundary 组件**

创建 `web/src/components/ErrorBoundary.tsx`：

```tsx
import { Component, type ReactNode } from 'react';

interface Props {
  children: ReactNode;
  /** 'app' = top-level (full reload), 'page' = page-level (navigate home) */
  level?: 'app' | 'page';
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export default class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error('ErrorBoundary caught:', error, info.componentStack);
  }

  render() {
    if (!this.state.hasError) {
      return this.props.children;
    }

    const isAppLevel = this.props.level === 'app';

    return (
      <div className="min-h-[300px] flex items-center justify-center p-8">
        <div className="text-center max-w-md">
          <h2 className="text-xl font-semibold text-gray-900 dark:text-gray-100 mb-2">
            出了点问题
          </h2>
          <p className="text-gray-600 dark:text-gray-400 mb-4 text-sm">
            {this.state.error?.message || '发生了未知错误'}
          </p>
          {isAppLevel ? (
            <button
              onClick={() => window.location.reload()}
              className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors text-sm"
            >
              重新加载页面
            </button>
          ) : (
            <a
              href="/"
              className="inline-block px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors text-sm"
            >
              回到首页
            </a>
          )}
        </div>
      </div>
    );
  }
}
```

- [ ] **Step 2: 在 App.tsx 添加顶级 ErrorBoundary**

将 `web/src/App.tsx` 修改为：

```tsx
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import Layout from './components/Layout';
import ProtectedRoute from './components/ProtectedRoute';
import ErrorBoundary from './components/ErrorBoundary';
import LoginPage from './pages/LoginPage';
import RegisterPage from './pages/RegisterPage';
import EntryListPage from './pages/EntryListPage';
import EntryDetailPage from './pages/EntryDetailPage';
import MemosPage from './pages/MemosPage';
import SettingsPage from './pages/SettingsPage';

const queryClient = new QueryClient();

function App() {
  return (
    <ErrorBoundary level="app">
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <Routes>
            <Route path="/login" element={<LoginPage />} />
            <Route path="/register" element={<RegisterPage />} />
            <Route
              path="/"
              element={
                <ProtectedRoute>
                  <Layout />
                </ProtectedRoute>
              }
            >
              <Route index element={<EntryListPage filter="unread" />} />
              <Route path="archived" element={<EntryListPage filter="archived" />} />
              <Route path="starred" element={<EntryListPage filter="starred" />} />
              <Route path="entry/:id" element={<EntryDetailPage />} />
              <Route path="memos" element={<MemosPage />} />
              <Route path="settings" element={<SettingsPage />} />
            </Route>
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </BrowserRouter>
      </QueryClientProvider>
    </ErrorBoundary>
  );
}

export default App;
```

- [ ] **Step 3: 在 Layout.tsx 添加页面级 ErrorBoundary**

在 `web/src/components/Layout.tsx` 中，添加 import 并包裹 `<Outlet />`：

在顶部添加 import：
```tsx
import ErrorBoundary from './ErrorBoundary';
```

将 `<main>` 中的 `<Outlet />` 包裹：
```tsx
      <main className="max-w-6xl mx-auto px-4 py-6">
        <ErrorBoundary level="page">
          <Outlet />
        </ErrorBoundary>
      </main>
```

- [ ] **Step 4: 前端编译验证**

Run: `cd web && npm run build`
Expected: BUILD SUCCESS

- [ ] **Step 5: Commit**

```bash
git add web/src/components/ErrorBoundary.tsx web/src/App.tsx web/src/components/Layout.tsx
git commit -m "feat: add two-layer ErrorBoundary (app-level reload, page-level navigate)"
```

---

### Task 6: 健康检查端点 [A6]

**Files:**
- Create: `src/api/health.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/search.rs`
- Modify: `docker-compose.yml`

- [ ] **Step 1: 写健康检查集成测试**

在 `tests/integration_health.rs` 中：

```rust
mod common;

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let app = common::TestApp::new().await;

    let res = app.client.get(app.url("/api/health")).send().await.unwrap();
    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["db"], "ok");
    // search should also be ok (in-memory index)
    assert_eq!(body["search"], "ok");

    app.cleanup().await;
}

#[tokio::test]
async fn health_endpoint_no_auth_required() {
    let app = common::TestApp::new().await;

    // No Authorization header
    let res = app.client.get(app.url("/api/health")).send().await.unwrap();
    // Should not be 401
    assert_eq!(res.status(), 200);

    app.cleanup().await;
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test health_endpoint`
Expected: FAIL — 404 not found (endpoint doesn't exist)

- [ ] **Step 3: 添加 SearchIndex::doc_count 方法**

在 `src/search.rs` 的 `impl SearchIndex` 中，在 `clear` 方法之前添加：

```rust
    /// Return the number of documents in the index (for health checks)
    pub fn doc_count(&self) -> Result<u64, tantivy::TantivyError> {
        let searcher = self.reader.searcher();
        Ok(searcher.num_docs())
    }
```

- [ ] **Step 4: 创建 health.rs handler**

创建 `src/api/health.rs`：

```rust
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::auth::middleware::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub db: String,
    pub search: String,
}

pub async fn health_check(
    State(state): State<AppState>,
) -> (StatusCode, Json<HealthResponse>) {
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();

    let search_ok = state.search_index.doc_count().is_ok();

    let status = if db_ok && search_ok {
        "ok"
    } else {
        "error"
    };

    let code = if db_ok && search_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        code,
        Json(HealthResponse {
            status: status.to_string(),
            db: if db_ok { "ok".to_string() } else { "error".to_string() },
            search: if search_ok { "ok".to_string() } else { "error".to_string() },
        }),
    )
}
```

- [ ] **Step 5: 注册 health 模块和路由**

在 `src/api/mod.rs` 顶部添加模块声明：
```rust
pub mod health;
```

在 `Router::new()` 链中，在 `// Auth` 注释之前添加：
```rust
        // Health (no auth)
        .route("/api/health", get(health::health_check))
```

- [ ] **Step 6: 运行测试确认通过**

Run: `cargo test health_endpoint`
Expected: 2 tests PASS

Run: `cargo test`
Expected: 全部 PASS

- [ ] **Step 7: 更新 docker-compose.yml healthcheck**

在 `docker-compose.yml` 的 `lettura` service 中添加 healthcheck：

```yaml
  lettura:
    build: .
    ports:
      - "3000:3000"
    environment:
      DATABASE_URL: postgres://lettura:lettura@postgres:5432/lettura
      JWT_SECRET: change-me-in-production
      LISTEN_ADDR: 0.0.0.0:3000
      INDEX_PATH: /data/tantivy
    volumes:
      - lettura_data:/data/tantivy
    depends_on:
      postgres:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/api/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 10s
```

- [ ] **Step 8: Commit**

```bash
git add src/api/health.rs src/api/mod.rs src/search.rs docker-compose.yml tests/integration_health.rs
git commit -m "feat: add /api/health endpoint checking DB and search index status"
```

---

### Task 7: RSS Feed Token 轮换 [A7]

**Files:**
- Modify: `src/models/user.rs`
- Modify: `src/api/auth.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: 写集成测试**

在 `tests/integration_auth.rs` 底部添加（如果文件末尾有 `}` 结尾注意定位）：

```rust
#[tokio::test]
async fn regenerate_feed_token() {
    let app = common::TestApp::new().await;

    // Register
    let res = app
        .client
        .post(app.url("/api/auth/register"))
        .json(&serde_json::json!({
            "username": "feeduser",
            "email": "feed@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    let auth: serde_json::Value = res.json().await.unwrap();
    let token = auth["access_token"].as_str().unwrap();

    // Get old feed token from DB directly
    let old_token: (String,) = sqlx::query_as("SELECT feed_token FROM users WHERE email = 'feed@example.com'")
        .fetch_one(&app.pool)
        .await
        .unwrap();

    // Regenerate
    let res = app
        .client
        .post(app.url("/api/auth/regenerate-feed-token"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let new_token = body["feed_token"].as_str().unwrap();

    // Verify token changed
    assert_ne!(old_token.0, new_token);
    assert_eq!(new_token.len(), 64); // 32 bytes hex = 64 chars

    app.cleanup().await;
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test regenerate_feed_token`
Expected: FAIL — 404 (endpoint doesn't exist)

- [ ] **Step 3: 添加 regenerate_feed_token model 函数**

在 `src/models/user.rs` 底部（`delete_refresh_token` 函数之后）添加：

```rust
pub async fn regenerate_feed_token(pool: &PgPool, user_id: Uuid) -> Result<String, ApiError> {
    let new_token = generate_feed_token();
    let row: (String,) = sqlx::query_as(
        "UPDATE users SET feed_token = $1 WHERE id = $2 RETURNING feed_token",
    )
    .bind(&new_token)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(row.0)
}

fn generate_feed_token() -> String {
    use rand::Rng;
    let bytes: Vec<u8> = (0..32).map(|_| rand::thread_rng().gen::<u8>()).collect();
    hex::encode(bytes)
}
```

- [ ] **Step 4: 添加 regenerate-feed-token handler**

在 `src/api/auth.rs` 底部添加：

```rust
#[derive(Serialize)]
pub struct FeedTokenResponse {
    pub feed_token: String,
}

pub async fn regenerate_feed_token(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<FeedTokenResponse>, ApiError> {
    let new_token = user::regenerate_feed_token(&state.pool, auth.user_id).await?;
    Ok(Json(FeedTokenResponse {
        feed_token: new_token,
    }))
}
```

- [ ] **Step 5: 注册路由**

在 `src/api/mod.rs` 的路由链中，在 `// Entries` 注释之前添加：

```rust
        .route("/api/auth/regenerate-feed-token", post(auth::regenerate_feed_token))
```

- [ ] **Step 6: 运行测试确认通过**

Run: `cargo test regenerate_feed_token`
Expected: PASS

Run: `cargo test`
Expected: 全部 PASS

- [ ] **Step 7: Commit**

```bash
git add src/models/user.rs src/api/auth.rs src/api/mod.rs tests/integration_auth.rs
git commit -m "feat: add POST /api/auth/regenerate-feed-token endpoint"
```

---

## Final Verification

- [ ] **Step 1: 全量后端测试**

Run: `cargo test`
Expected: 全部 PASS

- [ ] **Step 2: 前端编译**

Run: `cd web && npm run build`
Expected: BUILD SUCCESS

- [ ] **Step 3: Docker build 验证**

Run: `docker build -t lettura:test .`
Expected: BUILD SUCCESS（可选，取决于环境）
