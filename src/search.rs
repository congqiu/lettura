use std::path::Path;
use std::sync::Arc;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
pub struct SearchIndex {
    index: Index,
    writer: Arc<Mutex<IndexWriter>>,
    reader: IndexReader,
    schema: Schema,
    // Field handles
    f_id: Field,
    f_title: Field,
    f_content: Field,
    f_url: Field,
    f_domain: Field,
}

impl SearchIndex {
    /// Open or create an index at the given directory path
    pub fn open(index_path: &Path) -> Result<Self, tantivy::TantivyError> {
        let mut schema_builder = Schema::builder();
        let f_id = schema_builder.add_text_field("id", STRING | STORED);
        let f_title = schema_builder.add_text_field("title", TEXT | STORED);
        let f_content = schema_builder.add_text_field("content", TEXT);
        let f_url = schema_builder.add_text_field("url", STRING | STORED);
        let f_domain = schema_builder.add_text_field("domain", STRING);
        let schema = schema_builder.build();

        std::fs::create_dir_all(index_path).ok();

        let index = if Index::open_in_dir(index_path).is_ok() {
            Index::open_in_dir(index_path)?
        } else {
            Index::create_in_dir(index_path, schema.clone())?
        };

        let writer = index.writer(50_000_000)?; // 50MB buffer
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Self {
            index,
            writer: Arc::new(Mutex::new(writer)),
            reader,
            schema,
            f_id,
            f_title,
            f_content,
            f_url,
            f_domain,
        })
    }

    /// Create an in-memory index (for testing)
    pub fn in_memory() -> Result<Self, tantivy::TantivyError> {
        let mut schema_builder = Schema::builder();
        let f_id = schema_builder.add_text_field("id", STRING | STORED);
        let f_title = schema_builder.add_text_field("title", TEXT | STORED);
        let f_content = schema_builder.add_text_field("content", TEXT);
        let f_url = schema_builder.add_text_field("url", STRING | STORED);
        let f_domain = schema_builder.add_text_field("domain", STRING);
        let schema = schema_builder.build();

        let index = Index::create_in_ram(schema.clone());
        let writer = index.writer(15_000_000)?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Self {
            index,
            writer: Arc::new(Mutex::new(writer)),
            reader,
            schema,
            f_id,
            f_title,
            f_content,
            f_url,
            f_domain,
        })
    }

    /// Add or update a document in the index
    pub async fn upsert(
        &self,
        id: Uuid,
        title: &str,
        content: &str,
        url: &str,
        domain: &str,
    ) -> Result<(), tantivy::TantivyError> {
        let mut writer = self.writer.lock().await;
        // Delete existing doc first
        let id_str = id.to_string();
        let id_term = tantivy::Term::from_field_text(self.f_id, &id_str);
        writer.delete_term(id_term);
        writer.add_document(doc!(
            self.f_id => id_str,
            self.f_title => title,
            self.f_content => content,
            self.f_url => url,
            self.f_domain => domain,
        ))?;
        writer.commit()?;
        Ok(())
    }

    /// Remove a document from the index
    pub async fn delete(&self, id: Uuid) -> Result<(), tantivy::TantivyError> {
        let mut writer = self.writer.lock().await;
        let id_term = tantivy::Term::from_field_text(self.f_id, &id.to_string());
        writer.delete_term(id_term);
        writer.commit()?;
        Ok(())
    }

    /// Get a reference to the reader (useful for testing to manually reload)
    pub fn reader(&self) -> &IndexReader {
        &self.reader
    }

    /// Search and return matching entry UUIDs
    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<Uuid>, tantivy::TantivyError> {
        let searcher = self.reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.f_title, self.f_content]);
        let query = query_parser.parse_query(query_str)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut ids = Vec::new();
        for (_score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;
            if let Some(id_val) = doc.get_first(self.f_id) {
                if let Some(id_str) = id_val.as_str() {
                    if let Ok(uuid) = Uuid::parse_str(id_str) {
                        ids.push(uuid);
                    }
                }
            }
        }
        Ok(ids)
    }

    /// Return the number of documents in the index (for health checks)
    pub fn doc_count(&self) -> Result<u64, tantivy::TantivyError> {
        let searcher = self.reader.searcher();
        Ok(searcher.num_docs())
    }

    /// Clear index and rebuild from scratch
    pub async fn clear(&self) -> Result<(), tantivy::TantivyError> {
        let mut writer = self.writer.lock().await;
        writer.delete_all_documents()?;
        writer.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn index_and_search() {
        let idx = SearchIndex::in_memory().unwrap();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        idx.upsert(id1, "Rust Ownership", "Learn about ownership and borrowing in Rust", "https://example.com/rust", "example.com").await.unwrap();
        idx.upsert(id2, "Python Guide", "A beginner guide to Python programming", "https://example.com/python", "example.com").await.unwrap();

        // Reload reader to see committed changes
        idx.reader.reload().unwrap();

        let results = idx.search("ownership", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id1);

        let results = idx.search("Python", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id2);
    }

    #[tokio::test]
    async fn upsert_replaces_existing() {
        let idx = SearchIndex::in_memory().unwrap();
        let id = Uuid::new_v4();

        idx.upsert(id, "Old Title", "old content", "https://example.com", "example.com").await.unwrap();
        idx.upsert(id, "New Title", "new content about Rust", "https://example.com", "example.com").await.unwrap();

        idx.reader.reload().unwrap();

        let results = idx.search("old", 10).unwrap();
        assert!(results.is_empty());

        let results = idx.search("Rust", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id);
    }

    #[tokio::test]
    async fn delete_removes_from_index() {
        let idx = SearchIndex::in_memory().unwrap();
        let id = Uuid::new_v4();

        idx.upsert(id, "Title", "searchable content", "https://example.com", "example.com").await.unwrap();
        idx.reader.reload().unwrap();
        assert_eq!(idx.search("searchable", 10).unwrap().len(), 1);

        idx.delete(id).await.unwrap();
        idx.reader.reload().unwrap();
        assert_eq!(idx.search("searchable", 10).unwrap().len(), 0);
    }
}
