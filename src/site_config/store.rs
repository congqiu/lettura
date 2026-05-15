use super::SiteConfig;
use super::parser;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

/// Global config store, lazily initialized. Holds configs scanned from the
/// local override directory (set via `init_store`). No built-in configs are
/// shipped with the binary — users provide YAML files themselves.
static STORE: Lazy<RwLock<SiteConfigStore>> = Lazy::new(|| RwLock::new(SiteConfigStore::new()));

pub struct SiteConfigStore {
    local: HashMap<String, SiteConfig>,
    local_path: Option<String>,
}

impl SiteConfigStore {
    fn new() -> Self {
        Self {
            local: HashMap::new(),
            local_path: None,
        }
    }

    fn load_local(&mut self) {
        self.local.clear();
        let Some(local_path) = self.local_path.as_deref() else {
            return;
        };

        let dir = Path::new(local_path);
        let read = match std::fs::read_dir(dir) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(path = %local_path, error = %e, "failed to read site-configs directory");
                return;
            }
        };

        for entry in read.flatten() {
            let path = entry.path();
            if !path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e == "yaml" || e == "yml")
                .unwrap_or(false)
            {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(file = %path.display(), error = %e, "failed to read site config file");
                    continue;
                }
            };
            match parser::parse_config(stem, &content) {
                Ok(config) => {
                    tracing::debug!(domain = %stem, "loaded site config");
                    self.local.insert(stem.to_string(), config);
                }
                Err(e) => {
                    tracing::warn!(file = %path.display(), error = %e, "failed to parse site config");
                }
            }
        }

        tracing::info!(count = self.local.len(), "loaded local site configs");
    }

    /// Look up a site config for the given domain and URL.
    pub fn find(&self, domain: &str, url: &str) -> Option<SiteConfig> {
        if let Some(config) = self.local.get(domain)
            && config.matches_url(url)
        {
            return Some(config.clone());
        }
        None
    }
}

/// Initialize the global store with a local directory path.
/// If `local_path` is `Some`, all YAML files under it are scanned and parsed
/// into the store; parse errors are logged but non-fatal.
pub fn init_store(local_path: Option<String>) {
    let mut store = STORE
        .write()
        .expect("site config store lock is not poisoned");
    store.local_path = local_path;
    store.load_local();
}

/// Look up a site config from the global store.
pub fn find_config(domain: &str, url: &str) -> Option<SiteConfig> {
    let store = STORE
        .read()
        .expect("site config store lock is not poisoned");
    store.find(domain, url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    fn write_yaml(dir: &std::path::Path, name: &str, content: &str) {
        let mut file = std::fs::File::create(dir.join(name)).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn loads_local_yaml_files() {
        let dir = tempdir().unwrap();
        write_yaml(
            dir.path(),
            "example.com.yaml",
            r#"
response:
  type: html
  html:
    title: h1
    body: [article]
"#,
        );

        let mut store = SiteConfigStore::new();
        store.local_path = Some(dir.path().to_string_lossy().to_string());
        store.load_local();

        let cfg = store.find("example.com", "https://example.com/x").unwrap();
        assert_eq!(cfg.domain, "example.com");
        let html = cfg.response.html.unwrap();
        assert_eq!(html.title.as_deref(), Some("h1"));
    }

    #[test]
    fn skips_non_yaml_files() {
        let dir = tempdir().unwrap();
        write_yaml(dir.path(), "notes.txt", "some note");
        write_yaml(dir.path(), "real.com.yaml", "response:\n  type: html\n");

        let mut store = SiteConfigStore::new();
        store.local_path = Some(dir.path().to_string_lossy().to_string());
        store.load_local();
        assert!(store.local.contains_key("real.com"));
        assert!(!store.local.contains_key("notes"));
    }

    #[test]
    fn find_returns_none_when_url_doesnt_match() {
        let dir = tempdir().unwrap();
        write_yaml(
            dir.path(),
            "site.com.yaml",
            r#"
match:
  - "^/article/"
"#,
        );

        let mut store = SiteConfigStore::new();
        store.local_path = Some(dir.path().to_string_lossy().to_string());
        store.load_local();

        assert!(
            store
                .find("site.com", "https://site.com/article/1")
                .is_some()
        );
        assert!(store.find("site.com", "https://site.com/video/1").is_none());
    }

    #[test]
    fn broken_yaml_is_logged_and_skipped() {
        let dir = tempdir().unwrap();
        write_yaml(dir.path(), "broken.com.yaml", "not: valid: yaml:");
        write_yaml(dir.path(), "good.com.yaml", "response:\n  type: html\n");

        let mut store = SiteConfigStore::new();
        store.local_path = Some(dir.path().to_string_lossy().to_string());
        store.load_local();
        assert!(!store.local.contains_key("broken.com"));
        assert!(store.local.contains_key("good.com"));
    }
}
