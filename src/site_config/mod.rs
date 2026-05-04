pub mod parser;
pub mod store;

use serde::Deserialize;

/// Rewrite rule applied to a URL path before fetching.
#[derive(Debug, Clone, Deserialize)]
pub struct Rewrite {
    pub from: String,
    pub to: String,
}

/// Per-request HTTP overrides.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RequestConfig {
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub cookies: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub user_agent: Option<String>,
}

/// Rules for extracting fields out of a JSON response body via JSON Pointer.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct JsonExtract {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    /// If true, the `content` field is treated as HTML and cleaned; otherwise plain text.
    #[serde(default)]
    pub content_is_html: bool,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
}

/// Rules for extracting fields out of an HTML response body via CSS selectors.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct HtmlExtract {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Vec<String>,
    #[serde(default)]
    pub strip: Vec<String>,
    #[serde(default)]
    pub author: Option<String>,
}

/// Content type of the expected response.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ResponseType {
    #[default]
    Html,
    Json,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ResponseConfig {
    #[serde(default, rename = "type")]
    pub response_type: ResponseType,
    #[serde(default)]
    pub json: Option<JsonExtract>,
    #[serde(default)]
    pub html: Option<HtmlExtract>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RenderMode {
    Never,
    #[default]
    Auto,
    Force,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RenderConfig {
    #[serde(default)]
    pub mode: RenderMode,
    #[serde(default)]
    pub wait_for: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// Parsed site configuration for a domain, loaded from a YAML file.
///
/// `domain` is populated from the filename stem by the parser, not from YAML
/// content — the field is marked `#[serde(default)]` so it is optional in
/// user-written YAML.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SiteConfig {
    #[serde(default)]
    pub domain: String,
    #[serde(default, rename = "match")]
    pub url_match: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,

    #[serde(default)]
    pub rewrite: Vec<Rewrite>,

    #[serde(default)]
    pub request: RequestConfig,

    #[serde(default)]
    pub response: ResponseConfig,

    #[serde(default)]
    pub render: RenderConfig,
}

impl SiteConfig {
    /// Check if a URL path matches this config's `match`/`exclude` regex patterns.
    pub fn matches_url(&self, url: &str) -> bool {
        let path = extract_path(url);

        // Size limit for regex compilation to prevent ReDoS (1 MB)
        const REGEX_SIZE_LIMIT: usize = 1_000_000;

        if !self.url_match.is_empty() {
            let matched = self
                .url_match
                .iter()
                .filter_map(|p| regex::RegexBuilder::new(p).size_limit(REGEX_SIZE_LIMIT).build().ok())
                .any(|re| re.is_match(path));
            if !matched {
                return false;
            }
        }

        if !self.exclude.is_empty() {
            let excluded = self
                .exclude
                .iter()
                .filter_map(|p| regex::RegexBuilder::new(p).size_limit(REGEX_SIZE_LIMIT).build().ok())
                .any(|re| re.is_match(path));
            if excluded {
                return false;
            }
        }

        true
    }
}

fn extract_path(url: &str) -> &str {
    let without_scheme = url.split("://").last().unwrap_or(url);
    let after_domain = without_scheme.find('/').unwrap_or(without_scheme.len());
    let path_and_rest = &without_scheme[after_domain..];
    path_and_rest
        .split('?')
        .next()
        .unwrap_or(path_and_rest)
        .split('#')
        .next()
        .unwrap_or(path_and_rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_url_no_patterns() {
        let config = SiteConfig::default();
        assert!(config.matches_url("https://example.com/any/path"));
    }

    #[test]
    fn matches_url_with_match_pattern() {
        let mut config = SiteConfig::default();
        config.url_match = vec!["^/article/".to_string(), "^/post/".to_string()];
        assert!(config.matches_url("https://example.com/article/123"));
        assert!(!config.matches_url("https://example.com/video/456"));
    }

    #[test]
    fn matches_url_with_exclude_pattern() {
        let mut config = SiteConfig::default();
        config.exclude = vec!["^/video/".to_string()];
        assert!(!config.matches_url("https://example.com/video/123"));
        assert!(config.matches_url("https://example.com/article/456"));
    }

    #[test]
    fn matches_url_match_and_exclude() {
        let mut config = SiteConfig::default();
        config.url_match = vec!["^/".to_string()];
        config.exclude = vec!["^/video/".to_string(), "^/gallery/".to_string()];
        assert!(config.matches_url("https://example.com/article/123"));
        assert!(!config.matches_url("https://example.com/video/123"));
        assert!(!config.matches_url("https://example.com/gallery/123"));
    }

    #[test]
    fn invalid_regex_is_ignored() {
        let mut config = SiteConfig::default();
        config.url_match = vec!["[invalid".to_string(), "^/ok".to_string()];
        assert!(config.matches_url("https://example.com/ok"));
        assert!(!config.matches_url("https://example.com/bad"));
    }

    #[test]
    fn extract_path_handles_various_urls() {
        assert_eq!(extract_path("https://example.com/path"), "/path");
        assert_eq!(extract_path("https://example.com/path?q=1"), "/path");
        assert_eq!(extract_path("https://example.com/path#section"), "/path");
        assert_eq!(extract_path("https://example.com/"), "/");
        assert_eq!(extract_path("https://example.com"), "");
    }
}
