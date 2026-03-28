mod common;

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let app = common::TestApp::new().await;
    let res = app.client.get(app.url("/api/health")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["db"], "ok");
    assert_eq!(body["search"], "ok");
    app.cleanup().await;
}

#[tokio::test]
async fn health_endpoint_no_auth_required() {
    let app = common::TestApp::new().await;
    let res = app.client.get(app.url("/api/health")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    app.cleanup().await;
}
