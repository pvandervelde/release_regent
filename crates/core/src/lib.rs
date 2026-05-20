//! # Release Regent Core
//!
//! This crate contains the core business logic and architecture for Release Regent, providing
//! automated release management through webhook-driven workflows.
//!
//! ## Architecture Overview
//!
//! Release Regent follows a modular, trait-based architecture that enables comprehensive testing
//! and flexible deployment strategies:
//!
//! ```text
//! ┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
//! │   Webhook       │    │  Configuration   │    │   GitHub API    │
//! │   Processing    │────│  Management      │────│   Operations    │
//! └─────────────────┘    └──────────────────┘    └─────────────────┘
//!          │                       │                       │
//!          └───────────────────────┼───────────────────────┘
//!                                  │
//!                        ┌─────────▼─────────┐
//!                        │ Release Regent    │
//!                        │ Processor         │
//!                        └─────────┬─────────┘
//!                                  │
//!          ┌───────────────────────┼───────────────────────┐
//!          │                       │                       │
//! ┌────────▼────────┐    ┌─────────▼─────────┐    ┌────────▼────────┐
//! │   Versioning    │    │    Changelog      │    │   Validation    │
//! │   Calculation   │    │   Generation      │    │   & Errors      │
//! └─────────────────┘    └───────────────────┘    └─────────────────┘
//! ```
//!
//! ## Core Components
//!
//! ### 1. **Dependency Injection Architecture**
//!
//! The [`ReleaseRegentProcessor`] uses dependency injection for all external services,
//! enabling comprehensive testing and flexible deployment:
//!
//! ```rust,ignore
//! use release_regent_core::ReleaseRegentProcessor;
//! use release_regent_testing::{MockGitHubOperations, MockConfigurationProvider,
//!                             MockVersionCalculator};
//!
//! let processor = ReleaseRegentProcessor::new(
//!     MockGitHubOperations::new(),      // GitHub API operations
//!     MockConfigurationProvider::new(), // Configuration loading
//!     MockVersionCalculator::new(),     // Version calculation
//! );
//! ```
//!
//! ### 2. **Event Processing Pipeline**
//!
//! Events are delivered via the [`traits::event_source::EventSource`] trait and
//! processed by [`run_event_loop`]:
//!
//! - **Event Dispatch**: Route to the appropriate handler based on [`traits::event_source::EventType`]
//! - **Acknowledgement**: Mark successfully processed events as done
//! - **Rejection**: Reject permanently failed events without crashing the loop
//!
//! ### 3. **Version Calculation Engine**
//!
//! The [`versioning`] module provides semantic version calculation:
//!
//! - **Conventional Commits**: Parse commit messages following conventional commit spec
//! - **Semantic Versioning**: Full semver 2.0.0 compliance with validation
//! - **Version Strategies**: Multiple approaches to version calculation
//!
//! ### 4. **Configuration Management**
//!
//! The [`config`] module handles repository-specific and global configuration:
//!
//! - **Repository Settings**: Per-repo versioning and release configuration
//! - **Template Support**: Configurable PR titles, bodies, and branch naming
//! - **Validation**: Schema validation and environment-specific overrides
//!
//! ### 5. **Changelog Generation**
//!
//! The [`changelog`] module creates structured release notes:
//!
//! - **Commit Grouping**: Organize commits by type (features, fixes, etc.)
//! - **Template Rendering**: Customizable changelog formats
//! - **Metadata Integration**: Include issue numbers, authors, and breaking changes
//!
//! ## Workflow Orchestration
//!
//! ### Pull Request Merge Processing
//!
//! When a regular pull request is merged:
//!
//! 1. **Webhook Receipt**: Validate and parse GitHub webhook payload
//! 2. **Configuration Loading**: Load repository-specific settings
//! 3. **Commit Analysis**: Fetch and parse commits since last release
//! 4. **Version Calculation**: Determine next semantic version using conventional commits
//! 5. **Release PR Management**: Create or update release pull request
//! 6. **Changelog Generation**: Generate release notes from commit history
//!
//! ### Release PR Merge Processing
//!
//! When a release pull request is merged:
//!
//! 1. **Release Detection**: Identify merged release PR by branch pattern
//! 2. **Version Extraction**: Parse version from PR branch or title
//! 3. **GitHub Release Creation**: Create tag and GitHub release
//! 4. **Branch Cleanup**: Remove release branch after successful release
//!
//! ## Error Handling Strategy
//!
//! The crate uses a comprehensive error handling approach:
//!
//! - **Typed Errors**: [`CoreError`] enum covers all failure modes
//! - **Error Context**: Rich error messages with correlation IDs
//! - **Graceful Degradation**: Continue processing when possible
//! - **Retry Logic**: Exponential backoff for transient failures
//!
//! ## Testing Architecture
//!
//! Release Regent supports multiple testing levels:
//!
//! - **Unit Tests**: Individual component testing with mocks
//! - **Integration Tests**: End-to-end workflow testing
//! - **Contract Tests**: API integration validation
//! - **Behavioral Tests**: Specification compliance verification
//!
//! ## Usage Examples
//!
//! ### Basic Processor Setup
//!
//! ```rust,ignore
//! use release_regent_core::ReleaseRegentProcessor;
//! use release_regent_github_client::GitHubClient;
//! use release_regent_config_provider::FileConfigurationProvider;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let github_client = GitHubClient::new("github_token".to_string())?;
//!     let config_provider = FileConfigurationProvider::new("./config")?;
//!     let version_calculator = MyVersionCalculator::new();
//!
//!     let processor = ReleaseRegentProcessor::new(
//!         github_client,
//!         config_provider,
//!         version_calculator,
//!     );
//!
//!     Ok(())
//! }
//! ```
//!
//! ### Version Calculation
//!
//! ```rust
//! use release_regent_core::versioning::{VersionCalculator, SemanticVersion};
//!
//! let current_version = SemanticVersion {
//!     major: 1, minor: 0, patch: 0,
//!     prerelease: None, build: None
//! };
//!
//! let calculator = VersionCalculator::new(Some(current_version));
//!
//! // Parse commit messages from repository
//! let commits = vec![
//!     ("abc123".to_string(), "feat: add new user interface".to_string()),
//!     ("def456".to_string(), "fix: resolve authentication bug".to_string()),
//! ];
//!
//! let parsed_commits = VersionCalculator::parse_conventional_commits(&commits);
//! let next_version = calculator.calculate_next_version(&parsed_commits)?;
//!
//! println!("Next version: {}", next_version); // "1.1.0"
//! # Ok::<(), release_regent_core::CoreError>(())
//! ```
//!
//! ### Configuration Loading
//!
//! ```rust,ignore
//! use release_regent_core::{config::ReleaseRegentConfig, traits::ConfigurationProvider};
//!
//! let config = ReleaseRegentConfig::builder()
//!     .repository_owner("myorg")
//!     .repository_name("myrepo")
//!     .default_branch("main")
//!     .build()?;
//!
//! // Configure release PR templates
//! let pr_config = config.release_pr.unwrap_or_default();
//! println!("PR title template: {}", pr_config.title_template);
//! ```
//!
//! ## Performance Characteristics
//!
//! Release Regent is designed for high-throughput webhook processing:
//!
//! - **Async Processing**: Full async/await support with Tokio runtime
//! - **Concurrent Operations**: Parallel GitHub API calls where possible
//! - **Efficient Parsing**: Optimized commit message parsing with caching
//! - **Rate Limit Handling**: Automatic GitHub API rate limit management
//!
//! ## Security Features
//!
//! - **Webhook Validation**: HMAC signature verification for all incoming webhooks
//! - **Token Management**: Secure GitHub App token handling with automatic refresh
//! - **Input Sanitization**: Comprehensive validation of all external inputs
//! - **Audit Logging**: Structured logging with correlation IDs for security monitoring

