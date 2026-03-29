mod common;
use serde_json::json;

async fn get_auth_token(app: &common::TestApp) -> String {
    let res = app.client.post(app.url("/api/v1/auth/register"))
        .json(&json!({"username":"testuser","email":"test@example.com","password":"password123"}))
        .send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

fn auth_header(token: &str) -> String {
    format!("Bearer {}", token)
}

#[tokio::test]
async fn upload_and_create_page() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    let html_content = "<html><head><title>Test Page</title></head><body>Hello</body></html>";
    let res = app.client.post(app.url("/api/v1/pages/upload"))
        .header("Authorization", auth_header(&token))
        .multipart(reqwest::multipart::Form::new()
            .part("files", reqwest::multipart::Part::text(html_content)
                .file_name("index.html")
                .mime_str("text/html").unwrap()))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let upload: serde_json::Value = res.json().await.unwrap();
    assert_eq!(upload["html_files"].as_array().unwrap().len(), 1);
    assert_eq!(upload["default_entry"], "index.html");
    assert_eq!(upload["suggested_title"], "Test Page");

    let upload_id = upload["upload_id"].as_str().unwrap();
    let res = app.client.post(app.url("/api/v1/pages"))
        .header("Authorization", auth_header(&token))
        .json(&json!({
            "upload_id": upload_id,
            "entry_file": "index.html",
            "title": "Test Page",
        }))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let page: serde_json::Value = res.json().await.unwrap();
    assert!(page["slug"].is_string());
    assert_eq!(page["title"], "Test Page");

    app.cleanup().await;
}

#[tokio::test]
async fn list_pages() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    let res = app.client.get(app.url("/api/v1/pages"))
        .header("Authorization", auth_header(&token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["total"], 0);

    app.cleanup().await;
}

#[tokio::test]
async fn public_access_page() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    let html_content = "<html><head><title>Public</title></head><body>Public content</body></html>";
    let upload_res = app.client.post(app.url("/api/v1/pages/upload"))
        .header("Authorization", auth_header(&token))
        .multipart(reqwest::multipart::Form::new()
            .part("files", reqwest::multipart::Part::text(html_content)
                .file_name("index.html").mime_str("text/html").unwrap()))
        .send().await.unwrap();
    let upload: serde_json::Value = upload_res.json().await.unwrap();

    let create_res = app.client.post(app.url("/api/v1/pages"))
        .header("Authorization", auth_header(&token))
        .json(&json!({"upload_id": upload["upload_id"], "entry_file": "index.html", "title": "Public"}))
        .send().await.unwrap();
    let page: serde_json::Value = create_res.json().await.unwrap();
    let slug = page["slug"].as_str().unwrap();

    let res = app.client.get(app.url(&format!("/p/{}", slug)))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    assert!(body.contains("Public content"));

    app.cleanup().await;
}

#[tokio::test]
async fn password_protected_page() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    let html_content = "<html><head><title>Secret</title></head><body>Secret content</body></html>";
    let upload_res = app.client.post(app.url("/api/v1/pages/upload"))
        .header("Authorization", auth_header(&token))
        .multipart(reqwest::multipart::Form::new()
            .part("files", reqwest::multipart::Part::text(html_content)
                .file_name("index.html").mime_str("text/html").unwrap()))
        .send().await.unwrap();
    let upload: serde_json::Value = upload_res.json().await.unwrap();

    let create_res = app.client.post(app.url("/api/v1/pages"))
        .header("Authorization", auth_header(&token))
        .json(&json!({
            "upload_id": upload["upload_id"],
            "entry_file": "index.html",
            "title": "Secret",
            "password": "mypass123"
        }))
        .send().await.unwrap();
    assert_eq!(create_res.status(), 200);
    let page: serde_json::Value = create_res.json().await.unwrap();
    let slug = page["slug"].as_str().unwrap();

    let res = app.client.get(app.url(&format!("/p/{}", slug)))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    assert!(body.contains("需要密码"));
    assert!(!body.contains("Secret content"));

    let res = app.client.post(app.url(&format!("/p/{}/auth", slug)))
        .form(&json!({"password": "wrong"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    assert!(body.contains("密码错误"));

    let res = app.client.post(app.url(&format!("/p/{}/auth", slug)))
        .form(&json!({"password": "mypass123"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 302);

    app.cleanup().await;
}

#[tokio::test]
async fn update_and_delete_page() {
    let app = common::TestApp::new().await;
    let token = get_auth_token(&app).await;

    let html_content = "<html><body>Test</body></html>";
    let upload_res = app.client.post(app.url("/api/v1/pages/upload"))
        .header("Authorization", auth_header(&token))
        .multipart(reqwest::multipart::Form::new()
            .part("files", reqwest::multipart::Part::text(html_content)
                .file_name("index.html").mime_str("text/html").unwrap()))
        .send().await.unwrap();
    let upload: serde_json::Value = upload_res.json().await.unwrap();

    let create_res = app.client.post(app.url("/api/v1/pages"))
        .header("Authorization", auth_header(&token))
        .json(&json!({"upload_id": upload["upload_id"], "entry_file": "index.html", "title": "Original"}))
        .send().await.unwrap();
    let page: serde_json::Value = create_res.json().await.unwrap();
    let page_id = page["id"].as_str().unwrap();

    let res = app.client.patch(app.url(&format!("/api/v1/pages/{}", page_id)))
        .header("Authorization", auth_header(&token))
        .json(&json!({"title": "Updated"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let updated: serde_json::Value = res.json().await.unwrap();
    assert_eq!(updated["title"], "Updated");

    let res = app.client.delete(app.url(&format!("/api/v1/pages/{}", page_id)))
        .header("Authorization", auth_header(&token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);

    let res = app.client.get(app.url("/api/v1/pages?status=deleted"))
        .header("Authorization", auth_header(&token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["total"], 1);

    let res = app.client.post(app.url(&format!("/api/v1/pages/{}/restore", page_id)))
        .header("Authorization", auth_header(&token))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);

    app.cleanup().await;
}

#[tokio::test]
async fn upload_requires_auth() {
    let app = common::TestApp::new().await;
    let res = app.client.post(app.url("/api/v1/pages/upload"))
        .send().await.unwrap();
    assert_eq!(res.status(), 401);
    app.cleanup().await;
}
