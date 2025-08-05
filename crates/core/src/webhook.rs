//! Webhook processing for Release Regent
//!
//! This module handles GitHub webhook events and processes them for release management.

use crate::{traits::WebhookValidator, CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

/// GitHub webhook event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    /// Event type (e.g., "pull_request")
    pub event_type: String,
    /// Action within the event (e.g., "closed")
    pub action: String,
    /// Event payload
    pub payload: serde_json::Value,
    /// Webhook headers
    pub headers: HashMap<String, String>,
}

/// Pull request information from webhook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestInfo {
    /// PR number
    pub number: u64,
    /// PR title
    pub title: String,
    /// PR body
    pub body: String,
    /// Base branch
    pub base: String,
    /// Head branch
    pub head: String,
    /// Whether the PR was merged
    pub merged: bool,
    /// Merge commit SHA (if merged)
    pub merge_commit_sha: Option<String>,
}

/// Repository information from webhook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryInfo {
    /// Repository owner
    pub owner: String,
    /// Repository name
    pub name: String,
    /// Default branch
    pub default_branch: String,
}

/// Result of processing a webhook event
#[derive(Debug, Clone)]
pub enum ProcessingResult {
    /// A pull request was merged and should trigger release processing
    MergedPullRequest {
        repository: RepositoryInfo,
        pull_request: PullRequestInfo,
    },
}

/// Webhook processor with dependency injection
///
/// This processor uses trait abstractions for all external dependencies,
/// enabling comprehensive testing and separation of concerns.
pub struct WebhookProcessor<'a, W>
where
    W: WebhookValidator,
{
    webhook_validator: &'a W,
    webhook_secret: Option<String>,
}

impl<'a, W> WebhookProcessor<'a, W>
where
    W: WebhookValidator,
{
    /// Create a new webhook processor with injected dependencies
    ///
    /// # Arguments
    /// * `webhook_validator` - Webhook validation implementation
    /// * `webhook_secret` - Optional secret for validating webhook signatures
    pub fn new(webhook_validator: &'a W, webhook_secret: Option<String>) -> Self {
        Self {
            webhook_validator,
            webhook_secret,
        }
    }

    /// Process a webhook event
    ///
    /// # Arguments
    /// * `event` - The webhook event to process
    pub async fn process_event(
        &self,
        event: &WebhookEvent,
    ) -> CoreResult<Option<ProcessingResult>> {
        info!(
            "Processing webhook event: {} - {}",
            event.event_type, event.action
        );

        // Validate webhook signature if secret is configured
        if let Some(secret) = &self.webhook_secret {
            self.validate_signature(event, secret).await?;
        }

        match event.event_type.as_str() {
            "pull_request" => self.process_pull_request_event(event).await,
            _ => {
                debug!("Ignoring unsupported event type: {}", event.event_type);
                Ok(None)
            }
        }
    }

    /// Process pull request events
    async fn process_pull_request_event(
        &self,
        event: &WebhookEvent,
    ) -> CoreResult<Option<ProcessingResult>> {
        debug!(
            "Processing pull request event with action: {}",
            event.action
        );

        match event.action.as_str() {
            "closed" => {
                let pr_info = self.extract_pull_request_info(&event.payload)?;
                let repo_info = self.extract_repository_info(&event.payload)?;

                if pr_info.merged {
                    info!(
                        "Processing merged PR #{} in {}/{}",
                        pr_info.number, repo_info.owner, repo_info.name
                    );

                    Ok(Some(ProcessingResult::MergedPullRequest {
                        repository: repo_info,
                        pull_request: pr_info,
                    }))
                } else {
                    debug!("PR was closed but not merged, ignoring");
                    Ok(None)
                }
            }
            _ => {
                debug!("Ignoring PR action: {}", event.action);
                Ok(None)
            }
        }
    }

    /// Validate webhook signature using the injected validator
    async fn validate_signature(&self, event: &WebhookEvent, secret: &str) -> CoreResult<()> {
        debug!("Validating webhook signature");

        // Get signature from headers
        let signature = event
            .headers
            .get("x-hub-signature-256")
            .or_else(|| event.headers.get("X-Hub-Signature-256"))
            .ok_or_else(|| {
                CoreError::webhook("signature_validation", "Missing signature header")
            })?;

        // Get the raw payload for signature verification
        let payload = serde_json::to_vec(&event.payload).map_err(|e| {
            CoreError::webhook(
                "signature_validation",
                &format!("Failed to serialize payload for validation: {}", e),
            )
        })?;

        // Use injected webhook validator for signature verification
        let is_valid = self
            .webhook_validator
            .verify_signature(&payload, signature, secret)
            .await?;

        if !is_valid {
            return Err(CoreError::webhook(
                "signature_validation",
                "Invalid webhook signature",
            ));
        }

        debug!("Webhook signature validation passed");
        Ok(())
    }

    /// Extract pull request information from payload
    fn extract_pull_request_info(
        &self,
        _payload: &serde_json::Value,
    ) -> CoreResult<PullRequestInfo> {
        debug!("Extracting pull request information from payload");

        // TODO: Implement actual payload parsing
        // This will be implemented in subsequent issues

        Ok(PullRequestInfo {
            number: 1, // Placeholder
            title: "Test PR".to_string(),
            body: "Test PR body".to_string(),
            base: "main".to_string(),
            head: "feature-branch".to_string(),
            merged: true,
            merge_commit_sha: Some("abc123".to_string()),
        })
    }

    /// Extract repository information from payload
    fn extract_repository_info(&self, _payload: &serde_json::Value) -> CoreResult<RepositoryInfo> {
        debug!("Extracting repository information from payload");

        // TODO: Implement actual payload parsing
        // This will be implemented in subsequent issues

        Ok(RepositoryInfo {
            owner: "owner".to_string(),
            name: "repo".to_string(),
            default_branch: "main".to_string(),
        })
    }
}

impl WebhookEvent {
    /// Get the event type
    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    /// Get the action
    pub fn action(&self) -> &str {
        &self.action
    }

    /// Create a new webhook event
    pub fn new(
        event_type: String,
        action: String,
        payload: serde_json::Value,
        headers: HashMap<String, String>,
    ) -> Self {
        Self {
            event_type,
            action,
            payload,
            headers,
        }
    }
}

#[cfg(test)]
#[path = "webhook_tests.rs"]
mod tests;
