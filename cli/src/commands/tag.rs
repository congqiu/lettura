use serde_json::json;

use crate::cli::{TagArgs, UntagArgs};
use crate::client::ApiClient;
use crate::error::CliError;
use crate::output::emit_json;

pub async fn run_tag(client: &ApiClient, args: &TagArgs) -> Result<i32, CliError> {
    if args.filter.is_some() {
        return run_tag_batch(client, args).await;
    }
    let id = args.id.clone()
        .ok_or_else(|| CliError::BadArgs("id or --filter required".into()))?;
    let names: Vec<String> = if !args.add.is_empty() {
        args.add.clone()
    } else {
        args.names.clone()
    };
    if names.is_empty() {
        return Err(CliError::BadArgs("at least one tag name required".into()));
    }
    for name in &names {
        let _: serde_json::Value = client
            .post(&format!("/api/v1/entries/{id}/tags"), &json!({"label": name}))
            .await?;
    }
    emit_json(&json!({"id": id, "tags_added": names}), true).map_err(CliError::from)?;
    Ok(0)
}

pub async fn run_untag(client: &ApiClient, args: &UntagArgs) -> Result<i32, CliError> {
    if args.filter.is_some() {
        return run_untag_batch(client, args).await;
    }
    let id = args.id.clone()
        .ok_or_else(|| CliError::BadArgs("id or --filter required".into()))?;
    let names: Vec<String> = if !args.remove.is_empty() {
        args.remove.clone()
    } else {
        args.names.clone()
    };
    if names.is_empty() {
        return Err(CliError::BadArgs("at least one tag name required".into()));
    }
    for name in &names {
        let _: serde_json::Value = client
            .delete(&format!(
                "/api/v1/entries/{id}/tags/by-label/{}",
                urlencoding::encode(name)
            ))
            .await?;
    }
    emit_json(&json!({"id": id, "tags_removed": names}), true).map_err(CliError::from)?;
    Ok(0)
}

async fn run_tag_batch(client: &ApiClient, args: &TagArgs) -> Result<i32, CliError> {
    require_safety_flag(args.dry_run, args.yes)?;
    let filter_expr = args.filter.as_deref()
        .ok_or_else(|| CliError::BadArgs("--filter required for batch".into()))?;
    let filter = crate::filter::parse(filter_expr)
        .map_err(|e| CliError::BadArgs(e.to_string()))?;
    let filter_json = filter_to_server_shape(&filter);
    let add: Vec<String> = if !args.add.is_empty() { args.add.clone() } else { args.names.clone() };
    if add.is_empty() {
        return Err(CliError::BadArgs("--add or positional names required for batch tag".into()));
    }
    let body = json!({
        "filter": filter_json,
        "add": add,
        "dry_run": args.dry_run,
    });
    let resp: serde_json::Value = client.post("/api/v1/entries/bulk/tag", &body).await?;
    emit_json(&resp, true).map_err(CliError::from)?;
    Ok(0)
}

async fn run_untag_batch(client: &ApiClient, args: &UntagArgs) -> Result<i32, CliError> {
    require_safety_flag(args.dry_run, args.yes)?;
    let filter_expr = args.filter.as_deref()
        .ok_or_else(|| CliError::BadArgs("--filter required for batch".into()))?;
    let filter = crate::filter::parse(filter_expr)
        .map_err(|e| CliError::BadArgs(e.to_string()))?;
    let filter_json = filter_to_server_shape(&filter);
    let remove: Vec<String> = if !args.remove.is_empty() { args.remove.clone() } else { args.names.clone() };
    if remove.is_empty() {
        return Err(CliError::BadArgs("--remove or positional names required for batch untag".into()));
    }
    let body = json!({
        "filter": filter_json,
        "remove": remove,
        "dry_run": args.dry_run,
    });
    let resp: serde_json::Value = client.post("/api/v1/entries/bulk/untag", &body).await?;
    emit_json(&resp, true).map_err(CliError::from)?;
    Ok(0)
}

fn require_safety_flag(dry_run: bool, yes: bool) -> Result<(), CliError> {
    if !(dry_run || yes) {
        return Err(CliError::BadArgs(
            "batch write requires --dry-run or --yes".into(),
        ));
    }
    Ok(())
}

/// Translate the CLI filter DSL into the JSON shape the server's bulk endpoint expects.
/// The server accepts the same keys as ListParams.
pub(crate) fn filter_to_server_shape(filter: &crate::filter::Filter) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    if !filter.tags_include.is_empty() {
        map.insert("tag".into(), json!(filter.tags_include.join(",")));
    }
    if !filter.tags_exclude.is_empty() {
        map.insert("exclude_tag".into(), json!(filter.tags_exclude.join(",")));
    }
    if filter.untagged { map.insert("untagged".into(), json!(true)); }
    if let Some(d) = &filter.domain { map.insert("domain".into(), json!(d)); }
    if let Some(t) = filter.since { map.insert("since".into(), json!(t)); }
    if let Some(t) = filter.older_than { map.insert("before".into(), json!(t)); }
    if let Some(b) = filter.starred { map.insert("is_starred".into(), json!(b)); }
    if let Some(b) = filter.archived { map.insert("is_archived".into(), json!(b)); }
    if let Some(b) = filter.read { map.insert("is_read".into(), json!(b)); }
    if let Some(s) = &filter.search { map.insert("search".into(), json!(s)); }
    serde_json::Value::Object(map)
}
