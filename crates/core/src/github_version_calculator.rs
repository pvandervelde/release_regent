//! GitHub API-backed version calculator.
//!
//! Implements [`VersionCalculatorTrait`] using the GitHub API to fetch commit
//! history instead of spawning a local `git` subprocess.  This is the correct
//! implementation for server deployments where no local git clone exists.
//!
//! [`VersionCalculatorTrait`]: crate::traits::version_calculator::VersionCalculator

use crate::{
    traits::{
        git_operations::GetCommitsOptions,
        github_operations::GitHubOperations,
        version_calculator::{
            CalculationOptions, ChangelogEntry, CommitAnalysis, ValidationRules,
            VersionBump as TraitVersionBump, VersionCalculationResult,
            VersionCalculator as VersionCalculatorTrait, VersionContext, VersioningStrategy,
        },
    },
    versioning::{SemanticVersion, VersionCalculator as ConventionalCalculator},
    CoreError, CoreResult,
};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

/// Version calculator that fetches commits via the GitHub API.
///
/// Holds an unscoped GitHub API client (`G`).  On each `calculate_version`
/// call the client is scoped to `context.installation_id` before any API
/// requests are made, so the calculator can safely be constructed once at
/// server startup and reused across many webhook events.
///
/// # When to use
///
/// Use `GitHubVersionCalculator` whenever the server runs without a local
/// git clone.  For the CLI, which always runs inside a git working tree,
/// [`DefaultVersionCalculator`] remains the appropriate choice.
///
/// [`DefaultVersionCalculator`]: crate::DefaultVersionCalculator
#[derive(Debug, Clone)]
pub struct GitHubVersionCalculator<G: GitHubOperations> {
    github_operations: G,
}

impl<G: GitHubOperations> GitHubVersionCalculator<G> {
    /// Create a new calculator backed by the given (unscoped) GitHub API client.
    ///
    /// The client will be scoped per calculation using `context.installation_id`.
    #[must_use]
    pub fn new(github_operations: G) -> Self {
        Self { github_operations }
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

    /// Convert a parsed [`crate::versioning::ConventionalCommit`] to a [`CommitAnalysis`].
    ///
    /// The `date` and `author` parameters are threaded in from the originating
    /// [`crate::traits::git_operations::GitCommit`] because [`ConventionalCommit`]
    /// only carries the SHA and message text — the full commit metadata is
    /// discarded by the parser.
    fn to_commit_analysis(
        commit: crate::versioning::ConventionalCommit,
        date: chrono::DateTime<Utc>,
        author: String,
    ) -> CommitAnalysis {
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
            author,
            commit_type: Some(commit.commit_type),
            date,
            is_breaking: commit.breaking_change,
            message: commit.message,
            metadata: HashMap::new(),
            scope: commit.scope,
            sha: commit.sha,
            version_bump,
        }
    }

    /// Build a [`VersionCalculationResult`] from analyses and the computed version.
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

    /// Apply a version bump to a semantic version, returning the bumped version.
    ///
    /// When `major == 0` (pre-1.0 development), a `Major` bump is treated as a
    /// `Minor` bump. Semver 2.0 allows breaking changes within major version 0 to
    /// only advance the minor component so that projects stay on `0.x` until they
    /// deliberately ship their first stable `1.0.0` release.
    fn bump_version(
        current: SemanticVersion,
        bump: &TraitVersionBump,
        prerelease: Option<String>,
        build: Option<String>,
    ) -> CoreResult<SemanticVersion> {
        let mut next = match bump {
            TraitVersionBump::Major if current.major == 0 => SemanticVersion {
                major: 0,
                minor: current.minor + 1,
                patch: 0,
                prerelease: None,
                build: None,
            },
            TraitVersionBump::Major => SemanticVersion {
                major: current.major + 1,
                minor: 0,
                patch: 0,
                prerelease: None,
                build: None,
            },
            TraitVersionBump::Minor => SemanticVersion {
                major: current.major,
                minor: current.minor + 1,
                patch: 0,
                prerelease: None,
                build: None,
            },
            TraitVersionBump::Patch => SemanticVersion {
                major: current.major,
                minor: current.minor,
                patch: current.patch + 1,
                prerelease: None,
                build: None,
            },
            TraitVersionBump::None => current,
        };
        next.prerelease = prerelease;
        next.build = build;
        Ok(next)
    }

    /// Map from core `versioning::VersionBump` to the trait-layer `VersionBump`.
    ///
    /// This conversion helper is not yet called by any production code path — the
    /// trait's `VersionBump` is currently derived directly from
    /// `ConventionalCommit.breaking_change` / `commit_type` in
    /// [`to_commit_analysis`].  It is retained here because future refactors may
    /// need to convert an already-computed `versioning::VersionBump` (e.g., when
    /// integrating with `DefaultVersionCalculator` output) into the trait type.
    #[allow(dead_code)]
    fn local_to_trait_bump(bump: &crate::versioning::VersionBump) -> TraitVersionBump {
        match bump {
            crate::versioning::VersionBump::Major => TraitVersionBump::Major,
            crate::versioning::VersionBump::Minor => TraitVersionBump::Minor,
            crate::versioning::VersionBump::Patch => TraitVersionBump::Patch,
            crate::versioning::VersionBump::None => TraitVersionBump::None,
        }
    }
}

