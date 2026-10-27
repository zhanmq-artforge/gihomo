//! Core domain and parsing error definitions.

use thiserror::Error;

/// Errors originating from core domain operations, parsing, and configuration merging.
#[derive(Error, Debug)]
#[must_use]
#[non_exhaustive]
pub enum CoreError {
    /// Standard I/O failures.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization errors.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// YAML parsing or serialization errors.
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// Malformed or unsupported subscription content / share link.
    #[error("Invalid subscription format: {0}")]
    InvalidSubscription(String),

    /// Errors encountered while synthesizing or merging Mihomo configurations.
    #[error("Configuration merge error: {0}")]
    ConfigMerge(String),

    /// Target subscription identifier does not exist.
    #[error("Subscription not found: {0}")]
    NotFound(String),
}