pub mod changelog;
pub mod comment_command_processor;
pub mod config;
pub(crate) mod default_version_calculator;
pub mod errors;
pub(crate) mod github_version_calculator;
pub mod manifest;
pub(crate) mod pr_status_commenter;
pub mod release_automator;
pub mod release_orchestrator;
pub mod traits;
pub mod versioning;

pub use default_version_calculator::DefaultVersionCalculator;
pub use errors::{CoreError, CoreResult};
pub use github_version_calculator::GitHubVersionCalculator;
pub use manifest::{ManifestFileConfig, ManifestFormat};
pub use traits::{ConfigurationProvider, GitHubOperations, GitOperations, VersionCalculator};

// ─────────────────────────────────────────────────────────────────────────────
// MergedPullRequestHandler — event handler trait
// ─────────────────────────────────────────────────────────────────────────────

/// Handles `PullRequestMerged` events received by the event loop.
///
/// The trait decouples [`run_event_loop`] from the concrete
/// [`ReleaseRegentProcessor`] type so that tests can inject lightweight
/// no-op or spy implementations without wiring up the full dependency graph.
///
/// [`ReleaseRegentProcessor`] provides the production implementation.  For
/// tests and environments where the processor is not yet wired (e.g. the
/// server before GitHub credentials are fully configured), a simple no-op
/// implementation that returns `Ok(())` is sufficient.
#[async_trait::async_trait]
pub trait MergedPullRequestHandler: Send + Sync {
    /// Process a single `PullRequestMerged` event.
    ///
    /// Returns `Ok(())` on success.  Any `Err` returned here is treated as a
    /// processing failure by the event loop: the event will be rejected
    /// (permanently if the error is not retryable) and the loop will continue.
    async fn handle_merged_pull_request(
        &self,
        event: &traits::event_source::ProcessingEvent,
    ) -> CoreResult<()>;

    /// Process a single `ReleasePrMerged` event.
    ///
    /// Called when the event loop receives [`EventType::ReleasePrMerged`].
    /// The default implementation is a no-op that acknowledges the event
    /// without taking any action.  Override in the production processor to
    /// invoke [`release_automator::ReleaseAutomator`].
    async fn handle_release_pr_merged(
        &self,
        _event: &traits::event_source::ProcessingEvent,
    ) -> CoreResult<()> {
        Ok(())
    }

    /// Process a single `PullRequestCommentReceived` event.
    ///
    /// The default implementation is a no-op that acknowledges the event
    /// without taking any action.  Override in the production processor to
    /// invoke [`comment_command_processor::CommentCommandProcessor`].
    async fn handle_pr_comment(
        &self,
        _event: &traits::event_source::ProcessingEvent,
    ) -> CoreResult<()> {
        Ok(())
    }

