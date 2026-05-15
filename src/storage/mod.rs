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
    async fn store(
        &self,
        key: &str,
        data: &[u8],
        content_type: &str,
    ) -> Result<String, StorageError>;
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

pub async fn download_image(url: &str, max_size: usize) -> Result<(Vec<u8>, String), StorageError> {
    // SSRF protection: block requests to private/reserved IPs.
    if let Err(e) = crate::fetch::ssrf::validate_url(url) {
        return Err(StorageError::Io(format!("SSRF blocked: {e}")));
    }

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

    if let Some(len) = resp.content_length()
        && len as usize > max_size
    {
        return Err(StorageError::Io(format!("image too large: {len} bytes")));
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

    if data.len() > max_size {
        return Err(StorageError::Io(format!(
            "image too large: {} bytes",
            data.len()
        )));
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
        .and_then(mime_to_ext)
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
    // Strip query string and fragment first
    let path = url.split(['?', '#']).next()?;
    // Get the last path segment (filename)
    let filename = path.rsplit('/').next()?;
    let filename = filename.trim_end_matches('.');
    // Extract extension after the last dot
    let ext = filename.rsplit('.').next()?;
    // If there's no dot or the "extension" is the whole filename, no extension
    if ext.eq_ignore_ascii_case(filename) {
        return None;
    }
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
    storage: std::sync::Arc<dyn ImageStorage>,
    max_image_size: usize,
) -> String {
    // Collect image URLs in a sync block to avoid holding non-Send scraper types across await
    let img_urls: Vec<String> = {
        let doc = scraper::Html::parse_fragment(html);
        let img_sel = scraper::Selector::parse("img[src]").expect("valid CSS selector");
        doc.select(&img_sel)
            .filter_map(|el| el.value().attr("src").map(String::from))
            .filter(|src| src.starts_with("http"))
            .collect()
    };

    if img_urls.is_empty() {
        return html.to_string();
    }

    // Download images in parallel under a concurrency cap, then rewrite URLs
    // sequentially. Concurrency is bounded so a single entry with many images
    // can't open hundreds of connections at once.
    const MAX_PARALLEL_DOWNLOADS: usize = 8;
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_PARALLEL_DOWNLOADS));
    let mut handles = Vec::with_capacity(img_urls.len());

    for url in img_urls {
        let s = storage.clone();
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.ok()?;
            match download_image(&url, max_image_size).await {
                Ok((data, content_type)) => {
                    let key = image_key_from_url(&url, Some(&content_type));
                    match s.store(&key, &data, &content_type).await {
                        Ok(new_url) => Some((url, new_url)),
                        Err(e) => {
                            tracing::warn!("failed to store image {}: {}", url, e);
                            None
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("failed to download image {}: {}", url, e);
                    None
                }
            }
        }));
    }

    let mut replacements: Vec<(String, String)> = Vec::new();
    for h in handles {
        if let Ok(Some(pair)) = h.await {
            replacements.push(pair);
        }
    }

    // Apply longest-URL-first so a shorter URL that is a substring of a
    // longer one cannot accidentally match inside the already-rewritten text.
    replacements.sort_by_key(|b| std::cmp::Reverse(b.0.len()));

    let mut result = html.to_string();
    for (old_url, new_url) in replacements {
        result = result.replace(&old_url, &new_url);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- mime_to_ext ---

    #[test]
    fn mime_to_ext_svg() {
        assert_eq!(mime_to_ext("image/svg+xml"), Some("svg"));
    }

    #[test]
    fn mime_to_ext_png() {
        assert_eq!(mime_to_ext("image/png"), Some("png"));
    }

    #[test]
    fn mime_to_ext_gif() {
        assert_eq!(mime_to_ext("image/gif"), Some("gif"));
    }

    #[test]
    fn mime_to_ext_webp() {
        assert_eq!(mime_to_ext("image/webp"), Some("webp"));
    }

    #[test]
    fn mime_to_ext_jpeg() {
        assert_eq!(mime_to_ext("image/jpeg"), Some("jpg"));
    }

    #[test]
    fn mime_to_ext_jpg() {
        assert_eq!(mime_to_ext("image/jpg"), Some("jpg"));
    }

    #[test]
    fn mime_to_ext_avif() {
        assert_eq!(mime_to_ext("image/avif"), Some("avif"));
    }

    #[test]
    fn mime_to_ext_x_icon() {
        assert_eq!(mime_to_ext("image/x-icon"), Some("ico"));
    }

    #[test]
    fn mime_to_ext_vnd_microsoft_icon() {
        assert_eq!(mime_to_ext("image/vnd.microsoft.icon"), Some("ico"));
    }

    #[test]
    fn mime_to_ext_charset_suffix() {
        assert_eq!(mime_to_ext("image/jpeg; charset=binary"), Some("jpg"));
    }

    #[test]
    fn mime_to_ext_text_html_returns_none() {
        assert_eq!(mime_to_ext("text/html"), None);
    }

    #[test]
    fn mime_to_ext_empty_returns_none() {
        assert_eq!(mime_to_ext(""), None);
    }

    #[test]
    fn mime_to_ext_unknown_returns_none() {
        assert_eq!(mime_to_ext("unknown/type"), None);
    }

    // --- url_extension ---

    #[test]
    fn url_extension_png() {
        assert_eq!(url_extension("https://example.com/img.png"), Some("png"));
    }

    #[test]
    fn url_extension_jpg() {
        assert_eq!(url_extension("https://example.com/img.jpg"), Some("jpg"));
    }

    #[test]
    fn url_extension_jpeg() {
        assert_eq!(url_extension("https://example.com/img.jpeg"), Some("jpg"));
    }

    #[test]
    fn url_extension_gif() {
        assert_eq!(url_extension("https://example.com/img.gif"), Some("gif"));
    }

    #[test]
    fn url_extension_webp() {
        assert_eq!(url_extension("https://example.com/img.webp"), Some("webp"));
    }

    #[test]
    fn url_extension_svg() {
        assert_eq!(url_extension("https://example.com/img.svg"), Some("svg"));
    }

    #[test]
    fn url_extension_ico() {
        assert_eq!(url_extension("https://example.com/img.ico"), Some("ico"));
    }

    #[test]
    fn url_extension_avif() {
        assert_eq!(url_extension("https://example.com/img.avif"), Some("avif"));
    }

    #[test]
    fn url_extension_case_insensitive() {
        assert_eq!(url_extension("https://example.com/img.PNG"), Some("png"));
    }

    #[test]
    fn url_extension_strips_query() {
        assert_eq!(
            url_extension("https://example.com/img.png?w=200"),
            Some("png")
        );
    }

    #[test]
    fn url_extension_strips_fragment() {
        assert_eq!(
            url_extension("https://example.com/img.png#anchor"),
            Some("png")
        );
    }

    #[test]
    fn url_extension_no_ext_returns_none() {
        assert_eq!(url_extension("https://example.com/img"), None);
    }

    #[test]
    fn url_extension_unsupported_ext_returns_none() {
        assert_eq!(url_extension("https://example.com/img.txt"), None);
    }

    // --- image_key_from_url ---

    #[test]
    fn image_key_deterministic() {
        let key1 = image_key_from_url("https://example.com/img.png", None);
        let key2 = image_key_from_url("https://example.com/img.png", None);
        assert_eq!(key1, key2);
    }

    #[test]
    fn image_key_different_urls() {
        let key1 = image_key_from_url("https://example.com/a.png", None);
        let key2 = image_key_from_url("https://example.com/b.png", None);
        assert_ne!(key1, key2);
    }

    #[test]
    fn image_key_content_type_overrides_url_ext() {
        let key = image_key_from_url("https://x.com/img.png", Some("image/gif"));
        assert!(
            key.ends_with(".gif"),
            "expected key ending with .gif, got: {key}"
        );
    }

    #[test]
    fn image_key_no_content_type_uses_url_ext() {
        let key = image_key_from_url("https://x.com/img.png", None);
        assert!(
            key.ends_with(".png"),
            "expected key ending with .png, got: {key}"
        );
    }

    #[test]
    fn image_key_no_ext_defaults_to_jpg() {
        let key = image_key_from_url("https://x.com/img", None);
        assert!(
            key.ends_with(".jpg"),
            "expected key ending with .jpg, got: {key}"
        );
    }

    #[test]
    fn image_key_starts_with_images_prefix() {
        let key = image_key_from_url("https://x.com/img.png", None);
        assert!(
            key.starts_with("images/"),
            "expected key starting with images/, got: {key}"
        );
    }
}
