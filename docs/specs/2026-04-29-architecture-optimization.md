# Lettura 架构优化设计

> 创建日期: 2026-04-29
> 状态: ✅ 已完成

## 概述

本文档涵盖五个架构优化方向的设计方案：
1. ✅ 浏览器扩展技术栈升级
2. ✅ 深分页完整解决方案
3. ✅ 图片处理管道优化
4. ✅ 缓存层引入
5. ✅ 结构化日志增强

---

## 1. 浏览器扩展技术栈升级

### 1.1 当前状态

| 项目 | 现状 |
|------|------|
| 语言 | JavaScript (ES6+) |
| 文件结构 | 单文件 (background.js, popup.js) |
| 类型安全 | 无 |
| Token Refresh | 有竞态问题（并发请求时可能多次 refresh） |
| 构建工具 | 无 |

### 1.2 目标

- 迁移到 TypeScript
- 解决 Token Refresh 竞态问题
- 添加离线队列支持
- 改善开发体验（类型检查、构建优化）

### 1.3 技术方案

#### 1.3.1 项目结构

```
extension/
├── src/
│   ├── background/
│   │   ├── index.ts          # Service Worker 入口
│   │   ├── auth.ts           # 认证逻辑（含竞态安全的 refresh）
│   │   ├── offline-queue.ts  # 离线队列
│   │   └── context-menu.ts   # 右键菜单
│   ├── popup/
│   │   ├── App.tsx           # React 组件（可选）
│   │   ├── index.tsx
│   │   └── components/
│   ├── shared/
│   │   ├── types.ts          # 共享类型定义
│   │   ├── api.ts            # API 客户端
│   │   └── storage.ts        # Chrome Storage 封装
│   └── manifest.json
├── package.json
├── tsconfig.json
├── vite.config.ts            # Vite 构建
└── README.md
```

#### 1.3.2 Token Refresh 竞态解决方案

```typescript
// src/background/auth.ts
interface RefreshState {
  promise: Promise<string | null> | null;
}

const refreshState: RefreshState = { promise: null };

async function refreshToken(): Promise<string | null> {
  // 如果已有 refresh 进行中，复用该 promise
  if (refreshState.promise) {
    return refreshState.promise;
  }

  refreshState.promise = doRefresh();
  try {
    return await refreshState.promise;
  } finally {
    refreshState.promise = null;
  }
}

async function doRefresh(): Promise<string | null> {
  const { refresh_token } = await getLocalStorage(["refresh_token"]);
  if (!refresh_token) return null;

  try {
    const resp = await apiRequest("POST", "/api/v1/auth/refresh", {
      refresh_token,
    });

    if (!resp.ok) {
      await clearAllStorage();
      return null;
    }

    const data = await resp.json();
    await setSessionStorage({ access_token: data.access_token });
    if (data.refresh_token) {
      await setLocalStorage({ refresh_token: data.refresh_token });
    }
    return data.access_token;
  } catch (err) {
    console.error("Token refresh failed:", err);
    await clearAllStorage();
    return null;
  }
}
```

#### 1.3.3 离线队列设计

```typescript
// src/background/offline-queue.ts
interface QueuedAction {
  id: string;
  type: "save" | "archive" | "star" | "tag";
  payload: unknown;
  createdAt: number;
  retryCount: number;
}

const QUEUE_KEY = "offline_queue";

// 检测在线状态
function isOnline(): boolean {
  return navigator.onLine;
}

// 添加到队列
async function enqueue(action: Omit<QueuedAction, "id" | "createdAt" | "retryCount">): Promise<void> {
  const queue = await getQueue();
  queue.push({
    ...action,
    id: crypto.randomUUID(),
    createdAt: Date.now(),
    retryCount: 0,
  });
  await saveQueue(queue);
}

// 处理队列
async function processQueue(): Promise<void> {
  if (!isOnline()) return;

  const queue = await getQueue();
  const failed: QueuedAction[] = [];

  for (const item of queue) {
    try {
      await executeAction(item);
    } catch (err) {
      item.retryCount++;
      if (item.retryCount < 3) {
        failed.push(item);
      }
    }
  }

  await saveQueue(failed);
}

// 监听在线事件
chrome.runtime.onStartup.addListener(processQueue);
self.addEventListener("online", processQueue);
```

#### 1.3.4 类型定义

