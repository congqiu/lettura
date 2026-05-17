# 剩余架构优化交接文档

> 创建日期: 2026-05-17
> 状态: 待接手

## 背景

2026-05 完成了 #1 抓取队列持久化、#5 后台任务统一编排、#4 cache 收 AppState、#6 API codegen Phase 1。本文档列出仍未完成的优化项，每项包含**背景、目标、产出清单、验收标准、工作量估计**，接手人无需额外沟通就能开始。

完成一项后请 ping 验收：列出 commit hash + 跑过的测试。

---

## 任务索引（按优先级）

| ID | 标题 | 类型 | 工作量 | 优先级 |
|----|------|------|--------|--------|
| A | API codegen Phase 2 — 给剩余 handler 加 utoipa 注解 | 后端 | 1-2 天分阶段 | 高 |
| B | 前端用 schema.ts 替代手写 API type（配合 A） | 前端 | 半天 | 高 |
| C | EntryListPage 565 行拆分 | 前端 | 半天 | 中 |
| D | 路由级 code splitting (React.lazy) | 前端 | 2 小时 | 中 |
| E | axios → fetch + TanStack Query 统一 | 前端 | 半天 | 低 |
| F | backup endpoint 流式输出 | 后端 | 半天 | 中 |
| G | SearchBackend trait 抽象 | 后端 | 1 天 | 低（无多副本压力前不急） |
| H | Config 简化（figment / envy） | 后端 | 2-3 小时 | 低 |
| I | 砍胖 handler + 引入 service 层（pages 1033 行） | 后端 | 2-3 天 | 低（按需重构） |
| J | admin panel 浏览器端到端 UI smoke test | QA / 前端 | 1-2 小时 | 高 |

总计：高 + 中优先级约 3-4 天，低优先级按需做。

---

## A. API codegen Phase 2 — 剩余 handler 加 utoipa 注解

### 背景

`docs/specs/2026-05-16-fetch-queue-persistence.md` 完成后，2026-05-17 引入了 utoipa OpenAPI codegen（commit `a987205`）。框架已就绪，但仅 2 个 handler 加了注解（`/api/health`、`/api/v1/tags`）。剩余 77 个 handler 还没在 schema 里，前端无法获得这些 endpoint 的类型。

### 目标

把所有面向前端 / CLI 的 endpoint（约 60+，admin 内部接口可选）加上 utoipa 注解，并在 `src/api/openapi.rs::ApiDoc` 登记。覆盖率到 90%+。

### 产出清单

- 每个 handler 上加 `#[utoipa::path(method, path, tag, responses(...), security(...))]`
- 每个 request/response struct 加 `#[derive(utoipa::ToSchema)]`
- `src/api/openapi.rs::ApiDoc` 的 `paths(...)` 和 `components(schemas(...))` 列出全部
- 跑 `./dev.sh codegen` 重新生成 `web/src/api/openapi.json` + `web/src/api/schema.ts`，commit

**分批 commit 策略**（强烈建议，避免一个超大 PR）：
- batch 1: entries 模块（list_entries / get_entry / create_entry / update_entry / delete_entry / refetch / restore / permanently_delete）
- batch 2: tags（剩余）+ tagging_rules + site_rules
- batch 3: annotations + memos
- batch 4: auth + tokens + audit_logs
- batch 5: bulk + import + export
- batch 6: pages + pages_public（如果时间允许）
- batch 7: admin + fetch_jobs admin

每批一个 commit + `./dev.sh codegen` regenerate schema。

### 实施要点

- 参考已有示范：`src/api/health.rs::health_check` 和 `src/api/tags.rs::list_tags`
- 复杂类型（含 enum / nested struct）的 derive `ToSchema` 可能需要 `#[schema(value_type = String)]` 等 escape hatch — 看 utoipa 5.x docs
- `Entry` struct 在 `src/models/entry.rs` 是最大类型（含 JSON metadata 字段），可能需要 `#[schema(value_type = serde_json::Value)]`
- `Json<Vec<T>>` / `Json<T>` 自动处理，`Query<T>` / `Path<T>` 自动转 query/path 参数
- 用 `tag = "<module>"` 让前端按模块分组（utoipa 会自动按 tag 分类）

