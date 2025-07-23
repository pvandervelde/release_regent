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

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
