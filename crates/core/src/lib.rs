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
pub mod release_automator;
pub mod release_orchestrator;
pub mod traits;
pub mod versioning;

pub use default_version_calculator::DefaultVersionCalculator;
pub use errors::{CoreError, CoreResult};
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

        let correlation_id = &event.correlation_id;
        let owner = &event.repository.owner;
        let repo = &event.repository.name;

        // `branch_prefix` and `changelog_header` are not yet per-repo config
        // fields in `ReleaseRegentConfig`. Use the same default source as
        // `OrchestratorConfig` so both components stay in sync. When the schema
        // gains an explicit `release.branch_prefix` field, load it here via
        // `self.configuration_provider.get_merged_config(...)`.
        let config = AutomatorConfig {
            branch_prefix: release_orchestrator::OrchestratorConfig::default().branch_prefix,
            changelog_header: "## Changelog".to_string(),
        };

        match ReleaseAutomator::new(config, &self.github_operations.scoped_to(event.installation_id))
            .automate(owner, repo, event, correlation_id)
            .await
        {
            Ok(result) => {
                tracing::info!(result = ?result, "Release automation completed");
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
            .get_merged_config(owner, repo, LoadOptions::default())
            .await?;

        let config = CommentCommandConfig {
            orchestrator_config: release_orchestrator::OrchestratorConfig {
                branch_prefix: release_orchestrator::OrchestratorConfig::default().branch_prefix,
                title_template: repo_config.release_pr.title_template.clone(),
                changelog_header: "## Changelog".to_string(),
            },
            allow_override: repo_config.versioning.allow_override,
        };

        CommentCommandProcessor::new(config, &self.github_operations.scoped_to(event.installation_id))
            .process(event)
            .await
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// run_event_loop — public API
// ─────────────────────────────────────────────────────────────────────────────

/// Drive the event processing loop until `token` is cancelled.
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

        // The merge commit SHA is required: it is the head of the base branch
        // immediately after the merge and serves as the branch point for the
        // new release branch.
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

        let MergeCalcResult {
            calc_result,
            changelog,
            current_version,
            repo_config,
        } = self
            .calculate_version_for_merge(owner, repo, &base_sha, &base_branch)
            .await?;

        // Build orchestrator config honouring the repository PR title template.
        let orch_config = release_orchestrator::OrchestratorConfig {
            branch_prefix: release_orchestrator::OrchestratorConfig::DEFAULT_BRANCH_PREFIX
                .to_string(),
            title_template: repo_config.release_pr.title_template.clone(),
            changelog_header: "## Changelog".to_string(),
        };

        // Determine whether the merged PR is itself a release PR by checking
        // whether its head branch starts with the configured release prefix + "/v".
        let merged_pr_head_ref = event
            .payload
            .get("pull_request")
            .and_then(|pr| pr.get("head"))
            .and_then(|h| h.get("ref"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let is_release_pr =
            merged_pr_head_ref.starts_with(&format!("{}/v", orch_config.branch_prefix));

        // Resolve the merged PR number (needed to read override labels on the
        // feature-PR path, and logged for diagnostics on the release-PR path).
        let merged_pr_number: u64 = event
            .payload
            .get("pull_request")
            .and_then(|pr| pr.get("number"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        let scoped_github = self.github_operations.scoped_to(event.installation_id);
        let orchestrator =
            release_orchestrator::ReleaseOrchestrator::new(orch_config, &scoped_github);

        if is_release_pr {
            self.process_release_pr_merged(
                owner,
                repo,
                correlation_id,
                &orchestrator,
                &calc_result.next_version,
                &changelog,
                &base_branch,
                &base_sha,
            )
            .await
        } else {
            self.process_feature_pr_merged(
                owner,
                repo,
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
    }

    /// Load configuration and calculate the next version for a merge event.
    async fn calculate_version_for_merge(
        &self,
        owner: &str,
        repo: &str,
        base_sha: &str,
        base_branch: &str,
    ) -> CoreResult<MergeCalcResult> {
        use traits::configuration_provider::LoadOptions;
        use traits::version_calculator::{CalculationOptions, VersionContext, VersioningStrategy};

        let repo_config = self
            .configuration_provider
            .get_merged_config(owner, repo, LoadOptions::default())
            .await?;

        let current_version =
            versioning::resolve_current_version(&self.github_operations, owner, repo, false)
                .await?;

        let ctx = VersionContext {
            base_ref: current_version.as_ref().map(|v| format!("v{v}")),
            current_version: current_version.clone(),
            head_ref: base_sha.to_string(),
            owner: owner.to_string(),
            repo: repo.to_string(),
            target_branch: base_branch.to_string(),
        };

        let strategy = match repo_config.versioning.strategy {
            config::VersioningStrategy::Conventional | config::VersioningStrategy::External => {
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

        let calc_result = self
            .version_calculator
            .calculate_version(ctx, strategy, options)
            .await?;

        let changelog = format_changelog_for_release(&calc_result.changelog_entries);

        Ok(MergeCalcResult {
            calc_result,
            changelog,
            current_version,
            repo_config,
        })
    }

    /// Handle the release-PR path after a merged pull request.
    ///
    /// Orchestrates the next release cycle and clears stale bump-override labels
    /// from open feature PRs that were scoped to the completed release.
    #[allow(clippy::too_many_arguments)] // owner/repo/correlation/orchestrator/version/changelog/branch/sha is the minimal surface
    async fn process_release_pr_merged(
        &self,
        owner: &str,
        repo: &str,
        correlation_id: &str,
        orchestrator: &release_orchestrator::ReleaseOrchestrator<'_, G>,
        version: &versioning::SemanticVersion,
        changelog: &str,
        base_branch: &str,
        base_sha: &str,
    ) -> CoreResult<release_orchestrator::OrchestratorResult> {
        tracing::info!(
            owner = %owner,
            repo = %repo,
            version = %version,
            base_branch = %base_branch,
            correlation_id = %correlation_id,
            "Orchestrating for merged release PR"
        );

        let orch_result = orchestrator
            .orchestrate(
                owner,
                repo,
                version,
                changelog,
                base_branch,
                base_sha,
                correlation_id,
            )
            .await?;

        // After a release is published, clear stale rr:override-* labels from
        // any open feature PRs. Overrides were scoped to this release cycle.
        self.clear_stale_override_labels_after_release(owner, repo, correlation_id)
            .await;

        Ok(orch_result)
    }

    /// Clear stale bump-override labels from open feature PRs after a release.
    async fn clear_stale_override_labels_after_release(
        &self,
        owner: &str,
        repo: &str,
        correlation_id: &str,
    ) {
        use comment_command_processor::ALL_OVERRIDE_LABELS;

        for &label_name in ALL_OVERRIDE_LABELS {
            let query = format!("is:open label:{label_name}");
            match self
                .github_operations
                .search_pull_requests(owner, repo, &query)
                .await
            {
                Ok(stale_prs) => {
                    for stale_pr in stale_prs {
                        if let Err(e) = self
                            .github_operations
                            .remove_label(owner, repo, stale_pr.number, label_name)
                            .await
                        {
                            tracing::warn!(
                                error = %e,
                                pr = stale_pr.number,
                                label = label_name,
                                correlation_id = %correlation_id,
                                "Failed to remove stale override label; continuing"
                            );
                        }
                        let kind_str = label_name
                            .strip_prefix("rr:override-")
                            .unwrap_or(label_name);
                        let cleanup_body = format!(
                            "ℹ️ **Release Regent**: The `!release {kind_str}` override on \
                             this PR has been cleared because a new release was published \
                             before this PR merged. If the work in this PR still warrants \
                             a minimum bump for the next release, please re-post your \
                             `!release` command."
                        );
                        if let Err(e) = self
                            .github_operations
                            .create_issue_comment(owner, repo, stale_pr.number, &cleanup_body)
                            .await
                        {
                            tracing::warn!(
                                error = %e,
                                pr = stale_pr.number,
                                correlation_id = %correlation_id,
                                "Failed to post stale-override cleanup comment; continuing"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        label = label_name,
                        correlation_id = %correlation_id,
                        "Failed to search for PRs with stale override label; continuing"
                    );
                }
            }
        }
    }

    /// Handle the feature-PR path after a merged pull request.
    ///
    /// Applies any bump-floor override from the merged PR's labels, orchestrates
    /// the release PR, posts an audit comment if the floor was applied, and
    /// removes the consumed override labels.
    #[allow(clippy::too_many_arguments)] // owner/repo/pr_num/correlation/orchestrator/current/calc/changelog/branch/sha is minimal
    async fn process_feature_pr_merged(
        &self,
        owner: &str,
        repo: &str,
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

        // Read any rr:override-* label from the merged PR and apply it as a
        // minimum-bump floor before calling the orchestrator.
        let labels = if merged_pr_number > 0 {
            self.github_operations
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
                if let Err(e) = self
                    .github_operations
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
    #[allow(clippy::too_many_arguments)] // audit context requires all 8 data points; no good grouping
    async fn post_bump_floor_audit_comment(
        &self,
        owner: &str,
        repo: &str,
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
        if let Err(e) = self
            .github_operations
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
}

/// Format [`traits::version_calculator::ChangelogEntry`] items into a markdown
/// body suitable for a release PR.
///
/// Entries are grouped by [`ChangelogEntry::entry_type`] (e.g. "Added",
/// "Fixed") and sorted alphabetically within each group. Each entry line
/// includes the full 40-character commit SHA in `[sha]` notation so that the
/// [`release_orchestrator`] changelog merge/dedup logic can identify duplicates.
fn format_changelog_for_release(entries: &[traits::version_calculator::ChangelogEntry]) -> String {
    use std::collections::BTreeMap;
    use std::fmt::Write as _;

    if entries.is_empty() {
        return String::new();
    }

    // Group entries by type preserving the alphabetical section order from BTreeMap.
    let mut by_type: BTreeMap<&str, Vec<&traits::version_calculator::ChangelogEntry>> =
        BTreeMap::new();
    for entry in entries {
        by_type
            .entry(entry.entry_type.as_str())
            .or_default()
            .push(entry);
    }

    let mut out = String::new();
    for (entry_type, items) in &by_type {
        let _ = write!(out, "### {entry_type}\n\n");
        for item in items {
            let desc = if let Some(scope) = &item.scope {
                format!("**{scope}**: {}", item.description)
            } else {
                item.description.clone()
            };
            let line = if item.commit_sha.is_empty() {
                format!("- {desc}")
            } else {
                format!("- {desc} [{}]", item.commit_sha)
            };
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }

    out.trim_end().to_string()
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
