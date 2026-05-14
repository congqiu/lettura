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

/// Helper: publish a temp HTML file and return the page ID.
async fn publish_temp_page(
    bin: &std::path::Path,
    server: &str,
    token: &str,
    title: &str,
) -> (String, std::path::PathBuf) {
    let temp_dir = std::env::temp_dir().join(format!("lettura-contract-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let html_file = temp_dir.join("index.html");
    std::fs::write(
        &html_file,
        format!(
            "<html><head><title>{}</title></head><body>Content</body></html>",
            title
        ),
    )
    .unwrap();

    let (code, stdout, stderr) = run_cli(
        bin,
        server,
        token,
        &[
            "pages",
            "publish",
            html_file.to_str().unwrap(),
            "--title",
            title,
        ],
    )
    .await;
    assert_eq!(code, 0, "publish failed: stderr={stderr} stdout={stdout}");
    let page: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let page_id = page["id"].as_str().unwrap().to_string();
    (page_id, temp_dir)
}

#[tokio::test]
async fn cli_pages_publish_and_list() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let (page_id, temp_dir) = publish_temp_page(&bin, &app.addr, &token, "My Page").await;

    // list should include the published page
    let (code, stdout, stderr) = run_cli(&bin, &app.addr, &token, &["pages", "list"]).await;
    assert_eq!(code, 0, "list failed: stderr={stderr} stdout={stdout}");
    let list: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let items = list["items"].as_array().unwrap();
    assert!(!items.is_empty(), "list should have items");
    assert!(items.iter().any(|p| p["id"].as_str().unwrap() == page_id));

    std::fs::remove_dir_all(&temp_dir).ok();
    app.cleanup().await;
}

#[tokio::test]
async fn cli_pages_update_and_share() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let (page_id, temp_dir) = publish_temp_page(&bin, &app.addr, &token, "Update Test").await;

    // update title
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["pages", "update", &page_id, "--title", "Updated Title"],
    )
    .await;
    assert_eq!(code, 0, "update failed: stderr={stderr} stdout={stdout}");
    let updated: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(updated["title"].as_str().unwrap(), "Updated Title");

    // share
    let (code, stdout, stderr) =
        run_cli(&bin, &app.addr, &token, &["pages", "share", &page_id]).await;
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

    let (page_id, temp_dir) = publish_temp_page(&bin, &app.addr, &token, "Delete Test").await;

    // delete
    let (code, stdout, stderr) =
        run_cli(&bin, &app.addr, &token, &["pages", "delete", &page_id]).await;
    assert_eq!(code, 0, "delete failed: stderr={stderr} stdout={stdout}");

    // list --status deleted should include it
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["pages", "list", "--status", "deleted"],
    )
    .await;
    assert_eq!(
        code, 0,
        "list deleted failed: stderr={stderr} stdout={stdout}"
    );
    let list: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let items = list["items"].as_array().unwrap();
    assert!(items.iter().any(|p| p["id"].as_str().unwrap() == page_id));

    // restore
    let (code, stdout, stderr) =
        run_cli(&bin, &app.addr, &token, &["pages", "restore", &page_id]).await;
    assert_eq!(code, 0, "restore failed: stderr={stderr} stdout={stdout}");

    // list (active) should include it again
    let (code, stdout, stderr) = run_cli(&bin, &app.addr, &token, &["pages", "list"]).await;
    assert_eq!(
        code, 0,
        "list active failed: stderr={stderr} stdout={stdout}"
    );
    let list: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let items = list["items"].as_array().unwrap();
    assert!(items.iter().any(|p| p["id"].as_str().unwrap() == page_id));

    std::fs::remove_dir_all(&temp_dir).ok();
    app.cleanup().await;
}

