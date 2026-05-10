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
async fn import_wallabag_json() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let res = app.client.post(app.url("/api/v1/import/wallabag"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!([
            {"url": "https://example.com/article1", "title": "Article 1", "is_archived": 1, "is_starred": 0},
            {"url": "https://example.com/article2", "title": "Article 2", "content": "<p>Imported content</p>"},
            {"url": "", "title": "No URL"}
        ]))
        .send().await.unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["imported"], 2);
    assert_eq!(body["skipped"], 1);

    app.cleanup().await;
}

#[tokio::test]
async fn import_browser_bookmarks() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
<DT><A HREF="https://example.com/bookmark1">Bookmark 1</A>
<DT><A HREF="https://example.com/bookmark2">Bookmark 2</A>
</DL>"#;

    let res = app
        .client
        .post(app.url("/api/v1/import/browser"))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "text/html")
        .body(html.to_string())
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["imported"], 2);

    app.cleanup().await;
}

#[tokio::test]
async fn export_all_entries() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    // Create an entry first
    app.client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/exported"}))
        .send()
        .await
        .unwrap();

    let res = app
        .client
        .get(app.url("/api/v1/export"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["entries"].is_array());
    assert_eq!(body["entries"].as_array().unwrap().len(), 1);
    assert!(body["exported_at"].is_string());

    app.cleanup().await;
}

#[tokio::test]
async fn rss_feed_with_valid_token() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    // Create an entry
    app.client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/rss-test"}))
        .send()
        .await
        .unwrap();

    // Get user's feed token from DB
    let feed_token: (String,) = sqlx::query_as("SELECT feed_token FROM users LIMIT 1")
        .fetch_one(&app.pool)
        .await
        .unwrap();

    let res = app
        .client
        .get(app.url(&format!("/feed/{}/unread", feed_token.0)))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    assert!(body.contains("<rss"));
    assert!(body.contains("Lettura"));

    app.cleanup().await;
}

#[tokio::test]
async fn rss_feed_invalid_token_returns_404() {
    let app = common::TestApp::new().await;

    let res = app
        .client
        .get(app.url("/feed/invalid-token/unread"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 404);

    app.cleanup().await;
}

#[tokio::test]
async fn admin_list_users() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await; // First user = admin

    let res = app
        .client
        .get(app.url("/api/v1/admin/users"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let users: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0]["username"], "testuser");
    // password_hash should NOT be in response
    assert!(users[0].get("password_hash").is_none());

    app.cleanup().await;
}

#[tokio::test]
async fn admin_reindex() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let res = app
        .client
        .post(app.url("/api/v1/admin/reindex"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["message"], "reindex complete");

    app.cleanup().await;
}
