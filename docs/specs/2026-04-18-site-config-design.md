# 站点配置系统设计规格

## 背景

Lettura 当前的抓取系统存在两个核心问题：
1. **抓取失败率高** — 许多网站因反爬机制（403、需要 JS 渲染、需要特定 Cookie/Header）而无法获取内容
2. **提取质量差** — 即使抓到了 HTML，readability 算法提取的内容可能包含广告、侧边栏等噪音

现有的 `site_rules` 数据库表只支持简单的 CSS 选择器，无法控制抓取行为（渲染方式、HTTP 头等），也无法应对同一域名下不同 URL 路径的差异。

本设计引入一个受 **fivefilters/ftr-site-config** 启发的站点配置系统，以文件形式管理每个域名的抓取和提取规则，支持内置配置库和用户本地覆盖。

## 配置格式

每个域名一个文本文件，文件名为 `{domain}.txt`（如 `medium.com.txt`）。格式简洁，每行一个指令。

### 完整指令集

```
# ===== 抓取控制 =====

# 是否需要 JS 渲染（默认 false）
render: true

# 自定义 HTTP 请求头（可多行）
header: Cookie: session=abc123
header: Referer: https://example.com/login

# 覆盖 User-Agent（不设置则使用全局默认）
user_agent: Mozilla/5.0 ...

# 请求超时秒数（不设置则使用全局默认）
timeout: 60

# ===== 内容提取 =====

# 标题选择器（CSS，可多个用逗号分隔，依次尝试）
title: h1.article-title, h1.post-title

# 正文内容选择器（CSS，可多个用逗号分隔）
body: div.article-body, article, main

# 需要移除的元素（CSS，可多行）
strip: div.ads
strip: div.sidebar
strip: nav.breadcrumb

# 作者选择器
author: span.author, a[rel="author"]

# 发布日期选择器
date: time, span.date

# 预览图片选择器（提取 src 属性）
image: img.hero, meta[property="og:image"]

# ===== URL 路径匹配 =====

# 仅匹配特定路径前缀（可多行，不设置则匹配整个域名）
match: /article/
match: /post/

# 排除特定路径（可多行）
exclude: /video/
exclude: /gallery/
```

### 示例

**medium.com.txt**：
```
render: true
title: h1
body: article
strip: div.meteredContent + div
author: a[rel="author"]
date: time
```

**sspai.com.txt**：
```
title: h1.ArticleTitle
body: div.Article-content
author: span.ArticleAuthor
strip: div.Article-sideAction
```

**twitter.com.txt**：
```
render: true
match: /status/
title: article span[data-testid="tweetText"]
body: article div[data-testid="tweetText"]
```

## 规则优先级

查找顺序（从高到低）：

```
1. 用户本地覆盖文件 (LETTURA_SITE_CONFIGS_PATH 下的 {domain}.txt)
2. 内置配置库 (编译时通过 rust-embed 嵌入的 site-configs/*.txt)
3. 数据库 site_rules 表（用户通过 UI 管理的简单规则）
4. readability 自动提取（默认兜底行为）
```

每层如果找到匹配规则就使用，不再向下查找。

## 架构

### 文件结构

```
site-configs/                    # 内置配置库（编译时嵌入二进制）
├── medium.com.txt
├── twitter.com.txt
├── github.com.txt
├── sspai.com.txt
├── ...
└── zhihu.com.txt
```

### 数据流

```
URL 输入
  │
  ├─ 1. 提取域名
  │
  ├─ 2. 查找站点配置（本地覆盖 → 内置库 → DB site_rules）
  │     │
  │     ├─ 找到配置且有 render: true
  │     │     → 直接调用 browserless 渲染
  │     │
  │     ├─ 找到配置且有自定义 headers
  │     │     → 使用自定义 headers 发送请求
  │     │
  │     └─ 无配置或配置无特殊抓取指令
  │           → 使用默认 HTTP 客户端请求
  │
  ├─ 3. 获取 HTML
  │
  ├─ 4. 内容提取
  │     ├─ 配置有 body 选择器 → 使用配置的选择器
  │     ├─ 无配置 → readability 自动提取
  │     └─ 提取失败 → body fallback → raw HTML fallback
  │
  └─ 5. 存储结果
```

### 核心模块

**新增模块**: `src/site_config/`
- `parser.rs` — 配置文件解析器，解析 FTR 文本格式为结构体
- `store.rs` — 配置存储，管理内置库（rust-embed）和本地覆盖的查找
- `types.rs` — `SiteConfig` 结构体和辅助类型

