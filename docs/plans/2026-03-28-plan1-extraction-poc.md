# Plan 1: 项目脚手架 + 内容提取 PoC

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭建 Rust 项目骨架，实现纯 Rust 内容提取引擎并通过快照测试验证质量，决定是否继续纯 Rust 方案或切换到 Node.js 备选方案。

**Architecture:** 纯库项目（无 Web 服务），包含 extract 模块实现 Readability 算法。通过 HTML 快照文件做单元测试和集成测试，与 Readability.js 输出对比验证提取质量。

**Tech Stack:** Rust 2024 edition, scraper (HTML 解析), ammonia (HTML 清洗), reqwest (HTTP 抓取), regex, unicode-segmentation (CJK 分词)

**2 周时间盒：** 如果 Task 7 的 PoC 评估未达到 80% 匹配度，切换到 Node.js 备选方案（见 Task 8）。

---

## 文件结构

```
lettura/
├── Cargo.toml
├── src/
│   ├── lib.rs                    — 库入口，re-export extract 模块
│   └── extract/
│       ├── mod.rs                — ExtractResult 定义 + extract() 入口函数
│       ├── preprocess.rs         — HTML 预处理（移除 script/style/hidden）
│       ├── scoring.rs            — 节点评分算法
│       ├── readability.rs        — 正文候选节点选取 + 内容提取
│       ├── metadata.rs           — 标题、作者、语言、封面图提取
│       └── sanitize.rs           — ammonia HTML 白名单清洗
├── tests/
│   ├── fixtures/                 — HTML 快照文件
│   │   ├── blog_en_simple.html
│   │   ├── blog_en_simple.expected.txt
│   │   ├── news_zh.html
│   │   ├── news_zh.expected.txt
│   │   ├── tech_article.html
│   │   ├── tech_article.expected.txt
│   │   └── github_readme.html
│   └── extraction_test.rs        — 集成测试
├── scripts/
│   └── generate_expected.js      — 用 Readability.js 生成期望输出
└── docs/
    ├── specs/
    └── plans/
```

---

### Task 1: 项目脚手架

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/extract/mod.rs`

- [ ] **Step 1: 初始化 Cargo.toml**

```toml
[package]
name = "lettura"
version = "0.1.0"
edition = "2024"

[dependencies]
scraper = "0.22"
ammonia = "4"
regex = "1"
once_cell = "1"
unicode-segmentation = "1.12"
url = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"

[dev-dependencies]
pretty_assertions = "1"
```

- [ ] **Step 2: 创建 src/lib.rs**

```rust
pub mod extract;
```

- [ ] **Step 3: 创建 src/extract/mod.rs 定义核心类型**

```rust
pub mod metadata;
pub mod preprocess;
pub mod readability;
pub mod sanitize;
pub mod scoring;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ExtractResult {
    pub title: Option<String>,
    pub content: String,
    pub text_content: String,
    pub author: Option<String>,
    pub language: Option<String>,
    pub preview_image: Option<String>,
    pub excerpt: Option<String>,
    pub reading_time: u32,
}

#[derive(Error, Debug)]
pub enum ExtractError {
    #[error("failed to parse HTML")]
    ParseError,
    #[error("no article content found")]
    NoContent,
}

/// 从原始 HTML 提取文章内容的入口函数
pub fn extract(html: &str, url: Option<&str>) -> Result<ExtractResult, ExtractError> {
    let preprocessed = preprocess::preprocess(html);
    let document = scraper::Html::parse_document(&preprocessed);

    let meta = metadata::extract_metadata(&document, url);
    let article_html = readability::extract_content(&document)?;
    let clean_html = sanitize::sanitize(&article_html);
    let text_content = html_to_text(&clean_html);
    let reading_time = estimate_reading_time(&text_content);

    Ok(ExtractResult {
        title: meta.title,
        content: clean_html,
        text_content,
        author: meta.author,
        language: meta.language,
        preview_image: meta.preview_image,
        excerpt: meta.excerpt,
        reading_time,
    })
}

fn html_to_text(html: &str) -> String {
    let frag = scraper::Html::parse_fragment(html);
    frag.root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn estimate_reading_time(text: &str) -> u32 {
    use unicode_segmentation::UnicodeSegmentation;
    // 英文约 230 wpm, 中文约 400 cpm
    let word_count = text.unicode_words().count();
    let minutes = (word_count as f64 / 230.0).ceil() as u32;
    minutes.max(1)
}
```

- [ ] **Step 4: 创建占位子模块使其编译通过**

创建以下空文件（后续 Task 会填充实现）:

`src/extract/preprocess.rs`:
```rust
/// 预处理 HTML：移除 script、style、注释等非内容元素
pub fn preprocess(html: &str) -> String {
    html.to_string()
}
```

`src/extract/scoring.rs`:
```rust
/// 节点评分相关函数（占位）
```

`src/extract/readability.rs`:
```rust
use super::ExtractError;

/// 从 DOM 中提取正文 HTML
pub fn extract_content(document: &scraper::Html) -> Result<String, ExtractError> {
    Err(ExtractError::NoContent)
}
```

`src/extract/metadata.rs`:
```rust
pub struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub preview_image: Option<String>,
    pub excerpt: Option<String>,
}

pub fn extract_metadata(document: &scraper::Html, _url: Option<&str>) -> Metadata {
    Metadata {
        title: None,
        author: None,
        language: None,
        preview_image: None,
        excerpt: None,
    }
}
```

`src/extract/sanitize.rs`:
```rust
pub fn sanitize(html: &str) -> String {
    html.to_string()
}
```

- [ ] **Step 5: 验证编译通过**

Run: `cd /home/cc/workspace/lettura && cargo check`
Expected: 编译通过，可能有 unused 警告

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/
git commit -m "feat: init project scaffolding with extract module types"
```

---

### Task 2: HTML 预处理模块

**Files:**
- Modify: `src/extract/preprocess.rs`
- Create: `tests/fixtures/` (测试 HTML)

- [ ] **Step 1: 写失败测试**

