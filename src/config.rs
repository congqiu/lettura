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
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
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
        }
    }
}
