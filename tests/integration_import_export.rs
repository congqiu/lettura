mod common;
use serde_json::json;

async fn get_token(app: &common::TestApp) -> String {
    register_user(app, "testuser", "test@example.com").await
}

async fn register_user(app: &common::TestApp, username: &str, email: &str) -> String {
    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&json!({"username": username, "email": email, "password": "password123"}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    body["access_token"]
        .as_str()
        .unwrap_or_else(|| panic!("register failed: {body}"))
        .to_string()
}

#[tokio::test]
async fn import_wallabag_json() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let data = json!([
        {"url": "https://example.com/article1", "title": "Article 1", "is_archived": 1, "is_starred": 0},
        {"url": "https://example.com/article2", "title": "Article 2", "content": "<p>Imported content</p>"},
        {"url": "", "title": "No URL"}
    ]);

    // First import
    let res = app
        .client
        .post(app.url("/api/v1/import/wallabag"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&data)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["imported"], 2);
    assert_eq!(body["skipped"], 1);

    // Re-import same data: already-existing URLs should be skipped
    let res = app
        .client
        .post(app.url("/api/v1/import/wallabag"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&data)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["imported"], 0);
    assert_eq!(body["skipped"], 3);

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

    // First import
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
    assert_eq!(body["skipped"], 0);

    // Re-import same bookmarks: already-existing URLs should be skipped
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
    assert_eq!(body["imported"], 0);
    assert_eq!(body["skipped"], 2);

    app.cleanup().await;
}

#[tokio::test]
async fn import_lettura_backup() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    // First, create some data and export it
    let res1 = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/lettura-import-test"}))
        .send()
        .await
        .unwrap();
    let entry: serde_json::Value = res1.json().await.unwrap();

    // Add a tag
    app.client
        .post(app.url(&format!(
            "/api/v1/entries/{}/tags",
            entry["id"].as_str().unwrap()
        )))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"label": "migration-tag"}))
        .send()
        .await
        .unwrap();

    // Export
    let res = app
        .client
        .get(app.url("/api/v1/export"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let export_data: serde_json::Value = res.json().await.unwrap();

    // Import the exported data into the same account (should skip existing URLs)
    let res = app
        .client
        .post(app.url("/api/v1/import/lettura"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&export_data)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["imported"], 0);
    assert_eq!(body["skipped"], 1);

    app.cleanup().await;
}

#[tokio::test]
async fn import_lettura_cross_account_migration() {
    let app = common::TestApp::new().await;

    // --- Account A: build a dataset and export ---
    let token_a = register_user(&app, "alice", "alice@example.com").await;

    let entry_a: serde_json::Value = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token_a}"))
        .json(&json!({"url": "https://example.com/cross-account-article"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let entry_a_id = entry_a["id"].as_str().unwrap().to_string();

    // Tag
    app.client
        .post(app.url(&format!("/api/v1/entries/{entry_a_id}/tags")))
        .header("Authorization", format!("Bearer {token_a}"))
        .json(&json!({"label": "migrated"}))
        .send()
        .await
        .unwrap();

    // Annotation
    app.client
        .post(app.url(&format!("/api/v1/entries/{entry_a_id}/annotations")))
        .header("Authorization", format!("Bearer {token_a}"))
        .json(&json!({"quote": "important quote", "text": "alice note", "ranges": []}))
        .send()
        .await
        .unwrap();

    // Memo
    app.client
        .post(app.url("/api/v1/memos"))
        .header("Authorization", format!("Bearer {token_a}"))
        .json(&json!({"content": "alice memo"}))
        .send()
        .await
        .unwrap();

    // Export from A
    let export_data: serde_json::Value = app
        .client
        .get(app.url("/api/v1/export"))
        .header("Authorization", format!("Bearer {token_a}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(export_data["entries"].as_array().unwrap().len(), 1);
    assert_eq!(export_data["tags"].as_array().unwrap().len(), 1);
    assert_eq!(export_data["annotations"].as_array().unwrap().len(), 1);
    assert_eq!(export_data["memos"].as_array().unwrap().len(), 1);

    // --- Account B: receive A's backup ---
    let token_b = register_user(&app, "bob", "bob@example.com").await;

    let import_res = app
        .client
        .post(app.url("/api/v1/import/lettura"))
        .header("Authorization", format!("Bearer {token_b}"))
        .json(&export_data)
        .send()
        .await
        .unwrap();
    assert_eq!(import_res.status(), 200);
    let import_body: serde_json::Value = import_res.json().await.unwrap();
    assert_eq!(import_body["imported"], 1);
    assert_eq!(import_body["skipped"], 0);
    assert_eq!(import_body["failed_entries"], 0);
    assert_eq!(import_body["failed_annotations"], 0);
    assert_eq!(import_body["failed_memos"], 0);

    // --- Verify B now owns the data ---
    let entries_b: serde_json::Value = app
        .client
        .get(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token_b}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let entries_b = entries_b.as_array().unwrap();
    assert_eq!(entries_b.len(), 1);
    assert_eq!(
        entries_b[0]["url"],
        "https://example.com/cross-account-article"
    );
    let entry_b_id = entries_b[0]["id"].as_str().unwrap().to_string();
    // New account must own a freshly minted entry, NOT reuse A's id
    assert_ne!(entry_b_id, entry_a_id);
    // Tag link migrated to B's new entry
    let tags_b = entries_b[0]["tags"].as_array().unwrap();
    assert_eq!(tags_b.len(), 1);
    assert_eq!(tags_b[0]["label"], "migrated");

    let annotations_b: serde_json::Value = app
        .client
        .get(app.url(&format!("/api/v1/entries/{entry_b_id}/annotations")))
        .header("Authorization", format!("Bearer {token_b}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let annotations_b = annotations_b.as_array().unwrap();
    assert_eq!(annotations_b.len(), 1);
    assert_eq!(annotations_b[0]["quote"], "important quote");
    assert_eq!(annotations_b[0]["text"], "alice note");
    // Imported annotation must NOT keep A's original UUID (preserving the
    // source id would collide with A's row on a multi-tenant instance).
    assert_ne!(
        annotations_b[0]["id"],
        export_data["annotations"][0]["id"],
        "annotation id should be regenerated, not copied from the source"
    );

    let memos_b: serde_json::Value = app
        .client
        .get(app.url("/api/v1/memos"))
        .header("Authorization", format!("Bearer {token_b}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let memos_b = memos_b.as_array().unwrap();
    assert_eq!(memos_b.len(), 1);
    assert_eq!(memos_b[0]["content"], "alice memo");

    // --- Sanity: account A still owns its original copy untouched ---
    let entries_a: serde_json::Value = app
        .client
        .get(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {token_a}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(entries_a.as_array().unwrap().len(), 1);

    app.cleanup().await;
}

#[tokio::test]
async fn import_lettura_wrong_version() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let data = json!({
        "version": "2.0",
        "entries": [],
        "tags": [],
        "entry_tags": [],
        "annotations": [],
        "memos": [],
        "tagging_rules": [],
        "site_rules": []
    });

    let res = app
        .client
        .post(app.url("/api/v1/import/lettura"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&data)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 400);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"], "bad_request");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("unsupported export version")
    );

    app.cleanup().await;
}

#[tokio::test]
async fn export_all_entries() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    // Create entries
    let res1 = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/exported"}))
        .send()
        .await
        .unwrap();
    let entry1: serde_json::Value = res1.json().await.unwrap();

    let res2 = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/archived"}))
        .send()
        .await
        .unwrap();
    let entry2: serde_json::Value = res2.json().await.unwrap();

    // Archive one entry
    app.client
        .patch(app.url(&format!(
            "/api/v1/entries/{}",
            entry2["id"].as_str().unwrap()
        )))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"is_archived": true}))
        .send()
        .await
        .unwrap();

    // Add a tag
    app.client
        .post(app.url(&format!(
            "/api/v1/entries/{}/tags",
            entry1["id"].as_str().unwrap()
        )))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"label": "test-tag"}))
        .send()
        .await
        .unwrap();

    // Add an annotation
    app.client
        .post(app.url(&format!(
            "/api/v1/entries/{}/annotations",
            entry1["id"].as_str().unwrap()
        )))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"quote": "hello", "text": "note", "ranges": []}))
        .send()
        .await
        .unwrap();

    // Add a memo
    app.client
        .post(app.url("/api/v1/memos"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"content": "test memo"}))
        .send()
        .await
        .unwrap();

    // Export all
    let res = app
        .client
        .get(app.url("/api/v1/export"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["version"], "1.0");
    assert_eq!(body["scope"], "all");
    assert!(body["exported_at"].is_string());
    assert!(body["entries"].is_array());
    assert_eq!(body["entries"].as_array().unwrap().len(), 2);
    assert!(body["tags"].is_array());
    assert_eq!(body["tags"].as_array().unwrap().len(), 1);
    assert!(body["entry_tags"].is_array());
    assert_eq!(body["entry_tags"].as_array().unwrap().len(), 1);
    assert!(body["annotations"].is_array());
    assert_eq!(body["annotations"].as_array().unwrap().len(), 1);
    assert!(body["memos"].is_array());
    assert_eq!(body["memos"].as_array().unwrap().len(), 1);
    assert!(body["tagging_rules"].is_array());
    assert!(body["site_rules"].is_array());

    app.cleanup().await;
}

