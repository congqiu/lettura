//! In-memory caching layer using moka.
//!
//! Provides user-scoped caching for frequently accessed data like tags,
//! tagging rules, and site rules. Uses TTL-based expiration and LRU eviction.
//!
//! Caches are owned by [`Caches`] and reached via `AppState` / passed down
//! to model functions. Tests get their own instance per `TestApp::new`, so
//! state never leaks across test cases. In a future multi-replica deployment
//! the per-instance ownership also lets us swap an implementation (e.g.
//! Redis-backed) without changing call sites.

use moka::future::Cache;
use std::time::Duration;
use uuid::Uuid;

/// A cache keyed by user ID, storing per-user data.
///
/// Each user's data is cached independently with configurable TTL and capacity.
pub struct UserCache<T> {
    inner: Cache<Uuid, Vec<T>>,
}

impl<T: Clone + Send + Sync + 'static> UserCache<T> {
    /// Create a new user cache with the given capacity and TTL.
    ///
    /// # Arguments
    ///
    /// * `max_capacity` - Maximum number of users to cache
    /// * `ttl` - Time-to-live for cached entries
    pub fn new(max_capacity: u64, ttl: Duration) -> Self {
        Self {
            inner: Cache::builder()
                .max_capacity(max_capacity)
                .time_to_live(ttl)
                .build(),
        }
    }

    /// Get cached data for a user.
    pub async fn get(&self, user_id: Uuid) -> Option<Vec<T>> {
        self.inner.get(&user_id).await
    }

    /// Insert data for a user.
    pub async fn insert(&self, user_id: Uuid, data: Vec<T>) {
        self.inner.insert(user_id, data).await;
    }

    /// Invalidate cached data for a user.
    pub async fn invalidate(&self, user_id: Uuid) {
        self.inner.invalidate(&user_id).await;
    }

    /// Invalidate all cached data.
    pub async fn invalidate_all(&self) {
        self.inner.invalidate_all();
    }

    /// Get the approximate number of entries in the cache.
    pub fn entry_count(&self) -> u64 {
        self.inner.entry_count()
    }

    /// Run pending maintenance tasks (e.g. eviction) to make entry_count accurate.
    pub async fn run_pending_tasks(&self) {
        self.inner.run_pending_tasks().await;
    }
}

// ============================================================================
// Cache bundle owned by AppState
// ============================================================================

use crate::models::site_rule::SiteRule;
use crate::models::tag::{Tag, TagStats};
use crate::models::tagging_rule::TaggingRule;

/// Per-instance cache bundle. Held by `AppState` as `Arc<Caches>` so handler
/// and model code can pass `&state.caches` down without paying for clones.
pub struct Caches {
    /// User tags (5 min TTL, 1000 users max).
    pub tags: UserCache<Tag>,
    /// Tag stats (5 min TTL, 1000 users max).
    pub tag_stats: UserCache<TagStats>,
    /// Tagging rules (5 min TTL, 1000 users max). Highest-traffic — queried
    /// on every fetch pipeline run.
    pub tagging_rules: UserCache<TaggingRule>,
    /// Site rules (5 min TTL, 500 users max).
    pub site_rules: UserCache<SiteRule>,
}

impl Caches {
    pub fn new() -> Self {
        let ttl = Duration::from_secs(300);
        Self {
            tags: UserCache::new(1000, ttl),
            tag_stats: UserCache::new(1000, ttl),
            tagging_rules: UserCache::new(1000, ttl),
            site_rules: UserCache::new(500, ttl),
        }
    }
}

impl Default for Caches {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct TestData {
        value: i32,
    }

    #[tokio::test]
    async fn test_user_cache_basic_operations() {
        let cache: UserCache<TestData> = UserCache::new(100, Duration::from_secs(60));
        let user_id = Uuid::new_v4();

        // Initially empty
        assert!(cache.get(user_id).await.is_none());

        // Insert and retrieve
        cache.insert(user_id, vec![TestData { value: 1 }]).await;
        let result = cache.get(user_id).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1);

        // Invalidate
        cache.invalidate(user_id).await;
        assert!(cache.get(user_id).await.is_none());
    }

    #[tokio::test]
    async fn test_user_cache_ttl_expiration() {
        let cache: UserCache<TestData> = UserCache::new(100, Duration::from_millis(50));
        let user_id = Uuid::new_v4();

        cache.insert(user_id, vec![TestData { value: 1 }]).await;
        assert!(cache.get(user_id).await.is_some());

        // Wait for TTL
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(cache.get(user_id).await.is_none());
    }

    #[tokio::test]
    async fn invalidate_all() {
        let cache: UserCache<TestData> = UserCache::new(100, Duration::from_secs(60));
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        cache.insert(user1, vec![TestData { value: 1 }]).await;
        cache.insert(user2, vec![TestData { value: 2 }]).await;

        assert!(cache.get(user1).await.is_some());
        assert!(cache.get(user2).await.is_some());

        cache.invalidate_all().await;

        assert!(cache.get(user1).await.is_none());
        assert!(cache.get(user2).await.is_none());
    }

    #[tokio::test]
    async fn entry_count() {
        let cache: UserCache<TestData> = UserCache::new(100, Duration::from_secs(60));

        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        cache.insert(user1, vec![TestData { value: 1 }]).await;
        cache.run_pending_tasks().await;
        assert_eq!(cache.entry_count(), 1);

        cache.insert(user2, vec![TestData { value: 2 }]).await;
        cache.run_pending_tasks().await;
        assert_eq!(cache.entry_count(), 2);
    }

    #[tokio::test]
    async fn multi_user_isolation() {
        let cache: UserCache<TestData> = UserCache::new(100, Duration::from_secs(60));
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        cache
            .insert(user1, vec![TestData { value: 10 }, TestData { value: 20 }])
            .await;
        cache.insert(user2, vec![TestData { value: 30 }]).await;

        let result1 = cache.get(user1).await.unwrap();
        assert_eq!(result1.len(), 2);
        assert_eq!(result1[0].value, 10);
        assert_eq!(result1[1].value, 20);

        let result2 = cache.get(user2).await.unwrap();
        assert_eq!(result2.len(), 1);
        assert_eq!(result2[0].value, 30);

        // Invalidate user1 does not affect user2
        cache.invalidate(user1).await;
        assert!(cache.get(user1).await.is_none());
        assert!(cache.get(user2).await.is_some());
    }
}
