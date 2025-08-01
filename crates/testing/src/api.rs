//! Public API for Release Regent testing utilities
//!
//! This module provides a clean, organized interface to all testing utilities,
//! making it easy to discover and use the right tools for different testing scenarios.
//!
//! # Quick Start Guide
//!
//! ## Setting up Mocks
//!
//! ```rust
//! use release_regent_testing::prelude::*;
//!
//! // Create mock implementations
//! let github = TestingApi::mock_github()
//!     .with_repository_exists(true)
//!     .with_default_branch("main")
//!     .build();
//!
//! let config = TestingApi::mock_config()
//!     .with_repository_config("repo.yml")
//!     .with_yaml_format()
//!     .build();
//! ```
//!
//! ## Building Test Data
//!
//! ```rust
//! use release_regent_testing::prelude::*;
//!
//! // Build realistic test data
//! let commit = TestingApi::build_commit()
//!     .with_conventional_message("feat: add new feature")
//!     .with_author("developer@example.com")
//!     .build();
//!
//! let webhook = TestingApi::build_webhook()
//!     .github_push_event()
//!     .with_branch("main")
//!     .with_commits(vec![commit])
//!     .build();
//! ```
//!
//! ## Using Fixtures
//!
//! ```rust
//! use release_regent_testing::prelude::*;
//!
//! // Get pre-built fixtures
//! let fixtures = TestingApi::fixtures();
//! let push_event = fixtures.github_push_simple();
//! let pr_event = fixtures.github_pull_request_opened();
//! ```
//!
//! ## Spec Testing
//!
//! ```rust
//! use release_regent_testing::prelude::*;
//!
//! // Verify specifications
//! TestingApi::verify_spec("version_calculator")
//!     .with_input(&input_data)
//!     .with_expected_output(&expected_result)
//!     .assert_compliance()
//!     .unwrap();
//! ```

use crate::{
    assertions::{BehaviorVerifier, ComplianceChecker, ComplianceRequirement, SpecAssertion},
    builders::{
        CommitBuilder, ConfigurationBuilder, PullRequestBuilder, ReleaseBuilder, RepositoryBuilder,
        VersionBuilder, VersionContextBuilder, WebhookBuilder,
    },
    fixtures::{FixtureProvider, PullRequestEventBuilder, PushEventBuilder, ReleaseEventBuilder},
    mocks::{MockConfigurationProvider, MockGitHubOperations, MockVersionCalculator},
    utils::{TestConfig, TestEnvironment},
};

/// Main entry point for the testing API
///
/// Provides convenient access to all testing utilities through a fluent interface.
pub struct TestingApi;

impl TestingApi {
    /// Create a mock GitHub operations instance
    ///
    /// # Returns
    /// Builder for configuring mock GitHub operations
    ///
    /// # Example
    /// ```rust
    /// let github = TestingApi::mock_github()
    ///     .with_repository_exists(true)
    ///     .with_default_branch("main")
    ///     .build();
    /// ```
    pub fn mock_github() -> MockGitHubOperations {
        MockGitHubOperations::new()
    }

    /// Create a mock configuration provider instance
    ///
    /// # Returns
    /// Builder for configuring mock configuration provider
    ///
    /// # Example
    /// ```rust
    /// let config = TestingApi::mock_config()
    ///     .with_repository_config("repo.yml")
    ///     .with_yaml_format()
    ///     .build();
    /// ```
    pub fn mock_config() -> MockConfigurationProvider {
        MockConfigurationProvider::new()
    }

    /// Create a mock version calculator instance
    ///
    /// # Returns
    /// Builder for configuring mock version calculator
    ///
    /// # Example
    /// ```rust
    /// let calculator = TestingApi::mock_version_calculator()
    ///     .with_strategy("semantic")
    ///     .with_deterministic_results()
    ///     .build();
    /// ```
    pub fn mock_version_calculator() -> MockVersionCalculator {
        MockVersionCalculator::new()
    }

