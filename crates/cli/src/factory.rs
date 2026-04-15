//! Factory functions for creating [`ReleaseRegentProcessor`] instances.
//!
//! This module provides two creation paths:
//!
//! * **Mock mode** ([`create_mock_processor`]) — wires all three dependencies with
//!   in-process mocks from the `release-regent-testing` crate.  Useful during
//!   development and local integration tests where no real GitHub credentials are
//!   needed.
//!
//! * **Production mode** ([`create_production_processor`]) — reads GitHub App
//!   credentials from environment variables and connects to the real GitHub API.

use release_regent_core::ReleaseRegentProcessor;
use release_regent_testing::mocks::{
    MockConfigurationProvider, MockGitHubOperations, MockVersionCalculator,
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
// Type aliases for the two processor flavours
// ──────────────────────────────────────────────────────────────────────────────

/// Type alias for the all-mock processor returned by [`create_mock_processor`].
pub type MockProcessor =
    ReleaseRegentProcessor<MockGitHubOperations, MockConfigurationProvider, MockVersionCalculator>;

/// Type alias for the production processor returned by [`create_production_processor`].
pub type ProductionProcessor = ReleaseRegentProcessor<
    release_regent_github_client::GitHubClient,
    release_regent_config_provider::FileConfigurationProvider,
    DefaultVersionCalculator,
>;

// ──────────────────────────────────────────────────────────────────────────────
// Factory functions
// ──────────────────────────────────────────────────────────────────────────────

/// Create a processor that uses all-mock dependencies.
///
/// All three trait slots are filled with configurable in-process mocks from the
/// `release-regent-testing` crate.  The mocks return pre-configured responses
/// and track every call for assertion in tests.
///
/// # Use cases
///
/// * Local development without GitHub credentials (`rr run --mock …`)
/// * Integration testing in CI where GitHub access is unavailable
/// * Demonstration and onboarding scenarios
pub fn create_mock_processor() -> MockProcessor {
    info!("Creating mock processor for local development");
    let github_ops = MockGitHubOperations::new();
    let config_provider = MockConfigurationProvider::new();
    let version_calc = MockVersionCalculator::new();
    ReleaseRegentProcessor::new(github_ops, config_provider, version_calc)
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
    info!("Creating production processor from environment variables");

    let app_id: u64 = std::env::var("GITHUB_APP_ID")
        .map_err(|_| CliError::missing_dependency("GITHUB_APP_ID", "environment variable not set"))?
        .parse::<u64>()
        .map_err(|e| {
            CliError::invalid_argument("GITHUB_APP_ID", format!("Must be a number: {e}"))
        })?;

    let private_key = std::env::var("GITHUB_PRIVATE_KEY").map_err(|_| {
        CliError::missing_dependency("GITHUB_PRIVATE_KEY", "environment variable not set")
    })?;

    // Webhook secret is optional — an empty string disables signature validation.
    let webhook_secret = std::env::var("GITHUB_WEBHOOK_SECRET").unwrap_or_default();

    let installation_id: u64 = std::env::var("GITHUB_INSTALLATION_ID")
        .map_err(|_| {
            CliError::missing_dependency("GITHUB_INSTALLATION_ID", "environment variable not set")
        })?
        .parse::<u64>()
        .map_err(|e| {
            CliError::invalid_argument("GITHUB_INSTALLATION_ID", format!("Must be a number: {e}"))
        })?;

    let auth_config = release_regent_github_client::AuthConfig {
        app_id,
        private_key,
        webhook_secret,
    };

    let github_client =
        release_regent_github_client::GitHubClient::from_config(auth_config, installation_id)?;

    let config_dir = std::env::current_dir().map_err(|e| {
        CliError::command_execution(
            "current_dir",
            format!("Failed to get working directory: {e}"),
        )
    })?;

    let config_provider =
        release_regent_config_provider::FileConfigurationProvider::new(config_dir).await?;

    let version_calculator = DefaultVersionCalculator::new();

    Ok(ReleaseRegentProcessor::new(
        github_client,
        config_provider,
        version_calculator,
    ))
}
