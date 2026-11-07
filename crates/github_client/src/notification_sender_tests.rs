// Tests for `HttpNotificationSender` (`send_webhook` / `send_slack`).
// Uses wiremock to provide a local mock HTTP endpoint so no real network calls are made.

use std::collections::HashMap;

use release_regent_core::{traits::notification_sender::NotificationSender, CoreError};
use wiremock::{
    matchers::{body_json, header, method, path},
    Mock, MockServer, ResponseTemplate,
};

use super::HttpNotificationSender;

// ---------------------------------------------------------------------------
// send_webhook
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_send_webhook_success_returns_ok() {
    let mock_server = MockServer::start().await;
    let payload = serde_json::json!({ "event": "release_created", "summary": "v1.2.3 released" });

    Mock::given(method("POST"))
        .and(path("/hook"))
        .and(header("X-Custom-Header", "custom-value"))
        .and(body_json(&payload))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let sender = HttpNotificationSender::new().expect("sender construction should not fail");
    let mut headers = HashMap::new();
    headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());

    let result = sender
        .send_webhook(&format!("{}/hook", mock_server.uri()), &headers, &payload)
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_send_webhook_non_2xx_returns_webhook_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/hook"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let sender = HttpNotificationSender::new().expect("sender construction should not fail");
    let payload = serde_json::json!({ "event": "processing_error" });

    let result = sender
        .send_webhook(&format!("{}/hook", mock_server.uri()), &HashMap::new(), &payload)
        .await;

    assert!(matches!(
        result,
        Err(CoreError::Webhook { ref stage, .. }) if stage == "webhook"
    ));
}

#[tokio::test]
async fn test_send_webhook_connection_failure_returns_network_error() {
    let sender = HttpNotificationSender::new().expect("sender construction should not fail");
    let payload = serde_json::json!({ "event": "release_created" });

    // Port 0 on localhost never accepts connections.
    let result = sender
        .send_webhook("http://127.0.0.1:0/hook", &HashMap::new(), &payload)
        .await;

    assert!(matches!(result, Err(CoreError::Network { .. })));
}

// ---------------------------------------------------------------------------
// send_slack
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_send_slack_without_channel_omits_channel_field() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/slack"))
        .and(body_json(serde_json::json!({ "text": "*Release created*\nv1.2.3" })))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let sender = HttpNotificationSender::new().expect("sender construction should not fail");
    let result = sender
        .send_slack(
            &format!("{}/slack", mock_server.uri()),
            None,
            "*Release created*\nv1.2.3",
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_send_slack_with_channel_includes_channel_field() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/slack"))
        .and(body_json(
            serde_json::json!({ "text": "*Error*\ndetails", "channel": "#releases" }),
        ))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let sender = HttpNotificationSender::new().expect("sender construction should not fail");
    let result = sender
        .send_slack(
            &format!("{}/slack", mock_server.uri()),
            Some("#releases"),
            "*Error*\ndetails",
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_send_slack_non_2xx_returns_webhook_error_with_slack_stage() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/slack"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&mock_server)
        .await;

    let sender = HttpNotificationSender::new().expect("sender construction should not fail");
    let result = sender
        .send_slack(&format!("{}/slack", mock_server.uri()), None, "text")
        .await;

    assert!(matches!(
        result,
        Err(CoreError::Webhook { ref stage, .. }) if stage == "slack"
    ));
}
