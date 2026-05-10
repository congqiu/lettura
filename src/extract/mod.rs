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

#[derive(Debug, Clone, Default)]
pub struct SiteRuleConfig {
    pub content_selector: Option<String>,
    pub title_selector: Option<String>,
    pub strip_selectors: Option<Vec<String>>,
}

#[derive(Error, Debug)]
pub enum ExtractError {
    #[error("failed to parse HTML")]
    ParseError,
    #[error("no article content found")]
    NoContent,
}

/// Indicates which extraction method succeeded.
#[derive(Debug, Clone, PartialEq)]
pub enum ExtractMethod {
    SiteRule,
    Readability,
    BodyFallback,
    RawHtml,
}

/// Result from extract_with_fallback, including the method used.
pub struct FallbackExtractResult {
    pub inner: ExtractResult,
    pub method: ExtractMethod,
}

/// Extract article content using a multi-layer fallback strategy.
///
/// Strategy order:
/// 1. Site-specific CSS selectors (if provided)
/// 2. Readability algorithm
/// 3. `<body>` content extraction
/// 4. Sanitized raw HTML (last resort)
pub fn extract_with_fallback(
    html: &str,
    url: Option<&str>,
    site_rule: Option<&SiteRuleConfig>,
) -> Result<FallbackExtractResult, ExtractError> {
    let preprocessed = preprocess::preprocess(html);
    let mut document = scraper::Html::parse_document(&preprocessed);
    let mut meta = metadata::extract_metadata(&document, url);

    // Apply strip selectors from site rules
    if let Some(rule) = site_rule {
        if let Some(ref strip_selectors) = rule.strip_selectors {
            for sel_str in strip_selectors {
                if let Ok(sel) = scraper::Selector::parse(sel_str) {
                    let ids: Vec<_> = document.select(&sel).map(|el| el.id()).collect();
                    for id in ids {
                        if let Some(mut node) = document.tree.get_mut(id) {
                            node.detach();
                        }
                    }
                }
            }
        }
    }

    // Extract title — take from meta, site rule can override
    let meta_title = meta.title.take();
    let title = if let Some(rule) = site_rule {
        if let Some(ref title_selector) = rule.title_selector {
            extract_title_with_selector(&document, title_selector).or(meta_title)
        } else {
            meta_title
        }
    } else {
        meta_title
    };

    // Layer 1: Site-specific content selector
    if let Some(rule) = site_rule {
        if let Some(ref content_selector) = rule.content_selector {
            if let Ok(content) =
                readability::extract_content_with_selector(&document, content_selector)
            {
                let clean_html = sanitize::sanitize(&content);
                if !is_content_too_short(&clean_html) {
                    return Ok(FallbackExtractResult {
                        inner: build_result(title, clean_html, meta),
                        method: ExtractMethod::SiteRule,
                    });
                }
            }
        }
    }

    // Layer 2: Readability algorithm
    match readability::extract_content(&document) {
        Ok(content) => {
            let clean_html = sanitize::sanitize(&content);
            if !is_content_too_short(&clean_html) {
                return Ok(FallbackExtractResult {
                    inner: build_result(title, clean_html, meta),
                    method: ExtractMethod::Readability,
                });
            }
        }
        Err(_) => {}
    }

    // Layer 3: Body content extraction
    if let Ok(content) = extract_body_content(&document) {
        let clean_html = sanitize::sanitize(&content);
        if !is_content_too_short(&clean_html) {
            return Ok(FallbackExtractResult {
                inner: build_result(title, clean_html, meta),
                method: ExtractMethod::BodyFallback,
            });
        }
    }

    // Layer 4: Sanitized raw HTML as last resort
    let clean_html = sanitize::sanitize(html);
    if !is_content_too_short(&clean_html) {
        return Ok(FallbackExtractResult {
            inner: build_result(title, clean_html, meta),
            method: ExtractMethod::RawHtml,
        });
    }

    Err(ExtractError::NoContent)
}

/// Original extract function kept for backward compatibility.
pub fn extract(
    html: &str,
    url: Option<&str>,
    site_rule: Option<&SiteRuleConfig>,
) -> Result<ExtractResult, ExtractError> {
    let result = extract_with_fallback(html, url, site_rule)?;
    Ok(result.inner)
}

