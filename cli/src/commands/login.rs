use std::io::{BufRead, Write};

use crate::client::ApiClient;
use crate::config::{Config, Profile};
use crate::error::CliError;
use crate::output;

pub async fn run(profile_name: Option<&str>) -> Result<i32, CliError> {
    let mut stdout = std::io::stdout();
    let path = Config::default_path().map_err(|e| CliError::BadArgs(e.to_string()))?;
    let existing = Config::load_from(&path).unwrap_or_default();

    let default_url = existing
        .default_profile
        .as_ref()
        .and_then(|n| existing.profiles.get(n))
        .map(|p| p.url.as_str())
        .unwrap_or("");

    write!(stdout, "Server URL [{default_url}]: ").map_err(CliError::from)?;
    stdout.flush().map_err(CliError::from)?;

    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).map_err(CliError::from)?;
    let entered = line.trim();
    let url = if entered.is_empty() { default_url.to_string() } else { entered.to_string() };
    if url.is_empty() {
        return Err(CliError::BadArgs("URL required".into()));
    }

    // probe health with a short-timeout client
    let probe_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| CliError::Network(e.to_string()))?;
    let probe = probe_client
        .get(format!("{}/api/health", url.trim_end_matches('/')))
        .send()
        .await
        .map_err(|e| CliError::Network(format!("cannot reach {url}: {e}")))?;
    if !probe.status().is_success() {
        return Err(CliError::Network(format!("health check failed: HTTP {}", probe.status())));
    }

    output::info(&format!(
        "Open {}/settings and generate an API token, then paste it below.",
        url.trim_end_matches('/')
    ));
    write!(stdout, "Paste token: ").map_err(CliError::from)?;
    stdout.flush().map_err(CliError::from)?;

    let mut token_line = String::new();
    std::io::stdin().lock().read_line(&mut token_line).map_err(CliError::from)?;
    let token = token_line.trim().to_string();
    if !token.starts_with("lta_") {
        return Err(CliError::BadArgs("token must start with lta_".into()));
    }

    // verify
    let client = ApiClient::new(url.clone(), &token).map_err(CliError::from)?;
    let _me: serde_json::Value = client.get("/api/v1/auth/me", &[]).await?;

    // save
    let profile_name = profile_name.unwrap_or("default").to_string();
    let mut cfg = existing;
    if cfg.default_profile.is_none() {
        cfg.default_profile = Some(profile_name.clone());
    }
    cfg.profiles.insert(profile_name.clone(), Profile { url, token });
    cfg.save_to(&path).map_err(|e| CliError::ServerError(e.to_string()))?;

    output::info(&format!("Saved profile '{profile_name}'."));
    Ok(0)
}
