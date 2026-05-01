use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::{AuthSource, AuthUser};
use crate::state::AppState;
use crate::models::audit_log::{self, AuditAction, AuditResourceType};
use crate::models::{entry, memo};
use crate::tasks::fetcher::FetchJob;

use super::validate::ValidatedJson;

fn auth_source_str(auth: &AuthUser) -> String {
    match auth.source {
        AuthSource::Jwt => "jwt".to_string(),
        AuthSource::Pat { .. } => "pat".to_string(),
    }
}

pub async fn list_memos(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<memo::Memo>>, ApiError> {
    let memos = memo::list_memos(&state.pool, auth.user_id).await?;
    Ok(Json(memos))
}

pub async fn create_memo(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(params): ValidatedJson<memo::CreateMemo>,
) -> Result<Json<memo::Memo>, ApiError> {
    let m = memo::create_memo(&state.pool, auth.user_id, &params).await?;
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::CreateMemo,
            resource_type: Some(AuditResourceType::Memo),
            resource_id: Some(m.id),
            status: "success".to_string(),
            details: serde_json::json!({}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;
    Ok(Json(m))
}

pub async fn delete_memo(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(memo_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = memo::delete_memo(&state.pool, auth.user_id, memo_id).await?;
    if !deleted {
        return Err(ApiError::NotFound("memo not found".to_string()));
    }
    let _ = audit_log::insert(
        &state.pool,
        audit_log::InsertAuditLog {
            user_id: Some(auth.user_id),
            auth_source: auth_source_str(&auth),
            action: AuditAction::DeleteMemo,
            resource_type: Some(AuditResourceType::Memo),
            resource_id: Some(memo_id),
            status: "success".to_string(),
            details: serde_json::json!({}),
            error_message: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        },
    ).await;
    Ok(Json(serde_json::json!({"message": "deleted"})))
}

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
        let _ = audit_log::insert(
            &state.pool,
            audit_log::InsertAuditLog {
                user_id: Some(auth.user_id),
                auth_source: auth_source_str(&auth),
                action: AuditAction::PromoteMemo,
                resource_type: Some(AuditResourceType::Memo),
                resource_id: Some(memo_id),
                status: "success".to_string(),
                details: serde_json::json!({"entry_id": new_entry.id}),
                error_message: None,
                ip_address: None,
                user_agent: None,
                request_id: None,
            },
        ).await;
        Ok(Json(serde_json::json!({"message": "promoted to entry", "entry_id": new_entry.id})))
    } else {
        let new_entry =
            entry::create_entry(&state.pool, auth.user_id, &format!("memo:{}", memo_id)).await?;
        entry::update_entry_content(
            &state.pool,
            new_entry.id,
            Some(&m.content),
            Some(&format!("<p>{}</p>", m.content)),
            Some(&m.content),
            None, None, None, Some(1), 0, "manual",
        )
        .await?;
        memo::set_promoted_entry(&state.pool, memo_id, new_entry.id).await?;
        let _ = audit_log::insert(
            &state.pool,
            audit_log::InsertAuditLog {
                user_id: Some(auth.user_id),
                auth_source: auth_source_str(&auth),
                action: AuditAction::PromoteMemo,
                resource_type: Some(AuditResourceType::Memo),
                resource_id: Some(memo_id),
                status: "success".to_string(),
                details: serde_json::json!({"entry_id": new_entry.id}),
                error_message: None,
                ip_address: None,
                user_agent: None,
                request_id: None,
            },
        ).await;
        Ok(Json(serde_json::json!({"message": "promoted to entry", "entry_id": new_entry.id})))
    }
}

fn extract_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|word| word.starts_with("http://") || word.starts_with("https://"))
        .and_then(|word| url::Url::parse(word).ok())
        .map(|u| u.to_string())
}
