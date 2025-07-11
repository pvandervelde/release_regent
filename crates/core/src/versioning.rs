//! Version calculation and management for Release Regent
//!
//! This module handles semantic version calculation using multiple strategies.

use crate::{CoreError, CoreResult};
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
                    (Some(a), Some(b)) => a.cmp(b),    // compare pre-release strings
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

#[cfg(test)]
#[path = "versioning_tests.rs"]
mod tests;