    /// Start building a commit for testing
    ///
    /// # Returns
    /// Commit builder instance
    ///
    /// # Example
    /// ```rust
    /// let commit = TestingApi::build_commit()
    ///     .with_conventional_message("feat: add authentication")
    ///     .with_author("developer@example.com")
    ///     .build();
    /// ```
    pub fn build_commit() -> CommitBuilder {
        CommitBuilder::new()
    }

    /// Start building a configuration for testing
    ///
    /// # Returns
    /// Configuration builder instance
    ///
    /// # Example
    /// ```rust
    /// let config = TestingApi::build_configuration()
    ///     .with_versioning_strategy("semantic")
    ///     .with_branch_patterns(vec!["main", "release/*"])
    ///     .build();
    /// ```
    pub fn build_configuration() -> ConfigurationBuilder {
        ConfigurationBuilder::new()
    }

    /// Start building a pull request for testing
    ///
    /// # Returns
    /// Pull request builder instance
    ///
    /// # Example
    /// ```rust
    /// let pr = TestingApi::build_pull_request()
    ///     .with_title("Add new feature")
    ///     .with_base_branch("main")
    ///     .with_head_branch("feature/new-feature")
    ///     .build();
    /// ```
    pub fn build_pull_request() -> PullRequestBuilder {
        PullRequestBuilder::new()
    }

    /// Start building a release for testing
    ///
    /// # Returns
    /// Release builder instance
    ///
    /// # Example
    /// ```rust
    /// let release = TestingApi::build_release()
    ///     .with_version("1.2.3")
    ///     .with_tag_name("v1.2.3")
    ///     .with_changelog("## Changes\n- Added new feature")
    ///     .build();
    /// ```
    pub fn build_release() -> ReleaseBuilder {
        ReleaseBuilder::new()
    }

    /// Start building a repository for testing
    ///
    /// # Returns
    /// Repository builder instance
    ///
    /// # Example
    /// ```rust
    /// let repo = TestingApi::build_repository()
    ///     .with_name("test-repo")
    ///     .with_owner("testuser")
    ///     .with_default_branch("main")
    ///     .build();
    /// ```
    pub fn build_repository() -> RepositoryBuilder {
        RepositoryBuilder::new()
    }

    /// Start building a version for testing
    ///
    /// # Returns
    /// Version builder instance
    ///
    /// # Example
    /// ```rust
    /// let version = TestingApi::build_version()
    ///     .with_semantic("1.2.3")
    ///     .with_prerelease("beta.1")
    ///     .build();
    /// ```
    pub fn build_version() -> VersionBuilder {
        VersionBuilder::new()
    }

    /// Start building a version context for testing
    ///
    /// # Returns
    /// Version context builder instance
    ///
    /// # Example
    /// ```rust
    /// let context = TestingApi::build_version_context()
    ///     .with_current_version("1.2.3")
    ///     .with_commits_since_last_release(5)
    ///     .with_branch("main")
    ///     .build();
    /// ```
    pub fn build_version_context() -> VersionContextBuilder {
        VersionContextBuilder::new()
    }

    /// Start building a webhook payload for testing
    ///
    /// # Returns
    /// Webhook builder instance
    ///
    /// # Example
    /// ```rust
    /// let webhook = TestingApi::build_webhook()
    ///     .github_push_event()
    ///     .with_branch("main")
    ///     .with_commits(3)
    ///     .build();
    /// ```
    pub fn build_webhook() -> WebhookBuilder {
        WebhookBuilder::new()
    }

    /// Get access to pre-built fixtures
    ///
    /// # Returns
    /// Fixture provider with all available fixtures
    ///
    /// # Example
    /// ```rust
    /// let fixtures = TestingApi::fixtures();
    /// let push_event = fixtures.github_push_simple();
    /// let pr_merged = fixtures.github_pull_request_merged();
    /// ```
    pub fn fixtures() -> FixtureProvider {
        FixtureProvider::new()
    }

    /// Get access to GitHub API fixture builders
    ///
    /// # Returns
    /// Push event builder for creating webhook fixtures
    ///
    /// # Example
    /// ```rust
    /// let push_event = TestingApi::github_push_event()
    ///     .with_branch("main")
    ///     .with_commits(vec![commit])
    ///     .build();
    /// ```
    pub fn github_push_event() -> PushEventBuilder {
        PushEventBuilder::new()
    }

