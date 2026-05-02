use std::path::Path;
use std::sync::Arc;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, QueryParser, TermQuery};
use tantivy::schema::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
pub struct SearchIndex {
    index: Index,
    writer: Arc<Mutex<IndexWriter>>,
    reader: IndexReader,
    // Field handles
    f_id: Field,
    f_title: Field,
    f_content: Field,
    f_url: Field,
    f_domain: Field,
    f_user_id: Field,
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
        let f_user_id = schema_builder.add_text_field("user_id", STRING | STORED);
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
            f_id,
            f_title,
            f_content,
            f_url,
            f_domain,
            f_user_id,
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
        let f_user_id = schema_builder.add_text_field("user_id", STRING | STORED);
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
            f_id,
            f_title,
            f_content,
            f_url,
            f_domain,
            f_user_id,
        })
    }

    /// Add or update a document in the index. Writes to the in-memory buffer
    /// without committing — call `commit()` or rely on the background flush
    /// task to persist changes.
    pub async fn upsert(
        &self,
        id: Uuid,
        user_id: Uuid,
        title: &str,
        content: &str,
        url: &str,
        domain: &str,
    ) -> Result<(), tantivy::TantivyError> {
        let writer = self.writer.lock().await;
        let id_str = id.to_string();
        let id_term = tantivy::Term::from_field_text(self.f_id, &id_str);
        writer.delete_term(id_term);
        writer.add_document(doc!(
            self.f_id => id_str,
            self.f_title => title,
            self.f_content => content,
            self.f_url => url,
            self.f_domain => domain,
            self.f_user_id => user_id.to_string(),
        ))?;
        Ok(())
    }

    /// Remove a document from the index (buffered, not committed).
    pub async fn delete(&self, id: Uuid) -> Result<(), tantivy::TantivyError> {
        let writer = self.writer.lock().await;
        let id_term = tantivy::Term::from_field_text(self.f_id, &id.to_string());
        writer.delete_term(id_term);
        Ok(())
    }

    /// Flush pending changes to disk. Expensive — use sparingly.
    pub async fn commit(&self) -> Result<(), tantivy::TantivyError> {
        let mut writer = self.writer.lock().await;
        writer.commit()?;
        Ok(())
    }

    /// Get a reference to the reader (useful for testing to manually reload)
    pub fn reader(&self) -> &IndexReader {
        &self.reader
    }

    /// Search and return matching entry UUIDs.
    /// Uses fuzzy matching so partial terms match (e.g., "n" matches "n8n", "rus" matches "rust").
    pub fn search(&self, query_str: &str, user_id: Option<Uuid>, limit: usize) -> Result<Vec<Uuid>, tantivy::TantivyError> {
        let searcher = self.reader.searcher();
        let mut query_parser = QueryParser::for_index(&self.index, vec![self.f_title, self.f_content]);

        // Enable fuzzy matching on both fields so partial terms match.
        // transposition_cost_one=true and prefix_length=0 mean the entire term is fuzzy.
        query_parser.set_field_fuzzy(self.f_title, true, 1, true);
        query_parser.set_field_fuzzy(self.f_content, true, 1, true);

        let text_query = query_parser.parse_query(query_str)?;

        let query: Box<dyn tantivy::query::Query> = if let Some(uid) = user_id {
            let user_term = tantivy::Term::from_field_text(self.f_user_id, &uid.to_string());
            let user_query = Box::new(TermQuery::new(user_term, IndexRecordOption::Basic));
            Box::new(BooleanQuery::new(vec![
                (Occur::Must, Box::new(text_query) as _),
                (Occur::Must, user_query as _),
            ]))
        } else {
            Box::new(text_query)
        };

        let top_docs = searcher.search(query.as_ref(), &TopDocs::with_limit(limit))?;

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

    /// Clear index (buffered, not committed).
    pub async fn clear(&self) -> Result<(), tantivy::TantivyError> {
        let writer = self.writer.lock().await;
        writer.delete_all_documents()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_user_id() -> Uuid {
        Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()
    }

    fn test_user_id_2() -> Uuid {
        Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap()
    }

    #[tokio::test]
    async fn index_and_search() {
        let idx = SearchIndex::in_memory().unwrap();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let uid = test_user_id();

        idx.upsert(id1, uid, "Rust Ownership", "Learn about ownership and borrowing in Rust", "https://example.com/rust", "example.com").await.unwrap();
        idx.upsert(id2, uid, "Python Guide", "A beginner guide to Python programming", "https://example.com/python", "example.com").await.unwrap();
        idx.commit().await.unwrap();

        idx.reader.reload().unwrap();

        let results = idx.search("ownership", Some(uid), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id1);

        let results = idx.search("Python", Some(uid), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id2);
    }

    #[tokio::test]
    async fn upsert_replaces_existing() {
        let idx = SearchIndex::in_memory().unwrap();
        let id = Uuid::new_v4();
        let uid = test_user_id();

        idx.upsert(id, uid, "Old Title", "old content", "https://example.com", "example.com").await.unwrap();
        idx.upsert(id, uid, "New Title", "new content about Rust", "https://example.com", "example.com").await.unwrap();
        idx.commit().await.unwrap();

        idx.reader.reload().unwrap();

        let results = idx.search("old", Some(uid), 10).unwrap();
        assert!(results.is_empty());

        let results = idx.search("Rust", Some(uid), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id);
    }

    #[tokio::test]
    async fn delete_removes_from_index() {
        let idx = SearchIndex::in_memory().unwrap();
        let id = Uuid::new_v4();
        let uid = test_user_id();

        idx.upsert(id, uid, "Title", "searchable content", "https://example.com", "example.com").await.unwrap();
        idx.commit().await.unwrap();
        idx.reader.reload().unwrap();
        assert_eq!(idx.search("searchable", Some(uid), 10).unwrap().len(), 1);

        idx.delete(id).await.unwrap();
        idx.commit().await.unwrap();
        idx.reader.reload().unwrap();
        assert_eq!(idx.search("searchable", Some(uid), 10).unwrap().len(), 0);
    }

    #[tokio::test]
    async fn search_filters_by_user() {
        let idx = SearchIndex::in_memory().unwrap();
        let uid1 = test_user_id();
        let uid2 = test_user_id_2();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        idx.upsert(id1, uid1, "Rust Guide", "Learn Rust programming", "https://example.com/rust", "example.com").await.unwrap();
        idx.upsert(id2, uid2, "Rust Guide", "Another Rust tutorial", "https://example.com/rust2", "example.com").await.unwrap();
        idx.commit().await.unwrap();

        idx.reader.reload().unwrap();

        let results = idx.search("Rust", Some(uid1), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id1);

        let results = idx.search("Rust", Some(uid2), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id2);

        let results = idx.search("Rust", None, 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn fuzzy_search_matches_partial_tokens() {
        let idx = SearchIndex::in_memory().unwrap();
        let uid = test_user_id();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        idx.upsert(id1, uid, "n8n workflow automation", "Automate tasks with n8n", "https://n8n.io", "n8n.io").await.unwrap();
        idx.upsert(id2, uid, "Rust Guide", "Learn Rust programming", "https://example.com/rust", "example.com").await.unwrap();
        idx.commit().await.unwrap();

        idx.reader.reload().unwrap();

        // "n" should match "n8n" via fuzzy matching (edit distance 1)
        let results = idx.search("n", Some(uid), 10).unwrap();
        assert!(results.contains(&id1), "'n' should match 'n8n' entry, got {:?}", results);

        // "n8" should also match "n8n"
        let results = idx.search("n8", Some(uid), 10).unwrap();
        assert!(results.contains(&id1), "'n8' should match 'n8n' entry, got {:?}", results);

        // Full term still works
        let results = idx.search("n8n", Some(uid), 10).unwrap();
        assert!(results.contains(&id1), "'n8n' should match 'n8n' entry, got {:?}", results);

        // "Rus" should match "Rust" via fuzzy
        let results = idx.search("Rus", Some(uid), 10).unwrap();
        assert!(results.contains(&id2), "'Rus' should match 'Rust' entry, got {:?}", results);
    }
}
