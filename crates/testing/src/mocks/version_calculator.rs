//! Mock implementation of VersionCalculator trait
//!
//! Provides a comprehensive mock implementation for testing version calculation
//! without requiring actual commit analysis or external version calculation.

use crate::mocks::{CallResult, MockConfig, MockState, SharedMockState};
use async_trait::async_trait;
use release_regent_core::{
    traits::version_calculator::*, versioning::SemanticVersion, CoreError, CoreResult,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock implementation of VersionCalculator trait
///
/// This mock supports:
/// - Pre-configured version calculation results
/// - Deterministic version calculation for testing
/// - Commit analysis simulation
/// - Changelog generation simulation
/// - Version validation simulation
/// - Multiple versioning strategy support
///
/// # Example Usage
///
/// ```rust
/// use release_regent_testing::mocks::MockVersionCalculator;
/// use release_regent_core::versioning::SemanticVersion;
///
/// let mock = MockVersionCalculator::new()
///     .with_next_version(SemanticVersion::new(1, 2, 3))
///     .with_version_bump(VersionBump::Minor);
/// ```
#[derive(Debug)]
pub struct MockVersionCalculator {
    /// Shared state for tracking and configuration
    state: SharedMockState,
    /// Pre-configured version calculation results
    calculation_results: HashMap<String, VersionCalculationResult>,
    /// Pre-configured next versions for different contexts
    next_versions: HashMap<String, SemanticVersion>,
    /// Pre-configured version bumps
    version_bumps: HashMap<String, VersionBump>,
    /// Pre-configured commit analyses
    commit_analyses: HashMap<String, Vec<CommitAnalysis>>,
    /// Pre-configured changelog entries
    changelog_entries: HashMap<String, Vec<ChangelogEntry>>,
    /// Default version to return when none configured
    default_next_version: SemanticVersion,
    /// Default version bump to return when none configured
    default_version_bump: VersionBump,
}

impl MockVersionCalculator {
    /// Create a new mock with default configuration
    ///
    /// Returns a mock configured for basic testing scenarios with:
    /// - Deterministic behavior enabled
    /// - Call tracking enabled
    /// - Default version 1.0.0
    /// - Default minor version bump
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(MockState::new())),
            calculation_results: HashMap::new(),
            next_versions: HashMap::new(),
            version_bumps: HashMap::new(),
            commit_analyses: HashMap::new(),
            changelog_entries: HashMap::new(),
            default_next_version: SemanticVersion {
                major: 1,
                minor: 0,
                patch: 0,
                prerelease: None,
                build: None,
            },
            default_version_bump: VersionBump::Minor,
        }
    }

    /// Create a new mock with custom configuration
    ///
    /// # Parameters
    /// - `config`: Mock behavior configuration
    ///
    /// # Returns
    /// Configured mock instance
    pub fn with_config(config: MockConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(MockState::with_config(config))),
            calculation_results: HashMap::new(),
            next_versions: HashMap::new(),
            version_bumps: HashMap::new(),
            commit_analyses: HashMap::new(),
            changelog_entries: HashMap::new(),
            default_next_version: SemanticVersion {
                major: 1,
                minor: 0,
                patch: 0,
                prerelease: None,
                build: None,
            },
            default_version_bump: VersionBump::Minor,
        }
    }

    /// Configure the mock to return a specific version calculation result
    ///
    /// # Parameters
    /// - `context_key`: Unique key for the version context
    /// - `result`: Version calculation result to return
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_calculation_result(
        mut self,
        context_key: &str,
        result: VersionCalculationResult,
    ) -> Self {
        self.calculation_results
            .insert(context_key.to_string(), result);
        self
    }

    /// Configure the mock to return a specific next version
    ///
    /// # Parameters
    /// - `version`: Next version to return
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_next_version(mut self, version: SemanticVersion) -> Self {
        self.default_next_version = version;
        self
    }

    /// Configure the mock to return a specific version bump
    ///
    /// # Parameters
    /// - `bump`: Version bump to return
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_version_bump(mut self, bump: VersionBump) -> Self {
        self.default_version_bump = bump;
        self
    }

    /// Configure the mock with commit analyses for a context
    ///
    /// # Parameters
    /// - `context_key`: Unique key for the version context
    /// - `analyses`: Commit analyses to return
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_commit_analyses(
        mut self,
        context_key: &str,
        analyses: Vec<CommitAnalysis>,
    ) -> Self {
        self.commit_analyses
            .insert(context_key.to_string(), analyses);
        self
    }

    /// Configure the mock with changelog entries for a context
    ///
    /// # Parameters
    /// - `context_key`: Unique key for the version context
    /// - `entries`: Changelog entries to return
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_changelog_entries(
        mut self,
        context_key: &str,
        entries: Vec<ChangelogEntry>,
    ) -> Self {
        self.changelog_entries
            .insert(context_key.to_string(), entries);
        self
    }

    /// Get the call history for verification
    ///
    /// # Returns
    /// Reference to all recorded method calls
    pub async fn call_history(&self) -> Vec<crate::mocks::CallInfo> {
        self.state.read().await.call_history().to_vec()
    }

    /// Get the total number of calls made
    ///
    /// # Returns
    /// Total call count
    pub async fn call_count(&self) -> u64 {
        self.state.read().await.call_count()
    }

    /// Record a method call for tracking
    async fn record_call(&self, method: &str, parameters: &str, result: CallResult) {
        self.state
            .write()
            .await
            .record_call(method, parameters, result);
    }

    /// Check if quota has been exceeded
    async fn check_quota(&self) -> CoreResult<()> {
        if self.state.read().await.is_quota_exceeded() {
            return Err(CoreError::RateLimit {
                message: "Mock quota exceeded".to_string(),
                retry_after_seconds: None,
                context: None,
            });
        }
        Ok(())
    }

    /// Simulate latency if configured
    async fn simulate_latency(&self) {
        self.state.read().await.simulate_latency().await;
    }

    /// Check if should simulate failure
    async fn should_simulate_failure(&self) -> bool {
        self.state.read().await.should_simulate_failure()
    }

    /// Create a context key from version context
    fn create_context_key(&self, context: &VersionContext) -> String {
        format!(
            "{}/{}/{}",
            context.owner, context.repo, context.target_branch
        )
    }

    /// Create a default calculation result
    fn create_default_calculation_result(
        &self,
        context: &VersionContext,
        strategy: &VersioningStrategy,
    ) -> VersionCalculationResult {
        VersionCalculationResult {
            next_version: self.default_next_version.clone(),
            current_version: context.current_version.clone(),
            version_bump: self.default_version_bump.clone(),
            strategy: strategy.clone(),
            analyzed_commits: vec![],
            changelog_entries: vec![],
            is_prerelease: false,
            build_metadata: None,
            metadata: HashMap::new(),
        }
    }
}

