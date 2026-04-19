use super::SiteConfig;

/// Parse a site config file content into a SiteConfig.
/// `domain` is derived from the filename (e.g., "medium.com").
pub fn parse_config(domain: &str, content: &str) -> Result<SiteConfig, String> {
    let mut config = SiteConfig {
        domain: domain.to_string(),
        ..Default::default()
    };

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Split on first ": "
        let (key, value) = line
            .split_once(": ")
            .ok_or_else(|| format!("invalid config line (missing ': '): {}", line))?;

        match key {
            "render" => {
                config.render = value.trim().eq_ignore_ascii_case("true");
            }
            "header" => {
                let (name, val) = value
                    .split_once(": ")
                    .ok_or_else(|| format!("invalid header format: {}", value))?;
                config
                    .extra_headers
                    .push((name.trim().to_string(), val.trim().to_string()));
            }
            "user_agent" => {
                config.user_agent = Some(value.trim().to_string());
            }
            "timeout" => {
                config.timeout = Some(
                    value
                        .trim()
                        .parse::<u64>()
                        .map_err(|_| format!("invalid timeout value: {}", value))?,
                );
            }
            "title" => {
                config.title_selectors = split_selectors(value);
            }
            "body" => {
                config.body_selectors = split_selectors(value);
            }
            "strip" => {
                config.strip_selectors.extend(split_selectors(value));
            }
            "author" => {
                config.author_selector = Some(value.trim().to_string());
            }
            "date" => {
                config.date_selector = Some(value.trim().to_string());
            }
            "image" => {
                config.image_selector = Some(value.trim().to_string());
            }
            "match" => {
                config.match_patterns.push(value.trim().to_string());
            }
            "exclude" => {
                config.exclude_patterns.push(value.trim().to_string());
            }
            _ => {
                return Err(format!("unknown config key: {}", key));
            }
        }
    }

    Ok(config)
}

/// Split a comma-separated selector string into individual selectors.
fn split_selectors(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let config = parse_config(
            "example.com",
            r#"
title: h1.article-title
body: div.content
"#,
        )
        .unwrap();

        assert_eq!(config.domain, "example.com");
        assert!(!config.render);
        assert!(config.extra_headers.is_empty());
        assert_eq!(config.title_selectors, vec!["h1.article-title"]);
        assert_eq!(config.body_selectors, vec!["div.content"]);
    }

    #[test]
    fn parse_full_config() {
        let config = parse_config(
            "medium.com",
            r#"
# This is a comment
render: true
header: Cookie: session=abc
header: Referer: https://example.com
user_agent: Mozilla/5.0 Custom
timeout: 60

title: h1, .title
body: article, div.post-body, main
strip: div.ads
strip: nav.sidebar
author: span.author
date: time
image: img.hero
match: /article/
exclude: /video/
"#,
        )
        .unwrap();

        assert_eq!(config.domain, "medium.com");
        assert!(config.render);
        assert_eq!(
            config.extra_headers,
            vec![
                ("Cookie".to_string(), "session=abc".to_string()),
                ("Referer".to_string(), "https://example.com".to_string()),
            ]
        );
        assert_eq!(config.user_agent, Some("Mozilla/5.0 Custom".to_string()));
        assert_eq!(config.timeout, Some(60));
        assert_eq!(config.title_selectors, vec!["h1", ".title"]);
        assert_eq!(
            config.body_selectors,
            vec!["article", "div.post-body", "main"]
        );
        assert_eq!(config.strip_selectors, vec!["div.ads", "nav.sidebar"]);
        assert_eq!(config.author_selector, Some("span.author".to_string()));
        assert_eq!(config.date_selector, Some("time".to_string()));
        assert_eq!(config.image_selector, Some("img.hero".to_string()));
        assert_eq!(config.match_patterns, vec!["/article/"]);
        assert_eq!(config.exclude_patterns, vec!["/video/"]);
    }

    #[test]
    fn parse_empty_config() {
        let config = parse_config("empty.com", "").unwrap();
        assert_eq!(config.domain, "empty.com");
        assert!(config.title_selectors.is_empty());
        assert!(config.body_selectors.is_empty());
    }

    #[test]
    fn parse_comments_and_blank_lines() {
        let config = parse_config(
            "test.com",
            r#"
# Full line comment
title: h1

# Another comment
body: div.content
"#,
        )
        .unwrap();
        assert_eq!(config.title_selectors, vec!["h1"]);
        assert_eq!(config.body_selectors, vec!["div.content"]);
    }

    #[test]
    fn parse_invalid_line() {
        let result = parse_config("bad.com", "no colon here");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing ': '"));
    }

    #[test]
    fn parse_unknown_key() {
        let result = parse_config("bad.com", "unknown_key: value");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown config key"));
    }

    #[test]
    fn parse_invalid_timeout() {
        let result = parse_config("bad.com", "timeout: abc");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid timeout"));
    }

    #[test]
    fn parse_render_false() {
        let config = parse_config("test.com", "render: false").unwrap();
        assert!(!config.render);
    }

    #[test]
    fn parse_render_case_insensitive() {
        let config = parse_config("test.com", "render: True").unwrap();
        assert!(config.render);
    }
}