```typescript
// src/shared/types.ts
export interface Entry {
  id: string;
  url: string;
  title: string | null;
  domain: string;
  is_archived: boolean;
  is_starred: boolean;
  created_at: string;
  reading_time: number | null;
  preview_picture: string | null;
}

export interface SaveRequest {
  url: string;
  tags?: string[];
}

export interface ApiResponse<T> {
  data: T;
}

export interface AuthTokens {
  access_token: string;
  refresh_token?: string;
}
```

#### 1.3.5 构建配置

```typescript
// vite.config.ts
import { defineConfig } from "vite";
import { crx } from "@crxjs/vite-plugin";
import manifest from "./src/manifest.json";

export default defineConfig({
  plugins: [crx({ manifest })],
  build: {
    outDir: "dist",
    sourcemap: process.env.NODE_ENV === "development",
  },
});
```

### 1.4 迁移步骤

| 步骤 | 内容 | 预估时间 |
|------|------|----------|
| 1 | 初始化 TypeScript 项目，配置 Vite + @crxjs/vite-plugin | 0.5 天 |
| 2 | 迁移 shared/types.ts 和 shared/storage.ts | 0.5 天 |
| 3 | 迁移 auth.ts，实现竞态安全的 refresh | 0.5 天 |
| 4 | 迁移 background/index.ts 和 context-menu.ts | 0.5 天 |
| 5 | 迁移 popup（保持原生 JS 或引入 Preact） | 1 天 |
| 6 | 实现离线队列 | 1 天 |
| 7 | 测试 + 文档 | 0.5 天 |

**总计: 约 4.5 天**

---

## 2. 深分页完整解决方案

### 2.1 当前状态

| 层级 | 现状 |
|------|------|
| 后端 | 支持 offset + cursor 双模式，cursor 已实现但前端未用 |
| 前端 | 无分页 UI，一次性加载，缺少 cursor 参数定义 |
| 限制 | page <= 50 硬限制 |

### 2.2 目标

- 前端使用 cursor 分页 + 无限滚动
- 移除 page <= 50 硬限制（cursor 模式下）
- 提供流畅的浏览体验

### 2.3 技术方案

#### 2.3.1 前端 API 层改造

```typescript
// web/src/api/entries.ts
export interface ListParams {
  cursor?: string;      // 新增
  per_page?: number;
  is_archived?: boolean;
  is_starred?: boolean;
  search?: string;
  domain?: string;
  tags?: string[];
}

export interface ListResponse {
  entries: EntrySummary[];
  next_cursor: string | null;
  has_more: boolean;
}

export async function listEntries(params: ListParams = {}): Promise<ListResponse> {
  const res = await api.get('/entries', {
    params,
    headers: params.cursor ? {} : {}, // cursor 通过 query param 传递
  });

  const nextCursor = res.headers['x-next-cursor'] || null;

  return {
    entries: res.data,
    next_cursor: nextCursor,
    has_more: nextCursor !== null,
  };
}
```

#### 2.3.2 无限滚动 Hook

```typescript
// web/src/hooks/useInfiniteEntries.ts
import { useInfiniteQuery } from '@tanstack/react-query';
import { listEntries, ListParams } from '../api/entries';

export function useInfiniteEntries(baseParams: Omit<ListParams, 'cursor'>) {
  return useInfiniteQuery({
    queryKey: ['entries-infinite', baseParams],
    queryFn: ({ pageParam }) => listEntries({ ...baseParams, cursor: pageParam }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (lastPage) => lastPage.next_cursor ?? undefined,
    getPreviousPageParam: () => undefined, // 不支持向上加载
  });
}
```

#### 2.3.3 EntryList 组件改造

```typescript
// web/src/components/EntryList.tsx
import { useInfiniteEntries } from '../hooks/useInfiniteEntries';
import { useInView } from 'react-intersection-observer';

interface EntryListProps {
  filter: EntryFilter;
}

export function EntryList({ filter }: EntryListProps) {
  const {
    data,
    fetchNextPage,
    hasNextPage,
    isFetchingNextPage,
    isLoading,
    error,
  } = useInfiniteEntries(filter);

  const { ref: sentinelRef, inView } = useInView({
    threshold: 0,
    rootMargin: '200px', // 提前 200px 开始加载
  });

  React.useEffect(() => {
    if (inView && hasNextPage && !isFetchingNextPage) {
      fetchNextPage();
    }
  }, [inView, hasNextPage, isFetchingNextPage, fetchNextPage]);

  // 扁平化页面数据
  const entries = data?.pages.flatMap((page) => page.entries) ?? [];

  if (isLoading) return <LoadingSkeleton />;
  if (error) return <ErrorMessage error={error} />;

  return (
    <div className="entry-list">
      {entries.map((entry) => (
        <EntryCard key={entry.id} entry={entry} />
      ))}

      {/* 加载触发器 */}
      <div ref={sentinelRef} className="h-4" />

      {/* 加载状态 */}
      {isFetchingNextPage && (
        <div className="flex justify-center py-4">
          <Spinner />
        </div>
      )}

      {/* 无更多数据 */}
      {!hasNextPage && entries.length > 0 && (
        <div className="text-center text-gray-500 py-4">
          No more entries
        </div>
      )}
    </div>
  );
}
```

