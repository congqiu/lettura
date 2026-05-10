mod common;
use serde_json::json;

async fn get_token(app: &common::TestApp) -> String {
    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn create_and_list_memos() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;
    let res = app
        .client
        .post(app.url("/api/v1/memos"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"content": "remember this"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let memo: serde_json::Value = res.json().await.unwrap();
    assert_eq!(memo["content"], "remember this");
    let res = app
        .client
        .get(app.url("/api/v1/memos"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    let memos: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(memos.len(), 1);
    app.cleanup().await;
}

#[tokio::test]
async fn delete_memo() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;
    let res = app
        .client
        .post(app.url("/api/v1/memos"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"content": "to delete"}))
        .send()
        .await
        .unwrap();
    let memo: serde_json::Value = res.json().await.unwrap();
    let memo_id = memo["id"].as_str().unwrap();
    let res = app
        .client
        .delete(app.url(&format!("/api/v1/memos/{}", memo_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    app.cleanup().await;
}

#[tokio::test]
async fn promote_memo_with_url() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;
    let res = app
        .client
        .post(app.url("/api/v1/memos"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"content": "check out https://example.com/promoted"}))
        .send()
        .await
        .unwrap();
    let memo: serde_json::Value = res.json().await.unwrap();
    let memo_id = memo["id"].as_str().unwrap();
    let res = app
        .client
        .post(app.url(&format!("/api/v1/memos/{}/promote", memo_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["entry_id"].is_string());
    // Cannot promote again
    let res = app
        .client
        .post(app.url(&format!("/api/v1/memos/{}/promote", memo_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    app.cleanup().await;
}

#[tokio::test]
async fn empty_memo_rejected() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;
    let res = app
        .client
        .post(app.url("/api/v1/memos"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"content": ""}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    app.cleanup().await;
}
