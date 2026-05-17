use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::models::fetch_job::{self, FetchJobRow, FetchJobStatus};
use crate::state::AppState;

/// Maximum number of dead jobs that `retry_all_dead` will revive per call.
/// Capped so a single operator action cannot flood the worker pool.
const RETRY_ALL_DEAD_LIMIT: i64 = 100;

#[derive(Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct ListResponse {
    pub items: Vec<FetchJobRow>,
}

#[derive(Serialize)]
pub struct RetryAllResponse {
    pub retried: u64,
    pub remaining_dead: i64,
}

/// Admin endpoints are JWT-only. PATs are deliberately rejected because the
/// PAT middleware fixes `is_admin = false`, so they cannot reach the gated
/// branches. Returning a descriptive message helps operators understand why
/// their token is refused.
fn require_admin(auth: &AuthUser) -> Result<(), ApiError> {
    if auth.is_admin {
        Ok(())
    } else {
        Err(ApiError::Forbidden(
            "admin role required (PAT does not grant admin access)".to_string(),
        ))
    }
}

fn parse_status(s: &str) -> Option<FetchJobStatus> {
    match s {
        "pending" => Some(FetchJobStatus::Pending),
        "running" => Some(FetchJobStatus::Running),
        "failed" => Some(FetchJobStatus::Failed),
        "dead" => Some(FetchJobStatus::Dead),
        _ => None,
    }
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse>, ApiError> {
    require_admin(&auth)?;
    let status = q.status.as_deref().and_then(parse_status);
    let items = fetch_job::list_by_status(&state.pool, status, q.limit.unwrap_or(50))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(ListResponse { items }))
}

pub async fn get(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<FetchJobRow>, ApiError> {
    require_admin(&auth)?;
    let row = fetch_job::find_by_id(&state.pool, id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("fetch job".to_string()))?;
    Ok(Json(row))
}

pub async fn delete(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_admin(&auth)?;
    fetch_job::delete_by_id(&state.pool, id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn retry(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_admin(&auth)?;
    fetch_job::retry(&state.pool, id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn retry_all_dead(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<RetryAllResponse>, ApiError> {
    require_admin(&auth)?;
    let (retried, remaining_dead) = fetch_job::retry_all_dead(&state.pool, RETRY_ALL_DEAD_LIMIT)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(RetryAllResponse {
        retried,
        remaining_dead,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::middleware::{AuthSource, AuthUser, PatScope};

    #[test]
    fn require_admin_allows_admin_jwt() {
        let auth = AuthUser {
            user_id: Uuid::new_v4(),
            is_admin: true,
            source: AuthSource::Jwt,
        };
        assert!(require_admin(&auth).is_ok());
    }

    #[test]
    fn require_admin_rejects_non_admin_jwt() {
        let auth = AuthUser {
            user_id: Uuid::new_v4(),
            is_admin: false,
            source: AuthSource::Jwt,
        };
        match require_admin(&auth) {
            Err(ApiError::Forbidden(msg)) => assert!(msg.contains("PAT")),
            other => panic!("expected Forbidden, got {other:?}"),
        }
    }

    #[test]
    fn require_admin_rejects_pat_with_clear_message() {
        let auth = AuthUser {
            user_id: Uuid::new_v4(),
            is_admin: false,
            source: AuthSource::Pat {
                scope: PatScope::Write,
                token_id: Uuid::new_v4(),
            },
        };
        match require_admin(&auth) {
            Err(ApiError::Forbidden(msg)) => {
                assert!(msg.contains("PAT"), "message should mention PAT: {msg}");
                assert!(msg.contains("admin"), "message should mention admin: {msg}");
            }
            other => panic!("expected Forbidden, got {other:?}"),
        }
    }

    #[test]
    fn parse_status_known_values() {
        assert_eq!(parse_status("pending"), Some(FetchJobStatus::Pending));
        assert_eq!(parse_status("running"), Some(FetchJobStatus::Running));
        assert_eq!(parse_status("failed"), Some(FetchJobStatus::Failed));
        assert_eq!(parse_status("dead"), Some(FetchJobStatus::Dead));
    }

    #[test]
    fn parse_status_unknown_returns_none() {
        assert_eq!(parse_status("bogus"), None);
        assert_eq!(parse_status(""), None);
        assert_eq!(parse_status("PENDING"), None); // case-sensitive
    }
}
