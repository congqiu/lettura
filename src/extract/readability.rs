use super::scoring::score_nodes;
use super::ExtractError;
use scraper::{ElementRef, Html};

/// Extract the highest-scoring article content HTML from preprocessed DOM
pub fn extract_content(document: &Html) -> Result<String, ExtractError> {
    let scores = score_nodes(document);

    if scores.is_empty() {
        return Err(ExtractError::NoContent);
    }

    // Pick the top-scoring node
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

    // Collect valuable child content from the top-scoring node
    let content = collect_content(top_element, &scores);

    if content.trim().is_empty() {
        return Err(ExtractError::NoContent);
    }

    Ok(content)
}

/// Collect valuable content from a node, filtering out low-score children
fn collect_content(
    element: ElementRef,
    scores: &std::collections::HashMap<ego_tree::NodeId, f64>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    let top_score = scores.get(&element.id()).copied().unwrap_or(0.0);

    for child in element.children() {
        if let Some(child_el) = ElementRef::wrap(child) {
            let tag = child_el.value().name();

            // Keep paragraphs, headings, quotes, lists, pre, figure, etc.
            if matches!(
                tag,
                "p" | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "h5"
                    | "h6"
                    | "blockquote"
                    | "pre"
                    | "ul"
                    | "ol"
                    | "figure"
                    | "img"
                    | "table"
            ) {
                parts.push(child_el.html());
                continue;
            }

            // For container elements, check if they have enough text content
            if matches!(tag, "div" | "section" | "span" | "article") {
                let child_score = scores.get(&child_el.id()).copied().unwrap_or(0.0);
                let text: String = child_el.text().collect();
                let text_len = text.trim().len();

                // Keep if child score is high enough or has substantial text
                if child_score >= top_score * 0.2 || text_len > 80 {
                    parts.push(child_el.html());
                }
                continue;
            }

            // Keep inline elements directly
            if matches!(
                tag,
                "a" | "strong" | "em" | "b" | "i" | "br" | "code" | "mark"
            ) {
                parts.push(child_el.html());
            }
        } else if let Some(text) = child.value().as_text() {
            let t = text.trim();
            if !t.is_empty() {
                parts.push(t.to_string());
            }
        }
    }

    // If no meaningful parts collected, fall back to entire node's innerHTML
    if parts.is_empty() || parts.iter().all(|p| p.trim().is_empty()) {
        return element.inner_html();
    }

    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

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
                <a href="/x">Related link 1</a>
                <a href="/x">Related link 2</a>
            </div>
        </body></html>"#;

        let preprocessed = crate::extract::preprocess::preprocess(html);
        let doc = Html::parse_document(&preprocessed);
        let result = extract_content(&doc).unwrap();

        assert!(
            result.contains("First paragraph"),
            "should contain article text"
        );
        assert!(
            result.contains("Second paragraph"),
            "should contain article text"
        );
        assert!(
            !result.contains("Related link"),
            "should not contain sidebar"
        );
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
