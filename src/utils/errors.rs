use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    HttpRequestError(#[from] reqwest::Error),

    #[error("Configuration error: {0}")]
    ConfigError(#[from] config::ConfigError),

    #[error("Environment variable error: {0}")]
    EnvVarError(#[from] std::env::VarError),

    #[error("Other error: {0}")]
    Other(String),
}
