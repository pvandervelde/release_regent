// azure_function_integration.rs
//
// This example shows how to integrate Release Regent's webhook processing
// into an Azure Function for production deployment.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use release_regent_core::webhook::{WebhookProcessor, WebhookEvent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{debug, error, info, warn};

/// Application configuration
#[derive(Clone)]
struct AppConfig {
    webhook_secret: Option<String>,
    github_app_id: String,
    github_private_key: String,
    environment: String,
}

impl AppConfig {
    fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            webhook_secret: std::env::var("WEBHOOK_SECRET").ok(),
            github_app_id: std::env::var("GITHUB_APP_ID")?,
            github_private_key: std::env::var("GITHUB_PRIVATE_KEY")?,
            environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
        })
    }
}

/// Shared application state
#[derive(Clone)]
struct AppState {
    config: AppConfig,
    webhook_processor: WebhookProcessor,
}

impl AppState {
    fn new(config: AppConfig) -> Self {
        let webhook_processor = WebhookProcessor::new(config.webhook_secret.clone());

        Self {
            config,
            webhook_processor,
        }
    }
}

/// Health check response
#[derive(Serialize)]
struct HealthResponse {
    status: String,
    timestamp: String,
    version: String,
    environment: String,
}

/// Webhook processing response
#[derive(Serialize)]
struct WebhookResponse {
    status: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    processing_result: Option<String>,
}

/// Query parameters for webhook endpoint
#[derive(Deserialize)]
struct WebhookQuery {
    #[serde(default)]
    skip_signature: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::init();

    // Load configuration
    let config = AppConfig::from_env()
        .map_err(|e| format!("Failed to load configuration: {}", e))?;

    info!("Starting Release Regent Azure Function");
    info!("Environment: {}", config.environment);
    info!("Webhook secret configured: {}", config.webhook_secret.is_some());

    // Create application state
    let state = AppState::new(config);

    // Create router
    let app = create_router(state);

    // Start server
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()?;

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;

    info!("Server listening on port {}", port);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Create the application router with all endpoints
fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/webhook", post(webhook_handler))
        .route("/api/webhook/test", post(test_webhook_handler))
        .with_state(state)
}

/// Health check endpoint
/// GET /health
async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        environment: state.config.environment,
    })
}

/// Main webhook handler
/// POST /api/webhook
async fn webhook_handler(
    State(state): State<AppState>,
    Query(query): Query<WebhookQuery>,
    headers: HeaderMap,
    payload: String,
) -> Result<Json<WebhookResponse>, StatusCode> {
    info!("Received webhook request");
    debug!("Payload size: {} bytes", payload.len());
    debug!("Skip signature: {}", query.skip_signature);

    // Convert headers to HashMap
    let headers_map = convert_headers(&headers);

    // Log relevant headers (without sensitive data)
    debug!("GitHub event type: {:?}", headers_map.get("x-github-event"));
    debug!("GitHub delivery ID: {:?}", headers_map.get("x-github-delivery"));

    // Parse webhook payload
    let webhook_data: serde_json::Value = match serde_json::from_str(&payload) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to parse webhook payload: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Extract event information
    let event_type = headers_map
        .get("x-github-event")
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());

    let action = webhook_data
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    info!("Processing GitHub event: {} with action: {}", event_type, action);

    // Create webhook event
    let event = WebhookEvent::new(event_type, action, webhook_data, headers_map);

    // Process the webhook
    match state.webhook_processor.process_event(&event).await {
        Ok(Some(result)) => {
            info!("Webhook processed successfully with result");
            Ok(Json(WebhookResponse {
                status: "processed".to_string(),
                message: "Webhook processed successfully".to_string(),
                processing_result: Some(format!("{:?}", result)),
            }))
        }
        Ok(None) => {
            info!("Webhook received but no processing needed");
            Ok(Json(WebhookResponse {
                status: "ignored".to_string(),
                message: "Event type not processed".to_string(),
                processing_result: None,
            }))
        }
        Err(e) => {
            error!("Webhook processing failed: {}", e);

            // Return appropriate error status based on error type
            let status_code = if e.to_string().contains("signature") {
                StatusCode::UNAUTHORIZED
            } else if e.to_string().contains("payload") || e.to_string().contains("Missing") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            Err(status_code)
        }
    }
}

