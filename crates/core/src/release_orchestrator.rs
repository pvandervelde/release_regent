//! Release PR orchestration for Release Regent
//!
//! This module implements the [`ReleaseOrchestrator`], which is the heart of the
//! release-management workflow triggered whenever a regular (non-release) pull
//! request is merged into the default branch.
//!
//! ## Responsibilities
//!
//! 1. **Search** for an existing open release PR whose head branch matches the
//!    `release/v*` pattern.
//! 2. **Decide** what action to take based on the relationship between the
//!    existing PR's version (if any) and the newly calculated version:
//!
//! | Existing PR?   | Existing version vs new | Action                    |
//! |---------------|-------------------------|---------------------------|
//! | No             | —                       | Create new branch + PR    |
//! | Yes            | Lower than new          | Rename branch + update PR |
//! | Yes            | Equal to new            | Merge changelogs only     |
//! | Yes            | Higher than new         | No-op (never downgrade)   |
//!
//! 3. **Idempotency**: Every sub-operation is safe to retry without side effects.
//!    If a branch already exists the timestamped fallback is used instead of
//!    failing.
//!
//! ## `ETag` / concurrency note
//!
//! The orchestrator always performs a fresh `get_pull_request` call before any
//! update so that the caller operates on current data.  When task 11.0 adds
//! `If-Match` enforcement to the real GitHub client, `CoreError::Conflict` will
//! signal the caller to retry.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use release_regent_core::release_orchestrator::{ReleaseOrchestrator, OrchestratorConfig};
//! use release_regent_core::versioning::SemanticVersion;
//!
//! let config = OrchestratorConfig::default();
//! let orchestrator = ReleaseOrchestrator::new(config, &github);
//! let result = orchestrator.orchestrate(
//!     "myorg", "myrepo",
//!     &version, &changelog_body,
//!     "main", "abc123def456",
//!     "corr-id-001",
//! ).await?;
//! ```

use crate::{
    traits::github_operations::{CreatePullRequestParams, GitHubOperations, PullRequest},
    versioning::SemanticVersion,
    CoreError, CoreResult,
};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the [`ReleaseOrchestrator`].
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Branch name prefix; combined with the version to form `release/v1.2.3`.
    ///
    /// Defaults to `"release"`.
    pub branch_prefix: String,

    /// Template for the release PR title.
    ///
    /// Supports `{version}` (e.g. `"1.2.3"`) and `{version_tag}` (e.g. `"v1.2.3"`).
    /// Defaults to `"chore(release): {version_tag}"`.
    pub title_template: String,

    /// Sentinel that separates the generated changelog from any trailing content
    /// in the PR body.  The orchestrator will only look for commits between
    /// `## Changelog` and the next `##` heading (or end of string).
    ///
    /// Defaults to `"## Changelog"`.
    pub changelog_header: String,
}

impl OrchestratorConfig {
    /// The default branch prefix used when no explicit configuration is provided.
    pub const DEFAULT_BRANCH_PREFIX: &'static str = "release";
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            branch_prefix: Self::DEFAULT_BRANCH_PREFIX.to_string(),
            title_template: "chore(release): {version_tag}".to_string(),
            changelog_header: "## Changelog".to_string(),
        }
    }
}

/// The outcome of a single [`ReleaseOrchestrator::orchestrate`] call.
#[derive(Debug, Clone)]
pub enum OrchestratorResult {
    /// A new release branch and PR were created.
    Created {
        /// The newly created pull request.
        pr: PullRequest,
        /// The branch name that was created (may be the timestamped fallback).
        branch_name: String,
    },

    /// The body of an existing PR with the same version was updated (changelog
    /// entries were merged and deduplicated).
    Updated {
        /// The updated pull request.
        pr: PullRequest,
    },

    /// An existing PR with a lower version was renamed to the new version and
    /// its body was replaced.
    Renamed {
        /// The updated pull request.
        pr: PullRequest,
    },

