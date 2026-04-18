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
