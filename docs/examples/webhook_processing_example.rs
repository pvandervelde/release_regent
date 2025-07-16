// webhook_processing_example.rs
//
// This example demonstrates how to use Release Regent's webhook processing
// components to handle GitHub webhooks in a custom application.

use release_regent_core::webhook::{WebhookProcessor, WebhookEvent};
use release_regent_github_client::GitHubAuthManager;
use serde_json;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::init();

    // Example 1: Basic webhook processing
    println!("=== Example 1: Basic Webhook Processing ===");
    basic_webhook_processing().await?;

    // Example 2: Signature validation
    println!("\n=== Example 2: Signature Validation ===");
    signature_validation_example().await?;

    // Example 3: Custom webhook handler
    println!("\n=== Example 3: Custom Webhook Handler ===");
    custom_webhook_handler().await?;

    Ok(())
}

/// Example 1: Basic webhook processing without signature validation
async fn basic_webhook_processing() -> Result<(), Box<dyn std::error::Error>> {
    // Create a webhook processor without signature validation
    let processor = WebhookProcessor::new(None);

    // Create a sample webhook event (merged pull request)
    let mut headers = HashMap::new();
    headers.insert("x-github-event".to_string(), "pull_request".to_string());

    let payload = serde_json::json!({
        "action": "closed",
        "pull_request": {
            "id": 123456789,
            "number": 42,
            "state": "closed",
            "title": "feat: add new feature",
            "body": "This PR adds a new feature to the application.",
            "merged": true,
            "merge_commit_sha": "abc123def456789",
            "base": {
                "ref": "main",
                "sha": "def456789abc123"
            },
            "head": {
                "ref": "feature/new-feature",
                "sha": "789abc123def456"
            }
        },
        "repository": {
            "id": 987654321,
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "default_branch": "main",
            "owner": {
                "login": "owner",
                "type": "User"
            }
        }
    });

    let event = WebhookEvent::new(
        "pull_request".to_string(),
        "closed".to_string(),
        payload,
        headers,
    );

    // Process the webhook event
    match processor.process_event(&event).await {
        Ok(Some(result)) => {
            println!("✅ Successfully processed webhook event");
            println!("Result: {:?}", result);
        }
        Ok(None) => {
            println!("ℹ️ Event was ignored (not a merged PR or unsupported event type)");
        }
        Err(e) => {
            println!("❌ Failed to process webhook: {}", e);
        }
    }

    Ok(())
}

/// Example 2: Webhook processing with signature validation
async fn signature_validation_example() -> Result<(), Box<dyn std::error::Error>> {
    let webhook_secret = "my-super-secret-webhook-key";

    // Create a webhook processor with signature validation
    let processor = WebhookProcessor::new(Some(webhook_secret.to_string()));

    // Create a sample payload
    let payload = serde_json::json!({
        "action": "closed",
        "pull_request": {
            "number": 42,
            "merged": true,
            "title": "fix: resolve critical bug"
        },
        "repository": {
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "default_branch": "main",
            "owner": {
                "login": "owner"
            }
        }
    });

    // Calculate the correct signature for this payload
    let payload_bytes = serde_json::to_vec(&payload)?;
    let signature = calculate_webhook_signature(&payload_bytes, webhook_secret)?;

    // Create headers with the signature
    let mut headers = HashMap::new();
    headers.insert("x-github-event".to_string(), "pull_request".to_string());
    headers.insert("x-hub-signature-256".to_string(), signature);

    let event = WebhookEvent::new(
        "pull_request".to_string(),
        "closed".to_string(),
        payload,
        headers,
    );

    // Process the webhook event with signature validation
    match processor.process_event(&event).await {
        Ok(Some(result)) => {
            println!("✅ Successfully processed webhook with valid signature");
            println!("Result: {:?}", result);
        }
        Ok(None) => {
            println!("ℹ️ Event was ignored");
        }
        Err(e) => {
            println!("❌ Failed to process webhook: {}", e);

            // Demonstrate what happens with an invalid signature
            println!("\n--- Testing with invalid signature ---");
            test_invalid_signature(&processor).await?;
        }
    }

    Ok(())
}

/// Calculate webhook signature using the same method as GitHub
fn calculate_webhook_signature(
    payload: &[u8],
    secret: &str
) -> Result<String, Box<dyn std::error::Error>> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())?;
    mac.update(payload);
    let result = mac.finalize();
    let signature = hex::encode(result.into_bytes());

    Ok(format!("sha256={}", signature))
}

/// Test webhook processing with invalid signature
async fn test_invalid_signature(
    processor: &WebhookProcessor
) -> Result<(), Box<dyn std::error::Error>> {
    let payload = serde_json::json!({
        "action": "closed",
        "pull_request": {
            "number": 42,
            "merged": true
        }
    });

    let mut headers = HashMap::new();
    headers.insert("x-github-event".to_string(), "pull_request".to_string());
    headers.insert("x-hub-signature-256".to_string(), "sha256=invalid_signature".to_string());

    let event = WebhookEvent::new(
        "pull_request".to_string(),
        "closed".to_string(),
        payload,
        headers,
    );

    match processor.process_event(&event).await {
        Ok(_) => {
            println!("⚠️ Unexpected: Invalid signature was accepted");
        }
        Err(e) => {
            println!("✅ Expected: Invalid signature rejected with error: {}", e);
        }
    }

    Ok(())
}

/// Example 3: Custom webhook handler with filtering and processing
async fn custom_webhook_handler() -> Result<(), Box<dyn std::error::Error>> {
    let processor = WebhookProcessor::new(None);

    // Test various event types to demonstrate filtering
    let test_events = vec![
        // Merged PR - should be processed
        ("pull_request", "closed", serde_json::json!({
            "action": "closed",
            "pull_request": {
                "number": 1,
                "merged": true,
                "title": "feat: new feature"
            },
            "repository": {
                "name": "test-repo",
                "default_branch": "main",
                "owner": { "login": "owner" }
            }
        })),

        // Closed but not merged PR - should be ignored
        ("pull_request", "closed", serde_json::json!({
            "action": "closed",
            "pull_request": {
                "number": 2,
                "merged": false,
                "title": "feat: abandoned feature"
            }
        })),

        // Opened PR - should be ignored
        ("pull_request", "opened", serde_json::json!({
            "action": "opened",
            "pull_request": {
                "number": 3,
                "title": "feat: work in progress"
            }
        })),

        // Unsupported event type - should be ignored
        ("issues", "opened", serde_json::json!({
            "action": "opened",
            "issue": {
                "number": 1,
                "title": "Bug report"
            }
        })),
    ];

    for (event_type, action, payload) in test_events {
        println!("\n--- Processing {} event with action '{}' ---", event_type, action);

        let mut headers = HashMap::new();
        headers.insert("x-github-event".to_string(), event_type.to_string());

        let event = WebhookEvent::new(
            event_type.to_string(),
            action.to_string(),
            payload,
            headers,
        );

        match processor.process_event(&event).await {
            Ok(Some(result)) => {
                println!("✅ Event processed: {:?}", result);
            }
            Ok(None) => {
                println!("ℹ️ Event ignored (expected for non-merged PRs and unsupported events)");
            }
            Err(e) => {
                println!("❌ Processing error: {}", e);
            }
        }
    }

    Ok(())
}

// Additional helper functions and examples could be added here...
