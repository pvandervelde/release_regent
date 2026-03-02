# Configuration Reference

**Last Updated**: 2025-07-19
**Status**: Complete - Addresses Spec Feedback

## Overview

This document defines the complete configuration schema for Release Regent, including validation rules, template formats, and operational definitions that address the gaps identified in spec feedback.

## Configuration Architecture

### Hierarchical Configuration Loading

Release Regent uses a hierarchical configuration system that merges settings from multiple sources:

1. **Application Defaults**: Built-in sensible defaults
2. **Application-Wide Configuration**: Global settings for the entire installation
3. **Repository-Specific Overrides**: Per-repository customizations

### Configuration Sources

**Application-Wide Configuration**:

- CLI: Configuration file specified via `--config` flag or `RR_CONFIG_PATH` environment variable
- Serverless: Configuration stored in cloud configuration service or environment variables

**Repository-Specific Configuration**:

- File: `.release-regent.yml` in repository root
- GitHub: Configuration stored in `.github/release-regent.yml`
- Fallback: No repository config means use application defaults

## Configuration Schema

### Schema Versioning and Migration

**Configuration Schema Version**: `version: "1.0"`

Release Regent uses semantic versioning for configuration schema compatibility:

- **Major version changes**: Breaking changes requiring migration (e.g., `1.x` → `2.x`)
- **Minor version changes**: New optional fields, backward compatible (e.g., `1.0` → `1.1`)
- **Patch version changes**: Bug fixes, clarifications (e.g., `1.0.1` → `1.0.2`)

**Migration Strategy**:

```yaml
# Version 1.0 (current)
version: "1.0"

# Version 1.1 (future - new optional fields)
version: "1.1"
concurrency:  # New optional section
  retry_policy:
    max_attempts: 3

# Version 2.0 (future - breaking changes)
version: "2.0"
# Could require field renames, structure changes
```

**Backward Compatibility Rules**:

1. **Missing version field**: Assumes `version: "1.0"` with deprecation warning
2. **Older minor versions**: Loads successfully with default values for new fields
3. **Future minor versions**: Loads successfully, ignores unknown fields
4. **Major version mismatch**: Fails with clear migration instructions

### Root Configuration Structure

```yaml
version: "1.0"  # Configuration schema version (required)

# Core settings (required for basic operation)
version_prefix: "v"           # Prefix for version tags and branches
branches:
  main: "main"               # Main branch name (required)

# Release PR settings
release_pr:
  title_template: "chore(release): ${version}"
  body_template: |
    ## Release ${version}

    ${changelog}

    ### Metadata
    - **Commits**: ${commit_count} changes since ${previous_version}
    - **Generated**: ${date}
    - **Correlation ID**: ${correlation_id}
  draft: false
  labels: ["release"]
  assignees: []

# GitHub release settings
releases:
  draft: false
  prerelease: false
  generate_notes: true
  cleanup_branches: true

# Versioning strategy
versioning:
  strategy: "conventional"    # "conventional" | "external"
  external:
    command: "./scripts/calculate-version.sh"
    timeout_ms: 30000
    working_directory: "."
  allow_override: true        # Allow PR comment overrides
  fallback_strategy: "patch"  # "major" | "minor" | "patch"

# Error handling
error_handling:
  max_retries: 5
  backoff_multiplier: 2
  initial_delay_ms: 100
  max_delay_ms: 30000
  jitter_percent: 0.25

# Notifications
notifications:
  enabled: false
  strategy: "none"            # "none" | "github_issue" | "webhook" | "slack"
  github_issue:
    labels: ["release-regent", "bug"]
    assignees: []
  webhook:
    url: "https://example.com/webhook"
    headers: {}
    timeout_ms: 5000
  slack:
    webhook_url: "https://hooks.slack.com/services/XXX/YYY/ZZZ"
    channel: "#releases"

# Logging and observability
logging:
  level: "info"              # "debug" | "info" | "warn" | "error"
  format: "json"             # "json" | "text"
  correlation_ids: true
```

