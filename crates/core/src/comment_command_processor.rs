//! Comment command processor for Release Regent
//!
//! This module implements the [`CommentCommandProcessor`], which handles
//! [`EventType::PullRequestCommentReceived`] events and extracts recognised
//! command patterns from PR comment bodies.
//!
//! ## Recognised Commands
//!
//! | Command                       | Condition               | Effect                                                         |
//! |-------------------------------|-------------------------|----------------------------------------------------------------|
//! | `!set-version X.Y.Z`          | version > current tag   | Invokes `ReleaseOrchestrator` with pinned ver                  |
//! | `!set-version X.Y.Z`          | version ≤ current tag   | Posts rejection comment; acknowledges event                    |
//! | `!release major/minor/patch`  | always (stub)           | Posts "not yet supported" comment; acknowledges event          |
//!
//! ## Guards
//!
//! Processing of all commands is gated by `VersioningConfig::allow_override`.
//! When `allow_override` is `false` every comment is silently ignored and the
//! event is acknowledged without any GitHub API calls.
//!
//! Comments posted on **closed** pull requests are similarly ignored —
//! the open/closed state is read from `payload["issue"]["state"]` — so that
//! stale comments do not trigger spurious version changes.
//!
//! ## Payload Structure
//!
//! The processor expects the raw GitHub `issue_comment` webhook payload as the
//! [`ProcessingEvent::payload`] field:
//!
//! ```json
//! {
//!   "action": "created",
//!   "issue": { "number": 42, "state": "open" },
//!   "comment": { "body": "!set-version 2.0.0" }
//! }
//! ```

use std::cmp::Ordering;

use tracing::{debug, info, warn, Instrument};

pub use crate::versioning::BumpKind;
use crate::{
    release_orchestrator::{OrchestratorConfig, ReleaseOrchestrator},
    retry::retry_with_backoff,
    traits::{
        event_source::ProcessingEvent, git_operations::GetCommitsOptions,
        github_operations::GitHubOperations,
    },
    versioning::{
        apply_bump_floor, highest_bump_kind, resolve_current_version, SemanticVersion,
        VersionCalculator,
    },
    CoreResult,
};

// ─────────────────────────────────────────────────────────────────────────────────
// Label constants
// ─────────────────────────────────────────────────────────────────────────────────

/// GitHub label applied to a feature PR for a `!release major` override.
pub const OVERRIDE_LABEL_MAJOR: &str = "rr:override-major";
/// GitHub label applied to a feature PR for a `!release minor` override.
pub const OVERRIDE_LABEL_MINOR: &str = "rr:override-minor";
/// GitHub label applied to a feature PR for a `!release patch` override.
pub const OVERRIDE_LABEL_PATCH: &str = "rr:override-patch";
/// All three bump-override label names in one slice, useful for iteration.
pub const ALL_OVERRIDE_LABELS: &[&str] = &[
    OVERRIDE_LABEL_MAJOR,
    OVERRIDE_LABEL_MINOR,
    OVERRIDE_LABEL_PATCH,
];

