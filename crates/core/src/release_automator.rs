//! Release automation for Release Regent
//!
//! This module implements the [`ReleaseAutomator`], which is responsible for
//! creating GitHub releases when release pull requests are merged.
//!
//! ## Responsibilities
//!
//! When [`EventType::ReleasePrMerged`] arrives in the event loop, the automator:
//!
//! 1. **Extracts** the version from the PR head branch name (`release/v{version}`).
//! 2. **Extracts** the merge commit SHA from the webhook payload.
//! 3. **Creates an annotated Git tag** pointing to the merge commit.
//! 4. **Extracts** the changelog from the PR body.
//! 5. **Creates a GitHub release** using the tag, with the changelog as release
//!    notes and the pre-release flag set when the version contains a pre-release
//!    identifier.
//! 6. **Deletes the release branch** after a successful release (non-fatal on
//!    failure — the release has already been published).
//!
//! ## Idempotency
//!
//! All operations are safe to retry:
//!
//! - If `create_tag` returns [`CoreError::NotSupported`] (tag already exists),
//!   the automator checks whether a matching release also exists via
//!   `get_release_by_tag`.
//!   - **Release exists**: return `Ok(AutomatorResult::Created { release })` — already done.
//!   - **Release absent**: skip tag creation and proceed from step 4 onward to
//!     create the release using the existing tag.
//! - Other GitHub API failures are propagated so the event loop can retry.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use release_regent_core::release_automator::{ReleaseAutomator, AutomatorConfig};
//!
//! let config = AutomatorConfig::default();
//! let automator = ReleaseAutomator::new(config, &github);
//! let result = automator
//!     .automate("myorg", "myrepo", &event, "corr-id-001")
//!     .await?;
//! ```

use crate::{
    release_orchestrator::extract_changelog_from_pr_body,
    traits::{
        event_source::ProcessingEvent,
        github_operations::{CreateReleaseParams, GitHubOperations, Release},
    },
    versioning::{SemanticVersion, VersionCalculator},
    CoreError, CoreResult,
};
use tracing::{info, warn};

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the [`ReleaseAutomator`].
#[derive(Debug, Clone)]
pub struct AutomatorConfig {
    /// Branch name prefix; combined with the version to form `release/v1.2.3`.
    ///
    /// Defaults to `"release"`.
    pub branch_prefix: String,

    /// Sentinel that marks the start of the changelog section in the PR body.
    ///
    /// Defaults to `"## Changelog"`.
    pub changelog_header: String,
}

impl Default for AutomatorConfig {
    fn default() -> Self {
        Self {
            branch_prefix: "release".to_string(),
            changelog_header: "## Changelog".to_string(),
        }
    }
}

/// The outcome of a single [`ReleaseAutomator::automate`] call.
#[derive(Debug, Clone)]
pub enum AutomatorResult {
    /// The GitHub release is ready — either freshly created or already present.
    ///
    /// When the matching Git tag **and** GitHub release both existed before this
    /// call, no new resources are created and this variant is returned
    /// unchanged, making the operation safe to retry.
    Created {
        /// The created (or previously existing) GitHub release.
        release: Release,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// ReleaseAutomator
// ─────────────────────────────────────────────────────────────────────────────

/// Automates GitHub release creation from merged release pull requests.
///
/// Generic over `G` so that tests can inject an inline test double while
/// production code uses the real `GitHubClient`.
pub struct ReleaseAutomator<'a, G: GitHubOperations> {
    config: AutomatorConfig,
    github: &'a G,
}

impl<'a, G: GitHubOperations + Send + Sync> ReleaseAutomator<'a, G> {
    /// Create a new automator.
    ///
    /// # Parameters
    /// - `config`: Automator configuration (branch prefix, changelog header).
    /// - `github`: Reference to the `GitHubOperations` implementation to use.
    pub fn new(config: AutomatorConfig, github: &'a G) -> Self {
        Self { config, github }
    }

