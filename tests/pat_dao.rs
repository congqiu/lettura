mod common;

use chrono::{Duration, Utc};
use lettura::models::pat;

/// Register a user and return the user_id from DB.
async fn register_user(
    app: &common::TestApp,
    username: &str,
    email: &str,
) -> uuid::Uuid {
    app.client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": username,
            "email": email,
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    let (user_id,): (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM users WHERE email = $1")
            .bind(email)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    user_id
}

/// Generate a fresh token, hash, and prefix triplet.
fn make_token() -> (String, String, String) {
    let token = pat::generate_token();
    let hash = pat::hash_token(&token);
    let prefix = pat::token_prefix(&token);
    (token, hash, prefix)
}

// ─── Test 1 ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn insert_then_lookup_by_hash_returns_row() {
    let app = common::TestApp::new().await;
    let user_id = register_user(&app, "u1", "u1@test.com").await;

    let (_token, hash, prefix) = make_token();
    let id = pat::insert(
        &app.pool,
        user_id,
        "my token",
        &hash,
        &prefix,
        pat::Scope::Read,
        None,
    )
    .await
    .unwrap();

    let found = pat::find_by_hash(&app.pool, &hash).await.unwrap();
    assert!(found.is_some(), "expected Some after insert");
    let row = found.unwrap();
    assert_eq!(row.id, id);
    assert_eq!(row.user_id, user_id);
    assert_eq!(row.name, "my token");
    assert_eq!(row.token_hash, hash);
    assert_eq!(row.token_prefix, prefix);
    assert_eq!(row.scope, "read");
    assert!(row.expires_at.is_none());
    assert!(row.last_used_at.is_none());

    app.cleanup().await;
}

// ─── Test 2 ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_for_user_returns_only_that_users_tokens() {
    let app = common::TestApp::new().await;
    let user1 = register_user(&app, "u2a", "u2a@test.com").await;
    let user2 = register_user(&app, "u2b", "u2b@test.com").await;

    let (_t, h1, p1) = make_token();
    let (_t, h2, p2) = make_token();
    let (_t, h3, p3) = make_token();

    pat::insert(&app.pool, user1, "tok-a", &h1, &p1, pat::Scope::Read, None)
        .await
        .unwrap();
    pat::insert(&app.pool, user1, "tok-b", &h2, &p2, pat::Scope::Write, None)
        .await
        .unwrap();
    pat::insert(&app.pool, user2, "tok-c", &h3, &p3, pat::Scope::Read, None)
        .await
        .unwrap();

    let list = pat::list_for_user(&app.pool, user1).await.unwrap();
    assert_eq!(list.len(), 2, "user1 should have exactly 2 tokens");
    for row in &list {
        assert_eq!(row.user_id, user1, "returned token must belong to user1");
    }

    app.cleanup().await;
}

// ─── Test 3 ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_removes_row() {
    let app = common::TestApp::new().await;
    let user_id = register_user(&app, "u3", "u3@test.com").await;

    let (_token, hash, prefix) = make_token();
    let id = pat::insert(
        &app.pool,
        user_id,
        "to-delete",
        &hash,
        &prefix,
        pat::Scope::Write,
        None,
    )
    .await
    .unwrap();

    let deleted = pat::delete(&app.pool, user_id, id).await.unwrap();
    assert!(deleted, "delete should return true when row existed");

    let found = pat::find_by_hash(&app.pool, &hash).await.unwrap();
    assert!(found.is_none(), "row should be gone after delete");

    app.cleanup().await;
}

// ─── Test 4 ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn find_valid_by_hash_skips_expired() {
    let app = common::TestApp::new().await;
    let user_id = register_user(&app, "u4", "u4@test.com").await;

    let (_token, hash, prefix) = make_token();
    let expires = Utc::now() - Duration::days(1);
    pat::insert(
        &app.pool,
        user_id,
        "expired-tok",
        &hash,
        &prefix,
        pat::Scope::Read,
        Some(expires),
    )
    .await
    .unwrap();

    // find_valid_by_hash must return None for expired token
    let valid = pat::find_valid_by_hash(&app.pool, &hash).await.unwrap();
    assert!(valid.is_none(), "find_valid_by_hash must skip expired tokens");

    // find_by_hash still returns the row
    let raw = pat::find_by_hash(&app.pool, &hash).await.unwrap();
    assert!(raw.is_some(), "find_by_hash must still return expired token");

    app.cleanup().await;
}

// ─── Test 5 ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn touch_last_used_sets_timestamp_when_null() {
    let app = common::TestApp::new().await;
    let user_id = register_user(&app, "u5", "u5@test.com").await;

    let (_token, hash, prefix) = make_token();
    let id = pat::insert(
        &app.pool,
        user_id,
        "touch-me",
        &hash,
        &prefix,
        pat::Scope::Read,
        None,
    )
    .await
    .unwrap();

    // Before touch: last_used_at is NULL
    let before = pat::find_by_hash(&app.pool, &hash).await.unwrap().unwrap();
    assert!(before.last_used_at.is_none(), "last_used_at should start as None");

    pat::touch_last_used(&app.pool, id).await;

    let after = pat::find_by_hash(&app.pool, &hash).await.unwrap().unwrap();
    assert!(after.last_used_at.is_some(), "last_used_at should be set after touch");

    app.cleanup().await;
}

// ─── Test 6 ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn touch_last_used_debounces_within_60s() {
    let app = common::TestApp::new().await;
    let user_id = register_user(&app, "u6", "u6@test.com").await;

    let (_token, hash, prefix) = make_token();
    let id = pat::insert(
        &app.pool,
        user_id,
        "debounce-me",
        &hash,
        &prefix,
        pat::Scope::Read,
        None,
    )
    .await
    .unwrap();

    // Manually set last_used_at to 10 seconds ago (within 60 s debounce window)
    sqlx::query(
        "UPDATE personal_access_tokens SET last_used_at = now() - INTERVAL '10 seconds' WHERE id = $1",
    )
    .bind(id)
    .execute(&app.pool)
    .await
    .unwrap();

    let t1 = pat::find_by_hash(&app.pool, &hash)
        .await
        .unwrap()
        .unwrap()
        .last_used_at
        .unwrap();

    // Call touch — should be a no-op because 10 s < 60 s
    pat::touch_last_used(&app.pool, id).await;

    let t2 = pat::find_by_hash(&app.pool, &hash)
        .await
        .unwrap()
        .unwrap()
        .last_used_at
        .unwrap();

    assert_eq!(t1, t2, "last_used_at must not change within debounce window");

    app.cleanup().await;
}
