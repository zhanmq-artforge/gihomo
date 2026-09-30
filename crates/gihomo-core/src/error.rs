use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Invalid subscription format: {0}")]
    InvalidSubscription(String),

    #[error("Configuration merge error: {0}")]
    ConfigMerge(String),

    #[error("Subscription not found: {0}")]
    NotFound(String),
}
