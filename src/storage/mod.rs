pub mod local;
pub mod oss;

use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("storage io error: {0}")]
    Io(String),
    #[error("storage upload error: {0}")]
    Upload(String),
}

#[async_trait]
pub trait ImageStorage: Send + Sync {
    /// Store image data, returns the public URL to access it
    async fn store(&self, key: &str, data: &[u8], content_type: &str) -> Result<String, StorageError>;
    /// Delete a stored image
    async fn delete(&self, key: &str) -> Result<(), StorageError>;
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;
    /// Delete all objects under a prefix (e.g. "pages/abc/")
    async fn delete_prefix(&self, prefix: &str) -> Result<(), StorageError> {
        let _ = prefix;
        Ok(())
    }
}

/// Create storage backend based on config
pub fn create_storage(config: &crate::config::Config) -> Box<dyn ImageStorage> {
    match config.storage_type.as_str() {
        "oss" | "s3" => Box::new(oss::OssStorage::new(config)),
        _ => Box::new(local::LocalStorage::new(&config.storage_local_path)),
    }
}

const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024;

pub async fn download_image(url: &str) -> Result<(Vec<u8>, String), StorageError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Lettura/0.1")
        .build()
        .map_err(|e| StorageError::Io(e.to_string()))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| StorageError::Io(e.to_string()))?;

    if let Some(len) = resp.content_length() {
        if len as usize > MAX_IMAGE_SIZE {
            return Err(StorageError::Io(format!("image too large: {len} bytes")));
        }
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let data = resp
        .bytes()
        .await
        .map_err(|e| StorageError::Io(e.to_string()))?;

    if data.len() > MAX_IMAGE_SIZE {
        return Err(StorageError::Io(format!("image too large: {} bytes", data.len())));
    }

    Ok((data.to_vec(), content_type))
}

/// Generate storage key from URL and actual content type.
/// Prefers actual content_type over URL extension when available.
pub fn image_key_from_url(url: &str, content_type: Option<&str>) -> String {
    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(url.as_bytes()));
    // Use content-type derived extension if available, otherwise fall back to URL extension
    let ext = content_type
        .and_then(|ct| mime_to_ext(ct))
        .or_else(|| url_extension(url))
        .unwrap_or("jpg");
    format!("images/{}.{}", &hash[..16], ext)
}

/// Convert MIME type to file extension
fn mime_to_ext(content_type: &str) -> Option<&'static str> {
    match content_type.split(';').next()?.trim() {
        "image/svg+xml" => Some("svg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/avif" => Some("avif"),
        "image/x-icon" | "image/vnd.microsoft.icon" => Some("ico"),
        _ => None,
    }
}

/// Extract extension from URL
fn url_extension(url: &str) -> Option<&'static str> {
    let ext = url.rsplit(|c| c == '?' || c == '#').next()?;
    let ext = ext.trim_end_matches(|c| c == '/' || c == '.');
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("jpg"),
        "png" => Some("png"),
        "gif" => Some("gif"),
        "webp" => Some("webp"),
        "svg" => Some("svg"),
        "ico" => Some("ico"),
        "avif" => Some("avif"),
        _ => None,
    }
}

/// Process HTML content: download all images and rewrite URLs
pub async fn process_images(
    html: &str,
    storage: &dyn ImageStorage,
) -> String {
    // Collect image URLs in a sync block to avoid holding non-Send scraper types across await
    let img_urls: Vec<String> = {
        let doc = scraper::Html::parse_fragment(html);
        let img_sel = scraper::Selector::parse("img[src]").unwrap();
        doc.select(&img_sel)
            .filter_map(|el| el.value().attr("src").map(String::from))
            .filter(|src| src.starts_with("http"))
            .collect()
    };

    if img_urls.is_empty() {
        return html.to_string();
    }

    let mut result = html.to_string();

    for url in &img_urls {
        match download_image(url).await {
            Ok((data, content_type)) => {
                let key = image_key_from_url(url, Some(&content_type));
                match storage.store(&key, &data, &content_type).await {
                    Ok(new_url) => {
                        result = result.replace(url, &new_url);
                    }
                    Err(e) => {
                        tracing::warn!("failed to store image {}: {}", url, e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("failed to download image {}: {}", url, e);
            }
        }
    }

    result
}