impl Default for MockVersionCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VersionCalculator for MockVersionCalculator {
    /// Calculate next version based on commit history
    ///
    /// Returns the pre-configured calculation result or a default result.
    ///
    /// # Parameters
    /// - `context`: Version calculation context
    /// - `strategy`: Versioning strategy to use
    /// - `options`: Calculation options
    ///
    /// # Returns
    /// Version calculation result
    ///
    /// # Errors
    /// - `CoreError::Versioning` - Simulated version calculation error
    async fn calculate_version(
        &self,
        context: VersionContext,
        strategy: VersioningStrategy,
        _options: CalculationOptions,
    ) -> CoreResult<VersionCalculationResult> {
        let method = "calculate_version";
        let params = format!(
            "context={}/{}, strategy={:?}",
            context.owner, context.repo, strategy
        );

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::versioning("Simulated version calculation error");
            self.record_call(method, &params, CallResult::Error(error.to_string()));
            return Err(error);
        }

        let context_key = self.create_context_key(&context);
        let result = self
            .calculation_results
            .get(&context_key)
            .cloned()
            .unwrap_or_else(|| self.create_default_calculation_result(&context, &strategy));

        self.record_call(method, &params, CallResult::Success);
        Ok(result)
    }

    /// Analyze commits for version impact
    ///
    /// Returns the pre-configured commit analyses or empty list.
    ///
    /// # Parameters
    /// - `context`: Version calculation context
    /// - `strategy`: Versioning strategy for analysis
    /// - `commit_shas`: Specific commit SHAs to analyze
    ///
    /// # Returns
    /// List of commit analyses
    ///
    /// # Errors
    /// - `CoreError::Versioning` - Simulated commit analysis error
    async fn analyze_commits(
        &self,
        context: VersionContext,
        _strategy: VersioningStrategy,
        commit_shas: Vec<String>,
    ) -> CoreResult<Vec<CommitAnalysis>> {
        let method = "analyze_commits";
        let params = format!(
            "context={}/{}, commits={}",
            context.owner,
            context.repo,
            commit_shas.len()
        );

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::versioning("Simulated commit analysis error");
            self.record_call(method, &params, CallResult::Error(error.to_string()));
            return Err(error);
        }

        let context_key = self.create_context_key(&context);
        let analyses = self
            .commit_analyses
            .get(&context_key)
            .cloned()
            .unwrap_or_default();

        self.record_call(method, &params, CallResult::Success);
        Ok(analyses)
    }

    /// Validate a proposed version
    ///
    /// Returns true if validation succeeds, false otherwise.
    ///
    /// # Parameters
    /// - `context`: Version calculation context
    /// - `proposed_version`: Version to validate
    /// - `rules`: Validation rules to apply
    ///
    /// # Returns
    /// True if version is valid
    ///
    /// # Errors
    /// - `CoreError::Versioning` - Simulated validation error
    async fn validate_version(
        &self,
        context: VersionContext,
        proposed_version: SemanticVersion,
        _rules: ValidationRules,
    ) -> CoreResult<bool> {
        let method = "validate_version";
        let params = format!(
            "context={}/{}, version={}",
            context.owner, context.repo, proposed_version
        );

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::versioning("Simulated version validation error");
            self.record_call(method, &params, CallResult::Error(error.to_string()));
            return Err(error);
        }

        // Default to valid version
        let is_valid = true;
        self.record_call(method, &params, CallResult::Success);
        Ok(is_valid)
    }

    /// Get suggested version bump for a set of changes
    ///
    /// Returns the pre-configured version bump or default bump.
    ///
    /// # Parameters
    /// - `context`: Version calculation context
    /// - `strategy`: Versioning strategy to use
    /// - `commit_analyses`: Pre-analyzed commits
    ///
    /// # Returns
    /// Suggested version bump type
    ///
    /// # Errors
    /// - `CoreError::Versioning` - Simulated version bump determination error
    async fn get_version_bump(
        &self,
        context: VersionContext,
        _strategy: VersioningStrategy,
        _commit_analyses: Vec<CommitAnalysis>,
    ) -> CoreResult<VersionBump> {
        let method = "get_version_bump";
        let params = format!("context={}/{}", context.owner, context.repo);

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::versioning("Simulated version bump error");
            self.record_call(method, &params, CallResult::Error(error.to_string()));
            return Err(error);
        }

        let context_key = self.create_context_key(&context);
        let bump = self
            .version_bumps
            .get(&context_key)
            .cloned()
            .unwrap_or(self.default_version_bump.clone());

        self.record_call(method, &params, CallResult::Success);
        Ok(bump)
    }

    /// Generate changelog entries for commits
    ///
    /// Returns the pre-configured changelog entries or empty list.
    ///
    /// # Parameters
    /// - `context`: Version calculation context
    /// - `strategy`: Versioning strategy for changelog generation
    /// - `commit_analyses`: Analyzed commits to generate entries for
    /// - `version`: Target version for the changelog
    ///
    /// # Returns
    /// List of structured changelog entries
    ///
    /// # Errors
    /// - `CoreError::ChangelogGeneration` - Simulated changelog generation error
    async fn generate_changelog_entries(
        &self,
        context: VersionContext,
        _strategy: VersioningStrategy,
        _commit_analyses: Vec<CommitAnalysis>,
        version: SemanticVersion,
    ) -> CoreResult<Vec<ChangelogEntry>> {
        let method = "generate_changelog_entries";
        let params = format!(
            "context={}/{}, version={}",
            context.owner, context.repo, version
        );

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::changelog_generation("Simulated changelog generation error");
            self.record_call(method, &params, CallResult::Error(error.to_string()));
            return Err(error);
        }

        let context_key = self.create_context_key(&context);
        let entries = self
            .changelog_entries
            .get(&context_key)
            .cloned()
            .unwrap_or_default();

        self.record_call(method, &params, CallResult::Success);
        Ok(entries)
    }

    /// Preview version calculation without side effects
    ///
    /// Returns the same result as calculate_version with dry_run forced to true.
    ///
    /// # Parameters
    /// - `context`: Version calculation context
    /// - `strategy`: Versioning strategy to preview
    /// - `options`: Calculation options (dry_run forced to true)
    ///
    /// # Returns
    /// Preview of version calculation result
    ///
    /// # Errors
    /// - `CoreError::Versioning` - Simulated preview calculation error
    async fn preview_calculation(
        &self,
        context: VersionContext,
        strategy: VersioningStrategy,
        mut options: CalculationOptions,
    ) -> CoreResult<VersionCalculationResult> {
        // Force dry_run for preview
        options.dry_run = true;

        let method = "preview_calculation";
        let params = format!(
            "context={}/{}, strategy={:?}",
            context.owner, context.repo, strategy
        );

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::versioning("Simulated preview calculation error");
            self.record_call(method, &params, CallResult::Error(error.to_string()));
            return Err(error);
        }

        // Reuse calculate_version logic
        let result = self.calculate_version(context, strategy, options).await?;

        self.record_call(method, &params, CallResult::Success);
        Ok(result)
    }

    /// Get supported versioning strategies
    ///
    /// Returns a list of supported strategies.
    ///
    /// # Returns
    /// Map of strategy names to descriptions
    fn supported_strategies(&self) -> HashMap<String, String> {
        let mut strategies = HashMap::new();
        strategies.insert(
            "ConventionalCommits".to_string(),
            "Semantic versioning with conventional commits".to_string(),
        );
        strategies.insert(
            "CalendarVersion".to_string(),
            "Calendar-based versioning".to_string(),
        );
        strategies.insert(
            "External".to_string(),
            "Custom versioning using external command".to_string(),
        );
        strategies.insert("Manual".to_string(), "Manual versioning".to_string());
        strategies
    }

    /// Get default versioning strategy
    ///
    /// Returns the default ConventionalCommits strategy.
    ///
    /// # Returns
    /// Default versioning strategy configuration
    fn default_strategy(&self) -> VersioningStrategy {
        VersioningStrategy::ConventionalCommits {
            custom_types: HashMap::new(),
            include_prerelease: false,
        }
    }

    /// Parse conventional commit message
    ///
    /// Returns a mock commit analysis for any input.
    ///
    /// # Parameters
    /// - `commit_message`: Raw commit message to parse
    ///
    /// # Returns
    /// Parsed commit information, or None if not conventional format
    ///
    /// # Errors
    /// - `CoreError::InvalidInput` - Simulated parsing error
    fn parse_conventional_commit(
        &self,
        commit_message: &str,
    ) -> CoreResult<Option<CommitAnalysis>> {
        // Simple mock implementation that treats any message starting with known types as conventional
        let conventional_types = ["feat", "fix", "docs", "style", "refactor", "test", "chore"];

        for commit_type in &conventional_types {
            if commit_message.starts_with(&format!("{}:", commit_type)) {
                return Ok(Some(CommitAnalysis {
                    sha: "mock_sha".to_string(),
                    author: "mock_author".to_string(),
                    date: chrono::Utc::now(),
                    message: commit_message.to_string(),
                    commit_type: Some(commit_type.to_string()),
                    scope: None,
                    is_breaking: commit_message.contains("BREAKING CHANGE"),
                    version_bump: if commit_message.contains("BREAKING CHANGE") {
                        VersionBump::Major
                    } else if *commit_type == "feat" {
                        VersionBump::Minor
                    } else if *commit_type == "fix" {
                        VersionBump::Patch
                    } else {
                        VersionBump::None
                    },
                    metadata: HashMap::new(),
                }));
            }
        }

        Ok(None)
    }

    /// Apply version bump to existing version
    ///
    /// Applies the specified version bump to the current version.
    ///
    /// # Parameters
    /// - `current_version`: Current version to bump
    /// - `bump_type`: Type of version bump to apply
    /// - `prerelease`: Optional pre-release identifier
    /// - `build`: Optional build metadata
    ///
    /// # Returns
    /// New version with bump applied
    ///
    /// # Errors
    /// - `CoreError::Versioning` - Invalid version bump operation
    fn apply_version_bump(
        &self,
        current_version: SemanticVersion,
        bump_type: VersionBump,
        prerelease: Option<String>,
        build: Option<String>,
    ) -> CoreResult<SemanticVersion> {
        let mut new_version = match bump_type {
            VersionBump::Major => SemanticVersion {
                major: current_version.major + 1,
                minor: 0,
                patch: 0,
                prerelease: None,
                build: None,
            },
            VersionBump::Minor => SemanticVersion {
                major: current_version.major,
                minor: current_version.minor + 1,
                patch: 0,
                prerelease: None,
                build: None,
            },
            VersionBump::Patch => SemanticVersion {
                major: current_version.major,
                minor: current_version.minor,
                patch: current_version.patch + 1,
                prerelease: None,
                build: None,
            },
            VersionBump::None => current_version,
        };

        // Apply prerelease and build metadata if provided
        new_version.prerelease = prerelease;
        new_version.build = build;

        Ok(new_version)
    }
}
