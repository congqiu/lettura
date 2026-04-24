mod common;

use chrono::{Duration, Utc};
use serde_json::json;

async fn register_user(app: &common::TestApp, suffix: &str) -> uuid::Uuid {
    app.client
        .post(app.url("/api/v1/auth/register"))
        .json(&json!({
            "username": format!("user_{suffix}"),
            "email": format!("user_{suffix}@example.com"),
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    let (user_id,): (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM users WHERE email = $1")
            .bind(format!("user_{suffix}@example.com"))
            .fetch_one(&app.pool)
            .await
            .unwrap();
    user_id
}

// Test 1: PAT with Write scope authenticates and injects user
#[tokio::test]
async fn pat_bearer_authenticates_and_injects_user() {
    let app = common::TestApp::new().await;
    let user_id = register_user(&app, "pat1").await;

    let token = lettura::models::pat::generate_token();
    let hash = lettura::models::pat::hash_token(&token);
    let prefix = lettura::models::pat::token_prefix(&token);
    lettura::models::pat::insert(
        &app.pool,
        user_id,
        "test-pat",
        &hash,
        &prefix,
        lettura::models::pat::Scope::Write,
        None,
    )
    .await
    .unwrap();

    let res = app
        .client
        .get(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    app.cleanup().await;
}

// Test 2: Read scope blocks write (POST) requests
#[tokio::test]
async fn read_scope_blocks_write_request() {
    let app = common::TestApp::new().await;
    let user_id = register_user(&app, "pat2").await;

    let token = lettura::models::pat::generate_token();
    let hash = lettura::models::pat::hash_token(&token);
    let prefix = lettura::models::pat::token_prefix(&token);
    lettura::models::pat::insert(
        &app.pool,
        user_id,
        "read-only-pat",
        &hash,
        &prefix,
        lettura::models::pat::Scope::Read,
        None,
    )
    .await
    .unwrap();

    let res = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({"url": "https://x.test"}))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 403);
    app.cleanup().await;
}

// Test 3: Read scope allows GET requests
#[tokio::test]
async fn read_scope_allows_get_requests() {
    let app = common::TestApp::new().await;
    let user_id = register_user(&app, "pat3").await;

    let token = lettura::models::pat::generate_token();
    let hash = lettura::models::pat::hash_token(&token);
    let prefix = lettura::models::pat::token_prefix(&token);
    lettura::models::pat::insert(
        &app.pool,
        user_id,
        "read-scope-get-pat",
        &hash,
        &prefix,
        lettura::models::pat::Scope::Read,
        None,
    )
    .await
    .unwrap();

    let res = app
        .client
        .get(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    app.cleanup().await;
}

// Test 4: Expired PAT returns 401
#[tokio::test]
async fn expired_pat_returns_401() {
    let app = common::TestApp::new().await;
    let user_id = register_user(&app, "pat4").await;

    let token = lettura::models::pat::generate_token();
    let hash = lettura::models::pat::hash_token(&token);
    let prefix = lettura::models::pat::token_prefix(&token);
    let expired_at = Utc::now() - Duration::days(1);
    lettura::models::pat::insert(
        &app.pool,
        user_id,
        "expired-pat",
        &hash,
        &prefix,
        lettura::models::pat::Scope::Write,
        Some(expired_at),
    )
    .await
    .unwrap();

    let res = app
        .client
        .get(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
    app.cleanup().await;
}

// Test 5: Unknown/random PAT returns 401
#[tokio::test]
async fn unknown_pat_returns_401() {
    let app = common::TestApp::new().await;
    // No user, no PAT insert — just use a random lta_ token
    let random_token = "lta_thisTokenDoesNotExistInTheDatabaseAtAll1234567";

    let res = app
        .client
        .get(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {random_token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
    app.cleanup().await;
}

// Test 6: JWT still works alongside PAT
#[tokio::test]
async fn jwt_still_works_alongside_pat() {
    let app = common::TestApp::new().await;

    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&json!({
            "username": "jwt_user",
            "email": "jwt@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    let access_token = body["access_token"].as_str().unwrap().to_string();

    let res = app
        .client
        .get(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    app.cleanup().await;
}
