use reqwest::{Client, StatusCode, header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT}};
use serde::{de::DeserializeOwned, Serialize};

use crate::error::CliError;

pub struct ApiClient {
    http: Client,
    base: String,
}

impl ApiClient {
    pub fn new(base: String, token: &str) -> anyhow::Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))?,
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(concat!("lettura-cli/", env!("CARGO_PKG_VERSION"))),
        );
        let http = Client::builder().default_headers(headers).build()?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
        })
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, CliError> {
        let resp = self
            .http
            .get(format!("{}{}", self.base, path))
            .query(query)
            .send()
            .await
            .map_err(|e| CliError::Network(e.to_string()))?;
        handle_response(resp).await
    }

    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, CliError> {
        let resp = self
            .http
            .post(format!("{}{}", self.base, path))
            .json(body)
            .send()
            .await
            .map_err(|e| CliError::Network(e.to_string()))?;
        handle_response(resp).await
    }

    pub async fn delete<T: DeserializeOwned + Default>(&self, path: &str) -> Result<T, CliError> {
        let resp = self
            .http
            .delete(format!("{}{}", self.base, path))
            .send()
            .await
            .map_err(|e| CliError::Network(e.to_string()))?;
        let status = resp.status();
        if status == StatusCode::NO_CONTENT {
            return Ok(T::default());
        }
        let body = resp
            .text()
            .await
            .map_err(|e| CliError::Network(e.to_string()))?;
        if status.is_success() {
            return serde_json::from_str(&body)
                .map_err(|e| CliError::ServerError(format!("bad json: {e}")));
        }
        Err(map_status(status, body))
    }

    pub async fn http_patch<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, CliError> {
        let resp = self
            .http
            .patch(format!("{}{}", self.base, path))
            .json(body)
            .send()
            .await
            .map_err(|e| CliError::Network(e.to_string()))?;
        handle_response(resp).await
    }

    pub async fn get_text(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<String, CliError> {
        let resp = self
            .http
            .get(format!("{}{}", self.base, path))
            .query(query)
            .send()
            .await
            .map_err(|e| CliError::Network(e.to_string()))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| CliError::Network(e.to_string()))?;
        if status.is_success() {
            Ok(body)
        } else {
            Err(map_status(status, body))
        }
    }
}

async fn handle_response<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, CliError> {
    let status = resp.status();
    if status == StatusCode::NO_CONTENT {
        return serde_json::from_str("null")
            .map_err(|e| CliError::ServerError(e.to_string()));
    }
    let body = resp
        .text()
        .await
        .map_err(|e| CliError::Network(e.to_string()))?;
    if status.is_success() {
        return serde_json::from_str(&body)
            .map_err(|e| CliError::ServerError(format!("bad json: {e}")));
    }
    Err(map_status(status, body))
}

fn map_status(status: StatusCode, body: String) -> CliError {
    match status.as_u16() {
        401 => CliError::Unauthorized(body),
        403 => CliError::Forbidden(body),
        404 => CliError::NotFound(body),
        400 | 422 => CliError::BadArgs(body),
        409 => CliError::Conflict(body),
        429 => CliError::RateLimited { retry_after_sec: None, message: body },
        500..=599 => CliError::ServerError(body),
        _ => CliError::ServerError(format!("HTTP {status}: {body}")),
    }
}
