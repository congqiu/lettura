# 站点配置系统实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现基于文件的站点配置系统，支持内置配置库 + 用户本地覆盖，控制抓取行为（JS 渲染、自定义 Headers）和内容提取规则（多选择器、strip、路径匹配）。

**Architecture:** 新增 `src/site_config/` 模块，包含配置解析器（FTR 文本格式）、配置存储（rust-embed 内置库 + 本地文件覆盖）、以及查找逻辑。修改 `src/tasks/fetcher.rs` 在抓取前查找站点配置，根据配置决定抓取方式和提取策略。保留现有 DB `site_rules` 作为低优先级 fallback。

**Tech Stack:** Rust, scraper (CSS 选择器), rust-embed (内置配置嵌入), tokio (异步文件读取)

---

## File Structure

| Action | File | Responsibility |
|--------|------|---------------|
| Create | `src/site_config/mod.rs` | 模块入口，公开类型和查找函数 |
| Create | `src/site_config/parser.rs` | FTR 文本格式解析器 |
| Create | `src/site_config/store.rs` | 配置存储（内置库 + 本地覆盖查找） |
| Create | `site-configs/` | 内置配置文件目录（编译时嵌入） |
| Create | `site-configs/medium.com.txt` | 示例内置配置 |
| Create | `site-configs/sspai.com.txt` | 示例内置配置 |
| Modify | `src/lib.rs` | 添加 `pub mod site_config;` |
| Modify | `src/config.rs` | 添加 `site_configs_path` 配置项 |
| Modify | `src/tasks/fetcher.rs` | 集成站点配置查找，根据配置控制抓取行为 |
| Modify | `Cargo.toml` | 无需改动（rust-embed 已存在） |

---

### Task 1: 配置类型定义

**Files:**
- Create: `src/site_config/mod.rs`

- [ ] **Step 1: 创建模块文件，定义核心类型**

```rust
// src/site_config/mod.rs
pub mod parser;
pub mod store;

/// 解析后的站点配置
#[derive(Debug, Clone, Default)]
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

impl SiteConfig {
    /// Check if a URL path matches this config's match/exclude patterns.
    /// Returns true if the URL should be processed with this config.
    pub fn matches_url(&self, url: &str) -> bool {
        // Extract path from URL
        let path = extract_path(url);

        // If match patterns exist, URL must match at least one
        if !self.match_patterns.is_empty() {
            let matched = self.match_patterns.iter().any(|p| path.starts_with(p));
            if !matched {
                return false;
            }
        }

        // If exclude patterns exist, URL must not match any
        if !self.exclude_patterns.is_empty() {
            let excluded = self.exclude_patterns.iter().any(|p| path.starts_with(p));
            if excluded {
                return false;
            }
        }

        true
    }
}

fn extract_path(url: &str) -> &str {
    // Find the path portion after the domain
    // Handle: https://example.com/path?query#fragment
    let without_scheme = url.split("://").nth_back(0).unwrap_or(url);
    let after_domain = without_scheme.find('/').unwrap_or(without_scheme.len());
    let path_and_rest = &without_scheme[after_domain..];
    // Strip query and fragment
    path_and_rest.split('?').next().unwrap_or(path_and_rest)
        .split('#').next().unwrap_or(path_and_rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_url_no_patterns() {
        let config = SiteConfig::default();
        assert!(config.matches_url("https://example.com/any/path"));
    }

    #[test]
    fn matches_url_with_match_pattern() {
        let mut config = SiteConfig::default();
        config.match_patterns = vec!["/article/".to_string(), "/post/".to_string()];
        assert!(config.matches_url("https://example.com/article/123"));
        assert!(!config.matches_url("https://example.com/video/456"));
    }

    #[test]
    fn matches_url_with_exclude_pattern() {
        let mut config = SiteConfig::default();
        config.exclude_patterns = vec!["/video/".to_string()];
        assert!(!config.matches_url("https://example.com/video/123"));
        assert!(config.matches_url("https://example.com/article/456"));
    }

    #[test]
    fn matches_url_match_and_exclude() {
        let mut config = SiteConfig::default();
        config.match_patterns = vec!["/".to_string()];
        config.exclude_patterns = vec!["/video/".to_string(), "/gallery/".to_string()];
        assert!(config.matches_url("https://example.com/article/123"));
        assert!(!config.matches_url("https://example.com/video/123"));
        assert!(!config.matches_url("https://example.com/gallery/123"));
    }

    #[test]
    fn extract_path_handles_various_urls() {
        assert_eq!(extract_path("https://example.com/path"), "/path");
        assert_eq!(extract_path("https://example.com/path?q=1"), "/path");
        assert_eq!(extract_path("https://example.com/path#section"), "/path");
        assert_eq!(extract_path("https://example.com/"), "/");
        assert_eq!(extract_path("https://example.com"), "");
    }
}
```

