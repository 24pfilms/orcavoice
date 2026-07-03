use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Audio error: {0}")]
    Audio(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("History error: {0}")]
    History(String),
    #[error("Hotkey error: {0}")]
    Hotkey(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Output error: {0}")]
    Output(String),
    #[error("Provider error: {0}")]
    Provider(String),
    #[error("Secret error: {0}")]
    Secret(String),
}

impl From<AppError> for String {
    fn from(value: AppError) -> Self {
        value.to_string()
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        AppError::Config(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        AppError::Config(value.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        AppError::Network(value.to_string())
    }
}
