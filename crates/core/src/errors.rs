use thiserror::Error;

/// Error context information for better debugging and testing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorContext {
    /// Operation that was being performed
    pub operation: String,
    /// Component/module where error occurred
    pub component: String,
    /// Additional context data as key-value pairs
    pub context_data: std::collections::HashMap<String, String>,
    /// Error correlation ID for tracing
    pub correlation_id: Option<String>,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new(operation: impl Into<String>, component: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            component: component.into(),
            context_data: std::collections::HashMap::new(),
            correlation_id: None,
        }
    }

    /// Add context data
    pub fn with_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context_data.insert(key.into(), value.into());
        self
    }

    /// Add correlation ID for tracing
    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }
}

/// Errors that can occur in core Release Regent operations
#[derive(Error, Debug)]
pub enum CoreError {
    /// Authentication/authorization errors
    #[error("Authentication error: {message}")]
    Authentication {
        message: String,
        context: Option<ErrorContext>,
    },

    /// Optimistic-lock / concurrent-modification conflict
    ///
    /// Returned when a GitHub API update is rejected because the resource was
    /// modified concurrently (e.g. HTTP 412 Precondition Failed or HTTP 422
    /// branch-already-exists).  The caller should re-fetch the resource and
    /// retry the operation.
    #[error("Conflict on {resource}: resource was modified concurrently")]
    Conflict {
        /// Human-readable description of the conflicting resource
        resource: String,
        context: Option<ErrorContext>,
    },