在 `src/extract/preprocess.rs` 底部添加:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_script_tags() {
        let html = r#"<html><body><p>Hello</p><script>alert('x')</script></body></html>"#;
        let result = preprocess(html);
        assert!(!result.contains("<script"), "script tags should be removed");
        assert!(result.contains("Hello"));
    }

    #[test]
    fn removes_style_tags() {
        let html = r#"<html><body><p>Hello</p><style>.x{color:red}</style></body></html>"#;
        let result = preprocess(html);
        assert!(!result.contains("<style"), "style tags should be removed");
    }

    #[test]
    fn removes_hidden_elements() {
        let html = r#"<html><body><p>Visible</p><div style="display:none">Hidden</div><div aria-hidden="true">Also hidden</div></body></html>"#;
        let result = preprocess(html);
        assert!(result.contains("Visible"));
        assert!(!result.contains("Hidden"), "hidden elements should be removed");
    }

    #[test]
    fn removes_nav_and_footer() {
        let html = r#"<html><body><nav>Menu</nav><article><p>Content</p></article><footer>Footer</footer></body></html>"#;
        let result = preprocess(html);
        assert!(result.contains("Content"));
        assert!(!result.contains("Menu"), "nav should be removed");
        assert!(!result.contains("Footer"), "footer should be removed");
    }

    #[test]
    fn removes_html_comments() {
        let html = r#"<html><body><!-- comment --><p>Text</p></body></html>"#;
        let result = preprocess(html);
        assert!(!result.contains("comment"), "HTML comments should be removed");
        assert!(result.contains("Text"));
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --lib extract::preprocess`
Expected: FAIL — preprocess 目前是 identity function

- [ ] **Step 3: 实现预处理逻辑**

替换 `src/extract/preprocess.rs` 全部内容:

```rust
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{Html, Selector};

static REMOVE_TAGS: Lazy<Vec<Selector>> = Lazy::new(|| {
    [
        "script", "style", "noscript", "iframe", "object", "embed",
        "applet", "nav", "footer", "aside",
        "link[rel=stylesheet]", "meta[http-equiv=refresh]",
    ]
    .iter()
    .filter_map(|s| Selector::parse(s).ok())
    .collect()
});

static HIDDEN_SELECTOR: Lazy<Selector> = Lazy::new(|| {
    Selector::parse("[style*='display:none'], [style*='display: none'], [aria-hidden='true'], [hidden]").unwrap()
});

static COMMENT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"<!--[\s\S]*?-->").unwrap()
});

/// 预处理 HTML：移除 script、style、hidden 元素、注释等
pub fn preprocess(html: &str) -> String {
    // 先移除注释（避免干扰解析）
    let html = COMMENT_RE.replace_all(html, "");

    let document = Html::parse_document(&html);
    let mut ids_to_remove = std::collections::HashSet::new();

    // 收集需要移除的元素
    for selector in REMOVE_TAGS.iter() {
        for element in document.select(selector) {
            ids_to_remove.insert(element.id());
        }
    }
    for element in document.select(&HIDDEN_SELECTOR) {
        ids_to_remove.insert(element.id());
    }

    // 移除 class/id 明显是非内容的元素
    if let Ok(all_sel) = Selector::parse("*") {
        for element in document.select(&all_sel) {
            let val = element.value();
            let class = val.attr("class").unwrap_or("");
            let id = val.attr("id").unwrap_or("");
            let combined = format!("{} {}", class, id).to_lowercase();

            let dominated_by_negative = is_unlikely_candidate(&combined)
                && !is_positive_candidate(&combined)
                && val.name() != "body"
                && val.name() != "html"
                && val.name() != "article";

            if dominated_by_negative {
                ids_to_remove.insert(element.id());
            }
        }
    }

    // 重建 HTML，排除标记的节点及其子节点
    rebuild_html(&document, &ids_to_remove)
}

fn is_unlikely_candidate(text: &str) -> bool {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)banner|breadcrumbs|combx|comment|community|cover-wrap|disqus|extra|footer|gdpr|header|legends|menu|related|remark|replies|rss|shoutbox|sidebar|skyscraper|social|sponsor|supplemental|ad-break|agegate|pagination|pager|popup|yom-resolve").unwrap()
    });
    RE.is_match(text)
}

fn is_positive_candidate(text: &str) -> bool {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)article|body|content|entry|hentry|h-entry|main|page|pagination|post|text|blog|story").unwrap()
    });
    RE.is_match(text)
}

fn rebuild_html(
    document: &Html,
    exclude_ids: &std::collections::HashSet<scraper::node::NodeId>,
) -> String {
    use scraper::node::Node;
    let mut output = String::new();

    fn walk(
        node_ref: ego_tree::NodeRef<Node>,
        exclude_ids: &std::collections::HashSet<scraper::node::NodeId>,
        output: &mut String,
    ) {
        if exclude_ids.contains(&node_ref.id()) {
            return;
        }
        // 检查祖先是否被排除
        let mut ancestor = node_ref.parent();
        while let Some(a) = ancestor {
            if exclude_ids.contains(&a.id()) {
                return;
            }
            ancestor = a.parent();
        }

        match node_ref.value() {
            Node::Element(el) => {
                output.push('<');
                output.push_str(el.name());
                for (key, val) in el.attrs() {
                    output.push(' ');
                    output.push_str(key);
                    output.push_str("=\"");
                    output.push_str(val);
                    output.push('"');
                }
                output.push('>');
                for child in node_ref.children() {
                    walk(child, exclude_ids, output);
                }
                output.push_str("</");
                output.push_str(el.name());
                output.push('>');
            }
            Node::Text(text) => {
                output.push_str(text);
            }
            Node::Document => {
                for child in node_ref.children() {
                    walk(child, exclude_ids, output);
                }
            }
            _ => {}
        }
    }

    walk(document.tree.root(), exclude_ids, &mut output);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_script_tags() {
        let html = r#"<html><body><p>Hello</p><script>alert('x')</script></body></html>"#;
        let result = preprocess(html);
        assert!(!result.contains("<script"), "script tags should be removed");
        assert!(result.contains("Hello"));
    }

    #[test]
    fn removes_style_tags() {
        let html = r#"<html><body><p>Hello</p><style>.x{color:red}</style></body></html>"#;
        let result = preprocess(html);
        assert!(!result.contains("<style"), "style tags should be removed");
    }

    #[test]
    fn removes_hidden_elements() {
        let html = r#"<html><body><p>Visible</p><div style="display:none">Hidden</div><div aria-hidden="true">Also hidden</div></body></html>"#;
        let result = preprocess(html);
        assert!(result.contains("Visible"));
        assert!(!result.contains("Hidden"), "hidden elements should be removed");
    }

    #[test]
    fn removes_nav_and_footer() {
        let html = r#"<html><body><nav>Menu</nav><article><p>Content</p></article><footer>Footer</footer></body></html>"#;
        let result = preprocess(html);
        assert!(result.contains("Content"));
        assert!(!result.contains("Menu"), "nav should be removed");
        assert!(!result.contains("Footer"), "footer should be removed");
    }

    #[test]
    fn removes_html_comments() {
        let html = r#"<html><body><!-- comment --><p>Text</p></body></html>"#;
        let result = preprocess(html);
        assert!(!result.contains("comment"), "HTML comments should be removed");
        assert!(result.contains("Text"));
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --lib extract::preprocess`
Expected: 5 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/extract/preprocess.rs
git commit -m "feat: implement HTML preprocessing (strip scripts/styles/hidden/nav)"
```

---

### Task 3: 节点评分算法

**Files:**
- Modify: `src/extract/scoring.rs`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    #[test]
    fn paragraph_heavy_div_scores_high() {
        let html = r#"<html><body>
            <div id="content">
                <p>This is a long paragraph with substantial text content that should score well in the readability algorithm because it contains meaningful words.</p>
                <p>Another paragraph with more text content here. The scoring algorithm should give this container a high score based on text density.</p>
            </div>
            <div id="sidebar"><a href="#">Link1</a><a href="#">Link2</a><a href="#">Link3</a></div>
        </body></html>"#;
        let doc = Html::parse_document(html);
        let scores = score_nodes(&doc);

        // content div should score higher than sidebar div
        let content_score = find_score_by_id(&doc, &scores, "content");
        let sidebar_score = find_score_by_id(&doc, &scores, "sidebar");
        assert!(
            content_score > sidebar_score,
            "content ({}) should score higher than sidebar ({})",
            content_score, sidebar_score
        );
    }

    #[test]
    fn article_tag_gets_bonus() {
        let html = r#"<html><body>
            <article><p>Article text content with enough words to matter here.</p></article>
            <div><p>Same text content with enough words to matter here too.</p></div>
        </body></html>"#;
        let doc = Html::parse_document(html);
        let scores = score_nodes(&doc);
        let all: Vec<_> = scores.iter().collect();
        // article element should have a higher base score
        assert!(!all.is_empty());
    }

    #[test]
    fn high_link_density_scores_low() {
        let html = r#"<html><body>
            <div id="nav"><a href="#">Link</a> <a href="#">Link</a> <a href="#">Link</a> <a href="#">Link</a> <a href="#">Link</a></div>
            <div id="article"><p>This is substantial paragraph text without many links at all in the content area.</p></div>
        </body></html>"#;
        let doc = Html::parse_document(html);
        let scores = score_nodes(&doc);
        let nav_score = find_score_by_id(&doc, &scores, "nav");
        let article_score = find_score_by_id(&doc, &scores, "article");
        assert!(
            article_score > nav_score,
            "article ({}) should score higher than nav ({})",
            article_score, nav_score
        );
    }

    fn find_score_by_id(
        doc: &Html,
        scores: &std::collections::HashMap<scraper::node::NodeId, f64>,
        id: &str,
    ) -> f64 {
        let sel = scraper::Selector::parse(&format!("#{}", id)).unwrap();
        doc.select(&sel)
            .next()
            .and_then(|el| scores.get(&el.id()).copied())
            .unwrap_or(0.0)
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --lib extract::scoring`
Expected: FAIL — score_nodes 函数不存在

- [ ] **Step 3: 实现评分算法**

替换 `src/extract/scoring.rs` 全部内容:

```rust
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{node::NodeId, ElementRef, Html, Selector};
use std::collections::HashMap;

static POSITIVE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)article|body|content|entry|hentry|h-entry|main|page|pagination|post|text|blog|story|column").unwrap()
});

