use super::SiteConfig;
use regex::Regex;
use std::env;

/// Parse a YAML site config file content into a `SiteConfig`.
///
/// `domain` is derived from the filename (e.g., "medium.com") and overrides
/// whatever is written inside the YAML file. Placeholders of the form
/// `${ENV_XXX}` in string values are replaced with the value of the matching
/// process environment variable; unset vars are left as-is.
pub fn parse_config(domain: &str, content: &str) -> Result<SiteConfig, String> {
    let substituted = substitute_env(content);

    let mut config: SiteConfig =
        serde_yaml::from_str(&substituted).map_err(|e| format!("invalid YAML: {}", e))?;
    config.domain = domain.to_string();
    Ok(config)
}

/// Replace `${ENV_NAME}` placeholders with the value of the matching process
/// environment variable. Placeholders whose variable is unset are left as-is
/// (to avoid silent config corruption).
fn substitute_env(input: &str) -> String {
    // The regex captures the variable name inside ${...}. Names are limited
    // to [A-Z0-9_] to avoid matching unrelated YAML syntax.
    static RE_STR: &str = r"\$\{([A-Z][A-Z0-9_]*)\}";
    let re = Regex::new(RE_STR).expect("static regex compiles");

    re.replace_all(input, |caps: &regex::Captures| {
        let var_name = &caps[1];
        env::var(var_name).unwrap_or_else(|_| caps[0].to_string())
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::super::{RenderMode, ResponseType};
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let yaml = r#"
response:
  type: html
  html:
    title: h1.article-title
    body: [div.content]
"#;
        let config = parse_config("example.com", yaml).unwrap();
        assert_eq!(config.domain, "example.com");
        assert_eq!(config.response.response_type, ResponseType::Html);
        let html = config.response.html.unwrap();
        assert_eq!(html.title.as_deref(), Some("h1.article-title"));
        assert_eq!(html.body, vec!["div.content"]);
        assert_eq!(config.render.mode, RenderMode::Auto);
    }

    #[test]
    fn parse_full_json_config() {
        let yaml = r#"
match:
  - "^/p/"
exclude:
  - "^/video/"
rewrite:
  - from: "^/p/(\\d+)"
    to: "/api/articles/$1"
request:
  headers:
    Referer: "https://example.com/"
  cookies:
    session: "abc123"
  user_agent: "CustomBot/1.0"
response:
  type: json
  json:
    title: "/data/title"
    content: "/data/content"
    content_is_html: true
    author: "/data/author/name"
render:
  mode: auto
  wait_for: "article"
  timeout_ms: 20000
"#;
        let config = parse_config("zhuanlan.zhihu.com", yaml).unwrap();
        assert_eq!(config.url_match, vec!["^/p/"]);
        assert_eq!(config.exclude, vec!["^/video/"]);
        assert_eq!(config.rewrite.len(), 1);
        assert_eq!(config.rewrite[0].from, "^/p/(\\d+)");
        assert_eq!(config.rewrite[0].to, "/api/articles/$1");
        assert_eq!(
            config.request.headers.get("Referer"),
            Some(&"https://example.com/".to_string())
        );
        assert_eq!(
            config.request.cookies.get("session"),
            Some(&"abc123".to_string())
        );
        assert_eq!(config.request.user_agent.as_deref(), Some("CustomBot/1.0"));
        assert_eq!(config.response.response_type, ResponseType::Json);
        let json = config.response.json.unwrap();
        assert_eq!(json.title.as_deref(), Some("/data/title"));
        assert!(json.content_is_html);
        assert_eq!(config.render.mode, RenderMode::Auto);
        assert_eq!(config.render.wait_for.as_deref(), Some("article"));
        assert_eq!(config.render.timeout_ms, Some(20000));
    }

    #[test]
    fn parse_render_force() {
        let yaml = r#"
render:
  mode: force
"#;
        let config = parse_config("medium.com", yaml).unwrap();
        assert_eq!(config.render.mode, RenderMode::Force);
    }

    #[test]
    fn parse_empty_yaml_is_ok() {
        let config = parse_config("empty.com", "").unwrap();
        assert_eq!(config.domain, "empty.com");
        assert_eq!(config.render.mode, RenderMode::Auto);
    }

    #[test]
    fn parse_invalid_yaml_errors() {
        let result = parse_config("bad.com", "this: is: invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid YAML"));
    }

    #[test]
    fn env_placeholder_substituted_when_set() {
        // SAFETY: tests run with --test-threads=1 via .cargo/config.toml
        unsafe {
            env::set_var("TEST_COOKIE_TOKEN", "secret-value");
        }
        let yaml = r#"
request:
  cookies:
    auth: "${TEST_COOKIE_TOKEN}"
"#;
        let config = parse_config("any.com", yaml).unwrap();
        assert_eq!(
            config.request.cookies.get("auth"),
            Some(&"secret-value".to_string())
        );
        unsafe {
            env::remove_var("TEST_COOKIE_TOKEN");
        }
    }

    #[test]
    fn env_placeholder_kept_verbatim_when_unset() {
        unsafe {
            env::remove_var("DEFINITELY_UNSET_VAR_XYZ");
        }
        let yaml = r#"
request:
  cookies:
    auth: "${DEFINITELY_UNSET_VAR_XYZ}"
"#;
        let config = parse_config("any.com", yaml).unwrap();
        assert_eq!(
            config.request.cookies.get("auth"),
            Some(&"${DEFINITELY_UNSET_VAR_XYZ}".to_string())
        );
    }
}