    /// Changelog generation errors
    #[error("Changelog generation failed: {message}")]
    ChangelogGeneration {
        message: String,
        context: Option<ErrorContext>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Configuration-related errors
    #[error("Configuration error: {message}")]
    Config {
        message: String,
        context: Option<ErrorContext>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// GitHub API integration errors
    #[error("GitHub operation failed: {source}")]
    GitHub {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
        context: Option<ErrorContext>,
    },

    /// Internal state inconsistency
    #[error("Internal state error: {message}")]
    InternalState {
        message: String,
        context: Option<ErrorContext>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Invalid input provided to core operations
    #[error("Invalid input: {field} - {message}")]
    InvalidInput {
        field: String,
        message: String,
        context: Option<ErrorContext>,
    },

    /// I/O errors (file operations, network, etc.)
    #[error("I/O operation failed: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    /// JSON parsing errors
    #[error("JSON parsing failed: {source}")]
    JsonParsing {
        #[from]
        source: serde_json::Error,
    },

    /// Network-related errors
    #[error("Network error: {message}")]
    Network {
        message: String,
        context: Option<ErrorContext>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Resource not found
    #[error("Not found: {resource}")]
    NotFound {
        resource: String,
        context: Option<ErrorContext>,
    },

    /// Operation not supported in current context
    #[error("Operation not supported: {operation} - {context}")]
    NotSupported {
        operation: String,
        context: String,
        error_context: Option<ErrorContext>,
    },

    /// Rate limiting errors
    #[error("Rate limit exceeded: {message}")]
    RateLimit {
        message: String,
        retry_after_seconds: Option<u64>,
        context: Option<ErrorContext>,
    },

    /// Timeout errors
    #[error("Operation timed out: {operation} after {duration_ms}ms")]
    Timeout {
        operation: String,
        duration_ms: u64,
        context: Option<ErrorContext>,
    },

    /// TOML parsing errors
    #[error("TOML parsing failed: {source}")]
    TomlParsing {
        #[from]
        source: toml::de::Error,
    },

    /// Validation errors
    #[error("Validation failed: {field} - {message}")]
    Validation {
        field: String,
        message: String,
        context: Option<ErrorContext>,
    },

    /// Version calculation errors
    #[error("Version calculation failed: {reason}")]
    Versioning {
        reason: String,
        context: Option<ErrorContext>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Webhook processing errors
    #[error("Webhook processing failed: {stage} - {message}")]
    Webhook {
        stage: String,
        message: String,
        context: Option<ErrorContext>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// YAML parsing errors
    #[error("YAML parsing failed: {source}")]
    YamlParsing {
        #[from]
        source: serde_yaml::Error,
    },
}

impl CoreError {
    /// Create a new configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            context: None,
            source: None,
        }
    }

    /// Create a new configuration error with context
    pub fn config_with_context(message: impl Into<String>, context: ErrorContext) -> Self {
        Self::Config {
            message: message.into(),
            context: Some(context),
            source: None,
        }
    }

    /// Create a new configuration error with source
    pub fn config_with_source(
        message: impl Into<String>,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        Self::Config {
            message: message.into(),
            context: None,
            source: Some(source),
        }
    }

    /// Create a new internal state error
    pub fn internal_state(message: impl Into<String>) -> Self {
        Self::InternalState {
            message: message.into(),
            context: None,
            source: None,
        }
    }

    /// Create a new internal state error with context
    pub fn internal_state_with_context(message: impl Into<String>, context: ErrorContext) -> Self {
        Self::InternalState {
            message: message.into(),
            context: Some(context),
            source: None,
        }
    }

    /// Create a new invalid input error
    pub fn invalid_input(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidInput {
            field: field.into(),
            message: message.into(),
            context: None,
        }
    }

    /// Create a new invalid input error with context
    pub fn invalid_input_with_context(
        field: impl Into<String>,
        message: impl Into<String>,
        context: ErrorContext,
    ) -> Self {
        Self::InvalidInput {
            field: field.into(),
            message: message.into(),
            context: Some(context),
        }
    }

    /// Create a new not supported error
    pub fn not_supported(operation: impl Into<String>, context: impl Into<String>) -> Self {
        Self::NotSupported {
            operation: operation.into(),
            context: context.into(),
            error_context: None,
        }
    }

    /// Create a new not supported error with context
    pub fn not_supported_with_context(
        operation: impl Into<String>,
        context: impl Into<String>,
        error_context: ErrorContext,
    ) -> Self {
        Self::NotSupported {
            operation: operation.into(),
            context: context.into(),
            error_context: Some(error_context),
        }
    }

    /// Create a new versioning error
    pub fn versioning(reason: impl Into<String>) -> Self {
        Self::Versioning {
            reason: reason.into(),
            context: None,
            source: None,
        }
    }

    /// Create a new versioning error with context
    pub fn versioning_with_context(reason: impl Into<String>, context: ErrorContext) -> Self {
        Self::Versioning {
            reason: reason.into(),
            context: Some(context),
            source: None,
        }
    }

    /// Create a new changelog generation error
    pub fn changelog_generation(message: impl Into<String>) -> Self {
        Self::ChangelogGeneration {
            message: message.into(),
            context: None,
            source: None,
        }
    }

    /// Create a new changelog generation error with context
    pub fn changelog_generation_with_context(
        message: impl Into<String>,
        context: ErrorContext,
    ) -> Self {
        Self::ChangelogGeneration {
            message: message.into(),
            context: Some(context),
            source: None,
        }
    }

    /// Create a new webhook processing error
    pub fn webhook(stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Webhook {
            stage: stage.into(),
            message: message.into(),
            context: None,
            source: None,
        }
    }

    /// Create a new webhook processing error with context
    pub fn webhook_with_context(
        stage: impl Into<String>,
        message: impl Into<String>,
        context: ErrorContext,
    ) -> Self {
        Self::Webhook {
            stage: stage.into(),
            message: message.into(),
            context: Some(context),
            source: None,
        }
    }

    /// Create a new validation error
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
            context: None,
        }
    }

    /// Create a new validation error with context
    pub fn validation_with_context(
        field: impl Into<String>,
        message: impl Into<String>,
        context: ErrorContext,
    ) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
            context: Some(context),
        }
    }

    /// Create a new timeout error
    pub fn timeout(operation: impl Into<String>, duration_ms: u64) -> Self {
        Self::Timeout {
            operation: operation.into(),
            duration_ms,
            context: None,
        }
    }