static NEGATIVE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)hidden|banner|combx|comment|com-|contact|footer|footnote|gdpr|masthead|media|meta|outbrain|promo|related|scroll|share|shoutbox|sidebar|skyscraper|sponsor|shopping|tags|tool|widget").unwrap()
});

/// 对文档中所有元素节点计算内容评分
pub fn score_nodes(document: &Html) -> HashMap<NodeId, f64> {
    let mut scores: HashMap<NodeId, f64> = HashMap::new();
    let p_selector = Selector::parse("p, pre, td").unwrap();

    for element in document.select(&p_selector) {
        let text = element.text().collect::<String>();
        let text = text.trim();
        // 忽略太短的段落
        if text.len() < 25 {
            continue;
        }

        // 获取父节点和祖父节点
        let parent = element.parent().and_then(ElementRef::wrap);
        let grandparent = parent.and_then(|p| p.parent().and_then(ElementRef::wrap));

        if let Some(parent_el) = parent {
            let parent_id = parent_el.id();
            if !scores.contains_key(&parent_id) {
                scores.insert(parent_id, initial_score(parent_el));
            }

            let content_score = compute_content_score(text);
            *scores.entry(parent_id).or_insert(0.0) += content_score;

            if let Some(gp_el) = grandparent {
                let gp_id = gp_el.id();
                if !scores.contains_key(&gp_id) {
                    scores.insert(gp_id, initial_score(gp_el));
                }
                // 祖父节点获得一半分数
                *scores.entry(gp_id).or_insert(0.0) += content_score / 2.0;
            }
        }
    }

    // 对所有已评分节点应用链接密度惩罚
    let scored_ids: Vec<NodeId> = scores.keys().copied().collect();
    for node_id in scored_ids {
        if let Some(element) = document
            .tree
            .get(node_id)
            .and_then(|n| ElementRef::wrap(n))
        {
            let link_density = compute_link_density(element);
            if let Some(score) = scores.get_mut(&node_id) {
                *score *= 1.0 - link_density;
            }
        }
    }

    scores
}

/// 根据标签名和 class/id 计算初始分数
fn initial_score(element: ElementRef) -> f64 {
    let tag_score = match element.value().name() {
        "article" => 10.0,
        "div" | "section" => 5.0,
        "pre" | "td" | "blockquote" => 3.0,
        "address" | "ol" | "ul" | "dl" | "dd" | "dt" | "li" | "form" => -3.0,
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "th" => -5.0,
        _ => 0.0,
    };

    let class = element.value().attr("class").unwrap_or("");
    let id = element.value().attr("id").unwrap_or("");
    let class_id = format!("{} {}", class, id);

    let class_score = if POSITIVE_RE.is_match(&class_id) {
        25.0
    } else if NEGATIVE_RE.is_match(&class_id) {
        -25.0
    } else {
        0.0
    };

    tag_score + class_score
}

/// 计算段落文本的内容分数
fn compute_content_score(text: &str) -> f64 {
    let mut score = 1.0;
    // 逗号和句号加分
    score += text.matches(',').count() as f64;
    score += text.matches('，').count() as f64;
    score += text.matches('。').count() as f64;
    // 每 100 字符加 1 分，最多 3 分
    score += (text.len() as f64 / 100.0).min(3.0);
    score
}

