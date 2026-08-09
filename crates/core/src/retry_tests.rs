//! Test suite for [`retry_with_backoff`] (GitHub issue #138).
//!
//! ## Tier coverage
//!
//! - **Tier 1 (specification)**: one test per acceptance criterion in the task
//!   spec (AC1-AC9) plus an explicit BA-18 end-to-end test.
//! - **Tier 2 (adversarial)**: boundary values (`max_retries` = 0, 1), every
//!   retryable/non-retryable `CoreError` variant, side-effect verification
//!   (call counts via `AtomicU32`), exact-error-propagation, and structured
//!   logging passthrough via `tracing_test`.
//! - **Tier 3 (property-based)**: `proptest` invariants over `max_retries`,
//!   `backoff_multiplier`, and `initial_delay_ms` ranges, run under a paused
//!   Tokio clock so no test incurs real wall-clock delay.
//!
//! ## Paused-clock timing tests
//!
//! Tests that need to observe *elapsed* backoff delay use
//! `#[tokio::test(start_paused = true)]` together with `tokio::time::Instant`.
//! Under a paused clock, Tokio auto-advances virtual time to the next timer
//! deadline whenever every task is blocked solely on a timer, so these tests
//! run instantly in real wall-clock time while still exercising the genuine
//! `tokio::time::sleep` calls inside the implementation.
//!
//! This requires the `test-util` feature of the `tokio` crate. The workspace
//! `tokio` dependency only enables `"full"` (which does not itself include
//! `"test-util"`), but `crates/core/Cargo.toml` already depends on
//! `tokio-test = { workspace = true }` as a dev-dependency, and `tokio-test`
//! transitively requires `tokio/test-util`. Cargo's feature unification
//! enables that feature for the single resolved `tokio` package used by this
//! crate's test target, so `#[tokio::test(start_paused = true)]` and
//! `tokio::runtime::Builder::start_paused` are usable here today without any
//! `Cargo.toml` changes (verified by `cargo check -p release-regent-core
//! --tests`).

use super::*;
use crate::errors::CoreError;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing_test::traced_test;

// ─────────────────────────────────────────────────────────────────────────────
// Fixtures
// ─────────────────────────────────────────────────────────────────────────────

/// Builds an `ErrorHandlingConfig` with explicit, test-controlled values so
/// tests never depend on (and are never broken by) changes to the crate's
/// `Default` values.
fn config_with(
    max_retries: u32,
    backoff_multiplier: f64,
    initial_delay_ms: u64,
) -> ErrorHandlingConfig {
    ErrorHandlingConfig {
        max_retries,
        backoff_multiplier,
        initial_delay_ms,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tier 1 — Specification tests (AC1-AC9, BA-18)
// ─────────────────────────────────────────────────────────────────────────────

/// AC7: a first-attempt success returns `Ok` immediately, closure invoked
/// exactly once.
#[tokio::test]
async fn test_retry_with_backoff_first_attempt_success_returns_ok_once() {
    let config = config_with(5, 2.0, 1000);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<&'static str> =
        retry_with_backoff(&config, "first-attempt-success", None, move || {
            let calls = Arc::clone(&calls_clone);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok("success-value")
            }
        })
        .await;

    assert_eq!(result.expect("expected Ok"), "success-value");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "operation must be invoked exactly once when it succeeds on the first try"
    );
}

/// AC8: a success after N transient failures (N < max_retries) returns `Ok`
/// with the closure invoked N+1 times.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_succeeds_after_transient_failures_below_max_retries() {
    let config = config_with(5, 2.0, 10);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);
    let fail_count = 2u32; // N = 2, max_retries = 5, so N < max_retries

    let result: CoreResult<u32> =
        retry_with_backoff(&config, "succeeds-after-failures", None, move || {
            let calls = Arc::clone(&calls_clone);
            async move {
                let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt <= fail_count {
                    Err(CoreError::network(format!("transient failure #{attempt}")))
                } else {
                    Ok(attempt)
                }
            }
        })
        .await;

    assert_eq!(result.expect("expected eventual Ok"), fail_count + 1);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        fail_count + 1,
        "operation must be invoked exactly N+1 times (N failures then one success)"
    );
}

