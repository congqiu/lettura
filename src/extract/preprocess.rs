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

/// Preprocess HTML: remove script, style, hidden elements, comments, etc.
pub fn preprocess(html: &str) -> String {
    // Remove comments first (avoid interfering with parsing)
    let html = COMMENT_RE.replace_all(html, "");

    let document = Html::parse_document(&html);
    let mut ids_to_remove = std::collections::HashSet::new();

    // Collect elements to remove
    for selector in REMOVE_TAGS.iter() {
        for element in document.select(selector) {
            ids_to_remove.insert(element.id());
        }
    }
    for element in document.select(&HIDDEN_SELECTOR) {
        ids_to_remove.insert(element.id());
    }

    // Remove elements with class/id that are clearly non-content
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

    // Rebuild HTML excluding marked nodes and their children
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
    exclude_ids: &std::collections::HashSet<ego_tree::NodeId>,
) -> String {
    use scraper::node::Node;
    let mut output = String::new();

    fn walk(
        node_ref: ego_tree::NodeRef<Node>,
        exclude_ids: &std::collections::HashSet<ego_tree::NodeId>,
        output: &mut String,
    ) {
        if exclude_ids.contains(&node_ref.id()) {
            return;
        }
        // Check if any ancestor is excluded
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
                output.push_str(&text);
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