**修改模块**:
- `src/tasks/fetcher.rs` — 集成站点配置，根据配置决定抓取方式和提取策略
- `src/config.rs` — 新增 `LETTURA_SITE_CONFIGS_PATH` 配置项

### 核心类型

```rust
/// 解析后的站点配置
pub struct SiteConfig {
    pub domain: String,

    // 抓取控制
    pub render: bool,
    pub extra_headers: Vec<(String, String)>,
    pub user_agent: Option<String>,
    pub timeout: Option<u64>,

    // 内容提取
    pub title_selectors: Vec<String>,
    pub body_selectors: Vec<String>,
    pub strip_selectors: Vec<String>,
    pub author_selector: Option<String>,
    pub date_selector: Option<String>,
    pub image_selector: Option<String>,

    // URL 匹配
    pub match_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

/// 配置存储
pub struct SiteConfigStore {
    local_path: Option<PathBuf>,  // 本地覆盖目录
    // 内置配置通过 rust-embed 访问
}
```

### 配置解析器

解析规则：
- 空行和 `#` 开头的行被忽略
- `key: value` 格式
- `key` 必须是指令集中的一个
- `title`, `body`, `strip`, `header`, `match`, `exclude` 可多行（追加）
- `render` 值为 `true`/`false`
- 选择器值支持逗号分隔的多选择器

## 内置配置库策略

### 来源

从 [fivefilters/ftr-site-config](https://github.com/fivefilters/ftr-site-config) 仓库精选常用的、高质量的配置文件。该仓库由社区维护，包含数千个网站的配置。

### 选择标准

1. 全球 Top 1000 网站中常见的新闻/博客/内容平台
2. 中文用户常用的网站（知乎、微信公众号、掘金等）
3. 需要 JS 渲染的知名 SPA 网站（Twitter、Medium 等）

### 格式转换

ftr-site-config 使用 XPath 选择器，Lettura 使用 CSS 选择器。对于大多数简单规则，可以机械转换。对于无法转换的复杂 XPath，需要手动调整为等效 CSS 选择器。

初始版本可以只收录能用 CSS 选择器表达的配置，后续再考虑增加 XPath 支持。

### 嵌入方式

使用已有的 `rust-embed` crate（前端 SPA 也是用同样方式嵌入的）：

```rust
#[derive(rust_embed::RustEmbed)]
#[folder = "site-configs/"]
struct BuiltInConfigs;
```

## 运行时配置

### 环境变量

| 环境变量 | 默认值 | 说明 |
|---------|--------|------|
| `LETTURA_SITE_CONFIGS_PATH` | `/data/site-configs` | 用户自定义配置文件目录 |

### 本地覆盖

用户可以在 `LETTURA_SITE_CONFIGS_PATH` 目录下放置 `{domain}.txt` 文件来覆盖内置配置。对于容器部署，通过 volume mount 挂载：

```yaml
volumes:
  - ./my-site-configs:/data/site-configs
```

## 与现有系统的兼容性

1. **DB site_rules 保持不变** — 通过 UI 管理的简单规则继续工作，作为文件配置的下一级 fallback
2. **前端无需改动** — 站点配置是后端行为，前端 API 不变
3. **向后兼容** — 没有任何站点配置时，行为与当前完全一致

## 测试策略

1. **解析器单元测试** — 各种格式的配置文件解析，边界情况（空文件、无效行、多选择器等）
2. **存储查找测试** — 优先级查找（本地覆盖 > 内置 > 无）
3. **URL 匹配测试** — match/exclude 模式匹配
4. **集成测试** — 用真实配置文件测试完整的抓取 + 提取流程

## 初始内置配置

第一版计划内置以下网站的配置：

**国际网站**: medium.com, twitter.com/x.com, github.com (readme), nytimes.com, bbc.com, theguardian.com, wikipedia.org, stackoverflow.com, reddit.com

**中文网站**: sspai.com, zhihu.com, juejin.cn, 36kr.com, infoq.cn, ruanyifeng.com

**特别处理**: 需要渲染的网站标记 `render: true`

## 未来扩展

- XPath 选择器支持（如果 CSS 不够用）
- 规则热更新（从 Git 仓库同步最新配置）
- 管理后台 UI（查看和编辑站点配置）
- 配置文件分享社区
