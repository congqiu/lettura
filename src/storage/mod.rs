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
}

/// Create storage backend based on config
pub fn create_storage(config: &crate::config::Config) -> Box<dyn ImageStorage> {
    match config.storage_type.as_str() {
        "oss" | "s3" => Box::new(oss::OssStorage::new(config)),
        _ => Box::new(local::LocalStorage::new(&config.storage_local_path)),
    }
}

/// Download an image from URL, return (data, content_type)
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

    Ok((data.to_vec(), content_type))
}

/// Generate storage key from URL (hash-based)
pub fn image_key_from_url(url: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(url.as_bytes()));
    let ext = url
        .rsplit('.')
        .next()
        .and_then(|e| {
            let e = e.split('?').next().unwrap_or(e);
            if ["jpg", "jpeg", "png", "gif", "webp", "svg", "ico"].contains(&e) {
                Some(e)
            } else {
                None
            }
        })
        .unwrap_or("jpg");
    format!("images/{}.{}", &hash[..16], ext)
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
                let key = image_key_from_url(url);
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
