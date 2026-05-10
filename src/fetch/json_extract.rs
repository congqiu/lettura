//! Extract article fields from a JSON response body using JSON Pointer paths.
//!
//! Used when a site config declares `response.type: json` — typically when a
//! SPA page has an underlying API endpoint that returns article data as JSON.
//! Each field in `JsonExtract` (title, content, author, ...) is an RFC 6901
//! pointer that is evaluated against the parsed JSON root.

use crate::extract::{self, ExtractResult};
use crate::site_config::JsonExtract;

/// Extract an `ExtractResult` from a JSON response body.
///
/// Returns `Err` if the body is not valid JSON. Individual missing fields are
/// treated as `None` (non-fatal). The `content` field is sanitized as HTML if
/// `content_is_html` is true; otherwise it is treated as plain text and wrapped
/// in a single `<p>` element so downstream rendering stays consistent.
pub fn extract(body: &str, rules: &JsonExtract) -> Result<ExtractResult, String> {
    let root: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("invalid JSON: {}", e))?;

    let title = rules
        .title
        .as_deref()
        .and_then(|p| pointer_as_string(&root, p));
    let author = rules
        .author
        .as_deref()
        .and_then(|p| pointer_as_string(&root, p));
    let language = rules
        .language
        .as_deref()
        .and_then(|p| pointer_as_string(&root, p));

    let raw_content = rules
        .content
        .as_deref()
        .and_then(|p| pointer_as_string(&root, p))
        .unwrap_or_default();

    let (content_html, text_content) = if rules.content_is_html {
        let cleaned = extract::sanitize::sanitize(&raw_content);
        let text = extract::html_to_text(&cleaned);
        (cleaned, text)
    } else {
        let text = raw_content.trim().to_string();
        let html = if text.is_empty() {
            String::new()
        } else {
            format!("<p>{}</p>", html_escape(&text))
        };
        (html, text)
    };

    let reading_time = extract::estimate_reading_time(&text_content);

    Ok(ExtractResult {
        title,
        content: content_html,
        text_content,
        author,
        language,
        preview_image: None,
        excerpt: None,
        reading_time,
    })
}

/// Read a JSON value at `pointer` and coerce it to a string if possible.
/// Supports strings, integers, and booleans; returns `None` for other types.
fn pointer_as_string(root: &serde_json::Value, pointer: &str) -> Option<String> {
    let value = root.pointer(pointer)?;
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(title: &str, content: &str, is_html: bool) -> JsonExtract {
        JsonExtract {
            title: Some(title.to_string()),
            content: Some(content.to_string()),
            content_is_html: is_html,
            author: None,
            language: None,
            published_at: None,
        }
    }

    #[test]
    fn extracts_simple_string_fields() {
        let body = r#"{"data": {"title": "Hello", "content": "World"}}"#;
        let r = extract(body, &rules("/data/title", "/data/content", false)).unwrap();
        assert_eq!(r.title.as_deref(), Some("Hello"));
        assert_eq!(r.text_content, "World");
        assert_eq!(r.content, "<p>World</p>");
    }

    #[test]
    fn html_content_is_sanitized() {
        let body = r#"{"body": "<p>safe</p><script>alert(1)</script>"}"#;
        let r_rules = JsonExtract {
            content: Some("/body".to_string()),
            content_is_html: true,
            ..Default::default()
        };
        let r = extract(body, &r_rules).unwrap();
        assert!(r.content.contains("<p>safe</p>"));
        assert!(!r.content.contains("script"));
    }

    #[test]
    fn plain_text_is_wrapped_and_escaped() {
        let body = r#"{"content": "a < b & c"}"#;
        let r = extract(body, &rules("/missing", "/content", false)).unwrap();
        assert_eq!(r.content, "<p>a &lt; b &amp; c</p>");
        assert_eq!(r.text_content, "a < b & c");
    }

    #[test]
    fn missing_fields_become_none() {
        let body = r#"{"foo": "bar"}"#;
        let r = extract(body, &rules("/nope", "/also_nope", false)).unwrap();
        assert!(r.title.is_none());
        assert_eq!(r.text_content, "");
        assert_eq!(r.content, "");
    }

    #[test]
    fn author_and_language_pointers() {
        let body = r#"{"a": {"name": "Alice"}, "lang": "zh-CN"}"#;
        let rules = JsonExtract {
            author: Some("/a/name".to_string()),
            language: Some("/lang".to_string()),
            ..Default::default()
        };
        let r = extract(body, &rules).unwrap();
        assert_eq!(r.author.as_deref(), Some("Alice"));
        assert_eq!(r.language.as_deref(), Some("zh-CN"));
    }

    #[test]
    fn numeric_pointer_value_is_coerced() {
        let body = r#"{"id": 42}"#;
        let rules = JsonExtract {
            title: Some("/id".to_string()),
            ..Default::default()
        };
        let r = extract(body, &rules).unwrap();
        assert_eq!(r.title.as_deref(), Some("42"));
    }

    #[test]
    fn invalid_json_errors() {
        let r = extract("not-json", &rules("/x", "/y", false));
        assert!(r.is_err());
    }

    #[test]
    fn reading_time_is_computed() {
        let long = "word ".repeat(500);
        let body = format!(r#"{{"c": "{}"}}"#, long);
        let r_rules = JsonExtract {
            content: Some("/c".to_string()),
            content_is_html: false,
            ..Default::default()
        };
        let r = extract(&body, &r_rules).unwrap();
        assert!(r.reading_time >= 1);
    }
}
