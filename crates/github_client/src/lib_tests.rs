// Tests for type conversions, SDK integration, and error mapping.
// Note: Operations requiring live SDK authentication are tested via integration tests.
// These unit tests validate error mapping, public exports, and retry configuration.

use super::*;
use chrono::Utc;
use github_bot_sdk::error::ApiError;
use release_regent_core::CoreError;

// ============================================================================
// map_sdk_error tests
// ============================================================================

/// `ApiError::NotFound` must map to `CoreError::NotFound` and must NOT be retryable.
#[test]
fn test_map_sdk_error_not_found_maps_to_core_not_found() {
    let err = map_sdk_error(ApiError::NotFound);
    assert!(
        matches!(err, CoreError::NotFound { .. }),
        "expected CoreError::NotFound, got: {:?}",
        err
    );
    assert!(!err.is_retryable(), "NotFound should not be retryable");
}

/// `ApiError::AuthenticationFailed` must map to `CoreError::Authentication` (non-retryable).
#[test]
fn test_map_sdk_error_auth_failed_maps_to_authentication() {
    let err = map_sdk_error(ApiError::AuthenticationFailed);
    assert!(
        matches!(err, CoreError::Authentication { .. }),
        "expected CoreError::Authentication, got: {:?}",
        err
    );
    assert!(
        !err.is_retryable(),
        "AuthenticationFailed should not be retryable"
    );
}

/// `ApiError::AuthorizationFailed` must map to `CoreError::Authentication` (non-retryable).
#[test]
fn test_map_sdk_error_auth_authorization_failed_maps_to_authentication() {
    let err = map_sdk_error(ApiError::AuthorizationFailed);
    assert!(
        matches!(err, CoreError::Authentication { .. }),
        "expected CoreError::Authentication, got: {:?}",
        err
    );
    assert!(
        !err.is_retryable(),
        "AuthorizationFailed should not be retryable"
    );
}

/// `ApiError::Timeout` must map to `CoreError::Timeout` and MUST be retryable.
#[test]
fn test_map_sdk_error_timeout_maps_to_core_timeout() {
    let err = map_sdk_error(ApiError::Timeout);
    assert!(
        matches!(err, CoreError::Timeout { .. }),
        "expected CoreError::Timeout, got: {:?}",
        err
    );
    assert!(err.is_retryable(), "Timeout should be retryable");
}

/// `ApiError::RateLimitExceeded` must map to `CoreError::RateLimit` and MUST be retryable.
/// The `retry_after_seconds` must be populated from the `reset_at` timestamp.
#[test]
fn test_map_sdk_error_rate_limit_exceeded_maps_to_core_rate_limit() {
    let reset_at = Utc::now() + chrono::Duration::seconds(42);
    let err = map_sdk_error(ApiError::RateLimitExceeded { reset_at });
    assert!(
        matches!(err, CoreError::RateLimit { .. }),
        "expected CoreError::RateLimit, got: {:?}",
        err
    );
    assert!(err.is_retryable(), "RateLimitExceeded should be retryable");
    // retry_after_seconds should be Some and approximately 42 seconds
    if let CoreError::RateLimit {
        retry_after_seconds,
        ..
    } = &err
    {
        assert!(
            retry_after_seconds.is_some(),
            "retry_after_seconds should be set from reset_at"
        );
        let secs = retry_after_seconds.unwrap();
        assert!(secs <= 45, "retry_after_seconds too large: {}", secs);
    }
}

/// `ApiError::SecondaryRateLimit` must map to `CoreError::RateLimit` with a hard-coded
/// 60-second retry hint and MUST be retryable.
#[test]
fn test_map_sdk_error_secondary_rate_limit_maps_to_core_rate_limit() {
    let err = map_sdk_error(ApiError::SecondaryRateLimit);
    assert!(
        matches!(err, CoreError::RateLimit { .. }),
        "expected CoreError::RateLimit for SecondaryRateLimit, got: {:?}",
        err
    );
    assert!(err.is_retryable(), "SecondaryRateLimit should be retryable");
    if let CoreError::RateLimit {
        retry_after_seconds,
        ..
    } = &err
    {
        assert_eq!(*retry_after_seconds, Some(60));
    }
}

// Note: `ApiError::HttpClientError(reqwest::Error)` mapping to `CoreError::Network` is
// validated indirectly by the wiremock-based tests in `pr_management_tests.rs`.
// Constructing a `reqwest::Error` in a pure unit test requires the `blocking` feature
// which this workspace does not enable.

