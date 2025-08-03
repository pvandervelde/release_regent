//! Core business logic for Release Regent
//!
//! This crate contains the main business logic for Release Regent, including configuration
//! management, versioning strategies, and webhook processing.

pub mod changelog;
pub mod config;
pub mod errors;
pub mod traits;
pub mod versioning;
pub mod webhook;

pub use errors::{CoreError, CoreResult};
pub use traits::{ConfigurationProvider, GitHubOperations, VersionCalculator, WebhookValidator};

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
/// ```no_run
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
    /// ```no_run
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

        // Get commits since last release using GitHub operations
        // For now, we'll compare from the base branch to the merge commit
        let base_ref = repository.default_branch.clone();
        let head_ref = pull_request
            .merge_commit_sha
            .clone()
            .unwrap_or_else(|| pull_request.head.clone());

        let commits = self
            .github_operations
            .compare_commits(
                &repository.owner,
                &repository.name,
                &base_ref,
                &head_ref,
                None, // per_page
                None, // page
            )
            .await
            .map_err(|e| {
                tracing::error!(
                    "Failed to compare commits from {} to {}: {}",
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
