use super::parser;
use super::SiteConfig;
use once_cell::sync::Lazy;
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

/// Built-in site configs embedded at compile time.
#[derive(RustEmbed)]
#[folder = "site-configs/"]
struct BuiltInConfigs;

/// Global config store, lazily initialized.
static STORE: Lazy<RwLock<SiteConfigStore>> = Lazy::new(|| {
    let mut store = SiteConfigStore::new();
    store.load_builtin();
    RwLock::new(store)
});

pub struct SiteConfigStore {
    builtin: HashMap<String, SiteConfig>,
    local_path: Option<String>,
}

impl SiteConfigStore {
    fn new() -> Self {
        Self {
            builtin: HashMap::new(),
            local_path: None,
        }
    }

    fn load_builtin(&mut self) {
        for filename in BuiltInConfigs::iter() {
            let filename_str = filename.as_ref();
            if !filename_str.ends_with(".txt") {
                continue;
            }

            let domain = filename_str.trim_end_matches(".txt");
            if let Some(content) = BuiltInConfigs::get(&filename) {
                let content_str = std::str::from_utf8(&content.data).unwrap_or_default();
                match parser::parse_config(domain, content_str) {
                    Ok(config) => {
                        tracing::debug!(domain, "loaded built-in site config");
                        self.builtin.insert(domain.to_string(), config);
                    }
                    Err(e) => {
                        tracing::warn!(domain, error = %e, "failed to parse built-in site config");
                    }
                }
            }
        }
        tracing::info!(count = self.builtin.len(), "loaded built-in site configs");
    }

    /// Look up a site config for the given domain and URL.
    /// Priority: local override file -> built-in config.
    pub fn find(&self, domain: &str, url: &str) -> Option<SiteConfig> {
        // Try local override first
        if let Some(ref local_path) = self.local_path {
            let file_path = Path::new(local_path).join(format!("{}.txt", domain));
            if file_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    match parser::parse_config(domain, &content) {
                        Ok(config) if config.matches_url(url) => {
                            tracing::debug!(domain, "using local site config override");
                            return Some(config);
                        }
                        Ok(_) => {
                            tracing::debug!(domain, "local config found but URL doesn't match");
                        }
                        Err(e) => {
                            tracing::warn!(domain, error = %e, "failed to parse local site config");
                        }
                    }
                }
            }
        }

        // Try built-in config
        if let Some(config) = self.builtin.get(domain) {
            if config.matches_url(url) {
                tracing::debug!(domain, "using built-in site config");
                return Some(config.clone());
            }
        }

        None
    }
}

/// Initialize the global store with local override path.
pub fn init_store(local_path: Option<String>) {
    let mut store = STORE.write().unwrap();
    store.local_path = local_path;
}

/// Look up a site config from the global store.
pub fn find_config(domain: &str, url: &str) -> Option<SiteConfig> {
    let store = STORE.read().unwrap();
    store.find(domain, url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_configs_load_successfully() {
        reload_store();
        let store = STORE.read().unwrap();
        assert!(store.builtin.contains_key("medium.com"));
        assert!(store.builtin.contains_key("sspai.com"));
        assert!(store.builtin.contains_key("github.com"));
    }

    #[test]
    fn find_config_returns_builtin() {
        reload_store();
        let config = find_config("medium.com", "https://medium.com/some-article");
        assert!(config.is_some());
        let config = config.unwrap();
        assert!(config.render);
        assert_eq!(config.domain, "medium.com");
    }

    #[test]
    fn find_config_returns_none_for_unknown() {
        reload_store();
        let config = find_config("unknown-site-xyz.com", "https://unknown-site-xyz.com/article");
        assert!(config.is_none());
    }

    #[test]
    fn find_config_respects_match_patterns() {
        reload_store();
        // github.com config has match: /readme
        assert!(find_config("github.com", "https://github.com/user/repo/readme").is_some());
        assert!(find_config("github.com", "https://github.com/user/repo").is_none());
    }

    pub fn reload_store() {
        let mut store = STORE.write().unwrap();
        let local_path = store.local_path.clone();
        *store = SiteConfigStore::new();
        store.local_path = local_path;
        store.load_builtin();
    }
}
