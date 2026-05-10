use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub db: String,
    pub search: String,
}

pub async fn health_check(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let db_result = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await;
    let db_ok = db_result.is_ok();
    let db_msg = match &db_result {
        Ok(_) => "ok".to_string(),
        Err(e) => {
            tracing::error!("health check failed (db): {e}");
            "error".to_string()
        }
    };

    let search_result = state.search_index.doc_count();
    let search_ok = search_result.is_ok();
    let search_msg = match &search_result {
        Ok(count) => format!("ok ({count} docs)"),
        Err(e) => {
            tracing::error!("health check failed (search): {e}");
            "error".to_string()
        }
    };

    // ok = all good, degraded = partial failure, error = all down
    let (status, code) = determine_status(db_ok, search_ok);

    (
        code,
        Json(HealthResponse {
            status: status.to_string(),
            db: db_msg,
            search: search_msg,
        }),
    )
}

/// Determine overall health status and HTTP status code from component states.
/// Extracted as a pure function for testability.
fn determine_status(db_ok: bool, search_ok: bool) -> (&'static str, StatusCode) {
    if db_ok && search_ok {
        ("ok", StatusCode::OK)
    } else if db_ok {
        ("degraded", StatusCode::OK)
    } else {
        ("error", StatusCode::SERVICE_UNAVAILABLE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn status_both_healthy() {
        let (status, code) = determine_status(true, true);
        assert_eq!(status, "ok");
        assert_eq!(code, StatusCode::OK);
    }

    #[test]
    fn status_db_ok_search_down() {
        let (status, code) = determine_status(true, false);
        assert_eq!(status, "degraded");
        assert_eq!(code, StatusCode::OK);
    }

    #[test]
    fn status_db_down_search_ok() {
        let (status, code) = determine_status(false, true);
        assert_eq!(status, "error");
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn status_both_down() {
        let (status, code) = determine_status(false, false);
        assert_eq!(status, "error");
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn health_response_serializes() {
        let resp = HealthResponse {
            status: "ok".to_string(),
            db: "ok".to_string(),
            search: "ok (42 docs)".to_string(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["db"], "ok");
        assert_eq!(json["search"], "ok (42 docs)");
    }
}