    /// Process a single `PullRequestOpened` or `PullRequestUpdated` event.
    ///
    /// Called when the event loop receives [`EventType::PullRequestOpened`] or
    /// [`EventType::PullRequestUpdated`].  The default implementation is a
    /// no-op that acknowledges the event without taking any action.  Override
    /// in the production processor to post or refresh the projected-version
    /// status comment on the PR.
    async fn handle_pull_request_activity(
        &self,
        _event: &traits::event_source::ProcessingEvent,
    ) -> CoreResult<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl<G, C, V> MergedPullRequestHandler for ReleaseRegentProcessor<G, C, V>
where
    G: GitHubOperations + Send + Sync,
    C: ConfigurationProvider + Send + Sync,
    V: VersionCalculator + Send + Sync,
{
    async fn handle_merged_pull_request(
        &self,
        event: &traits::event_source::ProcessingEvent,
    ) -> CoreResult<()> {
        match self.handle_merged_pull_request(event).await {
            Ok(result) => {
                tracing::info!(result = ?result, "Release orchestration completed");
                self.try_refresh_feature_pr_status_comments(
                    &event.repository.owner,
                    &event.repository.name,
                    &event.repository.default_branch,
                )
                .await;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn handle_release_pr_merged(
        &self,
        event: &traits::event_source::ProcessingEvent,
    ) -> CoreResult<()> {
        use release_automator::{AutomatorConfig, ReleaseAutomator};
        use traits::configuration_provider::LoadOptions;

        let correlation_id = &event.correlation_id;
        let owner = &event.repository.owner;
        let repo = &event.repository.name;

        let repo_config = self
            .configuration_provider
            .get_merged_config(
                owner,
                repo,
                LoadOptions {
                    installation_id: Some(event.installation_id),
                    default_branch: Some(event.repository.default_branch.clone()),
                    ..Default::default()
                },
            )
            .await?;

        let config = AutomatorConfig {
            branch_prefix: release_orchestrator::OrchestratorConfig::DEFAULT_BRANCH_PREFIX
                .to_string(),
            changelog_header: release_orchestrator::extract_changelog_header(
                &repo_config.release_pr.body_template,
            ),
            version_prefix: repo_config.core.version_prefix.clone(),
            generate_release_notes: repo_config.releases.generate_notes,
        };

        match ReleaseAutomator::new(
            config,
            &self
                .github_operations
                .scoped_to(self.resolve_installation_id(owner, repo).await?),
        )
        .automate(owner, repo, event, correlation_id)
        .await
        {
            Ok(result) => {
                tracing::info!(result = ?result, "Release automation completed");
                // Clear stale rr:override-* labels from open feature PRs now that
                // a new release has been published.
                self.clear_stale_override_labels_after_release(
                    owner,
                    repo,
                    event.installation_id,
                    correlation_id,
                )
                .await;
                self.try_refresh_feature_pr_status_comments(
                    owner,
                    repo,
                    &event.repository.default_branch,
                )
                .await;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn handle_pr_comment(
        &self,
        event: &traits::event_source::ProcessingEvent,
    ) -> CoreResult<()> {
        use comment_command_processor::{CommentCommandConfig, CommentCommandProcessor};
        use traits::configuration_provider::LoadOptions;

        let owner = &event.repository.owner;
        let repo = &event.repository.name;

        let repo_config = self
            .configuration_provider
            .get_merged_config(
                owner,
                repo,
                LoadOptions {
                    installation_id: Some(event.installation_id),
                    default_branch: Some(event.repository.default_branch.clone()),
                    ..Default::default()
                },
            )
            .await?;

        let config = CommentCommandConfig {
            orchestrator_config: release_orchestrator::OrchestratorConfig {
                branch_prefix: release_orchestrator::OrchestratorConfig::DEFAULT_BRANCH_PREFIX
                    .to_string(),
                version_prefix: repo_config.core.version_prefix.clone(),
                title_template: repo_config.release_pr.title_template.clone(),
                changelog_header: release_orchestrator::extract_changelog_header(
                    &repo_config.release_pr.body_template,
                ),
                body_template: repo_config.release_pr.body_template.clone(),
                manifest_files: repo_config.release_pr.manifest_files.clone(),
                auto_detect_manifests: repo_config.release_pr.auto_detect_manifests,
            },
            allow_override: repo_config.versioning.allow_override,
        };

        CommentCommandProcessor::new(
            config,
            &self
                .github_operations
                .scoped_to(self.resolve_installation_id(owner, repo).await?),
        )
        .process(event)
        .await
    }

    async fn handle_pull_request_activity(
        &self,
        event: &traits::event_source::ProcessingEvent,
    ) -> CoreResult<()> {
        use release_automator::is_release_pr_branch;
        use traits::configuration_provider::LoadOptions;
        use traits::version_calculator::{CalculationOptions, VersionContext, VersioningStrategy};

        let owner = &event.repository.owner;
        let repo = &event.repository.name;

        let pr_number = event
            .payload
            .pointer("/pull_request/number")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        if pr_number == 0 {
            tracing::warn!(
                event_id = %event.event_id,
                "PR number missing from payload; skipping status comment"
            );
            return Ok(());
        }

        let pr_head_sha = event
            .payload
            .pointer("/pull_request/head/sha")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();

        if pr_head_sha.is_empty() {
            tracing::warn!(
                event_id = %event.event_id,
                pr = pr_number,
                "PR head SHA missing from payload; skipping status comment"
            );
            return Ok(());
        }

        let pr_head_branch = event
            .payload
            .pointer("/pull_request/head/ref")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let pr_author_login = event
            .payload
            .pointer("/pull_request/user/login")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();

        let installation_id = self.resolve_installation_id(owner, repo).await?;

        let repo_config = self
            .configuration_provider
            .get_merged_config(
                owner,
                repo,
                LoadOptions {
                    installation_id: Some(installation_id),
                    default_branch: Some(event.repository.default_branch.clone()),
                    ..Default::default()
                },
            )
            .await?;

        // Skip PRs from excluded authors.
        if repo_config
            .versioning
            .excluded_pr_authors
            .iter()
            .any(|a| a == &pr_author_login)
        {
            tracing::debug!(
                owner = %owner,
                repo = %repo,
                pr = pr_number,
                author = %pr_author_login,
                "Skipping PR status comment for excluded author"
            );
            return Ok(());
        }

        let branch_prefix =
            release_orchestrator::OrchestratorConfig::DEFAULT_BRANCH_PREFIX.to_string();
        let version_prefix = repo_config.core.version_prefix.clone();
        let scoped_github = self.github_operations.scoped_to(installation_id);

        let body = if is_release_pr_branch(&pr_head_branch, &branch_prefix, &version_prefix) {
            // Release PR path (F.3): extract version from branch name.
            let release_version = release_automator::extract_version_from_branch(
                &pr_head_branch,
                &branch_prefix,
                &version_prefix,
            )?;
            pr_status_commenter::render_release_pr_comment(
                &release_version,
                repo_config.versioning.allow_override,
            )
        } else {
            // Feature PR path (F.2): project the next version from commits.
            let current_version =
                versioning::resolve_current_version(&scoped_github, owner, repo, false).await?;

            let strategy = match repo_config.versioning.strategy {
                config::VersioningStrategy::Conventional
                | config::VersioningStrategy::External { .. } => {
                    VersioningStrategy::ConventionalCommits {
                        custom_types: std::collections::HashMap::new(),
                        include_prerelease: false,
                    }
                }
            };

            let ctx = VersionContext {
                base_ref: current_version
                    .as_ref()
                    .map(|v| format!("{}{v}", repo_config.core.version_prefix)),
                current_version: current_version.clone(),
                head_ref: pr_head_sha,
                owner: owner.to_string(),
                repo: repo.to_string(),
                target_branch: pr_head_branch,
            };

            let scoped_calc = self.version_calculator.scoped_to(installation_id);
            let calc_result = scoped_calc
                .calculate_version(ctx, strategy, CalculationOptions::default())
                .await?;

            let base_version = current_version.unwrap_or(versioning::SemanticVersion {
                major: 0,
                minor: 0,
                patch: 0,
                prerelease: None,
                build: None,
            });

            // Check whether a release PR is already open with a higher version.
            // The trailing * makes this a prefix match so all versioned release
            // branches are captured (e.g. "is:open head:release/v*").
            let release_search_query =
                format!("is:open head:{}/{}*", branch_prefix, version_prefix);
            let queued_release_version: Option<versioning::SemanticVersion> = scoped_github
                .search_pull_requests(owner, repo, &release_search_query)
                .await
                .unwrap_or_else(|e| {
                    tracing::debug!(
                        error = %e,
                        "Failed to search for open release PRs; assuming none"
                    );
                    vec![]
                })
                .iter()
                .filter_map(|pr| {
                    release_automator::extract_version_from_branch(
                        &pr.head.ref_name,
                        &branch_prefix,
                        &version_prefix,
                    )
                    .ok()
                })
                .max();

            pr_status_commenter::render_feature_pr_comment(
                &calc_result.next_version,
                &base_version,
                queued_release_version.as_ref(),
                repo_config.versioning.allow_override,
            )
        };

        pr_status_commenter::upsert_pr_status_comment(&scoped_github, owner, repo, pr_number, &body)
            .await
    }
}
///
/// The loop polls `source.next_event()` continuously:
///
/// - `Ok(Some(event))` — dispatches the event to the appropriate handler,
///   then calls `source.acknowledge()` on success or `source.reject()` on
///   failure.  Processing errors are **never** fatal to the loop.
/// - `Ok(None)` — sleeps for 100 ms before polling again (avoids busy-spin).
/// - `Err(e)` — logs the source-level error and continues; a bad message from
///   the source does not crash the loop.
///
/// The loop exits cleanly (returning `Ok(())`) when `token.is_cancelled()`
/// returns `true` at the top of any iteration.
///
/// # Structured logging
///
/// Every event dispatch is wrapped in a tracing span that records
/// `event_id`, `correlation_id`, and `event_type` as structured fields so that
/// all log lines emitted within the handler are automatically correlated.
///
/// # Cancellation
///
/// Cancellation is cooperative: the loop finishes processing the *current*
/// event (if any) before checking for cancellation again.  There is no forced
/// interruption mid-dispatch.
///
/// # Errors
///
/// Currently always returns `Ok(())`.  Future versions may propagate
/// unrecoverable infrastructure errors.
///
/// # Examples
///
/// ```rust,ignore
/// use release_regent_core::{run_event_loop, MergedPullRequestHandler};
/// use tokio_util::sync::CancellationToken;
///
/// let token = CancellationToken::new();
/// let source = MyEventSource::new();
/// let processor = build_processor();  // implements MergedPullRequestHandler
/// run_event_loop(&source, &processor, token).await?;
/// ```
pub async fn run_event_loop<S, H>(
    source: &S,
    handler: &H,
    token: tokio_util::sync::CancellationToken,
) -> CoreResult<()>
where
    S: traits::event_source::EventSource,
    H: MergedPullRequestHandler,
{
    use tracing::Instrument as _;
    use traits::event_source::EventType;

    loop {
        if token.is_cancelled() {
            break;
        }

        match source.next_event().await {
            Ok(Some(event)) => {
                let span = tracing::info_span!(
                    "process_event",
                    event_id = %event.event_id,
                    correlation_id = %event.correlation_id,
                    event_type = %event.event_type,
                );

                let dispatch_result: CoreResult<()> = async {
                    match &event.event_type {
                        EventType::PullRequestMerged => {
                            tracing::info!(
                                event_id = %event.event_id,
                                repository = %format!(
                                    "{}/{}",
                                    event.repository.owner, event.repository.name
                                ),
                                "Pull request merged — orchestrating release PR"
                            );
                            handler.handle_merged_pull_request(&event).await
                        }
                        EventType::ReleasePrMerged => {
                            tracing::info!(
                                event_id = %event.event_id,
                                repository = %format!(
                                    "{}/{}",
                                    event.repository.owner, event.repository.name
                                ),
                                "Release PR merged — running release automator"
                            );
                            handler.handle_release_pr_merged(&event).await
                        }
                        EventType::PullRequestCommentReceived => {
                            tracing::debug!(
                                event_id = %event.event_id,
                                "Pull request comment received — dispatching to comment handler"
                            );
                            handler.handle_pr_comment(&event).await
                        }
                        EventType::PullRequestOpened | EventType::PullRequestUpdated => {
                            tracing::debug!(
                                event_id = %event.event_id,
                                event_type = %event.event_type,
                                "Pull request activity — dispatching to activity handler"
                            );
                            handler.handle_pull_request_activity(&event).await
                        }
                        EventType::Unknown(raw) => {
                            tracing::debug!(
                                event_id = %event.event_id,
                                raw_type = %raw,
                                "Unknown event type; dropping"
                            );
                            Ok(())
                        }
                    }
                }
                .instrument(span)
                .await;

                match dispatch_result {
                    Ok(()) => {
                        if let Err(e) = source.acknowledge(&event.event_id).await {
                            tracing::error!(
                                error = %e,
                                event_id = %event.event_id,
                                "Failed to acknowledge event"
                            );
                        }
                    }
                    Err(e) => {
                        let permanent = !e.is_retryable();
                        tracing::warn!(
                            error = %e,
                            event_id = %event.event_id,
                            permanent,
                            "Event processing failed; rejecting"
                        );
                        if let Err(reject_err) = source.reject(&event.event_id, permanent).await {
                            tracing::error!(
                                error = %reject_err,
                                event_id = %event.event_id,
                                "Failed to reject event"
                            );
                        }
                    }
                }
            }
            Ok(None) => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            Err(e) => {
                tracing::error!(error = %e, "Event source error; continuing");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }

    Ok(())
}

/// Release Regent core engine
///
/// This is the main entry point for Release Regent operations. It orchestrates
/// the various modules to process webhook events and manage releases.
#[derive(Debug)]
pub struct ReleaseRegent {
    config: config::ReleaseRegentConfig,
}

impl ReleaseRegent {
    /// Create a new Release Regent instance with the provided configuration
    ///
    /// # Arguments
    /// * `config` - The Release Regent configuration
    ///
    /// # Examples
    /// ```no_run
    /// use release_regent_core::{ReleaseRegent, config::ReleaseRegentConfig};
    ///
    /// let config = ReleaseRegentConfig::default();
    /// let regent = ReleaseRegent::new(config);
    /// ```
    #[must_use]
    pub fn new(config: config::ReleaseRegentConfig) -> Self {
        Self { config }
    }

    /// Get the current configuration
    #[must_use]
    pub fn config(&self) -> &config::ReleaseRegentConfig {
        &self.config
    }
}

/// Bundled result from version calculation, used internally by
/// [`ReleaseRegentProcessor::handle_merged_pull_request`].
struct MergeCalcResult {
    calc_result: traits::version_calculator::VersionCalculationResult,
    changelog: String,
    current_version: Option<versioning::SemanticVersion>,
    repo_config: config::ReleaseRegentConfig,
}

/// Release Regent processor with dependency injection
///
/// This is the main business logic processor that uses dependency injection
/// for all external services, enabling comprehensive testing through trait
/// abstractions.
///
/// # Type Parameters
/// * `G` - GitHub operations implementation
/// * `C` - Configuration provider implementation
/// * `V` - Version calculator implementation
///
/// # Examples
/// ```ignore
/// use release_regent_core::{ReleaseRegentProcessor, config::ReleaseRegentConfig};
/// use release_regent_testing::{MockGitHubOperations, MockConfigurationProvider, MockVersionCalculator};
///
/// let github_ops = MockGitHubOperations::new();
/// let config_provider = MockConfigurationProvider::new();
/// let version_calc = MockVersionCalculator::new();
///
/// let processor = ReleaseRegentProcessor::new(github_ops, config_provider, version_calc);
/// ```
#[derive(Debug)]
pub struct ReleaseRegentProcessor<G, C, V>
where
    G: GitHubOperations,
    C: ConfigurationProvider,
    V: VersionCalculator,
{
    github_operations: G,
    configuration_provider: C,
    version_calculator: V,
}

impl<G, C, V> ReleaseRegentProcessor<G, C, V>
where
    G: GitHubOperations,
    C: ConfigurationProvider,
    V: VersionCalculator,
{
    /// Create a new Release Regent processor with injected dependencies
    ///
    /// # Arguments
    /// * `github_operations` - GitHub API operations implementation
    /// * `configuration_provider` - Configuration loading implementation
    /// * `version_calculator` - Version calculation implementation
    ///
    /// # Examples
    /// ```ignore
    /// use release_regent_core::ReleaseRegentProcessor;
    /// use release_regent_testing::{MockGitHubOperations, MockConfigurationProvider, MockVersionCalculator};
    ///
    /// let github_ops = MockGitHubOperations::new();
    /// let config_provider = MockConfigurationProvider::new();
    /// let version_calc = MockVersionCalculator::new();
    ///
    /// let processor = ReleaseRegentProcessor::new(github_ops, config_provider, version_calc);
    /// ```
    pub fn new(github_operations: G, configuration_provider: C, version_calculator: V) -> Self {
        Self {
            github_operations,
            configuration_provider,
            version_calculator,
        }
    }

    /// Get a reference to the GitHub operations
    pub fn github_operations(&self) -> &G {
        &self.github_operations
    }

    /// Get a reference to the configuration provider
    pub fn configuration_provider(&self) -> &C {
        &self.configuration_provider
    }

    /// Get a reference to the version calculator
    pub fn version_calculator(&self) -> &V {
        &self.version_calculator
    }

    /// Resolve a reliable installation ID for `owner/repo`.
    ///
    /// Calls the GitHub App API to look up the installation ID for the given
    /// repository.
    async fn resolve_installation_id(&self, owner: &str, repo: &str) -> CoreResult<u64> {
        self.github_operations
            .get_installation_id_for_repo(owner, repo)
            .await
    }

    /// Handle a merged pull request event by orchestrating the creation or
    /// update of a release PR.
    ///
    /// This is the main entry point for processing `EventType::PullRequestMerged`
    /// events. It performs the following steps in order:
    ///
    /// 1. Extract `base_branch` and `merge_commit_sha` from the event payload.
    /// 2. Load the merged repository configuration.
    /// 3. Resolve the current release version from Git tags.
    /// 4. Calculate the next semantic version from commit history.
    /// 5. Format a changelog body from the commit analysis.
    /// 6. Orchestrate the release PR (create, update, or rename via
    ///    [`release_orchestrator::ReleaseOrchestrator`]).
    ///
    /// # Parameters
    /// - `event`: The normalised `PullRequestMerged` processing event, including
    ///   `payload` (raw GitHub webhook JSON), `repository`, and `correlation_id`.
    ///
    /// # Returns
    /// The [`release_orchestrator::OrchestratorResult`] describing what action
    /// was taken (PR created, updated, renamed, or no-op).
    ///
    /// # Errors
    /// - [`CoreError::InvalidInput`] — the payload is missing `merge_commit_sha`
    ///   and `head.sha`.
    /// - [`CoreError::GitHub`] / [`CoreError::Network`] — a GitHub API call failed.
    /// - [`CoreError::Versioning`] — version calculation failed.
    /// - [`CoreError::Config`] — configuration loading failed.
    pub async fn handle_merged_pull_request(
        &self,
        event: &traits::event_source::ProcessingEvent,
    ) -> CoreResult<release_orchestrator::OrchestratorResult> {
        let owner = &event.repository.owner;
        let repo = &event.repository.name;
        let correlation_id = &event.correlation_id;

        // Extract base branch from payload; fall back to the repository's
        // configured default branch when the payload field is absent.
        let base_branch = event
            .payload
            .get("pull_request")
            .and_then(|pr| pr.get("base"))
            .and_then(|base| base.get("ref"))
            .and_then(|v| v.as_str())
            .unwrap_or(&event.repository.default_branch)
            .to_string();

        // Check the merged PR's head branch early to avoid running the expensive
        // calculate_version_for_merge (tag fetching + version calculation +
        // changelog generation) when this is a release PR merge.  We need only
        // the repository config on that path — not the full version pipeline.
        let merged_pr_head_ref = event
            .payload
            .get("pull_request")
            .and_then(|pr| pr.get("head"))
            .and_then(|h| h.get("ref"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let release_branch_prefix = format!(
            "{}/",
            release_orchestrator::OrchestratorConfig::DEFAULT_BRANCH_PREFIX
        );
        if merged_pr_head_ref.starts_with(&release_branch_prefix) {
            use traits::configuration_provider::LoadOptions;
            let installation_id = self.resolve_installation_id(owner, repo).await?;
            let repo_config = self
                .configuration_provider
                .get_merged_config(
                    owner,
                    repo,
                    LoadOptions {
                        installation_id: Some(installation_id),
                        default_branch: Some(base_branch.clone()),
                        ..Default::default()
                    },
                )
                .await?;
            if release_automator::is_release_pr_branch(
                &merged_pr_head_ref,
                release_orchestrator::OrchestratorConfig::DEFAULT_BRANCH_PREFIX,
                &repo_config.core.version_prefix,
            ) {
                return self
                    .process_release_pr_merged(
                        owner,
                        repo,
                        installation_id,
                        correlation_id,
                        &repo_config,
                        event,
                    )
                    .await;
            }
        }

        // Feature PR path: the merge commit SHA is required as the branch
        // point for the new release branch.
        let base_sha = event
            .payload
            .get("pull_request")
            .and_then(|pr| pr.get("merge_commit_sha"))
            .and_then(|v| v.as_str())
            .or_else(|| {
                event
                    .payload
                    .get("pull_request")
                    .and_then(|pr| pr.get("head"))
                    .and_then(|head| head.get("sha"))
                    .and_then(|v| v.as_str())
            })
            .ok_or_else(|| {
                CoreError::invalid_input(
                    "payload",
                    "PullRequestMerged payload is missing both \
                     merge_commit_sha and pull_request.head.sha",
                )
            })?
            .to_string();

        let installation_id = self.resolve_installation_id(owner, repo).await?;

        let MergeCalcResult {
            calc_result,
            changelog,
            current_version,
            repo_config,
        } = self
            .calculate_version_for_merge(owner, repo, &base_sha, &base_branch, installation_id)
            .await?;

        // Build orchestrator config honouring the repository PR title template.
        let orch_config = release_orchestrator::OrchestratorConfig {
            branch_prefix: release_orchestrator::OrchestratorConfig::DEFAULT_BRANCH_PREFIX
                .to_string(),
            version_prefix: repo_config.core.version_prefix.clone(),
            title_template: repo_config.release_pr.title_template.clone(),
            changelog_header: release_orchestrator::extract_changelog_header(
                &repo_config.release_pr.body_template,
            ),
            body_template: repo_config.release_pr.body_template.clone(),
            manifest_files: repo_config.release_pr.manifest_files.clone(),
            auto_detect_manifests: repo_config.release_pr.auto_detect_manifests,
        };

        // Resolve the merged PR number (needed to read override labels on the
        // feature-PR path).
        let merged_pr_number: u64 = event
            .payload
            .get("pull_request")
            .and_then(|pr| pr.get("number"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        let scoped_github = self.github_operations.scoped_to(installation_id);
        let orchestrator =
            release_orchestrator::ReleaseOrchestrator::new(orch_config, &scoped_github);

        self.process_feature_pr_merged(
            owner,
            repo,
            installation_id,
            merged_pr_number,
            correlation_id,
            &orchestrator,
            current_version.as_ref(),
            &calc_result,
            &changelog,
            &base_branch,
            &base_sha,
        )
        .await
    }

    /// Load configuration and calculate the next version for a merge event.
    async fn calculate_version_for_merge(
        &self,
        owner: &str,
        repo: &str,
        base_sha: &str,
        base_branch: &str,
        installation_id: u64,
    ) -> CoreResult<MergeCalcResult> {
        use traits::configuration_provider::LoadOptions;
        use traits::version_calculator::{CalculationOptions, VersionContext, VersioningStrategy};

        let repo_config = self
            .configuration_provider
            .get_merged_config(
                owner,
                repo,
                LoadOptions {
                    installation_id: Some(installation_id),
                    default_branch: Some(base_branch.to_string()),
                    ..Default::default()
                },
            )
            .await?;

        let scoped_github = self.github_operations.scoped_to(installation_id);
        let current_version =
            versioning::resolve_current_version(&scoped_github, owner, repo, false).await?;

        let ctx = VersionContext {
            base_ref: current_version
                .as_ref()
                .map(|v| format!("{}{v}", repo_config.core.version_prefix)),
            current_version: current_version.clone(),
            head_ref: base_sha.to_string(),
            owner: owner.to_string(),
            repo: repo.to_string(),
            target_branch: base_branch.to_string(),
        };

        let strategy = match repo_config.versioning.strategy {
            config::VersioningStrategy::Conventional
            | config::VersioningStrategy::External { .. } => {
                VersioningStrategy::ConventionalCommits {
                    custom_types: std::collections::HashMap::new(),
                    include_prerelease: false,
                }
            }
        };

        let options = CalculationOptions {
            generate_changelog: true,
            ..Default::default()
        };

        // Scope the calculator to the resolved installation before calling it,
        // keeping authentication concerns out of VersionContext.
        let scoped_calc = self.version_calculator.scoped_to(installation_id);
        let calc_result = scoped_calc
            .calculate_version(ctx, strategy, options)
            .await?;

        // Build ConventionalCommit items from the raw analyzed_commits so that
        // all ChangelogGenerator strategies receive the correct vocabulary:
        // - commit_type = raw conventional-commit type token ("feat", "fix", …)
        // - message     = full original commit string ("feat(auth): add OAuth")
        // These are required for git-cliff (message parsed as conv-commit) and
        // external tools (stdin line reconstructed from sha + message).
        // In production, commit_type is always Some(...) — parse_single_conventional_commit
        // falls back to "chore" for non-conventional messages such as merge commits.
        // Non-conventional commits reach git-cliff/External and are silently dropped there
        // by filter_unconventional=true; they are not omitted here.
        // The filter_map's `?` guards only against custom VersionCalculator
        // implementations that might return None for commit_type.
        let commits: Vec<versioning::ConventionalCommit> = calc_result
            .analyzed_commits
            .iter()
            .filter_map(|a| {
                let commit_type = a.commit_type.clone()?;
                // Extract the description by dropping the "type(scope): " prefix from
                // the full commit message.  For non-conventional messages (e.g. merge
                // commits) where no ": " separator exists, fall back to the full message.
                let description = a
                    .message
                    .find(": ")
                    .map(|i| a.message[i + 2..].to_string())
                    .unwrap_or_else(|| a.message.clone());
                Some(versioning::ConventionalCommit {
                    commit_type,
                    scope: a.scope.clone(),
                    description,
                    breaking_change: a.is_breaking,
                    message: a.message.clone(),
                    sha: a.sha.clone(),
                })
            })
            .collect();

        let changelog = changelog::ChangelogGenerator::with_config(repo_config.changelog.clone())
            .generate_changelog(&commits)?;

        Ok(MergeCalcResult {
            calc_result,
            changelog,
            current_version,
            repo_config,
        })
    }

    /// Handle the release-PR path when a merged pull request is identified as a
    /// release PR.
    ///
    /// This path is taken when `handle_merged_pull_request` (the inherent method)
    /// detects that the merged PR head branch starts with the configured release
    /// prefix — typically because the webhook event was classified as
    /// `PullRequestMerged` rather than `ReleasePrMerged` (the two differ when
    /// the server's default `version_prefix` does not match the repository's
    /// configured prefix).
    ///
    /// Invokes [`release_automator::ReleaseAutomator`] to create the annotated
    /// git tag and GitHub release, then clears stale bump-override labels from
    /// any open feature PRs.
    async fn process_release_pr_merged(
        &self,
        owner: &str,
        repo: &str,
        installation_id: u64,
        correlation_id: &str,
        repo_config: &config::ReleaseRegentConfig,
        event: &traits::event_source::ProcessingEvent,
    ) -> CoreResult<release_orchestrator::OrchestratorResult> {
        use release_automator::{AutomatorConfig, ReleaseAutomator};

        tracing::info!(
            owner = %owner,
            repo = %repo,
            correlation_id = %correlation_id,
            "Handling merged release PR via automator \
             (arrived as PullRequestMerged; head branch matches release prefix)"
        );

        let config = AutomatorConfig {
            branch_prefix: release_orchestrator::OrchestratorConfig::DEFAULT_BRANCH_PREFIX
                .to_string(),
            changelog_header: release_orchestrator::extract_changelog_header(
                &repo_config.release_pr.body_template,
            ),
            version_prefix: repo_config.core.version_prefix.clone(),
            generate_release_notes: repo_config.releases.generate_notes,
        };

        ReleaseAutomator::new(config, &self.github_operations.scoped_to(installation_id))
            .automate(owner, repo, event, correlation_id)
            .await?;

        // After the release is published, clear stale rr:override-* labels from
        // any open feature PRs.  These overrides were scoped to this release cycle.
        self.clear_stale_override_labels_after_release(
            owner,
            repo,
            installation_id,
            correlation_id,
        )
        .await;

        Ok(release_orchestrator::OrchestratorResult::TaggedRelease)
    }

    /// Clear stale bump-override labels from open feature PRs after a release.
    ///
    /// Enumerates all currently-open PRs via `list_pull_requests`, then
    /// inspects each one's actual labels via `list_pr_labels`.  Only PRs that
    /// carry at least one `rr:override-*` label are processed; all others are
    /// skipped without any API calls.
    ///
    /// For each qualifying PR:
    /// 1. Every `rr:override-*` label present is removed.
    /// 2. Exactly **one** notification comment is posted (regardless of how many
    ///    labels were removed), explaining that the override has been cleared
    ///    because a new release was published.
    ///
    /// All errors are logged as warnings and the loop continues; this function
    /// is intentionally best-effort and never propagates failures to callers.
    async fn clear_stale_override_labels_after_release(
        &self,
        owner: &str,
        repo: &str,
        installation_id: u64,
        correlation_id: &str,
    ) {
        use comment_command_processor::ALL_OVERRIDE_LABELS;
        use std::collections::HashSet;

        let scoped_github = self.github_operations.scoped_to(installation_id);

        // The GitHubClient implementation of search_pull_requests does not
        // filter by the `label:` qualifier (only `is:`, `head:`, and `base:`
        // are parsed).  Using list_pull_requests + per-PR list_pr_labels
        // guarantees we only act on PRs that actually carry override labels.
        let open_prs = match scoped_github
            .list_pull_requests(owner, repo, Some("open"), None, None, None, None)
            .await
        {
            Ok(prs) => prs,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    owner,
                    repo,
                    correlation_id = %correlation_id,
                    "Failed to list open PRs for override-label cleanup; skipping"
                );
                return;
            }
        };

        let override_label_set: HashSet<&str> = ALL_OVERRIDE_LABELS.iter().copied().collect();

        for pr in &open_prs {
            // Fetch the actual labels on this PR.
            let pr_labels = match scoped_github.list_pr_labels(owner, repo, pr.number).await {
                Ok(labels) => labels,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        pr = pr.number,
                        correlation_id = %correlation_id,
                        "Failed to list labels for PR during override cleanup; skipping"
                    );
                    continue;
                }
            };

            // Collect override labels that are actually present on this PR.
            let stale_overrides: Vec<String> = pr_labels
                .iter()
                .filter(|l| override_label_set.contains(l.name.as_str()))
                .map(|l| l.name.clone())
                .collect();

            if stale_overrides.is_empty() {
                continue;
            }

            // Remove each stale override label.
            for label_name in &stale_overrides {
                if let Err(e) = scoped_github
                    .remove_label(owner, repo, pr.number, label_name)
                    .await
                {
                    tracing::warn!(
                        error = %e,
                        pr = pr.number,
                        label = %label_name,
                        correlation_id = %correlation_id,
                        "Failed to remove stale override label; continuing"
                    );
                }
            }

            // Post exactly one cleanup comment per PR, listing every kind that
            // was removed.  This avoids duplicate comments when a PR carried
            // more than one override label.  Use the plural form and list each
            // command individually when multiple labels were removed so the
            // copy is grammatically correct and each label is a valid command.
            let cleanup_body = if stale_overrides.len() == 1 {
                let kind = stale_overrides[0]
                    .strip_prefix("rr:override-")
                    .unwrap_or(&stale_overrides[0]);
                format!(
                    "ℹ️ **Release Regent**: The `!release {kind}` override on \
                     this PR has been cleared because a new release was published \
                     before this PR merged. If the work in this PR still warrants \
                     a minimum bump for the next release, please re-post your \
                     `!release` command."
                )
            } else {
                let commands = stale_overrides
                    .iter()
                    .filter_map(|l| l.strip_prefix("rr:override-"))
                    .map(|kind| format!("`!release {kind}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "ℹ️ **Release Regent**: The {commands} overrides on \
                     this PR have been cleared because a new release was published \
                     before this PR merged. If the work in this PR still warrants \
                     a minimum bump for the next release, please re-post your \
                     `!release` command."
                )
            };
            if let Err(e) = scoped_github
                .create_issue_comment(owner, repo, pr.number, &cleanup_body)
                .await
            {
                tracing::warn!(
                    error = %e,
                    pr = pr.number,
                    correlation_id = %correlation_id,
                    "Failed to post stale-override cleanup comment; continuing"
                );
            }
        }
    }

    /// Handle the feature-PR path after a merged pull request.
    ///
    /// Applies any bump-floor override from the merged PR's labels, orchestrates
    /// the release PR, posts an audit comment if the floor was applied, and
    /// removes the consumed override labels.
    #[allow(clippy::too_many_arguments)] // owner/repo/installation_id/pr_num/correlation/orchestrator/current/calc/changelog/branch/sha is minimal
    async fn process_feature_pr_merged(
        &self,
        owner: &str,
        repo: &str,
        installation_id: u64,
        merged_pr_number: u64,
        correlation_id: &str,
        orchestrator: &release_orchestrator::ReleaseOrchestrator<'_, G>,
        current_version: Option<&versioning::SemanticVersion>,
        calc_result: &traits::version_calculator::VersionCalculationResult,
        changelog: &str,
        base_branch: &str,
        base_sha: &str,
    ) -> CoreResult<release_orchestrator::OrchestratorResult> {
        use comment_command_processor::{
            ALL_OVERRIDE_LABELS, OVERRIDE_LABEL_MAJOR, OVERRIDE_LABEL_MINOR, OVERRIDE_LABEL_PATCH,
        };
        use versioning::BumpKind;

        let scoped_github = self.github_operations.scoped_to(installation_id);

        // Read any rr:override-* label from the merged PR and apply it as a
        // minimum-bump floor before calling the orchestrator.
        let labels = if merged_pr_number > 0 {
            scoped_github
                .list_pr_labels(owner, repo, merged_pr_number)
                .await?
        } else {
            tracing::warn!(
                correlation_id = %correlation_id,
                "Merged PR payload is missing pull_request.number; \
                 bump-override floor will not be applied for this release"
            );
            vec![]
        };

        let floor_kind: Option<BumpKind> = labels.iter().find_map(|l| match l.name.as_str() {
            OVERRIDE_LABEL_MAJOR => Some(BumpKind::Major),
            OVERRIDE_LABEL_MINOR => Some(BumpKind::Minor),
            OVERRIDE_LABEL_PATCH => Some(BumpKind::Patch),
            _ => None,
        });

        let effective_version =
            if let (Some(ref floor), Some(current)) = (&floor_kind, current_version) {
                versioning::apply_bump_floor(current, &calc_result.next_version, floor)
            } else {
                calc_result.next_version.clone()
            };

        tracing::debug!(
            owner = %owner,
            repo = %repo,
            calculated = %calc_result.next_version,
            effective = %effective_version,
            floor = ?floor_kind,
            correlation_id = %correlation_id,
            "Resolved effective release version after bump-floor check"
        );

        // Guard: if the effective version equals the already-released current
        // version it means there are no version-bumping commits since the last
        // release and no bump-override floor was applied.  Creating a release
        // branch for a version that already has a tag would be wrong — it is
        // exactly the scenario that causes `release/v0.3.0` to be resurrected
        // after the 0.3.0 release branch is merged.
        if let Some(current) = current_version {
            if effective_version.compare_precedence(current) == std::cmp::Ordering::Equal {
                tracing::info!(
                    owner = %owner,
                    repo = %repo,
                    version = %effective_version,
                    correlation_id = %correlation_id,
                    "Effective version equals current released version; \
                     no version-bumping commits — skipping release branch creation"
                );
                return Ok(release_orchestrator::OrchestratorResult::NoBumpNeeded);
            }
        }

        tracing::info!(
            owner = %owner,
            repo = %repo,
            version = %effective_version,
            base_branch = %base_branch,
            correlation_id = %correlation_id,
            "Orchestrating release PR for merged pull request"
        );

        let orch_result = orchestrator
            .orchestrate(
                owner,
                repo,
                &effective_version,
                changelog,
                base_branch,
                base_sha,
                correlation_id,
            )
            .await?;

        // Post an audit comment on the release PR when the floor was applied.
        if effective_version != calc_result.next_version {
            if let Some(ref floor) = floor_kind {
                self.post_bump_floor_audit_comment(
                    owner,
                    repo,
                    installation_id,
                    merged_pr_number,
                    correlation_id,
                    &orch_result,
                    floor,
                    &calc_result.next_version,
                    &effective_version,
                )
                .await;
            }
        }

        // Consume override label: remove from the now-merged feature PR.
        // This is idempotent (remove_label treats 404 as Ok) and runs
        // unconditionally to clean up any label applied by `!release` commands.
        if floor_kind.is_some() {
            for &label in ALL_OVERRIDE_LABELS {
                if let Err(e) = scoped_github
                    .remove_label(owner, repo, merged_pr_number, label)
                    .await
                {
                    tracing::warn!(
                        error = %e,
                        merged_pr = merged_pr_number,
                        label,
                        correlation_id = %correlation_id,
                        "Failed to remove consumed override label; continuing"
                    );
                }
            }
        }

        Ok(orch_result)
    }

    /// Post an audit comment on the release PR explaining a bump-floor override.
    #[allow(clippy::too_many_arguments)] // audit context requires all 9 data points; no good grouping
    async fn post_bump_floor_audit_comment(
        &self,
        owner: &str,
        repo: &str,
        installation_id: u64,
        merged_pr_number: u64,
        correlation_id: &str,
        orch_result: &release_orchestrator::OrchestratorResult,
        floor: &versioning::BumpKind,
        calc_version: &versioning::SemanticVersion,
        eff_version: &versioning::SemanticVersion,
    ) {
        use versioning::BumpKind;

        let kind_str = match floor {
            BumpKind::Major => "major",
            BumpKind::Minor => "minor",
            BumpKind::Patch => "patch",
        };
        let release_pr_number = match orch_result {
            release_orchestrator::OrchestratorResult::Created { pr, .. }
            | release_orchestrator::OrchestratorResult::Updated { pr }
            | release_orchestrator::OrchestratorResult::Renamed { pr }
            | release_orchestrator::OrchestratorResult::NoOp { pr } => Some(pr.number),
            // NoBumpNeeded is returned before the orchestrator is called, so
            // there is no release PR to post the audit comment on.
            release_orchestrator::OrchestratorResult::NoBumpNeeded => None,
            // TaggedRelease is produced by the release-PR merge path, not by the
            // orchestrator, so there is no open release PR to comment on.
            release_orchestrator::OrchestratorResult::TaggedRelease => None,
        };
        let Some(release_pr) = release_pr_number else {
            return;
        };
        let audit_body = format!(
            "🔼 **Release Regent**: Version floor applied from `!release {kind_str}` \
             override on PR #{merged_pr_number}. The calculated version was \
             `{calc_version}` but was raised to `{eff_version}` to satisfy the requested \
             minimum {kind_str} bump.",
        );
        let scoped_github = self.github_operations.scoped_to(installation_id);
        if let Err(e) = scoped_github
            .create_issue_comment(owner, repo, release_pr, &audit_body)
            .await
        {
            tracing::warn!(
                error = %e,
                release_pr,
                merged_pr = merged_pr_number,
                correlation_id = %correlation_id,
                "Failed to post bump-floor audit comment; continuing"
            );
        }
    }

    /// Best-effort refresh of open feature PR status comments.
    ///
    /// Resolves the installation ID and repository configuration before
    /// delegating to [`Self::refresh_open_feature_pr_comments`].  All errors
    /// are logged and swallowed so that a refresh failure never fails the
    /// triggering merge event.
    async fn try_refresh_feature_pr_status_comments(
        &self,
        owner: &str,
        repo: &str,
        default_branch: &str,
    ) {
        use traits::configuration_provider::LoadOptions;

        let installation_id = match self.resolve_installation_id(owner, repo).await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    owner = %owner,
                    repo = %repo,
                    "Failed to resolve installation ID for PR status refresh"
                );
                return;
            }
        };

        let repo_config = match self
            .configuration_provider
            .get_merged_config(
                owner,
                repo,
                LoadOptions {
                    installation_id: Some(installation_id),
                    default_branch: Some(default_branch.to_string()),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    owner = %owner,
                    repo = %repo,
                    "Failed to load config for PR status refresh"
                );
                return;
            }
        };

        self.refresh_open_feature_pr_comments(owner, repo, installation_id, &repo_config)
            .await;
    }

    /// Refresh the status comment on open feature PRs after a base-version change.
    ///
    /// Only refreshes PRs that already have a `<!-- release-regent:pr-status -->`
    /// comment.  Skips release-branch PRs and PRs authored by logins in
    /// `excluded_pr_authors`.  Caps at 25 PRs per triggering event.
    ///
    /// All per-PR errors (list comments, calculate version, upsert) are logged
    /// and skipped so that one failing PR does not abort the rest.
    async fn refresh_open_feature_pr_comments(
        &self,
        owner: &str,
        repo: &str,
        installation_id: u64,
        repo_config: &config::ReleaseRegentConfig,
    ) {
        use release_automator::is_release_pr_branch;
        use traits::version_calculator::{CalculationOptions, VersionContext, VersioningStrategy};

        let branch_prefix =
            release_orchestrator::OrchestratorConfig::DEFAULT_BRANCH_PREFIX.to_string();
        let version_prefix = repo_config.core.version_prefix.clone();
        let scoped_github = self.github_operations.scoped_to(installation_id);

        let open_prs = match scoped_github
            .search_pull_requests(owner, repo, "is:open")
            .await
        {
            Ok(prs) => prs,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    owner = %owner,
                    repo = %repo,
                    "Failed to list open PRs for status comment refresh; skipping"
                );
                return;
            }
        };

        let excluded = &repo_config.versioning.excluded_pr_authors;

        // Find the highest open release PR version from the already-fetched list
        // so we can annotate feature PR comments when a release is already queued.
        let queued_release_version: Option<versioning::SemanticVersion> = open_prs
            .iter()
            .filter(|pr| is_release_pr_branch(&pr.head.ref_name, &branch_prefix, &version_prefix))
            .filter_map(|pr| {
                release_automator::extract_version_from_branch(
                    &pr.head.ref_name,
                    &branch_prefix,
                    &version_prefix,
                )
                .ok()
            })
            .max();

        // Feature PRs only; skip excluded authors; cap at 25.
        let candidates: Vec<_> = open_prs
            .into_iter()
            .filter(|pr| !is_release_pr_branch(&pr.head.ref_name, &branch_prefix, &version_prefix))
            .filter(|pr| {
                let login = pr.user.login.as_deref().unwrap_or_default();
                !excluded.iter().any(|a| a == login)
            })
            .take(25)
            .collect();

        let current_version =
            match versioning::resolve_current_version(&scoped_github, owner, repo, false).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        owner = %owner,
                        repo = %repo,
                        "Failed to resolve current version for PR refresh"
                    );
                    return;
                }
            };

        let base_version = current_version
            .clone()
            .unwrap_or(versioning::SemanticVersion {
                major: 0,
                minor: 0,
                patch: 0,
                prerelease: None,
                build: None,
            });

        let strategy = match repo_config.versioning.strategy {
            config::VersioningStrategy::Conventional
            | config::VersioningStrategy::External { .. } => {
                VersioningStrategy::ConventionalCommits {
                    custom_types: std::collections::HashMap::new(),
                    include_prerelease: false,
                }
            }
        };

        for pr in candidates {
            // Only refresh PRs that already have a status marker comment.
            let comments = match scoped_github
                .list_issue_comments(owner, repo, pr.number)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        pr = pr.number,
                        "Failed to list comments for PR refresh; skipping PR"
                    );
                    continue;
                }
            };

