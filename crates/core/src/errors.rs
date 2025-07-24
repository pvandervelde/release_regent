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
    /// Configuration-related errors
    #[error("Configuration error: {message}")]
    Config {
        message: String,
        context: Option<ErrorContext>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Version calculation errors
    #[error("Version calculation failed: {reason}")]
    Versioning {
        reason: String,
        context: Option<ErrorContext>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Changelog generation errors
    #[error("Changelog generation failed: {message}")]
    ChangelogGeneration {
        message: String,
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

    /// GitHub API integration errors
    #[error("GitHub operation failed: {source}")]
    GitHub {
        #[source]
        source: Box<release_regent_github_client::Error>,
        context: Option<ErrorContext>,
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

    /// TOML parsing errors
    #[error("TOML parsing failed: {source}")]
    TomlParsing {
        #[from]
        source: toml::de::Error,
    },

    /// JSON parsing errors
    #[error("JSON parsing failed: {source}")]
    JsonParsing {
        #[from]
        source: serde_json::Error,
    },

    /// Invalid input provided to core operations
    #[error("Invalid input: {field} - {message}")]
    InvalidInput {
        field: String,
        message: String,
        context: Option<ErrorContext>,
    },

    /// Operation not supported in current context
    #[error("Operation not supported: {operation} - {context}")]
    NotSupported {
        operation: String,
        context: String,
        error_context: Option<ErrorContext>,
    },

    /// Internal state inconsistency
    #[error("Internal state error: {message}")]
    InternalState {
        message: String,
        context: Option<ErrorContext>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Validation errors
    #[error("Validation failed: {field} - {message}")]
    Validation {
        field: String,
        message: String,
        context: Option<ErrorContext>,
    },

    /// Timeout errors
    #[error("Operation timed out: {operation} after {duration_ms}ms")]
    Timeout {
        operation: String,
        duration_ms: u64,
        context: Option<ErrorContext>,
    },

    /// Network-related errors
    #[error("Network error: {message}")]
    Network {
        message: String,
        context: Option<ErrorContext>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Authentication/authorization errors
    #[error("Authentication error: {message}")]
    Authentication {
        message: String,
        context: Option<ErrorContext>,
    },

    /// Rate limiting errors
    #[error("Rate limit exceeded: {message}")]
    RateLimit {
        message: String,
        retry_after_seconds: Option<u64>,
        context: Option<ErrorContext>,
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
            | Self::RateLimit { context, .. } => context.as_ref(),
            Self::NotSupported { error_context, .. } => error_context.as_ref(),
            _ => None,
        }
    }

    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Network { .. } | Self::RateLimit { .. } | Self::Timeout { .. }
        )
    }

    /// Get retry delay in seconds if applicable
    pub fn retry_delay_seconds(&self) -> Option<u64> {
        match self {
            Self::RateLimit {
                retry_after_seconds,
                ..
            } => *retry_after_seconds,
            Self::Network { .. } => Some(1), // Default 1 second for network errors
            Self::Timeout { .. } => Some(2), // Default 2 seconds for timeout errors
            _ => None,
        }
    }
}

impl From<release_regent_github_client::Error> for CoreError {
    fn from(error: release_regent_github_client::Error) -> Self {
        Self::GitHub {
            source: Box::new(error),
            context: None,
        }
    }
}

/// Result type for core operations
pub type CoreResult<T> = Result<T, CoreError>;

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_creation() {
        let error = CoreError::config("Test config error".to_string());
        match error {
            CoreError::Config {
                message,
                context,
                source,
            } => {
                assert_eq!(message, "Test config error");
                assert!(context.is_none());
                assert!(source.is_none());
            }
            _ => panic!("Expected Config error"),
        }
    }

    #[test]
    fn test_versioning_error_creation() {
        let error = CoreError::versioning("Test versioning error".to_string());
        match error {
            CoreError::Versioning {
                reason,
                context,
                source,
            } => {
                assert_eq!(reason, "Test versioning error");
                assert!(context.is_none());
                assert!(source.is_none());
            }
            _ => panic!("Expected Versioning error"),
        }
    }

    #[test]
    fn test_changelog_generation_error_creation() {
        let error = CoreError::changelog_generation("Test error message".to_string());
        match error {
            CoreError::ChangelogGeneration {
                message,
                context,
                source,
            } => {
                assert_eq!(message, "Test error message");
                assert!(context.is_none());
                assert!(source.is_none());
            }
            _ => panic!("Expected ChangelogGeneration error"),
        }
    }

    #[test]
    fn test_webhook_error_creation() {
        let error = CoreError::webhook("Test stage".to_string(), "Test webhook error".to_string());
        match error {
            CoreError::Webhook {
                stage,
                message,
                context,
                source,
            } => {
                assert_eq!(stage, "Test stage");
                assert_eq!(message, "Test webhook error");
                assert!(context.is_none());
                assert!(source.is_none());
            }
            _ => panic!("Expected Webhook error"),
        }
    }

    #[test]
    fn test_invalid_input_error_creation() {
        let error = CoreError::invalid_input("field1".to_string(), "Invalid value".to_string());
        match error {
            CoreError::InvalidInput {
                field,
                message,
                context,
            } => {
                assert_eq!(field, "field1");
                assert_eq!(message, "Invalid value");
                assert!(context.is_none());
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_not_supported_error_creation() {
        let error =
            CoreError::not_supported("some_operation".to_string(), "some_context".to_string());
        match error {
            CoreError::NotSupported {
                operation,
                context,
                error_context,
            } => {
                assert_eq!(operation, "some_operation");
                assert_eq!(context, "some_context");
                assert!(error_context.is_none());
            }
            _ => panic!("Expected NotSupported error"),
        }
    }

    #[test]
    fn test_internal_state_error_creation() {
        let error = CoreError::internal_state("Inconsistent state".to_string());
        match error {
            CoreError::InternalState {
                message,
                context,
                source,
            } => {
                assert_eq!(message, "Inconsistent state");
                assert!(context.is_none());
                assert!(source.is_none());
            }
            _ => panic!("Expected InternalState error"),
        }
    }

    #[test]
    fn test_error_context_creation() {
        let context = ErrorContext::new("test_operation", "test_component")
            .with_data("key1", "value1")
            .with_correlation_id("test-123");

        assert_eq!(context.operation, "test_operation");
        assert_eq!(context.component, "test_component");
        assert_eq!(
            context.context_data.get("key1"),
            Some(&"value1".to_string())
        );
        assert_eq!(context.correlation_id, Some("test-123".to_string()));
    }

    #[test]
    fn test_error_with_context() {
        let context =
            ErrorContext::new("config_load", "config_provider").with_data("file", "config.yaml");

        let error = CoreError::config_with_context("Failed to load config", context.clone());

        let retrieved_context = error.context().unwrap();
        assert_eq!(retrieved_context.operation, "config_load");
        assert_eq!(retrieved_context.component, "config_provider");
        assert_eq!(
            retrieved_context.context_data.get("file"),
            Some(&"config.yaml".to_string())
        );
    }

    #[test]
    fn test_retryable_errors() {
        let network_error = CoreError::network("Connection failed");
        let rate_limit_error = CoreError::rate_limit("Too many requests");
        let timeout_error = CoreError::timeout("Operation timed out", 5000);
        let config_error = CoreError::config("Invalid configuration");

        assert!(network_error.is_retryable());
        assert!(rate_limit_error.is_retryable());
        assert!(timeout_error.is_retryable());
        assert!(!config_error.is_retryable());
    }

    #[test]
    fn test_retry_delays() {
        let network_error = CoreError::network("Connection failed");
        let rate_limit_error = CoreError::rate_limit_with_retry("Too many requests", 60);
        let timeout_error = CoreError::timeout("Operation timed out", 5000);
        let config_error = CoreError::config("Invalid configuration");

        assert_eq!(network_error.retry_delay_seconds(), Some(1));
        assert_eq!(rate_limit_error.retry_delay_seconds(), Some(60));
        assert_eq!(timeout_error.retry_delay_seconds(), Some(2));
        assert_eq!(config_error.retry_delay_seconds(), None);
    }

    #[test]
    fn test_validation_error() {
        let error = CoreError::validation("email", "Invalid email format");
        match error {
            CoreError::Validation {
                field,
                message,
                context,
            } => {
                assert_eq!(field, "email");
                assert_eq!(message, "Invalid email format");
                assert!(context.is_none());
            }
            _ => panic!("Expected Validation error"),
        }
    }

    #[test]
    fn test_authentication_error() {
        let error = CoreError::authentication("Invalid token");
        match error {
            CoreError::Authentication { message, context } => {
                assert_eq!(message, "Invalid token");
                assert!(context.is_none());
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_new_error_types() {
        // Test timeout error
        let timeout = CoreError::timeout("db_query", 30000);
        assert!(timeout.is_retryable());

        // Test network error
        let network = CoreError::network("DNS resolution failed");
        assert!(network.is_retryable());

        // Test rate limit error
        let rate_limit = CoreError::rate_limit_with_retry("API quota exceeded", 3600);
        assert!(rate_limit.is_retryable());
        assert_eq!(rate_limit.retry_delay_seconds(), Some(3600));
    }
}
