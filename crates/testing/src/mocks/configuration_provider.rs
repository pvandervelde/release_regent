//! Mock implementation of ConfigurationProvider trait
//!
//! Provides a comprehensive mock implementation for testing configuration
//! loading and validation without requiring actual configuration files.

use crate::mocks::{CallResult, MockConfig, MockState, SharedMockState};
use async_trait::async_trait;
use release_regent_core::{
    config::ReleaseRegentConfig, traits::configuration_provider::*, CoreError, CoreResult,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock implementation of ConfigurationProvider trait
///
/// This mock supports:
/// - Pre-configured configuration data for testing
/// - Validation simulation and error testing
/// - Configuration source tracking
/// - Environment variable override simulation
/// - File watching simulation (placeholder)
///
/// # Example Usage
///
/// ```rust
/// use release_regent_testing::mocks::MockConfigurationProvider;
/// use release_regent_core::config::ReleaseRegentConfig;
///
/// let config = ReleaseRegentConfig::default();
/// let mock = MockConfigurationProvider::new()
///     .with_config("test.yaml", config)
///     .with_validation_success(true);
/// ```
#[derive(Debug)]
pub struct MockConfigurationProvider {
    /// Shared state for tracking and configuration
    state: SharedMockState,
    /// Pre-configured configuration data by source path
    configurations: HashMap<String, ReleaseRegentConfig>,
    /// Pre-configured repository-specific configurations
    repository_configs: HashMap<String, RepositoryConfig>,
    /// Validation results for different configurations
    validation_results: HashMap<String, ValidationResult>,
    /// Whether to simulate successful validation by default
    default_validation_success: bool,
}

impl MockConfigurationProvider {
    /// Create a new mock with default configuration
    ///
    /// Returns a mock configured for basic testing scenarios with:
    /// - Deterministic behavior enabled
    /// - Call tracking enabled
    /// - Successful validation by default
    /// - No failure simulation
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(MockState::new())),
            configurations: HashMap::new(),
            repository_configs: HashMap::new(),
            validation_results: HashMap::new(),
            default_validation_success: true,
        }
    }

    /// Create a new mock with custom configuration
    ///
    /// # Parameters
    /// - `config`: Mock behavior configuration
    ///
    /// # Returns
    /// Configured mock instance
    pub fn with_config(config: MockConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(MockState::with_config(config))),
            configurations: HashMap::new(),
            repository_configs: HashMap::new(),
            validation_results: HashMap::new(),
            default_validation_success: true,
        }
    }

    /// Configure the mock to return specific configuration for a source
    ///
    /// # Parameters
    /// - `source_path`: Configuration source path
    /// - `config`: Configuration data to return
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_configuration(mut self, source_path: &str, config: ReleaseRegentConfig) -> Self {
        self.configurations.insert(source_path.to_string(), config);
        self
    }

    /// Configure the mock to return repository-specific configuration
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `name`: Repository name
    /// - `config`: Repository-specific configuration
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_repository_config(
        mut self,
        owner: &str,
        name: &str,
        config: RepositoryConfig,
    ) -> Self {
        let key = format!("{}/{}", owner, name);
        self.repository_configs.insert(key, config);
        self
    }

    /// Configure validation result for a specific configuration
    ///
    /// # Parameters
    /// - `config_key`: Configuration identifier
    /// - `result`: Validation result to return
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_validation_result(mut self, config_key: &str, result: ValidationResult) -> Self {
        self.validation_results
            .insert(config_key.to_string(), result);
        self
    }

    /// Configure whether validation should succeed by default
    ///
    /// # Parameters
    /// - `success`: Whether validation should succeed
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_validation_success(mut self, success: bool) -> Self {
        self.default_validation_success = success;
        self
    }

    /// Get the call history for verification
    ///
    /// # Returns
    /// Reference to all recorded method calls
    pub async fn call_history(&self) -> Vec<crate::mocks::CallInfo> {
        self.state.read().await.call_history().to_vec()
    }

    /// Get the total number of calls made
    ///
    /// # Returns
    /// Total call count
    pub async fn call_count(&self) -> u64 {
        self.state.read().await.call_count()
    }

    /// Record a method call for tracking
    async fn record_call(&self, method: &str, parameters: &str, result: CallResult) {
        self.state
            .write()
            .await
            .record_call(method, parameters, result);
    }

    /// Check if quota has been exceeded
    async fn check_quota(&self) -> CoreResult<()> {
        if self.state.read().await.is_quota_exceeded() {
            return Err(CoreError::rate_limit("Mock quota exceeded"));
        }
        Ok(())
    }

    /// Simulate latency if configured
    async fn simulate_latency(&self) {
        self.state.read().await.simulate_latency().await;
    }

    /// Check if should simulate failure
    async fn should_simulate_failure(&self) -> bool {
        self.state.read().await.should_simulate_failure()
    }

    /// Create a default validation result
    fn create_default_validation_result(&self) -> ValidationResult {
        if self.default_validation_success {
            ValidationResult {
                is_valid: true,
                errors: vec![],
                warnings: vec![],
            }
        } else {
            ValidationResult {
                is_valid: false,
                errors: vec!["Mock validation error".to_string()],
                warnings: vec!["Mock validation warning".to_string()],
            }
        }
    }
}

