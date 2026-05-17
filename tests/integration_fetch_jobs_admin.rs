mod common;

use serde_json::json;

/// Register the very first user — by `src/api/auth.rs` they are promoted to
/// admin automatically — and return their JWT access token.
async fn admin_token(app: &common::TestApp) -> String {
    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&json!({
            "username": "admin",
            "email": "a@x.test",
            "password": "password123",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "register should succeed");
    let body: serde_json::Value = res.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

async fn admin_uid(app: &common::TestApp) -> uuid::Uuid {
    sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM users WHERE username='admin'")
        .fetch_one(&app.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn admin_list_dead_jobs() {
    let app = common::TestApp::new().await;
    let token = admin_token(&app).await;
    let uid = admin_uid(&app).await;

    let entry_id = app.create_entry(uid, "https://x.test/").await;
    let id = lettura::models::fetch_job::enqueue(
        &app.pool,
        entry_id,
        uid,
        "https://x.test/",
        0,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE fetch_jobs SET status='dead', last_error='boom' WHERE id=$1")
        .bind(id)
        .execute(&app.pool)
        .await
        .unwrap();

    let res = app
        .client
        .get(app.url("/api/v1/admin/fetch-jobs?status=dead"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["last_error"], "boom");
    // FetchJobStatus carries #[serde(rename_all = "lowercase")] so JSON
    // responses match the lowercase query-parameter contract used by the
    // admin endpoints and the Prometheus label table in main.rs.
    assert_eq!(items[0]["status"], "dead");

    app.cleanup().await;
}

#[tokio::test]
async fn admin_retry_dead_resets_to_pending() {
    let app = common::TestApp::new().await;
    let token = admin_token(&app).await;
    let uid = admin_uid(&app).await;

    let entry_id = app.create_entry(uid, "https://x.test/").await;
    let id = lettura::models::fetch_job::enqueue(
        &app.pool,
        entry_id,
        uid,
        "https://x.test/",
        0,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE fetch_jobs SET status='dead', attempts=5 WHERE id=$1")
        .bind(id)
        .execute(&app.pool)
        .await
        .unwrap();

    let res = app
        .client
        .post(app.url(&format!("/api/v1/admin/fetch-jobs/{id}/retry")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);

    let row = lettura::models::fetch_job::find_by_id(&app.pool, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.status, lettura::models::fetch_job::FetchJobStatus::Pending);
    assert_eq!(row.attempts, 0);
    assert!(row.last_error.is_none());

    app.cleanup().await;
}

#[tokio::test]
async fn retry_all_dead_capped_at_100() {
    let app = common::TestApp::new().await;
    let token = admin_token(&app).await;
    let uid = admin_uid(&app).await;

    // Create 150 dead jobs. The endpoint should cap retries at 100.
    for i in 0..150 {
        let url = format!("https://x.test/{i}");
        let eid = app.create_entry(uid, &url).await;
        let id = lettura::models::fetch_job::enqueue(&app.pool, eid, uid, &url, 0)
            .await
            .unwrap();
        sqlx::query("UPDATE fetch_jobs SET status='dead' WHERE id=$1")
            .bind(id)
            .execute(&app.pool)
            .await
            .unwrap();
    }

    let res = app
        .client
        .post(app.url("/api/v1/admin/fetch-jobs/retry-all-dead"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["retried"], 100);
    assert_eq!(body["remaining_dead"], 50);

    app.cleanup().await;
}

#[tokio::test]
async fn non_admin_forbidden() {
    let app = common::TestApp::new().await;
    // First user is auto-admin; register them but don't keep the token.
    let _ = admin_token(&app).await;

    // Second user is a normal account.
    let res = app
        .client
        .post(app.url("/api/v1/auth/register"))
        .json(&json!({
            "username": "normie",
            "email": "n@x.test",
            "password": "password123",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let token = body["access_token"].as_str().unwrap();

    let res = app
        .client
        .get(app.url("/api/v1/admin/fetch-jobs"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
    let body: serde_json::Value = res.json().await.unwrap();
    let msg = body["message"].as_str().unwrap();
    assert!(msg.contains("PAT"), "message should explain PAT limitation: {msg}");

    app.cleanup().await;
}
