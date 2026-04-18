use axum::extract::rejection::JsonRejection;
use axum::extract::FromRequest;
use axum::http::Request;
use axum::Json;
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

/// Deserialize `Option<bool>` from query strings where booleans arrive as strings
/// (e.g. `?is_archived=false` — serde_urlencoded passes "false" as a string).
pub fn deserialize_bool_from_string<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    use std::fmt;

    struct BoolOrString;

    impl<'de> de::Visitor<'de> for BoolOrString {
        type Value = Option<bool>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a boolean or a string \"true\"/\"false\"")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D2: de::Deserializer<'de>>(self, d: D2) -> Result<Self::Value, D2::Error> {
            d.deserialize_any(BoolOrString)
        }

        fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
            Ok(Some(v))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            match v {
                "true" => Ok(Some(true)),
                "false" => Ok(Some(false)),
                _ => Err(de::Error::invalid_value(de::Unexpected::Str(v), &self)),
            }
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }

    deserializer.deserialize_option(BoolOrString)
}