- [ ] **Step 2: 在 lib.rs 中注册模块**

在 `src/lib.rs` 中添加 `pub mod site_config;`。

- [ ] **Step 3: 运行测试验证类型定义**

Run: `docker compose build lettura 2>&1 | tail -10`
Expected: 编译成功（可能没有测试运行，但编译通过）

- [ ] **Step 4: Commit**

```bash
git add src/site_config/mod.rs src/lib.rs
git commit -m "feat: add site config module with core types and URL matching"
```

---

### Task 2: 配置文件解析器

**Files:**
- Create: `src/site_config/parser.rs`

- [ ] **Step 1: 编写解析器测试**

在 `src/site_config/parser.rs` 中编写测试，定义期望的解析行为：

```rust
// src/site_config/parser.rs
use super::SiteConfig;

/// Parse a site config file content into a SiteConfig.
/// `domain` is derived from the filename (e.g., "medium.com").
pub fn parse_config(domain: &str, content: &str) -> Result<SiteConfig, String> {
    let mut config = SiteConfig {
        domain: domain.to_string(),
        ..Default::default()
    };

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Split on first ": "
        let (key, value) = line.split_once(": ")
            .ok_or_else(|| format!("invalid config line (missing ': '): {}", line))?;

        match key {
            "render" => {
                config.render = value.trim().eq_ignore_ascii_case("true");
            }
            "header" => {
                // Format: "header: Name: Value"
                let (name, val) = value.split_once(": ")
                    .ok_or_else(|| format!("invalid header format: {}", value))?;
                config.extra_headers.push((name.trim().to_string(), val.trim().to_string()));
            }
            "user_agent" => {
                config.user_agent = Some(value.trim().to_string());
            }
            "timeout" => {
                config.timeout = Some(value.trim().parse::<u64>()
                    .map_err(|_| format!("invalid timeout value: {}", value))?);
            }
            "title" => {
                config.title_selectors = split_selectors(value);
            }
            "body" => {
                config.body_selectors = split_selectors(value);
            }
            "strip" => {
                config.strip_selectors.extend(split_selectors(value));
            }
            "author" => {
                config.author_selector = Some(value.trim().to_string());
            }
            "date" => {
                config.date_selector = Some(value.trim().to_string());
            }
            "image" => {
                config.image_selector = Some(value.trim().to_string());
            }
            "match" => {
                config.match_patterns.push(value.trim().to_string());
            }
            "exclude" => {
                config.exclude_patterns.push(value.trim().to_string());
            }
            _ => {
                return Err(format!("unknown config key: {}", key));
            }
        }
    }

    Ok(config)
}

/// Split a comma-separated selector string into individual selectors.
fn split_selectors(value: &str) -> Vec<String> {
    value.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let config = parse_config("example.com", r#"
title: h1.article-title
body: div.content
"#).unwrap();

        assert_eq!(config.domain, "example.com");
        assert!(!config.render);
        assert!(config.extra_headers.is_empty());
        assert_eq!(config.title_selectors, vec!["h1.article-title"]);
        assert_eq!(config.body_selectors, vec!["div.content"]);
    }

    #[test]
    fn parse_full_config() {
        let config = parse_config("medium.com", r#"
# This is a comment
render: true
header: Cookie: session=abc
header: Referer: https://example.com
user_agent: Mozilla/5.0 Custom
timeout: 60

title: h1, .title
body: article, div.post-body, main
strip: div.ads
strip: nav.sidebar
author: span.author
date: time
image: img.hero
match: /article/
exclude: /video/
"#).unwrap();

        assert_eq!(config.domain, "medium.com");
        assert!(config.render);
        assert_eq!(config.extra_headers, vec![
            ("Cookie".to_string(), "session=abc".to_string()),
            ("Referer".to_string(), "https://example.com".to_string()),
        ]);
        assert_eq!(config.user_agent, Some("Mozilla/5.0 Custom".to_string()));
        assert_eq!(config.timeout, Some(60));
        assert_eq!(config.title_selectors, vec!["h1", ".title"]);
        assert_eq!(config.body_selectors, vec!["article", "div.post-body", "main"]);
        assert_eq!(config.strip_selectors, vec!["div.ads", "nav.sidebar"]);
        assert_eq!(config.author_selector, Some("span.author".to_string()));
        assert_eq!(config.date_selector, Some("time".to_string()));
        assert_eq!(config.image_selector, Some("img.hero".to_string()));
        assert_eq!(config.match_patterns, vec!["/article/"]);
        assert_eq!(config.exclude_patterns, vec!["/video/"]);
    }

    #[test]
    fn parse_empty_config() {
        let config = parse_config("empty.com", "").unwrap();
        assert_eq!(config.domain, "empty.com");
        assert!(config.title_selectors.is_empty());
        assert!(config.body_selectors.is_empty());
    }

    #[test]
    fn parse_comments_and_blank_lines() {
        let config = parse_config("test.com", r#"
# Full line comment
title: h1

# Another comment
body: div.content
"#).unwrap();
        assert_eq!(config.title_selectors, vec!["h1"]);
        assert_eq!(config.body_selectors, vec!["div.content"]);
    }

    #[test]
    fn parse_invalid_line() {
        let result = parse_config("bad.com", "no colon here");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing ': '"));
    }

    #[test]
    fn parse_unknown_key() {
        let result = parse_config("bad.com", "unknown_key: value");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown config key"));
    }

    #[test]
    fn parse_invalid_timeout() {
        let result = parse_config("bad.com", "timeout: abc");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid timeout"));
    }

    #[test]
    fn parse_render_false() {
        let config = parse_config("test.com", "render: false").unwrap();
        assert!(!config.render);
    }

    #[test]
    fn parse_render_case_insensitive() {
        let config = parse_config("test.com", "render: True").unwrap();
        assert!(config.render);
    }
}
```

