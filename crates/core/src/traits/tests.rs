//! Tests for core trait data structures
//!
//! This module tests serialization/deserialization and basic invariants of the
//! data structures defined in the trait modules.  Trait object-safety, mock
//! error handling, async compatibility, and collection storage tests live in
//! `crates/core/tests/trait_contracts.rs` (integration tests) so that they can
//! use `release_regent_testing` mocks without triggering a circular compilation
//! unit issue.

use crate::traits::*;
use crate::versioning::SemanticVersion;
use chrono::Utc;
use std::collections::HashMap;

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
    let deserialized: github_operations::Repository =
        serde_json::from_str(&json).expect("Repository should deserialize");
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
    let deserialized: github_operations::Tag =
        serde_json::from_str(&json).expect("Tag should deserialize");
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
    let deserialized: version_calculator::VersionContext =
        serde_json::from_str(&json).expect("VersionContext should deserialize");
    assert_eq!(context.owner, deserialized.owner);
    assert_eq!(context.repo, deserialized.repo);

    // Test VersionBump enum
    let bump = version_calculator::VersionBump::Major;
    let json = serde_json::to_string(&bump).expect("VersionBump should serialize");
    let deserialized: version_calculator::VersionBump =
        serde_json::from_str(&json).expect("VersionBump should deserialize");
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
        format: "toml".to_string(),
        loaded_at: Utc::now(),
        location: "/path/to/config.toml".to_string(),
        source_type: "file".to_string(),
    };

    let json = serde_json::to_string(&source).expect("ConfigurationSource should serialize");
    let deserialized: configuration_provider::ConfigurationSource =
        serde_json::from_str(&json).expect("ConfigurationSource should deserialize");
    assert_eq!(source.source_type, deserialized.source_type);
    assert_eq!(source.location, deserialized.location);
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
