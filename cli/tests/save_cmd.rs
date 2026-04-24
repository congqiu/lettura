use httpmock::prelude::*;
use lettura_cli::cli::SaveArgs;
use lettura_cli::client::ApiClient;
use lettura_cli::commands;

fn save_args(url: &str, wait: bool) -> SaveArgs {
    SaveArgs {
        url: url.into(),
        title: None,
        tag: vec![],
        wait,
    }
}

#[tokio::test]
async fn save_without_wait_posts_and_returns() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v1/entries")
            .json_body_partial(r#"{"url":"https://a.test"}"#);
        then.status(200)
            .body(r#"{"id":"00000000-0000-0000-0000-000000000001","url":"https://a.test","already_existed":false,"tags":[],"status":"queued"}"#);
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let code = commands::save::run(&client, &save_args("https://a.test", false))
        .await
        .unwrap();
    assert_eq!(code, 0);
    m.assert();
}

#[tokio::test]
async fn save_with_wait_polls_until_content_present() {
    let server = MockServer::start();
    // POST returns the queued entry
    let _post = server.mock(|when, then| {
        when.method(POST).path("/api/v1/entries");
        then.status(200).body(r#"{"id":"00000000-0000-0000-0000-000000000001","url":"https://a.test","already_existed":false,"tags":[],"status":"queued"}"#);
    });
    // GET returns entry with content on second poll
    let _get1 = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v1/entries/00000000-0000-0000-0000-000000000001");
        then.status(200).body(r#"{"id":"00000000-0000-0000-0000-000000000001","url":"https://a.test","content":"<p>ready</p>"}"#);
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let code = commands::save::run(&client, &save_args("https://a.test", true))
        .await
        .unwrap();
    assert_eq!(code, 0);
}

#[tokio::test]
async fn save_without_wait_surfaces_already_existed_in_response() {
    // The command emits JSON (stdout) including already_existed=true when server says so;
    // this test only checks the command succeeds and HTTP path was hit.
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST).path("/api/v1/entries");
        then.status(200).body(r#"{"id":"00000000-0000-0000-0000-000000000001","url":"https://a.test","already_existed":true,"tags":["x"],"status":"existing"}"#);
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let code = commands::save::run(&client, &save_args("https://a.test", false))
        .await
        .unwrap();
    assert_eq!(code, 0);
    m.assert();
}
