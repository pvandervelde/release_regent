use thiserror::Error;

/// Errors that can occur in Azure Function operations
#[derive(Error, Debug)]
pub enum Error {
    /// Authentication errors
    #[error("Authentication failed: {message}")]
    Authentication { message: String },

    /// Azure Functions runtime errors
    #[error("Azure Functions runtime error: {source}")]
    AzureFunctions {
        #[from]
        source: azure_functions::error::Error,
    },

    /// Core operation errors
    #[error("Core operation failed: {source}")]
    Core {
        #[from]
        source: release_regent_core::CoreError,
    },

    /// Environment configuration errors
    #[error("Environment configuration error: {variable} - {message}")]
    Environment { variable: String, message: String },

    /// GitHub client errors
    #[error("GitHub operation failed: {source}")]
    GitHub {
        #[from]
        source: release_regent_github_client::GitHubError,
    },

    /// HTTP request processing errors
    #[error("HTTP request error: {status} - {message}")]
    HttpRequest { status: u16, message: String },

    /// Internal processing errors
    #[error("Internal processing error: {message}")]
    Internal { message: String },

    /// JSON processing errors
    #[error("JSON processing failed: {source}")]
    Json {
        #[from]
        source: serde_json::Error,
    },

    #[error("Failed to parse the file")]
    Parse { message: String },
}

impl Error {
    /// Create a new HTTP request error
    pub fn http_request(status: u16, message: impl Into<String>) -> Self {
        Self::HttpRequest {
            status,
            message: message.into(),
        }
    }

    /// Create a new authentication error
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
        }
    }

    /// Create a new environment error
    pub fn environment(variable: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Environment {
            variable: variable.into(),
            message: message.into(),
        }
    }

    /// Create a new internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}

/// Result type for function operations
pub type FunctionResult<T> = Result<T, Error>;

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;
