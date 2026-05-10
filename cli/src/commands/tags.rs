use crate::api_types::Tag;
use crate::cli::OutputFormat;
use crate::client::ApiClient;
use crate::error::CliError;
use crate::output::{emit_human_tags, emit_json};

pub async fn run(client: &ApiClient, output: OutputFormat, pretty: bool) -> Result<i32, CliError> {
    let tags: Vec<Tag> = client.get("/api/v1/tags", &[]).await?;
    match output {
        OutputFormat::Human => emit_human_tags(&tags).map_err(CliError::from)?,
        _ => emit_json(&tags, pretty).map_err(CliError::from)?,
    }
    Ok(0)
}
