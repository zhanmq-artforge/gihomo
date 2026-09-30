use thiserror::Error;

#[derive(Error, Debug)]
pub enum InfraError {
    #[error("Core error: {0}")]
    Core(#[from] gihomo_core::CoreError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Kernel process error: {0}")]
    Kernel(String),

    #[error("GSettings error: {0}")]
    GSettings(String),

    #[error("Subscription download failed with status {0}: {1}")]
    DownloadFailed(u16, String),

    #[error("Mihomo API error ({status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("WebSocket error: {0}")]
    WebSocket(String),
}
