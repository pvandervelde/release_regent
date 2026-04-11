//! Default version calculator implementation.
//!
//! Implements the [`VersionCalculatorTrait`] using local git log commands, enabling
//! callers to calculate semantic versions from conventional commit history without
//! requiring a GitHub API token.
//!
//! # Design
//!
//! The [`DefaultVersionCalculator`] bridges the concrete [`ConventionalCalculator`]
//! engine with the generic [`VersionCalculatorTrait`] interface consumed by
//! [`ReleaseRegentProcessor`].  It translates between the two `VersionBump` enums
//! and handles local git subprocess calls.
//!
//! [`VersionCalculatorTrait`]: crate::traits::version_calculator::VersionCalculator
//! [`ConventionalCalculator`]: crate::versioning::VersionCalculator
//! [`ReleaseRegentProcessor`]: crate::ReleaseRegentProcessor

use crate::{
    traits::version_calculator::{
        CalculationOptions, ChangelogEntry, CommitAnalysis, ValidationRules,
        VersionBump as TraitVersionBump, VersionCalculationResult,
        VersionCalculator as VersionCalculatorTrait, VersionContext, VersioningStrategy,
    },
    versioning::{
        ConventionalCommit, SemanticVersion, VersionBump as LocalVersionBump,
        VersionCalculator as ConventionalCalculator,
    },
    CoreError, CoreResult,
};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use tracing::debug;

#[cfg(test)]
#[path = "default_version_calculator_tests.rs"]
mod tests;

/// Production version calculator using local git log.
///
/// Fetches commit history by spawning `git log` subprocesses and delegates
/// parsing and version arithmetic to the core [`ConventionalCalculator`].
///
/// Used by the CLI and server when a concrete implementation of
/// [`VersionCalculatorTrait`] is required to build a [`ReleaseRegentProcessor`]
/// with production dependencies.
///
/// # Local git requirement
///
/// All methods that analyse commit history require `git` to be available on
/// `$PATH` and the working directory to be inside a git repository.
#[derive(Debug, Default)]
pub struct DefaultVersionCalculator;

/// Private helpers used within the trait implementation and tests.
#[allow(dead_code)] // methods are exercised from the test module
impl DefaultVersionCalculator {
    /// Create a new default version calculator.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Map from core `versioning::VersionBump` to the trait-layer `VersionBump`.
    fn local_to_trait_bump(bump: LocalVersionBump) -> TraitVersionBump {
        match bump {
            LocalVersionBump::Major => TraitVersionBump::Major,
            LocalVersionBump::Minor => TraitVersionBump::Minor,
            LocalVersionBump::Patch => TraitVersionBump::Patch,
            LocalVersionBump::None => TraitVersionBump::None,
        }
    }

    /// Map from the trait-layer `VersionBump` to core `versioning::VersionBump`.
    fn trait_to_local_bump(bump: &TraitVersionBump) -> LocalVersionBump {
        match bump {
            TraitVersionBump::Major => LocalVersionBump::Major,
            TraitVersionBump::Minor => LocalVersionBump::Minor,
            TraitVersionBump::Patch => LocalVersionBump::Patch,
            TraitVersionBump::None => LocalVersionBump::None,
        }
    }

