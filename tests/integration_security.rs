mod common;

#[tokio::test]
async fn responses_include_security_headers() {
    let app = common::TestApp::new().await;
    // POST to a real endpoint to get a response with headers
    let res = app
        .client
        .post(app.url("/api/v1/auth/login"))
        .json(&serde_json::json!({"email": "x@x.com", "password": "wrong"}))
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert_eq!(res.headers().get("x-frame-options").unwrap(), "DENY");
    assert_eq!(
        res.headers().get("referrer-policy").unwrap(),
        "strict-origin-when-cross-origin"
    );
    app.cleanup().await;
}