#### 2.3.4 后端调整

```rust
// src/api/entries.rs
// 移除 cursor 模式下的 page 限制检查
// 只在 offset 模式下保留限制

const MAX_PAGE: i64 = 50;

// 修改验证逻辑
if params.inner.cursor.is_none() {
    if let Some(p) = params.inner.page {
        if p > MAX_PAGE {
            return Err(ApiError::BadRequest(format!(
                "page {} exceeds max {} — use cursor pagination for deep pages",
                p, MAX_PAGE
            )));
        }
    }
}
// cursor 模式无限制
```

### 2.4 实施步骤

| 步骤 | 内容 | 预估时间 |
|------|------|----------|
| 1 | 更新前端 API 类型定义，添加 cursor 参数 | 0.5 天 |
| 2 | 实现 useInfiniteEntries hook | 0.5 天 |
| 3 | 改造 EntryListPage 使用无限滚动 | 1 天 |
| 4 | 添加加载状态、错误处理、空状态 | 0.5 天 |
| 5 | 后端调整（移除 cursor 模式限制） | 0.5 天 |
| 6 | 测试 + 文档 | 0.5 天 |

**总计: 约 3.5 天**

---

## 3. 图片处理管道优化

### 3.1 当前状态

| 项目 | 现状 |
|------|------|
| 处理时机 | 同步，在 fetch pipeline 中 |
| 并发控制 | Semaphore 限制 8 个并发 |
| 错误处理 | 单个图片失败不影响整体 |
| 存储后端 | LocalStorage / OssStorage |

### 3.2 问题分析

1. **阻塞抓取流程**: 图片处理在 `save()` 函数中同步执行，延长了整体抓取时间
2. **无进度反馈**: 用户无法知道图片处理进度
3. **无重试机制**: 网络抖动导致的失败无法重试
4. **无大小预警**: 10MB 限制是硬性的，无渐进式加载

### 3.3 目标

- 图片处理异步化，不阻塞抓取流程
- 支持渐进式图片加载
- 添加重试机制
- 提供处理状态

### 3.4 技术方案

#### 3.4.1 异步处理架构

```
Fetch Pipeline
      │
      ▼
  save() ─────────────────────────────────────┐
      │                                        │
      ▼                                        ▼
保存 entry (原始 HTML)              创建 ImageProcessJob
      │                               放入队列
      ▼                                        │
返回成功给用户                              ▼
                                    ImageProcessor (后台任务)
                                          │
                                          ▼
                                    处理图片，更新 entry content
```

#### 3.4.2 数据库迁移

```sql
-- migrations/X_image_process_jobs.sql
CREATE TYPE image_process_status AS ENUM ('pending', 'processing', 'completed', 'failed');

CREATE TABLE image_process_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entry_id UUID NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    original_html TEXT NOT NULL,
    status image_process_status NOT NULL DEFAULT 'pending',
    error_message TEXT,
    retry_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_image_process_jobs_status ON image_process_jobs(status)
    WHERE status IN ('pending', 'processing');
```

#### 3.4.3 后台处理器

