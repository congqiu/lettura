use crate::api_types::ListAuditLogsResponse;
use crate::cli::{AuditLogsArgs, OutputFormat};
use crate::client::ApiClient;
use crate::error::CliError;
use crate::output::{emit_human_audit_logs, emit_json};

pub async fn run(
    client: &ApiClient,
    args: &AuditLogsArgs,
    output: OutputFormat,
    pretty: bool,
) -> Result<i32, CliError> {
    let mut q: Vec<(&str, String)> = Vec::new();
    if let Some(action) = &args.action {
        q.push(("action", action.clone()));
    }
    if let Some(resource_type) = &args.resource_type {
        q.push(("resource_type", resource_type.clone()));
    }
    if let Some(status) = &args.status {
        q.push(("status", status.clone()));
    }
    if let Some(limit) = args.limit {
        q.push(("limit", limit.to_string()));
    }
    if let Some(offset) = args.offset {
        q.push(("offset", offset.to_string()));
    }

    let resp: ListAuditLogsResponse = client.get("/api/v1/audit-logs", &q).await?;
    match output {
        OutputFormat::Human => {
            emit_human_audit_logs(&resp.data, resp.total, resp.limit, resp.offset)
                .map_err(CliError::from)?;
        }
        _ => {
            emit_json(&resp, pretty).map_err(CliError::from)?;
        }
    }
    Ok(0)
}
