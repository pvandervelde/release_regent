//! Configuration validation using JSON Schema and custom rules.

use crate::errors::ConfigProviderResult;
use release_regent_core::config::{ReleaseRegentConfig, VersioningStrategy};
use std::collections::HashMap;

/// Result of configuration validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the configuration is valid
    pub is_valid: bool,
    /// List of validation errors
    pub errors: Vec<String>,
    /// List of validation warnings
    pub warnings: Vec<String>,
    /// Validation metadata
    pub metadata: HashMap<String, String>,
}

impl ValidationResult {
    /// Create a new valid result
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new invalid result with errors
    pub fn invalid(errors: Vec<String>) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a warning to the result
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Add warnings to the result
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings.extend(warnings);
        self
    }

    /// Add metadata to the result
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Check if there are any validation issues (errors or warnings)
    pub fn has_issues(&self) -> bool {
        !self.errors.is_empty() || !self.warnings.is_empty()
    }
}

/// Configuration validator with JSON Schema support and custom validation rules
pub struct ConfigValidator {
    /// Whether to enforce strict validation
    strict_mode: bool,
    /// Custom validation rules
    custom_rules: Vec<Box<dyn ValidationRule>>,
}

impl ConfigValidator {
    /// Create a new validator
    pub fn new() -> Self {
        Self {
            strict_mode: false,
            custom_rules: Vec::new(),
        }
    }

    /// Create a new validator in strict mode
    pub fn strict() -> Self {
        Self {
            strict_mode: true,
            custom_rules: Vec::new(),
        }
    }

    /// Add a custom validation rule
    pub fn with_rule(mut self, rule: Box<dyn ValidationRule>) -> Self {
        self.custom_rules.push(rule);
        self
    }

    /// Validate a configuration
    pub fn validate(&self, config: &ReleaseRegentConfig) -> ConfigProviderResult<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut metadata = HashMap::new();

        // Basic structural validation
        self.validate_structure(config, &mut errors, &mut warnings)?;

        // Custom rules validation
        for rule in &self.custom_rules {
            match rule.validate(config) {
                Ok(result) => {
                    errors.extend(result.errors);
                    warnings.extend(result.warnings);
                    metadata.extend(result.metadata);
                }
                Err(e) => {
                    errors.push(format!("Custom validation rule failed: {}", e));
                }
            }
        }

        // In strict mode, treat warnings as errors
        if self.strict_mode {
            errors.extend(warnings.clone());
            warnings.clear();
        }

        let is_valid = errors.is_empty();
        Ok(ValidationResult {
            is_valid,
            errors,
            warnings,
            metadata,
        })
    }

    /// Validate basic configuration structure
    fn validate_structure(
        &self,
        config: &ReleaseRegentConfig,
        errors: &mut Vec<String>,
        warnings: &mut Vec<String>,
    ) -> ConfigProviderResult<()> {
        // Validate branch configuration
        let branch_config = &config.core.branches;
        if branch_config.main.is_empty() {
            errors.push("Main branch name cannot be empty".to_string());
        }

        // Validate versioning configuration
        let versioning = &config.versioning;

        // Validate versioning specific settings based on strategy
        match versioning.strategy {
            VersioningStrategy::External => {
                if versioning.external.is_none() {
                    errors.push(
                        "External versioning strategy requires external configuration".to_string(),
                    );
                }
            }
            VersioningStrategy::Conventional => {
                // No additional validation needed for conventional commits
            }
        }

        // Validate webhook configuration
        if let Some(webhook) = &config.notifications.webhook {
            if webhook.url.is_empty() {
                errors.push("Webhook URL cannot be empty when webhook is configured".to_string());
            }

            // Validate URL format (basic check)
            if !webhook.url.starts_with("http://") && !webhook.url.starts_with("https://") {
                warnings.push("Webhook URL should use HTTP or HTTPS protocol".to_string());
            }
        }

        // Validate notification configuration
        let notifications = &config.notifications;
        if notifications.enabled {
            if let Some(slack) = &notifications.slack {
                if slack.webhook_url.is_empty() {
                    errors
                        .push("Slack notifications enabled but webhook URL is missing".to_string());
                }
            }
        }

        Ok(())
    }

    /// Check if a string is a valid semantic version
    fn is_valid_semver(&self, version: &str) -> bool {
        // Simple semantic version validation without regex dependency
        let parts: Vec<&str> = version.split('.').collect();

        // Must have at least major.minor.patch
        if parts.len() < 3 {
            return false;
        }

        // Check if first three parts are valid numbers
        for i in 0..3 {
            if parts[i].parse::<u32>().is_err() {
                return false;
            }
        }

        // If there are more than 3 parts, they could be pre-release or build metadata
        // For now, we'll accept any additional parts as valid
        true
    }
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for custom validation rules
pub trait ValidationRule: Send + Sync {
    /// Validate a configuration and return a validation result
    fn validate(&self, config: &ReleaseRegentConfig) -> ConfigProviderResult<ValidationResult>;