## Validation Rules

### Schema Version Validation (Critical)

The configuration version field is validated first, before any other processing:

**`version`**:

- **Required**: Must be present in all configuration files
- **Format**: Semantic version string (major.minor or major.minor.patch)
- **Validation**: Must match supported version pattern `^[0-9]+\.[0-9]+(\.[0-9]+)?$`
- **Supported Versions**: Currently supports `1.0`, `1.x` (where x ≥ 0)
- **Error Handling**:
  - Missing: "Configuration version is required. Add 'version: \"1.0\"' to your configuration"
  - Invalid format: "Configuration version must be in format 'major.minor' or 'major.minor.patch'"
  - Unsupported major: "Configuration version {version} is not supported. Current supported versions: 1.x"
  - Future minor: Warning only, loads with unknown fields ignored

**Migration Support**:

```rust
#[derive(Debug, Clone)]
pub struct ConfigVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: Option<u32>,
}

impl ConfigVersion {
    pub fn is_compatible_with(&self, supported: &ConfigVersion) -> CompatibilityResult {
        match self.major.cmp(&supported.major) {
            std::cmp::Ordering::Less => CompatibilityResult::Outdated,
            std::cmp::Ordering::Greater => CompatibilityResult::TooNew,
            std::cmp::Ordering::Equal => {
                if self.minor <= supported.minor {
                    CompatibilityResult::Compatible
                } else {
                    CompatibilityResult::ForwardCompatible
                }
            }
        }
    }
}

pub enum CompatibilityResult {
    Compatible,           // Same or older minor version - fully supported
    ForwardCompatible,    // Newer minor version - load with warnings
    Outdated,            // Older major version - needs migration
    TooNew,              // Newer major version - unsupported
}
```

### Critical Field Validation (Strict)

These fields must be correct or the application will fail to start:

**`branches.main`**:

- **Required**: Must be present
- **Format**: Valid Git branch name (alphanumeric, hyphens, underscores, forward slashes)
- **Validation**: `^[a-zA-Z0-9/_-]+$`
- **Error**: "Main branch name is required and must be a valid Git branch name"
- **Example**: `"main"`, `"master"`, `"develop"`

**`version_prefix`**:

- **Required**: Must be present
- **Format**: String that will be prepended to versions
- **Common Values**: `"v"`, `""` (empty), `"release-"`
- **Validation**: No whitespace, no special characters except hyphens and underscores
- **Error**: "Version prefix must not contain whitespace or special characters"

**`versioning.external.command`** (if external strategy):

- **Required**: When `versioning.strategy` is `"external"`
- **Validation**: File must exist and be executable
- **Error**: "External versioning command does not exist or is not executable: {path}"
- **Security**: Command must be within repository boundaries (no `../` traversal)

**`notifications.webhook.url`** (if webhook notifications):

- **Required**: When `notifications.strategy` is `"webhook"`
- **Format**: Valid HTTPS URL
- **Validation**: `^https://[a-zA-Z0-9.-]+(/.*)?$`
- **Error**: "Webhook URL must be a valid HTTPS URL"

### Optional Field Validation (Defaults)

These fields have sensible defaults if not specified:

**Template Fields**: All template strings have working defaults
**Timeout Values**: Performance-tested defaults for all timeout settings
**Boolean Flags**: Safe default values (false for draft modes, true for cleanup)
**Retry Settings**: Optimized for reliability without being overly aggressive

### Validation Implementation

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    pub version_prefix: String,
    pub branches: BranchConfig,
    pub release_pr: ReleasePrConfig,
    pub releases: ReleaseConfig,
    pub versioning: VersioningConfig,
    pub error_handling: ErrorHandlingConfig,
    pub notifications: NotificationConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug)]
pub struct ConfigValidationError {
    pub field_path: String,
    pub error_type: ValidationErrorType,
    pub message: String,
    pub suggestion: Option<String>,
}

