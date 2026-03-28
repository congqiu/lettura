use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::auth::middleware::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub db: String,
    pub search: String,
}

pub async fn health_check(
    State(state): State<AppState>,
) -> (StatusCode, Json<HealthResponse>) {
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();

    let search_ok = state.search_index.doc_count().is_ok();

    let status = if db_ok && search_ok { "ok" } else { "error" };
    let code = if db_ok && search_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        code,
        Json(HealthResponse {
            status: status.to_string(),
            db: if db_ok {
                "ok".to_string()
            } else {
                "error".to_string()
            },
            search: if search_ok {
                "ok".to_string()
            } else {
                "error".to_string()
            },
        }),
    )
}
