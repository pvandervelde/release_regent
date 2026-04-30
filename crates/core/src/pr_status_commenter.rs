//! PR status comment rendering and upsert infrastructure.
//!
//! This module provides utilities for posting and maintaining a single
//! status comment on every open pull request.  A signed HTML marker
//! (`<!-- release-regent:pr-status -->`) is embedded in each comment so that
//! subsequent calls update the existing comment in place rather than creating
//! duplicates.
//!
//! # Exported items
//!
//! - [`upsert_pr_status_comment`] — create or update the status comment
//! - [`render_feature_pr_comment`] — render the feature-branch comment body
//! - [`render_release_pr_comment`] — render the release-branch comment body
//! - [`PR_STATUS_MARKER`] — the HTML marker string embedded in every comment

use crate::{traits::github_operations::GitHubOperations, versioning::SemanticVersion, CoreResult};

/// HTML marker embedded in every status comment posted by Release Regent.
///
/// The marker is placed at the very start of the comment body so that
/// [`upsert_pr_status_comment`] can locate an existing comment to update
/// rather than creating a duplicate.
pub(crate) const PR_STATUS_MARKER: &str = "<!-- release-regent:pr-status -->";

/// Create or update the Release Regent status comment on an issue or PR.
///
/// Scans existing comments via [`GitHubOperations::list_issue_comments`].
/// If a comment whose body contains [`PR_STATUS_MARKER`] is found, it is
/// updated via [`GitHubOperations::update_issue_comment`].  Otherwise a new
/// comment is created via [`GitHubOperations::create_issue_comment`].
///
/// # Parameters
/// - `github`: GitHub API client scoped to the correct installation
/// - `owner`: Repository owner
/// - `repo`: Repository name
/// - `issue_number`: Issue or PR number to comment on
/// - `body`: New comment body (should begin with [`PR_STATUS_MARKER`])
///
/// # Errors
/// Propagates any error returned by the underlying GitHub API calls.
///
/// # Examples
///
/// ```rust,ignore
/// use release_regent_core::pr_status_commenter::{PR_STATUS_MARKER, upsert_pr_status_comment};
///
/// let body = format!("{PR_STATUS_MARKER}\n## Status\nAll good");
/// upsert_pr_status_comment(&github, "owner", "repo", 42, &body).await?;
/// ```
// CoreError is intentionally large; this is the project-wide pattern.
#[allow(clippy::result_large_err)]
pub(crate) async fn upsert_pr_status_comment<G: GitHubOperations>(
    github: &G,
    owner: &str,
    repo: &str,
    issue_number: u64,
    body: &str,
) -> CoreResult<()> {
    let comments = github
        .list_issue_comments(owner, repo, issue_number)
        .await?;

    let existing = comments.iter().find(|c| c.body.contains(PR_STATUS_MARKER));

    if let Some(comment) = existing {
        github
            .update_issue_comment(owner, repo, comment.id, body)
            .await
    } else {
        github
            .create_issue_comment(owner, repo, issue_number, body)
            .await
    }
}

/// Render the feature-branch PR status comment body.
///
/// Produces a Markdown string beginning with [`PR_STATUS_MARKER`].
///
/// When `projected_version == base_version` the commits in this PR do not
/// trigger any version bump; the comment says so rather than echoing back the
/// current version as "the next release".
///
/// When `queued_release_version` is `Some(v)` and `v > base_version` (i.e. a
/// release PR for a higher version is already open) a blockquote note is
/// appended so PR authors know their changes will land after that release.
///
/// # Parameters
/// - `projected_version`: Version that would be published if this PR is merged now
/// - `base_version`: Latest released version from which the projection starts
/// - `queued_release_version`: Highest open release-branch PR version, if any
/// - `allow_override`: When `true`, appends the `### Available commands` table
///
/// # Returns
/// A Markdown string beginning with [`PR_STATUS_MARKER`].
///
/// # Examples
///
/// ```rust,ignore
/// use release_regent_core::pr_status_commenter::render_feature_pr_comment;
/// use release_regent_core::versioning::SemanticVersion;
///
/// let body = render_feature_pr_comment(&v1_1_0, &v1_0_0, None, true);
/// assert!(body.contains("v1.1.0"));
/// ```
#[must_use]
pub(crate) fn render_feature_pr_comment(
    projected_version: &SemanticVersion,
    base_version: &SemanticVersion,
    queued_release_version: Option<&SemanticVersion>,
    allow_override: bool,
) -> String {
    let mut body = if projected_version == base_version {
        format!(
            "{marker}\n\
             **Release Regent \u{2014} no version change**\n\n\
             This PR's commits do not affect the version number.\n\
             It will be included in the next release.\n\
             _(Projection based on commits since `v{base}`. Updates automatically when other PRs land.)_",
            marker = PR_STATUS_MARKER,
            base = base_version,
        )
    } else {
        format!(
            "{marker}\n\
             **Release Regent \u{2014} projected release**\n\n\
             If this PR is merged now, the next release will be **v{projected}**.\n\
             _(Projection based on commits since `v{base}`. Updates automatically when other PRs land.)_",
            marker = PR_STATUS_MARKER,
            projected = projected_version,
            base = base_version,
        )
    };

    if let Some(queued) = queued_release_version {
        if queued > base_version {
            body.push_str(&format!(
                "\n\n\
                 > **Note:** Release PR for **v{queued}** is already open. \
                 This PR's changes will be included in a subsequent release.",
            ));
        }
    }

    if allow_override {
        body.push_str(
            "\n\n\
             ### Available commands\n\
             | Command | Effect |\n\
             |---------|--------|\n\
             | `!release major` | Force at least a major version bump for this PR |\n\
             | `!release minor` | Force at least a minor version bump |\n\
             | `!release patch` | Force at least a patch version bump |\n\
             | `!set-version X.Y.Z` | Pin the exact release version |",
        );
    }

    body
}

/// Render the release-branch PR status comment body.
///
/// Produces a Markdown string beginning with [`PR_STATUS_MARKER`] that
/// identifies the PR as a managed release PR and shows the target version.
///
/// # Parameters
/// - `release_version`: The version this release PR will publish
/// - `allow_override`: When `true`, appends the `### Available commands` table
///
/// # Returns
/// A Markdown string beginning with [`PR_STATUS_MARKER`].
///
/// # Examples
///
/// ```rust,ignore
/// use release_regent_core::pr_status_commenter::render_release_pr_comment;
/// use release_regent_core::versioning::SemanticVersion;
///
/// let body = render_release_pr_comment(&v2_0_0, true);
/// assert!(body.contains("v2.0.0"));
/// ```
#[must_use]
pub(crate) fn render_release_pr_comment(
    release_version: &SemanticVersion,
    allow_override: bool,
) -> String {
    let mut body = format!(
        "{marker}\n\
         **Release Regent \u{2014} release PR**\n\n\
         This PR is managed by Release Regent. Merging it will publish release **v{version}**.",
        marker = PR_STATUS_MARKER,
        version = release_version,
    );

    if allow_override {
        body.push_str(
            "\n\n\
             ### Available commands\n\
             | Command | Effect |\n\
             |---------|--------|\n\
             | `!set-version X.Y.Z` | Re-target this release to a different version |",
        );
    }

    body
}

#[cfg(test)]
#[path = "pr_status_commenter_tests.rs"]
mod tests;
