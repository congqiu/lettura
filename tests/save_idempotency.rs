mod common;
use common::TestApp;
use serde_json::json;

async fn register_and_login(app: &TestApp, email: &str) -> String {
    app.client.post(app.url("/api/v1/auth/register"))
        .json(&json!({"username":"u","email":email,"password":"password123"}))
        .send().await.unwrap();
    let login: serde_json::Value = app.client.post(app.url("/api/v1/auth/login"))
        .json(&json!({"email":email,"password":"password123"}))
        .send().await.unwrap().json().await.unwrap();
    login["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn save_same_url_twice_returns_already_existed() {
    let app = TestApp::new().await;
    let jwt = register_and_login(&app, "u@e.com").await;

    let first: serde_json::Value = app.client.post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"url":"https://example.test/post"}))
        .send().await.unwrap().json().await.unwrap();
    let id1 = first["id"].as_str().unwrap().to_string();
    assert_eq!(first["already_existed"], false);
    assert_eq!(first["status"], "queued");

    let second: serde_json::Value = app.client.post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"url":"https://example.test/post"}))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(second["already_existed"], true);
    assert_eq!(second["status"], "existing");
    assert_eq!(second["id"].as_str().unwrap(), id1);
    app.cleanup().await;
}

#[tokio::test]
async fn tags_are_merged_as_union_across_calls() {
    let app = TestApp::new().await;
    let jwt = register_and_login(&app, "u@e.com").await;

    let _first: serde_json::Value = app.client.post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"url":"https://example.test/post", "tag":["a","b"]}))
        .send().await.unwrap().json().await.unwrap();

    let second: serde_json::Value = app.client.post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"url":"https://example.test/post", "tag":["b","c"]}))
        .send().await.unwrap().json().await.unwrap();

    let tags: Vec<String> = second["tags"].as_array().unwrap()
        .iter().map(|v| v.as_str().unwrap().to_string()).collect();
    // Set semantics: should contain a, b, c; no duplicates
    let mut sorted = tags.clone();
    sorted.sort();
    assert_eq!(sorted, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    app.cleanup().await;
}

#[tokio::test]
async fn different_users_with_same_url_get_separate_entries() {
    let app = TestApp::new().await;
    let jwt_a = register_and_login(&app, "a@e.com").await;
    let jwt_b = {
        app.client.post(app.url("/api/v1/auth/register"))
            .json(&json!({"username":"u2","email":"b@e.com","password":"password123"}))
            .send().await.unwrap();
        let r: serde_json::Value = app.client.post(app.url("/api/v1/auth/login"))
            .json(&json!({"email":"b@e.com","password":"password123"}))
            .send().await.unwrap().json().await.unwrap();
        r["access_token"].as_str().unwrap().to_string()
    };

    let a: serde_json::Value = app.client.post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {jwt_a}"))
        .json(&json!({"url":"https://common.test/post"}))
        .send().await.unwrap().json().await.unwrap();
    let b: serde_json::Value = app.client.post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {jwt_b}"))
        .json(&json!({"url":"https://common.test/post"}))
        .send().await.unwrap().json().await.unwrap();

    assert_eq!(a["already_existed"], false);
    assert_eq!(b["already_existed"], false);
    assert_ne!(a["id"], b["id"]);
    app.cleanup().await;
}

#[tokio::test]
async fn save_with_title_on_new_entry_sets_title() {
    let app = TestApp::new().await;
    let jwt = register_and_login(&app, "u@e.com").await;

    let resp: serde_json::Value = app.client.post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({"url":"https://title.test/post", "title":"Custom Title"}))
        .send().await.unwrap().json().await.unwrap();
    let id = resp["id"].as_str().unwrap();

    let entry: serde_json::Value = app.client.get(app.url(&format!("/api/v1/entries/{id}")))
        .header("Authorization", format!("Bearer {jwt}"))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(entry["title"].as_str().unwrap(), "Custom Title");
    app.cleanup().await;
}
