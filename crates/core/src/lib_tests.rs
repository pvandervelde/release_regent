use super::*;

#[test]
fn test_release_regent_creation() {
    let config = config::ReleaseRegentConfig::default();
    let regent = ReleaseRegent::new(config);

    assert_eq!(regent.config().core.version_prefix, "v");
    assert_eq!(regent.config().core.branches.main, "main");
}

#[tokio::test]
async fn test_webhook_processing_placeholder() {
    let config = config::ReleaseRegentConfig::default();
    let regent = ReleaseRegent::new(config);

    let event = webhook::WebhookEvent::new(
        "pull_request".to_string(),
        "closed".to_string(),
        serde_json::json!({}),
        std::collections::HashMap::new(),
    );

    // This should succeed with the placeholder implementation
    let result = regent.process_webhook(event).await;
    assert!(result.is_ok());
}
