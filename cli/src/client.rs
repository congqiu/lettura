use reqwest::{
    Client, StatusCode,
    header::{AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT},
};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

use crate::api_types::UploadResponse;
use crate::error::CliError;

const DEFAULT_TIMEOUT_SECS: u64 = 30;

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
        let http = Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, CliError> {
        let resp = self
            .http
            .get(self.url(path))
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
            .post(self.url(path))
            .json(body)
            .send()
            .await
            .map_err(|e| CliError::Network(e.to_string()))?;
        handle_response(resp).await
    }

    pub async fn delete<T: DeserializeOwned + Default>(&self, path: &str) -> Result<T, CliError> {
        let resp = self
            .http
            .delete(self.url(path))
            .send()
            .await
            .map_err(|e| CliError::Network(e.to_string()))?;
        handle_response(resp).await
    }

    pub async fn http_patch<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, CliError> {
        let resp = self
            .http
            .patch(self.url(path))
            .json(body)
            .send()
            .await
            .map_err(|e| CliError::Network(e.to_string()))?;
        handle_response(resp).await
    }

    pub async fn get_text(&self, path: &str, query: &[(&str, String)]) -> Result<String, CliError> {
        let resp = self
            .http
            .get(self.url(path))
            .query(query)
            .send()
            .await
            .map_err(|e| CliError::Network(e.to_string()))?;
        let status = resp.status();
        let retry_after = parse_retry_after(&resp);
        let body = resp
            .text()
            .await
            .map_err(|e| CliError::Network(e.to_string()))?;
        if status.is_success() {
            Ok(body)
        } else {
            Err(map_status(status, body, retry_after))
        }
    }

    pub async fn upload_files(&self, file_path: &Path) -> Result<UploadResponse, CliError> {
        let file_name = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let file_bytes = tokio::fs::read(file_path)
            .await
            .map_err(|e| CliError::UploadFailed(format!("Failed to read file: {e}")))?;

        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("application/octet-stream")
            .map_err(|e| CliError::UploadFailed(format!("MIME error: {e}")))?;

        let form = reqwest::multipart::Form::new().part("files", part);

        let resp = self
            .http
            .post(self.url("/api/v1/pages/upload"))
            .multipart(form)
            .send()
            .await
            .map_err(|e| CliError::Network(e.to_string()))?;

        handle_response(resp).await
    }
}

async fn handle_response<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, CliError> {
    let status = resp.status();
    let retry_after = parse_retry_after(&resp);
    if status == StatusCode::NO_CONTENT {
        return serde_json::from_str("null").map_err(|e| CliError::ServerError(e.to_string()));
    }
    let body = resp
        .text()
        .await
        .map_err(|e| CliError::Network(e.to_string()))?;
    if status.is_success() {
        return serde_json::from_str(&body)
            .map_err(|e| CliError::ServerError(format!("bad json: {e}")));
    }
    Err(map_status(status, body, retry_after))
}

fn parse_retry_after(resp: &reqwest::Response) -> Option<u64> {
    resp.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
}

fn map_status(status: StatusCode, body: String, retry_after: Option<u64>) -> CliError {
    match status.as_u16() {
        401 => CliError::Unauthorized(body),
        403 => CliError::Forbidden(body),
        404 => CliError::NotFound(body),
        400 | 422 => CliError::BadArgs(body),
        409 => CliError::Conflict(body),
        429 => CliError::RateLimited {
            retry_after_sec: retry_after,
            message: body,
        },
        500..=599 => CliError::ServerError(body),
        _ => CliError::ServerError(format!("HTTP {status}: {body}")),
    }
}