/// AC3: retryable errors are retried until `max_retries` is exhausted, after
/// which the last error is returned. Total attempts = max_retries + 1.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_exhausts_max_retries_and_returns_last_error() {
    let config = config_with(3, 2.0, 10);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> = retry_with_backoff(&config, "always-fails", None, move || {
        let calls = Arc::clone(&calls_clone);
        async move {
            let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
            Err(CoreError::network(format!("failure #{attempt}")))
        }
    })
    .await;

    assert!(result.is_err(), "expected exhaustion to return Err");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        4, // max_retries (3) + 1 initial attempt
        "total attempts must equal max_retries + 1"
    );
}

/// AC2 / AC3: the exact *last* error (not the first) is propagated after
/// exhaustion — a stub that caches the first error instead of the last would
/// fail this test.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_propagates_the_last_error_not_the_first() {
    let config = config_with(1, 2.0, 10); // 2 total attempts
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> =
        retry_with_backoff(&config, "distinguish-last-error", None, move || {
            let calls = Arc::clone(&calls_clone);
            async move {
                let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
                Err(CoreError::network(format!("attempt-{attempt}-failure")))
            }
        })
        .await;

    let err = result.expect_err("expected Err after exhausting retries");
    assert!(
        err.to_string().contains("attempt-2-failure"),
        "expected the LAST attempt's error message, got: {err}"
    );
    assert!(
        !err.to_string().contains("attempt-1-failure"),
        "must not propagate the first attempt's error message, got: {err}"
    );
}

/// AC2: a `NotFound` error (non-retryable) returns immediately on the first
/// failure with the exact error and no further attempts.
#[tokio::test]
async fn test_retry_with_backoff_not_found_error_returns_immediately() {
    let config = config_with(5, 2.0, 1000);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> = retry_with_backoff(&config, "not-found-op", None, move || {
        let calls = Arc::clone(&calls_clone);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(CoreError::not_found("widget-123"))
        }
    })
    .await;

    let err = result.expect_err("NotFound must propagate as Err");
    assert!(
        matches!(err, CoreError::NotFound { ref resource, .. } if resource == "widget-123"),
        "expected the exact NotFound error to be propagated, got: {err:?}"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "non-retryable errors must not be retried"
    );
}

/// AC2: an `Authentication` error (non-retryable) returns immediately.
#[tokio::test]
async fn test_retry_with_backoff_authentication_error_returns_immediately() {
    let config = config_with(5, 2.0, 1000);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> = retry_with_backoff(&config, "auth-op", None, move || {
        let calls = Arc::clone(&calls_clone);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(CoreError::authentication("invalid credentials"))
        }
    })
    .await;

    assert!(matches!(result, Err(CoreError::Authentication { .. })));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// AC2: a `Validation` error (non-retryable) returns immediately.
#[tokio::test]
async fn test_retry_with_backoff_validation_error_returns_immediately() {
    let config = config_with(5, 2.0, 1000);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> = retry_with_backoff(&config, "validation-op", None, move || {
        let calls = Arc::clone(&calls_clone);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(CoreError::validation("email", "must not be empty"))
        }
    })
    .await;

    assert!(matches!(result, Err(CoreError::Validation { .. })));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// AC4: with `max_retries = 0`, exactly one attempt is made for a retryable
/// error.
#[tokio::test]
async fn test_retry_with_backoff_zero_max_retries_makes_one_attempt_on_retryable_error() {
    let config = config_with(0, 2.0, 1000);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> =
        retry_with_backoff(&config, "zero-retries-retryable", None, move || {
            let calls = Arc::clone(&calls_clone);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(CoreError::network("transient"))
            }
        })
        .await;

    assert!(result.is_err());
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "max_retries = 0 must make exactly one attempt even for a retryable error"
    );
}

