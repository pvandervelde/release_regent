//! # Version Calculation and Management for Release Regent
//!
//! This module provides comprehensive semantic version calculation and management capabilities
//! following the Semantic Versioning 2.0.0 specification. It supports conventional commit
//! parsing, version bumping, and semantic version validation.
//!
//! ## Architecture
//!
//! The versioning module is built around several key components:
//!
//! - **SemanticVersion**: Core version representation with full semver support
//! - **VersionCalculator**: Engine for calculating next versions from commit history
//! - **ConventionalCommit**: Structured representation of conventional commit messages
//! - **VersionBump**: Type-safe version increment operations
//!
//! ## Usage Examples
//!
//! ### Basic Version Calculation
//!
//! ```rust
//! use release_regent_core::versioning::{VersionCalculator, ConventionalCommit, SemanticVersion};
//!
//! // Start with an existing version
//! let current = SemanticVersion {
//!     major: 1, minor: 0, patch: 0,
//!     prerelease: None, build: None
//! };
//!
//! let calculator = VersionCalculator::new(Some(current));
//!
//! // Parse commits since last release
//! let commits = vec![
//!     ConventionalCommit {
//!         commit_type: "feat".to_string(),
//!         scope: Some("auth".to_string()),
//!         description: "add OAuth support".to_string(),
//!         breaking_change: false,
//!         message: "feat(auth): add OAuth support".to_string(),
//!         sha: "abc123".to_string(),
//!     }
//! ];
//!
//! let next_version = calculator.calculate_next_version(&commits)?;
//! assert_eq!(next_version.to_string(), "1.1.0"); // Minor bump for new feature
//! # Ok::<(), release_regent_core::CoreError>(())
//! ```
//!
//! ### Parsing Conventional Commits
//!
//! ```rust
//! use release_regent_core::versioning::VersionCalculator;
//!
//! let commit_data = vec![
//!     ("abc123".to_string(), "feat: add user authentication".to_string()),
//!     ("def456".to_string(), "fix(ui): resolve button alignment".to_string()),
//!     ("ghi789".to_string(), "feat!: remove deprecated API".to_string()),
//! ];
//!
//! let parsed_commits = VersionCalculator::parse_conventional_commits(&commit_data);
//!
//! assert_eq!(parsed_commits[0].commit_type, "feat");
//! assert!(!parsed_commits[0].breaking_change);
//!
//! assert_eq!(parsed_commits[1].commit_type, "fix");
//! assert_eq!(parsed_commits[1].scope, Some("ui".to_string()));
//!
//! assert_eq!(parsed_commits[2].commit_type, "feat");
//! assert!(parsed_commits[2].breaking_change); // Breaking change from '!'
//! ```
//!
//! ### Semantic Version Parsing
//!
//! ```rust
//! use release_regent_core::versioning::VersionCalculator;
//!
//! // Parse various semver formats
//! let basic = VersionCalculator::parse_version("1.2.3")?;
//! let with_prefix = VersionCalculator::parse_version("v2.0.0")?;
//! let prerelease = VersionCalculator::parse_version("1.0.0-alpha.1")?;
//! let with_build = VersionCalculator::parse_version("1.0.0+20210101.abcd123")?;
//! let full = VersionCalculator::parse_version("v2.0.0-beta.2+build.123")?;
//!
//! assert_eq!(basic.major, 1);
//! assert!(prerelease.is_prerelease());
//! assert!(with_build.has_build_metadata());
//! # Ok::<(), release_regent_core::CoreError>(())
//! ```
//!
//! ## Conventional Commit Support
//!
//! The module supports the full [Conventional Commits](https://www.conventionalcommits.org/) specification:
//!
//! - **Standard types**: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`
//! - **Scopes**: Optional component scopes like `feat(auth): ...`
//! - **Breaking changes**: Both `!` syntax and `BREAKING CHANGE:` footer
//! - **Fallback parsing**: Non-conventional commits are treated as `chore` type
//!
//! ### Version Bump Rules
//!
//! | Commit Type | Breaking Change | Version Bump |
//! |------------|----------------|--------------|
//! | `feat` | No | Minor |
//! | `fix` | No | Patch |
//! | Any | Yes | Major |
//! | Other types | No | None |
//!
//! ## Semantic Versioning Compliance
//!
//! This implementation strictly follows [Semantic Versioning 2.0.0](https://semver.org/):
//!
//! - **Core version**: `MAJOR.MINOR.PATCH` format
//! - **Pre-release**: Optional `-alpha.1`, `-beta.2`, etc.
//! - **Build metadata**: Optional `+build.123`, `+20210101.abcd`
//! - **Version precedence**: Correct ordering with pre-release handling
//! - **Validation**: Strict parsing with detailed error messages
//!
//! ## Error Handling
//!
//! All operations return `CoreResult<T>` for comprehensive error handling:
//!
//! ```rust
//! use release_regent_core::versioning::VersionCalculator;
//!
//! // Invalid version formats are caught
//! assert!(VersionCalculator::parse_version("invalid").is_err());
//! assert!(VersionCalculator::parse_version("01.2.3").is_err()); // Leading zeros
//! assert!(VersionCalculator::parse_version("1.2.3-").is_err()); // Empty prerelease
//! ```

