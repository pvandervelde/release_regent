//! Configuration builder for creating test repository configuration data

use crate::builders::{helpers::*, TestDataBuilder};
use release_regent_core::{config::*, traits::configuration_provider::RepositoryConfig};

/// Builder for creating test repository configuration data
#[derive(Debug, Clone)]
pub struct ConfigurationBuilder {
    config: ReleaseRegentConfig,
    name: String,
    owner: String,
}

impl ConfigurationBuilder {
    /// Create a new configuration builder with defaults
    pub fn new() -> Self {
        Self {
            config: create_default_config(),
            name: generate_repo_name(),
            owner: generate_github_login(),
        }
    }

    /// Set configuration
    pub fn with_config(mut self, config: ReleaseRegentConfig) -> Self {
        self.config = config;
        self
    }

    /// Set repository name
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Set repository owner
    pub fn with_owner(mut self, owner: &str) -> Self {
        self.owner = owner.to_string();
        self
    }

    /// Create configuration for a specific repository
    pub fn for_repository(owner: &str, name: &str) -> Self {
        Self::new().with_owner(owner).with_name(name)
    }
}

impl TestDataBuilder<RepositoryConfig> for ConfigurationBuilder {
    fn build(self) -> RepositoryConfig {
        RepositoryConfig {
            config: self.config,
            name: self.name,
            owner: self.owner,
        }
    }

    fn reset(self) -> Self {
        Self::new()
    }
}

impl Default for ConfigurationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a default ReleaseRegentConfig for testing
fn create_default_config() -> ReleaseRegentConfig {
    ReleaseRegentConfig {
        core: CoreConfig {
            version_prefix: "v".to_string(),
            branches: BranchConfig {
                main: "main".to_string(),
            },
        },
        release_pr: ReleasePrConfig {
            title_template: "Release {{version}}".to_string(),
            body_template: "Release notes for {{version}}".to_string(),
            draft: false,
        },
        releases: ReleasesConfig {
            draft: false,
            prerelease: false,
            generate_notes: true,
        },
        error_handling: ErrorHandlingConfig {
            max_retries: 3,
            backoff_multiplier: 2.0,
            initial_delay_ms: 1000,
        },
        notifications: NotificationConfig {
            enabled: false,
            strategy: NotificationStrategy::None,
            github_issue: None,
            webhook: None,
            slack: None,
        },
        versioning: VersioningConfig {
            strategy: VersioningStrategy::Conventional,
            external: None,
            allow_override: false,
        },
    }
}
