mod common;
use common::TestApp;

#[tokio::test]
async fn me_returns_user_info_with_jwt() {
    let app = TestApp::new().await;
    app.client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"tester","email":"t@e.com","password":"password123"}))
        .send()
        .await
        .unwrap();
    let login: serde_json::Value = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&serde_json::json!({"email":"t@e.com","password":"password123"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let jwt = login["access_token"].as_str().unwrap();

    let resp = app
        .client
        .get(app.url("/api/v1/auth/me"))
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["username"], "tester");
    assert_eq!(body["email"], "t@e.com");
    assert_eq!(body["auth_source"], "jwt");
    app.cleanup().await;
}

#[tokio::test]
async fn me_returns_pat_auth_source_for_pat_auth() {
    let app = TestApp::new().await;
    app.client
        .post(app.url("/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"u","email":"u@e.com","password":"password123"}))
        .send()
        .await
        .unwrap();
    let (user_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email=$1")
        .bind("u@e.com")
        .fetch_one(&app.pool)
        .await
        .unwrap();

    let token = lettura::models::pat::generate_token();
    lettura::models::pat::insert(
        &app.pool,
        user_id,
        "t",
        &lettura::models::pat::hash_token(&token),
        &lettura::models::pat::token_prefix(&token),
        lettura::models::pat::Scope::Write,
        None,
    )
    .await
    .unwrap();

    let resp = app
        .client
        .get(app.url("/api/v1/auth/me"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["auth_source"], "pat");
    app.cleanup().await;
}