    /// Create a new network error
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network {
            message: message.into(),
            context: None,
            source: None,
        }
    }

    /// Create a new not found error
    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::NotFound {
            resource: resource.into(),
            context: None,
        }
    }

    /// Create a conflict (optimistic-lock) error
    ///
    /// Use when a GitHub API update is rejected because the resource was
    /// modified concurrently (branch already exists, ETag mismatch, etc.).
    pub fn conflict(resource: impl Into<String>) -> Self {
        Self::Conflict {
            resource: resource.into(),
            context: None,
        }
    }

    /// Create a conflict error with context
    pub fn conflict_with_context(resource: impl Into<String>, context: ErrorContext) -> Self {
        Self::Conflict {
            resource: resource.into(),
            context: Some(context),
        }
    }

    /// Create a new authentication error
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
            context: None,
        }
    }

    /// Create a new rate limit error
    pub fn rate_limit(message: impl Into<String>) -> Self {
        Self::RateLimit {
            message: message.into(),
            retry_after_seconds: None,
            context: None,
        }
    }

    /// Create a new rate limit error with retry info
    pub fn rate_limit_with_retry(message: impl Into<String>, retry_after_seconds: u64) -> Self {
        Self::RateLimit {
            message: message.into(),
            retry_after_seconds: Some(retry_after_seconds),
            context: None,
        }
    }

    /// Create a new GitHub error from any error source
    pub fn github<E>(error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::GitHub {
            source: Box::new(error),
            context: None,
        }
    }

    /// Create a new GitHub error with context
    pub fn github_with_context<E>(error: E, context: ErrorContext) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::GitHub {
            source: Box::new(error),
            context: Some(context),
        }
    }

    /// Get the error context if available
    pub fn context(&self) -> Option<&ErrorContext> {
        match self {
            Self::Config { context, .. }
            | Self::Versioning { context, .. }
            | Self::ChangelogGeneration { context, .. }
            | Self::Webhook { context, .. }
            | Self::GitHub { context, .. }
            | Self::InvalidInput { context, .. }
            | Self::InternalState { context, .. }
            | Self::Validation { context, .. }
            | Self::Timeout { context, .. }
            | Self::Network { context, .. }
            | Self::Authentication { context, .. }
            | Self::Conflict { context, .. }
            | Self::RateLimit { context, .. } => context.as_ref(),
            Self::NotFound { context, .. } => context.as_ref(),
            Self::NotSupported { error_context, .. } => error_context.as_ref(),
            _ => None,
        }
    }

    /// Returns `true` for transient errors that are safe to retry after a back-off delay.
    ///
    /// # Retryable variants
    ///
    /// | Variant | Reason |
    /// |---------|--------|
    /// | [`Self::Network`] | Connection or transport failure; the remote may recover. |
    /// | [`Self::RateLimit`] | API quota exceeded; back off until `retry_after_seconds`. |
    /// | [`Self::Timeout`] | Operation timed out; a fresh attempt may succeed. |
    /// | [`Self::Conflict`] | Optimistic-lock collision (ETag mismatch / branch already exists); re-fetch the resource and retry. |
    ///
    /// All other variants represent permanent errors that will not resolve by retrying:
    /// configuration mistakes, bad input, auth failures, or parse errors.
    ///
    /// # Spec note
    ///
    /// `docs/specs/design/error-handling.md` lists "Authentication token expiration" under
    /// transient errors.  In this codebase every `Authentication` error originates from
    /// a permanent credential failure (401/403 from the GitHub API or a missing/invalid
    /// private key) rather than a short-lived token clock skew, so `Authentication` is
    /// classified as non-retryable here.  If a future variant specifically models token
    /// expiry that should be retried after re-authentication, add a dedicated variant
    /// rather than changing the blanket `Authentication` classification.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Network { .. }
                | Self::RateLimit { .. }
                | Self::Timeout { .. }
                // Conflict signals a concurrent modification; the caller should
                // re-fetch and retry (see module-level doc comment).
                | Self::Conflict { .. }
        )
    }

    /// Returns the number of seconds to wait before retrying, if a hint is available.
    ///
    /// | Variant | Delay |
    /// |---------|-------|
    /// | [`Self::RateLimit`] with `retry_after_seconds` | The value of `retry_after_seconds` (caller-supplied hint). |
    /// | [`Self::RateLimit`] without hint | `None` — caller should use its own back-off. |
    /// | [`Self::Network`] | `Some(1)` — conservative 1-second default. |
    /// | [`Self::Timeout`] | `Some(2)` — slightly longer default for timed-out operations. |
    /// | [`Self::Conflict`] | `None` — re-fetch and retry immediately (no prescribed delay). |
    /// | All other variants | `None` — non-retryable; delay is not applicable. |
    pub fn retry_delay_seconds(&self) -> Option<u64> {
        match self {
            Self::RateLimit {
                retry_after_seconds,
                ..
            } => *retry_after_seconds,
            Self::Network { .. } => Some(1), // Default 1 second for network errors
            Self::Timeout { .. } => Some(2), // Default 2 seconds for timeout errors
            // Conflict is retryable (re-fetch and retry), but the caller should
            // retry immediately after re-fetching rather than waiting a fixed delay.
            Self::Conflict { .. } => None,
            _ => None,
        }
    }
}

/// Result type for core operations
pub type CoreResult<T> = Result<T, CoreError>;

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;
