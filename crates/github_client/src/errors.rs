use thiserror::Error;

/// Errors that can occur when interacting with GitHub API
#[derive(Error, Debug)]
pub enum GitHubError {
    /// Authentication-related errors
    #[error("GitHub authentication failed: {message}")]
    Authentication { message: String },

    /// API request errors
    #[error("GitHub API request failed: {status} - {message}")]
    ApiRequest { status: u16, message: String },

    /// Rate limiting errors
    #[error("GitHub API rate limit exceeded. Reset at: {reset_time}")]
    RateLimit { reset_time: String },

    /// Network connectivity errors
    #[error("Network error when connecting to GitHub: {source}")]
    Network {
        #[from]
        source: reqwest::Error,
    },

    /// JSON parsing errors
    #[error("Failed to parse GitHub API response: {source}")]
    Parsing {
        #[from]
        source: serde_json::Error,
    },

    /// Octocrab library errors
    #[error("Octocrab error: {source}")]
    Octocrab {
        #[from]
        source: octocrab::Error,
    },

    /// Resource not found errors
    #[error("GitHub resource not found: {resource_type} '{resource_id}'")]
    NotFound {
        resource_type: String,
        resource_id: String,
    },

    /// Permission denied errors
    #[error("Insufficient permissions for GitHub operation: {operation}")]
    PermissionDenied { operation: String },

    /// Invalid input provided to GitHub API
    #[error("Invalid input for GitHub API: {field} - {message}")]
    InvalidInput { field: String, message: String },
}

impl GitHubError {
    /// Create a new authentication error
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
        }
    }

    /// Create a new API request error
    pub fn api_request(status: u16, message: impl Into<String>) -> Self {
        Self::ApiRequest {
            status,
            message: message.into(),
        }
    }

    /// Create a new rate limit error
    pub fn rate_limit(reset_time: impl Into<String>) -> Self {
        Self::RateLimit {
            reset_time: reset_time.into(),
        }
    }

    /// Create a new not found error
    pub fn not_found(resource_type: impl Into<String>, resource_id: impl Into<String>) -> Self {
        Self::NotFound {
            resource_type: resource_type.into(),
            resource_id: resource_id.into(),
        }
    }

    /// Create a new permission denied error
    pub fn permission_denied(operation: impl Into<String>) -> Self {
        Self::PermissionDenied {
            operation: operation.into(),
        }
    }

    /// Create a new invalid input error
    pub fn invalid_input(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidInput {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// Result type for GitHub operations
pub type GitHubResult<T> = Result<T, GitHubError>;

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;
