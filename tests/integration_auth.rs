mod common;
use serde_json::json;

#[tokio::test]
async fn register_first_user_becomes_admin() {
    let app = common::TestApp::new().await;
    let res = app.client.post(app.url("/api/auth/register"))
        .json(&json!({"username":"admin","email":"admin@example.com","password":"password123"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["access_token"].is_string());
    assert!(body["refresh_token"].is_string());
    assert_eq!(body["token_type"], "Bearer");
    app.cleanup().await;
}

#[tokio::test]
async fn register_duplicate_email_fails() {
    let app = common::TestApp::new().await;
    app.client.post(app.url("/api/auth/register"))
        .json(&json!({"username":"user1","email":"dup@example.com","password":"password123"}))
        .send().await.unwrap();
    let res = app.client.post(app.url("/api/auth/register"))
        .json(&json!({"username":"user2","email":"dup@example.com","password":"password456"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 409);
    app.cleanup().await;
}

#[tokio::test]
async fn login_with_valid_credentials() {
    let app = common::TestApp::new().await;
    app.client.post(app.url("/api/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    let res = app.client.post(app.url("/api/auth/login"))
        .json(&json!({"email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["access_token"].is_string());
    app.cleanup().await;
}

#[tokio::test]
async fn login_with_wrong_password_fails() {
    let app = common::TestApp::new().await;
    app.client.post(app.url("/api/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    let res = app.client.post(app.url("/api/auth/login"))
        .json(&json!({"email":"test@example.com","password":"wrongpassword"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 401);
    app.cleanup().await;
}

#[tokio::test]
async fn refresh_token_rotates() {
    let app = common::TestApp::new().await;
    let res = app.client.post(app.url("/api/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let refresh_token = body["refresh_token"].as_str().unwrap();

    let res = app.client.post(app.url("/api/auth/refresh"))
        .json(&json!({"refresh_token": refresh_token}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body2: serde_json::Value = res.json().await.unwrap();
    let new_refresh = body2["refresh_token"].as_str().unwrap();
    assert_ne!(refresh_token, new_refresh, "refresh token should rotate");

    let res = app.client.post(app.url("/api/auth/refresh"))
        .json(&json!({"refresh_token": refresh_token}))
        .send().await.unwrap();
    assert_eq!(res.status(), 401);
    app.cleanup().await;
}

#[tokio::test]
async fn logout_revokes_refresh_token() {
    let app = common::TestApp::new().await;
    let res = app.client.post(app.url("/api/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let access_token = body["access_token"].as_str().unwrap();
    let refresh_token = body["refresh_token"].as_str().unwrap();

    let res = app.client.post(app.url("/api/auth/logout"))
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({"refresh_token": refresh_token}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);

    let res = app.client.post(app.url("/api/auth/refresh"))
        .json(&json!({"refresh_token": refresh_token}))
        .send().await.unwrap();
    assert_eq!(res.status(), 401);
    app.cleanup().await;
}

#[tokio::test]
async fn short_password_rejected() {
    let app = common::TestApp::new().await;
    let res = app.client.post(app.url("/api/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"short"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 400);
    app.cleanup().await;
}
