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
//!    If the canonical release branch already exists (e.g. from a previous run that
//!    created the branch but failed before opening the PR), the orchestrator reuses
//!    it rather than failing.
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
    manifest::ManifestFileConfig,
    traits::github_operations::{
        CreatePullRequestParams, FileUpdate, GitHubOperations, PullRequest,
    },
    versioning::SemanticVersion,
    CoreError, CoreResult,
};
use chrono::Utc;
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

    /// Version prefix prepended to the semver when forming branch names and PR titles.
    ///
    /// For example, `"v"` (the default) produces `release/v1.2.3`; an empty string
    /// produces `release/1.2.3`.
    ///
    /// Defaults to `"v"`.
    pub version_prefix: String,

    /// Template for the release PR title.
    ///
    /// Supports `{version}` (e.g. `"1.2.3"`) and `{version_tag}` (e.g. `"v1.2.3"`).
    /// Defaults to `"chore(release): {version_tag}"`.
    pub title_template: String,

    /// Template for the release PR body.
    ///
    /// Use `${changelog}` as a placeholder; it is replaced with the formatted
    /// changelog entries for **this release only**.  Any text before or after the
    /// placeholder (version badges, footers, etc.) is preserved verbatim.
    ///
    /// Defaults to `"## Changelog\n\n${changelog}"`.
    pub body_template: String,

    /// Sentinel that separates the generated changelog from any trailing content
    /// in the PR body.  The orchestrator will only look for commits between
    /// `## Changelog` and the next `##` heading (or end of string).
    ///
    /// Must match the Markdown heading that immediately precedes `${changelog}`
    /// in [`Self::body_template`] so that changelog extraction from existing PR
    /// bodies works correctly when updating an existing release PR.
    ///
    /// Defaults to `"## Changelog"`.
    pub changelog_header: String,

    /// Explicit list of version manifest files to update when creating or updating
    /// the release branch.  Each entry is committed as part of the single atomic
    /// batch commit together with `CHANGELOG.md`.
    ///
    /// Entries here take precedence over auto-detected files when the same path
    /// appears in both lists.
    ///
    /// Defaults to an empty list (no explicit manifest files).
    pub manifest_files: Vec<ManifestFileConfig>,

    /// When `true` (the default), the orchestrator probes the repository for the
    /// well-known manifest files defined in [`crate::manifest::detect_standard_manifests`]
    /// and includes any that exist alongside the explicit [`Self::manifest_files`].
    ///
    /// Set to `false` to disable auto-detection and rely solely on the explicit list.
    pub auto_detect_manifests: bool,
}

impl OrchestratorConfig {
    /// The default branch prefix used when no explicit configuration is provided.
    pub const DEFAULT_BRANCH_PREFIX: &'static str = "release";

    /// The default version prefix used when no explicit configuration is provided.
    pub const DEFAULT_VERSION_PREFIX: &'static str = "v";

    /// The default PR body template string.
    pub const DEFAULT_BODY_TEMPLATE: &'static str = "## Changelog\n\n${changelog}";
}