#[tokio::test]
async fn export_with_scope() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    // Create two entries
    let res1 = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/unread"}))
        .send()
        .await
        .unwrap();
    let entry1: serde_json::Value = res1.json().await.unwrap();

    let res2 = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"url": "https://example.com/archived"}))
        .send()
        .await
        .unwrap();
    let entry2: serde_json::Value = res2.json().await.unwrap();

    // Archive entry2
    app.client
        .patch(app.url(&format!(
            "/api/v1/entries/{}",
            entry2["id"].as_str().unwrap()
        )))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"is_archived": true}))
        .send()
        .await
        .unwrap();

    // Star entry1
    app.client
        .patch(app.url(&format!(
            "/api/v1/entries/{}",
            entry1["id"].as_str().unwrap()
        )))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"is_starred": true}))
        .send()
        .await
        .unwrap();

    // Test unread scope
    let res = app
        .client
        .get(app.url("/api/v1/export?scope=unread"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["scope"], "unread");
    assert_eq!(body["entries"].as_array().unwrap().len(), 1);
    assert_eq!(body["entries"][0]["url"], "https://example.com/unread");

    // Test archived scope
    let res = app
        .client
        .get(app.url("/api/v1/export?scope=archived"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["scope"], "archived");
    assert_eq!(body["entries"].as_array().unwrap().len(), 1);
    assert_eq!(body["entries"][0]["url"], "https://example.com/archived");

    // Test starred scope
    let res = app
        .client
        .get(app.url("/api/v1/export?scope=starred"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["scope"], "starred");
    assert_eq!(body["entries"].as_array().unwrap().len(), 1);
    assert_eq!(body["entries"][0]["url"], "https://example.com/unread");

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