    /// Get the name of this validation rule
    fn name(&self) -> &str;

    /// Get the description of this validation rule
    fn description(&self) -> &str;
}

/// Example custom validation rule for GitHub repository settings
pub struct GitHubRepositoryRule;

impl ValidationRule for GitHubRepositoryRule {
    fn validate(&self, config: &ReleaseRegentConfig) -> ConfigProviderResult<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Validate GitHub-specific settings
        let releases = &config.releases;

        // Check if releases are configured properly
        if !releases.draft && !releases.prerelease && !releases.generate_notes {
            warnings.push(
                "All release options are disabled - releases may not be very informative"
                    .to_string(),
            );
        }

        // Validate release PR configuration
        let release_pr = &config.release_pr;
        if release_pr.title_template.is_empty() {
            warnings.push("Release PR title template is empty".to_string());
        }
        if release_pr.body_template.is_empty() {
            warnings.push("Release PR body template is empty".to_string());
        }

        Ok(if errors.is_empty() {
            ValidationResult::valid().with_warnings(warnings)
        } else {
            ValidationResult::invalid(errors).with_warnings(warnings)
        })
    }

    fn name(&self) -> &str {
        "github_repository"
    }

    fn description(&self) -> &str {
        "Validates GitHub repository-specific configuration settings"
    }
}

/// Custom validation rule for webhook security
pub struct WebhookSecurityRule;

impl ValidationRule for WebhookSecurityRule {
    fn validate(&self, config: &ReleaseRegentConfig) -> ConfigProviderResult<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if let Some(webhook) = &config.notifications.webhook {
            // Basic webhook URL validation
            if webhook.url.is_empty() {
                errors.push(
                    "Webhook URL is empty - this is required for webhook functionality".to_string(),
                );
            } else {
                // Check if using HTTPS for security
                if webhook.url.starts_with("http://") {
                    warnings.push(
                        "Webhook URL uses HTTP instead of HTTPS - consider using HTTPS for better security"
                            .to_string(),
                    );
                }

                // Check if URL looks valid
                if !webhook.url.contains("://") {
                    errors.push(
                        "Webhook URL appears to be malformed - should include protocol (http:// or https://)"
                            .to_string(),
                    );
                }
            }

            // Check for authentication headers
            if webhook.headers.is_empty() {
                warnings.push(
                    "No authentication headers configured for webhook - consider adding authorization headers for security"
                        .to_string(),
            );
            } else {
                // Check for common authentication headers
                let has_auth = webhook.headers.keys().any(|key| {
                    let key_lower = key.to_lowercase();
                    key_lower.contains("authorization")
                        || key_lower.contains("x-api-key")
                        || key_lower.contains("token")
                });

                if !has_auth {
                    warnings.push(
                        "No obvious authentication headers found - consider adding authorization for security"
                            .to_string(),
                    );
                }
            }
        }

        Ok(if errors.is_empty() {
            ValidationResult::valid().with_warnings(warnings)
        } else {
            ValidationResult::invalid(errors).with_warnings(warnings)
        })
    }

    fn name(&self) -> &str {
        "webhook_security"
    }

    fn description(&self) -> &str {
        "Validates webhook security configuration"
    }
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