### 验收标准

我会跑：

```bash
# 1. 编译通过 + 测试零回归
docker run ... cargo test --no-default-features --features test-utils --lib

# 2. schema 包含目标 endpoint
grep -c '"path":' web/src/api/openapi.json
# 期望 ≥ 60

# 3. 前端 codegen 跑得通
./dev.sh codegen

# 4. schema.ts 含目标类型
grep -E "Entry|EntrySummary|Annotation|Memo|TaggingRule" web/src/api/schema.ts
# 期望都能找到

# 5. src/api/openapi.rs::tests 仍通过
docker run ... cargo test --lib openapi
```

### 工作量估计

1-2 天分多个 session（不要一次写 60 个注解，会做疯）。建议每天 2-3 个 batch。

---

## B. 前端用 schema.ts 替代手写 API type

### 背景

A 完成后，`web/src/api/schema.ts` 含全部后端 endpoint 的精确 TS 类型。现在 `web/src/api/*.ts` 里所有 `export interface FetchJob { ... }` 都是手写，与后端容易漂移。

### 目标

把 `web/src/api/*.ts` 里的 `export interface` 全部改为从 `schema.ts` 引用，删除重复的手写类型。

### 产出清单

- 每个 `web/src/api/<resource>.ts` 文件：
  - import: `import type { paths, components } from './schema';`
  - 类型 alias: `export type Entry = components['schemas']['Entry'];`
  - 删除原手写 interface
- 调用 `api.get<T>(...)` / `api.post<T>(...)` 的泛型用从 schema 派生的类型
- 前端 `pnpm exec tsc --noEmit` 零 error

### 验收标准

```bash
# 1. tsc 通过
cd web && pnpm exec tsc --noEmit

# 2. eslint 不引新 error
cd web && pnpm run lint

# 3. vitest 全过
cd web && pnpm test

# 4. 浏览器跑核心路径：登录、列表、详情、新建 entry、tag、admin panel
#    （手动）

# 5. 检查 web/src/api/ 下原 export interface 全部删除
grep -rn "^export interface" web/src/api/ | grep -v "schema.ts"
# 期望：空或仅极少 helper 类型
```

### 工作量估计

