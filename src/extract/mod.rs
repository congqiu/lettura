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

/// Extract article content from raw HTML - main entry point
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
    // English ~230 wpm, Chinese ~400 cpm
    let word_count = text.unicode_words().count();
    let minutes = (word_count as f64 / 230.0).ceil() as u32;
    minutes.max(1)
}
