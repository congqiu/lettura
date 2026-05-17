use sqlx::PgPool;
use std::sync::Arc;

use crate::cache::Caches;
use crate::config::Config;
use crate::search::SearchIndex;
use crate::storage::ImageStorage;
use crate::tasks::fetcher::FetchQueue;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub fetch_queue: FetchQueue,
    pub search_index: SearchIndex,
    pub storage: Arc<dyn ImageStorage>,
    /// Per-instance in-memory caches (tags, tag stats, rules). Held by Arc so
    /// AppState.clone() is cheap and handler code can pass `&state.caches`
    /// down to model functions without copying the cache contents.
    pub caches: Arc<Caches>,
}
