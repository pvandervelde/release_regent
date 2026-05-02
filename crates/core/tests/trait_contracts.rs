//! Integration tests for core trait contracts
//!
//! These tests verify that:
//! 1. All traits are object-safe and can be used as `dyn Trait`
//! 2. Mocks return errors when configured with failure simulation
//! 3. Async trait methods are callable through trait object references
//! 4. Trait objects can be stored in heterogeneous collections
//!
//! These tests live in `crates/core/tests/` (integration tests) rather than
//! `crates/core/src/traits/tests.rs` (unit tests) because the mock types in
//! `release_regent_testing` are compiled against the *library* version of
//! `release_regent_core`.  Unit-test binaries are a distinct compilation unit
//! from the library, so the trait impl does not unify.  Integration tests
//! import the library as an external crate and therefore share the same type
//! universe as `release_regent_testing`.

use release_regent_core::traits::{
    configuration_provider::{self, ConfigurationProvider},
    git_operations::{GitOperations, ListTagsOptions},
    github_operations::{self, GitHubOperations},
    version_calculator::{self, VersionCalculator},
};
use release_regent_testing::mocks::{
    MockConfig, MockConfigurationProvider, MockGitHubOperations, MockVersionCalculator,
};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Trait object-safety tests
// ---------------------------------------------------------------------------

/// Test that `GitHubOperations` is object-safe: it can be coerced to `&dyn`.
#[tokio::test]
async fn test_github_operations_trait_object_safety() {
    let mock = MockGitHubOperations::new();
    let client: &dyn GitHubOperations = &mock;

    // An unseeded mock returns Ok(None) for get_latest_release.
    // The important thing is that this line compiles — proving vtable dispatch works.
    let result = client.get_latest_release("owner", "repo").await;
    assert!(result.is_ok(), "unseeded mock should return Ok(None)");
    assert!(result.unwrap().is_none());
}

/// Test that `ConfigurationProvider` is object-safe.
#[tokio::test]
async fn test_configuration_provider_trait_object_safety() {
    let mock = MockConfigurationProvider::new();
    let provider: &dyn ConfigurationProvider = &mock;

    let options = configuration_provider::LoadOptions::default();
    let result = provider.load_global_config(options).await;
    assert!(
        result.is_ok(),
        "unseeded mock should return Ok(default config)"
    );
}

/// Test that `VersionCalculator` is object-safe.
#[tokio::test]
async fn test_version_calculator_trait_object_safety() {
    let mock = MockVersionCalculator::new();
    let calculator: &dyn VersionCalculator = &mock;

    let context = version_calculator::VersionContext {
        owner: "test".to_string(),
        repo: "test".to_string(),
        current_version: None,
        target_branch: "main".to_string(),
        base_ref: None,
        head_ref: "HEAD".to_string(),
    };
    let strategy = version_calculator::VersioningStrategy::ConventionalCommits {
        custom_types: HashMap::new(),
        include_prerelease: false,
    };
    let options = version_calculator::CalculationOptions::default();

    let result = calculator
        .calculate_version(context, strategy, options)
        .await;
    assert!(
        result.is_ok(),
        "unseeded mock should return a default calculation result"
    );
}

// ---------------------------------------------------------------------------
// Failure simulation
// ---------------------------------------------------------------------------