```rust
// src/tasks/image_processor.rs
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct ImageProcessor {
    pool: PgPool,
    storage: Arc<dyn ImageStorage>,
    semaphore: Arc<Semaphore>,
}

impl ImageProcessor {
    pub fn new(pool: PgPool, storage: Arc<dyn ImageStorage>) -> Self {
        Self {
            pool,
            storage,
            semaphore: Arc::new(Semaphore::new(4)), // 最多 4 个并发处理任务
        }
    }

    pub async fn run(&self) {
        loop {
            // 获取待处理任务
            let job = self.claim_job().await;

            match job {
                Some(job) => {
                    let permit = self.semaphore.clone().acquire_owned().await.unwrap();
                    let processor = self.clone();

                    tokio::spawn(async move {
                        processor.process_job(&job).await;
                        drop(permit);
                    });
                }
                None => {
                    // 无任务，等待一段时间
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn process_job(&self, job: &ImageProcessJob) {
        match process_images(&job.original_html, self.storage.clone()).await {
            Ok(processed_html) => {
                // 更新 entry content
                entry::update_content(&self.pool, job.entry_id, &processed_html).await;
                self.mark_completed(job.id).await;
            }
            Err(e) => {
                if job.retry_count < 3 {
                    self.mark_retry(job.id).await;
                } else {
                    self.mark_failed(job.id, &e.to_string()).await;
                }
            }
        }
    }
}
```

#### 3.4.4 Fetch Pipeline 调整

```rust
// src/fetch/pipeline.rs
async fn save(ctx: &FetchContext, job: &FetchJob, result: &ExtractResult, status: i16, method: &str) {
    // 先保存原始 HTML
    entry::update_entry_content(
        &ctx.pool,
        job.entry_id,
        result.title.as_deref(),
        Some(&result.content), // 原始 HTML
        Some(&result.preview_picture),
        status,
        method,
    ).await;

    // 创建图片处理任务（异步）
    image_process_job::create(&ctx.pool, job.entry_id, &result.content).await;
}
```

#### 3.4.5 API 扩展：图片处理状态

```rust
// src/api/entries.rs
// 在 Entry 响应中添加图片处理状态

#[derive(Serialize)]
pub struct EntryDetail {
    // ... 现有字段
    pub image_status: Option<ImageStatus>,
}

pub enum ImageStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}
```

### 3.5 实施步骤

| 步骤 | 内容 | 预估时间 |
|------|------|----------|
| 1 | 创建数据库迁移 | 0.5 天 |
| 2 | 实现 image_process_job model | 0.5 天 |
| 3 | 实现 ImageProcessor 后台任务 | 1 天 |
| 4 | 修改 fetch pipeline，创建异步任务 | 0.5 天 |
| 5 | 添加 API 字段返回图片处理状态 | 0.5 天 |
| 6 | 测试 + 文档 | 0.5 天 |

**总计: 约 3.5 天**

---

## 4. 缓存层引入

### 4.1 当前状态

| 数据类型 | 是否缓存 | 查询频率 |
|----------|----------|----------|
| Site Config (YAML) | 是 (内存 HashMap) | 每次抓取 |
| Tags 列表 | 否 | 每次页面加载 |
| Site Rules | 否 | 每次页面加载 |
| Tagging Rules | 否 | **每次抓取** (高频) |
| Entries | 否 | 每次访问 |

### 4.2 目标

- 为高频查询添加进程内缓存
- 缓存失效策略简单可靠
- 不引入 Redis 等外部依赖

### 4.3 技术方案

#### 4.3.1 缓存库选择

使用 `moka` — 高性能 Rust 缓存库，支持 TTL、LRU、并发安全。

```toml
# Cargo.toml
[dependencies]
moka = { version = "0.12", features = ["future"] }
```

#### 4.3.2 缓存层设计

```rust
// src/cache/mod.rs
use moka::future::Cache;
use std::time::Duration;
use uuid::Uuid;

/// 用户级缓存，按 user_id 隔离
pub struct UserCache<T> {
    inner: Cache<Uuid, Vec<T>>,
}

impl<T: Clone + Send + Sync + 'static> UserCache<T> {
    pub fn new(max_capacity: u64, ttl: Duration) -> Self {
        Self {
            inner: Cache::builder()
                .max_capacity(max_capacity)
                .time_to_live(ttl)
                .build(),
        }
    }

    pub async fn get(&self, user_id: Uuid) -> Option<Vec<T>> {
        self.inner.get(&user_id).await
    }

    pub async fn insert(&self, user_id: Uuid, data: Vec<T>) {
        self.inner.insert(user_id, data).await;
    }

    pub async fn invalidate(&self, user_id: Uuid) {
        self.inner.invalidate(&user_id).await;
    }
}
```

#### 4.3.3 缓存实例

