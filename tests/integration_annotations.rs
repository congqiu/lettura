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
        .json(&json!({"url": "https://example.com/annotated"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let entry_id = body["id"].as_str().unwrap().to_string();
    (token, entry_id)
}

#[tokio::test]
async fn create_and_list_annotations() {
    let app = common::TestApp::new().await;
    let (token, entry_id) = setup(&app).await;
    let res = app.client.post(app.url(&format!("/api/entries/{}/annotations", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"quote": "important text", "text": "my note", "ranges": []}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let ann: serde_json::Value = res.json().await.unwrap();
    assert_eq!(ann["quote"], "important text");
    assert_eq!(ann["text"], "my note");
    let res = app.client.get(app.url(&format!("/api/entries/{}/annotations", entry_id)))
        .header("Authorization", format!("Bearer {}", token)).send().await.unwrap();
    let anns: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(anns.len(), 1);
    app.cleanup().await;
}

#[tokio::test]
async fn update_annotation() {
    let app = common::TestApp::new().await;
    let (token, entry_id) = setup(&app).await;
    let res = app.client.post(app.url(&format!("/api/entries/{}/annotations", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"quote": "text", "ranges": []})).send().await.unwrap();
    let ann: serde_json::Value = res.json().await.unwrap();
    let ann_id = ann["id"].as_str().unwrap();
    let res = app.client.patch(app.url(&format!("/api/annotations/{}", ann_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"text": "updated note"})).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let updated: serde_json::Value = res.json().await.unwrap();
    assert_eq!(updated["text"], "updated note");
    app.cleanup().await;
}

#[tokio::test]
async fn delete_annotation() {
    let app = common::TestApp::new().await;
    let (token, entry_id) = setup(&app).await;
    let res = app.client.post(app.url(&format!("/api/entries/{}/annotations", entry_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"quote": "to delete", "ranges": []})).send().await.unwrap();
    let ann: serde_json::Value = res.json().await.unwrap();
    let ann_id = ann["id"].as_str().unwrap();
    let res = app.client.delete(app.url(&format!("/api/annotations/{}", ann_id)))
        .header("Authorization", format!("Bearer {}", token)).send().await.unwrap();
    assert_eq!(res.status(), 200);
    app.cleanup().await;
}