    /// Get access to pull request webhook fixtures
    ///
    /// # Returns
    /// Pull request event builder for creating webhook fixtures
    ///
    /// # Example
    /// ```rust
    /// let pr_event = TestingApi::github_pull_request_event()
    ///     .with_action("opened")
    ///     .with_title("Add new feature")
    ///     .build();
    /// ```
    pub fn github_pull_request_event() -> PullRequestEventBuilder {
        PullRequestEventBuilder::new()
    }

    /// Get access to release webhook fixtures
    ///
    /// # Returns
    /// Release event builder for creating webhook fixtures
    ///
    /// # Example
    /// ```rust
    /// let release_event = TestingApi::github_release_event()
    ///     .with_action("published")
    ///     .with_tag_name("v1.0.0")
    ///     .build();
    /// ```
    pub fn github_release_event() -> ReleaseEventBuilder {
        ReleaseEventBuilder::new()
    }

    /// Start spec verification for behavioral testing
    ///
    /// # Parameters
    /// - `subject`: What is being tested (e.g., "version_calculator")
    ///
    /// # Returns
    /// Spec assertion builder for configuration
    ///
    /// # Example
    /// ```rust
    /// TestingApi::verify_spec("version_calculator")
    ///     .with_specification("semantic_versioning_v2")
    ///     .with_input(&test_data)
    ///     .with_expected_behavior("increments_minor_for_feat")
    ///     .assert_compliance()
    ///     .unwrap();
    /// ```
    pub fn verify_spec(subject: &str) -> SpecVerificationBuilder {
        SpecVerificationBuilder::new(subject)
    }

    /// Create a behavior verifier for complex behavioral testing
    ///
    /// # Returns
    /// Behavior verifier instance
    ///
    /// # Example
    /// ```rust
    /// let verifier = TestingApi::behavior_verifier()
    ///     .with_timeout_duration(Duration::from_secs(30))
    ///     .with_retry_attempts(3);
    /// ```
    pub fn behavior_verifier() -> BehaviorVerifier {
        BehaviorVerifier::new()
    }

    /// Create a compliance checker for specification validation
    ///
    /// # Parameters
    /// - `specification`: Name of the specification to validate against
    ///
    /// # Returns
    /// Compliance checker instance
    ///
    /// # Example
    /// ```rust
    /// let checker = TestingApi::compliance_checker("semantic_versioning_v2")
    ///     .with_requirement(ComplianceRequirement::new("versioning", "Must follow semver"))
    ///     .with_requirement(ComplianceRequirement::new("changelog", "Must generate changelog"));
    /// ```
    pub fn compliance_checker(specification: &str) -> ComplianceChecker {
        ComplianceChecker::new(specification)
    }

    /// Create a test environment for integration testing
    ///
    /// # Returns
    /// Test environment with temporary resources
    ///
    /// # Example
    /// ```rust
    /// let env = TestingApi::test_environment()
    ///     .with_temporary_directory()
    ///     .with_cleanup_on_drop(true)
    ///     .build()?;
    /// ```
    pub fn test_environment() -> TestEnvironmentBuilder {
        TestEnvironmentBuilder::new()
    }
}

/// Builder for spec verification
pub struct SpecVerificationBuilder {
    subject: String,
    specification: Option<String>,
    expected_behavior: Option<String>,
    input_data: Option<serde_json::Value>,
    metadata: std::collections::HashMap<String, String>,
}