impl Default for MockConfigurationProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConfigurationProvider for MockConfigurationProvider {
    /// Load global configuration from default location
    ///
    /// Returns the pre-configured global configuration or a default configuration.
    ///
    /// # Parameters
    /// - `options`: Configuration loading options
    ///
    /// # Returns
    /// Global Release Regent configuration
    ///
    /// # Errors
    /// - `CoreError::Config` - Simulated configuration error
    async fn load_global_config(&self, _options: LoadOptions) -> CoreResult<ReleaseRegentConfig> {
        let method = "load_global_config";
        let params = "options=<provided>".to_string();

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::config("Simulated global configuration loading error");
            self.record_call(method, &params, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        // Return default configuration for global config
        let default_config = ReleaseRegentConfig::default();
        self.record_call(method, &params, CallResult::Success).await;
        Ok(default_config)
    }

    /// Load configuration for a specific repository
    ///
    /// Returns the pre-configured repository-specific configuration.
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `options`: Configuration loading options
    ///
    /// # Returns
    /// Repository-specific configuration or None if not found
    ///
    /// # Errors
    /// - `CoreError::Config` - Simulated configuration error
    async fn load_repository_config(
        &self,
        owner: &str,
        repo: &str,
        _options: LoadOptions,
    ) -> CoreResult<Option<RepositoryConfig>> {
        let method = "load_repository_config";
        let params = format!("owner={}, repo={}", owner, repo);

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::config("Simulated repository config error");
            self.record_call(method, &params, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{}/{}", owner, repo);
        let config = self.repository_configs.get(&key).cloned();

        self.record_call(method, &params, CallResult::Success).await;
        Ok(config)
    }

    /// Get merged configuration for a repository
    ///
    /// Combines global and repository-specific configuration.
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `options`: Configuration loading options
    ///
    /// # Returns
    /// Merged configuration with repository overrides applied
    ///
    /// # Errors
    /// - `CoreError::Config` - Simulated configuration error
    async fn get_merged_config(
        &self,
        owner: &str,
        repo: &str,
        options: LoadOptions,
    ) -> CoreResult<ReleaseRegentConfig> {
        let method = "get_merged_config";
        let params = format!("owner={}, repo={}", owner, repo);

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::config("Simulated merged config error");
            self.record_call(method, &params, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        // Load global config and optionally merge with repository config
        let mut merged_config = self.load_global_config(options.clone()).await?;

        if let Some(repo_config) = self.load_repository_config(owner, repo, options).await? {
            // In a real implementation, this would merge the configurations
            // For the mock, we'll just return the repository config
            merged_config = repo_config.config;
        }

        self.record_call(method, &params, CallResult::Success).await;
        Ok(merged_config)
    }

    /// Validate configuration
    ///
    /// Returns the pre-configured validation result or a default result.
    ///
    /// # Parameters
    /// - `config`: Configuration to validate
    ///
    /// # Returns
    /// Validation result
    ///
    /// # Errors
    /// - `CoreError::Config` - Simulated validation error
    async fn validate_config(&self, _config: &ReleaseRegentConfig) -> CoreResult<ValidationResult> {
        let method = "validate_config";
        let params = "config=<provided>".to_string();

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::config("Simulated validation error");
            self.record_call(method, &params, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let result = self.create_default_validation_result();
        self.record_call(method, &params, CallResult::Success).await;
        Ok(result)
    }

    /// Save configuration to storage
    ///
    /// Mock implementation always returns not supported error.
    ///
    /// # Parameters
    /// - `config`: Configuration to save
    /// - `owner`: Repository owner (for repository-specific config)
    /// - `repo`: Repository name (for repository-specific config)
    /// - `global`: Whether this is global configuration
    ///
    /// # Returns
    /// Success confirmation
    ///
    /// # Errors
    /// - `CoreError::NotSupported` - Mock doesn't support saving
    async fn save_config(
        &self,
        _config: &ReleaseRegentConfig,
        owner: Option<&str>,
        repo: Option<&str>,
        global: bool,
    ) -> CoreResult<()> {
        let method = "save_config";
        let params = format!("owner={:?}, repo={:?}, global={}", owner, repo, global);

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Mock implementation doesn't support saving
        let error =
            CoreError::not_supported("MockConfigurationProvider", "save_config not implemented");
        self.record_call(method, &params, CallResult::Error(error.to_string()))
            .await;
        Err(error)
    }

    /// List all repository configurations
    ///
    /// Returns all pre-configured repository configurations.
    ///
    /// # Parameters
    /// - `options`: Configuration loading options
    ///
    /// # Returns
    /// List of repository configurations
    ///
    /// # Errors
    /// - `CoreError::Config` - Simulated configuration error
    async fn list_repository_configs(
        &self,
        _options: LoadOptions,
    ) -> CoreResult<Vec<RepositoryConfig>> {
        let method = "list_repository_configs";
        let params = "options=<provided>".to_string();

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::config("Simulated list repository configs error");
            self.record_call(method, &params, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let configs: Vec<RepositoryConfig> = self.repository_configs.values().cloned().collect();
        self.record_call(method, &params, CallResult::Success).await;
        Ok(configs)
    }

    /// Get configuration source information
    ///
    /// Returns mock source information.
    ///
    /// # Parameters
    /// - `owner`: Repository owner (optional, for repo-specific source info)
    /// - `repo`: Repository name (optional, for repo-specific source info)
    ///
    /// # Returns
    /// Configuration source metadata
    ///
    /// # Errors
    /// - `CoreError::Config` - Simulated source error
    async fn get_config_source(
        &self,
        owner: Option<&str>,
        repo: Option<&str>,
    ) -> CoreResult<ConfigurationSource> {
        let method = "get_config_source";
        let params = format!("owner={:?}, repo={:?}", owner, repo);

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::config("Simulated source not found error");
            self.record_call(method, &params, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let source_path = match (owner, repo) {
            (Some(o), Some(r)) => format!("mock://repository/{}/{}", o, r),
            _ => "mock://global".to_string(),
        };

        let source = ConfigurationSource {
            location: source_path,
            source_type: "mock".to_string(),
            format: "yaml".to_string(),
            loaded_at: chrono::Utc::now(),
        };

        self.record_call(method, &params, CallResult::Success).await;
        Ok(source)
    }

    /// Reload configuration from source
    ///
    /// Mock implementation simulates successful reload.
    ///
    /// # Parameters
    /// - `owner`: Repository owner (optional, for repo-specific reload)
    /// - `repo`: Repository name (optional, for repo-specific reload)
    ///
    /// # Returns
    /// Success confirmation
    ///
    /// # Errors
    /// - `CoreError::Config` - Simulated reload error
    async fn reload_config(&self, owner: Option<&str>, repo: Option<&str>) -> CoreResult<()> {
        let method = "reload_config";
        let params = format!("owner={:?}, repo={:?}", owner, repo);

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::config("Simulated reload error");
            self.record_call(method, &params, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        self.record_call(method, &params, CallResult::Success).await;
        Ok(())
    }

    /// Check if configuration exists
    ///
    /// Mock implementation checks pre-configured data.
    ///
    /// # Parameters
    /// - `owner`: Repository owner (None for global config)
    /// - `repo`: Repository name (None for global config)
    ///
    /// # Returns
    /// True if configuration exists, false otherwise
    ///
    /// # Errors
    /// - `CoreError::Config` - Simulated existence check error
    async fn config_exists(&self, owner: Option<&str>, repo: Option<&str>) -> CoreResult<bool> {
        let method = "config_exists";
        let params = format!("owner={:?}, repo={:?}", owner, repo);

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::config("Simulated config exists check error");
            self.record_call(method, &params, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let exists = match (owner, repo) {
            (Some(o), Some(r)) => {
                let key = format!("{}/{}", o, r);
                self.repository_configs.contains_key(&key)
            }
            _ => true, // Global config always exists in mock
        };

        self.record_call(method, &params, CallResult::Success).await;
        Ok(exists)
    }

    /// Get supported configuration formats
    ///
    /// Returns mock-supported formats.
    ///
    /// # Returns
    /// List of supported formats
    fn supported_formats(&self) -> Vec<String> {
        vec!["yaml".to_string(), "toml".to_string(), "json".to_string()]
    }

    /// Get default configuration
    ///
    /// Returns a default Release Regent configuration.
    ///
    /// # Returns
    /// Default Release Regent configuration
    ///
    /// # Errors
    /// - `CoreError::Config` - Simulated default config error
    async fn get_default_config(&self) -> CoreResult<ReleaseRegentConfig> {
        let method = "get_default_config";
        let params = "".to_string();

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::config("Simulated default config error");
            self.record_call(method, &params, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let default_config = ReleaseRegentConfig::default();
        self.record_call(method, &params, CallResult::Success).await;
        Ok(default_config)
    }
}
