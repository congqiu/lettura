use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub db: String,
    pub search: String,
}

pub async fn health_check(
    State(state): State<AppState>,
) -> (StatusCode, Json<HealthResponse>) {
    let db_result = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await;
    let db_ok = db_result.is_ok();
    let db_msg = match &db_result {
        Ok(_) => "ok".to_string(),
        Err(e) => format!("error: {e}"),
    };

    let search_result = state.search_index.doc_count();
    let search_ok = search_result.is_ok();
    let search_msg = match &search_result {
        Ok(count) => format!("ok ({count} docs)"),
        Err(e) => format!("error: {e}"),
    };

    // ok = all good, degraded = partial failure, error = all down
    let (status, code) = if db_ok && search_ok {
        ("ok", StatusCode::OK)
    } else if db_ok {
        ("degraded", StatusCode::OK)
    } else {
        ("error", StatusCode::SERVICE_UNAVAILABLE)
    };

    (
        code,
        Json(HealthResponse {
            status: status.to_string(),
            db: db_msg,
            search: search_msg,
        }),
    )
}
