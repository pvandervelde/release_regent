//! Generic retry-with-exponential-backoff helper for transient `CoreError`s.
//!
//! See [`retry_with_backoff`] for the full contract. This module exists to
//! close GitHub issue #138: `ErrorHandlingConfig` (`max_retries`,
//! `backoff_multiplier`, `initial_delay_ms`) was parsed and merged but never
//! consulted by any call site, so transient GitHub API failures (rate limits,
//! network blips, timeouts, optimistic-lock conflicts) were never retried.

use crate::config::ErrorHandlingConfig;
use crate::errors::CoreResult;
use rand::RngExt;

/// Hard ceiling on the backoff delay between retry attempts, in
/// milliseconds (30 seconds), matching the "Maximum delay" design ceiling
/// documented in `docs/specs/design/error-handling.md`.
///
/// `ErrorHandlingConfig` (`backoff_multiplier`, `initial_delay_ms`,
/// `max_retries`) is attacker-controllable (a repository owner can set
/// arbitrary values via `.release-regent.toml`, per ADR-007). Without this
/// cap, a pathological `backoff_multiplier` can make the computed delay
/// overflow to `f64::INFINITY` (which panics `Duration::from_secs_f64`) or
/// produce a finite-but-unreasonable delay spanning hours or days, hanging
/// event processing. This constant is enforced unconditionally in
/// [`retry_with_backoff`], regardless of what values are supplied in
/// `ErrorHandlingConfig`.
///
/// This cap alone is not sufficient: an attacker who cannot make a single
/// delay unreasonably long could instead set an enormous `max_retries` (see
/// [`MAX_RETRIES`]) to achieve the same denial-of-service outcome via many
/// capped-length delays instead of one huge one.
const MAX_DELAY_MS: f64 = 30_000.0;

/// Hard ceiling on the number of retries attempted, regardless of what
/// `config.max_retries` requests.
///
/// `config.max_retries` is a plain `u32` sourced from attacker-controllable
/// `.release-regent.toml` (ADR-007) with no upper bound of its own. Combined
/// with the existing per-attempt [`MAX_DELAY_MS`] cap (30 seconds), an
/// unbounded `max_retries` (e.g. `1_000_000`) could still block event
/// processing for roughly 347 days (`1_000_000 * 30s`), even though no
/// single delay ever exceeds the cap. This constant closes that gap by
/// bounding the retry *count*, independent of the per-delay cap.
const MAX_RETRIES: u32 = 20;

/// Retries `operation` while it returns a retryable `CoreError`
/// (`CoreError::is_retryable() == true`), honoring `config`'s
/// `max_retries`, `backoff_multiplier`, and `initial_delay_ms`.
///
/// Delay before retry attempt `n` (1-based) = `initial_delay_ms *
/// backoff_multiplier.powi(n - 1)` milliseconds, **capped at
/// [`MAX_DELAY_MS`] (30 seconds)** regardless of the config values supplied,
/// after which **±25% jitter is applied** (per the "Jitter" requirement in
/// `docs/specs/design/error-handling.md`) and the result is re-clamped into
/// `[0.0, MAX_DELAY_MS]`. Jitter prevents a thundering-herd retry storm: many
/// repositories rate-limited by the same GitHub quota at the same instant
/// would otherwise retry in lockstep. The per-delay cap is a defense-in-depth
/// safety measure: it protects against pathological or attacker-controlled
/// `ErrorHandlingConfig` values (e.g. an extreme, negative, or non-finite
/// `backoff_multiplier`) that would otherwise panic `Duration::from_secs_f64`
/// (which rejects infinite, NaN, and negative input) or produce an
/// unreasonably long, event-loop-hanging delay.
///
/// `config.max_retries` is likewise capped at [`MAX_RETRIES`] (20): total
/// attempts = `min(config.max_retries, MAX_RETRIES) + 1` (initial attempt +
/// up to the effective number of retries). Non-retryable errors are returned
/// immediately without delay.
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
/// non-retryable error is returned, or the effective retry limit
/// (`min(config.max_retries, MAX_RETRIES)`) has been exhausted.
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
    let effective_max_retries = config.max_retries.min(MAX_RETRIES);

    loop {
        let err = match f().await {
            Ok(value) => return Ok(value),
            Err(err) => err,
        };

        if attempt >= effective_max_retries || !err.is_retryable() {
            return Err(err);
        }

        attempt += 1;
        let raw_delay_ms =
            config.initial_delay_ms as f64 * config.backoff_multiplier.powi((attempt - 1) as i32);

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

        // Apply ±25% jitter (per `docs/specs/design/error-handling.md`) to
        // prevent a thundering-herd retry storm, then re-clamp: jitter can
        // push the value up to 1.25x `delay_ms`, which could exceed
        // `MAX_DELAY_MS` when `delay_ms` was already at or near the cap.
        // The RNG is scoped to this block (rather than bound for the rest
        // of the loop iteration) so it is dropped before the `.await` below:
        // `rand::rng()`'s thread-local RNG is `!Send`, and holding it live
        // across an await point would make the enclosing future `!Send`.
        let jitter_range = delay_ms * 0.25;
        let jittered_delay_ms = {
            let mut rng = rand::rng();
            (delay_ms + rng.random_range(-jitter_range..=jitter_range)).clamp(0.0, MAX_DELAY_MS)
        };

        tracing::warn!(
            operation_name = %operation_name,
            correlation_id = correlation_id.unwrap_or_default(),
            attempt,
            delay_ms = jittered_delay_ms,
            error = %err,
            "retrying operation after transient failure"
        );

        tokio::time::sleep(std::time::Duration::from_secs_f64(
            jittered_delay_ms / 1000.0,
        ))
        .await;
    }
}

#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;
