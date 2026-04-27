//! Configuration management for Release Regent
//!
//! This module handles loading and validating Release Regent configuration from
//! YAML files with support for both application-wide and repository-specific settings.

use crate::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info};

/// Branch configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchConfig {
    /// Main branch name
    #[serde(default = "default_main_branch")]
    pub main: String,
}

fn default_main_branch() -> String {
    "main".to_string()
}

impl Default for BranchConfig {
    fn default() -> Self {
        Self {
            main: default_main_branch(),
        }
    }
}

/// Core Release Regent settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    /// Version prefix (e.g., "v" for "v1.0.0")
    #[serde(default = "default_version_prefix")]
    pub version_prefix: String,
    /// Branch configuration
    #[serde(default)]
    pub branches: BranchConfig,
}

fn default_version_prefix() -> String {
    "v".to_string()
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            version_prefix: default_version_prefix(),
            branches: BranchConfig::default(),
        }
    }
}

/// Error handling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHandlingConfig {
    /// Maximum number of retries
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Backoff multiplier for retries
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    /// Initial delay in milliseconds
    #[serde(default = "default_initial_delay_ms")]
    pub initial_delay_ms: u64,
}

fn default_max_retries() -> u32 {
    5
}
fn default_backoff_multiplier() -> f64 {
    2.0
}
fn default_initial_delay_ms() -> u64 {
    1000
}

impl Default for ErrorHandlingConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            backoff_multiplier: default_backoff_multiplier(),
            initial_delay_ms: default_initial_delay_ms(),
        }
    }
}

/// External versioning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalVersioningConfig {
    /// Command to execute for version calculation
    pub command: String,
    /// Timeout in milliseconds
    pub timeout_ms: u64,
}

/// GitHub issue notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubIssueConfig {
    /// Labels to apply to issues
    pub labels: Vec<String>,
    /// Users to assign to issues
    pub assignees: Vec<String>,
}

/// Notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Whether notifications are enabled
    #[serde(default = "default_notifications_enabled")]
    pub enabled: bool,
    /// Notification strategy
    #[serde(default = "default_notification_strategy")]
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

fn default_notifications_enabled() -> bool {
    true
}
fn default_notification_strategy() -> NotificationStrategy {
    NotificationStrategy::GitHubIssue
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: default_notifications_enabled(),
            strategy: default_notification_strategy(),
            github_issue: Some(GitHubIssueConfig {
                labels: vec!["release-regent".to_string(), "bug".to_string()],
                assignees: vec![],
            }),
            webhook: None,
            slack: None,
        }
    }
}

/// Release PR configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleasePrConfig {
    /// PR title template
    #[serde(default = "default_pr_title_template")]
    pub title_template: String,
    /// PR body template
    #[serde(default = "default_pr_body_template")]
    pub body_template: String,
    /// Whether to create PRs as drafts
    #[serde(default)]
    pub draft: bool,
}

fn default_pr_title_template() -> String {
    "chore(release): ${version}".to_string()
}
fn default_pr_body_template() -> String {
    "## Changelog\n\n${changelog}".to_string()
}

impl Default for ReleasePrConfig {
    fn default() -> Self {
        Self {
            title_template: default_pr_title_template(),
            body_template: default_pr_body_template(),
            draft: false,
        }
    }
}

/// Main Release Regent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseRegentConfig {
    /// Core settings
    #[serde(default)]
    pub core: CoreConfig,
    /// Release PR settings
    #[serde(default)]
    pub release_pr: ReleasePrConfig,
    /// GitHub release settings
    #[serde(default)]
    pub releases: ReleasesConfig,
    /// Error handling configuration
    #[serde(default)]
    pub error_handling: ErrorHandlingConfig,
    /// Notification settings
    #[serde(default)]
    pub notifications: NotificationConfig,
    /// Versioning strategy
    #[serde(default)]
    pub versioning: VersioningConfig,
}

/// GitHub releases configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleasesConfig {
    /// Whether to create releases as drafts
    #[serde(default)]
    pub draft: bool,
    /// Whether to mark as prerelease
    #[serde(default)]
    pub prerelease: bool,
    /// Whether to generate release notes automatically
    #[serde(default = "default_generate_notes")]
    pub generate_notes: bool,
}

fn default_generate_notes() -> bool {
    true
}

impl Default for ReleasesConfig {
    fn default() -> Self {
        Self {
            draft: false,
            prerelease: false,
            generate_notes: default_generate_notes(),
        }
    }
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
    #[serde(default = "default_versioning_strategy")]
    pub strategy: VersioningStrategy,
    /// External versioning settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<ExternalVersioningConfig>,
    /// Whether to allow PR comment overrides
    #[serde(default = "default_allow_override")]
    pub allow_override: bool,
}

fn default_versioning_strategy() -> VersioningStrategy {
    VersioningStrategy::Conventional
}
fn default_allow_override() -> bool {
    true
}

impl Default for VersioningConfig {
    fn default() -> Self {
        Self {
            strategy: default_versioning_strategy(),
            external: None,
            allow_override: default_allow_override(),
        }
    }
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

/// Notification strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStrategy {
    /// Create GitHub issues for errors
    #[serde(rename = "github_issue")]
    GitHubIssue,
    /// Send webhook notifications
    Webhook,
    /// Send Slack notifications
    Slack,
    /// No notifications
    None,
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

impl Default for ReleaseRegentConfig {
    fn default() -> Self {
        Self {
            core: CoreConfig::default(),
            release_pr: ReleasePrConfig::default(),
            releases: ReleasesConfig::default(),
            error_handling: ErrorHandlingConfig::default(),
            notifications: NotificationConfig::default(),
            versioning: VersioningConfig::default(),
        }
    }
}

impl ReleaseRegentConfig {
    /// Load configuration from a YAML file
    ///
    /// # Arguments
    /// * `path` - Path to the configuration file
    ///
    /// # Errors
    /// - `CoreError::Io` - Failed to read the file
    /// - `CoreError::Config` - Failed to parse or validate the configuration
    #[allow(clippy::result_large_err)] // CoreError is intentionally large; established pattern
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
    ///
    /// # Errors
    /// - `CoreError::Config` - A required field is empty, contains invalid characters,
    ///   or is inconsistent with other fields (e.g. webhook URL missing when strategy is `webhook`)
    #[allow(clippy::result_large_err)] // CoreError is intentionally large; established pattern
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
