use async_trait::async_trait;
use std::path::PathBuf;

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

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let file_path = self.base_path.join(key);
        match tokio::fs::read(&file_path).await {
            Ok(data) => Ok(Some(data)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StorageError::Io(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_store_and_get() {
        let dir = TempDir::new().unwrap();
        let storage = LocalStorage::new(dir.path().to_str().unwrap());
        storage.store("test/hello.txt", b"hello world", "text/plain").await.unwrap();
        let data = storage.get("test/hello.txt").await.unwrap();
        assert_eq!(data, Some(b"hello world".to_vec()));
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let dir = TempDir::new().unwrap();
        let storage = LocalStorage::new(dir.path().to_str().unwrap());
        let data = storage.get("nope.txt").await.unwrap();
        assert!(data.is_none());
    }

    #[tokio::test]
    async fn test_delete_existing_file() {
        let dir = TempDir::new().unwrap();
        let storage = LocalStorage::new(dir.path().to_str().unwrap());

        // Store a file first
        storage.store("delete_me.txt", b"to be deleted", "text/plain").await.unwrap();

        // Verify it exists
        let data = storage.get("delete_me.txt").await.unwrap();
        assert_eq!(data, Some(b"to be deleted".to_vec()));

        // Delete it
        storage.delete("delete_me.txt").await.unwrap();

        // Verify it's gone
        let data = storage.get("delete_me.txt").await.unwrap();
        assert!(data.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_file() {
        let dir = TempDir::new().unwrap();
        let storage = LocalStorage::new(dir.path().to_str().unwrap());

        // Deleting a non-existent key should return an error (file not found),
        // not panic or return Ok.
        let result = storage.delete("ghost.txt").await;
        assert!(result.is_err(), "deleting a nonexistent file should return an error");
    }

    #[tokio::test]
    async fn test_store_with_nested_key() {
        let dir = TempDir::new().unwrap();
        let storage = LocalStorage::new(dir.path().to_str().unwrap());

        // Store with a nested key that requires subdirectory creation
        storage.store("subdir/nested/file.txt", b"deep content", "text/plain").await.unwrap();

        // Verify retrieval works
        let data = storage.get("subdir/nested/file.txt").await.unwrap();
        assert_eq!(data, Some(b"deep content".to_vec()));

        // Verify the URL format
        let url = storage.store("subdir/nested/file.txt", b"deep content", "text/plain").await.unwrap();
        assert_eq!(url, "/storage/subdir/nested/file.txt");
    }
}