    /// The existing PR already has a version higher than the calculated version;
    /// nothing was changed.
    NoOp {
        /// The existing pull request (unchanged).
        pr: PullRequest,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// ReleaseOrchestrator
// ─────────────────────────────────────────────────────────────────────────────

/// Orchestrates release PR creation and updates.
///
/// Generic over `G` so that tests can inject [`MockGitHubOperations`] while
/// production code uses the real [`GitHubClient`].
///
/// [`MockGitHubOperations`]: release_regent_testing::mocks::MockGitHubOperations
/// [`GitHubClient`]: release_regent_github_client::GitHubClient
pub struct ReleaseOrchestrator<'a, G: GitHubOperations> {
    config: OrchestratorConfig,
    github: &'a G,
}

impl<'a, G: GitHubOperations> ReleaseOrchestrator<'a, G> {
    /// Create a new orchestrator.
    ///
    /// # Parameters
    /// - `config`: Orchestration configuration (branch prefix, templates, …)
    /// - `github`: A reference to the `GitHubOperations` implementation to use
    pub fn new(config: OrchestratorConfig, github: &'a G) -> Self {
        Self { config, github }
    }

    // ── Public API ─────────────────────────────────────────────────────────

    /// Run the release orchestration workflow.
    ///
    /// Searches for an existing open release PR, then creates, updates, or
    /// renames it as appropriate for the supplied `version` and `changelog`.
    ///
    /// # Parameters
    /// - `owner`: Repository owner / organisation
    /// - `repo`: Repository name
    /// - `version`: The newly calculated semantic version
    /// - `changelog`: Formatted changelog body (the content inside the
    ///   `## Changelog` section)
    /// - `base_branch`: The base branch for the release PR (usually `main`)
    /// - `base_sha`: The commit SHA to create the release branch from
    /// - `correlation_id`: Tracing correlation ID propagated from the event
    ///
    /// # Errors
    ///
    /// Returns `CoreError::GitHub` when a GitHub API call fails, or
    /// `CoreError::InvalidInput` when a version cannot be parsed from an
    /// existing PR branch name.
    #[allow(clippy::too_many_arguments)] // owner/repo/version/changelog/branch/sha/correlation_id is the minimal release operation surface
    #[tracing::instrument(skip(self, changelog, version), fields(owner, repo, correlation_id, version = %version))]
    pub async fn orchestrate(
        &self,
        owner: &str,
        repo: &str,
        version: &SemanticVersion,
        changelog: &str,
        base_branch: &str,
        base_sha: &str,
        correlation_id: &str,
    ) -> CoreResult<OrchestratorResult> {
        info!(owner, repo, version = %version, correlation_id, "Starting release orchestration");

        let existing = self.search_for_existing_release_pr(owner, repo).await?;

        match existing {
            None => {
                debug!("No existing release PR found; creating new one");
                let (pr, branch_name) = self
                    .create_release_branch_and_pr(
                        owner,
                        repo,
                        version,
                        changelog,
                        base_branch,
                        base_sha,
                    )
                    .await?;
                Ok(OrchestratorResult::Created { pr, branch_name })
            }
            Some((existing_pr, existing_version)) => {
                use std::cmp::Ordering;
                match existing_version.compare_precedence(version) {
                    Ordering::Equal => {
                        debug!(
                            pr_number = existing_pr.number,
                            "Existing PR has same version; merging changelogs"
                        );
                        let pr = self
                            .update_release_pr(owner, repo, &existing_pr, version, changelog)
                            .await?;
                        Ok(OrchestratorResult::Updated { pr })
                    }
                    Ordering::Less => {
                        // existing version < new version → rename & update
                        debug!(
                            pr_number = existing_pr.number,
                            existing = %existing_version,
                            new = %version,
                            "Existing PR has lower version; renaming"
                        );
                        let pr = self
                            .rename_release_pr(
                                owner,
                                repo,
                                &existing_pr,
                                version,
                                changelog,
                                base_sha,
                                base_branch,
                            )
                            .await?;
                        Ok(OrchestratorResult::Renamed { pr })
                    }
                    Ordering::Greater => {
                        // existing version > new version → no-op
                        info!(
                            pr_number = existing_pr.number,
                            existing = %existing_version,
                            new = %version,
                            "Existing PR version is higher; skipping"
                        );
                        Ok(OrchestratorResult::NoOp { pr: existing_pr })
                    }
                }
            }
        }
    }

    // ── Private helpers ────────────────────────────────────────────────────

