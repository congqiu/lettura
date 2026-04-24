use crate::client::ApiClient;
use crate::error::CliError;

pub async fn run(client: &ApiClient) -> Result<i32, CliError> {
    let me: serde_json::Value = client.get("/api/v1/auth/me", &[]).await?;
    crate::output::emit_json(&me, true).map_err(CliError::from)?;
    Ok(0)
}
