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
//!                             MockVersionCalculator, MockWebhookValidator};
//!
//! let processor = ReleaseRegentProcessor::new(
//!     MockGitHubOperations::new(),     // GitHub API operations
//!     MockConfigurationProvider::new(), // Configuration loading
//!     MockVersionCalculator::new(),     // Version calculation
//!     MockWebhookValidator::new(),      // Webhook validation
//! );
//! ```
//!
//! ### 2. **Webhook Processing Pipeline**
//!
//! The [`webhook`] module handles GitHub webhook events with validation and processing:
//!
//! - **Event Validation**: Signature verification and payload validation
//! - **Event Parsing**: Extract repository and pull request information
//! - **Business Logic Routing**: Route to appropriate handlers based on event type
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
//! use release_regent_core::{ReleaseRegentProcessor, webhook::WebhookEvent};
//! use release_regent_github_client::GitHubClient;
//! use release_regent_config_provider::FileConfigurationProvider;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let github_client = GitHubClient::new("github_token".to_string())?;
//!     let config_provider = FileConfigurationProvider::new("./config")?;
//!     let version_calculator = MyVersionCalculator::new();
//!     let webhook_validator = MyWebhookValidator::new("webhook_secret");
//!
//!     let processor = ReleaseRegentProcessor::new(
//!         github_client,
//!         config_provider,
//!         version_calculator,
//!         webhook_validator,
//!     );
//!
//!     // Process incoming webhook
//!     let webhook_event = WebhookEvent::from_json(&payload_json)?;
//!     processor.process_webhook(webhook_event).await?;
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
pub mod config;
pub mod errors;
pub mod traits;
pub mod versioning;
pub mod webhook;

pub use errors::{CoreError, CoreResult};
pub use traits::{
    ConfigurationProvider, GitHubOperations, GitOperations, VersionCalculator, WebhookValidator,
};

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
/// use release_regent_core::run_event_loop;
/// use tokio_util::sync::CancellationToken;
///
/// let token = CancellationToken::new();
/// let source = MyEventSource::new();
/// run_event_loop(&source, token).await?;
/// ```
pub async fn run_event_loop<S>(
    source: &S,
    token: tokio_util::sync::CancellationToken,
) -> CoreResult<()>
where
    S: traits::event_source::EventSource,
{
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
                let _entered = span.enter();

                let dispatch_result: CoreResult<()> = match &event.event_type {
                    EventType::PullRequestMerged => {
                        tracing::info!(
                            event_id = %event.event_id,
                            repository = %format!(
                                "{}/{}",
                                event.repository.owner, event.repository.name
                            ),
                            "Pull request merged — release orchestrator not yet wired"
                        );
                        Ok(())
                    }
                    EventType::ReleasePrMerged => {
                        tracing::info!(
                            event_id = %event.event_id,
                            repository = %format!(
                                "{}/{}",
                                event.repository.owner, event.repository.name
                            ),
                            "Release PR merged — release automator not yet wired"
                        );
                        Ok(())
                    }
                    EventType::PullRequestCommentReceived => {
                        tracing::debug!(
                            event_id = %event.event_id,
                            "Pull request comment received — no handler yet"
                        );
                        Ok(())
                    }
                    EventType::Unknown(raw) => {
                        tracing::debug!(
                            event_id = %event.event_id,
                            raw_type = %raw,
                            "Unknown event type; dropping"
                        );
                        Ok(())
                    }
                };

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
    pub fn new(config: config::ReleaseRegentConfig) -> Self {
        Self { config }
    }

    /// Process a webhook event
    ///
    /// # Arguments
    /// * `event` - The webhook event to process
    pub async fn process_webhook(&self, event: webhook::WebhookEvent) -> CoreResult<()> {
        tracing::info!("Processing webhook event: {:?}", event.event_type());

        // TODO: Implement webhook processing pipeline
        // This will be implemented in subsequent issues

        Ok(())
    }

    /// Get the current configuration
    pub fn config(&self) -> &config::ReleaseRegentConfig {
        &self.config
    }
}