    /// Search the repository for an open release PR whose head branch starts
    /// with the configured `branch_prefix/v` pattern.
    ///
    /// Returns `None` when no matching PR exists, or the PR together with the
    /// parsed `SemanticVersion` extracted from its branch name.
    async fn search_for_existing_release_pr(
        &self,
        owner: &str,
        repo: &str,
    ) -> CoreResult<Option<(PullRequest, SemanticVersion)>> {
        let query = format!("is:open head:{}*", self.release_branch_prefix());
        let prs = self
            .github
            .search_pull_requests(owner, repo, &query)
            .await?;

        debug!(
            count = prs.len(),
            query, "search_pull_requests returned PRs"
        );

        // Select the highest-versioned match so that, if two release PRs coexist
        // (e.g. after a partial rename failure), we operate on the most recent one
        // rather than whatever GitHub happens to return first.
        let best = prs
            .into_iter()
            .filter_map(|pr| {
                self.parse_version_from_branch(&pr.head.ref_name)
                    .map(|version| (pr, version))
            })
            .max_by(|(_, va), (_, vb)| va.compare_precedence(vb));

        Ok(best)
    }

    /// Create a new release branch and pull request.
    ///
    /// On a `CoreError::Conflict` (branch already exists) the method retries once
    /// with a timestamped fallback branch name.
    async fn create_release_branch_and_pr(
        &self,
        owner: &str,
        repo: &str,
        version: &SemanticVersion,
        changelog: &str,
        base_branch: &str,
        base_sha: &str,
    ) -> CoreResult<(PullRequest, String)> {
        let branch_name = self.make_branch_name(version);

        let actual_branch = match self
            .github
            .create_branch(owner, repo, &branch_name, base_sha)
            .await
        {
            Ok(()) => branch_name,
            Err(CoreError::Conflict { .. }) => {
                let fallback = self.make_fallback_branch_name(version);
                warn!(
                    original = %branch_name,
                    fallback = %fallback,
                    "Branch conflict; retrying with timestamped fallback"
                );
                self.github
                    .create_branch(owner, repo, &fallback, base_sha)
                    .await?;
                fallback
            }
            Err(other) => return Err(other),
        };

        let title = self.render_title(version);
        let body = self.render_body(changelog);

        let pr = self
            .github
            .create_pull_request(
                owner,
                repo,
                CreatePullRequestParams {
                    base: base_branch.to_string(),
                    head: actual_branch.clone(),
                    title,
                    body: Some(body),
                    draft: false,
                    maintainer_can_modify: true,
                },
            )
            .await?;

        info!(pr_number = pr.number, branch = %actual_branch, "Created release PR");
        Ok((pr, actual_branch))
    }

    /// Update an existing release PR by merging new changelog entries into the
    /// existing PR body.
    ///
    /// Performs a fresh `get_pull_request` before the update so we operate on
    /// current data (prep for future `ETag` enforcement).
    async fn update_release_pr(
        &self,
        owner: &str,
        repo: &str,
        existing_pr: &PullRequest,
        version: &SemanticVersion,
        new_changelog: &str,
    ) -> CoreResult<PullRequest> {
        // Always re-fetch to get the latest body (ETag prep).
        let fresh_pr = self
            .github
            .get_pull_request(owner, repo, existing_pr.number)
            .await?;

        let merged_changelog =
            self.merge_changelog_bodies(fresh_pr.body.as_deref().unwrap_or(""), new_changelog);
        let new_body = self.render_body(&merged_changelog);
        let new_title = self.render_title(version);

        // Only send the title when it has actually changed; avoids a spurious
        // PR timeline entry on the equal-version (changelog-only) update path.
        let title_update = if new_title == fresh_pr.title {
            None
        } else {
            Some(new_title)
        };

        let updated = self
            .github
            .update_pull_request(
                owner,
                repo,
                fresh_pr.number,
                title_update,
                Some(new_body),
                None,
            )
            .await?;

        info!(pr_number = updated.number, "Updated release PR changelog");
        Ok(updated)
    }

