//! Error types and handling for the configuration provider crate.

use release_regent_core::errors::CoreError;
use std::path::PathBuf;
use thiserror::Error;

/// Result type for configuration provider operations
pub type ConfigProviderResult<T> = Result<T, ConfigProviderError>;

/// Comprehensive error types for configuration provider operations
#[derive(Error, Debug)]
pub enum ConfigProviderError {
    /// File system related errors
    #[error("File system error: {message}")]
    FileSystem {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Configuration file not found
    #[error("Configuration file not found: {path}")]
    ConfigFileNotFound { path: PathBuf },

    /// Invalid configuration format
    #[error("Invalid configuration format in {path}: {reason}")]
    InvalidFormat { path: PathBuf, reason: String },

    /// Configuration parsing errors
    #[error("Failed to parse configuration file {path}: {reason}")]
    ParseError {
        path: PathBuf,
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Configuration validation errors
    #[error("Configuration validation failed for {path}: {errors:?}")]
    ValidationError { path: PathBuf, errors: Vec<String> },

    /// Unsupported format errors
    #[error("Unsupported configuration format: {format} for file {path}")]
    UnsupportedFormat { format: String, path: PathBuf },

    /// Permission denied errors
    #[error("Permission denied accessing {path}: {reason}")]
    PermissionDenied { path: PathBuf, reason: String },

    /// Configuration merging errors
    #[error("Failed to merge configurations: {reason}")]
    MergeError { reason: String },

    /// IO errors
    #[error("IO error: {message}")]
    Io {
        message: String,
        #[source]
        source: std::io::Error,
    },

    /// Schema validation errors
    #[error("Schema validation error: {message}")]
    SchemaValidation { message: String },

    /// Builder configuration errors
    #[error("Configuration builder error: {message}")]
    Builder { message: String },

    /// Core error passthrough
    #[error("Core error: {0}")]
    Core(#[from] CoreError),
}

impl ConfigProviderError {
    /// Create a new file system error
    pub fn file_system(message: impl Into<String>) -> Self {
        Self::FileSystem {
            message: message.into(),
            source: None,
        }
    }

    /// Create a new file system error with source
    pub fn file_system_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::FileSystem {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create a new parse error
    pub fn parse_error(path: PathBuf, reason: impl Into<String>) -> Self {
        Self::ParseError {
            path,
            reason: reason.into(),
            source: None,
        }
    }

    /// Create a new parse error with source
    pub fn parse_error_with_source(
        path: PathBuf,
        reason: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::ParseError {
            path,
            reason: reason.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create a new validation error
    pub fn validation_error(path: PathBuf, errors: Vec<String>) -> Self {
        Self::ValidationError { path, errors }
    }

    /// Create a new IO error
    pub fn io_error(message: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            message: message.into(),
            source,
        }
    }

    /// Create a new builder error
    pub fn builder_error(message: impl Into<String>) -> Self {
        Self::Builder {
            message: message.into(),
        }
    }
}

impl From<std::io::Error> for ConfigProviderError {
    fn from(error: std::io::Error) -> Self {
        Self::io_error("IO operation failed", error)
    }
}

impl From<serde_yaml::Error> for ConfigProviderError {
    fn from(error: serde_yaml::Error) -> Self {
        let message = format!("YAML parsing error: {}", error);
        Self::ParseError {
            path: PathBuf::new(), // Will be set by caller if available
            reason: message,
            source: Some(Box::new(error)),
        }
    }
}

impl From<toml::de::Error> for ConfigProviderError {
    fn from(error: toml::de::Error) -> Self {
        let message = format!("TOML parsing error: {}", error);
        Self::ParseError {
            path: PathBuf::new(), // Will be set by caller if available
            reason: message,
            source: Some(Box::new(error)),
        }
    }
}