use crate::{CoreError, CoreResult};
use crate::traits::git_operations::{GitTag, ListTagsOptions};
use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::{debug, info};

/// Conventional commit information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConventionalCommit {
    /// Commit type (feat, fix, chore, etc.)
    pub commit_type: String,
    /// Scope (optional)
    pub scope: Option<String>,
    /// Commit description
    pub description: String,
    /// Whether this is a breaking change
    pub breaking_change: bool,
    /// Full commit message
    pub message: String,
    /// Commit SHA
    pub sha: String,
}

/// Semantic version representation
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticVersion {
    /// Major version number
    pub major: u64,
    /// Minor version number
    pub minor: u64,
    /// Patch version number
    pub patch: u64,
    /// Pre-release identifier (optional)
    pub prerelease: Option<String>,
    /// Build metadata (optional)
    pub build: Option<String>,
}

impl fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;

        if let Some(prerelease) = &self.prerelease {
            write!(f, "-{}", prerelease)?;
        }

        if let Some(build) = &self.build {
            write!(f, "+{}", build)?;
        }

        Ok(())
    }
}

impl SemanticVersion {
    /// Format the version as a string with optional prefix
    pub fn to_string_with_prefix(&self, include_prefix: bool) -> String {
        let base = self.to_string();
        if include_prefix {
            format!("v{}", base)
        } else {
            base
        }
    }

    /// Check if this version is a pre-release
    pub fn is_prerelease(&self) -> bool {
        self.prerelease.is_some()
    }

    /// Check if this version has build metadata
    pub fn has_build_metadata(&self) -> bool {
        self.build.is_some()
    }

    /// Compare versions ignoring build metadata (as per semver spec)
    pub fn compare_precedence(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        // Compare major.minor.patch first
        match (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)) {
            Ordering::Equal => {
                // If core versions are equal, compare pre-release
                match (&self.prerelease, &other.prerelease) {
                    (None, None) => Ordering::Equal,
                    (Some(_), None) => Ordering::Less, // pre-release < normal
                    (None, Some(_)) => Ordering::Greater, // normal > pre-release
                    (Some(a), Some(b)) => compare_prerelease(a, b),
                }
            }
            other => other,
        }
    }
}

/// Version bump types based on conventional commits
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionBump {
    /// Major version bump (breaking changes)
    Major,
    /// Minor version bump (new features)
    Minor,
    /// Patch version bump (bug fixes)
    Patch,
    /// No version bump needed
    None,
}

/// Version calculation engine
pub struct VersionCalculator {
    current_version: Option<SemanticVersion>,
}

impl VersionCalculator {
    /// Create a new version calculator
    pub fn new(current_version: Option<SemanticVersion>) -> Self {
        Self { current_version }
    }

    /// Calculate the next version based on conventional commits
    ///
    /// # Arguments
    /// * `commits` - List of conventional commits since last release
    pub fn calculate_next_version(
        &self,
        commits: &[ConventionalCommit],
    ) -> CoreResult<SemanticVersion> {
        info!("Calculating next version from {} commits", commits.len());

        let bump = self.determine_version_bump(commits);
        debug!("Determined version bump: {:?}", bump);

        let base_version = self.current_version.clone().unwrap_or_else(|| {
            debug!("No current version found, starting from 0.1.0");
            SemanticVersion {
                major: 0,
                minor: 1,
                patch: 0,
                prerelease: None,
                build: None,
            }
        });

        let next_version = self.apply_version_bump(&base_version, bump);
        info!("Calculated next version: {}", next_version);

        Ok(next_version)
    }