半天（10 个 api/*.ts 文件，每个 30 分钟左右，包含组件层调用点的 fixup）。

---

## C. EntryListPage 565 行拆分

### 背景

`web/src/pages/EntryListPage.tsx` 累计 565 行，含分页 / 过滤 / 选择 / 键盘快捷键 / 批量操作 / URL 状态同步等多种关注点。改一处经常误碰其他。

### 目标

拆成多个 hook + 子组件，每个文件 < 200 行，单一职责。

### 产出清单

- `web/src/hooks/useEntryListFilters.ts` — URL query string 解析 + setter
- `web/src/hooks/useEntrySelection.ts` — 选中 entry 集合管理（已有 multi-select 行为）
- `web/src/components/entries/EntryListToolbar.tsx` — 顶部过滤 + 批量操作按钮
- `web/src/components/entries/EntryListBody.tsx` — entries 渲染区
- `web/src/pages/EntryListPage.tsx` — 编排上面几个 hook + 组件，< 150 行

API 表面不变，行为完全等价。

### 验收标准

```bash
# 1. 行数检查
wc -l web/src/pages/EntryListPage.tsx
# 期望 < 150

# 2. tsc + lint + vitest
cd web && pnpm exec tsc --noEmit && pnpm run lint && pnpm test

# 3. 浏览器手动验证以下行为完全一致：
#    - 分页（cursor 翻页）
#    - 状态过滤（unread/archived/starred）
#    - tag 过滤
#    - 全选 + 反选 + 批量归档 / 加 tag / 删除
#    - 键盘快捷键（j/k/r/a/s 等，按现状）
#    - URL ?status=&tag=&cursor= 同步
```

### 工作量估计

半天。拆分本身机械，主要是回归测试。

---

## D. 路由级 code splitting (React.lazy)

### 背景

当前 `App.tsx` 静态 import 所有页面组件，首屏 bundle 含全部 page 代码（含 admin / settings / pages 等不常用 page）。

### 目标

用 `React.lazy + Suspense` 让非首屏 page 按需加载。

### 产出清单

- `web/src/App.tsx`:
  ```tsx
  const SettingsPage = lazy(() => import('./pages/SettingsPage'));
  const AuditLogsPage = lazy(() => import('./pages/AuditLogsPage'));
  const PagesPage = lazy(() => import('./pages/PagesPage'));
  // ... 等 admin / settings / 不常用 page
  ```
  + 包 `<Suspense fallback={<LoadingSpinner />}>` 在 `<Routes>` 周围
- `EntryListPage` / `LoginPage` / `EntryDetailPage` 保持静态 import（首屏关键路径）
- 看 `vite build` 输出 chunk 数量增多

### 验收标准

```bash
# 1. tsc + lint + vitest 通过
cd web && pnpm exec tsc --noEmit && pnpm run lint && pnpm test

# 2. vite build chunks 增多
cd web && pnpm build
# 看输出有多个 .js chunk（不再是单个大 bundle）

# 3. 浏览器手动验证：登录 → entry list 正常（首屏）→ 进 settings/admin 时
#    network tab 看到额外 .js 请求（懒加载）
```

### 工作量估计

2 小时。

---

## E. axios → fetch + TanStack Query 统一

### 背景

`web/src/api/client.ts` 用 axios（含 token 拦截 + 401 refresh），TanStack Query 又自己一套 fetcher 协议。两套 HTTP 客户端共存，bundle ~13KB 浪费。

### 目标

迁到原生 fetch（或 ofetch），axios 移除依赖。token 拦截改用 fetch 的 middleware/wrapper。

### 产出清单

- `web/src/api/client.ts` 重写为 fetch wrapper，含：
  - 401 自动 refresh 逻辑（保留现有 `refreshPromise` 锁）
  - `apiGet<T>(path, params?) → Promise<T>`、`apiPost<T>(path, body?) → Promise<T>` 等 helper
- 所有 `web/src/api/*.ts` 改用新 helper
- 移除 axios 依赖：`pnpm remove axios`
- bundle size 减少 ~13KB

### 验收标准

```bash
# 1. tsc + lint + vitest
cd web && pnpm exec tsc --noEmit && pnpm run lint && pnpm test

# 2. package.json 不再含 axios
grep '"axios"' web/package.json
# 期望：空

# 3. vite build 大小对比
cd web && pnpm build
# 与改前对比 dist/ 总大小（gzip 后）应减少

# 4. 浏览器手动验证：登录 → access_token 过期场景（手动改 expiry 测）
#    refresh 自动触发，单次并发不重复 refresh
```

### 工作量估计

半天。401 refresh 逻辑要小心。

---

## F. backup endpoint 流式输出

### 背景

`src/api/backup.rs::backup` 当前把所有 entries / annotations / memos / tags 全部 SELECT 进内存，序列化成单个巨大 `Json<BackupBundle>` 返回。用户 entries 几千条时即 OOM。

### 目标

改为流式输出 NDJSON（每行一条记录）或 zip 流，不在内存累计。

### 产出清单

- `src/api/backup.rs::backup`：
  - 返回类型从 `Json<BackupBundle>` 改为 `axum::body::Body`（stream）或 zip stream
  - 输出格式：每行一个 JSON 对象，第一行 metadata（version / user / counts），后续按 type 分段
  - 内部用 `sqlx::query(...).fetch(pool)` 流式拉，逐条 serialize + write
- `tests/integration_*`：加测试，造 1000 条 entry，断言响应能在常数内存下完成
- 前端 `web/src/components/settings/DataPanel.tsx` 下载逻辑可能需要适配（fetch + stream → blob）
- 改后旧的 `BackupBundle` JSON 格式不再生效，需文档说明（CLAUDE.md + docs/specs/ 加 backup format spec）

### 验收标准

```bash
# 1. 跑 backup endpoint 不 OOM（造 5000 条 entry）
docker compose exec lettura curl -s -H "Authorization: Bearer ..." /api/v1/admin/backup > backup.ndjson
wc -l backup.ndjson  # 期望 5000+ 行
# docker stats lettura 看内存峰值不超过 200MB

# 2. restore 能读流式格式（顺手把 restore 也 streamify，或确认两边格式兼容）
docker compose exec lettura curl -s -X POST -H "Authorization: Bearer ..." \
  --data-binary @backup.ndjson /api/v1/admin/restore

# 3. 现有 integration_import_export.rs 测试零回归

# 4. 前端 backup 下载按钮工作正常
```

### 工作量估计

半天。restore 端如果不同步改造可能需要兼容层。

---

## G. SearchBackend trait 抽象

### 背景

`src/search.rs::SearchIndex` 直接耦合 tantivy + 本地文件锁 + `Arc<Mutex<IndexWriter>>` 单写者。多副本部署完全走不通（文件锁冲突）。

### 目标

定义 `trait SearchBackend`，把 tantivy 包成一种实现 `LocalTantivyBackend`，未来可换 Meilisearch / Quickwit / OpenSearch 而不动 handler。

### 产出清单

- `src/search/mod.rs` 改成 trait 定义：
  ```rust
  #[async_trait]
  pub trait SearchBackend: Send + Sync {
      async fn upsert(&self, doc: SearchDoc) -> Result<(), SearchError>;
      async fn delete(&self, id: Uuid) -> Result<(), SearchError>;
      async fn commit(&self) -> Result<(), SearchError>;
      async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchHit>, SearchError>;
      fn doc_count(&self) -> Result<u64, SearchError>;
  }
  ```
- `src/search/local_tantivy.rs` 把现有实现挪进来
- `src/state.rs::AppState.search_index` 类型改为 `Arc<dyn SearchBackend>`
- `WorkerConfig.search_index` 同样改 trait object
- handler / worker / pipeline 代码改为通过 trait 调用，不直接知道 tantivy

### 验收标准

```bash
# 1. cargo test 全过
docker run ... cargo test --no-default-features --features test-utils

# 2. integration_search 测试零回归
docker run ... cargo test --test integration_search

# 3. 代码层 grep tantivy 应该只出现在 search/local_tantivy.rs 里
grep -rn "use tantivy" src/ | grep -v "src/search/local_tantivy.rs"
# 期望：空

# 4. router_with_search 接受 Option<Arc<dyn SearchBackend>>（不再是 Option<SearchIndex>）
```

### 工作量估计

1 天。

### 不做

- 不实现非 tantivy 后端（留作未来）
- 不改 Cargo features（仍然只编 tantivy）

---

## H. Config 简化（figment / envy）

### 背景

`src/config.rs::Config::from_env` 100+ 行重复 `env::var("X").ok().and_then(|v| v.parse().ok()).unwrap_or(...)` 样板。

### 目标

用 `figment` 或 `envy` 替代手写 from_env，按 struct 字段 + serde 默认值自动解析。

### 产出清单

- `Cargo.toml` 加 `figment = { version = "0.10", features = ["env"] }` 或 `envy`
- `src/config.rs::Config` 加 `#[derive(Deserialize)]` + 字段 `#[serde(default = "...")]` 标 default
- `Config::from_env` 简化为：
  ```rust
  pub fn from_env() -> Result<Self, ConfigError> {
      Figment::new()
          .merge(Env::prefixed("LETTURA_").split("__"))
          .merge(Env::raw())  // for non-prefixed like DATABASE_URL / JWT_SECRET
          .extract()
  }
  ```
- 保留现有所有 env var 名（向下兼容）和强制校验（JWT_SECRET 长度等）

### 验收标准

```bash
# 1. cargo test 全过
docker run ... cargo test --no-default-features --features test-utils

# 2. 现有所有 env var 仍然 work（手动设几个非默认值 + ./dev.sh up 启动看日志确认）

# 3. config.rs 行数显著减少
wc -l src/config.rs
# 期望 < 100 行（之前 ~200 行）
```

### 工作量估计

2-3 小时。

---

## I. 砍胖 handler + 引入 service 层

### 背景

`src/api/pages.rs` 1033 行混了 multipart 解析、zip 解压、文件系统遍历、HTML title 提取、MIME 推断、DB 写入、审计、handler 路由。改一处影响面大、难单元测试。`src/models/entry.rs` 769 行里 `list_entries / build_where_clause / attach_tags / next_cursor_from` 是业务逻辑而非 model，也该挪出。

### 目标

抽出 `src/services/` 层放业务逻辑，handler 退化成"解析请求 → 调 service → 序列化响应"。优先重构 `pages` 一个模块作示范，其余按需。

### 产出清单

- `src/services/mod.rs` + `src/services/pages.rs`（业务逻辑：upload / create / update / delete / 文件操作）
- `src/api/pages.rs` 仅保留 handler 函数，每个 < 30 行
- `src/services/pages/upload.rs`、`src/services/pages/zip.rs` 等子模块
- 单测能直接测 service（不需要起 HTTP）

### 验收标准

```bash
# 1. 行数控制
wc -l src/api/pages.rs
# 期望 < 300 行（handler-only）

# 2. cargo test 全过，特别是 integration_pages
docker run ... cargo test --test integration_pages

# 3. 新增 service 层单测覆盖至少 70%（zip / mime / path safety 等纯函数）
docker run ... cargo test --lib services::pages
```

### 工作量估计

2-3 天（仅 pages 模块）。

### 不做

- 不重构 entries / tags 等其他模块（按需进行）
- 不改 API 协议
- 不引入 trait abstraction（保持简单 service struct）

---

## J. admin panel 浏览器端到端 UI smoke test

### 背景

2026-05 的 fetch queue 持久化（commit `3e081a8`）引入了 Settings → 抓取队列 admin panel。代码 + tsc + lint + 集成测试都过，但**没在真实浏览器跑过端到端**——表格渲染、按钮交互、toast、5 秒轮询、ConfirmDialog 在 OrbStack/Safari/Chrome 下的实际行为没人验过。可能踩坑：CSP 把字体 / 资源拦了、TanStack Query devtools 没装、admin tab 没接到 SettingsPage 等。

### 目标

按下面 checklist 跑一遍，发现的问题写成 GitHub issue（或直接修 + 跟我同步）。

### 准备工作

```bash
# 1. 起服务
./dev.sh dev   # 后端 :3330 + 前端 vite :5173

# 2. 注册 admin（首个用户自动 admin，见 src/api/auth.rs:104）
#    浏览器开 http://localhost:5173 → Register → 用 username=admin

# 3. 造测试数据（在 dev compose 的 postgres 里）
./dev.sh psql
# 然后跑：
INSERT INTO entries (user_id, url, given_url, hashed_url, hashed_given_url)
SELECT id, 'https://x.test/dead-' || g, 'https://x.test/dead-' || g, md5(g::text), md5(g::text)
FROM users CROSS JOIN generate_series(1, 150) g WHERE username='admin';

-- 入队 + 全部标 dead
INSERT INTO fetch_jobs (entry_id, user_id, url, priority)
SELECT id, user_id, url, 0 FROM entries WHERE url LIKE 'https://x.test/dead-%';
UPDATE fetch_jobs SET status='dead', last_error='manual test', last_error_at=NOW()
WHERE url LIKE 'https://x.test/dead-%';

-- 再造 3 条 failed + 1 条 running
INSERT INTO entries (user_id, url, given_url, hashed_url, hashed_given_url)
SELECT id, 'https://x.test/failed-' || g, 'https://x.test/failed-' || g, md5('f'||g), md5('f'||g)
FROM users CROSS JOIN generate_series(1, 3) g WHERE username='admin';
INSERT INTO fetch_jobs (entry_id, user_id, url, status, attempts, last_error, last_error_at)
SELECT id, user_id, url, 'failed', 2, 'timeout', NOW() FROM entries WHERE url LIKE 'https://x.test/failed-%';
```

### 验收清单（每项 PASS/FAIL + 截图）

**导航与权限**
- [ ] admin 用户登录 → Settings 页能看到"抓取队列" tab/section
- [ ] 切到该 tab → 不报错、不白屏，loading 状态合理
- [ ] （进阶）注册第二个 user "normie"，登录后访问 Settings：抓取队列 panel 调 admin endpoint 返回 403，UI 应显示友好错误状态（不是裸 JSON / 不是白屏）

**状态切换**
- [ ] failed / dead / running / pending 四个 chip 都能点击，切换后表格刷新
- [ ] 当前选中的 chip 视觉上高亮（active 状态）
- [ ] 每个 tab 显示对应 status 的 job，空 tab 显示 "没有 X 状态的任务" 而非空白

**列表渲染**
- [ ] URL 列：长 URL 截断不破布局，hover 显示完整（title 属性）
- [ ] 尝试列：badge `attempts/max_attempts` 渲染正常
- [ ] 最后错误列：长错误截断 + hover 完整、空错误显示 `—`
- [ ] 时间列：`YYYY-MM-DD HH:MM:SS` 格式，无 ISO 残留的 T/Z

**自动刷新**
- [ ] 在 dead tab 停留 → DevTools Network 看每 5 秒一次 `/admin/fetch-jobs?status=dead` 请求
- [ ] 切到其他 tab → 旧 tab 的轮询停止
- [ ] 切回最小化窗口 → 验证是否仍轮询（TanStack Query 默认 `refetchIntervalInBackground: false`，可记录实际表现）

**单条重试**
- [ ] dead tab 某行点"重试" → 该行从 dead tab 消失（下一次 refetch 后）
- [ ] toast 显示 "已重新入队"
- [ ] 切到 pending tab 能看到该 job
- [ ] DB 验证：`SELECT status, attempts FROM fetch_jobs WHERE id = '<id>'` → status='pending', attempts=0

**复活死信**
- [ ] dead tab 显示"复活 100 个死信"按钮（dead 数 > 0 时）
- [ ] 点击 → toast 显示 "已复活 100 个死信任务（还有 50 个未复活，再点击一次继续）"
- [ ] 再点一次 → 复活剩余 50 个，toast 不再显示 "还有"
- [ ] dead tab 变空，pending tab 多 150 个
- [ ] empty 状态下"复活"按钮消失或 disabled

**单条删除**
- [ ] 某行点"删除" → 弹出 ConfirmDialog
- [ ] 取消 → 行仍在
- [ ] 确认 → 该行消失，DB 中对应 fetch_jobs 行被 DELETE
- [ ] toast 显示成功提示（如果有）

**异常态**
- [ ] 后端挂掉时（`./dev.sh down` 然后保持前端 5173 开着）→ panel 显示错误 / loading，不会无限转圈或白屏
- [ ] 后端恢复后 → 下一次 refetch 自动恢复显示

### 验收交付

完成清单后 ping：
- 通过项数 / 总项数
- FAIL 项的截图 + 复现步骤
- 任何"非清单内的问题"（CSP 警告、控制台 error、字体加载失败、accessibility issue 等）

### 不做

- 不修 FAIL 项（除非 trivial），列出来给后端 / 前端 owner 跟进
- 不测多用户并发场景
- 不做 Lighthouse / a11y 自动化（这是 smoke test 不是审计）

### 工作量估计

1-2 小时（含造数据、跑清单、截图）。

---

## 通用约定

- 注释英文，文档中文
- commit 信息中文（遵循近期 commit 风格）
- 每个 commit 原子可编译
- 测试在 docker 跑：`docker run --rm --network lettura_default -e DATABASE_URL=postgres://lettura:lettura@postgres:5432/lettura -v $(pwd):/app -v lettura-target:/app/target -v lettura-cargo-registry:/usr/local/cargo/registry -w /app lettura-bb-sccache cargo test ...`
- 不动现有的 12 个 commit 历史（feature 已合 main）
- 改动 utoipa schema 后必须跑 `./dev.sh codegen` regenerate
- 改动 model 函数签名后必须更新所有 caller（cargo check --all-targets 找漏）

## 验收流程

每完成一项，ping 给我一段：

```
Task: <ID> <标题>
Commit(s): <hash> ... <hash>
Tests run: <command output 关键行>
Notes: <实施中的取舍 / 偏离 / 待跟进>
```

我会按对应"验收标准"章节跑检查命令，确认后回复 ✅ 或列出 fail 项。

## 不在本文档的项

- 浏览器手动验证 admin panel（fetch queue 那次的 UI），我自己做
- ultrareview 等 AI 协助流程相关
- 文档站点 / CHANGELOG 等运营事项