    /// Fetch commit history from local git between two refs.
    ///
    /// Returns `(sha, subject)` pairs for every commit in `base..head` (or
    /// the latest 100 commits when `base_ref` is `None`).
    async fn fetch_git_commits(
        base_ref: Option<&str>,
        head_ref: &str,
    ) -> CoreResult<Vec<(String, String)>> {
        use std::process::Command;

        let mut cmd = Command::new("git");
        cmd.arg("log").arg("--format=%H %s");

        match base_ref {
            Some(base) => {
                cmd.arg(format!("{}..{}", base, head_ref));
            }
            None => {
                cmd.arg(head_ref).arg("-n").arg("100");
            }
        }

        let output = cmd
            .output()
            .map_err(|e| CoreError::versioning(format!("Failed to execute git log: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CoreError::versioning(format!("git log failed: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let commits: Vec<(String, String)> = stdout
            .lines()
            .filter_map(|line| {
                let mut iter = line.splitn(2, ' ');
                let sha = iter.next()?.to_string();
                let subject = iter.next()?.to_string();
                Some((sha, subject))
            })
            .collect();

        debug!(
            commit_count = commits.len(),
            "Fetched commits from local git log"
        );
        Ok(commits)
    }

    /// Convert a parsed [`ConventionalCommit`] into a trait-layer [`CommitAnalysis`].
    fn to_commit_analysis(commit: ConventionalCommit) -> CommitAnalysis {
        let version_bump = if commit.breaking_change {
            TraitVersionBump::Major
        } else if commit.commit_type == "feat" {
            TraitVersionBump::Minor
        } else if commit.commit_type == "fix" {
            TraitVersionBump::Patch
        } else {
            TraitVersionBump::None
        };

        CommitAnalysis {
            author: String::new(),
            commit_type: Some(commit.commit_type),
            date: Utc::now(),
            is_breaking: commit.breaking_change,
            message: commit.message,
            metadata: HashMap::new(),
            scope: commit.scope,
            sha: commit.sha,
            version_bump,
        }
    }

    /// Derive the highest `TraitVersionBump` from a slice of analyses.
    fn highest_bump(analyses: &[CommitAnalysis]) -> TraitVersionBump {
        let mut result = TraitVersionBump::None;
        for analysis in analyses {
            match analysis.version_bump {
                TraitVersionBump::Major => return TraitVersionBump::Major,
                TraitVersionBump::Minor if result != TraitVersionBump::Minor => {
                    result = TraitVersionBump::Minor;
                }
                TraitVersionBump::Patch if result == TraitVersionBump::None => {
                    result = TraitVersionBump::Patch;
                }
                _ => {}
            }
        }
        result
    }

    /// Build a `VersionCalculationResult` from analyses and the next version.
    fn build_result(
        context: &VersionContext,
        strategy: VersioningStrategy,
        analyses: Vec<CommitAnalysis>,
        next_version: SemanticVersion,
        bump: TraitVersionBump,
    ) -> VersionCalculationResult {
        let changelog_entries: Vec<ChangelogEntry> = analyses
            .iter()
            .filter(|a| a.version_bump != TraitVersionBump::None || a.is_breaking)
            .map(|a| ChangelogEntry {
                commit_sha: a.sha.clone(),
                description: a.message.clone(),
                entry_type: a.commit_type.clone().unwrap_or_else(|| "chore".to_string()),
                is_breaking: a.is_breaking,
                issues: Vec::new(),
                pr_number: None,
                scope: a.scope.clone(),
            })
            .collect();

        VersionCalculationResult {
            analyzed_commits: analyses,
            build_metadata: None,
            changelog_entries,
            current_version: context.current_version.clone(),
            is_prerelease: next_version.is_prerelease(),
            metadata: HashMap::new(),
            next_version,
            strategy,
            version_bump: bump,
        }
    }
}

#[async_trait]
impl VersionCalculatorTrait for DefaultVersionCalculator {
    /// Calculate the next version by running `git log` and applying conventional-commit rules.
    async fn calculate_version(
        &self,
        context: VersionContext,
        strategy: VersioningStrategy,
        _options: CalculationOptions,
    ) -> CoreResult<VersionCalculationResult> {
        debug!(
            owner = %context.owner,
            repo = %context.repo,
            base_ref = ?context.base_ref,
            head_ref = %context.head_ref,
            "Calculating version",
        );

        let raw_commits =
            Self::fetch_git_commits(context.base_ref.as_deref(), &context.head_ref).await?;

        let conventional = ConventionalCalculator::parse_conventional_commits(&raw_commits);

        let analyses: Vec<CommitAnalysis> = conventional
            .into_iter()
            .map(Self::to_commit_analysis)
            .collect();

        let bump = Self::highest_bump(&analyses);

        let current = context.current_version.clone().unwrap_or(SemanticVersion {
            major: 0,
            minor: 1,
            patch: 0,
            prerelease: None,
            build: None,
        });

        let next_version = self.apply_version_bump(current, bump.clone(), None, None)?;

        Ok(Self::build_result(
            &context,
            strategy,
            analyses,
            next_version,
            bump,
        ))
    }

    /// Analyse individual commits identified by their SHAs.
    async fn analyze_commits(
        &self,
        _context: VersionContext,
        _strategy: VersioningStrategy,
        commit_shas: Vec<String>,
    ) -> CoreResult<Vec<CommitAnalysis>> {
        use std::process::Command;

        let mut analyses = Vec::with_capacity(commit_shas.len());

        for sha in &commit_shas {
            let output = Command::new("git")
                .arg("log")
                .arg("-1")
                .arg("--format=%H %s")
                .arg(sha)
                .output()
                .map_err(|e| {
                    CoreError::versioning(format!("Failed to run git log for {sha}: {e}"))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CoreError::versioning(format!(
                    "git log failed for {sha}: {stderr}"
                )));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let line = stdout.trim();
            if let Some((commit_sha, subject)) = line.split_once(' ') {
                let raw = vec![(commit_sha.to_string(), subject.to_string())];
                let parsed = ConventionalCalculator::parse_conventional_commits(&raw);
                for c in parsed {
                    analyses.push(Self::to_commit_analysis(c));
                }
            }
        }

        Ok(analyses)
    }

    /// Validate a proposed version.  Always returns `true` for this implementation.
    async fn validate_version(
        &self,
        _context: VersionContext,
        _proposed_version: SemanticVersion,
        _rules: ValidationRules,
    ) -> CoreResult<bool> {
        // Semantic validation is enforced at parse time by SemanticVersion.
        // The local git calculator accepts any parsed version as valid.
        Ok(true)
    }

    /// Return the highest version bump implied by the provided analyses.
    async fn get_version_bump(
        &self,
        _context: VersionContext,
        _strategy: VersioningStrategy,
        commit_analyses: Vec<CommitAnalysis>,
    ) -> CoreResult<TraitVersionBump> {
        Ok(Self::highest_bump(&commit_analyses))
    }

    /// Generate changelog entries from commit analyses.
    async fn generate_changelog_entries(
        &self,
        _context: VersionContext,
        _strategy: VersioningStrategy,
        commit_analyses: Vec<CommitAnalysis>,
        _version: SemanticVersion,
    ) -> CoreResult<Vec<ChangelogEntry>> {
        let entries = commit_analyses
            .into_iter()
            .filter(|a| a.version_bump != TraitVersionBump::None || a.is_breaking)
            .map(|a| ChangelogEntry {
                commit_sha: a.sha.clone(),
                description: a.message.clone(),
                entry_type: a.commit_type.clone().unwrap_or_else(|| "chore".to_string()),
                is_breaking: a.is_breaking,
                issues: Vec::new(),
                pr_number: None,
                scope: a.scope.clone(),
            })
            .collect();
        Ok(entries)
    }

    /// Perform a dry-run calculation — delegates to `calculate_version`.
    async fn preview_calculation(
        &self,
        context: VersionContext,
        strategy: VersioningStrategy,
        options: CalculationOptions,
    ) -> CoreResult<VersionCalculationResult> {
        // Dry-run has no side effects in this implementation.
        self.calculate_version(context, strategy, options).await
    }

    /// Return the set of versioning strategies this calculator supports.
    fn supported_strategies(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert(
            "conventional_commits".to_string(),
            "Semantic versioning derived from conventional commits".to_string(),
        );
        map
    }

    /// Return the default versioning strategy.
    fn default_strategy(&self) -> VersioningStrategy {
        VersioningStrategy::ConventionalCommits {
            custom_types: HashMap::new(),
            include_prerelease: false,
        }
    }

    /// Parse a single commit message into a [`CommitAnalysis`].
    ///
    /// Returns `None` when the message does not follow the conventional commit
    /// specification (i.e. it lacks a recognised `<type>:` prefix).
    fn parse_conventional_commit(
        &self,
        commit_message: &str,
    ) -> CoreResult<Option<CommitAnalysis>> {
        // Lightweight check: does the first line begin with a conventional type?
        let known_types = [
            "feat", "fix", "chore", "docs", "style", "refactor", "perf", "test", "build", "ci",
            "revert",
        ];
        let first_line = commit_message.lines().next().unwrap_or("");
        let is_conventional = known_types.iter().any(|t| {
            first_line.starts_with(&format!("{t}("))
                || first_line.starts_with(&format!("{t}!"))
                || first_line.starts_with(&format!("{t}: "))
        });

        if !is_conventional {
            return Ok(None);
        }

        let raw = vec![("unknown".to_string(), commit_message.to_string())];
        let parsed = ConventionalCalculator::parse_conventional_commits(&raw);
        Ok(parsed.into_iter().next().map(Self::to_commit_analysis))
    }

    /// Apply a version bump to an existing version.
    fn apply_version_bump(
        &self,
        current_version: SemanticVersion,
        bump_type: TraitVersionBump,
        prerelease: Option<String>,
        build: Option<String>,
    ) -> CoreResult<SemanticVersion> {
        let mut next = match bump_type {
            TraitVersionBump::Major => SemanticVersion {
                major: current_version.major + 1,
                minor: 0,
                patch: 0,
                prerelease: None,
                build: None,
            },
            TraitVersionBump::Minor => SemanticVersion {
                major: current_version.major,
                minor: current_version.minor + 1,
                patch: 0,
                prerelease: None,
                build: None,
            },
            TraitVersionBump::Patch => SemanticVersion {
                major: current_version.major,
                minor: current_version.minor,
                patch: current_version.patch + 1,
                prerelease: None,
                build: None,
            },
            TraitVersionBump::None => current_version,
        };
        next.prerelease = prerelease;
        next.build = build;
        Ok(next)
    }
}
