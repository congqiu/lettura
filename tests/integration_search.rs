mod common;
use serde_json::json;

async fn setup_with_entries(app: &common::TestApp) -> String {
    let res = app.client.post(app.url("/api/v1/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let token = body["access_token"].as_str().unwrap().to_string();

    // Create entries with known URLs
    app.client.post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/rust-ownership"}))
        .send().await.unwrap();

    app.client.post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/python-guide"}))
        .send().await.unwrap();

    // Manually update entry content via DB so we can search it
    // (fetcher won't actually fetch in test since example.com won't return article content)
    sqlx::query("UPDATE entries SET title = 'Rust Ownership Guide', content = 'Learn about ownership and borrowing', text_content = 'Learn about ownership and borrowing', extract_method = 'readability' WHERE url = 'https://example.com/rust-ownership'")
        .execute(&app.pool).await.unwrap();

    sqlx::query("UPDATE entries SET title = 'Python Programming', content = 'A beginner guide to Python', text_content = 'A beginner guide to Python', extract_method = 'readability' WHERE url = 'https://example.com/python-guide'")
        .execute(&app.pool).await.unwrap();

    // Index them in search
    let entries: Vec<(uuid::Uuid, uuid::Uuid, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, user_id, title, COALESCE(text_content, ''), url, domain_name FROM entries WHERE user_id = (SELECT id FROM users LIMIT 1)"
    ).fetch_all(&app.pool).await.unwrap();

    for (id, user_id, title, text_content, url, domain) in &entries {
        app.search_index.upsert(*id, *user_id, title, text_content, url, domain.as_deref().unwrap_or("")).await.unwrap();
    }
    app.search_index.commit().await.unwrap();
    app.search_index.reader().reload().unwrap();

    token
}

#[tokio::test]
async fn search_finds_matching_entry() {
    let app = common::TestApp::new().await;
    let token = setup_with_entries(&app).await;

    let res = app.client.get(app.url("/api/v1/entries?search=ownership"))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();

    assert_eq!(res.status(), 200);
    let entries: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["title"], "Rust Ownership Guide");

    app.cleanup().await;
}

#[tokio::test]
async fn search_returns_empty_for_no_match() {
    let app = common::TestApp::new().await;
    let token = setup_with_entries(&app).await;

    let res = app.client.get(app.url("/api/v1/entries?search=nonexistent"))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();

    assert_eq!(res.status(), 200);
    let entries: Vec<serde_json::Value> = res.json().await.unwrap();
    assert!(entries.is_empty());

    app.cleanup().await;
}

#[tokio::test]
async fn empty_search_returns_all() {
    let app = common::TestApp::new().await;
    let token = setup_with_entries(&app).await;

    let res = app.client.get(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();

    assert_eq!(res.status(), 200);
    let entries: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(entries.len(), 2);

    app.cleanup().await;
}