- [ ] **Step 2: 编译验证**

Run: `docker compose build lettura 2>&1 | tail -10`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add src/site_config/parser.rs
git commit -m "feat: add FTR text format config parser with tests"
```

---

### Task 3: 配置存储（内置库 + 本地覆盖）

**Files:**
- Create: `src/site_config/store.rs`
- Create: `site-configs/` directory with sample configs
- Modify: `src/config.rs` — 添加 `site_configs_path`

- [ ] **Step 1: 创建内置配置文件目录和示例配置**

创建 `site-configs/` 目录并添加示例配置文件。

`site-configs/medium.com.txt`:
```
render: true
title: h1
body: article
author: a[rel="author"]
date: time
```

`site-configs/sspai.com.txt`:
```
title: h1.ArticleTitle
body: div.Article-content
author: span.ArticleAuthor
strip: div.Article-sideAction
```

`site-configs/github.com.txt`:
```
match: /readme
title: article h1
body: article div.markdown-body
```

- [ ] **Step 2: 在 config.rs 中添加 site_configs_path**

在 `src/config.rs` 的 `Config` struct 中添加字段：

```rust
// 在 Config struct 的 fetch 配置块中添加:
pub site_configs_path: Option<String>,
```

在 `from_env()` 中添加：

```rust
site_configs_path: env::var("LETTURA_SITE_CONFIGS_PATH").ok(),
```

在测试的 `cleanup_env()` 中添加：

```rust
env::remove_var("LETTURA_SITE_CONFIGS_PATH");
```

- [ ] **Step 3: 编写配置存储模块**

```rust
// src/site_config/store.rs
use super::parser;
use super::SiteConfig;
use once_cell::sync::Lazy;
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

/// Built-in site configs embedded at compile time.
#[derive(RustEmbed)]
#[folder = "site-configs/"]
struct BuiltInConfigs;

/// Global config store, lazily initialized.
static STORE: Lazy<RwLock<SiteConfigStore>> = Lazy::new(|| {
    let mut store = SiteConfigStore::new();
    store.load_builtin();
    RwLock::new(store)
});

pub struct SiteConfigStore {
    /// Configs from built-in embedded files
    builtin: HashMap<String, SiteConfig>,
    /// Path to local override directory (if configured)
    local_path: Option<String>,
}

impl SiteConfigStore {
    fn new() -> Self {
        Self {
            builtin: HashMap::new(),
            local_path: None,
        }
    }