            if !comments
                .iter()
                .any(|c| c.body.contains(pr_status_commenter::PR_STATUS_MARKER))
            {
                continue;
            }

            let ctx = VersionContext {
                base_ref: current_version
                    .as_ref()
                    .map(|v| format!("{}{v}", repo_config.core.version_prefix)),
                current_version: current_version.clone(),
                head_ref: pr.head.sha.clone(),
                owner: owner.to_string(),
                repo: repo.to_string(),
                target_branch: pr.head.ref_name.clone(),
            };

            let scoped_calc = self.version_calculator.scoped_to(installation_id);
            let calc_result = match scoped_calc
                .calculate_version(ctx, strategy.clone(), CalculationOptions::default())
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        pr = pr.number,
                        "Failed to project version for PR refresh; skipping PR"
                    );
                    continue;
                }
            };

            let body = pr_status_commenter::render_feature_pr_comment(
                &calc_result.next_version,
                &base_version,
                queued_release_version.as_ref(),
                repo_config.versioning.allow_override,
            );

            if let Err(e) = pr_status_commenter::upsert_pr_status_comment(
                &scoped_github,
                owner,
                repo,
                pr.number,
                &body,
            )
            .await
            {
                tracing::warn!(
                    error = %e,
                    pr = pr.number,
                    "Failed to refresh PR status comment; skipping PR"
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
