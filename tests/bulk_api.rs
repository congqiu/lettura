mod common;
use common::TestApp;
use serde_json::json;

async fn login_and_create_entries(app: &TestApp, urls: &[&str]) -> (String, Vec<String>) {
    app.client
        .post(app.url("/api/v1/auth/register"))
        .json(&json!({"username":"u","email":"u@e.com","password":"password123"}))
        .send()
        .await
        .unwrap();
    let login: serde_json::Value = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&json!({"email":"u@e.com","password":"password123"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let jwt = login["access_token"].as_str().unwrap().to_string();

    let mut ids = vec![];
    for url in urls {
        let r: serde_json::Value = app
            .client
            .post(app.url("/api/v1/entries"))
            .header("Authorization", format!("Bearer {jwt}"))
            .json(&json!({"url": url}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        ids.push(r["id"].as_str().unwrap().to_string());
    }
    (jwt, ids)
}

#[tokio::test]
async fn bulk_tag_dry_run_reports_matched_without_changes() {
    let app = TestApp::new().await;
    let (jwt, _ids) = login_and_create_entries(&app, &["https://a.test", "https://b.test"]).await;

    let r: serde_json::Value = app
        .client
        .post(app.url("/api/v1/entries/bulk/tag"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({
            "filter": {"untagged": true},
            "add": ["auto"],
            "dry_run": true,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r["matched"].as_i64().unwrap(), 2);
    assert_eq!(r["updated"].as_i64().unwrap(), 0);
    app.cleanup().await;
}

#[tokio::test]
async fn bulk_tag_applied() {
    let app = TestApp::new().await;
    let (jwt, _ids) = login_and_create_entries(&app, &["https://a.test", "https://b.test"]).await;

    let _: serde_json::Value = app
        .client
        .post(app.url("/api/v1/entries/bulk/tag"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"filter": {"untagged": true}, "add": ["auto"]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let entries: Vec<serde_json::Value> = app
        .client
        .get(app.url("/api/v1/entries?tag=auto"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(entries.len(), 2);
    app.cleanup().await;
}

#[tokio::test]
async fn bulk_archive_sets_is_archived() {
    let app = TestApp::new().await;
    let (jwt, _ids) = login_and_create_entries(&app, &["https://a.test", "https://b.test"]).await;

    let r: serde_json::Value = app
        .client
        .post(app.url("/api/v1/entries/bulk/archive"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"filter": {"is_archived": false}, "value": true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r["updated"].as_i64().unwrap(), 2);
    app.cleanup().await;
}

#[tokio::test]
async fn bulk_add_empty_returns_400() {
    let app = TestApp::new().await;
    let (jwt, _ids) = login_and_create_entries(&app, &["https://a.test"]).await;
    let r = app
        .client
        .post(app.url("/api/v1/entries/bulk/tag"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"filter": {}, "add": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    app.cleanup().await;
}

#[tokio::test]
async fn bulk_max_rejects_when_exceeded() {
    let app = TestApp::new().await;
    let (jwt, _ids) = login_and_create_entries(
        &app,
        &["https://a.test", "https://b.test", "https://c.test"],
    )
    .await;
    let r = app
        .client
        .post(app.url("/api/v1/entries/bulk/tag"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"filter": {}, "add": ["x"], "max": 2}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    app.cleanup().await;
}

#[tokio::test]
async fn bulk_tag_applies_all_labels_to_all_matched_entries() {
    let app = TestApp::new().await;
    let (jwt, _ids) = login_and_create_entries(
        &app,
        &["https://a.test", "https://b.test", "https://c.test"],
    )
    .await;

    let r: serde_json::Value = app
        .client
        .post(app.url("/api/v1/entries/bulk/tag"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"filter": {}, "add": ["rust", "tokio"]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r["matched"].as_i64().unwrap(), 3);
    assert_eq!(r["updated"].as_i64().unwrap(), 3);

    // Both labels should be applied to all 3 entries
    let with_rust: Vec<serde_json::Value> = app
        .client
        .get(app.url("/api/v1/entries?tag=rust"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(with_rust.len(), 3);

    let with_tokio: Vec<serde_json::Value> = app
        .client
        .get(app.url("/api/v1/entries?tag=tokio"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(with_tokio.len(), 3);

    // Only 2 unique tags should exist
    let all_tags: Vec<serde_json::Value> = app
        .client
        .get(app.url("/api/v1/tags"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(all_tags.len(), 2);

    app.cleanup().await;
}
