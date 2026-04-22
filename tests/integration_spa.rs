mod common;

#[tokio::test]
async fn sw_js_served_with_no_cache_and_etag() {
    let app = common::TestApp::new().await;
    let res = app.client.get(app.url("/sw.js")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers()
            .get("cache-control")
            .unwrap()
            .to_str()
            .unwrap(),
        "no-cache"
    );
    assert!(
        res.headers().get("etag").is_some(),
        "sw.js response must include an ETag header"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn sw_js_returns_304_on_if_none_match() {
    let app = common::TestApp::new().await;
    let first = app.client.get(app.url("/sw.js")).send().await.unwrap();
    let etag = first.headers().get("etag").unwrap().to_str().unwrap().to_string();

    let second = app
        .client
        .get(app.url("/sw.js"))
        .header("If-None-Match", &etag)
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), 304);
    assert_eq!(
        second.headers().get("cache-control").unwrap().to_str().unwrap(),
        "no-cache"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn index_html_served_with_no_cache() {
    let app = common::TestApp::new().await;
    let res = app.client.get(app.url("/")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers()
            .get("cache-control")
            .unwrap()
            .to_str()
            .unwrap(),
        "no-cache"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn manifest_served_with_no_cache() {
    let app = common::TestApp::new().await;
    let res = app
        .client
        .get(app.url("/manifest.webmanifest"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers()
            .get("cache-control")
            .unwrap()
            .to_str()
            .unwrap(),
        "no-cache"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn missing_js_returns_404_not_spa_fallback() {
    let app = common::TestApp::new().await;
    let res = app
        .client
        .get(app.url("/definitely-not-here.js"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
    let content_type = res
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    assert!(
        !content_type.contains("text/html"),
        "missing JS must not fall back to HTML"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn spa_route_falls_back_to_index_html() {
    let app = common::TestApp::new().await;
    let res = app
        .client
        .get(app.url("/articles/some-spa-route"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    assert!(
        body.contains("<div id=\"root\">") || body.contains("<div id='root'>"),
        "SPA fallback should return index.html shell"
    );
    app.cleanup().await;
}
