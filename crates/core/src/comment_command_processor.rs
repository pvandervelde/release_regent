//! Comment command processor for Release Regent
//!
//! This module implements the [`CommentCommandProcessor`], which handles
//! [`EventType::PullRequestCommentReceived`] events and extracts recognised
//! command patterns from PR comment bodies.
//!
//! ## Recognised Commands
//!
//! | Command                       | Condition               | Effect                                        |
//! |-------------------------------|-------------------------|-----------------------------------------------|
//! | `!set-version X.Y.Z`          | version > current tag   | Invokes `ReleaseOrchestrator` with pinned ver |
//! | `!set-version X.Y.Z`          | version ≤ current tag   | Posts rejection comment; acknowledges event   |
//! | `!release major/minor/patch`  | always (stub)           | Returns `CoreError::NotSupported`             |
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

use tracing::{debug, info, warn};

use crate::{
    release_orchestrator::{OrchestratorConfig, ReleaseOrchestrator},
    traits::{event_source::ProcessingEvent, github_operations::GitHubOperations},
    versioning::{resolve_current_version, SemanticVersion, VersionCalculator},
    CoreError, CoreResult,
};

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// Which conventional-commit bump dimension the user is requesting via
/// an `!release` override command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BumpKind {
    /// Force at least a major version bump.
    Major,
    /// Force at least a minor version bump.
    Minor,
    /// Force at least a patch version bump.
    Patch,
}

/// A command parsed from a pull request comment body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentCommand {
    /// `!set-version X.Y.Z` — pin the next release to exactly this version.
    SetVersion(SemanticVersion),
    /// `!release major|minor|patch` — override the minimum bump dimension.
    ///
    /// **Note**: this variant is recognised by the parser but the processor
    /// returns [`CoreError::NotSupported`] until the bump-override design is
    /// completed.
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

    /// Process a [`ProcessingEvent`] of type
    /// [`EventType::PullRequestCommentReceived`].
    ///
    /// Returns `Ok(())` for all non-action-producing cases—unknown command,
    /// disabled override, closed PR, or validation rejections—so the event loop
    /// acknowledges rather than retries.  Only GitHub API failures and the
    /// `!release` stub propagate as errors.
    ///
    /// # Errors
    ///
    /// - [`CoreError::NotSupported`] — `!release` bump override is a stub
    ///   (not yet implemented).  The event loop treats this as a permanent
    ///   failure and rejects the event without retrying.
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
        let _enter = span.enter();

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

        let Some(issue_number) = event
            .payload
            .get("issue")
            .and_then(|i| i.get("number"))
            .and_then(serde_json::Value::as_u64)
        else {
            debug!(
                event_id = %event.event_id,
                "payload missing issue.number — ignoring"
            );
            return Ok(());
        };

        let Some(comment_body_str) = event
            .payload
            .get("comment")
            .and_then(|c| c.get("body"))
            .and_then(serde_json::Value::as_str)
        else {
            debug!(
                event_id = %event.event_id,
                "payload missing comment.body — ignoring"
            );
            return Ok(());
        };
        let comment_body = comment_body_str.to_string();

        let command = parse_comment_command(&comment_body);
        debug!(
            event_id = %event.event_id,
            ?command,
            "Parsed comment command"
        );

        match command {
            CommentCommand::Unknown => Ok(()),
            CommentCommand::ReleaseBump(_kind) => Err(CoreError::not_supported(
                "!release bump override",
                "!release major/minor/patch is not yet fully implemented; \
                 the bump-override design (label persistence, precedence rules) \
                 must be completed before this command can be processed",
            )),
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

    // ── Private helpers ────────────────────────────────────────────────────

    /// Handle a validated `!set-version X.Y.Z` command.
    ///
    /// Validates the pinned version against the current released version, then
    /// invokes the [`ReleaseOrchestrator`] with the pinned version.  Validation
    /// failures post a rejection comment and return `Ok(())`.
    async fn handle_set_version(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        pinned_version: &SemanticVersion,
        correlation_id: &str,
    ) -> CoreResult<()> {
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
                return self
                    .post_rejection_comment(owner, repo, pr_number, &rejection)
                    .await;
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
            return self
                .post_rejection_comment(owner, repo, pr_number, &rejection)
                .await;
        }

        info!(
            pr_number,
            pinned = %pinned_version,
            "!set-version accepted — invoking release orchestrator"
        );

        // Retrieve the PR to extract the base branch name and SHA.
        let pr = self.github.get_pull_request(owner, repo, pr_number).await?;
        let base_branch = pr.base.ref_name.clone();
        let base_sha = pr.base.sha.clone();

        let orchestrator =
            ReleaseOrchestrator::new(self.config.orchestrator_config.clone(), self.github);

        orchestrator
            .orchestrate(
                owner,
                repo,
                pinned_version,
                "", // no new changelog entries for a direct version pin
                &base_branch,
                &base_sha,
                correlation_id,
            )
            .await
            .map(|_| ())
    }

    /// Post a rejection comment on the PR and return `Ok(())` so the event is
    /// acknowledged (not retried).
    async fn post_rejection_comment(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        body: &str,
    ) -> CoreResult<()> {
        self.github
            .create_issue_comment(owner, repo, pr_number, body)
            .await
    }
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

        if let Some(rest) = lower.strip_prefix("!set-version") {
            let version_str = rest.trim();
            // Strip an optional leading "v" that developers sometimes include.
            let version_str = version_str.strip_prefix('v').unwrap_or(version_str);
            if let Ok(v) = VersionCalculator::parse_version(version_str) {
                return CommentCommand::SetVersion(v);
            }
            // Malformed version string — skip this line and keep scanning.
            continue;
        }

        if let Some(rest) = lower.strip_prefix("!release") {
            return match rest.trim() {
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
