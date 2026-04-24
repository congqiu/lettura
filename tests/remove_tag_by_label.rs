mod common;
use common::TestApp;
use serde_json::json;

async fn jwt_and_entry(app: &TestApp) -> (String, String) {
    app.client.post(app.url("/api/v1/auth/register"))
        .json(&json!({"username":"u","email":"u@e.com","password":"password123"}))
        .send().await.unwrap();
    let login: serde_json::Value = app.client.post(app.url("/api/v1/auth/login"))
        .json(&json!({"email":"u@e.com","password":"password123"}))
        .send().await.unwrap().json().await.unwrap();
    let jwt = login["access_token"].as_str().unwrap().to_string();

    let entry: serde_json::Value = app.client.post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"url":"https://a.test"}))
        .send().await.unwrap().json().await.unwrap();
    let id = entry["id"].as_str().unwrap().to_string();
    (jwt, id)
}

#[tokio::test]
async fn delete_tag_by_label_removes_from_entry() {
    let app = TestApp::new().await;
    let (jwt, id) = jwt_and_entry(&app).await;

    // attach tag
    app.client.post(app.url(&format!("/api/v1/entries/{id}/tags")))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"label":"rust"}))
        .send().await.unwrap();

    // delete by label
    let r = app.client.delete(app.url(&format!("/api/v1/entries/{id}/tags/by-label/rust")))
        .header("Authorization", format!("Bearer {jwt}")).send().await.unwrap();
    assert_eq!(r.status(), 204);

    // list tags -> empty
    let tags: Vec<serde_json::Value> = app.client.get(app.url(&format!("/api/v1/entries/{id}/tags")))
        .header("Authorization", format!("Bearer {jwt}"))
        .send().await.unwrap().json().await.unwrap();
    assert!(tags.is_empty());
    app.cleanup().await;
}

#[tokio::test]
async fn delete_unknown_tag_label_returns_404() {
    let app = TestApp::new().await;
    let (jwt, id) = jwt_and_entry(&app).await;
    let r = app.client.delete(app.url(&format!("/api/v1/entries/{id}/tags/by-label/nonexistent")))
        .header("Authorization", format!("Bearer {jwt}")).send().await.unwrap();
    assert_eq!(r.status(), 404);
    app.cleanup().await;
}
