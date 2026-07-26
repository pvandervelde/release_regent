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

- File: `.release-regent.toml` in repository root
- GitHub: Configuration stored in `.github/release-regent.toml`
- Fallback: No repository config means use application defaults

## Repository Allow-List and Exclude-List (Bootstrap-Level, Outside the Merge Hierarchy)

Unlike the five-level `ReleaseRegentConfig` hierarchy described below, the repository
allow-list and exclude-list are **server bootstrap settings** evaluated before any
configuration level is resolved and before any GitHub API call is made for a given
event. Together they restrict which repositories the server acts on, independent of
which repositories the GitHub App is *installed on* — see
[System Architecture: Repository Allow-List vs. Installation Scope](../architecture/overview.md#repository-allow-list-vs-installation-scope).

### Allow-List

**Sources** (evaluated in this precedence order):

1. `ALLOWED_REPOS` environment variable — comma-separated glob patterns. Takes
   precedence over the file-based option below if both are set.
2. `allowed_repositories` — a **top-level array key** in the app-level bootstrap file
   (`CONFIG_DIR/release-regent.toml`). This key sits **outside** the
   `ReleaseRegentConfig` schema documented below: it is not nested under `[core]`,
   `[versioning]`, or any other section, and it is never merged with global, group, or
   repository policy. It is read once, directly, at server startup — exactly like
   `CONFIG_DIR` itself.

```toml
# CONFIG_DIR/release-regent.toml — bootstrap-only keys, read once at startup.
# NOT part of the ReleaseRegentConfig merge hierarchy below.
allowed_repositories = ["myorg/service-*", "myorg/lib-a", "otherorg/*"]
excluded_repositories = ["myorg/legacy-secrets"]

# The rest of this file may still contain the normal ReleaseRegentConfig
# app-level sections ([core], [release_pr], etc.) — allowed_repositories and
# excluded_repositories are simply additional top-level keys alongside them.
```

**Pattern syntax**: glob-style; `*` matches any sequence of characters. Patterns are
matched against the full `"owner/repo"` string.

**Default when omitted**: `["*"]` — act on every repository the App receives events
for. Preserves pre-existing behavior for single-repo/small-org installations.

**Explicit empty list**: `[]` denies every event — an operational kill switch,
distinct from leaving the setting unset. (This "empty means deny-all" semantic is
specific to the allow-list — see the Exclude-List section below, where empty/omitted
means the opposite: no exclusions.)

### Exclude-List

**Sources** (same precedence rule as the allow-list):

1. `EXCLUDED_REPOS` environment variable — comma-separated glob patterns. Takes
   precedence over the file-based option below if both are set.
2. `excluded_repositories` — a **top-level array key** in the app-level bootstrap file
   (`CONFIG_DIR/release-regent.toml`), sitting **outside** the `ReleaseRegentConfig`
   schema in exactly the same way as `allowed_repositories`: not nested under any
   section, never merged across configuration levels, read once at server startup.

The exclude-list exists because a broad allow-list glob (e.g. `myorg/*`) cannot by
itself express "except this one repository." A pattern in the exclude-list removes a
repository from processing even if it also matches an allow-list pattern.

**Pattern syntax**: identical to the allow-list — glob-style, matched against the
full `"owner/repo"` string.

**Default when omitted or explicitly empty**: **no exclusions** — behavior identical
to today (before this setting existed). Unlike the allow-list, an empty exclude-list
is *not* a special kill-switch case; omitted and explicit `[]` mean exactly the same
thing for the exclude-list.

### Evaluation Order — Exclude Always Wins

An event is processed if and only if it matches the allow-list **and** does not match
the exclude-list:

```text
is_allowed = matches_any(allowed_repositories) && !matches_any(excluded_repositories)
```

There is no "most specific pattern wins" logic. A match on the exclude-list
unconditionally overrides any match on the allow-list, regardless of how specific or
broad either pattern is (e.g. an exact-name entry in `excluded_repositories` always
wins over a broader glob in `allowed_repositories`, and vice versa — specificity is
irrelevant; only exclude-list membership matters).

### Behavior Change / Migration Note

> Matching for both the allow-list and the exclude-list is **case-insensitive** —
> configured patterns and the incoming `owner/repo` are lowercased before matching.
> Prior to this change, `ALLOWED_REPOS` entries were matched with exact, case-sensitive
> string comparison (with the single literal token `"*"` special-cased to mean "match
> everything"), and no exclude-list existed at all. Operators with existing
> `ALLOWED_REPOS` values that rely on exact case matching are unaffected in practice
> (GitHub enforces case-insensitive uniqueness of `owner/repo`), but should be aware
> the comparison itself has changed.

### Non-Matching / Excluded Repositories

**Non-matching or excluded repositories**: the event is dropped immediately after
signature validation — before configuration loading, version calculation, or any
GitHub API write operation. This is fire-and-forget: the HTTP response already
returned to GitHub reflects only the outcome of signature validation and does **not**
change based on the allow-list/exclude-list decision (i.e., GitHub does not receive a
distinct status code for "repository filtered out"). A `warn!` log records the
repository and event ID.

> **TODO — documentation reconciliation needed**: `docs/user/reference/environment-variables.md`
> currently states that non-matching repositories are "rejected with `403 Forbidden`."
> This spec treats the fire-and-forget behavior above (no HTTP status change) as
> authoritative, based on the current implementation. The two documents currently
> disagree and should be reconciled in a follow-up; this note intentionally does not
> resolve which one is correct.

### Validation

Each allow-list and exclude-list pattern is compiled at startup; a malformed pattern
in either list is a fatal startup error identifying the offending entry (by value and
list).

See [FR-9](../requirements/functional-requirements.md#fr-9-repository-scoping-for-large-organizations)
and [BA-66–BA-75](../testing/behavioral-assertions.md#repository-allow-list-assertions)
for the full functional and behavioral contract.

## Configuration Schema

### Schema Versioning and Migration

**Configuration Schema Version**: `version: "1.0"`

Release Regent uses semantic versioning for configuration schema compatibility:

- **Major version changes**: Breaking changes requiring migration (e.g., `1.x` → `2.x`)
- **Minor version changes**: New optional fields, backward compatible (e.g., `1.0` → `1.1`)
- **Patch version changes**: Bug fixes, clarifications (e.g., `1.0.1` → `1.0.2`)

**Migration Strategy**:

```toml
# Version 1.0 (current)
schema_version = "1.0"

# Version 1.1 (future - new optional fields)
schema_version = "1.1"
# concurrency section added

# Version 2.0 (future - breaking changes)
schema_version = "2.0"
# Could require field renames, structure changes
```

**Backward Compatibility Rules**:

1. **Missing version field**: Assumes `version: "1.0"` with deprecation warning
2. **Older minor versions**: Loads successfully with default values for new fields
3. **Future minor versions**: Loads successfully, ignores unknown fields
4. **Major version mismatch**: Fails with clear migration instructions

### Root Configuration Structure

```toml
schema_version = "1.0"  # Configuration schema version (required)

# Core settings (required for basic operation)
[core]
version_prefix = "v"           # Prefix for version tags and branches

[core.branches]
main = "main"               # Main branch name (required)

# Release PR settings
[release_pr]
title_template = "chore(release): ${version}"
body_template = """
## Release ${version}

${changelog}

### Metadata
- **Commits**: ${commit_count} changes since ${previous_version}
- **Generated**: ${date}
- **Correlation ID**: ${correlation_id}
"""
draft = false
labels = ["release"]
assignees = []

# GitHub release settings
[releases]
draft = false
prerelease = false
generate_notes = false  # Opt in to GitHub-generated notes; default is false so the custom changelog is used
cleanup_branches = true

# Versioning strategy
[versioning]
strategy = "conventional"    # "conventional" | "external"
allow_override = true        # Allow PR comment overrides

[versioning.strategy.external]
command = "./scripts/calculate-version.sh"
timeout_ms = 30000
working_directory = "."

# Error handling
[error_handling]
max_retries = 5
backoff_multiplier = 2
initial_delay_ms = 100
max_delay_ms = 30000
jitter_percent = 0.25

# Notifications
[notifications]
enabled = false
strategy = "none"            # "none" | "github_issue" | "webhook" | "slack"

[notifications.github_issue]
labels = ["release-regent", "bug"]
assignees = []

[notifications.webhook]
url = "https://example.com/webhook"
headers = {}
timeout_ms = 5000

[notifications.slack]
webhook_url = "https://hooks.slack.com/services/XXX/YYY/ZZZ"
channel = "#releases"

# Logging and observability
[logging]
level = "info"              # "debug" | "info" | "warn" | "error"
format = "json"             # "json" | "text"
correlation_ids = true
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

```toml
# Minimal working configuration
[core]
version_prefix = "v"

[core.branches]
main = "main"
```

### Standard Configuration

```toml
# Standard configuration for most repositories
[core]
version_prefix = "v"

[core.branches]
main = "main"

[release_pr]
title_template = "chore(release): ${version}"
body_template = """
## Release ${version}

${changelog}

### Metadata
- **Commits**: ${commit_count} changes since ${previous_version}
- **Generated**: ${date}
"""
draft = false
labels = ["release", "automated"]

[releases]
draft = false
prerelease = false
generate_notes = false  # Opt in to GitHub-generated notes; default is false so the custom changelog is used
cleanup_branches = true

[versioning]
strategy = "conventional"
allow_override = true
```

### Advanced Configuration

```toml
# Advanced configuration with external versioning and notifications
[core]
version_prefix = "v"

[core.branches]
main = "develop"

[release_pr]
title_template = "[RELEASE] ${version} - ${commit_count} changes"
body_template = """
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
"""
draft = false
labels = ["release", "automated", "v${version}"]
assignees = ["@release-team"]

[releases]
draft = false
prerelease = false
generate_notes = false  # Use our custom changelog instead
cleanup_branches = true

[versioning]
allow_override = true
fallback_strategy = "patch"

[versioning.strategy.external]
command = "./scripts/calculate-version.py"
timeout_ms = 15000
working_directory = "."

[error_handling]
max_retries = 3
backoff_multiplier = 1.5
initial_delay_ms = 200
max_delay_ms = 10000

[notifications]
enabled = true
strategy = "slack"

[notifications.slack]
webhook_url = "${SLACK_WEBHOOK_URL}"  # From environment
channel = "#releases"

[logging]
level = "info"
format = "json"
correlation_ids = true
```

### Repository-Specific Override Examples

**Disable notifications for a specific repository**:

```toml
# .release-regent.toml
[notifications]
strategy = "none"
```

**Use external versioning for Rust crates**:

```toml
# .release-regent.toml
[versioning.strategy.external]
command = "./scripts/cargo-version.sh"
timeout_ms = 10000
```

**Custom templates for documentation repositories**:

```toml
# .release-regent.toml
[release_pr]
title_template = "docs(release): ${version} - Update documentation"
body_template = """
## Documentation Release ${version}

${changelog}

This release updates the documentation with the following changes.
"""
```

## Environment Variable Support

Configuration values can reference environment variables using `${VAR_NAME}` syntax:

```toml
[notifications.slack]
webhook_url = "${SLACK_WEBHOOK_URL}"

[versioning.strategy.external]
command = "${VERSION_SCRIPT_PATH}/calculate-version.sh"
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
        repo_path.join(".release-regent.toml"),
        repo_path.join(".github/release-regent.toml"),
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

Configuration file: /path/to/.release-regent.toml
```

This comprehensive configuration reference addresses all the validation and template concerns raised in the spec feedback while providing practical examples and clear error handling guidance.