/// Map a GitHub label name to the [`BumpKind`] it represents, or `None` when
/// `name` is not one of the `rr:override-*` labels.
///
/// Shared by every site that needs to resolve an override floor from a PR's
/// labels (task #211): the merged-PR label check and the commit-history
/// fallback scan both call this instead of duplicating the match arms.
pub(crate) fn label_name_to_bump_kind(name: &str) -> Option<BumpKind> {
    match name {
        OVERRIDE_LABEL_MAJOR => Some(BumpKind::Major),
        OVERRIDE_LABEL_MINOR => Some(BumpKind::Minor),
        OVERRIDE_LABEL_PATCH => Some(BumpKind::Patch),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// A command parsed from a pull request comment body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentCommand {
    /// `!set-version X.Y.Z` — pin the next release to exactly this version.
    SetVersion(SemanticVersion),
    /// `!release major|minor|patch` — override the minimum bump dimension.
    ReleaseBump(BumpKind),
    /// No recognised command was found in the comment body.
    Unknown,
}

/// Configuration for [`CommentCommandProcessor`].
#[derive(Debug, Clone)]
pub struct CommentCommandConfig {
    /// Configuration forwarded to the release orchestrator when `!set-version`
    /// triggers an orchestration run.
    pub orchestrator_config: OrchestratorConfig,
    /// Whether PR comment overrides are enabled for this repository.
    ///
    /// When `false` all commands are silently ignored and the event is
    /// acknowledged without any GitHub API calls.
    pub allow_override: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// CommentCommandProcessor
// ─────────────────────────────────────────────────────────────────────────────

/// Processes [`EventType::PullRequestCommentReceived`] events.
///
/// Generic over `G` so that tests can inject an inline test double while
/// production code uses the real GitHub client.
pub struct CommentCommandProcessor<'a, G: GitHubOperations> {
    config: CommentCommandConfig,
    github: &'a G,
}

impl<'a, G: GitHubOperations + Send + Sync> CommentCommandProcessor<'a, G> {
    /// Create a new processor.
    ///
    /// # Parameters
    /// - `config`: Processor configuration including `allow_override` flag and
    ///   orchestrator settings used when a `!set-version` command succeeds.
    /// - `github`: Borrowed reference to the `GitHubOperations` implementation.
    pub fn new(config: CommentCommandConfig, github: &'a G) -> Self {
        Self { config, github }
    }

    /// Retry a GitHub operation using this processor's configured
    /// [`ErrorHandlingConfig`] retry policy.
    ///
    /// Thin wrapper around [`retry_with_backoff`] that captures
    /// `self.config.orchestrator_config.error_handling` so call sites only
    /// need to supply the operation name, correlation ID, and the operation
    /// itself.
    async fn retry<T, F, Fut>(
        &self,
        operation_name: &str,
        correlation_id: Option<&str>,
        f: F,
    ) -> CoreResult<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = CoreResult<T>>,
    {
        retry_with_backoff(
            &self.config.orchestrator_config.error_handling,
            operation_name,
            correlation_id,
            f,
        )
        .await
    }

    /// Process a [`ProcessingEvent`] of type
    /// [`EventType::PullRequestCommentReceived`].
    ///
    /// Returns `Ok(())` for all non-action-producing cases—unknown command,
    /// disabled override, closed PR, or validation rejections—so the event loop
    /// acknowledges rather than retries.  Only GitHub API failures propagate
    /// as errors.
    ///
    /// # Errors
    ///
    /// - [`CoreError::GitHub`] / [`CoreError::Network`] — a GitHub API call
    ///   failed; propagated so the event loop can retry if transient.
    pub async fn process(&self, event: &ProcessingEvent) -> CoreResult<()> {
        let span = tracing::info_span!(
            "comment_command_processor.process",
            event_id = %event.event_id,
            correlation_id = %event.correlation_id,
            owner = %event.repository.owner,
            repo = %event.repository.name,
        );
        self.process_inner(event).instrument(span).await
    }

    async fn process_inner(&self, event: &ProcessingEvent) -> CoreResult<()> {
        if !self.config.allow_override {
            debug!(
                event_id = %event.event_id,
                "allow_override is false — silently ignoring comment event"
            );
            return Ok(());
        }

        let owner = &event.repository.owner;
        let repo = &event.repository.name;

        // Extract PR open/closed state from the webhook payload.
        // Comments on non-open PRs are silently ignored.
        let pr_state = event
            .payload
            .get("issue")
            .and_then(|i| i.get("state"))
            .and_then(|s| s.as_str())
            .unwrap_or("unknown");

        if pr_state != "open" {
            debug!(
                event_id = %event.event_id,
                pr_state,
                "Comment is on a non-open PR — ignoring"
            );
            return Ok(());
        }

        let Some((issue_number, comment_body, commenter_login)) =
            Self::extract_comment_fields(&event.payload)
        else {
            debug!(
                event_id = %event.event_id,
                "payload missing required comment fields (issue.number, comment.body, or \
                 comment.user.login) — ignoring"
            );
            return Ok(());
        };

        // Parse the command first: if there is no recognised command in the
        // body there is nothing to do regardless of who posted it.  This
        // prevents spurious rejection replies to ordinary PR comments and
        // breaks the feedback loop caused by Release Regent responding to
        // its own posted comments.
        let command = parse_comment_command(&comment_body);
        debug!(
            event_id = %event.event_id,
            ?command,
            commenter_login,
            "Parsed comment command"
        );
        if command == CommentCommand::Unknown {
            return Ok(());
        }

        // Ignore comments from bot accounts — including our own — to prevent
        // feedback loops where a posted reply triggers another webhook that
        // we then process again.
        if commenter_login.ends_with("[bot]") {
            debug!(
                event_id = %event.event_id,
                commenter_login,
                "Ignoring command from bot account"
            );
            return Ok(());
        }

        // Only collaborators with write access or above may issue commands.
        let permission = self
            .retry(
                "get_collaborator_permission",
                Some(event.correlation_id.as_str()),
                || {
                    self.github
                        .get_collaborator_permission(owner, repo, &commenter_login)
                },
            )
            .await?;
        if !permission.can_issue_commands() {
            warn!(
                event_id = %event.event_id,
                commenter_login,
                ?permission,
                "Command rejected: commenter has insufficient permission"
            );
            let rejection = format!(
                "❌ **Release Regent**: @{commenter_login} — only collaborators with \
                 write access (or above) may use Release Regent commands."
            );
            return self
                .post_comment(owner, repo, issue_number, &rejection)
                .await;
        }

        match command {
            // Unknown is handled above with an early return; this arm is
            // unreachable but kept to satisfy exhaustiveness.
            CommentCommand::Unknown => Ok(()),
            CommentCommand::ReleaseBump(kind) => {
                self.handle_release_bump(owner, repo, issue_number, &kind, &event.correlation_id)
                    .await
            }
            CommentCommand::SetVersion(pinned_version) => {
                self.handle_set_version(
                    owner,
                    repo,
                    issue_number,
                    &pinned_version,
                    &event.correlation_id,
                )
                .await
            }
        }
    }

    /// Extract the fields needed to process a comment command from a webhook payload.
    ///
    /// Returns `Some((issue_number, comment_body, commenter_login))` when all
    /// required fields are present, or `None` if any are missing.
    fn extract_comment_fields(payload: &serde_json::Value) -> Option<(u64, String, String)> {
        let issue_number = payload
            .get("issue")
            .and_then(|i| i.get("number"))
            .and_then(serde_json::Value::as_u64)?;
        let comment_body = payload
            .get("comment")
            .and_then(|c| c.get("body"))
            .and_then(serde_json::Value::as_str)?
            .to_string();
        let commenter_login = payload
            .get("comment")
            .and_then(|c| c.get("user"))
            .and_then(|u| u.get("login"))
            .and_then(serde_json::Value::as_str)?
            .to_string();
        Some((issue_number, comment_body, commenter_login))
    }

    // ── Private helpers ────────────────────────────────────────────────────

    /// Handle a validated `!release major|minor|patch` command.
    ///
    /// Removes any existing `rr:override-*` labels from the commented-upon PR
    /// (idempotent), applies the new override label, then posts a confirmation
    /// comment explaining the effect.  All errors from label operations are
    /// logged as warnings rather than propagated so that a transient GitHub API
    /// failure does not cause the event to be retried indefinitely.
    async fn handle_release_bump(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        kind: &BumpKind,
        correlation_id: &str,
    ) -> CoreResult<()> {
        // Determine which label corresponds to this bump kind.
        let new_label = match kind {
            BumpKind::Major => OVERRIDE_LABEL_MAJOR,
            BumpKind::Minor => OVERRIDE_LABEL_MINOR,
            BumpKind::Patch => OVERRIDE_LABEL_PATCH,
        };

        // Read current labels to detect a replacement (used in the confirmation
        // message).  Propagate errors so transient API failures cause a retry.
        let current_labels = self
            .retry("list_pr_labels", Some(correlation_id), || {
                self.github.list_pr_labels(owner, repo, pr_number)
            })
            .await?;

        let replaced_kind: Option<&str> = ALL_OVERRIDE_LABELS
            .iter()
            .find(|&&l| current_labels.iter().any(|cl| cl.name == l))
            .copied();

        // Remove all existing override labels (idempotent; 404 → Ok).
        for label in ALL_OVERRIDE_LABELS {
            if let Err(e) = self
                .retry("remove_label", Some(correlation_id), || {
                    self.github.remove_label(owner, repo, pr_number, label)
                })
                .await
            {
                warn!(
                    pr_number,
                    label,
                    error = %e,
                    correlation_id,
                    "Failed to remove existing override label; continuing"
                );
            }
        }

        // Apply the new override label.
        let new_labels = [new_label];
        self.retry("add_labels", Some(correlation_id), || {
            self.github.add_labels(owner, repo, pr_number, &new_labels)
        })
        .await?;

        info!(
            pr_number,
            label = new_label,
            correlation_id,
            "!release override label applied"
        );

        // Post a confirmation comment.
        let kind_str = kind.as_str();
        let body = if let Some(old_label) = replaced_kind {
            let old_kind = old_label.strip_prefix("rr:override-").unwrap_or(old_label);
            if old_label == new_label {
                // Same label re-applied: confirm idempotent re-recording.
                format!(
                    "✅ **Release Regent**: `!release {kind_str}` override re-recorded \
                     (the existing `!release {kind_str}` override is unchanged). When this PR \
                     is merged, the next release version will be bumped by at least one \
                     {kind_str} increment."
                )
            } else {
                // Different label: replacing a previous override.
                format!(
                    "✅ **Release Regent**: `!release {kind_str}` override recorded \
                     (replacing previous `!release {old_kind}` override). When this PR \
                     is merged, the next release version will be bumped by at least one \
                     {kind_str} increment."
                )
            }
        } else {
            format!(
                "✅ **Release Regent**: `!release {kind_str}` override recorded. \
                 When this PR is merged, the next release version will be bumped by \
                 at least one {kind_str} increment."
            )
        };

        self.post_comment(owner, repo, pr_number, &body).await
    }

    /// Handle a validated `!set-version X.Y.Z` command.
    ///
    /// Validates the pinned version against the current released version, then
    /// invokes the [`ReleaseOrchestrator`] with the pinned version.  Validation
    /// failures post a rejection comment and return `Ok(())` so the event is
    /// acknowledged (not retried).
    async fn handle_set_version(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        pinned_version: &SemanticVersion,
        correlation_id: &str,
    ) -> CoreResult<()> {
        // Guard: only accept !set-version on the release PR (head branch release/v*).
        let pr = self
            .retry("get_pull_request", Some(correlation_id), || {
                self.github.get_pull_request(owner, repo, pr_number)
            })
            .await?;
        let branch_prefix = &self.config.orchestrator_config.branch_prefix;
        let release_head_prefix = format!("{branch_prefix}/v");
        if !pr.head.ref_name.starts_with(&release_head_prefix) {
            let rejection = format!(
                "⚠️ **Release Regent**: `!set-version` must be posted on the active \
                 release PR (branch `{branch_prefix}/v*`). Please re-post this \
                 command on the release PR."
            );
            warn!(
                pr_number,
                head_branch = %pr.head.ref_name,
                correlation_id,
                "!set-version rejected: not on release PR"
            );
            return self.post_comment(owner, repo, pr_number, &rejection).await;
        }

        // Resolve the currently released version from Git tags.
        let current_version = resolve_current_version(self.github, owner, repo, false).await?;

        // Validate: pinned must be strictly > current released version.
        if let Some(ref current) = current_version {
            if pinned_version.compare_precedence(current) != Ordering::Greater {
                let rejection = format!(
                    "❌ **Release Regent**: `!set-version {pinned_version}` was rejected — \
                     the specified version must be strictly greater than the current \
                     released version `{current}`."
                );
                warn!(
                    pr_number,
                    pinned = %pinned_version,
                    current = %current,
                    "Rejecting !set-version: not greater than current released version"
                );
                return self.post_comment(owner, repo, pr_number, &rejection).await;
            }
        }

        // Validate: minimum accepted version is 0.0.1 (rejects 0.0.0).
        let minimum = SemanticVersion {
            major: 0,
            minor: 0,
            patch: 1,
            prerelease: None,
            build: None,
        };
        if pinned_version.compare_precedence(&minimum) == Ordering::Less {
            let rejection = format!(
                "❌ **Release Regent**: `!set-version {pinned_version}` was rejected — \
                 the minimum allowed version is `0.0.1`."
            );
            warn!(
                pr_number,
                pinned = %pinned_version,
                "Rejecting !set-version: version is below minimum (0.0.1)"
            );
            return self.post_comment(owner, repo, pr_number, &rejection).await;
        }

        // Guard D: reject a pinned version that falls below an outstanding
        // `rr:override-*` bump-floor still recorded on a merged PR in the
        // current release cycle (task #211). The scan covers commits from
        // the last release tag up to this release PR's base SHA. When there
        // is no prior release tag (the very first release cycle) there is
        // nothing to scan "since the last release", so the scan is skipped
        // entirely (Fix B — code review follow-up) rather than passing an
        // empty `base` to `resolve_outstanding_override_floor`, which would
        // build an invalid comparison range against the real GitHub API.
        let outstanding_floor = if let Some(ref current) = current_version {
            let override_floor_base = format!(
                "{}{current}",
                self.config.orchestrator_config.version_prefix
            );
            resolve_outstanding_override_floor(
                self.github,
                owner,
                repo,
                &override_floor_base,
                &pr.base.sha,
            )
            .await?
        } else {
            None
        };
        if let Some((floor_kind, floor_pr_number)) = outstanding_floor {
            let floor_baseline = current_version.clone().unwrap_or(SemanticVersion {
                major: 0,
                minor: 0,
                patch: 0,
                prerelease: None,
                build: None,
            });
            let floor_version = apply_bump_floor(&floor_baseline, pinned_version, &floor_kind);
            if floor_version.compare_precedence(pinned_version) != Ordering::Equal {
                let kind_str = floor_kind.as_str();
                let rejection = format!(
                    "❌ **Release Regent**: `!set-version {pinned_version}` was rejected — an \
                     `rr:override-{kind_str}` label is outstanding on PR #{floor_pr_number}, \
                     which requires the release version to be at least `{floor_version}`. Use \
                     `!set-version {floor_version}` or higher, or remove the `!release \
                     {kind_str}` override first."
                );
                warn!(
                    pr_number,
                    pinned = %pinned_version,
                    floor = %floor_version,
                    floor_pr = floor_pr_number,
                    correlation_id,
                    "Rejecting !set-version: below outstanding override floor"
                );
                return self.post_comment(owner, repo, pr_number, &rejection).await;
            }
        }

        info!(
            pr_number,
            pinned = %pinned_version,
            "!set-version accepted — invoking release orchestrator"
        );

        let base_branch = pr.base.ref_name.clone();
        let base_sha = pr.base.sha.clone();

        // Extract the existing changelog from the PR body that is already in hand.
        // This avoids a second search_pull_requests call and ensures we read from
        // exactly the same PR that handle_set_version is operating on, which is
        // consistent with the orchestrator's PR-selection logic.
        let existing_changelog = crate::release_orchestrator::extract_changelog_from_pr_body(
            pr.body.as_deref().unwrap_or(""),
            &self.config.orchestrator_config.changelog_header,
        );

        let orchestrator =
            ReleaseOrchestrator::new(self.config.orchestrator_config.clone(), self.github);

        let orch_result = orchestrator
            .orchestrate(
                owner,
                repo,
                pinned_version,
                &existing_changelog,
                &base_branch,
                &base_sha,
                correlation_id,
            )
            .await?;

        let confirmation = Self::format_set_version_confirmation(
            pinned_version,
            &orch_result,
            &self.config.orchestrator_config.branch_prefix,
        );
        self.post_comment(owner, repo, pr_number, &confirmation)
            .await
    }

    /// Format the confirmation comment posted after a successful `!set-version` run.
    ///
    /// The wording varies by orchestration outcome:
    /// - `Created` / `Renamed` — a new or renamed release PR is now at the
    ///   pinned version.
    /// - `Updated` — the existing release PR already had the same version;
    ///   no version change was needed.
    /// - `NoOp` — the existing release PR already has a *higher* version;
    ///   the command was superseded.
    fn format_set_version_confirmation(
        pinned_version: &SemanticVersion,
        result: &crate::release_orchestrator::OrchestratorResult,
        branch_prefix: &str,
    ) -> String {
        use crate::release_orchestrator::OrchestratorResult;
        match result {
            OrchestratorResult::Created { .. } => format!(
                "✅ **Release Regent**: Release version pinned to `{pinned_version}`. \
                 A new release PR has been created."
            ),
            OrchestratorResult::Renamed { .. } => format!(
                "✅ **Release Regent**: Release version pinned to `{pinned_version}`. \
                 The release PR has been updated to this version."
            ),
            OrchestratorResult::Updated { .. } => format!(
                "✅ **Release Regent**: Release version `{pinned_version}` is already \
                 the active release PR version. No changes were needed."
            ),
            OrchestratorResult::NoOp { pr } => {
                // Strip the release branch prefix (e.g. "release/v") to show
                // a clean version number like "2.0.0" rather than the full
                // branch name like "release/v2.0.0".
                let release_v_prefix = format!("{branch_prefix}/v");
                let version_display = pr
                    .head
                    .ref_name
                    .strip_prefix(&release_v_prefix)
                    .unwrap_or(&pr.head.ref_name);
                format!(
                    "⚠️ **Release Regent**: `!set-version {pinned_version}` was not applied \
                     — the existing release PR is already at a higher version \
                     (`{version_display}`). To override, close the existing release PR first."
                )
            }
            // NoBumpNeeded is never returned by the orchestrator when driven
            // from a `!set-version` command: the caller always supplies an
            // explicit pinned version, so the orchestrator always proceeds past
            // the no-bump guard.  This arm exists only for exhaustive matching.
            OrchestratorResult::NoBumpNeeded => unreachable!(
                "NoBumpNeeded cannot be returned by orchestrate() from a \
                 !set-version command path"
            ),
            // TaggedRelease is produced by the release-PR merge path and is
            // never returned by `ReleaseOrchestrator::orchestrate()`.
            OrchestratorResult::TaggedRelease => unreachable!(
                "TaggedRelease cannot be returned by orchestrate() from a \
                 !set-version command path"
            ),
        }
    }

    /// Post a comment on the PR as a best-effort operation.
    ///
    /// If the GitHub API call fails the error is logged as a warning and
    /// `Ok(())` is returned, so the event is acknowledged rather than retried.
    async fn post_comment(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        body: &str,
    ) -> CoreResult<()> {
        if let Err(e) = self
            .retry("create_issue_comment", None, || {
                self.github
                    .create_issue_comment(owner, repo, pr_number, body)
            })
            .await
        {
            warn!(
                pr_number,
                error = %e,
                "Failed to post comment on PR; event will still be acknowledged"
            );
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// resolve_outstanding_override_floor — free function (task #211)
// ─────────────────────────────────────────────────────────────────────────────

/// Scan commits in the range `base..head` for an outstanding `rr:override-*`
/// bump-floor label on any pull request associated with those commits.
///
/// For each commit returned by [`crate::traits::GitOperations::get_commits_between`],
/// every pull request associated with it (via
/// [`GitHubOperations::list_pull_requests_for_commit`]) is inspected for
/// `rr:override-*` labels (via [`GitHubOperations::list_pr_labels`]). The
/// highest-severity [`BumpKind`] found across all commits/PRs is returned
/// together with the PR number it originated from.
///
/// Returns `Ok(None)` when no commits are in range, no commit resolves to an
/// associated PR, or no associated PR carries an `rr:override-*` label.
///
/// When more than one PR carries the highest-severity override (a tie), the
/// PR returned is whichever is encountered first during iteration — this is
/// an implementation detail, not a documented guarantee, since callers only
/// care about the floor value.
///
/// # Errors
///
/// Propagates any [`crate::CoreError`] returned by `get_commits_between`,
/// `list_pull_requests_for_commit`, or `list_pr_labels` — a transient GitHub
/// API failure must never be silently treated as "no floor outstanding".
// CoreError is intentionally large; this is the established pattern throughout the codebase.
#[allow(clippy::result_large_err)]
pub(crate) async fn resolve_outstanding_override_floor<G>(
    github: &G,
    owner: &str,
    repo: &str,
    base: &str,
    head: &str,
) -> CoreResult<Option<(BumpKind, u64)>>
where
    G: GitHubOperations + Send + Sync,
{
    let commits = github
        .get_commits_between(owner, repo, base, head, GetCommitsOptions::default())
        .await?;

    // Every `(BumpKind, pr_number)` found during the scan, in encounter
    // order. Collected rather than reduced inline so that
    // `versioning::highest_bump_kind` (a plain `Iterator::max`) can be reused
    // to pick the severity, while a separate lookup finds the
    // first-encountered PR at that severity — preserving the tie-break
    // documented above without a hand-rolled fold.
    let mut found: Vec<(BumpKind, u64)> = Vec::new();

    // Tracks which PR numbers have already had `list_pr_labels` called, so a
    // PR spanning multiple commits in the scanned range only triggers one
    // lookup (Fix C — code review follow-up on task #211).
    let mut seen_pr_numbers: std::collections::HashSet<u64> = std::collections::HashSet::new();

    for commit in &commits {
        let prs = github
            .list_pull_requests_for_commit(owner, repo, &commit.sha)
            .await?;

        // Only merged pull requests may contribute to the override floor — an
        // open or closed-but-unmerged PR's label must never be able to
        // inflate it (Fix A — code review follow-up on task #211).
        // `GET /repos/{owner}/{repo}/commits/{sha}/pulls` returns any PR
        // associated with the commit, not just merged ones.
        for pr in prs.iter().filter(|pr| pr.merged_at.is_some()) {
            if !seen_pr_numbers.insert(pr.number) {
                continue;
            }
            let labels = github.list_pr_labels(owner, repo, pr.number).await?;
            if let Some(kind) = labels.iter().find_map(|l| label_name_to_bump_kind(&l.name)) {
                found.push((kind, pr.number));
            }
        }
    }

    let highest = highest_bump_kind(found.iter().map(|(kind, _)| kind.clone()));
    let best = highest.and_then(|highest_kind| {
        found
            .iter()
            .find(|(kind, _)| *kind == highest_kind)
            .cloned()
    });

    Ok(best)
}

// ─────────────────────────────────────────────────────────────────────────────
// collect_merged_prs_with_override_labels — free function (task #211 Fix D)
// ─────────────────────────────────────────────────────────────────────────────

/// Scan commits in the range `base..head` for merged pull requests that carry
/// one or more `rr:override-*` labels.
///
/// Used by the release-ship-time cleanup path (Fix D — code review follow-up
/// on task #211): once a release has shipped, every `rr:override-*` label on
/// a pull request whose commits are included in that release must be
/// removed, since the floor those labels represent has already been baked
/// into the released version. This differs from
/// [`resolve_outstanding_override_floor`], which only returns the single
/// highest-severity override — the cleanup path needs *every* override label
/// on *every* qualifying PR so each one can be removed.
///
/// Reuses the same merged-PR-only filter (Fix A) and per-PR-number
/// deduplication (Fix C) as `resolve_outstanding_override_floor`, applied to
/// the same `get_commits_between` → `list_pull_requests_for_commit` →
/// `list_pr_labels` walk.
///
/// Returns `(pr_number, override_label_names)` pairs, one per qualifying PR,
/// in first-encountered order. PRs with no `rr:override-*` label are omitted.
///
/// # Errors
///
/// Propagates any [`crate::CoreError`] returned by `get_commits_between`,
/// `list_pull_requests_for_commit`, or `list_pr_labels`.
#[allow(clippy::result_large_err)]
pub(crate) async fn collect_merged_prs_with_override_labels<G>(
    github: &G,
    owner: &str,
    repo: &str,
    base: &str,
    head: &str,
) -> CoreResult<Vec<(u64, Vec<String>)>>
where
    G: GitHubOperations + Send + Sync,
{
    let commits = github
        .get_commits_between(owner, repo, base, head, GetCommitsOptions::default())
        .await?;

    let mut found: Vec<(u64, Vec<String>)> = Vec::new();
    let mut seen_pr_numbers: std::collections::HashSet<u64> = std::collections::HashSet::new();

    for commit in &commits {
        let prs = github
            .list_pull_requests_for_commit(owner, repo, &commit.sha)
            .await?;

        // Only merged pull requests qualify (Fix A) — see
        // `resolve_outstanding_override_floor` for rationale.
        for pr in prs.iter().filter(|pr| pr.merged_at.is_some()) {
            if !seen_pr_numbers.insert(pr.number) {
                continue;
            }
            let labels = github.list_pr_labels(owner, repo, pr.number).await?;
            let override_names: Vec<String> = labels
                .iter()
                .filter(|l| ALL_OVERRIDE_LABELS.contains(&l.name.as_str()))
                .map(|l| l.name.clone())
                .collect();
            if !override_names.is_empty() {
                found.push((pr.number, override_names));
            }
        }
    }

    Ok(found)
}

// ─────────────────────────────────────────────────────────────────────────────
// parse_comment_command — free function, public for unit testing
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a pull request comment body into a [`CommentCommand`].
///
/// Scanning is line-by-line and case-insensitive.  The **first** line that
/// matches a known command pattern wins; remaining lines are ignored.
///
/// A `!set-version` line with an unparseable semver string is skipped rather
/// than returning an error — the line is treated as unrecognised so the event
/// is acknowledged without side effects.
///
/// # Examples
///
/// ```
/// use release_regent_core::comment_command_processor::{
///     parse_comment_command, BumpKind, CommentCommand,
/// };
/// use release_regent_core::versioning::SemanticVersion;
///
/// let cmd = parse_comment_command("!set-version 2.3.0");
/// assert_eq!(
///     cmd,
///     CommentCommand::SetVersion(SemanticVersion {
///         major: 2, minor: 3, patch: 0,
///         prerelease: None, build: None
///     })
/// );
///
/// let bump = parse_comment_command("!release minor");
/// assert_eq!(bump, CommentCommand::ReleaseBump(BumpKind::Minor));
///
/// let unknown = parse_comment_command("just a regular comment");
/// assert_eq!(unknown, CommentCommand::Unknown);
/// ```
#[must_use]
pub fn parse_comment_command(body: &str) -> CommentCommand {
    for line in body.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();

        if lower.starts_with("!set-version") {
            // Slice the version argument from the *original* trimmed line so that
            // pre-release/build identifiers (e.g. "2.0.0-RC.1") are not lowercased.
            let version_str = trimmed["!set-version".len()..].trim();
            // Strip an optional leading "v" that developers sometimes include.
            let version_str = version_str.strip_prefix('v').unwrap_or(version_str);
            if let Ok(v) = VersionCalculator::parse_version(version_str) {
                return CommentCommand::SetVersion(v);
            }
            // Malformed version string — skip this line and keep scanning.
            continue;
        }

        if lower.starts_with("!release") {
            // Extract the bump dimension from the original trimmed line and
            // lowercase only that part for case-insensitive matching.
            let rest = trimmed["!release".len()..].trim().to_lowercase();
            return match rest.as_str() {
                "major" => CommentCommand::ReleaseBump(BumpKind::Major),
                "minor" => CommentCommand::ReleaseBump(BumpKind::Minor),
                "patch" => CommentCommand::ReleaseBump(BumpKind::Patch),
                _ => CommentCommand::Unknown,
            };
        }
    }

    CommentCommand::Unknown
}

#[cfg(test)]
#[path = "comment_command_processor_tests.rs"]
mod tests;