/// Derive the changelog-section header from a PR body template.
///
/// Scans `body_template` backwards from the `${changelog}` placeholder and
/// returns the first `## ` heading line found.  Falls back to `"## Changelog"`
/// when no such heading precedes the placeholder.
///
/// This keeps [`OrchestratorConfig::changelog_header`] and
/// [`OrchestratorConfig::body_template`] in sync automatically: callers only
/// need to set `body_template` and can derive `changelog_header` from it.
///
/// # Examples
///
/// ```
/// use release_regent_core::release_orchestrator::extract_changelog_header;
///
/// assert_eq!(extract_changelog_header("## Changelog\n\n${changelog}"), "## Changelog");
/// assert_eq!(extract_changelog_header("## Release Notes\n\n${changelog}"), "## Release Notes");
/// // Falls back when no heading precedes the placeholder.
/// assert_eq!(extract_changelog_header("${changelog}"), "## Changelog");
/// ```
pub fn extract_changelog_header(body_template: &str) -> String {
    // Walk lines before ${changelog} in reverse to find the nearest ## heading.
    let marker = "${changelog}";
    let prefix = match body_template.find(marker) {
        Some(i) => &body_template[..i],
        None => body_template,
    };
    prefix
        .lines()
        .rev()
        .find(|l| l.starts_with("## "))
        .unwrap_or("## Changelog")
        .to_string()
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        let body_template = Self::DEFAULT_BODY_TEMPLATE.to_string();
        let changelog_header = extract_changelog_header(&body_template);
        Self {
            branch_prefix: Self::DEFAULT_BRANCH_PREFIX.to_string(),
            version_prefix: Self::DEFAULT_VERSION_PREFIX.to_string(),
            title_template: "chore(release): {version_tag}".to_string(),
            changelog_header,
            body_template,
            manifest_files: Vec::new(),
            auto_detect_manifests: true,
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

    /// No version-bumping commits were found since the last release and no
    /// bump-override floor was applied, so the calculated next version is
    /// identical to the already-released version.
    ///
    /// No release branch or PR was created or modified.  This is the expected
    /// outcome when a `chore:`, `docs:`, or other non-bumping PR is merged
    /// immediately after a release.
    NoBumpNeeded,

    /// A merged pull request was identified as a release PR by the
    /// [`crate::ReleaseRegentProcessor`] even though it arrived via the
    /// `PullRequestMerged` event (misclassified by the server when its
    /// `version_prefix` default does not match the repository's configured
    /// prefix).  A git tag and GitHub release were created, and the release
    /// branch was deleted.
    ///
    /// No new release PR is opened on this path; the next release cycle
    /// begins when the next feature PR merges.
    TaggedRelease,
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
    #[tracing::instrument(skip(self, changelog, version), fields(owner, repo, base_branch, base_sha, correlation_id, version = %version))]
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
                            .update_release_pr(
                                owner,
                                repo,
                                &existing_pr,
                                version,
                                changelog,
                                base_sha,
                            )
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
    /// with the configured `{branch_prefix}/{version_prefix}` pattern.
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
    ///
    /// All file changes (CHANGELOG.md plus any version manifest files) are committed
    /// via [`GitHubOperations::batch_commit_files_rebased`] with `base_sha` as the
    /// explicit parent commit.  This guarantees the branch tip **never passes through
    /// a state where it equals `base_sha`**, which would cause GitHub to auto-close
    /// any open release PR (head == base → no diff to merge).
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
                // The branch already exists — most likely from a previous run that
                // created the branch but failed before opening the PR, or because an
                // open release PR was active when this orchestration ran.
                //
                // Do NOT call `force_update_branch(base_sha)` here: that would
                // temporarily set the branch tip equal to `base_sha`, making head
                // and base of any open PR identical and causing GitHub to auto-close
                // it.  Instead, `batch_commit_files_rebased` below creates the
                // release-files commit directly on top of `base_sha` and then
                // force-updates the branch ref to that new commit atomically.
                info!(
                    branch = %branch_name,
                    "Release branch already exists; reusing it via rebased commit"
                );
                branch_name
            }
            Err(other) => return Err(other),
        };

        let title = self.render_title(version);
        let body = self.render_body(changelog);

        // Fetch the existing CHANGELOG.md from the base branch so we can
        // prepend the new version section rather than overwriting history.
        let existing_changelog_file = self
            .github
            .get_file_content(owner, repo, "CHANGELOG.md", base_branch)
            .await
            .unwrap_or_else(|e| {
                warn!(error = %e, "Failed to fetch existing CHANGELOG.md; starting fresh");
                None
            })
            .unwrap_or_default();

        let today = Utc::now().format("%Y-%m-%d").to_string();
        let changelog_file_content = build_changelog_file_content(
            &existing_changelog_file,
            &version.to_string(),
            &today,
            changelog,
        );

        // Build the batch of files: CHANGELOG.md plus any manifest files.
        let mut file_updates = vec![FileUpdate {
            path: "CHANGELOG.md".to_string(),
            content: changelog_file_content,
        }];
        self.collect_manifest_updates(owner, repo, &actual_branch, version, &mut file_updates)
            .await;

        // Deduplicate by path — detect_standard_manifests may emit two configs for
        // the same file (e.g. PEP-617 + Poetry keys for pyproject.toml).  Keep the
        // last entry per path to match the Git Trees API's last-writer-wins semantics.
        let file_updates = dedup_file_updates_by_path(file_updates);

        let commit_message = format!(
            "chore(release): update release files for {}{}",
            self.config.version_prefix, version
        );

        // Use `batch_commit_files_rebased` (not `batch_commit_files`) so the new
        // commit's parent is always `base_sha` regardless of whatever the current
        // branch tip is.  The branch ref is then force-updated to the new commit.
        // This ensures the branch tip never equals `base_sha` at any point during
        // the operation, preventing GitHub from auto-closing the open release PR.
        self.github
            .batch_commit_files_rebased(
                owner,
                repo,
                &actual_branch,
                &file_updates,
                &commit_message,
                base_sha,
            )
            .await?;

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
        base_sha: &str,
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

        // Rebase the release branch onto the latest base SHA and commit the
        // updated release files in a single atomic operation.
        //
        // `batch_commit_files_rebased` creates the new commit with `base_sha`
        // as its parent and then force-updates the branch ref to that commit.
        // This keeps the release branch exactly one commit ahead of the base
        // branch without ever passing through a state where the branch tip
        // equals `base_sha` — which would cause GitHub to auto-close the open
        // PR because head and base would temporarily be identical.
        //
        // Files are committed BEFORE the PR metadata is updated so that, if the
        // commit step fails, the PR body never drifts ahead of the branch content.

        // Fetch the existing CHANGELOG.md from the PR *base* branch (e.g. master)
        // rather than the PR head branch.  The head branch may contain corrupted
        // content written by an older release-regent; the base branch is always the
        // authoritative source of historical changelog data.  This is exactly
        // symmetric with the create_release_branch_and_pr path.
        let existing_changelog_file = self
            .github
            .get_file_content(owner, repo, "CHANGELOG.md", &fresh_pr.base.ref_name)
            .await
            .unwrap_or_else(|e| {
                warn!(error = %e, "Failed to fetch existing CHANGELOG.md; starting fresh");
                None
            })
            .unwrap_or_default();

        let today = Utc::now().format("%Y-%m-%d").to_string();
        let changelog_file_content = build_changelog_file_content(
            &existing_changelog_file,
            &version.to_string(),
            &today,
            &merged_changelog,
        );

        // Build batch: CHANGELOG.md plus manifest files — all in one atomic commit.
        let mut file_updates = vec![FileUpdate {
            path: "CHANGELOG.md".to_string(),
            content: changelog_file_content,
        }];
        self.collect_manifest_updates(
            owner,
            repo,
            &fresh_pr.head.ref_name,
            version,
            &mut file_updates,
        )
        .await;

        // Deduplicate by path — see create_release_branch_and_pr for rationale.
        let file_updates = dedup_file_updates_by_path(file_updates);

        let commit_message = format!(
            "chore(release): update release files for {}{}",
            self.config.version_prefix, version
        );
        self.github
            .batch_commit_files_rebased(
                owner,
                repo,
                &fresh_pr.head.ref_name,
                &file_updates,
                &commit_message,
                base_sha,
            )
            .await?;

        // Update the PR title/body only after the branch files are committed so
        // the metadata always reflects what is actually on the branch.
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

    /// Returns the head-branch query prefix, e.g. `"release/v"` with the default config,
    /// or `"release/"` when `version_prefix` is empty.
    fn release_branch_prefix(&self) -> String {
        format!(
            "{}/{}",
            self.config.branch_prefix, self.config.version_prefix
        )
    }

    /// Construct the canonical release branch name, e.g. `"release/v1.2.3"` with
    /// the default config, or `"release/1.2.3"` when `version_prefix` is empty.
    pub(crate) fn make_branch_name(&self, version: &SemanticVersion) -> String {
        format!(
            "{}/{}{version}",
            self.config.branch_prefix, self.config.version_prefix,
        )
    }

    // ── Manifest file helpers ─────────────────────────────────────────────

    /// Build the list of manifest [`FileUpdate`] entries for the release commit.
    ///
    /// Combines auto-detected and explicitly configured manifest files, reads
    /// their current content from the branch, applies the version substitution,
    /// and appends the results to `updates`.  Files that are absent on the branch
    /// or whose update fails are skipped with a `warn!` log — they must not
    /// prevent PR creation from succeeding.
    ///
    /// Explicit entries from `self.config.manifest_files` take precedence: if
    /// a path appears in both the explicit list and the auto-detected set, the
    /// explicit config is used and the auto-detected one is dropped.
    async fn collect_manifest_updates(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
        version: &SemanticVersion,
        updates: &mut Vec<FileUpdate>,
    ) {
        use crate::manifest::{detect_standard_manifests, update_manifest_content};

        let version_str = version.to_string();

        // Build the effective manifest list: explicit entries first, then
        // auto-detected ones that do not duplicate an explicit path.
        let explicit_paths: std::collections::HashSet<&str> = self
            .config
            .manifest_files
            .iter()
            .map(|m| m.path.as_str())
            .collect();

        let mut manifests: Vec<std::borrow::Cow<'_, crate::manifest::ManifestFileConfig>> = self
            .config
            .manifest_files
            .iter()
            .map(std::borrow::Cow::Borrowed)
            .collect();

        if self.config.auto_detect_manifests {
            // Probe the set of well-known files.  We store the content from the
            // probe so auto-detected files can be updated without a second API
            // call.  Explicit-list files always do a fresh fetch below.
            let candidates = [
                "Cargo.toml",
                "package.json",
                "pyproject.toml",
                "composer.json",
            ];
            let mut probe_cache: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for &candidate in &candidates {
                if explicit_paths.contains(candidate) {
                    continue; // explicit list takes precedence
                }
                match self
                    .github
                    .get_file_content(owner, repo, candidate, branch)
                    .await
                {
                    Ok(Some(content)) => {
                        probe_cache.insert(candidate.to_string(), content);
                    }
                    Ok(None) => {} // file not present — skip silently
                    Err(e) => {
                        warn!(
                            path = candidate,
                            error = %e,
                            "Failed to probe for manifest file during auto-detection; skipping"
                        );
                    }
                }
            }
            // Discover Cargo workspace member crates from the root Cargo.toml.
            // Glob-pattern entries (e.g. "crates/*") are skipped because they
            // require filesystem enumeration; list those paths explicitly in
            // `manifest_files` config if they need to be updated individually.
            if let Some(root_cargo) = probe_cache.get("Cargo.toml") {
                for member_path in cargo_workspace_member_cargo_tomls(root_cargo) {
                    if explicit_paths.contains(member_path.as_str()) {
                        continue;
                    }
                    if probe_cache.contains_key(&member_path) {
                        continue;
                    }
                    match self
                        .github
                        .get_file_content(owner, repo, &member_path, branch)
                        .await
                    {
                        Ok(Some(content)) => {
                            debug!(
                                path = %member_path,
                                "Found Cargo workspace member manifest"
                            );
                            probe_cache.insert(member_path, content);
                        }
                        Ok(None) => {} // member's Cargo.toml absent on this branch — skip
                        Err(e) => {
                            warn!(
                                path = %member_path,
                                error = %e,
                                "Failed to probe Cargo workspace member manifest; skipping"
                            );
                        }
                    }
                }
            }
            let existing: Vec<&str> = probe_cache.keys().map(|s| s.as_str()).collect();
            for cfg in detect_standard_manifests(&existing) {
                manifests.push(std::borrow::Cow::Owned(cfg));
            }

            // For each manifest, read current content and apply update.
            for manifest in &manifests {
                // Reuse cached probe content for auto-detected files; explicit-list
                // files always get a fresh fetch.
                let content = if let Some(cached) = probe_cache.get(manifest.path.as_str()) {
                    cached.clone()
                } else {
                    match self
                        .github
                        .get_file_content(owner, repo, &manifest.path, branch)
                        .await
                    {
                        Ok(Some(c)) => c,
                        Ok(None) => {
                            warn!(
                                path = %manifest.path,
                                branch,
                                "Manifest file not found on release branch; skipping"
                            );
                            continue;
                        }
                        Err(e) => {
                            warn!(
                                path = %manifest.path,
                                error = %e,
                                "Failed to read manifest file; skipping"
                            );
                            continue;
                        }
                    }
                };

                match update_manifest_content(
                    &content,
                    &manifest.format,
                    &manifest.version_key,
                    &version_str,
                ) {
                    Ok(updated) => {
                        updates.push(FileUpdate {
                            path: manifest.path.clone(),
                            content: updated,
                        });
                        debug!(
                            path = %manifest.path,
                            version = %version_str,
                            "Manifest file queued for version update"
                        );
                    }
                    Err(e) => {
                        warn!(
                            path = %manifest.path,
                            key = %manifest.version_key,
                            error = %e,
                            "Failed to apply version to manifest file; skipping"
                        );
                    }
                }
            }
        } else {
            // No auto-detection; process only explicitly configured manifests.
            for manifest in &manifests {
                let content = match self
                    .github
                    .get_file_content(owner, repo, &manifest.path, branch)
                    .await
                {
                    Ok(Some(c)) => c,
                    Ok(None) => {
                        warn!(
                            path = %manifest.path,
                            branch,
                            "Manifest file not found on release branch; skipping"
                        );
                        continue;
                    }
                    Err(e) => {
                        warn!(
                            path = %manifest.path,
                            error = %e,
                            "Failed to read manifest file; skipping"
                        );
                        continue;
                    }
                };

                match update_manifest_content(
                    &content,
                    &manifest.format,
                    &manifest.version_key,
                    &version_str,
                ) {
                    Ok(updated) => {
                        updates.push(FileUpdate {
                            path: manifest.path.clone(),
                            content: updated,
                        });
                        debug!(
                            path = %manifest.path,
                            version = %version_str,
                            "Manifest file queued for version update"
                        );
                    }
                    Err(e) => {
                        warn!(
                            path = %manifest.path,
                            key = %manifest.version_key,
                            error = %e,
                            "Failed to apply version to manifest file; skipping"
                        );
                    }
                }
            }
        }
    }

    /// Render the PR title from the configured template.
    ///
    /// Supports both `${variable}` (config-file style) and `{variable}` (internal style)
    /// for `version` (e.g. `"0.2.0"`) and `version_tag` (e.g. `"v0.2.0"`).
    fn render_title(&self, version: &SemanticVersion) -> String {
        let version_str = version.to_string();
        let version_tag_str = format!("{}{version_str}", self.config.version_prefix);
        self.config
            .title_template
            .replace("${version_tag}", &version_tag_str)
            .replace("${version}", &version_str)
            .replace("{version_tag}", &version_tag_str)
            .replace("{version}", &version_str)
    }

    /// Render the PR body by substituting `${changelog}` in the configured
    /// body template with the current release's changelog entries.
    fn render_body(&self, changelog: &str) -> String {
        self.config.body_template.replace("${changelog}", changelog)
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
    /// Expects the branch to start with `{branch_prefix}/{version_prefix}` followed by a
    /// valid semver string, e.g. `"release/v1.2.3"` when `version_prefix` is `"v"`, or
    /// `"release/1.2.3"` when `version_prefix` is `""`.
    fn parse_version_from_branch(&self, branch: &str) -> Option<SemanticVersion> {
        let prefix = self.release_branch_prefix();
        let version_str = branch.strip_prefix(&prefix)?;
        crate::versioning::VersionCalculator::parse_version(version_str).ok()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a root `Cargo.toml` and return a `Cargo.toml` path for each
/// workspace member that is listed as an **explicit path** (no glob characters).
///
/// Glob entries such as `"crates/*"` are silently skipped because enumerating
/// them requires a directory listing that is not available here.  Users should
/// add those member paths explicitly to `manifest_files` in the config when
/// they need per-crate version updates.
///
/// Returns an empty `Vec` when the content cannot be parsed or the file
/// contains no `[workspace] members` array.
fn cargo_workspace_member_cargo_tomls(workspace_cargo_toml: &str) -> Vec<String> {
    let doc: toml_edit::DocumentMut = match workspace_cargo_toml.parse() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let members = match doc
        .get("workspace")
        .and_then(|w| w.as_table())
        .and_then(|t| t.get("members"))
        .and_then(|m| m.as_array())
    {
        Some(a) => a,
        None => return Vec::new(),
    };

    members
        .iter()
        .filter_map(|v| v.as_str())
        // Skip glob patterns — they require filesystem enumeration.
        .filter(|s| !s.contains('*') && !s.contains('?') && !s.contains('[') && !s.contains('{'))
        .map(|s| format!("{s}/Cargo.toml"))
        .collect()
}

/// Deduplicate a [`FileUpdate`] list by path, keeping the **last** entry for
/// each unique path.
///
/// `detect_standard_manifests` may produce two configs for the same file (e.g.
/// both `project.version` and `tool.poetry.version` for `pyproject.toml`).
/// Passing duplicate paths to the Git Trees API results in silent data loss — the
/// last blob silently wins.  This helper makes the last-writer-wins semantics
/// explicit and predictable.
///
/// Emits a `warn!` log when duplicates are detected, because a mixed
/// workspace+package root `Cargo.toml` (containing both `[workspace.package]`
/// and `[package]`) will produce two entries for the same path; the warning
/// helps operators distinguish an expected deduplication from an accidental one.
fn dedup_file_updates_by_path(updates: Vec<FileUpdate>) -> Vec<FileUpdate> {
    let mut seen = std::collections::HashSet::new();
    let mut duplicates: Vec<String> = Vec::new();
    let mut deduped: Vec<FileUpdate> = updates
        .into_iter()
        .rev()
        .filter(|u| {
            if seen.insert(u.path.clone()) {
                true
            } else {
                duplicates.push(u.path.clone());
                false
            }
        })
        .collect();
    deduped.reverse();
    if !duplicates.is_empty() {
        warn!(
            paths = ?duplicates,
            "Duplicate manifest paths detected; keeping last entry for each. \
             For a mixed workspace+package root Cargo.toml, \
             workspace.package.version is kept because detect_standard_manifests \
             emits it last (after package.version)"
        );
    }
    deduped
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
/// Build the complete CHANGELOG.md content by inserting a new version section.
///
/// Inserts `## [{version_str}] - {date_str}\n\n{changelog_body}` after the
/// file-level `# Changelog` header and before any existing `## [version]`
/// sections.  If the file already contains a section for `version_str` it is
/// replaced so that the operation is idempotent.  When `existing_content` is
/// empty or lacks a `# Changelog` header, one is generated automatically.
pub(crate) fn build_changelog_file_content(
    existing_content: &str,
    version_str: &str,
    date_str: &str,
    changelog_body: &str,
) -> String {
    let (file_header, rest) = split_changelog_header_and_rest(existing_content);
    // Accept the file header only if it is non-empty AND begins with a `# `
    // level-1 Markdown heading.  Content that starts with `## ` or `### `
    // section markers (e.g. raw changelog entries leaked from a corrupted PR
    // body) is not a valid file-level header and must be discarded.
    let file_header =
        if file_header.trim().is_empty() || !file_header.trim_start().starts_with("# ") {
            "# Changelog".to_string()
        } else {
            file_header.trim_end().to_string()
        };
    let history = skip_existing_version_section(rest, version_str);
    let body = changelog_body.trim();
    let history = history.trim_start();
    if history.is_empty() {
        format!("{file_header}\n\n## [{version_str}] - {date_str}\n\n{body}\n")
    } else {
        format!("{file_header}\n\n## [{version_str}] - {date_str}\n\n{body}\n\n{history}\n")
    }
}

/// Split a CHANGELOG.md string into the file-level header (anything before
/// the first `## ` version line) and the remainder (from that `## ` line on).
fn split_changelog_header_and_rest(content: &str) -> (&str, &str) {
    if content.starts_with("## ") {
        return ("", content);
    }
    if let Some(pos) = content.find("\n## ") {
        (&content[..pos], &content[pos + 1..])
    } else {
        (content, "")
    }
}

/// If `rest` starts with a version section for `version_str`, skip over it
/// and return the remaining content.  Otherwise return `rest` unchanged.
fn skip_existing_version_section<'a>(rest: &'a str, version_str: &str) -> &'a str {
    let version_prefix = format!("## [{version_str}]");
    if !rest.starts_with(version_prefix.as_str()) {
        return rest;
    }
    if let Some(next_pos) = rest[1..].find("\n## ").map(|i| i + 1) {
        &rest[next_pos + 1..]
    } else {
        ""
    }
}

/// Merge two formatted changelog strings, deduplicating committed entries by SHA.
///
/// Lines matching the pattern `- ... [<hex-SHA>]` (where the SHA is 7–40
/// hexadecimal characters) are treated as commit entries.  New entries from
/// `new_section` that share a SHA with an entry in `existing_section` are
/// dropped.  All other lines are kept as-is from `existing_section`, with
/// unique new entries appended.
fn merge_changelog_sections(existing_section: &str, new_section: &str) -> String {
    /// Extract the last hex SHA token from a commit line if it exists.
    fn extract_sha(line: &str) -> Option<&str> {
        // Looks for `[<sha>]` at end of line where sha is 7-40 hex chars.
        // Both abbreviated (7-char) and full (40-char) SHAs are accepted so
        // that changelogs from any supported strategy can be deduplicated.
        let inner = line.rfind('[').and_then(|i| {
            let after = &line[i + 1..];
            let close = after.find(']')?;
            Some(&after[..close])
        })?;
        if (7..=40).contains(&inner.len()) && inner.chars().all(|c| c.is_ascii_hexdigit()) {
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
