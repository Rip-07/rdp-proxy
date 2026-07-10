use thiserror::Error;

/// Application-level errors.
#[derive(Debug, Error)]
pub enum AppError {
    /// Configuration could not be loaded or parsed.
    #[error("configuration error: {0}")]
    Config(String),

    /// A runtime I/O failure occurred.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