    /// Run the release automation workflow for a merged release PR.
    ///
    /// Reads required data from `event.payload` using standard GitHub webhook
    /// field paths.  Missing fields cause [`CoreError::InvalidInput`] so the
    /// event loop can distinguish a malformed payload (permanent) from a
    /// transient API error.
    ///
    /// # Parameters
    /// - `owner`: Repository owner / organisation.
    /// - `repo`: Repository name.
    /// - `event`: The [`ProcessingEvent`] of type [`EventType::ReleasePrMerged`].
    /// - `correlation_id`: Tracing correlation ID propagated from the event.
    ///
    /// # Errors
    ///
    /// - [`CoreError::InvalidInput`] — branch name cannot be parsed as a
    ///   semantic version, or the payload is missing required fields.
    /// - [`CoreError::GitHub`] — a GitHub API call failed for a non-idempotent
    ///   reason.
    #[tracing::instrument(skip(self, event), fields(owner, repo, correlation_id))]
    pub async fn automate(
        &self,
        owner: &str,
        repo: &str,
        event: &ProcessingEvent,
        correlation_id: &str,
    ) -> CoreResult<AutomatorResult> {
        let (branch, merge_sha, pr_body) = extract_payload_fields(event)?;
        let version = extract_version_from_branch(&branch, &self.config.branch_prefix)?;
        let tag_name = version.to_string_with_prefix(true);

        info!(
            owner, repo, branch = %branch, tag = %tag_name, sha = %merge_sha,
            correlation_id, "Automating GitHub release for merged release PR"
        );

        // Create the annotated Git tag, handling the idempotent case where the
        // tag already exists.
        if let Some(existing) = self
            .ensure_tag_and_get_existing_release(owner, repo, &tag_name, &merge_sha)
            .await?
        {
            // Tag and release both exist — this is a full idempotent retry.
            // Still attempt branch cleanup: a previous run may have succeeded at
            // tag+release creation but failed before (or during) deletion.
            if let Err(e) = self.github.delete_branch(owner, repo, &branch).await {
                warn!(
                    error = %e, branch = %branch,
                    "Failed to delete release branch in idempotent path; continuing"
                );
            } else {
                tracing::debug!(branch = %branch, "Deleted release branch (idempotent path)");
            }
            return Ok(AutomatorResult::Created { release: existing });
        }

        // Extract changelog and create the GitHub release.
        let changelog = extract_changelog_from_pr_body(&pr_body, &self.config.changelog_header);
        let is_prerelease = version.is_prerelease();

        let release = self
            .github
            .create_release(
                owner,
                repo,
                CreateReleaseParams {
                    tag_name: tag_name.clone(),
                    name: Some(tag_name.clone()),
                    body: Some(changelog),
                    draft: false,
                    prerelease: is_prerelease,
                    generate_release_notes: false,
                    target_commitish: Some(merge_sha),
                },
            )
            .await?;

        info!(
            release_id = release.id, tag = %tag_name, prerelease = is_prerelease,
            "Created GitHub release"
        );

        // Delete the release branch (non-fatal on failure).
        if let Err(e) = self.github.delete_branch(owner, repo, &branch).await {
            warn!(
                error = %e, branch = %branch,
                "Failed to delete release branch after release creation; continuing"
            );
        } else {
            tracing::debug!(branch = %branch, "Deleted release branch");
        }

        Ok(AutomatorResult::Created { release })
    }

    // ── Private helpers ────────────────────────────────────────────────────

