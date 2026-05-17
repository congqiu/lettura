use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub database_url: String,
    #[serde(default)]
    pub jwt_secret: String,
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,
    #[serde(default = "default_index_path")]
    pub index_path: String,
    // Storage
    #[serde(default = "default_storage_type")]
    pub storage_type: String,
    #[serde(default = "default_storage_local_path")]
    pub storage_local_path: String,
    #[serde(default = "default_pages_storage_path")]
    pub pages_storage_path: String,
    // OSS (S3-compatible)
    #[serde(default)]
    pub oss_endpoint: String,
    #[serde(default = "default_oss_region")]
    pub oss_region: String,
    #[serde(default)]
    pub oss_bucket: String,
    #[serde(default)]
    pub oss_access_key: String,
    #[serde(default)]
    pub oss_secret_key: String,
    #[serde(default)]
    pub oss_public_url: String,
    // DB connection pool
    #[serde(default = "default_db_max_connections")]
    pub db_max_connections: u32,
    #[serde(default = "default_db_min_connections")]
    pub db_min_connections: u32,
    #[serde(default = "default_db_acquire_timeout_secs")]
    pub db_acquire_timeout_secs: u64,
    // CORS
    #[serde(default = "default_cors_origins")]
    pub cors_origins: String,
    // Production mode
    #[serde(default)]
    pub production: bool,
    #[serde(default)]
    pub trust_proxy: bool,
    #[serde(default = "default_true")]
    pub disable_registration: bool,
    // Metrics
    #[serde(default)]
    pub metrics_enabled: bool,
    #[serde(default)]
    pub metrics_bearer_token: Option<String>,
    // Fetch
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    #[serde(default = "default_fetch_timeout_secs")]
    pub fetch_timeout_secs: u64,
    #[serde(default = "default_fetch_max_retries")]
    pub fetch_max_retries: u32,
    #[serde(default)]
    pub proxy: Option<String>,
    #[serde(default)]
    pub site_configs_path: Option<String>,
    // Fetch queue worker tuning
    #[serde(default = "default_fetch_concurrency")]
    pub fetch_concurrency: usize,
    #[serde(default = "default_fetch_max_attempts")]
    pub fetch_max_attempts: i16,
    #[serde(default = "default_fetch_lease_secs")]
    pub fetch_lease_secs: u64,
    #[serde(default = "default_fetch_dead_ttl_days")]
    pub fetch_dead_ttl_days: i64,
    // Render fallback
    #[serde(default = "default_rendering_enabled")]
    pub rendering_enabled: String,
    #[serde(default)]
    pub chromium_path: Option<String>,
    #[serde(default = "default_render_concurrency")]
    pub render_concurrency: usize,
    #[serde(default = "default_render_timeout_ms")]
    pub render_timeout_ms: u64,
    // Public base URL
    #[serde(default)]
    pub public_base_url: Option<String>,
    // Operational tuning
    #[serde(default = "default_import_max_body_bytes")]
    pub import_max_body_bytes: usize,
    #[serde(default = "default_pages_max_upload_bytes")]
    pub pages_max_upload_bytes: usize,
    #[serde(default = "default_max_image_size")]
    pub max_image_size: usize,
    #[serde(default = "default_auth_rate_limit")]
    pub auth_rate_limit: u32,
    #[serde(default = "default_global_rate_limit")]
    pub global_rate_limit: u32,
    #[serde(default = "default_search_commit_interval_secs")]
    pub search_commit_interval_secs: u64,
    #[serde(default = "default_token_cleanup_interval_secs")]
    pub token_cleanup_interval_secs: u64,
    #[serde(default = "default_metrics_interval_secs")]
    pub metrics_interval_secs: u64,
}

// Default value functions
fn default_listen_addr() -> String { "0.0.0.0:3330".to_string() }
fn default_index_path() -> String { "/data/tantivy".to_string() }
fn default_storage_type() -> String { "local".to_string() }
fn default_storage_local_path() -> String { "/data/storage".to_string() }
fn default_pages_storage_path() -> String { "/data/pages".to_string() }
fn default_oss_region() -> String { "auto".to_string() }
fn default_db_max_connections() -> u32 { 10 }
fn default_db_min_connections() -> u32 { 2 }
fn default_db_acquire_timeout_secs() -> u64 { 30 }
fn default_cors_origins() -> String { "*".to_string() }
fn default_true() -> bool { true }
fn default_user_agent() -> String {
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36".to_string()
}
fn default_fetch_timeout_secs() -> u64 { 30 }
fn default_fetch_max_retries() -> u32 { 3 }
fn default_fetch_concurrency() -> usize { 5 }
fn default_fetch_max_attempts() -> i16 { 5 }
fn default_fetch_lease_secs() -> u64 { 300 }
fn default_fetch_dead_ttl_days() -> i64 { 30 }
fn default_rendering_enabled() -> String { "auto".to_string() }
fn default_render_concurrency() -> usize { 2 }
fn default_render_timeout_ms() -> u64 { 15000 }
fn default_import_max_body_bytes() -> usize { 50 * 1024 * 1024 }
fn default_pages_max_upload_bytes() -> usize { 10 * 1024 * 1024 }
fn default_max_image_size() -> usize { 10 * 1024 * 1024 }
fn default_auth_rate_limit() -> u32 { 10 }
fn default_global_rate_limit() -> u32 { 100 }
fn default_search_commit_interval_secs() -> u64 { 3 }
fn default_token_cleanup_interval_secs() -> u64 { 3600 }
fn default_metrics_interval_secs() -> u64 { 15 }