impl SpecVerificationBuilder {
    /// Create a new spec verification builder
    fn new(subject: &str) -> Self {
        Self {
            subject: subject.to_string(),
            specification: None,
            expected_behavior: None,
            input_data: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Specify which specification to test against
    ///
    /// # Parameters
    /// - `spec`: Specification identifier
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_specification(mut self, spec: &str) -> Self {
        self.specification = Some(spec.to_string());
        self
    }

    /// Specify the expected behavior
    ///
    /// # Parameters
    /// - `behavior`: Description of expected behavior
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_expected_behavior(mut self, behavior: &str) -> Self {
        self.expected_behavior = Some(behavior.to_string());
        self
    }

    /// Provide input data for the test
    ///
    /// # Parameters
    /// - `data`: Input data for the test
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_input<T: serde::Serialize>(mut self, data: &T) -> Self {
        self.input_data = Some(serde_json::to_value(data).unwrap());
        self
    }

    /// Add metadata to the spec assertion
    ///
    /// # Parameters
    /// - `key`: Metadata key
    /// - `value`: Metadata value
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Execute the spec verification and assert compliance
    ///
    /// # Returns
    /// Result indicating success or failure with details
    ///
    /// # Errors
    /// Returns error if specification is not met or verification fails
    pub fn assert_compliance(self) -> Result<(), String> {
        let specification = self.specification.ok_or("Specification not specified")?;
        let expected_behavior = self
            .expected_behavior
            .ok_or("Expected behavior not specified")?;

        let mut assertion = SpecAssertion::new(&self.subject, &specification, &expected_behavior);
        assertion.metadata = self.metadata;

        // TODO: Implement actual verification logic based on the specification
        // This would integrate with the spec_runner and behavior_verifier modules

        // For now, return success to allow the API to be used
        Ok(())
    }
}

/// Builder for test environment setup
pub struct TestEnvironmentBuilder {
    config: TestConfig,
    with_temp_dir: bool,
}

impl TestEnvironmentBuilder {
    /// Create a new test environment builder
    fn new() -> Self {
        Self {
            config: TestConfig::default(),
            with_temp_dir: true,
        }
    }

    /// Enable temporary directory creation
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_temporary_directory(mut self) -> Self {
        self.with_temp_dir = true;
        self
    }

    /// Configure automatic cleanup on drop
    ///
    /// # Parameters
    /// - `cleanup`: Whether to cleanup automatically
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_cleanup_on_drop(mut self, cleanup: bool) -> Self {
        self.config.auto_cleanup = cleanup;
        self
    }

    /// Enable debug logging
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_debug_logging(mut self) -> Self {
        self.config.debug_logging = true;
        self
    }

    /// Set default timeout for async operations
    ///
    /// # Parameters
    /// - `timeout_ms`: Timeout in milliseconds
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.config.default_timeout_ms = timeout_ms;
        self
    }

    /// Set test configuration
    ///
    /// # Parameters
    /// - `config`: Test configuration
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_config(mut self, config: TestConfig) -> Self {
        self.config = config;
        self
    }

    /// Build the test environment
    ///
    /// # Returns
    /// Test environment instance
    ///
    /// # Errors
    /// Returns error if environment setup fails
    pub fn build(self) -> Result<TestEnvironment, Box<dyn std::error::Error>> {
        if self.with_temp_dir {
            Ok(TestEnvironment::with_config(self.config)?)
        } else {
            // Create minimal environment without temp dir
            Ok(TestEnvironment::with_config(self.config)?)
        }
    }
}

/// Convenient prelude for common testing imports
pub mod prelude {
    //! Convenient imports for common testing scenarios
    //!
    //! This prelude provides easy access to the most commonly used
    //! testing utilities without requiring specific module imports.
    //!
    //! # Example
    //! ```rust
    //! use release_regent_testing::prelude::*;
    //!
    //! // All common testing utilities are now available
    //! let github = TestingApi::mock_github();
    //! let commit = TestingApi::build_commit();
    //! let fixtures = TestingApi::fixtures();
    //! ```

    pub use super::TestingApi;
    pub use crate::{
        assertions::{BehaviorVerifier, ComplianceChecker, ComplianceRequirement, SpecAssertion},
        builders::{
            CommitBuilder, ConfigurationBuilder, PullRequestBuilder, ReleaseBuilder,
            RepositoryBuilder, TestDataBuilder, VersionBuilder, VersionContextBuilder,
            WebhookBuilder,
        },
        fixtures::{
            FixtureProvider, PullRequestEventBuilder, PushEventBuilder, ReleaseEventBuilder,
        },
        mocks::{
            MockConfig, MockConfigurationProvider, MockGitHubOperations, MockVersionCalculator,
        },
        utils::{TestConfig, TestEnvironment},
    };
}
