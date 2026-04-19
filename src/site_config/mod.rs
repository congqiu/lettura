pub mod parser;
pub mod store;

/// Parsed site configuration for a domain.
#[derive(Debug, Clone, Default)]
pub struct SiteConfig {
    pub domain: String,

    // Fetch control
    pub render: bool,
    pub extra_headers: Vec<(String, String)>,
    pub user_agent: Option<String>,
    pub timeout: Option<u64>,

    // Content extraction
    pub title_selectors: Vec<String>,
    pub body_selectors: Vec<String>,
    pub strip_selectors: Vec<String>,
    pub author_selector: Option<String>,
    pub date_selector: Option<String>,
    pub image_selector: Option<String>,

    // URL matching
    pub match_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

impl SiteConfig {
    /// Check if a URL path matches this config's match/exclude patterns.
    pub fn matches_url(&self, url: &str) -> bool {
        let path = extract_path(url);

        if !self.match_patterns.is_empty() {
            let matched = self
                .match_patterns
                .iter()
                .any(|p| path.starts_with(p.as_str()));
            if !matched {
                return false;
            }
        }

        if !self.exclude_patterns.is_empty() {
            let excluded = self
                .exclude_patterns
                .iter()
                .any(|p| path.starts_with(p.as_str()));
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
        config.match_patterns = vec!["/article/".to_string(), "/post/".to_string()];
        assert!(config.matches_url("https://example.com/article/123"));
        assert!(!config.matches_url("https://example.com/video/456"));
    }

    #[test]
    fn matches_url_with_exclude_pattern() {
        let mut config = SiteConfig::default();
        config.exclude_patterns = vec!["/video/".to_string()];
        assert!(!config.matches_url("https://example.com/video/123"));
        assert!(config.matches_url("https://example.com/article/456"));
    }

    #[test]
    fn matches_url_match_and_exclude() {
        let mut config = SiteConfig::default();
        config.match_patterns = vec!["/".to_string()];
        config.exclude_patterns = vec!["/video/".to_string(), "/gallery/".to_string()];
        assert!(config.matches_url("https://example.com/article/123"));
        assert!(!config.matches_url("https://example.com/video/123"));
        assert!(!config.matches_url("https://example.com/gallery/123"));
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