```rust
// src/cache/mod.rs
use once_cell::sync::Lazy;
use std::sync::Arc;

pub static TAG_CACHE: Lazy<Arc<UserCache<Tag>>> = Lazy::new(|| {
    Arc::new(UserCache::new(1000, Duration::from_secs(300))) // 5 分钟 TTL
});

pub static TAGGING_RULE_CACHE: Lazy<Arc<UserCache<TaggingRule>>> = Lazy::new(|| {
    Arc::new(UserCache::new(1000, Duration::from_secs(300)))
});

pub static SITE_RULE_CACHE: Lazy<Arc<UserCache<SiteRule>>> = Lazy::new(|| {
    Arc::new(UserCache::new(500, Duration::from_secs(300)))
});
```

#### 4.3.4 Model 层集成

```rust
// src/models/tag.rs
use crate::cache::TAG_CACHE;

pub async fn list_tags_cached(pool: &PgPool, user_id: Uuid) -> Result<Vec<Tag>, ModelError> {
    // 尝试从缓存获取
    if let Some(cached) = TAG_CACHE.get(user_id).await {
        return Ok(cached);
    }

    // 查询数据库
    let tags = list_tags(pool, user_id).await?;

    // 写入缓存
    TAG_CACHE.insert(user_id, tags.clone()).await;

    Ok(tags)
}

pub async fn create_tag(pool: &PgPool, user_id: Uuid, label: &str) -> Result<Tag, ModelError> {
    let tag = /* 创建标签 */;

    // 失效缓存
    TAG_CACHE.invalidate(user_id).await;

    Ok(tag)
}
```

#### 4.3.5 Fetch Pipeline 使用缓存

```rust
// src/fetch/pipeline.rs
async fn apply_tagging_rules(
    pool: &PgPool,
    user_id: Uuid,
    entry_id: Uuid,
    url: &str,
    result: &ExtractResult,
) {
    // 使用缓存版本
    let rules = match crate::models::tagging_rule::list_rules_cached(pool, user_id).await {
        Ok(r) => r,
        Err(_) => return,
    };
    // ...
}
```

### 4.4 缓存失效策略

| 操作 | 失效范围 |
|------|----------|
| 创建/更新/删除 Tag | 该用户的 TAG_CACHE |
| 创建/更新/删除 TaggingRule | 该用户的 TAGGING_RULE_CACHE |
| 创建/更新/删除 SiteRule | 该用户的 SITE_RULE_CACHE |

### 4.5 实施步骤

| 步骤 | 内容 | 预估时间 |
|------|------|----------|
| 1 | 添加 moka 依赖，实现 UserCache | 0.5 天 |
| 2 | 为 Tag 添加缓存层 | 0.5 天 |
| 3 | 为 TaggingRule 添加缓存层（最高优先级） | 0.5 天 |
| 4 | 为 SiteRule 添加缓存层 | 0.5 天 |
| 5 | 集成到 API handlers 和 fetch pipeline | 0.5 天 |
| 6 | 测试 + 文档 | 0.5 天 |

**总计: 约 3 天**

---

## 5. 结构化日志增强

### 5.1 当前状态

| 项目 | 现状 |
|------|------|
| 日志库 | tracing + tracing-subscriber |
| 输出格式 | 默认格式（非结构化） |
| 请求追踪 | 部分使用 `#[instrument]` |
| 日志级别 | 通过 RUST_LOG 环境变量控制 |

### 5.2 目标

- 结构化 JSON 日志输出
- 请求 ID 追踪
- 关键操作审计日志
- 便于日志聚合和分析

### 5.3 技术方案

#### 5.3.1 JSON 格式日志

```rust
// src/main.rs
use tracing_subscriber::{fmt, EnvFilter};

fn init_logging() {
    let format = fmt::format::json();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .event_format(format)
        .with_target(false)  // JSON 格式中不需要 target
        .with_current_span(false)
        .init();
}
```

#### 5.3.2 请求 ID 中间件

```rust
// src/middleware/request_id.rs
use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;
use tracing::Span;

const REQUEST_ID_HEADER: &str = "X-Request-Id";

pub async fn request_id_layer(
    mut request: Request,
    next: Next,
) -> Response {
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // 设置到 tracing span
    Span::current().record("request_id", &request_id);

    // 添加到请求扩展中
    request.extensions_mut().insert(RequestId(request_id.clone()));

    let mut response = next.run(request).await;

    // 返回响应头
    response.headers_mut().insert(
        REQUEST_ID_HEADER,
        request_id.parse().unwrap(),
    );

    response
}

#[derive(Clone)]
pub struct RequestId(pub String);
```

#### 5.3.3 Tracing Layer 集成

