mod common;
use common::TestApp;

#[tokio::test]
async fn skill_endpoint_returns_markdown() {
    let app = TestApp::new().await;
    let resp = app
        .client
        .get(app.url("/skills/lettura.md"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        ct.starts_with("text/markdown"),
        "content-type was: {ct}"
    );
    let body = resp.text().await.unwrap();
    assert!(body.contains("# Lettura CLI"));
    app.cleanup().await;
}

#[tokio::test]
async fn skill_endpoint_substitutes_server_version() {
    let app = TestApp::new().await;
    let resp = app
        .client
        .get(app.url("/skills/lettura.md"))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("{{SERVER_VERSION}}"),
        "placeholder should be rendered"
    );
    // skills/lettura.md includes 'Version: {{SERVER_VERSION}}'; the rendered value should appear
    assert!(
        body.contains("Version: "),
        "Version: line should be present"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn skill_endpoint_is_public_no_auth_required() {
    // No Authorization header; should still succeed.
    let app = TestApp::new().await;
    let resp = app
        .client
        .get(app.url("/skills/lettura.md"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    app.cleanup().await;
}
