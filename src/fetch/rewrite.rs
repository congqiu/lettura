//! URL path rewriting driven by site config `rewrite` rules.
//!
//! A rewrite rule has the form `{ from: <regex>, to: <template> }`, where:
//! - `from` is a regex applied to the URL path (scheme/host/query are kept intact).
//! - `to` is a replacement template supporting `$1`..`$9` capture group references.
//!
//! Rules are tried in order; the first matching rule wins and the URL is
//! returned with its path substituted. Invalid regex patterns are logged and
//! skipped rather than propagated as errors.

use crate::site_config::Rewrite;
use regex::Regex;
use url::Url;

/// Apply the first matching rewrite rule to the given URL. Returns the original
/// URL unchanged if no rule matches or if the URL cannot be parsed.
pub fn apply(url: &str, rules: &[Rewrite]) -> String {
    if rules.is_empty() {
        return url.to_string();
    }

    let Ok(mut parsed) = Url::parse(url) else {
        return url.to_string();
    };

    let path = parsed.path().to_string();

    for rule in rules {
        let re = match Regex::new(&rule.from) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(pattern = %rule.from, error = %e, "invalid rewrite regex, skipping");
                continue;
            }
        };
        if re.is_match(&path) {
            let new_path = re.replace(&path, rule.to.as_str()).to_string();
            parsed.set_path(&new_path);
            return parsed.to_string();
        }
    }

    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(from: &str, to: &str) -> Rewrite {
        Rewrite {
            from: from.to_string(),
            to: to.to_string(),
        }
    }

    #[test]
    fn no_rules_returns_original() {
        let out = apply("https://example.com/p/123", &[]);
        assert_eq!(out, "https://example.com/p/123");
    }

    #[test]
    fn rewrites_path_with_capture_group() {
        let rules = vec![rule(r"^/p/(\d+)", "/api/articles/$1")];
        let out = apply("https://zhuanlan.zhihu.com/p/123456", &rules);
        assert_eq!(out, "https://zhuanlan.zhihu.com/api/articles/123456");
    }

    #[test]
    fn preserves_query_string() {
        let rules = vec![rule(r"^/p/(\d+)", "/api/articles/$1")];
        let out = apply("https://zhuanlan.zhihu.com/p/123?utm=x", &rules);
        assert_eq!(
            out,
            "https://zhuanlan.zhihu.com/api/articles/123?utm=x"
        );
    }

    #[test]
    fn first_matching_rule_wins() {
        let rules = vec![
            rule(r"^/old/", "/new/"),
            rule(r"^/old/.*", "/should-not-apply"),
        ];
        let out = apply("https://example.com/old/thing", &rules);
        assert_eq!(out, "https://example.com/new/thing");
    }

    #[test]
    fn no_match_returns_original() {
        let rules = vec![rule(r"^/post/", "/api/post/")];
        let out = apply("https://example.com/article/x", &rules);
        assert_eq!(out, "https://example.com/article/x");
    }

    #[test]
    fn invalid_regex_is_skipped() {
        let rules = vec![rule("[invalid", "/x"), rule(r"^/ok", "/done")];
        let out = apply("https://example.com/ok/path", &rules);
        assert_eq!(out, "https://example.com/done/path");
    }

    #[test]
    fn invalid_url_returns_original() {
        let rules = vec![rule(r"^/x", "/y")];
        let out = apply("not a url", &rules);
        assert_eq!(out, "not a url");
    }
}
