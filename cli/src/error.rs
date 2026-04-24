use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub enum ExitCode {
    Success = 0,
    NotFound = 2,
    Unauthorized = 3,
    BadArgs = 4,
    ServerError = 5,
    RateLimited = 6,
    Conflict = 7,
}

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("not_found: {0}")]
    NotFound(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("bad_args: {0}")]
    BadArgs(String),
    #[error("server_error: {0}")]
    ServerError(String),
    #[error("rate_limited: {message}")]
    RateLimited { retry_after_sec: Option<u64>, message: String },
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("network: {0}")]
    Network(String),
}

impl CliError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::NotFound(_) => ExitCode::NotFound,
            Self::Unauthorized(_) | Self::Forbidden(_) => ExitCode::Unauthorized,
            Self::BadArgs(_) => ExitCode::BadArgs,
            Self::ServerError(_) | Self::Network(_) => ExitCode::ServerError,
            Self::RateLimited { .. } => ExitCode::RateLimited,
            Self::Conflict(_) => ExitCode::Conflict,
        }
    }

    pub fn code_name(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "not_found",
            Self::Unauthorized(_) => "unauthorized",
            Self::Forbidden(_) => "forbidden",
            Self::BadArgs(_) => "bad_args",
            Self::ServerError(_) => "server_error",
            Self::RateLimited { .. } => "rate_limited",
            Self::Conflict(_) => "conflict",
            Self::Network(_) => "network",
        }
    }

    pub fn hint(&self) -> Option<String> {
        match self {
            Self::Unauthorized(_) => Some("Run `lettura-cli login` to refresh your credentials.".into()),
            Self::RateLimited { retry_after_sec: Some(s), .. } => Some(format!("Retry after {s} seconds.")),
            Self::NotFound(_) => Some("Use `lettura-cli list` to find entry ids.".into()),
            _ => None,
        }
    }
}

#[derive(Serialize)]
pub struct ErrorReport<'a> {
    pub error: ErrorBody<'a>,
}

#[derive(Serialize)]
pub struct ErrorBody<'a> {
    pub code: &'a str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

pub fn emit_error_to_stderr(err: &CliError) {
    let report = ErrorReport {
        error: ErrorBody {
            code: err.code_name(),
            message: err.to_string(),
            hint: err.hint(),
        },
    };
    let _ = serde_json::to_writer(std::io::stderr(), &report);
    eprintln!();
}

// anyhow::Error → CliError::ServerError (catch-all for unexpected)
impl From<anyhow::Error> for CliError {
    fn from(e: anyhow::Error) -> Self {
        CliError::ServerError(e.to_string())
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::ServerError(e.to_string())
    }
}
