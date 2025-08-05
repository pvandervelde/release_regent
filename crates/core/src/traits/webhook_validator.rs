//! Webhook validation trait
//!
//! This trait defines the contract for validating webhook signatures and payloads.
//! It abstracts the underlying signature verification to enable testing and
//! provide a stable interface.

use crate::CoreResult;
use async_trait::async_trait;

/// Webhook validation operations contract
///
/// This trait defines webhook validation operations required by Release Regent.
/// Implementations must handle signature verification and payload validation.
///
/// # Error Handling
///
/// All methods return `CoreResult<T>` and must properly map validation errors
/// to `CoreError` variants. Common error scenarios include:
/// - Invalid signatures
/// - Missing headers
/// - Malformed payloads
/// - Configuration errors
///
/// # Security
///
/// Implementations must use secure comparison methods to prevent timing attacks
/// when validating signatures.
#[async_trait]
pub trait WebhookValidator: Send + Sync {
    /// Verify webhook signature
    ///
    /// # Parameters
    /// - `payload`: Raw webhook payload bytes
    /// - `signature`: Signature from webhook headers (e.g., X-Hub-Signature-256)
    /// - `secret`: Webhook secret for verification
    ///
    /// # Returns
    /// `true` if signature is valid, `false` otherwise
    ///
    /// # Errors
    /// - `CoreError::Webhook` - Signature verification failed
    /// - `CoreError::InvalidInput` - Invalid signature format or missing data
    async fn verify_signature(
        &self,
        payload: &[u8],
        signature: &str,
        secret: &str,
    ) -> CoreResult<bool>;

    /// Validate webhook payload structure
    ///
    /// # Parameters
    /// - `payload`: JSON payload to validate
    /// - `event_type`: GitHub event type (e.g., "pull_request")
    ///
    /// # Returns
    /// `true` if payload is valid for the event type
    ///
    /// # Errors
    /// - `CoreError::Webhook` - Payload validation failed
    /// - `CoreError::InvalidInput` - Invalid JSON or missing required fields
    async fn validate_payload(
        &self,
        payload: &serde_json::Value,
        event_type: &str,
    ) -> CoreResult<bool>;
}