/// AC4: with `max_retries = 0`, exactly one attempt is made regardless of
/// error type (non-retryable case).
#[tokio::test]
async fn test_retry_with_backoff_zero_max_retries_makes_one_attempt_on_non_retryable_error() {
    let config = config_with(0, 2.0, 1000);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> =
        retry_with_backoff(&config, "zero-retries-non-retryable", None, move || {
            let calls = Arc::clone(&calls_clone);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(CoreError::invalid_input("field", "bad value"))
            }
        })
        .await;

    assert!(result.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// AC9: `correlation_id = None` does not affect success semantics.
#[tokio::test]
async fn test_retry_with_backoff_correlation_id_none_does_not_affect_success() {
    let config = config_with(5, 2.0, 1000);

    let result: CoreResult<u32> =
        retry_with_backoff(&config, "corr-id-none", None, || async { Ok(7) }).await;

    assert_eq!(result.expect("expected Ok"), 7);
}

/// AC9: `correlation_id = Some(..)` does not affect success semantics.
#[tokio::test]
async fn test_retry_with_backoff_correlation_id_some_does_not_affect_success() {
    let config = config_with(5, 2.0, 1000);

    let result: CoreResult<u32> = retry_with_backoff(
        &config,
        "corr-id-some",
        Some("test-correlation-id"),
        || async { Ok(7) },
    )
    .await;

    assert_eq!(result.expect("expected Ok"), 7);
}

/// BA-18 end-to-end: a default `ErrorHandlingConfig` (`max_retries = 5`) used
/// around an always-failing retryable operation results in exactly 6 total
/// attempts (1 initial + 5 retries) before returning `Err`.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_ba18_default_config_makes_six_total_attempts() {
    let config = ErrorHandlingConfig::default();
    assert_eq!(
        config.max_retries, 5,
        "BA-18 assumes the documented default of 5"
    );

    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> =
        retry_with_backoff(&config, "ba-18-default-config", None, move || {
            let calls = Arc::clone(&calls_clone);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(CoreError::rate_limit("secondary rate limit"))
            }
        })
        .await;

    assert!(
        result.is_err(),
        "BA-18: exhausting all retries must return Err"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        6,
        "BA-18: default max_retries=5 must yield 6 total attempts (1 initial + 5 retries)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tier 2 — Adversarial / boundary / stub-killing tests
// ─────────────────────────────────────────────────────────────────────────────

/// Every retryable variant must actually be retried: `RateLimit`.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_rate_limit_error_is_retried() {
    let config = config_with(2, 2.0, 10);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> = retry_with_backoff(&config, "rate-limit-retry", None, move || {
        let calls = Arc::clone(&calls_clone);
        async move {
            let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt < 2 {
                Err(CoreError::rate_limit("quota exceeded"))
            } else {
                Ok(())
            }
        }
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

/// Every retryable variant must actually be retried: `Timeout`.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_timeout_error_is_retried() {
    let config = config_with(2, 2.0, 10);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> = retry_with_backoff(&config, "timeout-retry", None, move || {
        let calls = Arc::clone(&calls_clone);
        async move {
            let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt < 2 {
                Err(CoreError::timeout("fetch", 30_000))
            } else {
                Ok(())
            }
        }
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

/// Every retryable variant must actually be retried: `Conflict`.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_conflict_error_is_retried() {
    let config = config_with(2, 2.0, 10);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> = retry_with_backoff(&config, "conflict-retry", None, move || {
        let calls = Arc::clone(&calls_clone);
        async move {
            let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt < 2 {
                Err(CoreError::conflict("release/v1.2.3"))
            } else {
                Ok(())
            }
        }
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

/// Non-retryable variant coverage distinct from `NotFound`/`Authentication`/
/// `Validation`: `InvalidInput` must not be retried.
#[tokio::test]
async fn test_retry_with_backoff_invalid_input_error_not_retried() {
    let config = config_with(5, 2.0, 1000);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> = retry_with_backoff(&config, "invalid-input-op", None, move || {
        let calls = Arc::clone(&calls_clone);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(CoreError::invalid_input("branch", "must not be empty"))
        }
    })
    .await;

    assert!(matches!(result, Err(CoreError::InvalidInput { .. })));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// Non-retryable variant coverage: `Config` errors must not be retried.
#[tokio::test]
async fn test_retry_with_backoff_config_error_not_retried() {
    let config = config_with(5, 2.0, 1000);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> = retry_with_backoff(&config, "config-error-op", None, move || {
        let calls = Arc::clone(&calls_clone);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(CoreError::config("missing required field"))
        }
    })
    .await;

    assert!(matches!(result, Err(CoreError::Config { .. })));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// Boundary: `max_retries = 1` allows exactly two total attempts (1 initial +
/// 1 retry), not one and not three.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_max_retries_one_allows_exactly_two_attempts() {
    let config = config_with(1, 2.0, 10);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> = retry_with_backoff(&config, "max-retries-one", None, move || {
        let calls = Arc::clone(&calls_clone);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(CoreError::network("always fails"))
        }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "max_retries = 1 must yield exactly 2 total attempts"
    );
}

/// Stub-killing: the operation's returned value must be the value actually
/// produced on the successful attempt, not a hardcoded/default value. Uses a
/// non-trivial, non-Default-looking success payload.
#[tokio::test]
async fn test_retry_with_backoff_returns_actual_operation_output_not_a_default() {
    let config = config_with(5, 2.0, 1000);

    #[derive(Debug, PartialEq, Eq)]
    struct Payload(String);

    let result: CoreResult<Payload> = retry_with_backoff(&config, "payload-op", None, || async {
        Ok(Payload("distinctive-payload-42".to_string()))
    })
    .await;

    assert_eq!(
        result.expect("expected Ok"),
        Payload("distinctive-payload-42".to_string()),
        "a stub returning Default/hardcoded values would fail this assertion"
    );
}

/// Side-effect: after a successful attempt, the operation must not be
/// invoked again (no trailing/extra call after success).
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_does_not_call_operation_again_after_success() {
    let config = config_with(5, 2.0, 10);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> = retry_with_backoff(&config, "no-extra-calls", None, move || {
        let calls = Arc::clone(&calls_clone);
        async move {
            let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt == 1 {
                Err(CoreError::network("one transient failure"))
            } else {
                Ok(())
            }
        }
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "must stop calling the operation immediately after it succeeds"
    );
}

/// Side-effect / paused-clock: a first-attempt success must not incur any
/// backoff delay at all.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_first_attempt_success_incurs_no_delay() {
    let config = config_with(5, 2.0, 5_000); // large delay: any accidental sleep would be obvious
    let start = tokio::time::Instant::now();

    let result: CoreResult<()> =
        retry_with_backoff(&config, "no-delay-on-success", None, || async { Ok(()) }).await;

    assert!(result.is_ok());
    assert!(
        start.elapsed() < Duration::from_millis(1),
        "a successful first attempt must not wait for any backoff delay, elapsed = {:?}",
        start.elapsed()
    );
}

/// Side-effect / paused-clock: a non-retryable error must return with zero
/// delay (no backoff wait before giving up).
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_non_retryable_error_incurs_no_delay() {
    let config = config_with(5, 2.0, 5_000);
    let start = tokio::time::Instant::now();

    let result: CoreResult<()> =
        retry_with_backoff(&config, "no-delay-non-retryable", None, || async {
            Err(CoreError::not_found("thing"))
        })
        .await;

    assert!(result.is_err());
    assert!(
        start.elapsed() < Duration::from_millis(1),
        "non-retryable errors must return immediately with no delay, elapsed = {:?}",
        start.elapsed()
    );
}

/// Paused-clock: cumulative elapsed (simulated) time for an always-failing
/// retryable operation must reflect the exponential backoff schedule implied
/// by `initial_delay_ms` and `backoff_multiplier` (sum of the geometric
/// series for the retries actually taken).
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_cumulative_delay_matches_exponential_schedule() {
    // max_retries=3, initial_delay_ms=100, multiplier=2.0
    // delays: attempt2=100ms, attempt3=200ms, attempt4=400ms => total 700ms
    let config = config_with(3, 2.0, 100);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);
    let start = tokio::time::Instant::now();

    let result: CoreResult<()> =
        retry_with_backoff(&config, "exponential-schedule", None, move || {
            let calls = Arc::clone(&calls_clone);
            async move {
                let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
                Err::<(), _>(CoreError::network(format!("failure #{attempt}")))
            }
        })
        .await;

    assert!(result.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(700),
        "expected cumulative simulated delay >= 700ms (100+200+400), got {elapsed:?}"
    );
    // Generous upper bound to catch a stub that sleeps a fixed/huge duration
    // regardless of config.
    assert!(
        elapsed < Duration::from_millis(2_000),
        "cumulative delay grew far beyond the exponential schedule, got {elapsed:?}"
    );
}

/// Mutation-kill: pins the exact delay of the FIRST retry attempt (n=1) to
/// `initial_delay_ms * backoff_multiplier^0 == initial_delay_ms`, with a tight
/// upper bound. This specifically catches an off-by-one in the exponent
/// (`attempt - 1` vs `attempt`) that the looser cumulative-schedule test
/// (`test_retry_with_backoff_cumulative_delay_matches_exponential_schedule`,
/// bounded `[700ms, 2000ms)`) does not catch: mutating `attempt - 1` to
/// `attempt / 1` (a semantically-identical no-op division) shifts the
/// exponent by one, doubling the first-retry delay from 1000ms to 2000ms,
/// which still fits under that test's generous 2000ms ceiling and survives.
/// cargo-mutants confirmed this exact survivor
/// (`crates/core/src/retry.rs:87:55: replace - with / in retry_with_backoff`)
/// on 2026-08-08; this test closes that gap.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_first_retry_delay_matches_initial_delay_exactly() {
    // max_retries=1, initial_delay_ms=1000, multiplier=2.0.
    // Exponent for the first (and only) retry must be 0, so delay == 1000ms
    // exactly, not 2000ms (which an `attempt - 1` -> `attempt` off-by-one
    // mutation would produce).
    let config = config_with(1, 2.0, 1_000);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);
    let start = tokio::time::Instant::now();

    let result: CoreResult<()> =
        retry_with_backoff(&config, "first-retry-delay-pin", None, move || {
            let calls = Arc::clone(&calls_clone);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(CoreError::network("always fails"))
            }
        })
        .await;

    assert!(result.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(1_000),
        "first retry delay must be at least initial_delay_ms (1000ms), got {elapsed:?}"
    );
    // Tight upper bound: an off-by-one exponent mutation (attempt - 1 ->
    // attempt) would double this to 2000ms and must fail this assertion.
    assert!(
        elapsed < Duration::from_millis(1_500),
        "first retry delay must equal initial_delay_ms (1000ms) exactly \
         (exponent must be 0, not 1) — got {elapsed:?}, which suggests an \
         off-by-one in the backoff exponent calculation"
    );
}

/// State isolation: two independent `retry_with_backoff` invocations (with
/// independent counters) must not influence each other's attempt counts.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_calls_are_isolated_between_invocations() {
    let config = config_with(2, 2.0, 10);

    let calls_a = Arc::new(AtomicU32::new(0));
    let calls_a_clone = Arc::clone(&calls_a);
    let result_a: CoreResult<()> = retry_with_backoff(&config, "isolated-a", None, move || {
        let calls = Arc::clone(&calls_a_clone);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(CoreError::network("a fails"))
        }
    })
    .await;

    let calls_b = Arc::new(AtomicU32::new(0));
    let calls_b_clone = Arc::clone(&calls_b);
    let result_b: CoreResult<()> = retry_with_backoff(&config, "isolated-b", None, move || {
        let calls = Arc::clone(&calls_b_clone);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(CoreError::network("b fails"))
        }
    })
    .await;

    assert!(result_a.is_err());
    assert!(result_b.is_err());
    assert_eq!(calls_a.load(Ordering::SeqCst), 3);
    assert_eq!(calls_b.load(Ordering::SeqCst), 3);
}