/// `ApiError::HttpError` with a 5xx status must map to `CoreError::Network`
/// and MUST be retryable.
#[test]
fn test_map_sdk_error_http_error_5xx_maps_to_core_network() {
    for status in [500u16, 502, 503, 504] {
        let err = map_sdk_error(ApiError::HttpError {
            status,
            message: format!("{} Internal Server Error", status),
        });
        assert!(
            matches!(err, CoreError::Network { .. }),
            "status {} should map to CoreError::Network, got: {:?}",
            status,
            err
        );
        assert!(err.is_retryable(), "HTTP {} should be retryable", status);
    }
}

/// `ApiError::HttpError` with a 429 status must map to `CoreError::RateLimit`
/// and MUST be retryable.
#[test]
fn test_map_sdk_error_http_error_429_maps_to_core_rate_limit() {
    let err = map_sdk_error(ApiError::HttpError {
        status: 429,
        message: "Too Many Requests".to_string(),
    });
    assert!(
        matches!(err, CoreError::RateLimit { .. }),
        "HTTP 429 should map to CoreError::RateLimit, got: {:?}",
        err
    );
    assert!(err.is_retryable(), "HTTP 429 should be retryable");
}

/// `ApiError::HttpError` with a 401 status must map to `CoreError::Authentication`
/// and must NOT be retryable.
#[test]
fn test_map_sdk_error_http_error_401_maps_to_authentication() {
    let err = map_sdk_error(ApiError::HttpError {
        status: 401,
        message: "Unauthorized".to_string(),
    });
    assert!(
        matches!(err, CoreError::Authentication { .. }),
        "HTTP 401 should map to CoreError::Authentication, got: {:?}",
        err
    );
    assert!(!err.is_retryable(), "HTTP 401 should not be retryable");
}

/// `ApiError::HttpError` with 422 (validation) must map to `CoreError::GitHub`
/// and must NOT be retryable.
#[test]
fn test_map_sdk_error_http_error_4xx_maps_to_github() {
    for status in [422u16, 400, 410] {
        let err = map_sdk_error(ApiError::HttpError {
            status,
            message: format!("client error {}", status),
        });
        assert!(
            matches!(err, CoreError::GitHub { .. }),
            "HTTP {} should map to CoreError::GitHub, got: {:?}",
            status,
            err
        );
        assert!(
            !err.is_retryable(),
            "HTTP {} should not be retryable",
            status
        );
    }
}

/// `ApiError::InvalidRequest` must map to `CoreError::GitHub` and must NOT be retryable.
#[test]
fn test_map_sdk_error_invalid_request_maps_to_github() {
    let err = map_sdk_error(ApiError::InvalidRequest {
        message: "branch name already exists".to_string(),
    });
    assert!(
        matches!(err, CoreError::GitHub { .. }),
        "expected CoreError::GitHub, got: {:?}",
        err
    );
    assert!(
        !err.is_retryable(),
        "InvalidRequest should not be retryable"
    );
}

// ============================================================================
// Retry configuration tests
// ============================================================================

/// The retry constant must be set to 5 per the error-handling spec.
///
/// This test will FAIL until the implementation phase changes `MAX_RETRIES` from
/// the placeholder value of 3 to the spec-required value of 5.
#[test]
fn test_max_retries_constant_matches_spec() {
    assert_eq!(
        MAX_RETRIES, 5u32,
        "docs/specs/design/error-handling.md requires 5 max retry attempts"
    );
}

// ============================================================================
// Public export tests
// ============================================================================

#[test]
fn test_github_client_exports() {
    // Verify public API exports exist
    use crate::{AuthConfig, Error, GitHubClient, GitHubResult};

    // Type checking - ensures types are properly exported
    let _: Option<GitHubClient> = None;
    let _: Option<AuthConfig> = None;
    let _: Option<Error> = None;
    let _: GitHubResult<()> = Ok(());
}

#[test]
fn test_sdk_types_reexported() {
    // Verify SDK types are re-exported
    use crate::{GitHubAppId, SdkInstallationId};

    let app_id = GitHubAppId::new(12345);
    assert_eq!(app_id.as_u64(), 12345);

    let installation_id = SdkInstallationId::new(67890);
    assert_eq!(installation_id.as_u64(), 67890);
}