impl Config {
    pub fn from_env() -> Result<Self, String> {
        // envy::prefixed strips "LETTURA_" from env var names before matching
        // struct fields, so LETTURA_FETCH_TIMEOUT → fetch_timeout.
        let mut config: Config = envy::prefixed("LETTURA_")
            .from_env()
            .map_err(|e| format!("config error: {e}"))?;

        // Backward compat: accept DATABASE_URL / JWT_SECRET without LETTURA_ prefix
        if config.database_url.is_empty() {
            config.database_url = std::env::var("DATABASE_URL")
                .map_err(|_| "DATABASE_URL (or LETTURA_DATABASE_URL) must be set".to_string())?;
        }
        if config.jwt_secret.is_empty() {
            config.jwt_secret = std::env::var("JWT_SECRET")
                .map_err(|_| "JWT_SECRET (or LETTURA_JWT_SECRET) must be set".to_string())?;
        }

        // Backward compat: CORS_ORIGINS without LETTURA_ prefix overrides the
        // default. envy::prefixed("LETTURA_") doesn't see CORS_ORIGINS at all,
        // so without this override the default "*" would always win — and
        // production-mode validation below would then reject every prod boot.
        // Empty string is treated as explicit "no origins" (old behavior).
        if let Ok(v) = std::env::var("CORS_ORIGINS") {
            config.cors_origins = v;
        }

        // Validation: JWT_SECRET
        if config.jwt_secret == "change-me-in-production" {
            return Err("JWT_SECRET must not be the default value 'change-me-in-production'".to_string());
        }
        if config.jwt_secret.len() < 32 {
            return Err(format!("JWT_SECRET must be at least 32 characters (got {})", config.jwt_secret.len()));
        }
        if config.jwt_secret == "change-me-to-a-random-secret-at-least-32-characters-long" {
            return Err("JWT_SECRET must be changed from the default value".to_string());
        }

        // Validation: CORS in production
        if config.production && config.cors_origins == "*" {
            return Err("CORS_ORIGINS must not be '*' in production mode. Set CORS_ORIGINS to specific allowed origins.".to_string());
        }

        Ok(config)
    }

    /// Returns true when rendering should be attempted at runtime. Always false
    /// when the `rendering` feature is compiled out.
    pub fn rendering_runtime_enabled(&self) -> bool {
        #[cfg(not(feature = "rendering"))]
        {
            return false;
        }
        #[cfg(feature = "rendering")]
        {
            !matches!(self.rendering_enabled.as_str(), "false" | "0" | "off")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn set_env(jwt_secret: &str) {
        unsafe {
            env::set_var("DATABASE_URL", "postgres://localhost/test");
            env::set_var("JWT_SECRET", jwt_secret);
        }
    }

    fn cleanup_env() {
        // Only clean up vars that tests actually set
        let vars = [
            "DATABASE_URL", "JWT_SECRET",
            "LETTURA_RENDER_CONCURRENCY", "LETTURA_RENDERING_ENABLED",
            "LETTURA_PRODUCTION", "CORS_ORIGINS",
        ];
        unsafe {
            for v in &vars { env::remove_var(v); }
        }
    }

    #[test]
    fn rejects_short_jwt_secret() {
        set_env("too-short");
        let result = Config::from_env();
        cleanup_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 32 characters"));
    }

    #[test]
    fn rejects_default_jwt_secret() {
        set_env("change-me-in-production");
        let result = Config::from_env();
        cleanup_env();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("must not be the default value")
        );
    }

    #[test]
    fn accepts_valid_jwt_secret() {
        set_env("a-very-secure-secret-that-is-at-least-32-chars!");
        let result = Config::from_env();
        cleanup_env();
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().jwt_secret,
            "a-very-secure-secret-that-is-at-least-32-chars!"
        );
    }

    #[test]
    fn render_concurrency_parses() {
        set_env("a-very-secure-secret-that-is-at-least-32-chars!");
        unsafe {
            env::set_var("LETTURA_RENDER_CONCURRENCY", "4");
        }
        let cfg = Config::from_env().unwrap();
        assert_eq!(cfg.render_concurrency, 4);
        cleanup_env();
    }

    #[test]
    fn rendering_enabled_disabled_via_env() {
        set_env("a-very-secure-secret-that-is-at-least-32-chars!");
        unsafe {
            env::set_var("LETTURA_RENDERING_ENABLED", "false");
        }
        let cfg = Config::from_env().unwrap();
        assert!(!cfg.rendering_runtime_enabled());
        cleanup_env();
    }

    #[test]
    fn rejects_wildcard_cors_in_production_mode() {
        set_env("a-very-secure-secret-that-is-at-least-32-chars!");
        unsafe {
            env::set_var("LETTURA_PRODUCTION", "true");
        }
        unsafe {
            env::set_var("CORS_ORIGINS", "*");
        }
        let result = Config::from_env();
        unsafe {
            env::remove_var("LETTURA_PRODUCTION");
        }
        cleanup_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("CORS_ORIGINS"));
    }
}