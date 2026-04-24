use crate::cli::SaveArgs;
use crate::client::ApiClient;
use crate::error::CliError;
use crate::output::emit_json;

pub async fn run(client: &ApiClient, args: &SaveArgs) -> Result<i32, CliError> {
    let body = serde_json::json!({
        "url": args.url,
        "title": args.title,
        "tag": args.tag,
    });
    let resp: serde_json::Value = client.post("/api/v1/entries", &body).await?;

    if !args.wait {
        emit_json(&resp, true).map_err(CliError::from)?;
        return Ok(0);
    }

    // --wait: poll GET /entries/{id} up to 30s until content is populated.
    let id = resp["id"].as_str()
        .ok_or_else(|| CliError::ServerError("missing id in create response".into()))?
        .to_string();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut interval_ms = 500u64;
    loop {
        if std::time::Instant::now() > deadline {
            return Err(CliError::ServerError(format!(
                "save queued but not ready after 30s; try `lettura-cli get {id}` later"
            )));
        }
        let entry: serde_json::Value = client
            .get(&format!("/api/v1/entries/{id}"), &[])
            .await?;
        let has_content = entry.get("content")
            .and_then(|c| c.as_str())
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        if has_content {
            emit_json(&entry, true).map_err(CliError::from)?;
            return Ok(0);
        }
        tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
        interval_ms = (interval_ms * 2).min(4000); // exponential backoff, cap at 4s
    }
}
