use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub listen_addr: String,
    pub index_path: String,
    // Storage
    pub storage_type: String,       // "local" or "oss"
    pub storage_local_path: String, // local storage directory
    pub pages_storage_path: String,
    // OSS (S3-compatible)
    pub oss_endpoint: String,
    pub oss_region: String,
    pub oss_bucket: String,
    pub oss_access_key: String,
    pub oss_secret_key: String,
    pub oss_public_url: String, // custom public URL prefix (optional)
    // DB connection pool
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub db_acquire_timeout_secs: u64,
    // CORS
    pub cors_origins: String,
    // Production mode
    pub production: bool,

    /// Trust X-Forwarded-For / X-Real-IP headers for rate limiting.
    /// Only enable when running behind a trusted reverse proxy.
    pub trust_proxy: bool,

    /// Disable new user registration
    pub disable_registration: bool,
    // Metrics
    pub metrics_enabled: bool,
    pub metrics_bearer_token: Option<String>,
    // Fetch
    pub user_agent: String,
    pub fetch_timeout_secs: u64,
    pub fetch_max_retries: u32,
    pub proxy: Option<String>,
    pub site_configs_path: Option<String>,
    // Render fallback (honored when the `rendering` feature is compiled in)
    pub rendering_enabled: String, // "auto" | "true" | "false"
    pub chromium_path: Option<String>,
    pub render_concurrency: usize,
    pub render_timeout_ms: u64,
    // Public base URL for skill endpoint and other public resources
    pub public_base_url: Option<String>,
    // Operational tuning
    pub import_max_body_bytes: usize,
    pub pages_max_upload_bytes: usize,
    pub max_image_size: usize,
    pub auth_rate_limit: u32,
    pub global_rate_limit: u32,
    pub search_commit_interval_secs: u64,
    pub token_cleanup_interval_secs: u64,
    pub metrics_interval_secs: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let jwt_secret =
            env::var("JWT_SECRET").map_err(|_| "JWT_SECRET must be set".to_string())?;

        if jwt_secret == "change-me-in-production" {
            return Err(
                "JWT_SECRET must not be the default value 'change-me-in-production'".to_string(),
            );
        }

        if jwt_secret.len() < 32 {
            return Err(format!(
                "JWT_SECRET must be at least 32 characters (got {})",
                jwt_secret.len()
            ));
        }

        let cors_origins = env::var("CORS_ORIGINS").unwrap_or_else(|_| "*".to_string());

        let production = env::var("LETTURA_PRODUCTION")
            .ok()
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        // Reject wildcard CORS in production for security
        if production && cors_origins == "*" {
            return Err("CORS_ORIGINS must not be '*' in production mode. Set CORS_ORIGINS to specific allowed origins.".to_string());
        }

        // Reject obvious weak defaults
        if jwt_secret == "change-me-to-a-random-secret-at-least-32-characters-long" {
            return Err("JWT_SECRET must be changed from the default value".to_string());
        }

        Ok(Self {
            database_url: env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set".to_string())?,
            jwt_secret,
            listen_addr: env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:3330".to_string()),
            index_path: env::var("INDEX_PATH").unwrap_or_else(|_| "/data/tantivy".to_string()),
            storage_type: env::var("STORAGE_TYPE").unwrap_or_else(|_| "local".to_string()),
            storage_local_path: env::var("STORAGE_LOCAL_PATH").unwrap_or_else(|_| "/data/storage".to_string()),
            pages_storage_path: env::var("PAGES_STORAGE_PATH").unwrap_or_else(|_| "/data/pages".to_string()),
            oss_endpoint: env::var("OSS_ENDPOINT").unwrap_or_default(),
            oss_region: env::var("OSS_REGION").unwrap_or_else(|_| "auto".to_string()),
            oss_bucket: env::var("OSS_BUCKET").unwrap_or_default(),
            oss_access_key: env::var("OSS_ACCESS_KEY").unwrap_or_default(),
            oss_secret_key: env::var("OSS_SECRET_KEY").unwrap_or_default(),
            oss_public_url: env::var("OSS_PUBLIC_URL").unwrap_or_default(),
            db_max_connections: env::var("DB_MAX_CONNECTIONS").ok().and_then(|v| v.parse().ok()).unwrap_or(10),
            db_min_connections: env::var("DB_MIN_CONNECTIONS").ok().and_then(|v| v.parse().ok()).unwrap_or(2),
            db_acquire_timeout_secs: env::var("DB_ACQUIRE_TIMEOUT").ok().and_then(|v| v.parse().ok()).unwrap_or(30),
            cors_origins,
            production,
            trust_proxy: env::var("LETTURA_TRUST_PROXY").ok().map(|v| v == "true" || v == "1").unwrap_or(false),
            disable_registration: env::var("LETTURA_DISABLE_REGISTRATION").ok().map(|v| v == "true" || v == "1").unwrap_or(true),
            metrics_enabled: env::var("METRICS_ENABLED").ok().map(|v| v == "true" || v == "1").unwrap_or(false),
            metrics_bearer_token: env::var("LETTURA_METRICS_BEARER_TOKEN").ok(),
            user_agent: env::var("LETTURA_USER_AGENT").unwrap_or_else(|_| {
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36".to_string()
            }),
            fetch_timeout_secs: env::var("LETTURA_FETCH_TIMEOUT").ok().and_then(|v| v.parse().ok()).unwrap_or(30),
            fetch_max_retries: env::var("LETTURA_FETCH_MAX_RETRIES").ok().and_then(|v| v.parse().ok()).unwrap_or(3),
            proxy: env::var("LETTURA_PROXY").ok(),
            site_configs_path: env::var("LETTURA_SITE_CONFIGS_PATH").ok(),
            rendering_enabled: env::var("LETTURA_RENDERING_ENABLED").unwrap_or_else(|_| "auto".to_string()),
            chromium_path: env::var("LETTURA_CHROMIUM_PATH").ok(),
            render_concurrency: env::var("LETTURA_RENDER_CONCURRENCY").ok().and_then(|v| v.parse().ok()).unwrap_or(2),
            render_timeout_ms: env::var("LETTURA_RENDER_TIMEOUT_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(15000),
            public_base_url: env::var("LETTURA_PUBLIC_BASE_URL").ok(),
            import_max_body_bytes: env::var("LETTURA_IMPORT_MAX_BODY_MB").ok().and_then(|v| v.parse().ok()).map(|mb: usize| mb * 1024 * 1024).unwrap_or(50 * 1024 * 1024),
            pages_max_upload_bytes: env::var("LETTURA_PAGES_MAX_UPLOAD_MB").ok().and_then(|v| v.parse().ok()).map(|mb: usize| mb * 1024 * 1024).unwrap_or(10 * 1024 * 1024),
            max_image_size: env::var("LETTURA_MAX_IMAGE_MB").ok().and_then(|v| v.parse().ok()).map(|mb: usize| mb * 1024 * 1024).unwrap_or(10 * 1024 * 1024),
            auth_rate_limit: env::var("LETTURA_AUTH_RATE_LIMIT").ok().and_then(|v| v.parse().ok()).unwrap_or(10),
            global_rate_limit: env::var("LETTURA_GLOBAL_RATE_LIMIT").ok().and_then(|v| v.parse().ok()).unwrap_or(100),
            search_commit_interval_secs: env::var("LETTURA_SEARCH_COMMIT_INTERVAL").ok().and_then(|v| v.parse().ok()).unwrap_or(3),
            token_cleanup_interval_secs: env::var("LETTURA_TOKEN_CLEANUP_INTERVAL").ok().and_then(|v| v.parse().ok()).unwrap_or(3600),
            metrics_interval_secs: env::var("LETTURA_METRICS_INTERVAL").ok().and_then(|v| v.parse().ok()).unwrap_or(15),
        })
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

    // Helper to set all required env vars with a given JWT_SECRET value.
    // SAFETY: tests run with --test-threads=1 so no concurrent env mutation.
    fn set_env(jwt_secret: &str) {
        unsafe {
            env::set_var("DATABASE_URL", "postgres://localhost/test");
            env::set_var("JWT_SECRET", jwt_secret);
        }
    }

    // Helper to clean up env vars after each test.
    // SAFETY: tests run with --test-threads=1 so no concurrent env mutation.
    fn cleanup_env() {
        unsafe {
            env::remove_var("DATABASE_URL");
            env::remove_var("JWT_SECRET");
            env::remove_var("LISTEN_ADDR");
            env::remove_var("INDEX_PATH");
            env::remove_var("STORAGE_TYPE");
            env::remove_var("STORAGE_LOCAL_PATH");
            env::remove_var("OSS_ENDPOINT");
            env::remove_var("OSS_REGION");
            env::remove_var("OSS_BUCKET");
            env::remove_var("OSS_ACCESS_KEY");
            env::remove_var("OSS_SECRET_KEY");
            env::remove_var("OSS_PUBLIC_URL");
            env::remove_var("DB_MAX_CONNECTIONS");
            env::remove_var("DB_MIN_CONNECTIONS");
            env::remove_var("DB_ACQUIRE_TIMEOUT");
            env::remove_var("CORS_ORIGINS");
            env::remove_var("LETTURA_PRODUCTION");
            env::remove_var("METRICS_ENABLED");
            env::remove_var("PAGES_STORAGE_PATH");
            env::remove_var("LETTURA_USER_AGENT");
            env::remove_var("LETTURA_FETCH_TIMEOUT");
            env::remove_var("LETTURA_FETCH_MAX_RETRIES");
            env::remove_var("LETTURA_PROXY");
            env::remove_var("LETTURA_SITE_CONFIGS_PATH");
            env::remove_var("LETTURA_RENDERING_ENABLED");
            env::remove_var("LETTURA_CHROMIUM_PATH");
            env::remove_var("LETTURA_RENDER_CONCURRENCY");
            env::remove_var("LETTURA_RENDER_TIMEOUT_MS");
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
