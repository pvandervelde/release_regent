use thiserror::Error;

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;

/// Errors that can occur in webhook server operations
#[derive(Error, Debug)]
pub enum Error {
    /// Authentication errors.
    ///
    /// Reserved for Task 2.0 (Azure AD credential management). No active code
    /// paths produce this variant yet.
    #[allow(dead_code)]
    #[error("Authentication failed: {message}")]
    Authentication { message: String },

    /// Azure identity errors.
    ///
    /// Reserved for Task 2.0 (Azure AD credential management). No active code
    /// paths produce this variant yet.
    #[allow(dead_code)]
    #[error("Azure identity error: {message}")]
    AzureIdentity { message: String },

    /// Azure Key Vault errors.
    ///
    /// Reserved for Task 2.0 (Azure Key Vault secret provider). No active code
    /// paths produce this variant yet.
    #[allow(dead_code)]
    #[error("Azure Key Vault error: {message}")]
    AzureKeyVault { message: String },

    /// Configuration provider errors
    #[error("Configuration provider error: {message}")]
    ConfigProvider { message: String },

    /// Core operation errors
    #[error("Core operation failed: {source}")]
    Core {
        #[from]
        source: release_regent_core::CoreError,
    },

    /// Environment configuration errors
    #[error("Environment configuration error: {variable} - {message}")]
    Environment { variable: String, message: String },

    /// GitHub client errors returned directly from [`release_regent_github_client`].
    ///
    /// Note: most GitHub errors reach the server as [`Error::Core`] (wrapped in a
    /// `CoreError::GitHub`) because `GitHubClient::from_config` returns a `CoreResult`.
    /// This variant is reserved for future code paths that return a
    /// `github_client::Error` directly without the `Core` wrapper.
    #[error("GitHub operation failed: {source}")]
    GitHub {
        #[from]
        source: release_regent_github_client::Error,
    },

    /// Internal processing errors
    #[error("Internal processing error: {message}")]
    Internal { message: String },

    /// A repository allow-list or exclude-list entry failed to compile as a glob pattern.
    ///
    /// `list_name` identifies which configuration list the offending pattern came
    /// from (e.g. `"allowed_repos"` or `"excluded_repos"`), so operators can tell
    /// at a glance which environment variable / config key to fix. Produced by
    /// [`crate::handler::compile_repo_patterns`] and propagated out of `main` to
    /// abort server startup (BA-71, BA-75).
    #[error("Invalid glob pattern in {list_name}: '{pattern}' - {message}")]
    InvalidRepoPattern {
        list_name: String,
        pattern: String,
        message: String,
    },

    /// JSON processing errors
    #[error("JSON processing failed: {source}")]
    Json {
        #[from]
        source: serde_json::Error,
    },
}

impl Error {
    /// Create a new authentication error.
    ///
    /// Reserved for Task 2.0. Not called by any active production code path.
    #[allow(dead_code)]
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
        }
    }

    /// Create a new Azure identity error.
    ///
    /// Reserved for Task 2.0. Not called by any active production code path.
    #[allow(dead_code)]
    pub fn azure_identity(message: impl Into<String>) -> Self {
        Self::AzureIdentity {
            message: message.into(),
        }
    }

    /// Create a new Azure Key Vault error.
    ///
    /// Reserved for Task 2.0. Not called by any active production code path.
    #[allow(dead_code)]
    pub fn azure_key_vault(message: impl Into<String>) -> Self {
        Self::AzureKeyVault {
            message: message.into(),
        }
    }

    /// Create a new configuration provider error
    pub fn config_provider(message: impl Into<String>) -> Self {
        Self::ConfigProvider {
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

    /// Create a new invalid-repo-pattern error.
    ///
    /// - `list_name` — which configuration list the pattern came from (e.g.
    ///   `"allowed_repos"` or `"excluded_repos"`).
    /// - `pattern` — the raw (pre-lowercasing) pattern string that failed to compile.
    /// - `message` — the underlying `glob::PatternError` description.
    pub fn invalid_repo_pattern(
        list_name: impl Into<String>,
        pattern: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::InvalidRepoPattern {
            list_name: list_name.into(),
            pattern: pattern.into(),
            message: message.into(),
        }
    }

    // Note: `parse` and `http_request` constructors removed — the `Parse` and
    // `HttpRequest` variants were unused in all production code paths and have
    // been deleted from the enum.
}
