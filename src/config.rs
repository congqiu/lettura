use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub listen_addr: String,
    pub index_path: String,
    // Storage
    pub storage_type: String,           // "local" or "oss"
    pub storage_local_path: String,     // local storage directory
    // OSS (S3-compatible)
    pub oss_endpoint: String,
    pub oss_region: String,
    pub oss_bucket: String,
    pub oss_access_key: String,
    pub oss_secret_key: String,
    pub oss_public_url: String,         // custom public URL prefix (optional)
    // DB connection pool
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub db_acquire_timeout_secs: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let jwt_secret = env::var("JWT_SECRET")
            .map_err(|_| "JWT_SECRET must be set".to_string())?;

        if jwt_secret == "change-me-in-production" {
            return Err("JWT_SECRET must not be the default value 'change-me-in-production'".to_string());
        }

        if jwt_secret.len() < 32 {
            return Err(format!(
                "JWT_SECRET must be at least 32 characters (got {})",
                jwt_secret.len()
            ));
        }

        Ok(Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            jwt_secret,
            listen_addr: env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            index_path: env::var("INDEX_PATH").unwrap_or_else(|_| "/data/tantivy".to_string()),
            storage_type: env::var("STORAGE_TYPE").unwrap_or_else(|_| "local".to_string()),
            storage_local_path: env::var("STORAGE_LOCAL_PATH").unwrap_or_else(|_| "/data/storage".to_string()),
            oss_endpoint: env::var("OSS_ENDPOINT").unwrap_or_default(),
            oss_region: env::var("OSS_REGION").unwrap_or_else(|_| "auto".to_string()),
            oss_bucket: env::var("OSS_BUCKET").unwrap_or_default(),
            oss_access_key: env::var("OSS_ACCESS_KEY").unwrap_or_default(),
            oss_secret_key: env::var("OSS_SECRET_KEY").unwrap_or_default(),
            oss_public_url: env::var("OSS_PUBLIC_URL").unwrap_or_default(),
            db_max_connections: env::var("DB_MAX_CONNECTIONS").ok().and_then(|v| v.parse().ok()).unwrap_or(10),
            db_min_connections: env::var("DB_MIN_CONNECTIONS").ok().and_then(|v| v.parse().ok()).unwrap_or(2),
            db_acquire_timeout_secs: env::var("DB_ACQUIRE_TIMEOUT").ok().and_then(|v| v.parse().ok()).unwrap_or(30),
        })
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
        assert!(result.unwrap_err().contains("must not be the default value"));
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
}
