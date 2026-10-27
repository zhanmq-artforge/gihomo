//! Application service error definitions.

use gihomo_core::CoreError;
use gihomo_infra::InfraError;
use thiserror::Error;

/// High-level errors originating from application services and orchestration workflows.
#[derive(Debug, Error)]
#[must_use]
#[non_exhaustive]
pub enum AppError {
    /// Domain-level errors propagated from `gihomo-core`.
    #[error(transparent)]
    Core(#[from] CoreError),

    /// Infrastructure errors propagated from `gihomo-infra`.
    #[error(transparent)]
    Infrastructure(#[from] InfraError),

    /// High-level operation failure message.
    #[error("{0}")]
    Operation(String),
}

impl AppError {
    /// Creates a new operational error from any displayable message string.
    pub fn operation(message: impl Into<String>) -> Self {
        Self::Operation(message.into())
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self::Operation(message)
    }
}

impl From<&str> for AppError {
    fn from(message: &str) -> Self {
        Self::Operation(message.to_string())
    }
}
