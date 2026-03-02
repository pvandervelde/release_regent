use super::*;

// ──────────────────────────────────────────────────────────────
// PassThroughWebhookValidator
// ──────────────────────────────────────────────────────────────

#[test]
fn pass_through_validator_can_be_constructed() {
    let _v = PassThroughWebhookValidator::new();
}

/// `verify_signature` always returns `Ok(true)` — no secret checking for local files.
#[tokio::test]
async fn verify_signature_returns_true_for_any_input() {
    let v = PassThroughWebhookValidator::new();
    let result = v
        .verify_signature(b"some payload bytes", "sha256=abc", "any_secret")
        .await;
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    assert_eq!(result.unwrap(), true);
}

/// `validate_payload` always returns `Ok(true)` — no structural checks for local files.
#[tokio::test]
async fn validate_payload_returns_true_for_any_input() {
    let v = PassThroughWebhookValidator::new();
    let result = v
        .validate_payload(&serde_json::json!({"action": "closed"}), "pull_request")
        .await;
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    assert_eq!(result.unwrap(), true);
}

// ──────────────────────────────────────────────────────────────
// create_mock_processor
// ──────────────────────────────────────────────────────────────

/// `create_mock_processor` succeeds and returns a usable processor.
#[test]
fn create_mock_processor_returns_a_processor() {
    // This should not panic.
    let _p = create_mock_processor();
}

// ──────────────────────────────────────────────────────────────
// create_production_processor  — missing env vars path
// ──────────────────────────────────────────────────────────────

/// `create_production_processor` returns `CliError::MissingDependency` when
/// `GITHUB_APP_ID` is absent from the environment.
#[tokio::test]
async fn create_production_processor_fails_with_missing_app_id() {
    // Remove all credentials to force the missing-dependency path.
    std::env::remove_var("GITHUB_APP_ID");
    std::env::remove_var("GITHUB_PRIVATE_KEY");
    std::env::remove_var("GITHUB_WEBHOOK_SECRET");
    std::env::remove_var("GITHUB_INSTALLATION_ID");

    let result = create_production_processor().await;
    assert!(
        result.is_err(),
        "Expected error when env vars are missing, got Ok"
    );

    let err = result.err().expect("expected an error");
    match err {
        crate::errors::CliError::MissingDependency { ref dependency, .. } => {
            assert_eq!(dependency, "GITHUB_APP_ID");
        }
        other => panic!(
            "Expected MissingDependency for GITHUB_APP_ID, got {:?}",
            other
        ),
    }
}
