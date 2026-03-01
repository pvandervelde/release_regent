use super::*;

// ──────────────────────────────────────────────────────────────
// PassThroughWebhookValidator
// ──────────────────────────────────────────────────────────────

#[test]
fn pass_through_validator_can_be_constructed() {
    let _v = PassThroughWebhookValidator::new();
}

/// `verify_signature` should always return `Ok(true)` — validated after
/// Phase 2 implementation.  In Phase 1 the body is `todo!()` so the test
/// is marked `should_panic`.
#[tokio::test]
#[should_panic]
async fn verify_signature_panics_until_implemented() {
    let v = PassThroughWebhookValidator::new();
    let _ = v.verify_signature(b"payload", "sha256=abc", "secret").await;
}

/// `validate_payload` should always return `Ok(true)` — validated after
/// Phase 2 implementation.  In Phase 1 the body is `todo!()` so the test
/// is marked `should_panic`.
#[tokio::test]
#[should_panic]
async fn validate_payload_panics_until_implemented() {
    let v = PassThroughWebhookValidator::new();
    let _ = v
        .validate_payload(&serde_json::json!({}), "pull_request")
        .await;
}

// ──────────────────────────────────────────────────────────────
// create_mock_processor
// ──────────────────────────────────────────────────────────────

/// `create_mock_processor` should return a processor — validated after Phase 2.
#[test]
#[should_panic]
fn create_mock_processor_panics_until_implemented() {
    let _p = create_mock_processor();
}

// ──────────────────────────────────────────────────────────────
// create_production_processor  — missing env vars path
// ──────────────────────────────────────────────────────────────

/// When `GITHUB_APP_ID` is not set the function must return a
/// `CliError::MissingDependency` — validated after Phase 2.
#[tokio::test]
#[should_panic]
async fn create_production_processor_panics_until_implemented() {
    // Remove env vars to force the missing-dependency code path.
    std::env::remove_var("GITHUB_APP_ID");
    std::env::remove_var("GITHUB_PRIVATE_KEY");
    std::env::remove_var("GITHUB_WEBHOOK_SECRET");
    std::env::remove_var("GITHUB_INSTALLATION_ID");

    let _result = create_production_processor().await;
}