    /// Create the annotated Git tag for `tag_name` at `merge_sha`.
    ///
    /// Returns `Ok(Some(release))` when the tag **and** release already exist
    /// (idempotent resumption).  Returns `Ok(None)` when the caller should
    /// continue and create the release.
    ///
    /// # Errors
    ///
    /// Returns the underlying `CoreError` on any non-idempotent GitHub API
    /// failure.
    async fn ensure_tag_and_get_existing_release(
        &self,
        owner: &str,
        repo: &str,
        tag_name: &str,
        merge_sha: &str,
    ) -> CoreResult<Option<Release>> {
        let tag_message = format!("Release {tag_name}");
        match self
            .github
            .create_tag(owner, repo, tag_name, merge_sha, Some(tag_message), None)
            .await
        {
            Ok(_) => Ok(None),
            Err(CoreError::NotSupported { .. }) => {
                // Tag already exists; check whether a release also exists.
                tracing::debug!(tag = %tag_name, "Tag already exists; checking for existing release");
                match self.github.get_release_by_tag(owner, repo, tag_name).await {
                    Ok(existing_release) => {
                        info!(
                            tag = %tag_name, release_id = existing_release.id,
                            "Release already exists for tag; returning idempotent result"
                        );
                        Ok(Some(existing_release))
                    }
                    Err(CoreError::NotFound { .. } | CoreError::NotSupported { .. }) => {
                        // Tag exists but release does not — fall through to create release.
                        tracing::debug!(
                            tag = %tag_name,
                            "Tag exists but release is absent; creating release from existing tag"
                        );
                        Ok(None)
                    }
                    Err(other) => Err(other),
                }
            }
            Err(other) => Err(other),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Free helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Extract the three required fields from a `ReleasePrMerged` webhook payload.
///
/// Returns `(branch, merge_sha, pr_body)`.
///
/// # Errors
///
/// Returns [`CoreError::InvalidInput`] when `pull_request.head.ref` is absent
/// or when neither `merge_commit_sha` nor `pull_request.head.sha` are present.
// `CoreError` is a large enum used uniformly throughout the codebase.
// The same allow is applied to `extract_version_from_branch` and other free
// functions in this file for the same reason.
#[allow(clippy::result_large_err)]
fn extract_payload_fields(event: &ProcessingEvent) -> CoreResult<(String, String, String)> {
    let branch = event
        .payload
        .get("pull_request")
        .and_then(|pr| pr.get("head"))
        .and_then(|h| h.get("ref"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            CoreError::invalid_input(
                "payload",
                "ReleasePrMerged payload is missing pull_request.head.ref",
            )
        })?
        .to_string();

    let merge_sha = event
        .payload
        .get("pull_request")
        .and_then(|pr| pr.get("merge_commit_sha"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            event
                .payload
                .get("pull_request")
                .and_then(|pr| pr.get("head"))
                .and_then(|h| h.get("sha"))
                .and_then(serde_json::Value::as_str)
        })
        .ok_or_else(|| {
            CoreError::invalid_input(
                "payload",
                "ReleasePrMerged payload is missing both \
                 merge_commit_sha and pull_request.head.sha",
            )
        })?
        .to_string();

    let pr_body = event
        .payload
        .get("pull_request")
        .and_then(|pr| pr.get("body"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();

    Ok((branch, merge_sha, pr_body))
}

/// Extract a [`SemanticVersion`] from a release branch name.
///
/// Expects the branch to start with `{branch_prefix}/v` followed by a valid
/// semantic version string, e.g. `"release/v1.2.3"` or `"release/v1.0.0-rc.1"`.
///
/// # Errors
///
/// Returns [`CoreError::InvalidInput`] when:
/// - The branch name does not start with `{branch_prefix}/v`.
/// - The version suffix is not a valid semantic version.
///
/// # Examples
///
/// ```
/// use release_regent_core::release_automator::extract_version_from_branch;
///
/// let v = extract_version_from_branch("release/v1.2.3", "release").unwrap();
/// assert_eq!(v.to_string(), "1.2.3");
///
/// let pre = extract_version_from_branch("release/v1.0.0-rc.1", "release").unwrap();
/// assert!(pre.is_prerelease());
///
/// assert!(extract_version_from_branch("feature/abc", "release").is_err());
/// ```
// `CoreError` is a large enum used uniformly throughout the codebase.
// Boxing it would require changing the entire `CoreResult<T>` type alias — a
// project-wide refactor. Allow the lint here as it is consistent with the
// existing pattern in `release_orchestrator.rs` and other modules.
#[allow(clippy::result_large_err)]
pub fn extract_version_from_branch(
    branch: &str,
    branch_prefix: &str,
) -> CoreResult<SemanticVersion> {
    let prefix = format!("{branch_prefix}/v");
    let version_str = branch.strip_prefix(&prefix).ok_or_else(|| {
        CoreError::invalid_input(
            "branch",
            format!(
                "Branch '{branch}' does not match the expected release branch \
                 pattern '{branch_prefix}/v<version>'"
            ),
        )
    })?;
    VersionCalculator::parse_version(version_str)
}

/// Returns `true` when the branch name starts with the release branch prefix
/// (`{branch_prefix}/v`).
///
/// **This is a prefix-only check.** It does not validate that the version
/// suffix is a valid semantic version. For example, `"release/vnot-valid"`
/// returns `true` even though [`extract_version_from_branch`] would return an
/// error for that branch. Use this function as a lightweight pre-filter and
/// call [`extract_version_from_branch`] whenever you need to parse or validate
/// the version.
///
/// # Examples
///
/// ```
/// use release_regent_core::release_automator::is_release_pr_branch;
///
/// assert!(is_release_pr_branch("release/v1.2.3", "release"));
/// assert!(is_release_pr_branch("release/v1.0.0-rc.1", "release"));
/// // Prefix-only: "release/vnot-valid" passes the prefix check even though
/// // extract_version_from_branch would reject the version suffix.
/// assert!(is_release_pr_branch("release/vnot-valid", "release"));
/// assert!(!is_release_pr_branch("feature/my-feature", "release"));
/// assert!(!is_release_pr_branch("release/not-a-version", "release"));
/// ```
#[must_use]
pub fn is_release_pr_branch(branch: &str, branch_prefix: &str) -> bool {
    let prefix = format!("{branch_prefix}/v");
    branch.starts_with(&prefix)
}

#[cfg(test)]
#[path = "release_automator_tests.rs"]
mod tests;