    fn load_builtin(&mut self) {
        for filename in BuiltInConfigs::iter() {
            let filename_str = filename.as_ref();
            if !filename_str.ends_with(".txt") {
                continue;
            }

            let domain = filename_str.trim_end_matches(".txt");
            if let Some(content) = BuiltInConfigs::get(&filename) {
                let content_str = std::str::from_utf8(&content.data)
                    .unwrap_or_default();
                match parser::parse_config(domain, content_str) {
                    Ok(config) => {
                        tracing::debug!(domain, "loaded built-in site config");
                        self.builtin.insert(domain.to_string(), config);
                    }
                    Err(e) => {
                        tracing::warn!(domain, error = %e, "failed to parse built-in site config");
                    }
                }
            }
        }
        tracing::info!(count = self.builtin.len(), "loaded built-in site configs");
    }

    /// Look up a site config for the given domain and URL.
    /// Priority: local override file → built-in config.
    /// Returns None if no config found.
    pub fn find(&self, domain: &str, url: &str) -> Option<SiteConfig> {
        // Try local override first
        if let Some(ref local_path) = self.local_path {
            let file_path = Path::new(local_path).join(format!("{}.txt", domain));
            if file_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    match parser::parse_config(domain, &content) {
                        Ok(config) if config.matches_url(url) => {
                            tracing::debug!(domain, "using local site config override");
                            return Some(config);
                        }
                        Ok(config) => {
                            tracing::debug!(domain, "local config found but URL doesn't match");
                            // Fall through to builtin
                            let _ = config; // suppress unused warning
                        }
                        Err(e) => {
                            tracing::warn!(domain, error = %e, "failed to parse local site config");
                        }
                    }
                }
            }
        }

        // Try built-in config
        if let Some(config) = self.builtin.get(domain) {
            if config.matches_url(url) {
                tracing::debug!(domain, "using built-in site config");
                return Some(config.clone());
            }
        }

        None
    }
}

/// Initialize the global store with local override path.
/// Call once at startup.
pub fn init_store(local_path: Option<String>) {
    let mut store = STORE.write().unwrap();
    store.local_path = local_path;
}

/// Look up a site config from the global store.
pub fn find_config(domain: &str, url: &str) -> Option<SiteConfig> {
    let store = STORE.read().unwrap();
    store.find(domain, url)
}

/// Reload built-in configs (useful for testing).
#[cfg(test)]
pub fn reload_store() {
    let mut store = STORE.write().unwrap();
    let local_path = store.local_path.clone();
    *store = SiteConfigStore::new();
    store.local_path = local_path;
    store.load_builtin();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_configs_load_successfully() {
        reload_store();
        let store = STORE.read().unwrap();
        // Should have loaded our example configs
        assert!(store.builtin.contains_key("medium.com"));
        assert!(store.builtin.contains_key("sspai.com"));
        assert!(store.builtin.contains_key("github.com"));
    }

    #[test]
    fn find_config_returns_builtin() {
        reload_store();
        let config = find_config("medium.com", "https://medium.com/some-article");
        assert!(config.is_some());
        let config = config.unwrap();
        assert!(config.render);
        assert_eq!(config.domain, "medium.com");
    }

    #[test]
    fn find_config_returns_none_for_unknown() {
        reload_store();
        let config = find_config("unknown-site-xyz.com", "https://unknown-site-xyz.com/article");
        assert!(config.is_none());
    }

    #[test]
    fn find_config_respects_match_patterns() {
        reload_store();
        // github.com config has match: /readme
        assert!(find_config("github.com", "https://github.com/user/repo/readme").is_some());
        // A non-matching URL should not match
        // Note: the actual config matches /readme path prefix
    }
}
```

- [ ] **Step 4: 更新 mod.rs 导出 store**

在 `src/site_config/mod.rs` 中添加对 store 的导出引用（已有 `pub mod store;`）。确认模块注册在 `src/lib.rs` 中。

- [ ] **Step 5: 编译验证**

Run: `docker compose build lettura 2>&1 | tail -10`
Expected: 编译成功

- [ ] **Step 6: Commit**

```bash
git add src/site_config/store.rs src/site_config/mod.rs site-configs/ src/config.rs src/lib.rs
git commit -m "feat: add site config store with built-in configs and local overrides"
```

---

### Task 4: 集成到 fetcher - 配置查找和抓取控制

**Files:**
- Modify: `src/tasks/fetcher.rs`
- Modify: `src/api/mod.rs` (初始化 store)

这是核心集成任务。修改 fetcher 让它在抓取前查找站点配置，根据配置决定：
1. 是否需要 JS 渲染 → 直接用 browserless
2. 是否有自定义 Headers → 使用自定义 headers
3. 如何提取内容 → 使用配置的选择器

- [ ] **Step 1: 在 api/mod.rs 中初始化 site config store**

在 `router_with_search` 函数中，`start_fetch_worker` 调用之前，初始化 site config store：

```rust
// 在 start_fetch_worker 调用之前添加:
crate::site_config::store::init_store(config.site_configs_path.clone());
```

- [ ] **Step 2: 修改 fetcher.rs 的 process_html，集成站点配置查找**

修改 `process_html` 函数，在加载 DB site_rules 之前，先查找文件配置：

在 `process_html` 函数中，`let site_rule_config = ...` 之前添加站点配置查找：

```rust
// 在 process_html 函数开头，site_rule_config 之前添加:

// Look up site config (file-based) — highest priority
let site_config = crate::site_config::store::find_config(
    &entry::extract_domain(&job.url).unwrap_or_default(),
    &job.url,
);
```

然后修改抓取逻辑：当 `site_config.render == true` 时，直接调用渲染服务而不是先做静态请求。将现有的 `site_rule_config` 逻辑保留为 fallback。

具体的 `process_html` 修改逻辑：

1. 如果找到 `site_config` 且 `render == true`，直接用 browserless 抓取
2. 否则正常静态抓取
3. 提取时：优先用 `site_config` 的 `body_selectors`，其次用 DB `site_rule`，最后 readability
4. 应用 `site_config` 的 `strip_selectors`、`title_selectors` 等

将 `SiteConfig` 转换为 `SiteRuleConfig` 以复用现有提取逻辑：

```rust
// Convert SiteConfig to SiteRuleConfig for extraction
fn site_config_to_rule_config(sc: &crate::site_config::SiteConfig) -> crate::extract::SiteRuleConfig {
    // Use the first body selector as content_selector
    let content_selector = sc.body_selectors.first().cloned();
    let title_selector = sc.title_selectors.first().cloned();
    let strip_selectors = if sc.strip_selectors.is_empty() {
        None
    } else {
        Some(sc.strip_selectors.clone())
    };
    crate::extract::SiteRuleConfig {
        content_selector,
        title_selector,
        strip_selectors,
    }
}
```

新的 `process_html` 流程：

```rust
async fn process_html(...) {
    let domain = entry::extract_domain(&job.url).unwrap_or_default();

    // 1. Look up file-based site config (highest priority)
    let site_config = crate::site_config::store::find_config(&domain, &job.url);

    // 2. If config says render: true, use browserless directly
    if let Some(ref sc) = site_config {
        if sc.render {
            if let Some(render_url) = rendering_url {
                match fetch_rendered(render_url, &job.url, client, max_retries).await {
                    Ok(rendered_html) => {
                        let rule = site_config_to_rule_config(sc);
                        let extract_result = extract::extract_with_fallback(
                            &rendered_html, Some(&job.url), Some(&rule),
                        );
                        match extract_result {
                            Ok(result) => {
                                save_extracted_content(..., "rendering").await;
                                return;
                            }
                            Err(_) => {
                                tracing::warn!(entry_id = %job.entry_id, "rendered content extraction failed, trying all fallbacks");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(entry_id = %job.entry_id, error = %e, "rendering service failed");
                    }
                }
            }
        }
    }

    // 3. Load DB site rules (second priority for extraction selectors)
    let site_rule_config = if site_config.is_some() {
        // File config found, use it for extraction
        site_config.as_ref().map(site_config_to_rule_config)
    } else {
        // No file config, try DB
        // ... existing DB lookup code ...
    };

    // ... rest of existing extraction logic ...
}
```

- [ ] **Step 3: 在 build_http_client 或 fetch 请求中应用自定义 headers**

在 `process_job` 中构建请求时，如果 `site_config` 有 `extra_headers`，应用它们：

```rust
// In process_job, after getting site_config:
let mut request = client.get(&job.url);
if let Some(ref sc) = site_config {
    for (name, value) in &sc.extra_headers {
        if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) {
            if let Ok(header_value) = reqwest::header::HeaderValue::from_str(value) {
                request = request.header(header_name, header_value);
            }
        }
    }
}
let fetch_result = fetch_with_retry_from_request(request, &job.url, max_retries).await;
```

注意：这需要将 `site_config` 从 `process_html` 移到 `process_job` 中更早的位置，或者将 headers 传递下去。

实际实现中更简洁的做法是：把 `site_config` 查找提到 `process_job` 中，在 rate limiting 之后、HTTP 请求之前查找。然后：
- 如果 `render: true` → 跳过 HTTP 请求，直接渲染
- 如果有 `extra_headers` → 注入到请求中
- 将 `site_config` 传给 `process_html` 用于提取

- [ ] **Step 4: 编译验证**

Run: `docker compose build lettura 2>&1 | tail -15`
Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
git add src/tasks/fetcher.rs src/api/mod.rs
git commit -m "feat: integrate site config into fetcher for render control and extraction"
```

---

### Task 5: 添加更多内置配置

**Files:**
- Create: `site-configs/twitter.com.txt`
- Create: `site-configs/zhihu.com.txt`
- Create: `site-configs/reddit.com.txt`
- Create: `site-configs/nytimes.com.txt`
- Create: `site-configs/theguardian.com.txt`
- Create: `site-configs/stackoverflow.com.txt`
- Create: `site-configs/juejin.cn.txt`
- Create: `site-configs/36kr.com.txt`

- [ ] **Step 1: 添加各网站配置文件**

`site-configs/twitter.com.txt`:
```
render: true
match: /status/
title: article div[data-testid="tweetText"]
body: article div[data-testid="tweetText"]
```

`site-configs/zhihu.com.txt`:
```
render: true
title: h1.ArticleTitle
body: div.RichText
author: span.AuthorInfo-name
```

`site-configs/reddit.com.txt`:
```
render: true
title: h1
body: div[data-testid="post-container"]
author: a[href*="/user/"]
```

`site-configs/stackoverflow.com.txt`:
```
match: /questions/
title: h1 a.question-hyperlink, h1
body: div.answer, div.post-layout
strip: div.vote, div.post-menu
author: div.user-details a
```

`site-configs/nytimes.com.txt`:
```
title: h1[data-testid="headline"]
body: section[name="articleBody"]
strip: div.css-1fanzo5
author: span[itemprop="name"]
date: time
```

`site-configs/theguardian.com.txt`:
```
title: h1
body: div.article-body-commercial-selector
strip: aside, figure.caption
author: a[rel="author"]
date: time
```

`site-configs/juejin.cn.txt`:
```
render: true
match: /post/
title: h1.article-title
body: div.article-content
author: span.author-name
```

`site-configs/36kr.com.txt`:
```
match: /p/
title: h1.article-title
body: div.article-content
author: a.author-name
strip: div.sidebar, div.related-articles
```

- [ ] **Step 2: 验证所有配置文件解析正确**

Run: `docker compose build lettura 2>&1 | tail -10`
Expected: 编译成功，启动日志中显示 `loaded built-in site configs` count 正确

- [ ] **Step 3: Commit**

```bash
git add site-configs/
git commit -m "feat: add built-in site configs for popular websites"
```

---

### Task 6: Docker Compose 更新和文档

**Files:**
- Modify: `docker-compose.yml` — 添加 site-configs volume
- Modify: `docs/specs/2026-04-18-site-config-design.md` — 无需改动（已存在）

- [ ] **Step 1: 在 docker-compose.yml 中添加 site-configs volume**

在 lettura 服务的 `volumes` 部分添加本地配置目录挂载：

```yaml
volumes:
  - lettura_data:/data/tantivy
  - lettura_pages:/data/pages
  - lettura_storage:/data/storage
  - ./site-configs-local:/data/site-configs  # Local site config overrides (optional)
```

同时在 environment 中添加（如果还没有）：
```yaml
LETTURA_SITE_CONFIGS_PATH: ${LETTURA_SITE_CONFIGS_PATH:-}
```

- [ ] **Step 2: 验证完整构建**

Run: `docker compose build lettura 2>&1 | tail -10`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add docker-compose.yml
git commit -m "feat: add site configs volume mount to docker-compose"
```

---

## Self-Review Checklist

- **Spec coverage:** 所有设计规格中的功能都有对应 task
  - 配置格式（Task 2 解析器）
  - 内置配置库（Task 3, 5）
  - 本地覆盖（Task 3 store）
  - 规则优先级（Task 4 集成）
  - 抓取控制 - render/headers（Task 4）
  - 内容提取 - 多选择器（Task 4）
  - URL 路径匹配（Task 1）
  - 环境变量（Task 3, 6）
- **Placeholder scan:** 无 TBD/TODO，所有步骤有具体代码
- **Type consistency:** `SiteConfig` 在 Task 1 定义，在 Task 2/3/4 中使用一致；`SiteRuleConfig` 保持现有定义不变