pub fn validate_configuration(config: &Configuration) -> Result<(), Vec<ConfigValidationError>> {
    let mut errors = Vec::new();

    // Validate main branch name
    if config.branches.main.is_empty() {
        errors.push(ConfigValidationError {
            field_path: "branches.main".to_string(),
            error_type: ValidationErrorType::Missing,
            message: "Main branch name is required".to_string(),
            suggestion: Some("Add 'branches.main: \"main\"' to your configuration".to_string()),
        });
    } else if !is_valid_branch_name(&config.branches.main) {
        errors.push(ConfigValidationError {
            field_path: "branches.main".to_string(),
            error_type: ValidationErrorType::InvalidFormat,
            message: format!("Invalid branch name: {}", config.branches.main),
            suggestion: Some("Branch names must contain only alphanumeric characters, hyphens, underscores, and forward slashes".to_string()),
        });
    }

    // Validate external versioning command
    if config.versioning.strategy == VersioningStrategy::External {
        if let Some(ref external_config) = config.versioning.external {
            if !Path::new(&external_config.command).exists() {
                errors.push(ConfigValidationError {
                    field_path: "versioning.external.command".to_string(),
                    error_type: ValidationErrorType::InvalidReference,
                    message: format!("External command does not exist: {}", external_config.command),
                    suggestion: Some("Ensure the script exists and has execute permissions".to_string()),
                });
            }
        } else {
            errors.push(ConfigValidationError {
                field_path: "versioning.external".to_string(),
                error_type: ValidationErrorType::Missing,
                message: "External versioning configuration required when strategy is 'external'".to_string(),
                suggestion: Some("Add 'versioning.external.command' to your configuration".to_string()),
            });
        }
    }

    // Validate webhook URL if webhook notifications enabled
    if config.notifications.strategy == NotificationStrategy::Webhook {
        if let Some(ref webhook_config) = config.notifications.webhook {
            if let Err(_) = Url::parse(&webhook_config.url) {
                errors.push(ConfigValidationError {
                    field_path: "notifications.webhook.url".to_string(),
                    error_type: ValidationErrorType::InvalidFormat,
                    message: format!("Invalid webhook URL: {}", webhook_config.url),
                    suggestion: Some("Webhook URL must be a valid HTTPS URL".to_string()),
                });
            }
        } else {
            errors.push(ConfigValidationError {
                field_path: "notifications.webhook".to_string(),
                error_type: ValidationErrorType::Missing,
                message: "Webhook configuration required when strategy is 'webhook'".to_string(),
                suggestion: Some("Add 'notifications.webhook.url' to your configuration".to_string()),
            });
        }
    }

    // Validate template syntax
    if let Err(template_errors) = validate_templates(config) {
        errors.extend(template_errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn is_valid_branch_name(name: &str) -> bool {
    // Git branch name validation rules
    let pattern = regex::Regex::new(r"^[a-zA-Z0-9/_-]+$").unwrap();
    pattern.is_match(name) && !name.starts_with('-') && !name.ends_with('/')
}
```

## Template System

### Template Variables

All template strings support variable substitution using `${variable}` syntax:

**`${version}`**: Semantic version without prefix (e.g., "1.2.3")
**`${version_tag}`**: Version with configured prefix (e.g., "v1.2.3")
**`${changelog}`**: Generated changelog content with markdown formatting
**`${commit_count}`**: Number of commits since last release (integer)
**`${date}`**: Current date in ISO 8601 format (e.g., "2025-07-19T10:30:00Z")
**`${correlation_id}`**: Unique request identifier for tracing
**`${previous_version}`**: Previous release version for context
**`${repository}`**: Repository name in "owner/repo" format
**`${branch}`**: Target branch name (usually main branch)

### Template Validation

```rust
pub fn validate_templates(config: &Configuration) -> Result<(), Vec<ConfigValidationError>> {
    let mut errors = Vec::new();

    // Validate PR title template
    if let Err(error) = validate_template_syntax(&config.release_pr.title_template) {
        errors.push(ConfigValidationError {
            field_path: "release_pr.title_template".to_string(),
            error_type: ValidationErrorType::InvalidFormat,
            message: format!("Invalid template syntax: {}", error),
            suggestion: Some("Check for unclosed variables or invalid variable names".to_string()),
        });
    }

    // Validate PR body template
    if let Err(error) = validate_template_syntax(&config.release_pr.body_template) {
        errors.push(ConfigValidationError {
            field_path: "release_pr.body_template".to_string(),
            error_type: ValidationErrorType::InvalidFormat,
            message: format!("Invalid template syntax: {}", error),
            suggestion: Some("Check for unclosed variables or invalid variable names".to_string()),
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_template_syntax(template: &str) -> Result<(), String> {
    let valid_variables = [
        "version", "version_tag", "changelog", "commit_count",
        "date", "correlation_id", "previous_version", "repository", "branch"
    ];

    // Find all variable references
    let var_pattern = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();

    for capture in var_pattern.captures_iter(template) {
        let var_name = &capture[1];
        if !valid_variables.contains(&var_name) {
            return Err(format!("Unknown template variable: {}", var_name));
        }
    }

    // Check for unclosed variables
    if template.contains("${") && template.matches("${").count() != template.matches("}").count() {
        return Err("Unclosed template variable".to_string());
    }

    Ok(())
}
```

### Template Rendering

```rust
pub struct TemplateVariables {
    pub version: String,
    pub version_tag: String,
    pub changelog: String,
    pub commit_count: u32,
    pub date: String,
    pub correlation_id: String,
    pub previous_version: Option<String>,
    pub repository: String,
    pub branch: String,
}

pub fn render_template(template: &str, variables: &TemplateVariables) -> Result<String, TemplateError> {
    let mut result = template.to_string();

    // Replace all variables
    result = result.replace("${version}", &variables.version);
    result = result.replace("${version_tag}", &variables.version_tag);
    result = result.replace("${changelog}", &variables.changelog);
    result = result.replace("${commit_count}", &variables.commit_count.to_string());
    result = result.replace("${date}", &variables.date);
    result = result.replace("${correlation_id}", &variables.correlation_id);
    result = result.replace("${repository}", &variables.repository);
    result = result.replace("${branch}", &variables.branch);

    if let Some(ref prev_version) = variables.previous_version {
        result = result.replace("${previous_version}", prev_version);
    } else {
        result = result.replace("${previous_version}", "initial release");
    }

    Ok(result)
}
```

### Fallback Templates

When template rendering fails, use these fallback templates:

```rust
const FALLBACK_PR_TITLE: &str = "chore(release): ${version}";

const FALLBACK_PR_BODY: &str = r#"## Release ${version}

${changelog}

### Metadata
- **Commits**: ${commit_count} changes
- **Generated**: ${date}
- **Correlation ID**: ${correlation_id}

---
*This release was automatically generated by Release Regent*"#;
```

## Configuration Examples

### Minimal Configuration

```yaml
# Minimal working configuration
version_prefix: "v"
branches:
  main: "main"
```

### Standard Configuration

```yaml
# Standard configuration for most repositories
version_prefix: "v"
branches:
  main: "main"

release_pr:
  title_template: "chore(release): ${version}"
  body_template: |
    ## Release ${version}

    ${changelog}

    ### Metadata
    - **Commits**: ${commit_count} changes since ${previous_version}
    - **Generated**: ${date}
  draft: false
  labels: ["release", "automated"]

releases:
  draft: false
  prerelease: false
  generate_notes: true
  cleanup_branches: true

versioning:
  strategy: "conventional"
  allow_override: true
```

### Advanced Configuration

```yaml
# Advanced configuration with external versioning and notifications
version_prefix: "v"
branches:
  main: "develop"

release_pr:
  title_template: "[RELEASE] ${version} - ${commit_count} changes"
  body_template: |
    # 🚀 Release ${version}

    This release contains ${commit_count} changes since ${previous_version}.

    ## What's Changed

    ${changelog}

    ## Release Information

    - **Repository**: ${repository}
    - **Branch**: ${branch}
    - **Generated**: ${date}
    - **Correlation ID**: ${correlation_id}

    ## Next Steps

    Once this PR is merged, the release will be automatically published to GitHub.
  draft: false
  labels: ["release", "automated", "v${version}"]
  assignees: ["@release-team"]

releases:
  draft: false
  prerelease: false
  generate_notes: false  # Use our custom changelog instead
  cleanup_branches: true

versioning:
  strategy: "external"
  external:
    command: "./scripts/calculate-version.py"
    timeout_ms: 15000
    working_directory: "."
  allow_override: true
  fallback_strategy: "patch"

error_handling:
  max_retries: 3
  backoff_multiplier: 1.5
  initial_delay_ms: 200
  max_delay_ms: 10000

notifications:
  enabled: true
  strategy: "slack"
  slack:
    webhook_url: "${SLACK_WEBHOOK_URL}"  # From environment
    channel: "#releases"

logging:
  level: "info"
  format: "json"
  correlation_ids: true
```

### Repository-Specific Override Examples

**Disable notifications for a specific repository**:

```yaml
# .release-regent.yml
notifications:
  strategy: "none"
```

**Use external versioning for Rust crates**:

```yaml
# .release-regent.yml
versioning:
  strategy: "external"
  external:
    command: "./scripts/cargo-version.sh"
    timeout_ms: 10000
```

**Custom templates for documentation repositories**:

```yaml
# .release-regent.yml
release_pr:
  title_template: "docs(release): ${version} - Update documentation"
  body_template: |
    ## Documentation Release ${version}

    ${changelog}

    This release updates the documentation with the following changes.
```

## Environment Variable Support

Configuration values can reference environment variables using `${VAR_NAME}` syntax:

```yaml
notifications:
  slack:
    webhook_url: "${SLACK_WEBHOOK_URL}"

versioning:
  external:
    command: "${VERSION_SCRIPT_PATH}/calculate-version.sh"
```

**Variable Resolution**:

1. Check for environment variable
2. Use literal value if environment variable not found
3. Fail validation if required environment variable is missing

## Configuration Loading Process

```rust
pub async fn load_configuration(
    app_config_path: Option<&Path>,
    repo_path: &Path
) -> Result<Configuration, ConfigError> {
    // 1. Start with built-in defaults
    let mut config = Configuration::default();

    // 2. Load application-wide configuration
    if let Some(app_config_path) = app_config_path {
        let app_config = load_config_file(app_config_path).await?;
        config = merge_configurations(config, app_config)?;
    }

    // 3. Look for repository-specific configuration
    let repo_config_paths = [
        repo_path.join(".release-regent.yml"),
        repo_path.join(".github/release-regent.yml"),
    ];

    for repo_config_path in &repo_config_paths {
        if repo_config_path.exists() {
            let repo_config = load_config_file(repo_config_path).await?;
            config = merge_configurations(config, repo_config)?;
            break;
        }
    }

    // 4. Resolve environment variables
    config = resolve_environment_variables(config)?;

    // 5. Validate final configuration
    validate_configuration(&config)?;

    Ok(config)
}
```

## Configuration Error Messages

When configuration validation fails, provide clear, actionable error messages:

```
❌ Configuration validation failed:

1. branches.main (MISSING)
   Main branch name is required
   → Add 'branches.main: "main"' to your configuration

2. versioning.external.command (INVALID_REFERENCE)
   External command does not exist: ./scripts/version.sh
   → Ensure the script exists and has execute permissions

3. release_pr.title_template (INVALID_FORMAT)
   Unknown template variable: release_version
   → Use ${version} instead of ${release_version}

Configuration file: /path/to/.release-regent.yml
```

This comprehensive configuration reference addresses all the validation and template concerns raised in the spec feedback while providing practical examples and clear error handling guidance.
