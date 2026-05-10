use crate::api_types::EntrySummary;
use crate::cli::{ListArgs, OutputFormat};
use crate::client::ApiClient;
use crate::error::CliError;
use crate::filter;
use crate::output::{emit_human_entries, emit_ids, emit_json};

pub async fn run(
    client: &ApiClient,
    args: &ListArgs,
    output: OutputFormat,
    pretty: bool,
) -> Result<i32, CliError> {
    let parsed = args
        .filter
        .as_deref()
        .map(filter::parse)
        .transpose()
        .map_err(|e| CliError::BadArgs(e.to_string()))?
        .unwrap_or_default();
    let mut q = parsed.to_query();
    q.push(("per_page", args.limit.unwrap_or(20).to_string()));
    if let Some(fields) = &args.fields {
        q.push(("fields", fields.clone()));
    }

    let entries: Vec<EntrySummary> = client.get("/api/v1/entries", &q).await?;
    match output {
        OutputFormat::Ids => {
            emit_ids(entries.iter().map(|e| e.id.clone())).map_err(CliError::from)?;
        }
        OutputFormat::Json => {
            emit_json(&entries, pretty).map_err(CliError::from)?;
        }
        OutputFormat::Human => {
            emit_human_entries(&entries).map_err(CliError::from)?;
        }
    }
    Ok(0)
}