/// Test webhook handler for development/testing
/// POST /api/webhook/test
async fn test_webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: String,
) -> Result<Json<WebhookResponse>, StatusCode> {
    warn!("Test webhook endpoint called - should not be used in production!");

    if state.config.environment == "production" {
        error!("Test endpoint called in production environment");
        return Err(StatusCode::NOT_FOUND);
    }

    info!("Processing test webhook");

    // For test endpoint, we'll create a simple merged PR event
    let test_payload = if payload.trim().is_empty() {
        serde_json::json!({
            "action": "closed",
            "pull_request": {
                "id": 12345,
                "number": 42,
                "state": "closed",
                "title": "test: sample pull request",
                "body": "This is a test pull request for webhook testing",
                "merged": true,
                "merge_commit_sha": "abc123def456",
                "base": {
                    "ref": "main",
                    "sha": "def456abc123"
                },
                "head": {
                    "ref": "test/webhook",
                    "sha": "123abc456def"
                }
            },
            "repository": {
                "id": 67890,
                "name": "test-repo",
                "full_name": "testowner/test-repo",
                "default_branch": "main",
                "owner": {
                    "login": "testowner",
                    "type": "User"
                }
            }
        })
    } else {
        match serde_json::from_str(&payload) {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to parse test payload: {}", e);
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    };

    // Create headers for test event
    let mut headers_map = convert_headers(&headers);
    headers_map.insert("x-github-event".to_string(), "pull_request".to_string());
    headers_map.insert("x-github-delivery".to_string(), "test-delivery-123".to_string());

    let action = test_payload
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("closed")
        .to_string();

    let event = WebhookEvent::new(
        "pull_request".to_string(),
        action,
        test_payload,
        headers_map,
    );

    // Create a processor without signature validation for testing
    let test_processor = WebhookProcessor::new(None);

    match test_processor.process_event(&event).await {
        Ok(Some(result)) => {
            info!("Test webhook processed successfully");
            Ok(Json(WebhookResponse {
                status: "test_processed".to_string(),
                message: "Test webhook processed successfully".to_string(),
                processing_result: Some(format!("{:?}", result)),
            }))
        }
        Ok(None) => {
            Ok(Json(WebhookResponse {
                status: "test_ignored".to_string(),
                message: "Test webhook ignored".to_string(),
                processing_result: None,
            }))
        }
        Err(e) => {
            error!("Test webhook processing failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Convert Axum HeaderMap to HashMap<String, String>
fn convert_headers(headers: &HeaderMap) -> HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            let name_str = name.as_str().to_lowercase();
            let value_str = value.to_str().ok()?.to_string();
            Some((name_str, value_str))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let config = AppConfig {
            webhook_secret: Some("test-secret".to_string()),
            github_app_id: "123456".to_string(),
            github_private_key: "test-key".to_string(),
            environment: "test".to_string(),
        };

        let state = AppState::new(config.clone());

        let response = health_check(State(state)).await;

        assert_eq!(response.0.status, "healthy");
        assert_eq!(response.0.environment, "test");
    }

    #[test]
    fn test_convert_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("X-GitHub-Event", "pull_request".parse().unwrap());
        headers.insert("X-Hub-Signature-256", "sha256=abc123".parse().unwrap());

        let converted = convert_headers(&headers);

        assert_eq!(converted.get("x-github-event"), Some(&"pull_request".to_string()));
        assert_eq!(converted.get("x-hub-signature-256"), Some(&"sha256=abc123".to_string()));
    }

    #[tokio::test]
    async fn test_webhook_processor_integration() {
        // Test that we can create a processor and it works with our Azure Function setup
        let processor = WebhookProcessor::new(Some("test-secret".to_string()));

        let payload = serde_json::json!({
            "action": "closed",
            "pull_request": {
                "number": 42,
                "merged": false  // Not merged, should be ignored
            }
        });

        let mut headers = HashMap::new();
        headers.insert("x-github-event".to_string(), "pull_request".to_string());

        let event = WebhookEvent::new(
            "pull_request".to_string(),
            "closed".to_string(),
            payload,
            headers,
        );

        // Should return None for non-merged PR
        let result = processor.process_event(&event).await.unwrap();
        assert!(result.is_none());
    }
}
