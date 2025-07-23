//! Tests for core trait abstractions
//!
//! This module contains tests that validate the trait contracts and ensure
//! that mock implementations behave correctly for testing scenarios.

use crate::traits::*;
use crate::versioning::SemanticVersion;
use chrono::Utc;
use std::collections::HashMap;

/// Test that GitHubOperations trait can be object-safe and used in generic contexts
#[tokio::test]
async fn test_github_operations_trait_object_safety() {
    // This test verifies that GitHubOperations can be used as a trait object
    let mock: Box<dyn GitHubOperations> = Box::new(github_operations::MockGitHubOperations);

    // Test that all methods return the expected error for unimplemented mock
    let result = mock.get_repository("owner", "repo").await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("not yet implemented"));
    }
}

/// Test that ConfigurationProvider trait can be object-safe and used in generic contexts
#[tokio::test]
async fn test_configuration_provider_trait_object_safety() {
    // This test verifies that ConfigurationProvider can be used as a trait object
    let mock: Box<dyn ConfigurationProvider> = Box::new(configuration_provider::MockConfigurationProvider);

    // Test that all methods return the expected error for unimplemented mock
    let options = configuration_provider::LoadOptions::default();
    let result = mock.load_global_config(options).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("not yet implemented"));
    }
}

/// Test that VersionCalculator trait can be object-safe and used in generic contexts
#[tokio::test]
async fn test_version_calculator_trait_object_safety() {
    // This test verifies that VersionCalculator can be used as a trait object
    let mock: Box<dyn VersionCalculator> = Box::new(version_calculator::MockVersionCalculator);

    // Test that all methods return the expected error for unimplemented mock
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

    let result = mock.calculate_version(context, strategy, options).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("not yet implemented"));
    }
}

/// Test GitHub operations data structures serialization and deserialization
#[test]
fn test_github_operations_data_structures() {
    // Test Repository serialization
    let repo = github_operations::Repository {
        clone_url: "https://github.com/test-owner/test-repo.git".to_string(),
        default_branch: "main".to_string(),
        description: Some("Test repository".to_string()),
        full_name: "test-owner/test-repo".to_string(),
        homepage: None,
        id: 123,
        name: "test-repo".to_string(),
        owner: "test-owner".to_string(),
        private: false,
        ssh_url: "git@github.com:test-owner/test-repo.git".to_string(),
    };

    let json = serde_json::to_string(&repo).expect("Repository should serialize");
    let deserialized: github_operations::Repository = serde_json::from_str(&json)
        .expect("Repository should deserialize");
    assert_eq!(repo.id, deserialized.id);
    assert_eq!(repo.name, deserialized.name);
    assert_eq!(repo.owner, deserialized.owner);

    // Test Tag serialization
    let tag = github_operations::Tag {
        commit_sha: "abc123".to_string(),
        created_at: Some(Utc::now()),
        message: Some("Release v1.0.0".to_string()),
        name: "v1.0.0".to_string(),
        tagger: None,
    };

    let json = serde_json::to_string(&tag).expect("Tag should serialize");
    let deserialized: github_operations::Tag = serde_json::from_str(&json)
        .expect("Tag should deserialize");
    assert_eq!(tag.name, deserialized.name);
    assert_eq!(tag.commit_sha, deserialized.commit_sha);
}

/// Test version calculator data structures
#[test]
fn test_version_calculator_data_structures() {
    // Test VersionContext
    let context = version_calculator::VersionContext {
        base_ref: Some("v1.0.0".to_string()),
        current_version: Some(SemanticVersion {
            major: 1,
            minor: 0,
            patch: 0,
            prerelease: None,
            build: None,
        }),
        head_ref: "HEAD".to_string(),
        owner: "owner".to_string(),
        repo: "repo".to_string(),
        target_branch: "main".to_string(),
    };

    let json = serde_json::to_string(&context).expect("VersionContext should serialize");
    let deserialized: version_calculator::VersionContext = serde_json::from_str(&json)
        .expect("VersionContext should deserialize");
    assert_eq!(context.owner, deserialized.owner);
    assert_eq!(context.repo, deserialized.repo);

    // Test VersionBump enum
    let bump = version_calculator::VersionBump::Major;
    let json = serde_json::to_string(&bump).expect("VersionBump should serialize");
    let deserialized: version_calculator::VersionBump = serde_json::from_str(&json)
        .expect("VersionBump should deserialize");
    assert_eq!(bump, deserialized);
}

