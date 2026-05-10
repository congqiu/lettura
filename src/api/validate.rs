use axum::Json;
use axum::extract::FromRequest;
use axum::extract::rejection::JsonRejection;
use axum::http::Request;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use validator::Validate;

use super::error::ApiError;

/// Axum extractor that deserializes JSON and validates with the `validator` crate.
/// Returns structured field-level errors via `ApiError::Validation`.
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
            let mut fields: HashMap<String, Vec<String>> = HashMap::new();
            for (field, errors) in e.field_errors() {
                let msgs: Vec<String> = errors
                    .iter()
                    .map(|err| {
                        err.message
                            .as_ref()
                            .map(|m| m.to_string())
                            .unwrap_or_else(|| "validation failed".to_string())
                    })
                    .collect();
                fields.insert(field.to_string(), msgs);
            }
            ApiError::Validation(fields)
        })?;

        Ok(ValidatedJson(value))
    }
}

/// Re-export for backward compatibility.
pub use crate::models::serde_helpers::deserialize_bool_from_string;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct WrapBool {
        #[serde(default, deserialize_with = "deserialize_bool_from_string")]
        val: Option<bool>,
    }

    #[test]
    fn reexported_deserialize_bool_from_true_string() {
        let w: WrapBool = serde_qs::from_str("val=true").unwrap();
        assert_eq!(w.val, Some(true));
    }

    #[test]
    fn reexported_deserialize_bool_from_false_string() {
        let w: WrapBool = serde_qs::from_str("val=false").unwrap();
        assert_eq!(w.val, Some(false));
    }

    #[test]
    fn reexported_deserialize_bool_invalid_is_error() {
        let res = serde_qs::from_str::<WrapBool>("val=yes");
        assert!(res.is_err());
    }

    #[test]
    fn reexported_deserialize_bool_absent_is_none() {
        let w: WrapBool = serde_qs::from_str("").unwrap();
        assert_eq!(w.val, None);
    }
}
