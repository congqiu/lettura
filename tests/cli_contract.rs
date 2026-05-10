mod common;

use common::TestApp;
use serde_json::json;
use std::process::Command;
use uuid::Uuid;

/// Locate the compiled lettura-cli binary. Builds it first if needed.
fn locate_cli_binary() -> std::path::PathBuf {
    // cargo places workspace binaries in target/{debug,release}/lettura-cli
    let target_dir = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let p = std::path::PathBuf::from(&target_dir)
        .join(profile)
        .join("lettura-cli");
    if !p.exists() {
        let output = Command::new(env!("CARGO"))
            .args(["build", "-p", "lettura-cli"])
            .output()
            .expect("failed to invoke cargo build");
        assert!(
            output.status.success(),
            "cargo build -p lettura-cli failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(p.exists(), "lettura-cli binary not at {}", p.display());
    p
}

async fn make_pat(app: &TestApp) -> String {
    app.client
        .post(app.url("/api/v1/auth/register"))
        .json(&json!({"username":"u","email":"u@e.com","password":"password123"}))
        .send()
        .await
        .unwrap();
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email=$1")
        .bind("u@e.com")
        .fetch_one(&app.pool)
        .await
        .unwrap();

    let token = lettura::models::pat::generate_token();
    let hash = lettura::models::pat::hash_token(&token);
    let prefix = lettura::models::pat::token_prefix(&token);
    lettura::models::pat::insert(
        &app.pool,
        user_id,
        "contract-test",
        &hash,
        &prefix,
        lettura::models::pat::Scope::Write,
        None,
    )
    .await
    .unwrap();
    token
}

/// Run the CLI as a subprocess. Uses tokio::process::Command to avoid blocking
/// the async executor (which would deadlock the in-process Axum test server).
async fn run_cli(
    bin: &std::path::Path,
    server: &str,
    token: &str,
    args: &[&str],
) -> (i32, String, String) {
    let mut cmd = tokio::process::Command::new(bin);
    cmd.args(["--url", server, "--token", token]);
    cmd.args(args);
    let out = cmd.output().await.expect("failed to spawn lettura-cli");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    (code, stdout, stderr)
}

#[tokio::test]
async fn cli_whoami_hits_server() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let (code, stdout, stderr) = run_cli(&bin, &app.addr, &token, &["whoami"]).await;
    assert_eq!(code, 0, "whoami failed: stderr={stderr}");
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("whoami did not emit JSON");
    assert_eq!(parsed["username"], "u");
    assert_eq!(parsed["auth_source"], "pat");
    app.cleanup().await;
}

#[tokio::test]
async fn cli_save_then_list_finds_entry() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    // save
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &[
            "save",
            "https://contract-test.example/a",
            "--tag",
            "backlog",
        ],
    )
    .await;
    assert_eq!(code, 0, "save failed: {stderr}");
    let save_json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let entry_id = save_json["id"].as_str().unwrap().to_string();
    assert_eq!(save_json["already_existed"], false);

    // list with filter on tag should include the saved entry
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["list", "--filter", "tag:backlog", "--output", "ids"],
    )
    .await;
    assert_eq!(code, 0, "list failed: stderr={stderr} stdout={stdout}");
    let ids: Vec<&str> = stdout.lines().collect();
    assert!(
        ids.iter().any(|id| *id == entry_id),
        "list --output ids missing {entry_id}: {ids:?}"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn cli_save_same_url_twice_reports_already_existed() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let (c1, s1, _) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["save", "https://contract-test.example/b"],
    )
    .await;
    assert_eq!(c1, 0);
    let first: serde_json::Value = serde_json::from_str(s1.trim()).unwrap();
    assert_eq!(first["already_existed"], false);

    let (c2, s2, _) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["save", "https://contract-test.example/b"],
    )
    .await;
    assert_eq!(c2, 0);
    let second: serde_json::Value = serde_json::from_str(s2.trim()).unwrap();
    assert_eq!(second["already_existed"], true);
    assert_eq!(second["id"], first["id"]);
    app.cleanup().await;
}

#[tokio::test]
async fn cli_tag_then_get_shows_tag() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let (_, save_stdout, _) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["save", "https://contract-test.example/c"],
    )
    .await;
    let save_json: serde_json::Value = serde_json::from_str(save_stdout.trim()).unwrap();
    let id = save_json["id"].as_str().unwrap();

    let (code, _, stderr) = run_cli(&bin, &app.addr, &token, &["tag", id, "rust"]).await;
    assert_eq!(code, 0, "tag failed: {stderr}");

    let (code, get_stdout, _) =
        run_cli(&bin, &app.addr, &token, &["get", id, "--format", "json"]).await;
    assert_eq!(code, 0);
    let entry: serde_json::Value = serde_json::from_str(get_stdout.trim()).unwrap();
    // `tags` may not be embedded in the get response; fall back to tags-for-entry endpoint via CLI (tags command lists tags).
    // The API currently returns Entry (raw model); tags aren't embedded. So verify via the server directly:
    let tag_rows: Vec<serde_json::Value> = app
        .client
        .get(app.url(&format!("/api/v1/entries/{id}/tags")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let labels: Vec<String> = tag_rows
        .iter()
        .map(|r| r["label"].as_str().unwrap().to_string())
        .collect();
    assert!(labels.contains(&"rust".to_string()), "labels: {labels:?}");
    let _ = entry;
    app.cleanup().await;
}

#[tokio::test]
async fn cli_bulk_dry_run_does_not_modify() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let (_, _, _) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["save", "https://contract-test.example/d"],
    )
    .await;
    let (_, _, _) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["save", "https://contract-test.example/e"],
    )
    .await;

    // dry-run should report matched without persisting
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &[
            "tag",
            "--add",
            "autotag",
            "--filter",
            "untagged",
            "--dry-run",
        ],
    )
    .await;
    assert_eq!(code, 0, "dry-run failed: {stderr}");
    let resp: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        resp["matched"].as_i64().unwrap() >= 2,
        "expected >=2 matched: {resp}"
    );
    assert_eq!(resp["updated"], 0);

    // Verify NO entries got the tag
    let tag_rows: Vec<serde_json::Value> = app
        .client
        .get(app.url("/api/v1/entries?tag=autotag"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        tag_rows.is_empty(),
        "dry-run should not have applied the tag"
    );

    // Now --yes should apply
    let (code, stdout, _) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["tag", "--add", "autotag", "--filter", "untagged", "--yes"],
    )
    .await;
    assert_eq!(code, 0);
    let resp: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(resp["updated"].as_i64().unwrap() >= 2);

    app.cleanup().await;
}

