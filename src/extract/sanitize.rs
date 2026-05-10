use ammonia::Builder;
use std::collections::HashSet;

/// Sanitize HTML with a whitelist strategy, keeping only safe reading-related tags and attributes
pub fn sanitize(html: &str) -> String {
    let mut tags = HashSet::new();
    for tag in &[
        "p",
        "br",
        "hr",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "strong",
        "em",
        "b",
        "i",
        "u",
        "s",
        "mark",
        "small",
        "sub",
        "sup",
        "blockquote",
        "pre",
        "code",
        "kbd",
        "samp",
        "ul",
        "ol",
        "li",
        "dl",
        "dt",
        "dd",
        "a",
        "img",
        "figure",
        "figcaption",
        "table",
        "thead",
        "tbody",
        "tfoot",
        "tr",
        "th",
        "td",
        "caption",
        "div",
        "span",
        "section",
        "article",
        "details",
        "summary",
        "time",
        "abbr",
    ] {
        tags.insert(*tag);
    }

    let mut tag_attrs = std::collections::HashMap::new();
    tag_attrs.insert(
        "a",
        vec!["href", "title"].into_iter().collect::<HashSet<&str>>(),
    );
    tag_attrs.insert(
        "img",
        vec!["src", "alt", "title", "width", "height"]
            .into_iter()
            .collect(),
    );
    tag_attrs.insert("td", vec!["colspan", "rowspan"].into_iter().collect());
    tag_attrs.insert("th", vec!["colspan", "rowspan"].into_iter().collect());
    tag_attrs.insert("time", vec!["datetime"].into_iter().collect());
    tag_attrs.insert("abbr", vec!["title"].into_iter().collect());

    Builder::new()
        .tags(tags)
        .tag_attributes(tag_attrs.into_iter().collect())
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