fn build_result(
    title: Option<String>,
    clean_html: String,
    meta: metadata::Metadata,
) -> ExtractResult {
    let text_content = html_to_text(&clean_html);
    let reading_time = estimate_reading_time(&text_content);
    ExtractResult {
        title,
        content: clean_html,
        text_content,
        author: meta.author,
        language: meta.language,
        preview_image: meta.preview_image,
        excerpt: meta.excerpt,
        reading_time,
    }
}

/// Check if extracted content is meaninglessly short.
fn is_content_too_short(html: &str) -> bool {
    let text = html_to_text(html);
    text.trim().len() < 50
}

/// Extract content from the `<body>` tag as a fallback.
fn extract_body_content(document: &scraper::Html) -> Result<String, ExtractError> {
    let selector = scraper::Selector::parse("body").map_err(|_| ExtractError::ParseError)?;
    let body = document
        .select(&selector)
        .next()
        .ok_or(ExtractError::NoContent)?;
    let content = body.inner_html();
    if content.trim().is_empty() {
        return Err(ExtractError::NoContent);
    }
    Ok(content)
}

fn extract_title_with_selector(document: &scraper::Html, selector: &str) -> Option<String> {
    let sel = scraper::Selector::parse(selector).ok()?;
    let el = document.select(&sel).next()?;
    let text = el.text().collect::<String>().trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

pub fn html_to_text(html: &str) -> String {
    let frag = scraper::Html::parse_fragment(html);
    frag.root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn estimate_reading_time(text: &str) -> u32 {
    use unicode_segmentation::UnicodeSegmentation;
    let word_count = text.unicode_words().count();
    let minutes = (word_count as f64 / 230.0).ceil() as u32;
    minutes.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_with_fallback_readability() {
        let html = r#"<html><body>
            <header><h1>Title</h1><nav>Menu items</nav></header>
            <div class="article-body">
                <p>First paragraph of the article with enough content to be considered substantial by the scoring algorithm.</p>
                <p>Second paragraph continues the article with more meaningful text content that adds to the overall score.</p>
                <p>Third paragraph wraps up the main points of this test article with additional text for scoring.</p>
            </div>
            <div class="sidebar">
                <a href="/x">Related link 1</a>
                <a href="/x">Related link 2</a>
            </div>
        </body></html>"#;

        let result = extract_with_fallback(html, None, None).unwrap();
        assert_eq!(result.method, ExtractMethod::Readability);
        assert!(result.inner.content.contains("First paragraph"));
        assert!(!result.inner.content.contains("Related link"));
    }

    #[test]
    fn extract_with_fallback_body_fallback() {
        // HTML with no clear article structure but body content
        let html = r#"<html><body>
            <div>Some text that is not in a typical article structure but has enough content to be useful for reading purposes.</div>
            <div>More text content here to make the overall body have substantial text for the fallback extraction to work.</div>
        </body></html>"#;

        let result = extract_with_fallback(html, None, None).unwrap();
        assert!(result.inner.text_content.len() > 50);
    }

    #[test]
    fn extract_with_fallback_empty_html() {
        let html = r#"<html><body></body></html>"#;
        let result = extract_with_fallback(html, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn extract_with_fallback_site_rule() {
        let html = r#"<html><body>
            <div class="main-content">
                <p>This is the main article content extracted via site-specific selector rule that has sufficient length for validation.</p>
            </div>
            <div class="noise">Sidebar noise content that should be excluded from the final result.</div>
        </body></html>"#;

        let rule = SiteRuleConfig {
            content_selector: Some(".main-content".to_string()),
            title_selector: None,
            strip_selectors: None,
        };

        let result = extract_with_fallback(html, None, Some(&rule)).unwrap();
        assert_eq!(result.method, ExtractMethod::SiteRule);
        assert!(result.inner.content.contains("main article content"));
        assert!(!result.inner.content.contains("Sidebar noise"));
    }

    #[test]
    fn backward_compat_extract_still_works() {
        let html = r#"<html><body>
            <article>
                <p>This is the main article content that should be extracted because it is inside an article tag and has sufficient text length.</p>
                <p>More content inside the article element to ensure proper scoring behavior in the extraction pipeline.</p>
            </article>
        </body></html>"#;

        let result = extract(html, None, None).unwrap();
        assert!(result.content.contains("main article content"));
    }
}
