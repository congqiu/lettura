mod common;

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let app = common::TestApp::new().await;
    let res = app.client.get(app.url("/api/health")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["db"], "ok");
    assert!(body["search"].as_str().unwrap().starts_with("ok"));
    app.cleanup().await;
}

#[tokio::test]
async fn health_endpoint_no_auth_required() {
    let app = common::TestApp::new().await;
    let res = app.client.get(app.url("/api/health")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    app.cleanup().await;
}

#[tokio::test]
async fn old_api_path_redirects_to_v1() {
    let app = common::TestApp::new().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let res = client.get(app.url("/api/entries")).send().await.unwrap();
    assert_eq!(res.status(), 301);
    assert!(
        res.headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("/api/v1/entries")
    );
    app.cleanup().await;
}

#[tokio::test]
async fn old_api_path_preserves_query_string() {
    let app = common::TestApp::new().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let res = client
        .get(app.url("/api/entries?search=test&page=2"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 301);
    let location = res.headers().get("location").unwrap().to_str().unwrap();
    assert_eq!(location, "/api/v1/entries?search=test&page=2");
    app.cleanup().await;
}

#[tokio::test]
async fn health_not_redirected() {
    let app = common::TestApp::new().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let res = client.get(app.url("/api/health")).send().await.unwrap();
    // /api/health should NOT be redirected, it should respond directly
    assert_eq!(res.status(), 200);
    app.cleanup().await;
}
