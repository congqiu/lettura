mod common;
use serde_json::json;

async fn get_token(app: &common::TestApp) -> String {
    let res = app.client.post(app.url("/api/v1/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

// --- Tagging Rules ---

#[tokio::test]
async fn create_and_list_tagging_rules() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let res = app.client.post(app.url("/api/v1/tagging-rules"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "rule": {"operator": "AND", "conditions": [{"field": "domainName", "op": "eq", "value": "github.com"}]},
            "tags": ["github", "code"]
        }))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let rule: serde_json::Value = res.json().await.unwrap();
    assert_eq!(rule["tags"], json!(["github", "code"]));

    let res = app.client.get(app.url("/api/v1/tagging-rules"))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    let rules: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(rules.len(), 1);

    app.cleanup().await;
}

#[tokio::test]
async fn update_tagging_rule() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let res = app.client.post(app.url("/api/v1/tagging-rules"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "rule": {"operator": "AND", "conditions": []},
            "tags": ["old"]
        }))
        .send().await.unwrap();
    let rule: serde_json::Value = res.json().await.unwrap();
    let rule_id = rule["id"].as_str().unwrap();

    let res = app.client.patch(app.url(&format!("/api/v1/tagging-rules/{}", rule_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"tags": ["new", "updated"]}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let updated: serde_json::Value = res.json().await.unwrap();
    assert_eq!(updated["tags"], json!(["new", "updated"]));

    app.cleanup().await;
}

#[tokio::test]
async fn delete_tagging_rule() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let res = app.client.post(app.url("/api/v1/tagging-rules"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"rule": {"operator": "AND", "conditions": []}, "tags": ["x"]}))
        .send().await.unwrap();
    let rule: serde_json::Value = res.json().await.unwrap();
    let rule_id = rule["id"].as_str().unwrap();

    let res = app.client.delete(app.url(&format!("/api/v1/tagging-rules/{}", rule_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);

    app.cleanup().await;
}

// --- Site Rules ---

#[tokio::test]
async fn create_and_list_site_rules() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let res = app.client.post(app.url("/api/v1/site-rules"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "domain": "example.com",
            "content_selector": "article.main",
            "title_selector": "h1.title",
            "strip_selectors": [".ads", ".sidebar"]
        }))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let rule: serde_json::Value = res.json().await.unwrap();
    assert_eq!(rule["domain"], "example.com");
    assert_eq!(rule["content_selector"], "article.main");

    let res = app.client.get(app.url("/api/v1/site-rules"))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    let rules: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(rules.len(), 1);

    app.cleanup().await;
}

#[tokio::test]
async fn update_site_rule() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let res = app.client.post(app.url("/api/v1/site-rules"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"domain": "test.com", "content_selector": ".old"}))
        .send().await.unwrap();
    let rule: serde_json::Value = res.json().await.unwrap();
    let rule_id = rule["id"].as_str().unwrap();

    let res = app.client.patch(app.url(&format!("/api/v1/site-rules/{}", rule_id)))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"content_selector": ".new-content"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let updated: serde_json::Value = res.json().await.unwrap();
    assert_eq!(updated["content_selector"], ".new-content");

    app.cleanup().await;
}

#[tokio::test]
async fn delete_site_rule() {
    let app = common::TestApp::new().await;
    let token = get_token(&app).await;

    let res = app.client.post(app.url("/api/v1/site-rules"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"domain": "del.com", "content_selector": ".x"}))
        .send().await.unwrap();
    let rule: serde_json::Value = res.json().await.unwrap();
    let rule_id = rule["id"].as_str().unwrap();

    let res = app.client.delete(app.url(&format!("/api/v1/site-rules/{}", rule_id)))
        .header("Authorization", format!("Bearer {}", token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);

    app.cleanup().await;
}
