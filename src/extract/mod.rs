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

/// Extract article content from raw HTML - main entry point
pub fn extract(html: &str, url: Option<&str>, site_rule: Option<&SiteRuleConfig>) -> Result<ExtractResult, ExtractError> {
    let preprocessed = preprocess::preprocess(html);
    let mut document = scraper::Html::parse_document(&preprocessed);

    if let Some(rule) = site_rule {
        if let Some(ref strip_selectors) = rule.strip_selectors {
            for sel_str in strip_selectors {
                if let Ok(sel) = scraper::Selector::parse(sel_str) {
                    let ids: Vec<_> = document.select(&sel)
                        .map(|el| el.id())
                        .collect();
                    for id in ids {
                        if let Some(mut node) = document.tree.get_mut(id) {
                            node.detach();
                        }
                    }
                }
            }
        }
    }

    let meta = metadata::extract_metadata(&document, url);

    let title = if let Some(rule) = site_rule {
        if let Some(ref title_selector) = rule.title_selector {
            extract_title_with_selector(&document, title_selector).or(meta.title)
        } else {
            meta.title
        }
    } else {
        meta.title
    };

    let article_html = if let Some(rule) = site_rule {
        if let Some(ref content_selector) = rule.content_selector {
            readability::extract_content_with_selector(&document, content_selector)?
        } else {
            readability::extract_content(&document)?
        }
    } else {
        readability::extract_content(&document)?
    };

    let clean_html = sanitize::sanitize(&article_html);
    let text_content = html_to_text(&clean_html);
    let reading_time = estimate_reading_time(&text_content);

    Ok(ExtractResult {
        title,
        content: clean_html,
        text_content,
        author: meta.author,
        language: meta.language,
        preview_image: meta.preview_image,
        excerpt: meta.excerpt,
        reading_time,
    })
}

fn extract_title_with_selector(document: &scraper::Html, selector: &str) -> Option<String> {
    let sel = scraper::Selector::parse(selector).ok()?;
    let el = document.select(&sel).next()?;
    let text = el.text().collect::<String>().trim().to_string();
    if text.is_empty() { None } else { Some(text) }
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
    // English ~230 wpm, Chinese ~400 cpm
    let word_count = text.unicode_words().count();
    let minutes = (word_count as f64 / 230.0).ceil() as u32;
    minutes.max(1)
}
