mod common;
use serde_json::json;

async fn setup(app: &common::TestApp) -> (String, String) {
    let res = app.client.post(app.url("/api/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let token = body["access_token"].as_str().unwrap().to_string();
    let res = app.client.post(app.url("/api/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/tagged"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let entry_id = body["id"].as_str().unwrap().to_string();
    (token, entry_id)
}

#[tokio::test]
async fn add_and_list_tags() {
    let app = common::TestApp::new().await;
    let (token, entry_id) = setup(&app).await;
    let res = app.client.post(app.url(&format!("/api/entries/{}/tags", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"label": "Rust"})).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let tag: serde_json::Value = res.json().await.unwrap();
    assert_eq!(tag["label"], "Rust");
    assert_eq!(tag["slug"], "rust");
    let res = app.client.get(app.url("/api/tags"))
        .header("Authorization", format!("Bearer {}", token)).send().await.unwrap();
    let tags: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(tags.len(), 1);
    app.cleanup().await;
}

#[tokio::test]
async fn remove_tag_from_entry() {
    let app = common::TestApp::new().await;
    let (token, entry_id) = setup(&app).await;
    let res = app.client.post(app.url(&format!("/api/entries/{}/tags", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"label": "ToRemove"})).send().await.unwrap();
    let tag: serde_json::Value = res.json().await.unwrap();
    let tag_id = tag["id"].as_str().unwrap();
    let res = app.client.delete(app.url(&format!("/api/entries/{}/tags/{}", entry_id, tag_id)))
        .header("Authorization", format!("Bearer {}", token)).send().await.unwrap();
    assert_eq!(res.status(), 200);
    app.cleanup().await;
}

#[tokio::test]
async fn delete_tag() {
    let app = common::TestApp::new().await;
    let (token, entry_id) = setup(&app).await;
    let res = app.client.post(app.url(&format!("/api/entries/{}/tags", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"label": "Deletable"})).send().await.unwrap();
    let tag: serde_json::Value = res.json().await.unwrap();
    let tag_id = tag["id"].as_str().unwrap();
    let res = app.client.delete(app.url(&format!("/api/tags/{}", tag_id)))
        .header("Authorization", format!("Bearer {}", token)).send().await.unwrap();
    assert_eq!(res.status(), 200);
    app.cleanup().await;
}
