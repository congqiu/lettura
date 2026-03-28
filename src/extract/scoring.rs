use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::collections::HashMap;

static POSITIVE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)article|body|content|entry|hentry|h-entry|main|page|pagination|post|text|blog|story|column").unwrap()
});

static NEGATIVE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)hidden|banner|combx|comment|com-|contact|footer|footnote|gdpr|masthead|media|meta|outbrain|promo|related|scroll|share|shoutbox|sidebar|skyscraper|sponsor|shopping|tags|tool|widget").unwrap()
});

/// Score all element nodes in the document for content likelihood
pub fn score_nodes(document: &Html) -> HashMap<ego_tree::NodeId, f64> {
    let mut scores: HashMap<ego_tree::NodeId, f64> = HashMap::new();
    let p_selector = Selector::parse("p, pre, td").unwrap();

    for element in document.select(&p_selector) {
        let text = element.text().collect::<String>();
        let text = text.trim();
        // Ignore short paragraphs
        if text.len() < 25 {
            continue;
        }

        // Get parent and grandparent nodes
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
                // Grandparent gets half the score
                *scores.entry(gp_id).or_insert(0.0) += content_score / 2.0;
            }
        }
    }

    // Apply link density penalty to all scored nodes
    let scored_ids: Vec<ego_tree::NodeId> = scores.keys().copied().collect();
    for node_id in scored_ids {
        if let Some(node_ref) = document.tree.get(node_id) {
            if let Some(element) = ElementRef::wrap(node_ref) {
                let link_density = compute_link_density(element);
                if let Some(score) = scores.get_mut(&node_id) {
                    *score *= 1.0 - link_density;
                }
            }
        }
    }

    scores
}

/// Compute initial score based on tag name and class/id attributes
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
    let id_attr = element.value().attr("id").unwrap_or("");
    let class_id = format!("{} {}", class, id_attr);

    let class_score = if POSITIVE_RE.is_match(&class_id) {
        25.0
    } else if NEGATIVE_RE.is_match(&class_id) {
        -25.0
    } else {
        0.0
    };

    tag_score + class_score
}

/// Compute content score for paragraph text
fn compute_content_score(text: &str) -> f64 {
    let mut score = 1.0;
    // Commas and periods add score
    score += text.matches(',').count() as f64;
    score += text.matches('\u{FF0C}').count() as f64; // ，
    score += text.matches('\u{3002}').count() as f64; // 。
    // Every 100 chars adds 1 point, max 3
    score += (text.len() as f64 / 100.0).min(3.0);
    score
}

/// Compute link text ratio within an element
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

    #[test]
    fn paragraph_heavy_div_scores_high() {
        let html = r#"<html><body>
            <div id="content">
                <p>This is a long paragraph with substantial text content that should score well in the readability algorithm because it contains meaningful words.</p>
                <p>Another paragraph with more text content here. The scoring algorithm should give this container a high score based on text density.</p>
            </div>
            <div id="sidebar"><a href="/x">Link1</a><a href="/x">Link2</a><a href="/x">Link3</a></div>
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
            <div id="nav"><p><a href="/x">Link</a> <a href="/x">Link</a> <a href="/x">Link</a> <a href="/x">Link</a> <a href="/x">Link</a> some padding text to reach threshold</p></div>
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
        scores: &HashMap<ego_tree::NodeId, f64>,
        id: &str,
    ) -> f64 {
        let sel = Selector::parse(&format!("#{}", id)).unwrap();
        doc.select(&sel)
            .next()
            .and_then(|el| scores.get(&el.id()).copied())
            .unwrap_or(0.0)
    }
}
