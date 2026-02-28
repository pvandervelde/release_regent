//! Error types for GitHub client operations

use thiserror::Error;

/// Result type for GitHub client operations
pub type GitHubResult<T> = Result<T, Error>;

/// Errors that can occur during GitHub operations
#[derive(Debug, Error)]
pub enum Error {
    /// GitHub API error
    #[error("GitHub API error: {message}")]
    Api {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Authentication error
    #[error("Authentication error: {message}")]
    Auth {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Network error
    #[error("Network error: {message}")]
    Network {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Resource not found
    #[error("Resource not found: {resource}")]
    NotFound { resource: String },

    /// Invalid input
    #[error("Invalid input: {message}")]
    InvalidInput { message: String },

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimit,

    /// Other error
    #[error("GitHub client error: {message}")]
    Other {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl From<github_bot_sdk::error::ApiError> for Error {
    fn from(err: github_bot_sdk::error::ApiError) -> Self {
        Error::Api {
            message: err.to_string(),
            source: Some(Box::new(err)),
        }
    }
}

impl From<github_bot_sdk::error::AuthError> for Error {
    fn from(err: github_bot_sdk::error::AuthError) -> Self {
        Error::Auth {
            message: err.to_string(),
            source: Some(Box::new(err)),
        }
    }
}

impl From<Error> for release_regent_core::CoreError {
    fn from(err: Error) -> Self {
        match err {
            Error::Api { message, source } => release_regent_core::CoreError::GitHub {
                source: source.unwrap_or_else(|| {
                    Box::new(std::io::Error::new(std::io::ErrorKind::Other, message))
                }),
                context: None,
            },
            Error::Auth { message, source } => release_regent_core::CoreError::GitHub {
                source: source.unwrap_or_else(|| {
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        message,
                    ))
                }),
                context: None,
            },
            Error::Network { message, source } => release_regent_core::CoreError::Network {
                message,
                source,
                context: None,
            },
            Error::NotFound { resource } => release_regent_core::CoreError::GitHub {
                source: Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, resource)),
                context: None,
            },
            Error::InvalidInput { message } => release_regent_core::CoreError::InvalidInput {
                field: "unknown".to_string(),
                message,
                context: None,
            },
            Error::RateLimit => release_regent_core::CoreError::RateLimit {
                message: "Rate limit exceeded".to_string(),
                retry_after_seconds: None,
                context: None,
            },
            Error::Other { message, source } => release_regent_core::CoreError::GitHub {
                source: source.unwrap_or_else(|| {
                    Box::new(std::io::Error::new(std::io::ErrorKind::Other, message))
                }),
                context: None,
            },
        }
    }
}

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;