#[async_trait]
impl<G: GitHubOperations + 'static> VersionCalculatorTrait for GitHubVersionCalculator<G> {
    /// Calculate the next version by fetching commits from the GitHub API and
    /// applying conventional-commit rules.
    ///
    /// The underlying GitHub client must already be scoped to the correct
    /// installation (i.e. call `scoped_to(installation_id)` first).
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
            "Calculating version via GitHub API",
        );

        let raw_commits: Vec<(String, String)>;
        let mut sha_to_meta: HashMap<String, (chrono::DateTime<Utc>, String)> = HashMap::new();

        if let Some(ref base) = context.base_ref {
            let commits = self
                .github_operations
                .get_commits_between(
                    &context.owner,
                    &context.repo,
                    base,
                    &context.head_ref,
                    GetCommitsOptions::default(),
                )
                .await?;
            debug!(
                commit_count = commits.len(),
                "Fetched commits between refs via GitHub API"
            );
            // Build a lookup table keyed by SHA so that to_commit_analysis can
            // populate the date and author fields from the original GitCommit
            // rather than falling back to Utc::now() / empty string.
            for c in &commits {
                sha_to_meta.insert(c.sha.clone(), (c.author_date, c.author.name.clone()));
            }
            raw_commits = commits.into_iter().map(|c| (c.sha, c.subject)).collect();
        } else {
            // No base ref — first release, no prior tag to compare against.
            debug!("No base_ref; skipping commit analysis for first release");
            raw_commits = Vec::new();
        };

        let conventional = ConventionalCalculator::parse_conventional_commits(&raw_commits);
        let analyses: Vec<CommitAnalysis> = conventional
            .into_iter()
            .map(|c| {
                let (date, author) = sha_to_meta
                    .remove(&c.sha)
                    .unwrap_or_else(|| (Utc::now(), String::new()));
                Self::to_commit_analysis(c, date, author)
            })
            .collect();

        let bump = Self::highest_bump(&analyses);

        let current = context.current_version.clone().unwrap_or(SemanticVersion {
            major: 0,
            minor: 1,
            patch: 0,
            prerelease: None,
            build: None,
        });

        let next_version = Self::bump_version(current, &bump, None, None)?;

        Ok(Self::build_result(
            &context,
            strategy,
            analyses,
            next_version,
            bump,
        ))
    }

    /// Analyse individual commits identified by their SHAs using the GitHub API.
    async fn analyze_commits(
        &self,
        context: VersionContext,
        _strategy: VersioningStrategy,
        commit_shas: Vec<String>,
    ) -> CoreResult<Vec<CommitAnalysis>> {
        let mut analyses = Vec::with_capacity(commit_shas.len());

        for sha in &commit_shas {
            let commit = self
                .github_operations
                .get_commit(&context.owner, &context.repo, sha)
                .await
                .map_err(|e| CoreError::versioning(format!("Failed to fetch commit {sha}: {e}")))?;

            let date = commit.author_date;
            let author = commit.author.name.clone();
            let raw = vec![(commit.sha.clone(), commit.subject.clone())];
            let parsed = ConventionalCalculator::parse_conventional_commits(&raw);
            for c in parsed {
                analyses.push(Self::to_commit_analysis(c, date, author.clone()));
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
    fn parse_conventional_commit(
        &self,
        commit_message: &str,
    ) -> CoreResult<Option<CommitAnalysis>> {
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
        Ok(parsed
            .into_iter()
            .next()
            .map(|c| Self::to_commit_analysis(c, Utc::now(), String::new())))
    }

    /// Apply a version bump to an existing version.
    fn apply_version_bump(
        &self,
        current_version: SemanticVersion,
        bump_type: TraitVersionBump,
        prerelease: Option<String>,
        build: Option<String>,
    ) -> CoreResult<SemanticVersion> {
        Self::bump_version(current_version, &bump_type, prerelease, build)
    }

    /// Return a copy of this calculator with the GitHub client scoped to the
    /// given installation ID.
    ///
    /// This is the correct way to supply authentication context to
    /// `GitHubVersionCalculator` — scope it before calling `calculate_version`
    /// rather than embedding the installation ID in `VersionContext`.
    fn scoped_to(&self, installation_id: u64) -> Arc<dyn VersionCalculatorTrait + Send + Sync> {
        Arc::new(Self {
            github_operations: self.github_operations.scoped_to(installation_id),
        })
    }
}
