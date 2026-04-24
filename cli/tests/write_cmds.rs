use httpmock::prelude::*;
use httpmock::Method::PATCH;
use httpmock::Method::POST;
use lettura_cli::cli::{StateChangeArgs, TagArgs, UntagArgs};
use lettura_cli::client::ApiClient;
use lettura_cli::commands;

#[tokio::test]
async fn tag_add_posts_to_entry_tags() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v1/entries/abc/tags")
            .json_body_partial(r#"{"label":"rust"}"#);
        then.status(200).body(r#"{"id":"tag","label":"rust"}"#);
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let args = TagArgs {
        id: Some("abc".into()),
        names: vec!["rust".into()],
        add: vec![],
        filter: None,
        dry_run: false,
        yes: false,
    };
    let code = commands::tag::run_tag(&client, &args).await.unwrap();
    assert_eq!(code, 0);
    m.assert();
}

#[tokio::test]
async fn untag_deletes_by_label() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(DELETE).path("/api/v1/entries/abc/tags/by-label/rust");
        then.status(204);
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let args = UntagArgs {
        id: Some("abc".into()),
        names: vec!["rust".into()],
        remove: vec![],
        filter: None,
        dry_run: false,
        yes: false,
    };
    let code = commands::tag::run_untag(&client, &args).await.unwrap();
    assert_eq!(code, 0);
    m.assert();
}

#[tokio::test]
async fn archive_patches_is_archived_true() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(PATCH)
            .path("/api/v1/entries/abc")
            .json_body_partial(r#"{"is_archived":true}"#);
        then.status(200).body(r#"{"id":"abc","is_archived":true}"#);
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let args = StateChangeArgs {
        id: Some("abc".into()),
        filter: None,
        dry_run: false,
        yes: false,
    };
    let code = commands::state::run_archive(&client, &args).await.unwrap();
    assert_eq!(code, 0);
    m.assert();
}

#[tokio::test]
async fn star_patches_is_starred_true() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(PATCH)
            .path("/api/v1/entries/abc")
            .json_body_partial(r#"{"is_starred":true}"#);
        then.status(200).body(r#"{"id":"abc","is_starred":true}"#);
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let args = StateChangeArgs {
        id: Some("abc".into()),
        filter: None,
        dry_run: false,
        yes: false,
    };
    let code = commands::state::run_star(&client, &args).await.unwrap();
    assert_eq!(code, 0);
    m.assert();
}

#[tokio::test]
async fn tag_without_id_and_filter_errors() {
    let client = ApiClient::new("http://localhost:1".into(), "lta_x").unwrap();
    let args = TagArgs {
        id: None,
        names: vec!["rust".into()],
        add: vec![],
        filter: None,
        dry_run: false,
        yes: false,
    };
    let err = commands::tag::run_tag(&client, &args).await.unwrap_err();
    assert!(matches!(err, lettura_cli::error::CliError::BadArgs(_)));
}

#[tokio::test]
async fn bulk_tag_requires_dry_run_or_yes() {
    let client = ApiClient::new("http://localhost:1".into(), "lta_x").unwrap();
    let args = TagArgs {
        id: None,
        names: vec![],
        add: vec!["x".into()],
        filter: Some("untagged".into()),
        dry_run: false,
        yes: false,
    };
    let err = commands::tag::run_tag(&client, &args).await.unwrap_err();
    assert!(matches!(err, lettura_cli::error::CliError::BadArgs(_)));
}

#[tokio::test]
async fn bulk_tag_dry_run_posts_to_bulk_endpoint() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(POST).path("/api/v1/entries/bulk/tag")
            .json_body_partial(r#"{"dry_run":true}"#);
        then.status(200).body(r#"{"matched":5,"updated":0,"ids":[]}"#);
    });
    let client = ApiClient::new(server.base_url(), "lta_x").unwrap();
    let args = TagArgs {
        id: None,
        names: vec![],
        add: vec!["x".into()],
        filter: Some("untagged".into()),
        dry_run: true,
        yes: false,
    };
    let code = commands::tag::run_tag(&client, &args).await.unwrap();
    assert_eq!(code, 0);
    m.assert();
}

#[tokio::test]
async fn bulk_archive_requires_safety_flag() {
    let client = ApiClient::new("http://localhost:1".into(), "lta_x").unwrap();
    let args = StateChangeArgs {
        id: None,
        filter: Some("domain:x.test".into()),
        dry_run: false,
        yes: false,
    };
    let err = commands::state::run_archive(&client, &args).await.unwrap_err();
    assert!(matches!(err, lettura_cli::error::CliError::BadArgs(_)));
}
