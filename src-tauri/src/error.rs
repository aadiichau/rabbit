use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Input(String),
    #[error("Network problem: {0}")]
    Network(String),
    #[error("The server answered HTTP {0}.")]
    Http(u16),
    #[error("The media link expired or was refused (HTTP {0}).")]
    Expired(u16),
    #[error("Not enough free disk space to finish this download.")]
    DiskFull,
    #[error("Rabbit isn't allowed to write there: {0}")]
    Permission(String),
    #[error("File error: {0}")]
    Io(String),
    #[error("This content requires a signed-in account on the platform.")]
    LoginRequired(String),
    #[error("This link isn't supported: {0}")]
    Unsupported(String),
    #[error("Couldn't read this link: {0}")]
    Extractor(String),
    #[error("{0}")]
    Tool(String),
    #[error("The finished file failed verification: {0}")]
    Verification(String),
    #[error("Cancelled.")]
    Cancelled,
    #[error("{0}")]
    NotFound(String),
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Input(_) => "input",
            AppError::Network(_) => "network",
            AppError::Http(_) => "http",
            AppError::Expired(_) => "expired",
            AppError::DiskFull => "diskFull",
            AppError::Permission(_) => "permission",
            AppError::Io(_) => "io",
            AppError::LoginRequired(_) => "loginRequired",
            AppError::Unsupported(_) => "unsupported",
            AppError::Extractor(_) => "extractor",
            AppError::Tool(_) => "tool",
            AppError::Verification(_) => "verification",
            AppError::Cancelled => "cancelled",
            AppError::NotFound(_) => "notFound",
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(self, AppError::Network(_) | AppError::Http(500..=599) | AppError::Http(429) | AppError::Expired(_) | AppError::Verification(_))
    }

    /// Extra detail for the "Details" disclosure (raw tool output etc.).
    pub fn detail(&self) -> Option<&str> {
        match self {
            AppError::LoginRequired(d) => Some(d),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::StorageFull => AppError::DiskFull,
            std::io::ErrorKind::PermissionDenied => AppError::Permission(e.to_string()),
            _ if e.raw_os_error() == Some(112) => AppError::DiskFull, // ERROR_DISK_FULL
            _ => AppError::Io(e.to_string()),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        if let Some(s) = e.status() {
            return AppError::Http(s.as_u16());
        }
        // Strip the URL: media URLs can carry signed tokens and must not reach logs or UI.
        AppError::Network(e.without_url().to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorInfo {
    pub kind: String,
    pub message: String,
    pub detail: Option<String>,
    pub retryable: bool,
}

impl From<&AppError> for ErrorInfo {
    fn from(e: &AppError) -> Self {
        ErrorInfo { kind: e.kind().to_string(), message: e.to_string(), detail: e.detail().map(str::to_string), retryable: e.retryable() }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        ErrorInfo::from(self).serialize(s)
    }
}

pub type AppResult<T> = Result<T, AppError>;
