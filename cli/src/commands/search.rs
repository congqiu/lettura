use crate::api_types::EntrySummary;
use crate::cli::{OutputFormat, SearchArgs};
use crate::client::ApiClient;
use crate::error::CliError;
use crate::output::{emit_ids, emit_json};

pub async fn run(
    client: &ApiClient,
    args: &SearchArgs,
    output: OutputFormat,
    pretty: bool,
) -> Result<i32, CliError> {
    let mut q: Vec<(&str, String)> = vec![("search", args.query.clone())];
    q.push(("per_page", args.limit.unwrap_or(20).to_string()));
    let entries: Vec<EntrySummary> = client.get("/api/v1/entries", &q).await?;
    match output {
        OutputFormat::Ids => {
            emit_ids(entries.iter().map(|e| e.id)).map_err(CliError::from)?;
        }
        _ => emit_json(&entries, pretty).map_err(CliError::from)?,
    }
    Ok(0)
}
