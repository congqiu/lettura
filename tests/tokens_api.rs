mod common;

use serde_json::json;

/// Register a user and return the JWT access token.
async fn register_and_login(app: &common::TestApp, suffix: &str) -> String {
    app.client
        .post(app.url("/api/v1/auth/register"))
        .json(&json!({
            "username": format!("tok_user_{suffix}"),
            "email": format!("tok_{suffix}@example.com"),
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    let res = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&json!({
            "email": format!("tok_{suffix}@example.com"),
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = res.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

// Test 1: Create token returns plaintext and 201
#[tokio::test]
async fn create_token_returns_plaintext_and_201() {
    let app = common::TestApp::new().await;
    let jwt = register_and_login(&app, "ct1").await;

    let res = app
        .client
        .post(app.url("/api/v1/tokens"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"name": "t1", "scope": "write"}))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 201);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["id"].as_str().is_some(), "id must be present");
    assert_eq!(body["name"].as_str().unwrap(), "t1");
    assert_eq!(body["scope"].as_str().unwrap(), "write");
    let token = body["token"].as_str().expect("token must be present");
    assert!(token.starts_with("lta_"), "token must start with lta_");
    assert!(token.len() >= 44, "token too short: {}", token.len());

    app.cleanup().await;
}

// Test 2: PAT cannot create token (must get 403)
#[tokio::test]
async fn pat_cannot_create_token() {
    let app = common::TestApp::new().await;
    let jwt = register_and_login(&app, "ct2").await;

    // Retrieve user_id from DB
    let (user_id,): (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM users WHERE email = 'tok_ct2@example.com'")
            .fetch_one(&app.pool)
            .await
            .unwrap();

    // Create a PAT directly via the DAO
    let pat_token = lettura::models::pat::generate_token();
    let hash = lettura::models::pat::hash_token(&pat_token);
    let prefix = lettura::models::pat::token_prefix(&pat_token);
    lettura::models::pat::insert(
        &app.pool,
        user_id,
        "my-pat",
        &hash,
        &prefix,
        lettura::models::pat::Scope::Write,
        None,
    )
    .await
    .unwrap();

    // Use PAT bearer to try creating a token — should be 403
    let res = app
        .client
        .post(app.url("/api/v1/tokens"))
        .header("Authorization", format!("Bearer {pat_token}"))
        .json(&json!({"name": "from-pat", "scope": "write"}))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 403);

    // Silence unused variable warning
    let _ = jwt;
    app.cleanup().await;
}

// Test 3: List tokens never exposes token_hash or plaintext token
#[tokio::test]
async fn list_tokens_never_exposes_token_hash_or_plaintext() {
    let app = common::TestApp::new().await;
    let jwt = register_and_login(&app, "ct3").await;

    // Create a token
    let create_res = app
        .client
        .post(app.url("/api/v1/tokens"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"name": "secret-tok", "scope": "read"}))
        .send()
        .await
        .unwrap();

    assert_eq!(create_res.status(), 201);
    let created: serde_json::Value = create_res.json().await.unwrap();
    let plaintext_token = created["token"].as_str().unwrap().to_string();

    // List tokens
    let list_res = app
        .client
        .get(app.url("/api/v1/tokens"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap();

    assert_eq!(list_res.status(), 200);
    let list_text = list_res.text().await.unwrap();

    // Must not contain token_hash key
    assert!(
        !list_text.contains("token_hash"),
        "token_hash key must not appear in list response"
    );
    // Must not contain the plaintext token
    assert!(
        !list_text.contains(&plaintext_token),
        "plaintext token must not appear in list response"
    );

    // Parse as array and check it has 1 item
    let arr: serde_json::Value = serde_json::from_str(&list_text).unwrap();
    assert!(arr.is_array());
    assert_eq!(arr.as_array().unwrap().len(), 1);

    app.cleanup().await;
}

// Test 4: Delete token removes it
#[tokio::test]
async fn delete_token_removes_it() {
    let app = common::TestApp::new().await;
    let jwt = register_and_login(&app, "ct4").await;

    // Create a token
    let create_res = app
        .client
        .post(app.url("/api/v1/tokens"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"name": "to-delete", "scope": "write"}))
        .send()
        .await
        .unwrap();

    assert_eq!(create_res.status(), 201);
    let created: serde_json::Value = create_res.json().await.unwrap();
    let id = created["id"].as_str().unwrap();

    // Delete it
    let del_res = app
        .client
        .delete(app.url(&format!("/api/v1/tokens/{id}")))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap();

    assert_eq!(del_res.status(), 204);

    // List should now be empty
    let list_res = app
        .client
        .get(app.url("/api/v1/tokens"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap();

    assert_eq!(list_res.status(), 200);
    let arr: serde_json::Value = list_res.json().await.unwrap();
    assert!(arr.as_array().unwrap().is_empty(), "list should be empty after delete");

    app.cleanup().await;
}

// Test 5: Delete unknown token returns 404
#[tokio::test]
async fn delete_token_404_for_unknown_id() {
    let app = common::TestApp::new().await;
    let jwt = register_and_login(&app, "ct5").await;

    let random_id = uuid::Uuid::new_v4();
    let res = app
        .client
        .delete(app.url(&format!("/api/v1/tokens/{random_id}")))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 404);

    app.cleanup().await;
}

// Test 6: Create token with invalid scope returns 400
#[tokio::test]
async fn create_token_with_invalid_scope_returns_400() {
    let app = common::TestApp::new().await;
    let jwt = register_and_login(&app, "ct6").await;

    let res = app
        .client
        .post(app.url("/api/v1/tokens"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"name": "bad", "scope": "admin"}))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 400);

    app.cleanup().await;
}

// Test 7: Create token with expires_in_days stores future timestamp
#[tokio::test]
async fn create_token_with_expires_in_days_stores_future_timestamp() {
    let app = common::TestApp::new().await;
    let jwt = register_and_login(&app, "ct7").await;

    let res = app
        .client
        .post(app.url("/api/v1/tokens"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"name": "expiring", "scope": "read", "expires_in_days": 30}))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 201);
    let created: serde_json::Value = res.json().await.unwrap();
    let id = created["id"].as_str().unwrap();

    // List and inspect the token
    let list_res = app
        .client
        .get(app.url("/api/v1/tokens"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap();

    assert_eq!(list_res.status(), 200);
    let arr: Vec<serde_json::Value> = list_res.json().await.unwrap();
    let row = arr.iter().find(|t| t["id"].as_str() == Some(id)).expect("token not found in list");

    let expires_at_str = row["expires_at"]
        .as_str()
        .expect("expires_at must be present when expires_in_days was set");

    let expires_at = chrono::DateTime::parse_from_rfc3339(expires_at_str)
        .expect("expires_at must be a valid RFC3339 timestamp");

    let now = chrono::Utc::now();
    let lower = now + chrono::Duration::days(29);
    let upper = now + chrono::Duration::days(31);

    assert!(
        expires_at > lower && expires_at < upper,
        "expires_at {expires_at} not within (now+29d, now+31d)"
    );

    app.cleanup().await;
}
