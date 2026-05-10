use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::auth::middleware::AuthUser;
use crate::models::audit_log::{
    self, AuditAction, AuditLog, AuditResourceType, ListAuditLogsFilter,
};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListAuditLogsQuery {
    #[serde(default)]
    action: Option<AuditAction>,
    #[serde(default)]
    resource_type: Option<AuditResourceType>,
    #[serde(default)]
    resource_id: Option<Uuid>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limit_returns_50() {
        assert_eq!(default_limit(), 50);
    }

    #[test]
    fn limit_clamp_within_range() {
        assert_eq!(50i64.clamp(1, 200), 50);
    }

    #[test]
    fn limit_clamp_below_minimum() {
        assert_eq!(0i64.clamp(1, 200), 1);
        assert_eq!((-5i64).clamp(1, 200), 1);
    }

    #[test]
    fn limit_clamp_above_maximum() {
        assert_eq!(300i64.clamp(1, 200), 200);
        assert_eq!(1000i64.clamp(1, 200), 200);
    }

    #[test]
    fn limit_clamp_at_boundaries() {
        assert_eq!(1i64.clamp(1, 200), 1);
        assert_eq!(200i64.clamp(1, 200), 200);
    }
}

#[derive(Serialize)]
pub struct ListAuditLogsResponse {
    data: Vec<AuditLog>,
    total: i64,
    limit: i64,
    offset: i64,
}

pub async fn list_audit_logs(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(q): Query<ListAuditLogsQuery>,
) -> Result<Json<ListAuditLogsResponse>, ApiError> {
    let limit = q.limit.clamp(1, 200);

    let filter = ListAuditLogsFilter {
        user_id: Some(auth.user_id),
        action: q.action,
        resource_type: q.resource_type,
        resource_id: q.resource_id,
        status: q.status,
        limit,
        offset: q.offset,
    };

    let (data, total) = tokio::join!(
        audit_log::list(&state.pool, &filter),
        audit_log::count(&state.pool, &filter),
    );

    Ok(Json(ListAuditLogsResponse {
        data: data.map_err(|e| ApiError::Internal(e.to_string()))?,
        total: total.map_err(|e| ApiError::Internal(e.to_string()))?,
        limit,
        offset: q.offset,
    }))
}