    /// Determine the type of version bump needed
    fn determine_version_bump(&self, commits: &[ConventionalCommit]) -> VersionBump {
        let mut has_breaking = false;
        let mut has_features = false;
        let mut has_fixes = false;

        for commit in commits {
            if commit.breaking_change {
                has_breaking = true;
            } else if commit.commit_type == "feat" {
                has_features = true;
            } else if commit.commit_type == "fix" {
                has_fixes = true;
            }
        }

        if has_breaking {
            VersionBump::Major
        } else if has_features {
            VersionBump::Minor
        } else if has_fixes {
            VersionBump::Patch
        } else {
            VersionBump::None
        }
    }

    /// Apply version bump to base version
    fn apply_version_bump(&self, base: &SemanticVersion, bump: VersionBump) -> SemanticVersion {
        match bump {
            VersionBump::Major => SemanticVersion {
                major: base.major + 1,
                minor: 0,
                patch: 0,
                prerelease: None,
                build: None,
            },
            VersionBump::Minor => SemanticVersion {
                major: base.major,
                minor: base.minor + 1,
                patch: 0,
                prerelease: None,
                build: None,
            },
            VersionBump::Patch => SemanticVersion {
                major: base.major,
                minor: base.minor,
                patch: base.patch + 1,
                prerelease: None,
                build: None,
            },
            VersionBump::None => base.clone(),
        }
    }

    /// Parse a semantic version string
    ///
    /// Supports full semantic versioning specification including:
    /// - Core format: MAJOR.MINOR.PATCH
    /// - Pre-release: 1.0.0-alpha.1, 1.0.0-beta.2
    /// - Build metadata: 1.0.0+20210101.abcd123
    /// - Version prefix: configurable 'v' prefix support
    pub fn parse_version(version_str: &str) -> CoreResult<SemanticVersion> {
        debug!("Parsing version string: {}", version_str);

        if version_str.trim().is_empty() {
            return Err(CoreError::versioning(
                "Version string cannot be empty".to_string(),
            ));
        }

        // Remove optional 'v' prefix
        let clean_version = version_str.trim_start_matches('v');

        // Split on '+' to separate build metadata
        let (version_part, build) = match clean_version.split_once('+') {
            Some((version, build)) => (version, Some(build.to_string())),
            None => (clean_version, None),
        };

        // Split on '-' to separate pre-release
        let (core_version, prerelease) = match version_part.split_once('-') {
            Some((core, prerelease)) => (core, Some(prerelease.to_string())),
            None => (version_part, None),
        };

        // Parse core version components
        let parts: Vec<&str> = core_version.split('.').collect();
        if parts.len() != 3 {
            return Err(CoreError::versioning(format!(
                "Invalid version format: expected MAJOR.MINOR.PATCH, got {}",
                version_str
            )));
        }

        // Validate and parse each component
        let major = Self::parse_version_component(parts[0], "major")?;
        let minor = Self::parse_version_component(parts[1], "minor")?;
        let patch = Self::parse_version_component(parts[2], "patch")?;

        // Validate pre-release format if present
        if let Some(ref pre) = prerelease {
            Self::validate_prerelease(pre)?;
        }

        // Validate build metadata format if present
        if let Some(ref build_meta) = build {
            Self::validate_build_metadata(build_meta)?;
        }

        Ok(SemanticVersion {
            major,
            minor,
            patch,
            prerelease,
            build,
        })
    }

    /// Parse and validate a single version component
    fn parse_version_component(component: &str, component_name: &str) -> CoreResult<u64> {
        if component.is_empty() {
            return Err(CoreError::versioning(format!(
                "{} version component cannot be empty",
                component_name
            )));
        }

        // Check for leading zeros (not allowed except for "0")
        if component.len() > 1 && component.starts_with('0') {
            return Err(CoreError::versioning(format!(
                "{} version component cannot have leading zeros: {}",
                component_name, component
            )));
        }

        component.parse().map_err(|_| {
            CoreError::versioning(format!(
                "Invalid {} version component: {} (must be a non-negative integer)",
                component_name, component
            ))
        })
    }