/// Test configuration provider data structures
#[test]
fn test_configuration_provider_data_structures() {
    // Test LoadOptions default implementation
    let options = configuration_provider::LoadOptions::default();
    assert!(!options.apply_env_overrides);
    assert!(!options.cache);
    assert!(options.cache_ttl.is_none());
    assert!(options.env_prefix.is_none());
    assert!(!options.validate);

    // Test ValidationResult
    let validation = configuration_provider::ValidationResult {
        errors: vec![],
        is_valid: true,
        warnings: vec!["Minor warning".to_string()],
    };
    assert!(validation.is_valid);
    assert!(validation.errors.is_empty());
    assert_eq!(validation.warnings.len(), 1);

    // Test ConfigurationSource serialization
    let source = configuration_provider::ConfigurationSource {
        format: "yaml".to_string(),
        loaded_at: Utc::now(),
        location: "/path/to/config.yaml".to_string(),
        source_type: "file".to_string(),
    };

    let json = serde_json::to_string(&source).expect("ConfigurationSource should serialize");
    let deserialized: configuration_provider::ConfigurationSource = serde_json::from_str(&json)
        .expect("ConfigurationSource should deserialize");
    assert_eq!(source.source_type, deserialized.source_type);
    assert_eq!(source.location, deserialized.location);
}

/// Test error handling for mock implementations
#[tokio::test]
async fn test_mock_error_handling() {
    let github_mock = github_operations::MockGitHubOperations;
    let config_mock = configuration_provider::MockConfigurationProvider;
    let version_mock = version_calculator::MockVersionCalculator;

    // Test GitHub operations error
    let result = github_mock.list_tags("owner", "repo", None, None).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e, crate::CoreError::NotSupported { .. }));
    }

    // Test configuration provider error
    let options = configuration_provider::LoadOptions::default();
    let result = config_mock.load_repository_config("owner", "repo", options).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e, crate::CoreError::NotSupported { .. }));
    }

    // Test version calculator error
    let context = version_calculator::VersionContext {
        base_ref: None,
        current_version: None,
        head_ref: "HEAD".to_string(),
        owner: "owner".to_string(),
        repo: "repo".to_string(),
        target_branch: "main".to_string(),
    };
    let commits = vec!["abc123".to_string(), "def456".to_string()];
    let strategy = version_calculator::VersioningStrategy::ConventionalCommits {
        custom_types: HashMap::new(),
        include_prerelease: false,
    };

    let result = version_mock.analyze_commits(context, strategy, commits).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e, crate::CoreError::NotSupported { .. }));
    }
}

/// Test trait method signatures for async compatibility
#[tokio::test]
async fn test_async_trait_compatibility() {
    // Test that async methods can be called in async contexts
    async fn test_github_operations(client: &dyn GitHubOperations) -> crate::CoreResult<()> {
        let _repo = client.get_repository("owner", "repo").await?;
        Ok(())
    }

    async fn test_config_provider(provider: &dyn ConfigurationProvider) -> crate::CoreResult<()> {
        let options = configuration_provider::LoadOptions::default();
        let _config = provider.load_global_config(options).await?;
        Ok(())
    }

    async fn test_version_calculator(calculator: &dyn VersionCalculator) -> crate::CoreResult<()> {
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

        let _result = calculator.calculate_version(context, strategy, options).await?;
        Ok(())
    }

    let github_mock = github_operations::MockGitHubOperations;
    let config_mock = configuration_provider::MockConfigurationProvider;
    let version_mock = version_calculator::MockVersionCalculator;

    // These should compile and fail with the expected "not implemented" error
    assert!(test_github_operations(&github_mock).await.is_err());
    assert!(test_config_provider(&config_mock).await.is_err());
    assert!(test_version_calculator(&version_mock).await.is_err());
}

/// Test that all trait objects can be stored in collections
#[test]
fn test_trait_objects_in_collections() {
    let github_clients: Vec<Box<dyn GitHubOperations>> = vec![
        Box::new(github_operations::MockGitHubOperations),
    ];
    assert_eq!(github_clients.len(), 1);

    let config_providers: Vec<Box<dyn ConfigurationProvider>> = vec![
        Box::new(configuration_provider::MockConfigurationProvider),
    ];
    assert_eq!(config_providers.len(), 1);

    let version_calculators: Vec<Box<dyn VersionCalculator>> = vec![
        Box::new(version_calculator::MockVersionCalculator),
    ];
    assert_eq!(version_calculators.len(), 1);
}

/// Test versioning strategy enum variants
#[test]
fn test_versioning_strategy_variants() {
    let conventional = version_calculator::VersioningStrategy::ConventionalCommits {
        custom_types: HashMap::new(),
        include_prerelease: true,
    };

    let calendar = version_calculator::VersioningStrategy::CalendarVersion {
        format: "YYYY.MM.DD".to_string(),
        include_build: false,
    };

    let external = version_calculator::VersioningStrategy::External {
        command: "calculate-version".to_string(),
        env_vars: HashMap::new(),
        timeout_ms: 30000,
    };

    let manual = version_calculator::VersioningStrategy::Manual {
        next_version: SemanticVersion {
            major: 2,
            minor: 0,
            patch: 0,
            prerelease: None,
            build: None,
        },
    };

    // Test serialization of all variants
    assert!(serde_json::to_string(&conventional).is_ok());
    assert!(serde_json::to_string(&calendar).is_ok());
    assert!(serde_json::to_string(&external).is_ok());
    assert!(serde_json::to_string(&manual).is_ok());
}
