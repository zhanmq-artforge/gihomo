use gihomo_core::CoreError;
use gihomo_infra::InfraError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(transparent)]
    Infrastructure(#[from] InfraError),
    #[error("{0}")]
    Operation(String),
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
