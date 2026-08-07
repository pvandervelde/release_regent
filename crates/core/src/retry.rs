//! Generic retry-with-exponential-backoff helper for transient `CoreError`s.
//!
//! See [`retry_with_backoff`] for the full contract. This module exists to
//! close GitHub issue #138: `ErrorHandlingConfig` (`max_retries`,
//! `backoff_multiplier`, `initial_delay_ms`) was parsed and merged but never
//! consulted by any call site, so transient GitHub API failures (rate limits,
//! network blips, timeouts, optimistic-lock conflicts) were never retried.

use crate::config::ErrorHandlingConfig;
use crate::errors::CoreResult;

/// Hard ceiling on the backoff delay between retry attempts, in
/// milliseconds (30 seconds), matching the "Maximum delay" design ceiling
/// documented in `docs/specs/design/error-handling.md`.
///
/// `ErrorHandlingConfig` (`backoff_multiplier`, `initial_delay_ms`) is
/// attacker-controllable (a repository owner can set arbitrary values via
/// `.release-regent.toml`, per ADR-007). Without this cap, a pathological
/// `backoff_multiplier` can make the computed delay overflow to
/// `f64::INFINITY` (which panics `Duration::from_secs_f64`) or produce a
/// finite-but-unreasonable delay spanning hours or days, hanging event
/// processing. This constant is enforced unconditionally in
/// [`retry_with_backoff`], regardless of what values are supplied in
/// `ErrorHandlingConfig`.
const MAX_DELAY_MS: f64 = 30_000.0;

/// Retries `operation` while it returns a retryable `CoreError`
/// (`CoreError::is_retryable() == true`), honoring `config`'s
/// `max_retries`, `backoff_multiplier`, and `initial_delay_ms`.
///
/// Delay before retry attempt `n` (1-based) = `initial_delay_ms *
/// backoff_multiplier.powi(n - 1)` milliseconds, **capped at
/// [`MAX_DELAY_MS`] (30 seconds)** regardless of the config values supplied.
/// This cap is a defense-in-depth safety measure: it protects against
/// pathological or attacker-controlled `ErrorHandlingConfig` values (e.g. an
/// extreme, negative, or non-finite `backoff_multiplier`) that would
/// otherwise panic `Duration::from_secs_f64` (which rejects infinite, NaN,
/// and negative input) or produce an unreasonably long, event-loop-hanging
/// delay.
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
    config: &ErrorHandlingConfig,
    operation_name: &str,
    correlation_id: Option<&str>,
    mut f: F,
) -> CoreResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = CoreResult<T>>,
{
    let mut attempt: u32 = 0;

    loop {
        let err = match f().await {
            Ok(value) => return Ok(value),
            Err(err) => err,
        };

        if attempt >= config.max_retries || !err.is_retryable() {
            return Err(err);
        }

        attempt += 1;
        let raw_delay_ms = config.initial_delay_ms as f64
            * config.backoff_multiplier.powi((attempt - 1) as i32);

        // Defense-in-depth: `raw_delay_ms` is derived from
        // attacker-controllable config (`backoff_multiplier`,
        // `initial_delay_ms`). Non-finite results (overflow to infinity, or
        // NaN) are clamped to the max delay directly, since `f64::clamp`
        // leaves NaN untouched and would otherwise flow into
        // `Duration::from_secs_f64`, which panics on infinite, NaN, or
        // negative input. Finite values (including negative multiplier
        // results and finite-but-huge values) are clamped into
        // `[0.0, MAX_DELAY_MS]` in one step.
        let delay_ms = if raw_delay_ms.is_finite() {
            raw_delay_ms.clamp(0.0, MAX_DELAY_MS)
        } else {
            MAX_DELAY_MS
        };

        tracing::warn!(
            operation_name = %operation_name,
            correlation_id = correlation_id.unwrap_or_default(),
            attempt,
            delay_ms,
            error = %err,
            "retrying operation after transient failure"
        );

        tokio::time::sleep(std::time::Duration::from_secs_f64(delay_ms / 1000.0)).await;
    }
}

#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;