    /// Rename a lower-version release PR to the new version.
    ///
    /// Strategy:
    /// 1. Create new branch at `base_sha`.
    /// 2. Create new PR pointing to the new branch.
    /// 3. Close the old PR.
    /// 4. Delete the old branch (non-fatal if it fails — log and continue).
    #[allow(clippy::too_many_arguments)] // owner/repo/old_pr/version/changelog/sha/branch is the minimal rename surface
    async fn rename_release_pr(
        &self,
        owner: &str,
        repo: &str,
        old_pr: &PullRequest,
        version: &SemanticVersion,
        changelog: &str,
        base_sha: &str,
        base_branch: &str,
    ) -> CoreResult<PullRequest> {
        // Create new branch + PR for the higher version.
        let (new_pr, new_branch) = self
            .create_release_branch_and_pr(owner, repo, version, changelog, base_branch, base_sha)
            .await?;

        // Close the superseded PR.
        if let Err(e) = self
            .github
            .update_pull_request(
                owner,
                repo,
                old_pr.number,
                None,
                None,
                Some("closed".to_string()),
            )
            .await
        {
            warn!(
                error = %e,
                pr_number = old_pr.number,
                "Failed to close old release PR; continuing"
            );
        }

        // Delete the old branch (non-fatal).
        let old_branch = &old_pr.head.ref_name;
        if let Err(e) = self.github.delete_branch(owner, repo, old_branch).await {
            warn!(
                error = %e,
                branch = %old_branch,
                "Failed to delete old release branch; continuing"
            );
        }

        info!(
            old_pr_number = old_pr.number,
            new_pr_number = new_pr.number,
            new_branch = %new_branch,
            "Renamed release PR to new version"
        );

        Ok(new_pr)
    }

    // ── Naming helpers ─────────────────────────────────────────────────────

    /// Returns the head-branch query prefix, e.g. `"release/v"`.
    fn release_branch_prefix(&self) -> String {
        format!("{}/v", self.config.branch_prefix)
    }

    /// Construct the canonical release branch name, e.g. `"release/v1.2.3"`.
    pub(crate) fn make_branch_name(&self, version: &SemanticVersion) -> String {
        format!(
            "{}/{}",
            self.config.branch_prefix,
            version.to_string_with_prefix(true)
        )
    }

    /// Construct a timestamped fallback branch name used when the canonical
    /// branch already exists, e.g. `"release/v1.2.3-1711234567"`.
    pub(crate) fn make_fallback_branch_name(&self, version: &SemanticVersion) -> String {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!(
            "{}/{}-{ts}",
            self.config.branch_prefix,
            version.to_string_with_prefix(true)
        )
    }

    // ── Template / body helpers ────────────────────────────────────────────

    /// Render the PR title from the configured template.
    fn render_title(&self, version: &SemanticVersion) -> String {
        self.config
            .title_template
            .replace("{version}", &version.to_string())
            .replace("{version_tag}", &version.to_string_with_prefix(true))
    }

    /// Wrap the changelog body in the standard PR body format.
    fn render_body(&self, changelog: &str) -> String {
        format!("{}\n\n{}", self.config.changelog_header, changelog)
    }

    /// Extract the changelog section from a PR body.
    ///
    /// Returns everything between the configured changelog header and the next
    /// `##` heading (or end of string), trimmed.  Delegates to the public free
    /// function [`extract_changelog_from_pr_body`].
    fn extract_changelog_from_body<'b>(&self, body: &'b str) -> &'b str {
        // The free function returns an owned String; we extract a static slice
        // from `body` directly to preserve the borrow lifetime.
        let header = &self.config.changelog_header;
        let Some(after_header) = body
            .find(header.as_str())
            .map(|i| &body[i + header.len()..])
        else {
            return "";
        };

        // Find the next `##` heading after the changelog header.
        let end = after_header.find("\n##").unwrap_or(after_header.len());

