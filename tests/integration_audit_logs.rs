mod common;
use common::TestApp;

#[tokio::test]
async fn audit_log_insert_works_directly() {
    let app = TestApp::new().await;

    app.client.post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"direct","email":"direct@e.com","password":"password123"}))
        .send().await.unwrap();
    let login: serde_json::Value = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&serde_json::json!({"email":"direct@e.com","password":"password123"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let _jwt = login["access_token"].as_str().unwrap();

    let (user_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email=$1")
        .bind("direct@e.com")
        .fetch_one(&app.pool)
        .await
        .unwrap();

    let log = lettura::models::audit_log::insert(
        &app.pool,
        lettura::models::audit_log::InsertAuditLog {
            user_id: Some(user_id),
            auth_source: "jwt".to_string(),
            action: lettura::models::audit_log::AuditAction::CreateEntry,
            resource_type: Some(lettura::models::audit_log::AuditResourceType::Entry),
            resource_id: Some(user_id),
            status: "success".to_string(),
            details: serde_json::json!({"test": true}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        log.action,
        lettura::models::audit_log::AuditAction::CreateEntry
    );
    assert_eq!(log.status, "success");

    app.cleanup().await;
}

#[tokio::test]
async fn saving_entry_creates_audit_log() {
    let app = TestApp::new().await;

    // Register and login
    app.client.post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"auditor","email":"audit@e.com","password":"password123"}))
        .send().await.unwrap();
    let login: serde_json::Value = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&serde_json::json!({"email":"audit@e.com","password":"password123"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let jwt = login["access_token"].as_str().unwrap();

    // Create an entry
    let resp = app
        .client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&serde_json::json!({"url":"https://example.com/article"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Query audit logs
    let resp = app
        .client
        .get(app.url("/api/v1/audit-logs"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let logs = body["data"].as_array().unwrap();
    assert!(
        !logs.is_empty(),
        "audit logs should not be empty after creating entry"
    );

    let log = &logs[0];
    assert_eq!(log["action"], "create_entry");
    assert_eq!(log["status"], "success");
    assert_eq!(log["auth_source"], "jwt");
    assert!(log["details"]["after"]["url"].as_str().is_some());

    app.cleanup().await;
}

#[tokio::test]
async fn audit_logs_support_filtering_by_action() {
    let app = TestApp::new().await;

    app.client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"f","email":"f@e.com","password":"password123"}))
        .send()
        .await
        .unwrap();
    let login: serde_json::Value = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&serde_json::json!({"email":"f@e.com","password":"password123"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let jwt = login["access_token"].as_str().unwrap();

    // Create two entries
    for _ in 0..2 {
        app.client
            .post(app.url("/api/v1/entries"))
            .header("Authorization", format!("Bearer {jwt}"))
            .json(&serde_json::json!({"url":"https://example.com/article"}))
            .send()
            .await
            .unwrap();
    }

    // Filter by action
    let resp = app
        .client
        .get(app.url("/api/v1/audit-logs?action=create_entry"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["data"].as_array().unwrap().len(), 2);
    assert_eq!(body["total"], 2);

    app.cleanup().await;
}

#[tokio::test]
async fn audit_logs_pagination_works() {
    let app = TestApp::new().await;

    app.client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"p","email":"p@e.com","password":"password123"}))
        .send()
        .await
        .unwrap();
    let login: serde_json::Value = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&serde_json::json!({"email":"p@e.com","password":"password123"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let jwt = login["access_token"].as_str().unwrap();

    // Create 3 entries
    for i in 0..3 {
        app.client
            .post(app.url("/api/v1/entries"))
            .header("Authorization", format!("Bearer {jwt}"))
            .json(&serde_json::json!({"url": format!("https://example.com/{}", i)}))
            .send()
            .await
            .unwrap();
    }

    // Limit 2, filtered by create_entry so register/login don't count
    let resp = app
        .client
        .get(app.url("/api/v1/audit-logs?limit=2&offset=0&action=create_entry"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["data"].as_array().unwrap().len(), 2);
    assert_eq!(body["total"], 3);
    assert_eq!(body["limit"], 2);
    assert_eq!(body["offset"], 0);

    app.cleanup().await;
}

#[tokio::test]
async fn audit_logs_are_isolated_per_user() {
    let app = TestApp::new().await;

    // User A
    app.client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"ua","email":"ua@e.com","password":"password123"}))
        .send()
        .await
        .unwrap();
    let login_a: serde_json::Value = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&serde_json::json!({"email":"ua@e.com","password":"password123"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let jwt_a = login_a["access_token"].as_str().unwrap();

    // User B
    app.client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"ub","email":"ub@e.com","password":"password123"}))
        .send()
        .await
        .unwrap();
    let login_b: serde_json::Value = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&serde_json::json!({"email":"ub@e.com","password":"password123"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let jwt_b = login_b["access_token"].as_str().unwrap();

    // User A creates entry
    app.client
        .post(app.url("/api/v1/entries"))
        .header("Authorization", format!("Bearer {jwt_a}"))
        .json(&serde_json::json!({"url":"https://example.com/a"}))
        .send()
        .await
        .unwrap();

    // User B checks create_entry logs - should be empty (only User A created entries)
    let resp = app
        .client
        .get(app.url("/api/v1/audit-logs?action=create_entry"))
        .header("Authorization", format!("Bearer {jwt_b}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["data"].as_array().unwrap().is_empty(),
        "user B should have no create_entry logs"
    );

    app.cleanup().await;
}
