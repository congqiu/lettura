mod common;
use serde_json::json;

async fn get_auth_token(app: &common::TestApp) -> String {
    let res = app.client.post(app.url("/api/v1/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn create_entry_returns_ok() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;
    let res = app.client.post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/article"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["given_url"], "https://example.com/article");
    assert!(body["id"].is_string());
    assert_eq!(body["extract_method"], "pending");
    app.cleanup().await;
}

#[tokio::test]
async fn duplicate_url_returns_conflict() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;
    app.client.post(app.url("/api/v1/entries")).header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/dup"})).send().await.unwrap();
    let res = app.client.post(app.url("/api/v1/entries")).header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/dup"})).send().await.unwrap();
    assert_eq!(res.status(), 409);
    app.cleanup().await;
}

#[tokio::test]
async fn list_entries_empty() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;
    let res = app.client.get(app.url("/api/v1/entries")).header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: Vec<serde_json::Value> = res.json().await.unwrap();
    assert!(body.is_empty());
    app.cleanup().await;
}

#[tokio::test]
async fn get_entry_by_id() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;
    let res = app.client.post(app.url("/api/v1/entries")).header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/get-test"})).send().await.unwrap();
    let created: serde_json::Value = res.json().await.unwrap();
    let entry_id = created["id"].as_str().unwrap();
    let res = app.client.get(app.url(&format!("/api/v1/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {}", token)).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["id"], entry_id);
    app.cleanup().await;
}

#[tokio::test]
async fn update_entry_star_and_archive() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;
    let res = app.client.post(app.url("/api/v1/entries")).header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/update-test"})).send().await.unwrap();
    let created: serde_json::Value = res.json().await.unwrap();
    let entry_id = created["id"].as_str().unwrap();

    let res = app.client.patch(app.url(&format!("/api/v1/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"is_starred": true})).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["is_starred"], true);
    assert!(body["starred_at"].is_string());

    let res = app.client.patch(app.url(&format!("/api/v1/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"is_archived": true})).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["is_archived"], true);
    app.cleanup().await;
}

#[tokio::test]
async fn delete_entry_works() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;
    let res = app.client.post(app.url("/api/v1/entries")).header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/delete-test"})).send().await.unwrap();
    let created: serde_json::Value = res.json().await.unwrap();
    let entry_id = created["id"].as_str().unwrap();

    let res = app.client.delete(app.url(&format!("/api/v1/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {}", token)).send().await.unwrap();
    assert_eq!(res.status(), 200);

    let res = app.client.get(app.url(&format!("/api/v1/entries/{}", entry_id)))
        .header("Authorization", format!("Bearer {}", token)).send().await.unwrap();
    assert_eq!(res.status(), 404);
    app.cleanup().await;
}

#[tokio::test]
async fn unauthenticated_request_rejected() {
    let app = common::TestApp::new().await;
    let res = app.client.get(app.url("/api/v1/entries")).send().await.unwrap();
    assert_eq!(res.status(), 401);
    app.cleanup().await;
}