    /// Validate pre-release version format
    fn validate_prerelease(prerelease: &str) -> CoreResult<()> {
        if prerelease.is_empty() {
            return Err(CoreError::versioning(
                "Pre-release identifier cannot be empty".to_string(),
            ));
        }

        // Pre-release can contain ASCII alphanumeric characters and hyphens
        // Each dot-separated identifier must not be empty
        for identifier in prerelease.split('.') {
            if identifier.is_empty() {
                return Err(CoreError::versioning(
                    "Pre-release identifiers cannot be empty".to_string(),
                ));
            }

            // Check for invalid characters (must be ASCII alphanumeric or hyphen)
            if !identifier
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                return Err(CoreError::versioning(format!(
                    "Invalid pre-release identifier: {} (only ASCII alphanumeric characters and hyphens allowed)",
                    identifier
                )));
            }

            // Numeric identifiers must not have leading zeros
            if identifier.chars().all(|c| c.is_ascii_digit())
                && identifier.len() > 1
                && identifier.starts_with('0')
            {
                return Err(CoreError::versioning(format!(
                    "Numeric pre-release identifier cannot have leading zeros: {}",
                    identifier
                )));
            }
        }

        Ok(())
    }

    /// Validate build metadata format
    fn validate_build_metadata(build: &str) -> CoreResult<()> {
        if build.is_empty() {
            return Err(CoreError::versioning(
                "Build metadata cannot be empty".to_string(),
            ));
        }

        // Build metadata can contain ASCII alphanumeric characters and hyphens
        // Each dot-separated identifier must not be empty
        for identifier in build.split('.') {
            if identifier.is_empty() {
                return Err(CoreError::versioning(
                    "Build metadata identifiers cannot be empty".to_string(),
                ));
            }

            // Check for invalid characters (must be ASCII alphanumeric or hyphen)
            if !identifier
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                return Err(CoreError::versioning(format!(
                    "Invalid build metadata identifier: {} (only ASCII alphanumeric characters and hyphens allowed)",
                    identifier
                )));
            }
        }

        Ok(())
    }

    /// Parse conventional commits from commit messages
    ///
    /// Parses commit messages according to the conventional commits specification
    /// using the git-conventional library for robust parsing.
    pub fn parse_conventional_commits(
        commit_messages: &[(String, String)],
    ) -> Vec<ConventionalCommit> {
        debug!("Parsing {} commit messages", commit_messages.len());

        commit_messages
            .iter()
            .map(|(sha, message)| Self::parse_single_conventional_commit(sha, message))
            .collect()
    }

    /// Parse a single conventional commit message
    fn parse_single_conventional_commit(sha: &str, message: &str) -> ConventionalCommit {
        match git_conventional::Commit::parse(message) {
            Ok(parsed_commit) => {
                let commit_type = parsed_commit.type_().as_str().to_string();
                let scope = parsed_commit.scope().map(|s| s.as_str().to_string());
                let description = parsed_commit.description().to_string();
                let breaking_change = parsed_commit.breaking();

                ConventionalCommit {
                    commit_type,
                    scope,
                    description,
                    breaking_change,
                    message: message.to_string(),
                    sha: sha.to_string(),
                }
            }
            Err(err) => {
                debug!("Failed to parse commit as conventional: {}", err);
                // Fallback for non-conventional commits - treat as chore
                ConventionalCommit {
                    commit_type: "chore".to_string(),
                    scope: None,
                    description: message.lines().next().unwrap_or(message).to_string(),
                    breaking_change: false,
                    message: message.to_string(),
                    sha: sha.to_string(),
                }
            }
        }
    }
}