/// Release Regent processor with dependency injection
///
/// This is the main business logic processor that uses dependency injection
/// for all external services. It separates webhook processing from GitHub operations
/// and enables comprehensive testing through trait abstractions.
///
/// # Type Parameters
/// * `G` - GitHub operations implementation
/// * `C` - Configuration provider implementation
/// * `V` - Version calculator implementation
/// * `W` - Webhook validator implementation
///
/// # Examples
/// ```ignore
/// use release_regent_core::{ReleaseRegentProcessor, config::ReleaseRegentConfig};
/// use release_regent_testing::{MockGitHubOperations, MockConfigurationProvider, MockVersionCalculator, MockWebhookValidator};
///
/// let github_ops = MockGitHubOperations::new();
/// let config_provider = MockConfigurationProvider::new();
/// let version_calc = MockVersionCalculator::new();
/// let webhook_validator = MockWebhookValidator::new();
///
/// let processor = ReleaseRegentProcessor::new(github_ops, config_provider, version_calc, webhook_validator);
/// ```
#[derive(Debug)]
pub struct ReleaseRegentProcessor<G, C, V, W>
where
    G: GitHubOperations,
    C: ConfigurationProvider,
    V: VersionCalculator,
    W: WebhookValidator,
{
    github_operations: G,
    configuration_provider: C,
    version_calculator: V,
    webhook_validator: W,
}

