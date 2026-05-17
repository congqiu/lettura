use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

use crate::api::auth_source_str;
use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::{entry, memo};
use crate::state::AppState;
use crate::tasks::fetcher::FetchJob;

use super::validate::ValidatedJson;

#[utoipa::path(
    get,
    path = "/api/v1/memos",
    tag = "memos",
    responses(
        (status = 200, description = "List of memos", body = Vec<memo::Memo>),
        (status = 401, description = "Missing or invalid auth"),
    ),
    security(("bearer" = [])),
)]
pub async fn list_memos(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<memo::Memo>>, ApiError> {
    let memos = memo::list_memos(&state.pool, auth.user_id).await?;
    Ok(Json(memos))
}

#[utoipa::path(
    post,
    path = "/api/v1/memos",
    tag = "memos",
    request_body = memo::CreateMemo,
    responses(
        (status = 201, description = "Memo created", body = memo::Memo),
        (status = 401, description = "Missing or invalid auth"),
        (status = 422, description = "Validation error"),
    ),
    security(("bearer" = [])),
)]
pub async fn create_memo(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(params): ValidatedJson<memo::CreateMemo>,
) -> Result<Json<memo::Memo>, ApiError> {
    let m = memo::create_memo(&state.pool, auth.user_id, &params).await?;
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::CreateMemo,
        Some(AuditResourceType::Memo),
        Some(m.id),
        serde_json::json!({}),
    )
    .await;
    Ok(Json(m))
}

#[utoipa::path(
    delete,
    path = "/api/v1/memos/{id}",
    tag = "memos",
    params(("id" = Uuid, Path, description = "Memo ID")),
    responses(
        (status = 200, description = "Memo deleted"),
        (status = 401, description = "Missing or invalid auth"),
        (status = 404, description = "Memo not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn delete_memo(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(memo_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = memo::delete_memo(&state.pool, auth.user_id, memo_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("memo not found".to_string()));
    }
    audit_log::log_success(
        &state.pool,
        Some(auth.user_id),
        auth_source_str(&auth),
        AuditAction::DeleteMemo,
        Some(AuditResourceType::Memo),
        Some(memo_id),
        serde_json::json!({}),
    )
    .await;
    Ok(Json(serde_json::json!({"message": "deleted"})))
}

#[utoipa::path(
    post,
    path = "/api/v1/memos/{id}/promote",
    tag = "memos",
    params(("id" = Uuid, Path, description = "Memo ID")),
    responses(
        (status = 200, description = "Memo promoted to entry"),
        (status = 401, description = "Missing or invalid auth"),
        (status = 404, description = "Memo not found"),
    ),
    security(("bearer" = [])),
)]
pub async fn promote_memo(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(memo_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let m = memo::find_memo_by_id(&state.pool, auth.user_id, memo_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("memo not found".to_string()))?;

    if m.promoted_entry_id.is_some() {
        return Err(ApiError::BadRequest("memo already promoted".to_string()));
    }

    let url = extract_url(&m.content).or(m.source_url.clone());

    if let Some(url) = url {
        let new_entry = entry::create_entry(&state.pool, auth.user_id, &url).await?;
        memo::set_promoted_entry(&state.pool, memo_id, new_entry.id).await?;
        let _ = state
            .fetch_queue
            .send(FetchJob {
                entry_id: new_entry.id,
                user_id: auth.user_id,
                url: new_entry.url.clone(),
            })
            .await;
        audit_log::log_success(
            &state.pool,
            Some(auth.user_id),
            auth_source_str(&auth),
            AuditAction::PromoteMemo,
            Some(AuditResourceType::Memo),
            Some(memo_id),
            serde_json::json!({"entry_id": new_entry.id}),
        )
        .await;
        Ok(Json(
            serde_json::json!({"message": "promoted to entry", "entry_id": new_entry.id}),
        ))
    } else {
        let new_entry =
            entry::create_entry(&state.pool, auth.user_id, &format!("memo:{}", memo_id)).await?;
        entry::update_entry_content(
            &state.pool,
            new_entry.id,
            &entry::ExtractedContent {
                title: Some(m.content.clone()),
                content: Some(format!("<p>{}</p>", m.content)),
                text_content: Some(m.content.clone()),
                reading_time: Some(1),
                http_status: 0,
                extract_method: "manual".to_string(),
                ..Default::default()
            },
        )
        .await?;
        memo::set_promoted_entry(&state.pool, memo_id, new_entry.id).await?;
        audit_log::log_success(
            &state.pool,
            Some(auth.user_id),
            auth_source_str(&auth),
            AuditAction::PromoteMemo,
            Some(AuditResourceType::Memo),
            Some(memo_id),
            serde_json::json!({"entry_id": new_entry.id}),
        )
        .await;
        Ok(Json(
            serde_json::json!({"message": "promoted to entry", "entry_id": new_entry.id}),
        ))
    }
}

fn extract_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|word| word.starts_with("http://") || word.starts_with("https://"))
        .and_then(|word| url::Url::parse(word).ok())
        .map(|u| u.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_url ──

    #[test]
    fn extract_url_finds_http_url() {
        assert_eq!(
            extract_url("check out http://example.com/page"),
            Some("http://example.com/page".to_string())
        );
    }

    #[test]
    fn extract_url_finds_https_url() {
        assert_eq!(
            extract_url("see https://example.com"),
            Some("https://example.com/".to_string())
        );
    }

    #[test]
    fn extract_url_returns_first_when_multiple() {
        let result = extract_url("http://first.com and https://second.com");
        assert_eq!(result, Some("http://first.com/".to_string()));
    }

    #[test]
    fn extract_url_returns_none_when_no_url() {
        assert_eq!(extract_url("just some plain text here"), None);
    }

    #[test]
    fn extract_url_returns_none_without_scheme() {
        assert_eq!(extract_url("visit example.com today"), None);
    }

    #[test]
    fn extract_url_finds_url_surrounded_by_text() {
        assert_eq!(
            extract_url("before https://mid.com after"),
            Some("https://mid.com/".to_string())
        );
    }

    #[test]
    fn extract_url_returns_none_for_invalid_url() {
        // "http://" alone fails url::Url::parse (no host)
        assert_eq!(extract_url("http://"), None);
    }

    #[test]
    fn extract_url_preserves_fragment_and_query() {
        assert_eq!(
            extract_url("https://example.com/path?q=1#section"),
            Some("https://example.com/path?q=1#section".to_string())
        );
    }
}