/// Compare two semver pre-release strings following the semver 2.0 specification (§11.4).
///
/// Each dot-separated identifier is compared pairwise from left to right:
/// - Both identifiers are all-digit: compared numerically (`beta.11 > beta.2`).
/// - Left is numeric, right is alphanumeric: `Less` (spec §11.4.3).
/// - Left is alphanumeric, right is numeric: `Greater`.
/// - Both are alphanumeric: compared lexically in ASCII order.
///
/// When all compared pairs are equal, the version with more identifiers is `Greater`
/// (e.g. `alpha.1 > alpha` per §11.4.4).
fn compare_prerelease(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let mut a_ids = a.split('.');
    let mut b_ids = b.split('.');

    loop {
        match (a_ids.next(), b_ids.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,    // fewer identifiers is less
            (Some(_), None) => return Ordering::Greater, // more identifiers is greater
            (Some(a_id), Some(b_id)) => {
                let ord = match (a_id.parse::<u64>(), b_id.parse::<u64>()) {
                    (Ok(a_num), Ok(b_num)) => a_num.cmp(&b_num),
                    (Ok(_), Err(_)) => Ordering::Less,    // numeric < alphanumeric
                    (Err(_), Ok(_)) => Ordering::Greater, // alphanumeric > numeric
                    (Err(_), Err(_)) => a_id.cmp(b_id),  // both alphanumeric: ASCII order
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

/// Returns the highest semantic-version tag from `tags`, ignoring non-semver names.
///
/// Tags whose names cannot be parsed as a semantic version (with optional `v` prefix)
/// are silently ignored. When `include_prerelease` is `false`, tags with a pre-release
/// component are excluded before computing the maximum.
///
/// Returns `None` when `tags` is empty or no tag name parses as a valid semantic version
/// string (subject to the `include_prerelease` filter).
///
/// Build metadata is ignored when comparing versions (as per semver 2.0 spec).
///
/// # Examples
///
/// ```rust
/// use release_regent_core::traits::git_operations::{GitTag, GitTagType};
/// use release_regent_core::versioning::latest_semver_tag;
///
/// let tags = vec![
///     GitTag { name: "v1.0.0".to_string(), target_sha: "abc".to_string(),
///              tag_type: GitTagType::Lightweight, message: None, tagger: None, created_at: None },
///     GitTag { name: "v2.0.0".to_string(), target_sha: "def".to_string(),
///              tag_type: GitTagType::Lightweight, message: None, tagger: None, created_at: None },
/// ];
///
/// let latest = latest_semver_tag(&tags, false);
/// assert_eq!(latest.unwrap().to_string(), "2.0.0");
/// ```
#[must_use]
pub fn latest_semver_tag(tags: &[GitTag], include_prerelease: bool) -> Option<SemanticVersion> {
    tags.iter()
        // Use `VersionCalculator::parse_version` directly rather than the convenience
        // method `GitTag::parse_semver()`. The latter's `is_semver()` pre-check rejects
        // valid pre-release tags (e.g. `v1.0.0-rc.1`) because it treats the patch
        // component `"0-rc"` as non-numeric. Calling `parse_version` directly preserves
        // full semver 2.0 support including pre-release identifiers.
        .filter_map(|t| VersionCalculator::parse_version(&t.name).ok())
        .filter(|v| include_prerelease || !v.is_prerelease())
        .max_by(SemanticVersion::compare_precedence)
}

/// Determines the current release baseline version for a repository by querying its tags.
///
/// Fetches all Git tags via [`crate::traits::GitOperations::list_tags`], then returns the
/// highest tag whose name parses as a valid semantic version string. By default,
/// pre-release tags (e.g. `v1.0.0-alpha.1`) are excluded from consideration.
/// Pass `include_prerelease = true` to include them.
///
/// Returns `Ok(None)` for repositories that have no tags or no tags parseable as semver.
/// This is a valid, non-error state: version calculation will default to `0.1.0`.
///
/// # Pagination
///
/// Correctness depends on the `G: GitOperations` implementation returning the **complete**
/// tag list. [`ListTagsOptions::default`] passes `limit: None`; if the underlying client
/// treats this as a cap (e.g. 100 tags), the highest semver tag may not be included,
/// producing a stale baseline. Callers on repositories with many tags should ensure
/// their `list_tags` implementation pages through all results.
///
/// # Errors
///
/// Returns `Err` only when the GitHub API or network layer fails inside `list_tags`.
///
/// # Examples
///
/// ```rust,ignore
/// use release_regent_core::versioning::resolve_current_version;
///
/// let version = resolve_current_version(&github, "myorg", "myrepo", false).await?;
/// match version {
///     Some(v) => println!("Latest release: {v}"),
///     None    => println!("No releases yet — starting from 0.1.0"),
/// }
/// ```
pub async fn resolve_current_version<G>(
    github: &G,
    owner: &str,
    repo: &str,
    include_prerelease: bool,
) -> CoreResult<Option<SemanticVersion>>
where
    G: crate::traits::GitOperations,
{
    let tags = github
        .list_tags(owner, repo, ListTagsOptions::default())
        .await?;

    let version = latest_semver_tag(&tags, include_prerelease);

    debug!(
        owner = %owner,
        repo = %repo,
        include_prerelease,
        resolved = ?version.as_ref().map(ToString::to_string),
        "resolved current version from tags"
    );

    Ok(version)
}

#[cfg(test)]
#[path = "versioning_tests.rs"]
mod tests;
