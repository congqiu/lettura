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
        author: extract_meta_content(
            document,
            &[("name", "author"), ("property", "article:author")],
        ),
        language: extract_language(document),
        preview_image: extract_meta_content(
            document,
            &[("property", "og:image"), ("name", "twitter:image")],
        ),
        excerpt: extract_meta_content(
            document,
            &[
                ("property", "og:description"),
                ("name", "description"),
                ("name", "twitter:description"),
            ],
        ),
    }
}

fn extract_title(document: &Html) -> Option<String> {
    // Prefer og:title
    if let Some(og) = extract_meta_content(document, &[("property", "og:title")]) {
        return Some(og);
    }

    // Fallback: <title> tag, clean site name suffix
    let title_sel = Selector::parse("title").ok()?;
    let title_el = document.select(&title_sel).next()?;
    let raw = title_el.text().collect::<String>().trim().to_string();

    if raw.is_empty() {
        return None;
    }

    Some(clean_title(&raw))
}

fn clean_title(title: &str) -> String {
    // Split by common separators, take the longest part
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
    // From <html lang="...">
    let html_sel = Selector::parse("html").ok()?;
    let html_el = document.select(&html_sel).next()?;
    html_el
        .value()
        .attr("lang")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            // Fallback: content-language meta
            extract_meta_content(document, &[("http-equiv", "content-language")])
        })
}

fn extract_meta_content(document: &Html, attrs: &[(&str, &str)]) -> Option<String> {
    let meta_sel = Selector::parse("meta").ok()?;
    for meta in document.select(&meta_sel) {
        let el = meta.value();
        for (attr_name, attr_value) in attrs {
            if el
                .attr(attr_name)
                .map(|v| v.eq_ignore_ascii_case(attr_value))
                == Some(true)
                && let Some(content) = el.attr("content")
            {
                let trimmed = content.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let html =
            r#"<html><head><meta name="author" content="John Doe"></head><body></body></html>"#;
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
        assert_eq!(
            meta.preview_image.as_deref(),
            Some("https://example.com/img.jpg")
        );
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
        let html =
            r#"<html><head><title>Article Title | My Site</title></head><body></body></html>"#;
        let doc = Html::parse_document(html);
        let meta = extract_metadata(&doc, None);
        assert_eq!(meta.title.as_deref(), Some("Article Title"));
    }
}
