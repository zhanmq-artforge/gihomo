//! Infrastructure layer error definitions.

use thiserror::Error;

/// Errors originating from external services, system calls, GSettings, kernel management, and API clients.
#[derive(Error, Debug)]
#[must_use]
#[non_exhaustive]
pub enum InfraError {
    /// Errors originating from core domain.
    #[error("Core error: {0}")]
    Core(#[from] gihomo_core::CoreError),

    /// Standard I/O failures.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP request failures.
    #[error("Network request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization failures.
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// Kernel process execution, polkit, or supervision errors.
    #[error("Kernel process error: {0}")]
    Kernel(String),

    /// GNOME GSettings proxy configuration errors.
    #[error("GSettings error: {0}")]
    GSettings(String),

    /// Subscription or resource download failures with HTTP status.
    #[error("Subscription download failed with status {status}: {message}")]
    DownloadFailed { status: u16, message: String },

    /// Mihomo external controller API returned an error status.
    #[error("Mihomo API error ({status}): {message}")]
    ApiError { status: u16, message: String },

    /// WebSocket connection or decoding error.
    #[error("WebSocket error: {0}")]
    WebSocket(String),
}
