//! Factory functions for creating [`ReleaseRegentProcessor`] instances.
//!
//! This module provides two creation paths:
//!
//! * **Mock mode** ([`create_mock_processor`]) — wires all four dependencies with
//!   in-process mocks from the `release-regent-testing` crate.  Useful during
//!   development and local integration tests where no real GitHub credentials are
//!   needed.
//!
//! * **Production mode** ([`create_production_processor`]) — reads GitHub App
//!   credentials from environment variables and connects to the real GitHub API.

use async_trait::async_trait;
use release_regent_core::{traits::WebhookValidator, CoreResult, ReleaseRegentProcessor};
use release_regent_testing::mocks::{
    MockConfigurationProvider, MockGitHubOperations, MockVersionCalculator, MockWebhookValidator,
};
use tracing::info;

use crate::{
    errors::{CliError, CliResult},
    version_calculator::DefaultVersionCalculator,
};

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;

// ──────────────────────────────────────────────────────────────────────────────
// PassThroughWebhookValidator
// ──────────────────────────────────────────────────────────────────────────────

/// Webhook validator that unconditionally approves every request.
///
/// Used in the CLI where webhooks are loaded from local files and there is
/// no actual HTTP signature to verify.  This must **never** be used in a
/// deployed service that receives webhooks from the internet.
#[derive(Debug, Default)]
pub struct PassThroughWebhookValidator;

impl PassThroughWebhookValidator {
    /// Create a new pass-through validator.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl WebhookValidator for PassThroughWebhookValidator {
    /// Always returns `Ok(true)` — signature checking is skipped for local files.
    async fn verify_signature(
        &self,
        _payload: &[u8],
        _signature: &str,
        _secret: &str,
    ) -> CoreResult<bool> {
        todo!("implement verify_signature")
    }

    /// Always returns `Ok(true)` — payload structure validation is skipped.
    async fn validate_payload(
        &self,
        _payload: &serde_json::Value,
        _event_type: &str,
    ) -> CoreResult<bool> {
        todo!("implement validate_payload")
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Type aliases for the two processor flavours
// ──────────────────────────────────────────────────────────────────────────────

/// Type alias for the all-mock processor returned by [`create_mock_processor`].
pub type MockProcessor = ReleaseRegentProcessor<
    MockGitHubOperations,
    MockConfigurationProvider,
    MockVersionCalculator,
    MockWebhookValidator,
>;

/// Type alias for the production processor returned by [`create_production_processor`].
pub type ProductionProcessor = ReleaseRegentProcessor<
    release_regent_github_client::GitHubClient,
    release_regent_config_provider::FileConfigurationProvider,
    DefaultVersionCalculator,
    PassThroughWebhookValidator,
>;

// ──────────────────────────────────────────────────────────────────────────────
// Factory functions
// ──────────────────────────────────────────────────────────────────────────────

/// Create a processor that uses all-mock dependencies.
///
/// All four trait slots are filled with configurable in-process mocks from the
/// `release-regent-testing` crate.  The mocks return pre-configured responses
/// and track every call for assertion in tests.
///
/// # Use cases
///
/// * Local development without GitHub credentials (`rr run --mock …`)
/// * Integration testing in CI where GitHub access is unavailable
/// * Demonstration and onboarding scenarios
pub fn create_mock_processor() -> MockProcessor {
    todo!("implement create_mock_processor")
}

/// Create a processor that uses production dependencies sourced from environment variables.
///
/// Reads the following environment variables:
///
/// | Variable | Description |
/// |---|---|
/// | `GITHUB_APP_ID` | Numeric GitHub App installation ID |
/// | `GITHUB_PRIVATE_KEY` | PEM-encoded private key of the GitHub App |
/// | `GITHUB_WEBHOOK_SECRET` | Secret used to sign webhooks (may be empty) |
/// | `GITHUB_INSTALLATION_ID` | Installation ID for the target repository |
///
/// # Errors
///
/// Returns [`CliError::MissingDependency`] when a required environment variable is absent.
/// Returns [`CliError::ConfigFile`] when GitHub authentication initialisation fails.
/// Returns [`CliError::Core`] when the `FileConfigurationProvider` cannot access the
/// current directory.
pub async fn create_production_processor() -> CliResult<ProductionProcessor> {
    todo!("implement create_production_processor")
}
