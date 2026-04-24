use crate::client::ApiClient;
use crate::config::Resolved;
use crate::error::CliError;

pub async fn run(resolved: &Resolved) -> Result<i32, CliError> {
    let client = ApiClient::new(resolved.url.clone(), &resolved.token)
        .map_err(CliError::from)?;
    let me: serde_json::Value = client.get("/api/v1/auth/me", &[]).await?;
    println!("{}", serde_json::to_string_pretty(&me).unwrap());
    Ok(0)
}
