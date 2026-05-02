//! In-memory caching layer using moka.
//!
//! Provides user-scoped caching for frequently accessed data like tags,
//! tagging rules, and site rules. Uses TTL-based expiration and LRU eviction.

use std::time::Duration;
use std::sync::Arc;
use uuid::Uuid;
use moka::future::Cache;

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
}

// ============================================================================
// Cache instances for various data types
// ============================================================================

use crate::models::tag::Tag;
use crate::models::tagging_rule::TaggingRule;
use crate::models::site_rule::SiteRule;

/// Cache for user tags (5 minute TTL, 1000 users max).
pub static TAG_CACHE: once_cell::sync::Lazy<Arc<UserCache<Tag>>> =
    once_cell::sync::Lazy::new(|| {
        Arc::new(UserCache::new(1000, Duration::from_secs(300)))
    });

/// Cache for tagging rules (5 minute TTL, 1000 users max).
/// This is the highest priority cache as it's queried on every fetch.
pub static TAGGING_RULE_CACHE: once_cell::sync::Lazy<Arc<UserCache<TaggingRule>>> =
    once_cell::sync::Lazy::new(|| {
        Arc::new(UserCache::new(1000, Duration::from_secs(300)))
    });

/// Cache for site rules (5 minute TTL, 500 users max).
pub static SITE_RULE_CACHE: once_cell::sync::Lazy<Arc<UserCache<SiteRule>>> =
    once_cell::sync::Lazy::new(|| {
        Arc::new(UserCache::new(500, Duration::from_secs(300)))
    });

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
}
