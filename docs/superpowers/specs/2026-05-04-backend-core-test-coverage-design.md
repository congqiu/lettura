# 后端核心逻辑测试覆盖设计

## 目标

为后端核心业务逻辑补充纯函数单元测试，不依赖数据库或外部服务，提升关键模块的测试信心。

## 范围

6 个模块，预计新增约 25-30 个测试。

## 模块详情

### 1. `models/serde_helpers.rs` — 自定义反序列化器测试（6-8 个测试）

当前状态：0 个测试

新增测试：
- `deserialize_i64_from_string`：
  - 数字字符串 → 正确解析
  - 非数字字符串 → 报错
  - null（visit_unit）→ None
  - 原生整数（visit_i64/visit_u64）→ 正确解析
- `deserialize_bool_from_string`：
  - "true"/"false" → 正确解析
  - "1"/"0" → **应报错**（当前实现只接受 "true"/"false"）
  - 原生 bool（visit_bool）→ 正确解析
  - 其他无效字符串 → 报错

### 2. `models/entry.rs` — 纯函数测试（8-10 个测试）

当前状态：2 个测试（cursor encode/decode）

新增测试：
- `hash_url`：
  - 不同 URL 产生不同哈希
  - 相同 URL 产生相同哈希
  - 空 URL 边界
- `extract_domain`：
  - 常见域名（https://example.com/path → example.com）
  - 带端口（http://localhost:3000 → localhost:3000）
  - 带子域名
  - 无效 URL 处理
  - IP 地址
- `next_cursor_from`：
  - 正常分页（items >= per_page → Some(cursor)）
  - 不足一页（items < per_page → None）
  - 空列表 → None
  - 注：使用辅助函数 `fn mock_summary()` 构造 EntrySummary 测试数据

### 3. `models/tag.rs` — `slugify` 测试（5-6 个测试）

当前状态：0 个单元测试（仅集成测试覆盖）

新增测试：
- 英文标签 → 小写化 + 连字符
- 中文标签处理（当前无截断逻辑，验证超长标签不会被截断）
- 特殊字符处理
- 空字符串
- 已有 slug 格式的标签保持不变

### 4. `models/audit_log.rs` — `new_entry` 构造逻辑测试（3-4 个测试）

当前状态：0 个单元测试（仅集成测试覆盖）

新增测试：
- `new_entry` 对不同 `AuditAction` 枚举值的构造结果
- 默认字段填充验证
- `new_entry` 构造的结构体字段完整性

注意：`fire_and_forget` 依赖 PgPool 和 tokio::spawn，不是纯函数，不在本阶段覆盖。`created_at` 时间戳由数据库生成，不在纯函数测试范围内。

### 5. `fetch/pipeline.rs` — 提取可测试纯逻辑（3-5 个测试）

当前状态：0 个测试，450 行，核心抓取编排器

策略：
- 提取 `SHORT_CONTENT_THRESHOLD` 相关判断逻辑为纯函数 `fn should_try_render(text_len: usize, render_mode: RenderMode) -> bool`
- 提取 `html_rules_from_config` 中 YAML→SiteRuleConfig 的映射逻辑（如有纯计算部分）
- 对提取出的纯函数写单元测试
- 不改动 `process` 的外部行为，只做内部拆分

注意：pipeline 中大部分函数依赖 FetchContext/PgPool/SearchIndex，纯函数测试收益有限。核心编排逻辑推迟到集成测试阶段覆盖。

### 6. `tasks/fetcher.rs` — 队列逻辑测试（2-3 个测试）

当前状态：0 个测试

新增测试：
- `FetchQueue::send` 基本发送行为
- 通道关闭后的错误处理

注意：`FetchQueue` 使用 mpsc 有界通道（容量 5000），通道满时是背压等待而非限流。测试通道满场景需要 5000+ 次 send，不实际，降级为可选。

## 实施顺序

按依赖关系和风险排序：

1. `models/serde_helpers.rs` — 最简单，纯函数，热身
2. `models/entry.rs` — 纯函数，补充已有测试
3. `models/tag.rs` — 纯函数
4. `models/audit_log.rs` — 需要理解构造逻辑
5. `fetch/pipeline.rs` — 最复杂，需要拆分
6. `tasks/fetcher.rs` — 需要理解异步队列

## 原则

- 严格 TDD：先写测试 → 确认失败 → 实现/拆分 → 确认通过
- 测试在 Docker 中运行：`docker compose exec lettura cargo test`
- 只测纯函数和可隔离逻辑，不引入 mock 框架
- 代码注释用英文