impl<G, C, V, W> ReleaseRegentProcessor<G, C, V, W>
where
    G: GitHubOperations,
    C: ConfigurationProvider,
    V: VersionCalculator,
    W: WebhookValidator,
{
    /// Create a new Release Regent processor with injected dependencies
    ///
    /// # Arguments
    /// * `github_operations` - GitHub API operations implementation
    /// * `configuration_provider` - Configuration loading implementation
    /// * `version_calculator` - Version calculation implementation
    /// * `webhook_validator` - Webhook validation implementation
    ///
    /// # Examples
    /// ```ignore
    /// use release_regent_core::ReleaseRegentProcessor;
    /// use release_regent_testing::{MockGitHubOperations, MockConfigurationProvider, MockVersionCalculator, MockWebhookValidator};
    ///
    /// let github_ops = MockGitHubOperations::new();
    /// let config_provider = MockConfigurationProvider::new();
    /// let version_calc = MockVersionCalculator::new();
    /// let webhook_validator = MockWebhookValidator::new();
    ///
    /// let processor = ReleaseRegentProcessor::new(github_ops, config_provider, version_calc, webhook_validator);
    /// ```
    pub fn new(
        github_operations: G,
        configuration_provider: C,
        version_calculator: V,
        webhook_validator: W,
    ) -> Self {
        Self {
            github_operations,
            configuration_provider,
            version_calculator,
            webhook_validator,
        }
    }

    /// Process a webhook event with full business logic
    ///
    /// This method coordinates the complete webhook processing workflow:
    /// 1. Load configuration for the repository
    /// 2. Process the webhook event
    /// 3. Calculate new version if needed
    /// 4. Create release via GitHub operations
    ///
    /// # Arguments
    /// * `event` - The webhook event to process
    ///
    /// # Returns
    /// Result indicating success or failure of processing
    ///
    /// # Errors
    /// * `CoreError::Configuration` - Configuration loading failed
    /// * `CoreError::GitHub` - GitHub API operations failed
    /// * `CoreError::Versioning` - Version calculation failed
    /// * `CoreError::Webhook` - Webhook processing failed
    pub async fn process_webhook(&self, event: webhook::WebhookEvent) -> CoreResult<()> {
        tracing::info!("Processing webhook event: {:?}", event.event_type());

        // Process the webhook event to extract relevant information
        let processing_result = self.process_webhook_event(&event).await.map_err(|e| {
            tracing::error!("Failed to process webhook event: {}", e);
            e
        })?;

        // If we have a result to process, handle it
        if let Some(result) = processing_result {
            self.handle_processing_result(result).await.map_err(|e| {
                tracing::error!("Failed to handle processing result: {}", e);
                e
            })?;
        }

        tracing::info!("Successfully processed webhook event");
        Ok(())
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

    /// Get a reference to the webhook validator
    pub fn webhook_validator(&self) -> &W {
        &self.webhook_validator
    }

    /// Process webhook event and extract actionable information
    ///
    /// This method handles the webhook event parsing and validation,
    /// separating webhook concerns from business logic.
    async fn process_webhook_event(
        &self,
        event: &webhook::WebhookEvent,
    ) -> CoreResult<Option<webhook::ProcessingResult>> {
        // Create webhook processor with injected validator
        let webhook_processor = webhook::WebhookProcessor::new(&self.webhook_validator, None);
        webhook_processor.process_event(event).await
    }

    /// Handle the processing result from webhook event
    ///
    /// This method coordinates the business logic for each type of processing result,
    /// using the injected dependencies for all external operations.
    async fn handle_processing_result(&self, result: webhook::ProcessingResult) -> CoreResult<()> {
        match result {
            webhook::ProcessingResult::MergedPullRequest {
                repository,
                pull_request,
            } => {
                self.handle_merged_pull_request(repository, pull_request)
                    .await
            }
        }
    }

    /// Handle a merged pull request
    ///
    /// This method implements the complete workflow for processing a merged PR:
    /// 1. Load repository configuration
    /// 2. Get commits since last release
    /// 3. Calculate new version
    /// 4. Create release
    async fn handle_merged_pull_request(
        &self,
        repository: webhook::RepositoryInfo,
        pull_request: webhook::PullRequestInfo,
    ) -> CoreResult<()> {
        tracing::info!(
            "Handling merged PR #{} in {}/{}",
            pull_request.number,
            repository.owner,
            repository.name
        );

        // Load configuration for this repository
        let load_options = traits::configuration_provider::LoadOptions::default();
        let repo_config = self
            .configuration_provider
            .load_repository_config(&repository.owner, &repository.name, load_options.clone())
            .await
            .map_err(|e| {
                tracing::error!("Failed to load repository configuration: {}", e);
                e
            })?;

        // Get merged configuration (global + repository-specific)
        let _merged_config = self
            .configuration_provider
            .get_merged_config(&repository.owner, &repository.name, load_options)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get merged configuration: {}", e);
                e
            })?;

        tracing::debug!(
            "Loaded configuration for {}/{}: {:?}",
            repository.owner,
            repository.name,
            repo_config.is_some()
        );

        // Get commits since last release using Git operations
        // For now, we'll compare from the base branch to the merge commit
        let base_ref = repository.default_branch.clone();
        let head_ref = pull_request
            .merge_commit_sha
            .clone()
            .unwrap_or_else(|| pull_request.head.clone());

        let commits = self
            .github_operations
            .get_commits_between(
                &repository.owner,
                &repository.name,
                &base_ref,
                &head_ref,
                crate::traits::git_operations::GetCommitsOptions::default(),
            )
            .await
            .map_err(|e| {
                tracing::error!(
                    "Failed to get commits between {} and {}: {}",
                    base_ref,
                    head_ref,
                    e
                );
                e
            })?;

        tracing::debug!("Found {} commits since last release", commits.len());

        // Calculate new version using version calculator
        let version_context = traits::version_calculator::VersionContext {
            base_ref: Some(base_ref),
            current_version: None, // TODO: Get current version from tags
            head_ref,
            owner: repository.owner.clone(),
            repo: repository.name.clone(),
            target_branch: repository.default_branch.clone(),
        };

        // Use conventional commits strategy as default
        let strategy = traits::version_calculator::VersioningStrategy::ConventionalCommits {
            custom_types: std::collections::HashMap::new(),
            include_prerelease: false,
        };

        let options = traits::version_calculator::CalculationOptions {
            generate_changelog: true,
            validate: true,
            ..Default::default()
        };

        let version_result = self
            .version_calculator
            .calculate_version(version_context, strategy, options)
            .await
            .map_err(|e| {
                tracing::error!("Failed to calculate new version: {}", e);
                e
            })?;

        tracing::info!("Calculated new version: {}", version_result.next_version);

        // TODO: Create release using GitHub operations
        // TODO: Create tag and release notes

        tracing::info!("Successfully processed merged PR #{}", pull_request.number);
        Ok(())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