/// 计算元素内链接文本占总文本的比例
fn compute_link_density(element: ElementRef) -> f64 {
    let total_text: String = element.text().collect();
    let total_len = total_text.trim().len();
    if total_len == 0 {
        return 0.0;
    }

    let a_selector = Selector::parse("a").unwrap();
    let link_text_len: usize = element
        .select(&a_selector)
        .map(|a| a.text().collect::<String>().trim().len())
        .sum();

    link_text_len as f64 / total_len as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    #[test]
    fn paragraph_heavy_div_scores_high() {
        let html = r#"<html><body>
            <div id="content">
                <p>This is a long paragraph with substantial text content that should score well in the readability algorithm because it contains meaningful words.</p>
                <p>Another paragraph with more text content here. The scoring algorithm should give this container a high score based on text density.</p>
            </div>
            <div id="sidebar"><a href="#">Link1</a><a href="#">Link2</a><a href="#">Link3</a></div>
        </body></html>"#;
        let doc = Html::parse_document(html);
        let scores = score_nodes(&doc);

        let content_score = find_score_by_id(&doc, &scores, "content");
        let sidebar_score = find_score_by_id(&doc, &scores, "sidebar");
        assert!(
            content_score > sidebar_score,
            "content ({}) should score higher than sidebar ({})",
            content_score, sidebar_score
        );
    }

    #[test]
    fn article_tag_gets_bonus() {
        let html = r#"<html><body>
            <article id="art"><p>Article text content with enough words to matter here in this test.</p></article>
            <div id="dv"><p>Div text content with enough words to matter here in this test too.</p></div>
        </body></html>"#;
        let doc = Html::parse_document(html);
        let scores = score_nodes(&doc);
        let art_score = find_score_by_id(&doc, &scores, "art");
        let div_score = find_score_by_id(&doc, &scores, "dv");
        assert!(
            art_score > div_score,
            "article ({}) should score higher than div ({})",
            art_score, div_score
        );
    }

    #[test]
    fn high_link_density_scores_low() {
        let html = r#"<html><body>
            <div id="nav"><p><a href="#">Link</a> <a href="#">Link</a> <a href="#">Link</a> <a href="#">Link</a> <a href="#">Link</a> some padding text to reach threshold</p></div>
            <div id="article"><p>This is substantial paragraph text without many links at all in the content area for testing.</p></div>
        </body></html>"#;
        let doc = Html::parse_document(html);
        let scores = score_nodes(&doc);
        let nav_score = find_score_by_id(&doc, &scores, "nav");
        let article_score = find_score_by_id(&doc, &scores, "article");
        assert!(
            article_score > nav_score,
            "article ({}) should score higher than nav ({})",
            article_score, nav_score
        );
    }

    fn find_score_by_id(
        doc: &Html,
        scores: &HashMap<NodeId, f64>,
        id: &str,
    ) -> f64 {
        let sel = Selector::parse(&format!("#{}", id)).unwrap();
        doc.select(&sel)
            .next()
            .and_then(|el| scores.get(&el.id()).copied())
            .unwrap_or(0.0)
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --lib extract::scoring`
Expected: 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/extract/scoring.rs
git commit -m "feat: implement node scoring algorithm for readability extraction"
```

---

### Task 4: 正文提取管道

**Files:**
- Modify: `src/extract/readability.rs`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    #[test]
    fn extracts_article_content() {
        let html = r#"<html><body>
            <header><h1>Title</h1><nav>Menu items</nav></header>
            <div class="article-body">
                <p>First paragraph of the article with enough content to be considered substantial by the scoring algorithm.</p>
                <p>Second paragraph continues the article with more meaningful text content that adds to the overall score.</p>
                <p>Third paragraph wraps up the main points of this test article with additional text for scoring.</p>
            </div>
            <div class="sidebar">
                <a href="#">Related link 1</a>
                <a href="#">Related link 2</a>
            </div>
        </body></html>"#;

        let preprocessed = crate::extract::preprocess::preprocess(html);
        let doc = Html::parse_document(&preprocessed);
        let result = extract_content(&doc).unwrap();

        assert!(result.contains("First paragraph"), "should contain article text");
        assert!(result.contains("Second paragraph"), "should contain article text");
        assert!(!result.contains("Related link"), "should not contain sidebar");
    }

    #[test]
    fn returns_error_for_empty_content() {
        let html = r#"<html><body></body></html>"#;
        let doc = Html::parse_document(html);
        let result = extract_content(&doc);
        assert!(result.is_err());
    }

    #[test]
    fn extracts_from_article_tag() {
        let html = r#"<html><body>
            <article>
                <p>This is the main article content that should be extracted because it is inside an article tag and has sufficient text length.</p>
                <p>More content inside the article element to ensure proper scoring behavior in the extraction pipeline.</p>
            </article>
            <footer><p>Footer content should not appear</p></footer>
        </body></html>"#;

        let preprocessed = crate::extract::preprocess::preprocess(html);
        let doc = Html::parse_document(&preprocessed);
        let result = extract_content(&doc).unwrap();

        assert!(result.contains("main article content"));
        assert!(!result.contains("Footer content"));
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --lib extract::readability`
Expected: FAIL

- [ ] **Step 3: 实现正文提取**

替换 `src/extract/readability.rs` 全部内容:

```rust
use super::scoring::score_nodes;
use super::ExtractError;
use scraper::{ElementRef, Html, Selector};

/// 从预处理后的 DOM 中提取得分最高的正文节点的内容 HTML
pub fn extract_content(document: &Html) -> Result<String, ExtractError> {
    let scores = score_nodes(document);

    if scores.is_empty() {
        return Err(ExtractError::NoContent);
    }

    // 选取最高分节点
    let top_node_id = scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(id, _)| *id)
        .ok_or(ExtractError::NoContent)?;

    let top_element = document
        .tree
        .get(top_node_id)
        .and_then(ElementRef::wrap)
        .ok_or(ExtractError::NoContent)?;

    // 从得分最高的节点中收集有价值的子内容
    let content = collect_content(top_element, &scores);

    if content.trim().is_empty() {
        return Err(ExtractError::NoContent);
    }

    Ok(content)
}

/// 收集节点内的有价值内容，过滤低分子节点
fn collect_content(element: ElementRef, scores: &std::collections::HashMap<scraper::node::NodeId, f64>) -> String {
    let mut parts: Vec<String> = Vec::new();
    let top_score = scores.get(&element.id()).copied().unwrap_or(0.0);

    for child in element.children() {
        if let Some(child_el) = ElementRef::wrap(child) {
            let tag = child_el.value().name();

            // 保留段落、标题、引用、列表、pre、figure 等内容元素
            if matches!(tag, "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                | "blockquote" | "pre" | "ul" | "ol" | "figure" | "img" | "table") {
                parts.push(child_el.html());
                continue;
            }

            // 对 div/section 等容器，检查是否有足够的文本内容
            if matches!(tag, "div" | "section" | "span" | "article") {
                let child_score = scores.get(&child_el.id()).copied().unwrap_or(0.0);
                let text: String = child_el.text().collect();
                let text_len = text.trim().len();

                // 如果子节点得分足够高或文本足够长，保留
                if child_score >= top_score * 0.2 || text_len > 80 {
                    parts.push(child_el.html());
                }
                continue;
            }

            // 其他内联元素直接保留
            if matches!(tag, "a" | "strong" | "em" | "b" | "i" | "br" | "code" | "mark") {
                parts.push(child_el.html());
            }
        } else if let Some(text) = child.value().as_text() {
            let t = text.trim();
            if !t.is_empty() {
                parts.push(t.to_string());
            }
        }
    }

    // 如果没有收集到有意义的部分，回退到整个节点的 innerHTML
    if parts.is_empty() || parts.iter().all(|p| p.trim().is_empty()) {
        return element.inner_html();
    }

    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    #[test]
    fn extracts_article_content() {
        let html = r#"<html><body>
            <header><h1>Title</h1><nav>Menu items</nav></header>
            <div class="article-body">
                <p>First paragraph of the article with enough content to be considered substantial by the scoring algorithm.</p>
                <p>Second paragraph continues the article with more meaningful text content that adds to the overall score.</p>
                <p>Third paragraph wraps up the main points of this test article with additional text for scoring.</p>
            </div>
            <div class="sidebar">
                <a href="#">Related link 1</a>
                <a href="#">Related link 2</a>
            </div>
        </body></html>"#;

        let preprocessed = crate::extract::preprocess::preprocess(html);
        let doc = Html::parse_document(&preprocessed);
        let result = extract_content(&doc).unwrap();

        assert!(result.contains("First paragraph"), "should contain article text");
        assert!(result.contains("Second paragraph"), "should contain article text");
        assert!(!result.contains("Related link"), "should not contain sidebar");
    }

    #[test]
    fn returns_error_for_empty_content() {
        let html = r#"<html><body></body></html>"#;
        let doc = Html::parse_document(html);
        let result = extract_content(&doc);
        assert!(result.is_err());
    }

    #[test]
    fn extracts_from_article_tag() {
        let html = r#"<html><body>
            <article>
                <p>This is the main article content that should be extracted because it is inside an article tag and has sufficient text length.</p>
                <p>More content inside the article element to ensure proper scoring behavior in the extraction pipeline.</p>
            </article>
            <footer><p>Footer content should not appear</p></footer>
        </body></html>"#;

        let preprocessed = crate::extract::preprocess::preprocess(html);
        let doc = Html::parse_document(&preprocessed);
        let result = extract_content(&doc).unwrap();

        assert!(result.contains("main article content"));
        assert!(!result.contains("Footer content"));
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --lib extract::readability`
Expected: 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/extract/readability.rs
git commit -m "feat: implement article content extraction with top-scoring node selection"
```

---

### Task 5: 元数据提取

**Files:**
- Modify: `src/extract/metadata.rs`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    #[test]
    fn extracts_title_from_og_meta() {
        let html = r#"<html><head><meta property="og:title" content="OG Title"><title>Page Title - Site</title></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.title.as_deref(), Some("OG Title"));
    }

    #[test]
    fn falls_back_to_title_tag() {
        let html = r#"<html><head><title>Page Title</title></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.title.as_deref(), Some("Page Title"));
    }

    #[test]
    fn extracts_author() {
        let html = r#"<html><head><meta name="author" content="John Doe"></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.author.as_deref(), Some("John Doe"));
    }

    #[test]
    fn extracts_language_from_html_attr() {
        let html = r#"<html lang="zh-CN"><head></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.language.as_deref(), Some("zh-CN"));
    }

    #[test]
    fn extracts_preview_image() {
        let html = r#"<html><head><meta property="og:image" content="https://example.com/img.jpg"></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.preview_image.as_deref(), Some("https://example.com/img.jpg"));
    }

    #[test]
    fn extracts_description_as_excerpt() {
        let html = r#"<html><head><meta name="description" content="A short summary"></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.excerpt.as_deref(), Some("A short summary"));
    }

    #[test]
    fn cleans_title_site_suffix() {
        let html = r#"<html><head><title>Article Title | My Site</title></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.title.as_deref(), Some("Article Title"));
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --lib extract::metadata`
Expected: FAIL — 函数返回全 None

- [ ] **Step 3: 实现元数据提取**

替换 `src/extract/metadata.rs` 全部内容:

```rust
use scraper::{Html, Selector};

pub struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub preview_image: Option<String>,
    pub excerpt: Option<String>,
}

pub fn extract_metadata(document: &Html, _url: Option<&str>) -> Metadata {
    Metadata {
        title: extract_title(document),
        author: extract_meta_content(document, &[
            ("name", "author"),
            ("property", "article:author"),
        ]),
        language: extract_language(document),
        preview_image: extract_meta_content(document, &[
            ("property", "og:image"),
            ("name", "twitter:image"),
        ]),
        excerpt: extract_meta_content(document, &[
            ("property", "og:description"),
            ("name", "description"),
            ("name", "twitter:description"),
        ]),
    }
}

fn extract_title(document: &Html) -> Option<String> {
    // 优先: og:title
    if let Some(og) = extract_meta_content(document, &[("property", "og:title")]) {
        return Some(og);
    }

    // 回退: <title> 标签，清理站点名后缀
    let title_sel = Selector::parse("title").ok()?;
    let title_el = document.select(&title_sel).next()?;
    let raw = title_el.text().collect::<String>().trim().to_string();

    if raw.is_empty() {
        return None;
    }

    Some(clean_title(&raw))
}

fn clean_title(title: &str) -> String {
    // 按常见分隔符拆分，取最长的部分
    let separators = [" | ", " - ", " — ", " :: ", " / ", " » "];
    for sep in &separators {
        if title.contains(sep) {
            let parts: Vec<&str> = title.split(sep).collect();
            if let Some(longest) = parts.iter().max_by_key(|p| p.len()) {
                let cleaned = longest.trim().to_string();
                if !cleaned.is_empty() {
                    return cleaned;
                }
            }
        }
    }
    title.to_string()
}

fn extract_language(document: &Html) -> Option<String> {
    // 从 <html lang="..."> 提取
    let html_sel = Selector::parse("html").ok()?;
    let html_el = document.select(&html_sel).next()?;
    html_el
        .value()
        .attr("lang")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            // 回退: content-language meta
            extract_meta_content(document, &[("http-equiv", "content-language")])
        })
}

fn extract_meta_content(document: &Html, attrs: &[(&str, &str)]) -> Option<String> {
    let meta_sel = Selector::parse("meta").ok()?;
    for meta in document.select(&meta_sel) {
        let el = meta.value();
        for (attr_name, attr_value) in attrs {
            if el.attr(attr_name).map(|v| v.eq_ignore_ascii_case(attr_value)) == Some(true) {
                if let Some(content) = el.attr("content") {
                    let trimmed = content.trim().to_string();
                    if !trimmed.is_empty() {
                        return Some(trimmed);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    #[test]
    fn extracts_title_from_og_meta() {
        let html = r#"<html><head><meta property="og:title" content="OG Title"><title>Page Title - Site</title></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.title.as_deref(), Some("OG Title"));
    }

    #[test]
    fn falls_back_to_title_tag() {
        let html = r#"<html><head><title>Page Title</title></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.title.as_deref(), Some("Page Title"));
    }

    #[test]
    fn extracts_author() {
        let html = r#"<html><head><meta name="author" content="John Doe"></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.author.as_deref(), Some("John Doe"));
    }

    #[test]
    fn extracts_language_from_html_attr() {
        let html = r#"<html lang="zh-CN"><head></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.language.as_deref(), Some("zh-CN"));
    }

    #[test]
    fn extracts_preview_image() {
        let html = r#"<html><head><meta property="og:image" content="https://example.com/img.jpg"></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.preview_image.as_deref(), Some("https://example.com/img.jpg"));
    }

    #[test]
    fn extracts_description_as_excerpt() {
        let html = r#"<html><head><meta name="description" content="A short summary"></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.excerpt.as_deref(), Some("A short summary"));
    }

    #[test]
    fn cleans_title_site_suffix() {
        let html = r#"<html><head><title>Article Title | My Site</title></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.title.as_deref(), Some("Article Title"));
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --lib extract::metadata`
Expected: 7 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/extract/metadata.rs
git commit -m "feat: implement metadata extraction (title, author, language, image, excerpt)"
```

---

### Task 6: HTML 清洗

**Files:**
- Modify: `src/extract/sanitize.rs`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_safe_tags() {
        let html = r#"<p>Hello <strong>world</strong></p>"#;
        let result = sanitize(html);
        assert!(result.contains("<p>"));
        assert!(result.contains("<strong>"));
    }

    #[test]
    fn removes_script_in_content() {
        let html = r#"<p>Text</p><script>evil()</script>"#;
        let result = sanitize(html);
        assert!(!result.contains("script"));
        assert!(result.contains("Text"));
    }

    #[test]
    fn removes_event_handlers() {
        let html = r#"<p onclick="evil()">Text</p>"#;
        let result = sanitize(html);
        assert!(!result.contains("onclick"));
        assert!(result.contains("Text"));
    }

    #[test]
    fn keeps_images_with_src() {
        let html = r#"<img src="https://example.com/img.jpg" alt="photo">"#;
        let result = sanitize(html);
        assert!(result.contains("img"));
        assert!(result.contains("src"));
    }

    #[test]
    fn keeps_links() {
        let html = r#"<a href="https://example.com">Link</a>"#;
        let result = sanitize(html);
        assert!(result.contains("href"));
        assert!(result.contains("Link"));
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --lib extract::sanitize`
Expected: FAIL — sanitize 是 identity function，script 不会被移除

- [ ] **Step 3: 实现 HTML 清洗**

替换 `src/extract/sanitize.rs` 全部内容:

```rust
use ammonia::Builder;
use std::collections::HashSet;

/// 使用白名单策略清洗 HTML，只保留安全的阅读相关标签和属性
pub fn sanitize(html: &str) -> String {
    let mut tags = HashSet::new();
    for tag in &[
        "p", "br", "hr", "h1", "h2", "h3", "h4", "h5", "h6",
        "strong", "em", "b", "i", "u", "s", "mark", "small", "sub", "sup",
        "blockquote", "pre", "code", "kbd", "samp",
        "ul", "ol", "li", "dl", "dt", "dd",
        "a", "img", "figure", "figcaption",
        "table", "thead", "tbody", "tfoot", "tr", "th", "td", "caption",
        "div", "span", "section", "article",
        "details", "summary", "time", "abbr",
    ] {
        tags.insert(*tag);
    }

    let mut tag_attrs = std::collections::HashMap::new();
    tag_attrs.insert("a", vec!["href", "title"].into_iter().collect::<HashSet<&str>>());
    tag_attrs.insert("img", vec!["src", "alt", "title", "width", "height"].into_iter().collect());
    tag_attrs.insert("td", vec!["colspan", "rowspan"].into_iter().collect());
    tag_attrs.insert("th", vec!["colspan", "rowspan"].into_iter().collect());
    tag_attrs.insert("time", vec!["datetime"].into_iter().collect());
    tag_attrs.insert("abbr", vec!["title"].into_iter().collect());

    Builder::new()
        .tags(tags)
        .tag_attributes(tag_attrs.into_iter().map(|(k, v)| (k, v)).collect())
        .link_rel(Some("noopener noreferrer"))
        .clean(html)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_safe_tags() {
        let html = r#"<p>Hello <strong>world</strong></p>"#;
        let result = sanitize(html);
        assert!(result.contains("<p>"));
        assert!(result.contains("<strong>"));
    }

    #[test]
    fn removes_script_in_content() {
        let html = r#"<p>Text</p><script>evil()</script>"#;
        let result = sanitize(html);
        assert!(!result.contains("script"));
        assert!(result.contains("Text"));
    }

    #[test]
    fn removes_event_handlers() {
        let html = r#"<p onclick="evil()">Text</p>"#;
        let result = sanitize(html);
        assert!(!result.contains("onclick"));
        assert!(result.contains("Text"));
    }

    #[test]
    fn keeps_images_with_src() {
        let html = r#"<img src="https://example.com/img.jpg" alt="photo">"#;
        let result = sanitize(html);
        assert!(result.contains("img"));
        assert!(result.contains("src"));
    }

    #[test]
    fn keeps_links() {
        let html = r#"<a href="https://example.com">Link</a>"#;
        let result = sanitize(html);
        assert!(result.contains("href"));
        assert!(result.contains("Link"));
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --lib extract::sanitize`
Expected: 5 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/extract/sanitize.rs
git commit -m "feat: implement HTML sanitization with ammonia whitelist"
```

---

### Task 7: 端到端集成测试 + 快照测试

**Files:**
- Create: `tests/extraction_test.rs`
- Create: `tests/fixtures/blog_en_simple.html`
- Create: `scripts/generate_expected.js`

- [ ] **Step 1: 创建测试用 HTML fixture**

`tests/fixtures/blog_en_simple.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta property="og:title" content="Understanding Rust Ownership">
    <meta name="author" content="Jane Smith">
    <meta name="description" content="A deep dive into Rust's ownership system.">
    <meta property="og:image" content="https://example.com/rust.jpg">
    <title>Understanding Rust Ownership - Tech Blog</title>
</head>
<body>
    <header>
        <nav><a href="/">Home</a> <a href="/about">About</a></nav>
    </header>
    <main>
        <article class="post-content">
            <h1>Understanding Rust Ownership</h1>
            <p class="meta">Published on Jan 1, 2026 by Jane Smith</p>
            <p>Rust's ownership system is one of its most distinctive features. It enables memory safety without garbage collection, making Rust programs both fast and safe. In this article, we will explore the three rules of ownership and how they work together to prevent common bugs.</p>
            <h2>The Three Rules</h2>
            <p>Every value in Rust has a single owner. When the owner goes out of scope, the value is dropped. Ownership can be transferred through moves or shared through references. These simple rules eliminate entire categories of bugs at compile time, including use-after-free, double-free, and data races.</p>
            <h2>Borrowing and References</h2>
            <p>Instead of transferring ownership, you can create references to values. References come in two flavors: shared references (&T) which allow multiple readers, and mutable references (&mut T) which allow a single writer. The borrow checker enforces these rules at compile time, ensuring thread safety without runtime overhead.</p>
            <p>Understanding these concepts is essential for writing idiomatic Rust code. While the learning curve can be steep, the safety guarantees are well worth the investment.</p>
        </article>
    </main>
    <aside class="sidebar">
        <h3>Related Posts</h3>
        <ul>
            <li><a href="/post1">Rust for Beginners</a></li>
            <li><a href="/post2">Async Rust Guide</a></li>
            <li><a href="/post3">Error Handling in Rust</a></li>
        </ul>
    </aside>
    <footer>
        <p>Copyright 2026 Tech Blog. All rights reserved.</p>
    </footer>
    <script>console.log('analytics');</script>
</body>
</html>
```

- [ ] **Step 2: 创建 Readability.js 期望输出生成脚本**

`scripts/generate_expected.js`:

```javascript
// 用法: node scripts/generate_expected.js tests/fixtures/blog_en_simple.html
// 需要: npm install @mozilla/readability jsdom
const { Readability } = require("@mozilla/readability");
const { JSDOM } = require("jsdom");
const fs = require("fs");
const path = require("path");

const filePath = process.argv[2];
if (!filePath) {
    console.error("Usage: node generate_expected.js <html-file>");
    process.exit(1);
}

const html = fs.readFileSync(filePath, "utf-8");
const doc = new JSDOM(html, { url: "https://example.com/article" });
const reader = new Readability(doc.window.document);
const article = reader.parse();

if (!article) {
    console.error("Readability failed to parse");
    process.exit(1);
}

const output = {
    title: article.title,
    textContent: article.textContent.trim(),
    excerpt: article.excerpt,
    byline: article.byline,
    length: article.length,
};

const outPath = filePath.replace(".html", ".expected.json");
fs.writeFileSync(outPath, JSON.stringify(output, null, 2));
console.log(`Written to ${outPath}`);
```

- [ ] **Step 3: 生成期望输出文件（手动或通过脚本）**

如果本机有 Node.js:
```bash
cd /home/cc/workspace/lettura
npm init -y --silent
npm install @mozilla/readability jsdom --save-dev
node scripts/generate_expected.js tests/fixtures/blog_en_simple.html
```

如果没有 Node.js，手动创建 `tests/fixtures/blog_en_simple.expected.json`:
```json
{
  "title": "Understanding Rust Ownership",
  "textContent_contains": [
    "ownership system is one of its most distinctive",
    "Every value in Rust has a single owner",
    "shared references",
    "mutable references"
  ],
  "textContent_excludes": [
    "Related Posts",
    "Copyright 2026",
    "analytics"
  ]
}
```

- [ ] **Step 4: 写集成测试**

`tests/extraction_test.rs`:

```rust
use lettura::extract;
use pretty_assertions::assert_eq;

fn load_fixture(name: &str) -> String {
    let path = format!("tests/fixtures/{}", name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path, e))
}

fn load_expected(name: &str) -> serde_json::Value {
    let path = format!("tests/fixtures/{}", name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read expected {}: {}", path, e));
    serde_json::from_str(&content).unwrap()
}

#[test]
fn blog_en_simple_extracts_content() {
    let html = load_fixture("blog_en_simple.html");
    let result = extract::extract(&html, Some("https://example.com/article")).unwrap();

    // 元数据验证
    assert_eq!(result.title.as_deref(), Some("Understanding Rust Ownership"));
    assert_eq!(result.author.as_deref(), Some("Jane Smith"));
    assert_eq!(result.language.as_deref(), Some("en"));
    assert_eq!(result.preview_image.as_deref(), Some("https://example.com/rust.jpg"));

    // 内容验证: 包含文章正文
    let expected = load_expected("blog_en_simple.expected.json");

    for phrase in expected["textContent_contains"].as_array().unwrap() {
        let phrase = phrase.as_str().unwrap();
        assert!(
            result.text_content.contains(phrase),
            "Expected text_content to contain: '{}'\nGot: {}",
            phrase,
            &result.text_content[..result.text_content.len().min(500)]
        );
    }

    // 内容验证: 不包含非文章内容
    for phrase in expected["textContent_excludes"].as_array().unwrap() {
        let phrase = phrase.as_str().unwrap();
        assert!(
            !result.text_content.contains(phrase),
            "Expected text_content to NOT contain: '{}'",
            phrase
        );
    }

    // 阅读时间应该合理 (1-5 分钟)
    assert!(result.reading_time >= 1 && result.reading_time <= 5,
        "reading_time should be 1-5 min, got {}", result.reading_time);
}

#[test]
fn extract_returns_error_for_garbage_html() {
    let html = "<html><body><div></div></body></html>";
    let result = extract::extract(html, None);
    assert!(result.is_err(), "should fail for content-less HTML");
}

#[test]
fn extract_handles_chinese_content() {
    let html = r#"<html lang="zh-CN"><head><title>Rust 所有权机制详解</title></head><body>
        <article>
            <p>Rust 的所有权系统是其最独特的功能之一。它在没有垃圾回收的情况下实现内存安全，使 Rust 程序既快速又安全。在本文中，我们将探讨所有权的三条规则及其如何协同工作以防止常见错误。</p>
            <p>Rust 中的每个值都有一个唯一的所有者。当所有者离开作用域时，该值将被丢弃。所有权可以通过移动来转移，也可以通过引用来共享。这些简单的规则在编译时消除了整类错误，包括使用后释放、双重释放和数据竞争。</p>
        </article>
    </body></html>"#;

    let result = extract::extract(html, None).unwrap();
    assert_eq!(result.language.as_deref(), Some("zh-CN"));
    assert!(result.text_content.contains("所有权系统"));
    assert!(result.reading_time >= 1);
}
```

- [ ] **Step 5: 运行集成测试**

Run: `cargo test --test extraction_test`
Expected: 3 tests PASS（如果评分/提取算法工作正确）

如果有测试失败，根据失败原因调整评分算法参数（Task 3 中的阈值），直到通过为止。这是 PoC 迭代的一部分。

- [ ] **Step 6: Commit**

```bash
git add tests/ scripts/
git commit -m "feat: add extraction integration tests with HTML fixtures"
```

---

### Task 8: PoC 评估与决策

**Files:**
- 无新文件，这是一个评估步骤

- [ ] **Step 1: 运行全部测试**

Run: `cargo test`
Expected: 所有单元测试 + 集成测试 PASS

- [ ] **Step 2: 添加更多真实世界 fixture 测试**

收集更多 HTML 快照（保存真实网页的 HTML 到 `tests/fixtures/`），覆盖:
- 英文新闻 (如 CNN, BBC 风格)
- 中文博客
- 技术文章 (如 Medium 风格)
- GitHub README 页面
- 知乎/微信公众号文章

每个 fixture 创建对应的 `.expected.json` 文件。目标: 30+ 个测试用例。

- [ ] **Step 3: 评估提取质量**

对每个 fixture，人工对比提取结果:
- 正文是否完整提取？
- 是否包含非文章内容（广告、导航、评论）？
- 标题是否正确？
- 作者和语言是否正确？

通过率 >= 80% → **继续纯 Rust 方案**，进入 Plan 2
通过率 < 80% → **切换到 Node.js 备选方案**，执行 Task 9

- [ ] **Step 4: 记录评估结果**

将评估结果记录到 `docs/poc-evaluation.md`:

```markdown
# Content Extraction PoC Evaluation

Date: YYYY-MM-DD
Method: Pure Rust (scraper crate)

## Results

| Fixture | Title | Content | Sidebar Removed | Score |
|---------|-------|---------|-----------------|-------|
| blog_en_simple | ✅ | ✅ | ✅ | Pass |
| ... | ... | ... | ... | ... |

## Conclusion

Pass rate: XX/XX (XX%)
Decision: [Continue Rust / Switch to Node.js]
```

- [ ] **Step 5: Commit**

```bash
git add tests/fixtures/ docs/poc-evaluation.md
git commit -m "docs: record PoC extraction evaluation results"
```

---

### Task 9: Node.js 备选方案（仅在 Task 8 决定切换时执行）

**Files:**
- Create: `scripts/readability_extract.js`
- Modify: `src/extract/mod.rs`
- Create: `src/extract/node_fallback.rs`

> **仅在 Task 8 评估结果 < 80% 时执行此 Task。否则跳过。**

- [ ] **Step 1: 创建 Node.js 提取脚本**

`scripts/readability_extract.js`:

```javascript
const { Readability } = require("@mozilla/readability");
const { JSDOM } = require("jsdom");

// 从 stdin 读取 HTML，输出 JSON 到 stdout
let input = "";
process.stdin.setEncoding("utf-8");
process.stdin.on("data", (chunk) => (input += chunk));
process.stdin.on("end", () => {
    try {
        const url = process.argv[2] || "https://example.com";
        const doc = new JSDOM(input, { url });
        const reader = new Readability(doc.window.document);
        const article = reader.parse();

        if (!article) {
            process.stdout.write(JSON.stringify({ error: "parse_failed" }));
            process.exit(0);
        }

        process.stdout.write(JSON.stringify({
            title: article.title,
            content: article.content,
            textContent: article.textContent,
            excerpt: article.excerpt,
            byline: article.byline,
            lang: article.lang,
        }));
    } catch (e) {
        process.stdout.write(JSON.stringify({ error: e.message }));
    }
});
```

- [ ] **Step 2: 创建 Rust 端 Node.js 调用封装**

`src/extract/node_fallback.rs`:

```rust
use std::io::Write;
use std::process::{Command, Stdio};

use super::{ExtractError, ExtractResult};

/// 通过 Node.js 子进程调用 Readability.js 提取内容
pub fn extract_via_node(html: &str, url: Option<&str>) -> Result<ExtractResult, ExtractError> {
    let script_path = std::env::var("LETTURA_READABILITY_SCRIPT")
        .unwrap_or_else(|_| "scripts/readability_extract.js".to_string());

    let mut cmd = Command::new("node");
    cmd.arg(&script_path);
    if let Some(u) = url {
        cmd.arg(u);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|_| ExtractError::ParseError)?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(html.as_bytes()).ok();
    }

    let output = child.wait_with_output().map_err(|_| ExtractError::ParseError)?;

    if !output.status.success() {
        return Err(ExtractError::ParseError);
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|_| ExtractError::ParseError)?;

    if json.get("error").is_some() {
        return Err(ExtractError::NoContent);
    }

    let content = json["content"].as_str().unwrap_or("").to_string();
    let text_content = json["textContent"].as_str().unwrap_or("").to_string();

    if content.is_empty() {
        return Err(ExtractError::NoContent);
    }

    let reading_time = {
        let words = text_content.split_whitespace().count();
        ((words as f64 / 230.0).ceil() as u32).max(1)
    };

    Ok(ExtractResult {
        title: json["title"].as_str().map(String::from),
        content: super::sanitize::sanitize(&content),
        text_content,
        author: json["byline"].as_str().map(String::from),
        language: json["lang"].as_str().map(String::from),
        preview_image: None, // Readability.js 不提取图片
        excerpt: json["excerpt"].as_str().map(String::from),
        reading_time,
    })
}
```

- [ ] **Step 3: 修改 extract() 入口函数添加 fallback 逻辑**

在 `src/extract/mod.rs` 中添加:

```rust
pub mod node_fallback;

// 修改 extract() 函数:
pub fn extract(html: &str, url: Option<&str>) -> Result<ExtractResult, ExtractError> {
    let preprocessed = preprocess::preprocess(html);
    let document = scraper::Html::parse_document(&preprocessed);

    let meta = metadata::extract_metadata(&document, url);

    // 尝试纯 Rust 提取
    match readability::extract_content(&document) {
        Ok(article_html) => {
            let clean_html = sanitize::sanitize(&article_html);
            let text_content = html_to_text(&clean_html);
            let reading_time = estimate_reading_time(&text_content);
            Ok(ExtractResult {
                title: meta.title,
                content: clean_html,
                text_content,
                author: meta.author,
                language: meta.language,
                preview_image: meta.preview_image,
                excerpt: meta.excerpt,
                reading_time,
            })
        }
        Err(_) => {
            // Fallback 到 Node.js
            node_fallback::extract_via_node(html, url)
        }
    }
}
```

- [ ] **Step 4: 运行所有测试确认 fallback 工作**

Run: `cargo test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add scripts/readability_extract.js src/extract/node_fallback.rs src/extract/mod.rs
git commit -m "feat: add Node.js Readability.js fallback for content extraction"
```

---

## 后续计划

Plan 1 完成后（内容提取 PoC 通过），继续以下计划:

- **Plan 2: 核心后端** — DB 迁移, 数据模型, Auth (JWT), Entry CRUD API, 抓取队列
- **Plan 3: 高级功能** — Tags, Memos, Annotations, tantivy 全文搜索, 自动打标签规则, Import/Export, RSS, Admin API
- **Plan 4: 前端 SPA** — React + Vite + Tailwind, 核心页面, Tiptap 编辑器, 响应式/PWA
- **Plan 5: 浏览器扩展 + Docker** — Chrome/Firefox 扩展, Dockerfile, docker-compose