/// `operation_name` is a passthrough logging value: differing values must not
/// change retry/success semantics.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_operation_name_value_does_not_affect_retry_count() {
    let config = config_with(2, 2.0, 10);

    for name in [
        "",
        "short-name",
        "a-very-long-operation-name-for-logging-purposes",
    ] {
        let calls = Arc::new(AtomicU32::new(0));
        let calls_clone = Arc::clone(&calls);
        let result: CoreResult<()> = retry_with_backoff(&config, name, None, move || {
            let calls = Arc::clone(&calls_clone);
            async move {
                let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt < 2 {
                    Err(CoreError::network("transient"))
                } else {
                    Ok(())
                }
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "operation_name={name:?} must not affect success"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "operation_name={name:?} must not affect attempt count"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tier 2 — Structured logging passthrough (tracing_test harness already used
// elsewhere in this crate; see release_orchestrator_tracing_tests.rs)
// ─────────────────────────────────────────────────────────────────────────────

/// When a `correlation_id` is supplied, it must be discoverable in the
/// structured log output emitted while retrying (logging only — must not
/// alter retry semantics, verified separately above).
#[tokio::test(start_paused = true)]
#[traced_test]
async fn test_retry_with_backoff_logs_contain_correlation_id_when_retrying() {
    let config = config_with(2, 2.0, 10);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let _result: CoreResult<()> = retry_with_backoff(
        &config,
        "logging-op",
        Some("unique-corr-id-RETRY-GAMMA"),
        move || {
            let calls = Arc::clone(&calls_clone);
            async move {
                let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt < 2 {
                    Err(CoreError::network("transient"))
                } else {
                    Ok(())
                }
            }
        },
    )
    .await;

    assert!(
        logs_contain("unique-corr-id-RETRY-GAMMA"),
        "correlation_id must appear in structured log output during a retry"
    );
}

/// `operation_name` must be discoverable in the structured log output.
#[tokio::test(start_paused = true)]
#[traced_test]
async fn test_retry_with_backoff_logs_contain_operation_name_when_retrying() {
    let config = config_with(2, 2.0, 10);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let _result: CoreResult<()> =
        retry_with_backoff(&config, "unique-operation-name-DELTA", None, move || {
            let calls = Arc::clone(&calls_clone);
            async move {
                let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt < 2 {
                    Err(CoreError::network("transient"))
                } else {
                    Ok(())
                }
            }
        })
        .await;

    assert!(
        logs_contain("unique-operation-name-DELTA"),
        "operation_name must appear in structured log output during a retry"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tier 2 — Security regression tests (pathological `backoff_multiplier`,
// GitHub issue #138 security review, CRITICAL/HIGH/MEDIUM findings)
// ─────────────────────────────────────────────────────────────────────────────

/// CRITICAL: an extreme `backoff_multiplier` (e.g. `1e308`) makes
/// `backoff_multiplier.powi(n)` overflow to `f64::INFINITY` within a couple
/// of retries. `Duration::from_secs_f64` panics on infinite input, so
/// without the `MAX_DELAY_MS` cap this would crash the event-processing
/// task. Asserts the call completes normally (returns `Err`, does not
/// panic).
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_extreme_multiplier_no_panic() {
    let config = config_with(3, 1e308, 1000);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> =
        retry_with_backoff(&config, "extreme-multiplier", None, move || {
            let calls = Arc::clone(&calls_clone);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(CoreError::network("always fails"))
            }
        })
        .await;

    assert!(
        result.is_err(),
        "an always-failing operation must still return Err after retries are exhausted"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 4, "1 initial + 3 retries");
}

/// MEDIUM: a negative `backoff_multiplier` (e.g. `-5.0`) produces
/// alternating-sign delays via `powi`, which is also invalid input to
/// `Duration::from_secs_f64` (panics on negative values, not just infinite/
/// NaN). Asserts no panic and eventual `Err` after exhausting retries.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_negative_multiplier_no_panic() {
    let config = config_with(3, -5.0, 1000);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> =
        retry_with_backoff(&config, "negative-multiplier", None, move || {
            let calls = Arc::clone(&calls_clone);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(CoreError::network("always fails"))
            }
        })
        .await;

    assert!(
        result.is_err(),
        "an always-failing operation must still return Err after retries are exhausted"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 4, "1 initial + 3 retries");
}

/// CRITICAL: a NaN `backoff_multiplier` propagates through `powi` to a NaN
/// delay. `Duration::from_secs_f64` panics on NaN input, and `f64::clamp`
/// leaves NaN unchanged (comparisons with NaN are always false), so the
/// implementation must special-case non-finite values explicitly. Asserts
/// no panic.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_nan_multiplier_no_panic() {
    let config = config_with(3, f64::NAN, 1000);
    let calls = Arc::new(AtomicU32::new(0));
    let calls_clone = Arc::clone(&calls);

    let result: CoreResult<()> = retry_with_backoff(&config, "nan-multiplier", None, move || {
        let calls = Arc::clone(&calls_clone);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(CoreError::network("always fails"))
        }
    })
    .await;

    assert!(
        result.is_err(),
        "an always-failing operation must still return Err after retries are exhausted"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 4, "1 initial + 3 retries");
}

/// HIGH: even without overflowing to infinity, a large-but-finite
/// multiplier (e.g. `10.0` with several retries) would otherwise produce
/// multi-hour/day delays. Using the paused-clock technique, asserts the
/// simulated elapsed time for a single retry delay never exceeds
/// `MAX_DELAY_MS` (30_000ms), i.e. the cap is actually enforced and not
/// merely panic-safe.
#[tokio::test(start_paused = true)]
async fn test_retry_with_backoff_delay_capped_at_max_ms() {
    let config = config_with(1, 1e308, 1000);
    let start = tokio::time::Instant::now();

    let result: CoreResult<()> = retry_with_backoff(&config, "capped-delay", None, || async {
        Err(CoreError::network("always fails"))
    })
    .await;

    assert!(result.is_err());
    let elapsed = start.elapsed();
    assert!(
        elapsed <= Duration::from_millis(30_000),
        "retry delay must be capped at MAX_DELAY_MS (30_000ms), got {elapsed:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tier 3 — Property-based tests
// ─────────────────────────────────────────────────────────────────────────────

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// For any `max_retries`, an always-failing retryable operation is
        /// invoked exactly `max_retries + 1` times before `retry_with_backoff`
        /// returns `Err`.
        #[test]
        fn prop_total_attempts_always_equals_max_retries_plus_one(
            max_retries in 0u32..8u32,
            backoff_multiplier in 1.0f64..4.0f64,
            initial_delay_ms in 1u64..50u64,
        ) {
            let config = config_with(max_retries, backoff_multiplier, initial_delay_ms);
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .start_paused(true)
                .build()
                .expect("failed to build paused test runtime");

            rt.block_on(async move {
                let calls = Arc::new(AtomicU32::new(0));
                let calls_clone = Arc::clone(&calls);

                let result: CoreResult<()> = retry_with_backoff(
                    &config,
                    "prop-always-fails",
                    None,
                    move || {
                        let calls = Arc::clone(&calls_clone);
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            Err(CoreError::network("always fails"))
                        }
                    },
                )
                .await;

                prop_assert!(result.is_err());
                prop_assert_eq!(calls.load(Ordering::SeqCst), max_retries + 1);
                Ok(())
            })?;
        }

        /// For any `max_retries` >= 0, a non-retryable error results in
        /// exactly one attempt — retry configuration must never cause a
        /// non-retryable error to be retried.
        #[test]
        fn prop_non_retryable_errors_are_never_retried(
            max_retries in 0u32..8u32,
            backoff_multiplier in 1.0f64..4.0f64,
            initial_delay_ms in 1u64..50u64,
        ) {
            let config = config_with(max_retries, backoff_multiplier, initial_delay_ms);
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .start_paused(true)
                .build()
                .expect("failed to build paused test runtime");

            rt.block_on(async move {
                let calls = Arc::new(AtomicU32::new(0));
                let calls_clone = Arc::clone(&calls);

                let result: CoreResult<()> = retry_with_backoff(
                    &config,
                    "prop-non-retryable",
                    None,
                    move || {
                        let calls = Arc::clone(&calls_clone);
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            Err(CoreError::validation("field", "always invalid"))
                        }
                    },
                )
                .await;

                prop_assert!(result.is_err());
                prop_assert_eq!(calls.load(Ordering::SeqCst), 1);
                Ok(())
            })?;
        }

        /// Cumulative (simulated) elapsed delay for an always-failing
        /// retryable operation must be at least the sum of the geometric
        /// backoff series implied by `config`, for any valid combination of
        /// `max_retries`, `backoff_multiplier`, and `initial_delay_ms`.
        #[test]
        fn prop_cumulative_delay_scales_with_config(
            max_retries in 1u32..5u32,
            backoff_multiplier in 1.0f64..3.0f64,
            initial_delay_ms in 10u64..200u64,
        ) {
            let config = config_with(max_retries, backoff_multiplier, initial_delay_ms);
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .start_paused(true)
                .build()
                .expect("failed to build paused test runtime");

            rt.block_on(async move {
                let calls = Arc::new(AtomicU32::new(0));
                let calls_clone = Arc::clone(&calls);
                let start = tokio::time::Instant::now();

                let result: CoreResult<()> = retry_with_backoff(
                    &config,
                    "prop-cumulative-delay",
                    None,
                    move || {
                        let calls = Arc::clone(&calls_clone);
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            Err(CoreError::network("always fails"))
                        }
                    },
                )
                .await;

                prop_assert!(result.is_err());
                let elapsed_ms = start.elapsed().as_millis() as f64;
                let expected_ms: f64 = (0..max_retries)
                    .map(|k| initial_delay_ms as f64 * backoff_multiplier.powi(k as i32))
                    .sum();
                // Allow a small tolerance for timer-resolution rounding.
                prop_assert!(
                    elapsed_ms >= expected_ms * 0.9,
                    "elapsed {elapsed_ms}ms below expected cumulative backoff {expected_ms}ms"
                );
                Ok(())
            })?;
        }
    }
}
