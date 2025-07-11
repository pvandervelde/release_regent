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
    pub fn parse_version(version_str: &str) -> CoreResult<SemanticVersion> {
        debug!("Parsing version string: {}", version_str);

        // TODO: Implement full semantic version parsing
        // This will be implemented in subsequent issues

        // Simple placeholder implementation
        let parts: Vec<&str> = version_str.trim_start_matches('v').split('.').collect();
        if parts.len() != 3 {
            return Err(CoreError::versioning(format!(
                "Invalid version format: {}",
                version_str
            )));
        }

        let major = parts[0]
            .parse()
            .map_err(|_| CoreError::versioning(format!("Invalid major version: {}", parts[0])))?;

        let minor = parts[1]
            .parse()
            .map_err(|_| CoreError::versioning(format!("Invalid minor version: {}", parts[1])))?;

        let patch = parts[2]
            .parse()
            .map_err(|_| CoreError::versioning(format!("Invalid patch version: {}", parts[2])))?;

        Ok(SemanticVersion {
            major,
            minor,
            patch,
            prerelease: None,
            build: None,
        })
    }

    /// Parse conventional commits from commit messages
    ///
    /// Parses commit messages according to the conventional commits specification:
    /// <type>[optional scope]: <description>
    ///
    /// [optional body]
    ///
    /// [optional footer(s)]
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
        let lines: Vec<&str> = message.lines().collect();
        if lines.is_empty() {
            return ConventionalCommit {
                commit_type: "chore".to_string(),
                scope: None,
                description: message.to_string(),
                breaking_change: false,
                message: message.to_string(),
                sha: sha.to_string(),
            };
        }

        let header = lines[0];
        let (commit_type, scope, description, breaking_from_header) = Self::parse_header(header);

        // Check for breaking changes in body/footer
        let breaking_from_body = Self::has_breaking_change_in_body(message);
        let breaking_change = breaking_from_header || breaking_from_body;

        ConventionalCommit {
            commit_type,
            scope,
            description,
            breaking_change,
            message: message.to_string(),
            sha: sha.to_string(),
        }
    }

    /// Parse the header line of a conventional commit
    /// Returns (type, scope, description, breaking_change)
    fn parse_header(header: &str) -> (String, Option<String>, String, bool) {
        // Regex pattern for conventional commit header
        // <type>[(scope)][!]: <description>
        let re = regex::Regex::new(r"^([a-z]+)(?:\(([^)]+)\))?(!)?: (.+)$").unwrap();

        if let Some(captures) = re.captures(header) {
            let commit_type = captures
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| "chore".to_string());
            let scope = captures.get(2).map(|m| m.as_str().to_string());
            let breaking_exclamation = captures.get(3).is_some();
            let description = captures
                .get(4)
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| header.to_string());

            (commit_type, scope, description, breaking_exclamation)
        } else {
            // Non-conventional commit - treat as chore
            ("chore".to_string(), None, header.to_string(), false)
        }
    }

    /// Check if the commit body contains breaking change indicators
    fn has_breaking_change_in_body(message: &str) -> bool {
        message.contains("BREAKING CHANGE:") || message.contains("BREAKING-CHANGE:")
    }
}

#[cfg(test)]
#[path = "versioning_tests.rs"]
mod tests;
