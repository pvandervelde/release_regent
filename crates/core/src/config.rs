//! Configuration management for Release Regent
//!
//! This module handles loading and validating Release Regent configuration from
//! YAML files with support for both application-wide and repository-specific settings.

use crate::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info};

/// Main Release Regent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseRegentConfig {
    /// Core settings
    pub core: CoreConfig,
    /// Release PR settings
    pub release_pr: ReleasePrConfig,
    /// GitHub release settings
    pub releases: ReleasesConfig,
    /// Error handling configuration
    pub error_handling: ErrorHandlingConfig,
    /// Notification settings
    pub notifications: NotificationConfig,
    /// Versioning strategy
    pub versioning: VersioningConfig,
}

/// Core Release Regent settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    /// Version prefix (e.g., "v" for "v1.0.0")
    pub version_prefix: String,
    /// Branch configuration
    pub branches: BranchConfig,
}

/// Branch configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchConfig {
    /// Main branch name
    pub main: String,
}

/// Release PR configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleasePrConfig {
    /// PR title template
    pub title_template: String,
    /// PR body template
    pub body_template: String,
    /// Whether to create PRs as drafts
    pub draft: bool,
}

/// GitHub releases configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleasesConfig {
    /// Whether to create releases as drafts
    pub draft: bool,
    /// Whether to mark as prerelease
    pub prerelease: bool,
    /// Whether to generate release notes automatically
    pub generate_notes: bool,
}

/// Error handling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHandlingConfig {
    /// Maximum number of retries
    pub max_retries: u32,
    /// Backoff multiplier for retries
    pub backoff_multiplier: f64,
    /// Initial delay in milliseconds
    pub initial_delay_ms: u64,
}

/// Notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Whether notifications are enabled
    pub enabled: bool,
    /// Notification strategy
    pub strategy: NotificationStrategy,
    /// GitHub issue notification settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_issue: Option<GitHubIssueConfig>,
    /// Webhook notification settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook: Option<WebhookConfig>,
    /// Slack notification settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slack: Option<SlackConfig>,
}

/// Notification strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStrategy {
    /// Create GitHub issues for errors
    GitHubIssue,
    /// Send webhook notifications
    Webhook,
    /// Send Slack notifications
    Slack,
    /// No notifications
    None,
}

/// GitHub issue notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubIssueConfig {
    /// Labels to apply to issues
    pub labels: Vec<String>,
    /// Users to assign to issues
    pub assignees: Vec<String>,
}

/// Webhook notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Webhook URL
    pub url: String,
    /// Additional headers
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
}

/// Slack notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    /// Slack webhook URL
    pub webhook_url: String,
    /// Channel to post to (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
}

/// Versioning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersioningConfig {
    /// Versioning strategy
    pub strategy: VersioningStrategy,
    /// External versioning settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<ExternalVersioningConfig>,
    /// Whether to allow PR comment overrides
    pub allow_override: bool,
}

/// Versioning strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersioningStrategy {
    /// Use conventional commits
    Conventional,
    /// Use external script/command
    External,
}

/// External versioning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalVersioningConfig {
    /// Command to execute for version calculation
    pub command: String,
    /// Timeout in milliseconds
    pub timeout_ms: u64,
}

impl Default for ReleaseRegentConfig {
    fn default() -> Self {
        Self {
            core: CoreConfig {
                version_prefix: "v".to_string(),
                branches: BranchConfig {
                    main: "main".to_string(),
                },
            },
            release_pr: ReleasePrConfig {
                title_template: "chore(release): ${version}".to_string(),
                body_template: "## Changelog\n\n${changelog}".to_string(),
                draft: false,
            },
            releases: ReleasesConfig {
                draft: false,
                prerelease: false,
                generate_notes: true,
            },
            error_handling: ErrorHandlingConfig {
                max_retries: 5,
                backoff_multiplier: 2.0,
                initial_delay_ms: 1000,
            },
            notifications: NotificationConfig {
                enabled: true,
                strategy: NotificationStrategy::GitHubIssue,
                github_issue: Some(GitHubIssueConfig {
                    labels: vec!["release-regent".to_string(), "bug".to_string()],
                    assignees: vec![],
                }),
                webhook: None,
                slack: None,
            },
            versioning: VersioningConfig {
                strategy: VersioningStrategy::Conventional,
                external: None,
                allow_override: true,
            },
        }
    }
}

impl ReleaseRegentConfig {
    /// Load configuration from a YAML file
    ///
    /// # Arguments
    /// * `path` - Path to the configuration file
    pub async fn load_from_file<P: AsRef<Path>>(path: P) -> CoreResult<Self> {
        let path = path.as_ref();
        info!("Loading configuration from: {}", path.display());

        let content = tokio::fs::read_to_string(path).await?;
        let config: Self = serde_yaml::from_str(&content)?;

        debug!("Configuration loaded successfully");
        config.validate()?;

        Ok(config)
    }

    /// Validate the configuration
    pub fn validate(&self) -> CoreResult<()> {
        // Validate main branch name
        if self.core.branches.main.trim().is_empty() {
            return Err(CoreError::config("Main branch name cannot be empty"));
        }

        // Validate version prefix
        if self.core.version_prefix.contains(char::is_whitespace) {
            return Err(CoreError::config(
                "Version prefix cannot contain whitespace characters",
            ));
        }

        // Validate notification configuration
        match self.notifications.strategy {
            NotificationStrategy::Webhook => {
                if self.notifications.webhook.is_none() {
                    return Err(CoreError::config(
                        "Webhook configuration required when strategy is 'webhook'",
                    ));
                }
            }
            NotificationStrategy::Slack => {
                if self.notifications.slack.is_none() {
                    return Err(CoreError::config(
                        "Slack configuration required when strategy is 'slack'",
                    ));
                }
            }
            _ => {} // No additional validation needed
        }

        // Validate versioning configuration
        if matches!(self.versioning.strategy, VersioningStrategy::External)
            && self.versioning.external.is_none()
        {
            return Err(CoreError::config(
                "External versioning configuration required when strategy is 'external'",
            ));
        }

        debug!("Configuration validation passed");
        Ok(())
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
