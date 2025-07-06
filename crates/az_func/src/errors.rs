use thiserror::Error;

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;

/// Errors that can occur in Azure Function operations
#[derive(Error, Debug)]
#[allow(dead_code)] // Allow during foundation phase
pub enum Error {
    /// Authentication errors
    #[error("Authentication failed: {message}")]
    Authentication { message: String },

    /// Azure identity errors
    #[error("Azure identity error: {message}")]
    AzureIdentity { message: String },

    /// Azure Key Vault errors
    #[error("Azure Key Vault error: {message}")]
    AzureKeyVault { message: String },

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

#[allow(dead_code)] // Allow during foundation phase
impl Error {
    /// Create a new authentication error
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
        }
    }

    /// Create a new Azure identity error
    pub fn azure_identity(message: impl Into<String>) -> Self {
        Self::AzureIdentity {
            message: message.into(),
        }
    }

    /// Create a new Azure Key Vault error
    pub fn azure_key_vault(message: impl Into<String>) -> Self {
        Self::AzureKeyVault {
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

    /// Create a new HTTP request error
    pub fn http_request(status: u16, message: impl Into<String>) -> Self {
        Self::HttpRequest {
            status,
            message: message.into(),
        }
    }

    /// Create a new internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Create a new parse error
    pub fn parse(message: impl Into<String>) -> Self {
        Self::Parse {
            message: message.into(),
        }
    }
}

/// Result type for function operations
pub type FunctionResult<T> = Result<T, Error>;
