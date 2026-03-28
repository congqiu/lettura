use async_trait::async_trait;
use std::path::{Path, PathBuf};

use super::{ImageStorage, StorageError};

pub struct LocalStorage {
    base_path: PathBuf,
}

impl LocalStorage {
    pub fn new(base_path: &str) -> Self {
        let base = PathBuf::from(base_path);
        std::fs::create_dir_all(&base).ok();
        Self { base_path: base }
    }
}

#[async_trait]
impl ImageStorage for LocalStorage {
    async fn store(&self, key: &str, data: &[u8], _content_type: &str) -> Result<String, StorageError> {
        let file_path = self.base_path.join(key);

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| StorageError::Io(e.to_string()))?;
        }

        tokio::fs::write(&file_path, data)
            .await
            .map_err(|e| StorageError::Io(e.to_string()))?;

        // Return relative URL served by the app
        Ok(format!("/storage/{}", key))
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let file_path = self.base_path.join(key);
        tokio::fs::remove_file(&file_path)
            .await
            .map_err(|e| StorageError::Io(e.to_string()))?;
        Ok(())
    }
}
