use thiserror::Error;

/// Errors that can occur in core Release Regent operations
#[derive(Error, Debug)]
pub enum CoreError {
    /// Configuration-related errors
    #[error("Configuration error: {message}")]
    Config { message: String },

    /// Version calculation errors
    #[error("Version calculation failed: {reason}")]
    Versioning { reason: String },

    /// Changelog generation errors
    #[error("Changelog generation failed: {message}")]
    ChangelogGeneration { message: String },

    /// Webhook processing errors
    #[error("Webhook processing failed: {stage} - {message}")]
    Webhook { stage: String, message: String },

    /// GitHub API integration errors
    #[error("GitHub operation failed: {source}")]
    GitHub {
        source: Box<release_regent_github_client::Error>,
    },

    /// I/O errors (file operations, network, etc.)
    #[error("I/O operation failed: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    /// YAML parsing errors
    #[error("YAML parsing failed: {source}")]
    YamlParsing {
        #[from]
        source: serde_yaml::Error,
    },

    /// Invalid input provided to core operations
    #[error("Invalid input: {field} - {message}")]
    InvalidInput { field: String, message: String },

    /// Operation not supported in current context
    #[error("Operation not supported: {operation} - {context}")]
    NotSupported { operation: String, context: String },

    /// Internal state inconsistency
    #[error("Internal state error: {message}")]
    InternalState { message: String },
}

impl CoreError {
    /// Create a new configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Create a new internal state error
    pub fn internal_state(message: impl Into<String>) -> Self {
        Self::InternalState {
            message: message.into(),
        }
    }

    /// Create a new invalid input error
    pub fn invalid_input(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidInput {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Create a new not supported error
    pub fn not_supported(operation: impl Into<String>, context: impl Into<String>) -> Self {
        Self::NotSupported {
            operation: operation.into(),
            context: context.into(),
        }
    }

    /// Create a new versioning error
    pub fn versioning(reason: impl Into<String>) -> Self {
        Self::Versioning {
            reason: reason.into(),
        }
    }

    /// Create a new changelog generation error
    pub fn changelog_generation(message: impl Into<String>) -> Self {
        Self::ChangelogGeneration {
            message: message.into(),
        }
    }

    /// Create a new webhook processing error
    pub fn webhook(stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Webhook {
            stage: stage.into(),
            message: message.into(),
        }
    }
}

impl From<release_regent_github_client::Error> for CoreError {
    fn from(error: release_regent_github_client::Error) -> Self {
        Self::GitHub {
            source: Box::new(error),
        }
    }
}

/// Result type for core operations
pub type CoreResult<T> = Result<T, CoreError>;

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;
