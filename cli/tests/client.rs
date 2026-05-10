use httpmock::prelude::*;
use lettura_cli::client::ApiClient;
use lettura_cli::error::CliError;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Hello {
    msg: String,
}

#[tokio::test]
async fn get_returns_json_on_200() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(GET).path("/api/v1/thing");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"msg":"hi"}"#);
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let r: Hello = client.get("/api/v1/thing", &[]).await.unwrap();
    assert_eq!(r, Hello { msg: "hi".into() });
}

#[tokio::test]
async fn get_maps_401_to_unauthorized() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(GET).path("/x");
        then.status(401).body("no auth");
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let err = client.get::<Hello>("/x", &[]).await.unwrap_err();
    assert!(matches!(err, CliError::Unauthorized(_)));
}

#[tokio::test]
async fn get_maps_404_to_not_found() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(GET).path("/x");
        then.status(404).body("missing");
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let err = client.get::<Hello>("/x", &[]).await.unwrap_err();
    assert!(matches!(err, CliError::NotFound(_)));
}

#[tokio::test]
async fn get_maps_500_to_server_error() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(GET).path("/x");
        then.status(500).body("oops");
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let err = client.get::<Hello>("/x", &[]).await.unwrap_err();
    assert!(matches!(err, CliError::ServerError(_)));
}

#[tokio::test]
async fn post_sends_json_body() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(POST)
            .path("/save")
            .header("content-type", "application/json")
            .body(r#"{"url":"https://x"}"#);
        then.status(201).body(r#"{"msg":"ok"}"#);
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let r: Hello = client
        .post("/save", &serde_json::json!({"url": "https://x"}))
        .await
        .unwrap();
    assert_eq!(r.msg, "ok");
}

#[tokio::test]
async fn delete_204_returns_default() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(DELETE).path("/x");
        then.status(204);
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let r: serde_json::Value = client.delete("/x").await.unwrap();
    assert_eq!(r, serde_json::Value::Null);
}

#[tokio::test]
async fn network_error_maps_to_network_variant() {
    // unreachable port — use a short-lived bound then close
    let client = ApiClient::new("http://127.0.0.1:1".into(), "lta_x").unwrap();
    let err = client.get::<Hello>("/x", &[]).await.unwrap_err();
    assert!(matches!(err, CliError::Network(_)));
}

#[tokio::test]
async fn bearer_token_header_is_set() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(GET)
            .path("/echo")
            .header("authorization", "Bearer lta_secret123");
        then.status(200).body(r#"{"msg":"ok"}"#);
    });
    let client = ApiClient::new(server.base_url(), "lta_secret123").unwrap();
    let r: Hello = client.get("/echo", &[]).await.unwrap();
    assert_eq!(r.msg, "ok");
}

#[tokio::test]
async fn get_text_returns_raw_body_for_success() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(GET).path("/skill");
        then.status(200)
            .header("content-type", "text/markdown")
            .body("# Skill\n\nhello");
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let text = client.get_text("/skill", &[]).await.unwrap();
    assert!(text.contains("hello"));
}