```rust
// src/main.rs
use tower_http::trace::{TraceLayer, DefaultMakeSpan};
use tracing_subscriber::fmt::format::FmtSpan;

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .with_span_events(FmtSpan::CLOSE)
        .with_target(false)
        .init();
}

// 在路由中添加
let app = Router::new()
    // ... routes
    .layer(TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new()
            .include_headers(true)))
    .layer(middleware::from_fn(request_id_layer));
```

#### 5.3.4 关键操作审计日志

```rust
// src/audit.rs
use tracing::{info, instrument};

#[instrument(skip_all, fields(
    user_id = %user_id,
    action = %action,
    resource_type = %resource_type,
    resource_id = %resource_id,
))]
pub fn log_audit_event(
    user_id: Uuid,
    action: &str,
    resource_type: &str,
    resource_id: &str,
    details: Option<serde_json::Value>,
) {
    info!(
        target: "audit",
        action = %action,
        resource_type = %resource_type,
        resource_id = %resource_id,
        details = ?details,
        "audit event"
    );
}

// 使用示例
log_audit_event(
    user.id,
    "entry.save",
    "entry",
    &entry.id.to_string(),
    Some(json!({ "url": url })),
);
```

#### 5.3.5 日志输出示例

```json
{
  "timestamp": "2026-04-29T10:30:00.123456Z",
  "level": "INFO",
  "message": "audit event",
  "target": "audit",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "user_id": "123e4567-e89b-12d3-a456-426614174000",
  "action": "entry.save",
  "resource_type": "entry",
  "resource_id": "987e6543-e21c-43d2-b789-123456789abc",
  "details": {
    "url": "https://example.com/article"
  }
}
```

### 5.4 实施步骤

| 步骤 | 内容 | 预估时间 |
|------|------|----------|
| 1 | 配置 JSON 格式日志输出 | 0.5 天 |
| 2 | 实现 Request ID 中间件 | 0.5 天 |
| 3 | 集成 TraceLayer 到路由 | 0.5 天 |
| 4 | 添加审计日志模块 | 0.5 天 |
| 5 | 为关键 API 添加审计日志 | 0.5 天 |
| 6 | 测试 + 文档 | 0.5 天 |

**总计: 约 3 天**

---

## 实施优先级与依赖关系

```
┌─────────────────────────────────────────────────────────────┐
│                        Phase 1 (基础)                        │
│  ┌──────────────────┐     ┌──────────────────┐              │
│  │ 结构化日志增强    │     │ 缓存层引入        │              │
│  │ (3 天)           │     │ (3 天)           │              │
│  └──────────────────┘     └────────┬─────────┘              │
└────────────────────────────────────┼────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────┐
│                        Phase 2 (前端)                        │
│  ┌──────────────────┐     ┌──────────────────┐              │
│  │ 深分页完整解决    │     │ 浏览器扩展升级    │              │
│  │ (3.5 天)         │     │ (4.5 天)         │              │
│  └──────────────────┘     └──────────────────┘              │
└─────────────────────────────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────┐
│                        Phase 3 (后台)                        │
│  ┌──────────────────┐                                        │
│  │ 图片处理管道优化  │                                        │
│  │ (3.5 天)         │                                        │
│  └──────────────────┘                                        │
└─────────────────────────────────────────────────────────────┘
```

### 总时间估算

| 阶段 | 内容 | 时间 |
|------|------|------|
| Phase 1 | 结构化日志 + 缓存层 | 6 天 |
| Phase 2 | 深分页 + 浏览器扩展 | 8 天 |
| Phase 3 | 图片处理管道 | 3.5 天 |
| **总计** | | **17.5 天** |

### 建议执行顺序

1. **结构化日志** — 为后续优化提供可观测性基础
2. **缓存层** — 立即见效的性能优化，特别是 tagging_rule 缓存
3. **深分页** — 前端体验优化，独立于其他模块
4. **浏览器扩展** — 可与深分页并行开发
5. **图片处理管道** — 最后实施，依赖缓存层和日志系统

---

## 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 浏览器扩展迁移引入 bug | 用户体验下降 | 保持原有 JS 文件作为回滚方案 |
| 缓存一致性 | 数据不一致 | 使用明确的失效策略，添加缓存监控 |
| 图片异步处理失败 | 内容不完整 | 重试机制 + 失败状态可见 |
| JSON 日志格式变更 | 日志聚合工具适配 | 提供格式文档，预留字段扩展空间 |
