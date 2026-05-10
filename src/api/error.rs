use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Validation(HashMap<String, Vec<String>>),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    Internal(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::BadRequest(msg) => write!(f, "bad request: {msg}"),
            ApiError::Validation(fields) => write!(f, "validation error: {fields:?}"),
            ApiError::Unauthorized(msg) => write!(f, "unauthorized: {msg}"),
            ApiError::Forbidden(msg) => write!(f, "forbidden: {msg}"),
            ApiError::NotFound(msg) => write!(f, "not found: {msg}"),
            ApiError::Conflict(msg) => write!(f, "conflict: {msg}"),
            ApiError::Internal(msg) => write!(f, "internal: {msg}"),
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    message: String,
}

#[derive(Serialize)]
struct ValidationErrorBody {
    error: String,
    fields: HashMap<String, Vec<String>>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::Validation(fields) => {
                let body = ValidationErrorBody {
                    error: "validation".to_string(),
                    fields,
                };
                (StatusCode::BAD_REQUEST, axum::Json(body)).into_response()
            }
            other => {
                let (status, error_type, message) = match other {
                    ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
                    ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "unauthorized", msg),
                    ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg),
                    ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg),
                    ApiError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg),
                    ApiError::Internal(msg) => {
                        tracing::error!("internal error: {}", msg);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "internal_error",
                            "internal server error".to_string(),
                        )
                    }
                    ApiError::Validation(_) => unreachable!(),
                };

                let body = ErrorBody {
                    error: error_type.to_string(),
                    message,
                };

                (status, axum::Json(body)).into_response()
            }
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::Database(db_err) => {
                if let Some(constraint) = db_err.constraint() {
                    let msg = match constraint {
                        "users_email_key" => "email already exists",
                        "users_username_key" => "username already exists",
                        "idx_entries_user_hashed_url" => "URL already saved",
                        _ => "duplicate record",
                    };
                    return ApiError::Conflict(msg.to_string());
                }
                tracing::error!("database error: {e}");
                ApiError::Internal("internal server error".to_string())
            }
            _ => {
                tracing::error!("database error: {e}");
                ApiError::Internal("internal server error".to_string())
            }
        }
    }
}

impl From<crate::models::error::ModelError> for ApiError {
    fn from(e: crate::models::error::ModelError) -> Self {
        match e {
            crate::models::error::ModelError::NotFound(msg) => ApiError::NotFound(msg),
            crate::models::error::ModelError::Conflict(msg) => ApiError::Conflict(msg),
            crate::models::error::ModelError::Database(msg) => {
                tracing::error!("database error: {msg}");
                ApiError::Internal("internal server error".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    // --- Display tests ---

    #[test]
    fn display_bad_request() {
        assert_eq!(
            ApiError::BadRequest("msg".to_string()).to_string(),
            "bad request: msg"
        );
    }

    #[test]
    fn display_unauthorized() {
        assert_eq!(
            ApiError::Unauthorized("msg".to_string()).to_string(),
            "unauthorized: msg"
        );
    }

    #[test]
    fn display_forbidden() {
        assert_eq!(
            ApiError::Forbidden("msg".to_string()).to_string(),
            "forbidden: msg"
        );
    }

    #[test]
    fn display_not_found() {
        assert_eq!(
            ApiError::NotFound("msg".to_string()).to_string(),
            "not found: msg"
        );
    }

    #[test]
    fn display_conflict() {
        assert_eq!(
            ApiError::Conflict("msg".to_string()).to_string(),
            "conflict: msg"
        );
    }

    #[test]
    fn display_internal() {
        assert_eq!(
            ApiError::Internal("msg".to_string()).to_string(),
            "internal: msg"
        );
    }

    #[test]
    fn display_validation() {
        let mut fields = HashMap::new();
        fields.insert("email".to_string(), vec!["invalid".to_string()]);
        let displayed = ApiError::Validation(fields).to_string();
        assert!(displayed.starts_with("validation error:"));
    }

    // --- IntoResponse status code tests ---

    #[tokio::test]
    async fn into_response_bad_request() {
        let response = ApiError::BadRequest("msg".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn into_response_unauthorized() {
        let response = ApiError::Unauthorized("msg".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn into_response_forbidden() {
        let response = ApiError::Forbidden("msg".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn into_response_not_found() {
        let response = ApiError::NotFound("msg".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn into_response_conflict() {
        let response = ApiError::Conflict("msg".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn into_response_internal() {
        let response = ApiError::Internal("msg".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn into_response_validation() {
        let mut fields = HashMap::new();
        fields.insert("field".to_string(), vec!["error".to_string()]);
        let response = ApiError::Validation(fields).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // --- From<ModelError> tests ---

    #[test]
    fn from_model_error_not_found() {
        let err = crate::models::error::ModelError::NotFound("item".to_string());
        match ApiError::from(err) {
            ApiError::NotFound(_) => {}
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn from_model_error_conflict() {
        let err = crate::models::error::ModelError::Conflict("dup".to_string());
        match ApiError::from(err) {
            ApiError::Conflict(_) => {}
            other => panic!("expected Conflict, got {:?}", other),
        }
    }

    #[test]
    fn from_model_error_database() {
        let err = crate::models::error::ModelError::Database("db fail".to_string());
        match ApiError::from(err) {
            ApiError::Internal(_) => {}
            other => panic!("expected Internal, got {:?}", other),
        }
    }
}
