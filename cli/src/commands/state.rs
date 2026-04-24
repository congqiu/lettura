use crate::cli::StateChangeArgs;
use crate::client::ApiClient;
use crate::error::CliError;

async fn toggle(
    client: &ApiClient,
    args: &StateChangeArgs,
    field: &'static str,
    value: bool,
) -> Result<i32, CliError> {
    if args.filter.is_some() {
        return toggle_batch(client, args, field, value).await;
    }
    let id = args.id.as_ref()
        .ok_or_else(|| CliError::BadArgs("id or --filter required".into()))?;
    let body = serde_json::json!({ field: value });
    let resp: serde_json::Value = client
        .http_patch(&format!("/api/v1/entries/{id}"), &body).await?;
    crate::output::emit_json(&resp, true).map_err(CliError::from)?;
    Ok(0)
}

async fn toggle_batch(
    client: &ApiClient,
    args: &StateChangeArgs,
    field: &'static str,
    value: bool,
) -> Result<i32, CliError> {
    if !(args.dry_run || args.yes) {
        return Err(CliError::BadArgs(
            "batch write requires --dry-run or --yes".into(),
        ));
    }
    let filter_expr = args.filter.as_deref().unwrap();
    let filter = crate::filter::parse(filter_expr)
        .map_err(|e| CliError::BadArgs(e.to_string()))?;
    let filter_json = crate::commands::tag::filter_to_server_shape(&filter);
    let endpoint = match field {
        "is_archived" if value => "archive",
        "is_archived" => "unarchive",
        _ if value => "star",
        _ => "unstar",
    };
    let body = serde_json::json!({
        "filter": filter_json,
        "value": value,
        "dry_run": args.dry_run,
    });
    let resp: serde_json::Value = client
        .post(&format!("/api/v1/entries/bulk/{endpoint}"), &body).await?;
    crate::output::emit_json(&resp, true).map_err(CliError::from)?;
    Ok(0)
}

pub async fn run_archive(client: &ApiClient, args: &StateChangeArgs) -> Result<i32, CliError> {
    toggle(client, args, "is_archived", true).await
}

pub async fn run_unarchive(client: &ApiClient, args: &StateChangeArgs) -> Result<i32, CliError> {
    toggle(client, args, "is_archived", false).await
}

pub async fn run_star(client: &ApiClient, args: &StateChangeArgs) -> Result<i32, CliError> {
    toggle(client, args, "is_starred", true).await
}

pub async fn run_unstar(client: &ApiClient, args: &StateChangeArgs) -> Result<i32, CliError> {
    toggle(client, args, "is_starred", false).await
}