        after_header[..end].trim()
    }

    /// Merge two changelog bodies, deduplicating entries by commit SHA.
    ///
    /// Each line in a changelog body that starts with `- ` and contains a
    /// 40-character hex SHA is treated as a unique commit entry.  Lines that
    /// do not match this pattern (e.g. section headers) are included from the
    /// *existing* body only, so that formatting is preserved.
    ///
    /// The merged result preserves the order of the existing body and appends
    /// any new entries from `new_changelog` that were not already present.
    #[must_use]
    pub fn merge_changelog_bodies(&self, existing_body: &str, new_changelog: &str) -> String {
        let existing_changelog = self.extract_changelog_from_body(existing_body);
        merge_changelog_sections(existing_changelog, new_changelog)
    }

    /// Try to parse a [`SemanticVersion`] from a branch name.
    ///
    /// Expects the branch to start with `{branch_prefix}/v` followed by a
    /// valid semver string, e.g. `"release/v1.2.3" → SemanticVersion { 1, 2, 3 }`.
    fn parse_version_from_branch(&self, branch: &str) -> Option<SemanticVersion> {
        let prefix = self.release_branch_prefix();
        let version_str = branch.strip_prefix(&prefix)?;
        crate::versioning::VersionCalculator::parse_version(version_str).ok()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public free function: changelog extraction
// ─────────────────────────────────────────────────────────────────────────────

/// Extract the changelog section from a PR body string.
///
/// Scans `body` for the first occurrence of `changelog_header` (e.g.
/// `"## Changelog"`) and returns the text between that header and the next
/// `##` heading — or the end of the string if no further heading exists — with
/// surrounding whitespace trimmed.
///
/// Returns an empty string when `changelog_header` is not found in `body`.
///
/// # Parameters
///
/// - `body`: The full PR body (markdown string) to scan.
/// - `changelog_header`: The exact header string used to delimit the changelog
///   section.  Must match the value used when creating release PRs (default:
///   `"## Changelog"`).
///
/// # Examples
///
/// ```rust
/// use release_regent_core::release_orchestrator::extract_changelog_from_pr_body;
///
/// let body = "## Changelog\n\n- feat: add widget [abc123]\n\n## Notes\n\nSee wiki.";
/// let changelog = extract_changelog_from_pr_body(body, "## Changelog");
/// assert_eq!(changelog, "- feat: add widget [abc123]");
///
/// let missing = extract_changelog_from_pr_body("No header here.", "## Changelog");
/// assert_eq!(missing, "");
/// ```
#[must_use]
pub fn extract_changelog_from_pr_body(body: &str, changelog_header: &str) -> String {
    let Some(after_header) = body
        .find(changelog_header)
        .map(|i| &body[i + changelog_header.len()..])
    else {
        return String::new();
    };

    let end = after_header.find("\n##").unwrap_or(after_header.len());
    after_header[..end].trim().to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Free helper: changelog merging
// ─────────────────────────────────────────────────────────────────────────────

/// Merge two formatted changelog strings, deduplicating by commit SHA.
///
/// Lines matching the pattern `- ... [<40-hex-SHA>]` are treated as commit
/// entries.  New entries from `new_section` that share a SHA with an entry in
/// `existing_section` are dropped.  All other lines are kept as-is from
/// `existing_section`, with unique new entries appended.
fn merge_changelog_sections(existing_section: &str, new_section: &str) -> String {
    /// Extract the last 40-character hex token from a commit line if it exists.
    fn extract_sha(line: &str) -> Option<&str> {
        // Looks for `[<sha>]` at end of line where sha is exactly 40 hex chars.
        let inner = line.rfind('[').and_then(|i| {
            let after = &line[i + 1..];
            let close = after.find(']')?;
            Some(&after[..close])
        })?;
        if inner.len() == 40 && inner.chars().all(|c| c.is_ascii_hexdigit()) {
            Some(inner)
        } else {
            None
        }
    }

    // Collect SHAs already present in the existing section.
    let existing_shas: std::collections::HashSet<&str> =
        existing_section.lines().filter_map(extract_sha).collect();

    // Collect section headers (`### …`) already present in the existing section
    // so we can detect entirely new sub-sections in the incoming changelog.
    let existing_headers: std::collections::HashSet<&str> = existing_section
        .lines()
        .filter(|l| l.starts_with("###"))
        .collect();

    // Walk new_section line-by-line. For each commit line whose SHA is not yet
    // in the existing section, include it. If that commit's sub-section heading
    // is absent from the existing section, prepend the heading so the appended
    // entries retain their categorical context.
    let mut new_lines: Vec<&str> = Vec::new();
    let mut current_header: Option<&str> = None;
    let mut header_emitted = false;

    for line in new_section.lines() {
        if line.starts_with("###") {
            current_header = Some(line);
            header_emitted = false;
        } else if let Some(sha) = extract_sha(line) {
            if !existing_shas.contains(sha) {
                // Emit the section header once before the first new entry
                // when the header itself is absent from the existing section.
                if let Some(h) = current_header {
                    if !existing_headers.contains(h) && !header_emitted {
                        new_lines.push(h);
                        header_emitted = true;
                    }
                }
                new_lines.push(line);
            }
        }
    }

    if new_lines.is_empty() {
        return existing_section.to_string();
    }

    let mut result = existing_section.to_string();
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    result.push_str(&new_lines.join("\n"));
    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "release_orchestrator_tests.rs"]
mod tests;
