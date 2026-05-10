use async_trait::async_trait;
use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::region::Region;

use super::{ImageStorage, StorageError};

/// Check whether an S3 error message indicates a "not found" (404) response.
/// This is a pure logic function extracted for testability.
pub fn is_not_found_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("no such key") || lower.contains("not found") || lower.contains("404")
}

pub struct OssStorage {
    bucket: Box<Bucket>,
    public_url_prefix: String,
}

impl OssStorage {
    pub fn new(config: &crate::config::Config) -> Self {
        let region = Region::Custom {
            region: config.oss_region.clone(),
            endpoint: config.oss_endpoint.clone(),
        };

        let credentials = Credentials::new(
            Some(&config.oss_access_key),
            Some(&config.oss_secret_key),
            None,
            None,
            None,
        )
        .expect("invalid OSS credentials");

        let bucket = Bucket::new(&config.oss_bucket, region, credentials)
            .expect("invalid OSS bucket config")
            .with_path_style();

        let public_url_prefix = if config.oss_public_url.is_empty() {
            format!("{}/{}", config.oss_endpoint, config.oss_bucket)
        } else {
            config.oss_public_url.clone()
        };

        Self {
            bucket,
            public_url_prefix,
        }
    }
}

#[async_trait]
impl ImageStorage for OssStorage {
    async fn store(
        &self,
        key: &str,
        data: &[u8],
        content_type: &str,
    ) -> Result<String, StorageError> {
        self.bucket
            .put_object_with_content_type(key, data, content_type)
            .await
            .map_err(|e| StorageError::Upload(e.to_string()))?;

        Ok(format!("{}/{}", self.public_url_prefix, key))
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        self.bucket
            .delete_object(key)
            .await
            .map_err(|e| StorageError::Upload(e.to_string()))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        match self.bucket.get_object(key).await {
            Ok(data) => Ok(Some(data.to_vec())),
            Err(e) => {
                let msg = e.to_string();
                if is_not_found_error(&msg) {
                    Ok(None)
                } else {
                    Err(StorageError::Io(e.to_string()))
                }
            }
        }
    }

    async fn delete_prefix(&self, prefix: &str) -> Result<(), StorageError> {
        let results = self
            .bucket
            .list(prefix.to_string(), Some("/".to_string()))
            .await
            .map_err(|e| StorageError::Io(e.to_string()))?;
        for list_result in results {
            for obj in list_result.contents {
                self.bucket
                    .delete_object(&obj.key)
                    .await
                    .map_err(|e| StorageError::Upload(e.to_string()))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_not_found_error tests ---

    #[test]
    fn not_found_no_such_key() {
        assert!(is_not_found_error(
            "no such key: The specified key does not exist"
        ));
        assert!(is_not_found_error("No such key"));
        assert!(is_not_found_error("NO SUCH KEY"));
    }

    #[test]
    fn not_found_not_found() {
        assert!(is_not_found_error("Object not found in bucket"));
    }

    #[test]
    fn not_found_404() {
        assert!(is_not_found_error("HTTP 404 Not Found"));
    }

    #[test]
    fn not_found_case_insensitive() {
        assert!(is_not_found_error("NO SUCH KEY"));
        assert!(is_not_found_error("Not Found"));
        assert!(is_not_found_error("http 404"));
    }

    #[test]
    fn not_other_error() {
        assert!(!is_not_found_error("Access Denied"));
        assert!(!is_not_found_error("Internal Server Error"));
        assert!(!is_not_found_error("timeout"));
    }

    #[test]
    fn not_found_empty_string() {
        assert!(!is_not_found_error(""));
    }

    // --- StorageError display formatting tests ---

    #[test]
    fn storage_error_io_display() {
        let err = StorageError::Io("disk full".into());
        assert_eq!(format!("{err}"), "storage io error: disk full");
    }

    #[test]
    fn storage_error_upload_display() {
        let err = StorageError::Upload("connection refused".into());
        assert_eq!(format!("{err}"), "storage upload error: connection refused");
    }
}