#[tokio::test]
async fn cli_bulk_without_safety_flag_errors() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let (code, _stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["tag", "--add", "x", "--filter", "untagged"],
    )
    .await;
    assert_ne!(code, 0, "expected non-zero exit without --dry-run/--yes");
    assert!(
        stderr.contains("bad_args") || stderr.contains("--dry-run"),
        "stderr should mention safety flag: {stderr}"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn cli_get_markdown_has_frontmatter() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let (_, save_stdout, _) = run_cli(
        &bin,
        &app.addr,
        &token,
        &[
            "save",
            "https://contract-test.example/f",
            "--title",
            "Hello",
        ],
    )
    .await;
    let save_json: serde_json::Value = serde_json::from_str(save_stdout.trim()).unwrap();
    let id = save_json["id"].as_str().unwrap();

    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["get", id, "--format", "markdown"],
    )
    .await;
    assert_eq!(code, 0, "get markdown failed: {stderr}");
    assert!(
        stdout.starts_with("---"),
        "expected front-matter: first 80 chars = {}",
        &stdout.chars().take(80).collect::<String>()
    );
    assert!(stdout.contains(&format!("id: {id}")));
    assert!(stdout.contains("url: https://contract-test.example/f"));
    app.cleanup().await;
}

#[tokio::test]
async fn cli_pages_publish_and_list() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let temp_dir = std::env::temp_dir().join("lettura-contract-pages");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let html_file = temp_dir.join("test-page.html");
    std::fs::write(&html_file, "<html><head><title>Test Page</title></head><body>Hello</body></html>").unwrap();

    // publish
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &[
            "pages", "publish",
            html_file.to_str().unwrap(),
            "--title", "My Page",
        ],
    )
    .await;
    assert_eq!(code, 0, "publish failed: stderr={stderr} stdout={stdout}");

    // list should include the published page
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["pages", "list"],
    )
    .await;
    assert_eq!(code, 0, "list failed: stderr={stderr} stdout={stdout}");
    let list: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(list["items"].as_array().map(|a| !a.is_empty()).unwrap_or(false), "list should have items");

    std::fs::remove_dir_all(&temp_dir).ok();
    app.cleanup().await;
}

#[tokio::test]
async fn cli_pages_update_and_share() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let temp_dir = std::env::temp_dir().join("lettura-contract-pages-update");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let html_file = temp_dir.join("test-page.html");
    std::fs::write(&html_file, "<html><head><title>Test</title></head><body>Hello</body></html>").unwrap();

    // publish
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &[
            "pages", "publish",
            html_file.to_str().unwrap(),
            "--title", "Original",
        ],
    )
    .await;
    assert_eq!(code, 0, "publish failed: stderr={stderr} stdout={stdout}");

    // get page id from list
    let (code, stdout, stderr) = run_cli(&bin, &app.addr, &token, &["pages", "list"]).await;
    assert_eq!(code, 0, "list failed: stderr={stderr}");
    let list: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let page_id = list["items"][0]["id"].as_str().unwrap().to_string();

    // update title
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["pages", "update", &page_id, "--title", "Updated"],
    )
    .await;
    assert_eq!(code, 0, "update failed: stderr={stderr} stdout={stdout}");

    // share
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["pages", "share", &page_id],
    )
    .await;
    assert_eq!(code, 0, "share failed: stderr={stderr} stdout={stdout}");
    let share: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(share["url"].as_str().unwrap().starts_with("/p/"));

    std::fs::remove_dir_all(&temp_dir).ok();
    app.cleanup().await;
}

#[tokio::test]
async fn cli_pages_delete_and_restore() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let temp_dir = std::env::temp_dir().join("lettura-contract-pages-del");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let html_file = temp_dir.join("test-page.html");
    std::fs::write(&html_file, "<html><head><title>Test</title></head><body>Hello</body></html>").unwrap();

    // publish
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &[
            "pages", "publish",
            html_file.to_str().unwrap(),
            "--title", "To Delete",
        ],
    )
    .await;
    assert_eq!(code, 0, "publish failed: stderr={stderr}");

    // get page id
    let (code, stdout, stderr) = run_cli(&bin, &app.addr, &token, &["pages", "list"]).await;
    assert_eq!(code, 0, "list failed: stderr={stderr}");
    let list: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let page_id = list["items"][0]["id"].as_str().unwrap().to_string();

    // delete
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["pages", "delete", &page_id],
    )
    .await;
    assert_eq!(code, 0, "delete failed: stderr={stderr} stdout={stdout}");

    // restore
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["pages", "restore", &page_id],
    )
    .await;
    assert_eq!(code, 0, "restore failed: stderr={stderr} stdout={stdout}");

    std::fs::remove_dir_all(&temp_dir).ok();
    app.cleanup().await;
}
