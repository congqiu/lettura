use axum::extract::rejection::JsonRejection;
use axum::extract::FromRequest;
use axum::http::Request;
use axum::Json;
use serde::de::DeserializeOwned;
use validator::Validate;

use super::error::ApiError;

/// Axum extractor that deserializes JSON and validates with the `validator` crate.
pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
{
    type Rejection = ApiError;

    async fn from_request(
        req: Request<axum::body::Body>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|e| ApiError::BadRequest(e.body_text()))?;

        value.validate().map_err(|e| {
            let messages: Vec<String> = e
                .field_errors()
                .into_iter()
                .map(|(field, errors)| {
                    let msgs: Vec<String> = errors
                        .iter()
                        .filter_map(|err| err.message.as_ref().map(|m| m.to_string()))
                        .collect();
                    if msgs.is_empty() {
                        format!("{field}: validation failed")
                    } else {
                        format!("{field}: {}", msgs.join(", "))
                    }
                })
                .collect();
            ApiError::BadRequest(messages.join("; "))
        })?;

        Ok(ValidatedJson(value))
    }
}
