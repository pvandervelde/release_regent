//! Generic retry-with-exponential-backoff helper for transient `CoreError`s.
//!
//! See [`retry_with_backoff`] for the full contract. This module exists to
//! close GitHub issue #138: `ErrorHandlingConfig` (`max_retries`,
//! `backoff_multiplier`, `initial_delay_ms`) was parsed and merged but never
//! consulted by any call site, so transient GitHub API failures (rate limits,
//! network blips, timeouts, optimistic-lock conflicts) were never retried.
//!
//! # Status
//!
//! This is a signature-only stub (TDD RED phase). The body is intentionally
//! `todo!()` — the Coder replaces it in the GREEN phase. Do not call this
//! function from production code until the implementation lands.

use crate::config::ErrorHandlingConfig;
use crate::errors::CoreResult;

/// Retries `operation` while it returns a retryable `CoreError`
/// (`CoreError::is_retryable() == true`), honoring `config`'s
/// `max_retries`, `backoff_multiplier`, and `initial_delay_ms`.
///
/// Delay before retry attempt `n` (1-based) = `initial_delay_ms *
/// backoff_multiplier.powi(n - 1)` milliseconds.
///
/// Total attempts = `max_retries + 1` (initial attempt + up to `max_retries`
/// retries). Non-retryable errors are returned immediately without delay.
///
/// # Parameters
/// - `config`: Retry policy (max retries, backoff multiplier, initial delay).
/// - `operation_name`: Human-readable operation name, used for structured
///   logging only; does not affect retry/success semantics.
/// - `correlation_id`: Optional tracing correlation ID, used for structured
///   logging only; does not affect retry/success semantics.
/// - `f`: The fallible async operation to attempt, invoked at least once.
///
/// # Errors
///
/// Returns the last error produced by `operation` once either a
/// non-retryable error is returned, or `max_retries` retries have been
/// exhausted.
pub async fn retry_with_backoff<T, F, Fut>(
    _config: &ErrorHandlingConfig,
    _operation_name: &str,
    _correlation_id: Option<&str>,
    _f: F,
) -> CoreResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = CoreResult<T>>,
{
    todo!("retry_with_backoff is not yet implemented — see GitHub issue #138 (GREEN phase)")
}

#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;
