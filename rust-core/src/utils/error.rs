use thiserror::Error;

pub type Result<T> = std::result::Result<T, AudioError>;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("I/O error: {0}")]
    IoError(String),

    #[error("Decode error: {0}")]
    DecodeError(String),

    #[error("Encode error: {0}")]
    EncodeError(String),

    #[error("Output error: {0}")]
    OutputError(String),

    #[error("Input error: {0}")]
    InputError(String),

    #[error("DSP error: {0}")]
    DSPError(String),

    #[error("Resample error: {0}")]
    ResampleError(String),

    #[error("Bit-Perfect error: {0}")]
    BitPerfectError(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Cancelled")]
    Cancelled,

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<std::io::Error> for AudioError {
    fn from(err: std::io::Error) -> Self {
        AudioError::IoError(err.to_string())
    }
}

impl From<symphonia::core::errors::Error> for AudioError {
    fn from(err: symphonia::core::errors::Error) -> Self {
        AudioError::DecodeError(err.to_string())
    }
}

impl From<cpal::StreamError> for AudioError {
    fn from(err: cpal::StreamError) -> Self {
        AudioError::OutputError(err.to_string())
    }
}

impl From<cpal::DefaultStreamConfigError> for AudioError {
    fn from(err: cpal::DefaultStreamConfigError) -> Self {
        AudioError::OutputError(err.to_string())
    }
}

impl From<cpal::BuildStreamError> for AudioError {
    fn from(err: cpal::BuildStreamError) -> Self {
        AudioError::OutputError(err.to_string())
    }
}

impl From<cpal::SupportedStreamConfigsError> for AudioError {
    fn from(err: cpal::SupportedStreamConfigsError) -> Self {
        AudioError::OutputError(err.to_string())
    }
}

impl From<config::ConfigError> for AudioError {
    fn from(err: config::ConfigError) -> Self {
        AudioError::IoError(err.to_string())
    }
}

#[cfg(feature = "rusqlite")]
impl From<rusqlite::Error> for AudioError {
    fn from(err: rusqlite::Error) -> Self {
        AudioError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for AudioError {
    fn from(err: serde_json::Error) -> Self {
        AudioError::DecodeError(err.to_string())
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for AudioError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        AudioError::IoError(err.to_string())
    }
}
