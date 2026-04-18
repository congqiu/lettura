use sqlx::PgPool;
use std::sync::Arc;

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
}