#[tokio::test]
async fn cli_pages_publish_directory() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    // Create a directory with multiple files
    let temp_dir = std::env::temp_dir().join(format!("lettura-contract-dir-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::fs::write(
        temp_dir.join("index.html"),
        "<html><head><title>Dir Page</title></head><body>Main</body></html>",
    )
    .unwrap();
    std::fs::write(temp_dir.join("style.css"), "body { color: red; }").unwrap();

    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &[
            "pages",
            "publish",
            temp_dir.to_str().unwrap(),
            "--title",
            "Dir Page",
        ],
    )
    .await;
    assert_eq!(
        code, 0,
        "publish dir failed: stderr={stderr} stdout={stdout}"
    );
    let page: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(page["id"].as_str().is_some());

    std::fs::remove_dir_all(&temp_dir).ok();
    app.cleanup().await;
}

#[tokio::test]
async fn cli_pages_password_protection() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let (page_id, temp_dir) = publish_temp_page(&bin, &app.addr, &token, "Password Test").await;

    // update with password
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["pages", "update", &page_id, "--password", "s3cret!"],
    )
    .await;
    assert_eq!(
        code, 0,
        "set password failed: stderr={stderr} stdout={stdout}"
    );
    let updated: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        updated["has_password"].as_bool().unwrap_or(false),
        "page should have password"
    );

    // clear password
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["pages", "update", &page_id, "--clear-password"],
    )
    .await;
    assert_eq!(
        code, 0,
        "clear password failed: stderr={stderr} stdout={stdout}"
    );
    let updated: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        !updated["has_password"].as_bool().unwrap_or(false),
        "page should not have password"
    );

    std::fs::remove_dir_all(&temp_dir).ok();
    app.cleanup().await;
}

#[tokio::test]
async fn cli_pages_update_files_and_entry_file() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let (page_id, temp_dir) = publish_temp_page(&bin, &app.addr, &token, "Files Test").await;

    // Create a new file to replace
    let new_file = temp_dir.join("updated.html");
    std::fs::write(
        &new_file,
        "<html><head><title>Updated</title></head><body>New content</body></html>",
    )
    .unwrap();

    // update with --files to replace content
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &[
            "pages",
            "update",
            &page_id,
            "--files",
            new_file.to_str().unwrap(),
        ],
    )
    .await;
    assert_eq!(
        code, 0,
        "update files failed: stderr={stderr} stdout={stdout}"
    );

    // update with --entry-file to change the entry point
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["pages", "update", &page_id, "--entry-file", "updated.html"],
    )
    .await;
    assert_eq!(
        code, 0,
        "update entry-file failed: stderr={stderr} stdout={stdout}"
    );
    let updated: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(updated["entry_file"].as_str().unwrap(), "updated.html");

    std::fs::remove_dir_all(&temp_dir).ok();
    app.cleanup().await;
}

#[tokio::test]
async fn cli_pages_expires_at() {
    let app = TestApp::new().await;
    let token = make_pat(&app).await;
    let bin = locate_cli_binary();

    let (page_id, temp_dir) = publish_temp_page(&bin, &app.addr, &token, "Expires Test").await;

    // set expires_at
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &[
            "pages",
            "update",
            &page_id,
            "--expires-at",
            "2099-12-31T23:59:59Z",
        ],
    )
    .await;
    assert_eq!(
        code, 0,
        "set expires_at failed: stderr={stderr} stdout={stdout}"
    );
    let updated: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        updated["expires_at"].as_str().is_some(),
        "expires_at should be set"
    );

    // clear expires_at with "none"
    let (code, stdout, stderr) = run_cli(
        &bin,
        &app.addr,
        &token,
        &["pages", "update", &page_id, "--expires-at", "none"],
    )
    .await;
    assert_eq!(
        code, 0,
        "clear expires_at failed: stderr={stderr} stdout={stdout}"
    );
    let updated: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        updated["expires_at"].is_null(),
        "expires_at should be null after clearing"
    );

    std::fs::remove_dir_all(&temp_dir).ok();
    app.cleanup().await;
}