/// Test that mocks configured with failure simulation return errors.
#[tokio::test]
async fn test_mock_error_handling() {
    // GitHub: failure simulation on get_latest_release
    let github_mock = MockGitHubOperations::with_config(MockConfig {
        simulate_failures: true,
        failure_rate: 1.0,
        ..MockConfig::default()
    });
    let result = github_mock.get_latest_release("owner", "repo").await;
    assert!(
        result.is_err(),
        "mock configured for failure should return an error"
    );

    // GitHub: failure simulation on list_tags
    let github_mock2 = MockGitHubOperations::with_config(MockConfig {
        simulate_failures: true,
        failure_rate: 1.0,
        ..MockConfig::default()
    });
    let result = github_mock2
        .list_tags("owner", "repo", ListTagsOptions::default())
        .await;
    assert!(
        result.is_err(),
        "mock configured for failure should return an error"
    );

    // ConfigurationProvider: failure simulation on load_repository_config
    let config_mock = MockConfigurationProvider::with_config(MockConfig {
        simulate_failures: true,
        failure_rate: 1.0,
        ..MockConfig::default()
    });
    let options = configuration_provider::LoadOptions::default();
    let result = config_mock
        .load_repository_config("owner", "repo", options)
        .await;
    assert!(
        result.is_err(),
        "mock configured for failure should return an error"
    );

    // VersionCalculator: failure simulation on analyze_commits
    let version_mock = MockVersionCalculator::with_config(MockConfig {
        simulate_failures: true,
        failure_rate: 1.0,
        ..MockConfig::default()
    });
    let context = version_calculator::VersionContext {
        base_ref: None,
        current_version: None,
        head_ref: "HEAD".to_string(),
        owner: "owner".to_string(),
        repo: "repo".to_string(),
        target_branch: "main".to_string(),
    };
    let strategy = version_calculator::VersioningStrategy::ConventionalCommits {
        custom_types: HashMap::new(),
        include_prerelease: false,
    };
    let result = version_mock
        .analyze_commits(context, strategy, vec!["abc123".to_string()])
        .await;
    assert!(
        result.is_err(),
        "mock configured for failure should return an error"
    );
}

// ---------------------------------------------------------------------------
// Async trait compatibility
// ---------------------------------------------------------------------------

/// Test that async trait methods can be called through trait object references.
#[tokio::test]
async fn test_async_trait_compatibility() {
    async fn use_github_operations(client: &dyn GitHubOperations) -> bool {
        // Any call that goes through the vtable proves async dispatch works.
        client.get_latest_release("owner", "repo").await.is_ok()
    }

    async fn use_config_provider(provider: &dyn ConfigurationProvider) -> bool {
        let options = configuration_provider::LoadOptions::default();
        provider.load_global_config(options).await.is_ok()
    }

    async fn use_version_calculator(calculator: &dyn VersionCalculator) -> bool {
        let context = version_calculator::VersionContext {
            base_ref: None,
            current_version: None,
            head_ref: "HEAD".to_string(),
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            target_branch: "main".to_string(),
        };
        let strategy = version_calculator::VersioningStrategy::ConventionalCommits {
            custom_types: HashMap::new(),
            include_prerelease: false,
        };
        let options = version_calculator::CalculationOptions::default();
        calculator
            .calculate_version(context, strategy, options)
            .await
            .is_ok()
    }

    let github_mock = MockGitHubOperations::new();
    let config_mock = MockConfigurationProvider::new();
    let version_mock = MockVersionCalculator::new();

    assert!(use_github_operations(&github_mock).await);
    assert!(use_config_provider(&config_mock).await);
    assert!(use_version_calculator(&version_mock).await);
}

// ---------------------------------------------------------------------------
// Collection storage
// ---------------------------------------------------------------------------

/// Test that trait objects can be stored in heterogeneous `Vec` collections.
#[test]
fn test_trait_objects_in_collections() {
    let github_clients: Vec<Box<dyn github_operations::GitHubOperations>> =
        vec![Box::new(MockGitHubOperations::new())];
    assert_eq!(github_clients.len(), 1);

    let config_providers: Vec<Box<dyn configuration_provider::ConfigurationProvider>> =
        vec![Box::new(MockConfigurationProvider::new())];
    assert_eq!(config_providers.len(), 1);

    let version_calculators: Vec<Box<dyn version_calculator::VersionCalculator>> =
        vec![Box::new(MockVersionCalculator::new())];
    assert_eq!(version_calculators.len(), 1);
}
