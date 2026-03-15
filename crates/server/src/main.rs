//! Web server host for Release Regent
//!
//! This application provides an HTTP server that receives GitHub webhook events,
//! validates HMAC-SHA256 signatures using the `github-bot-sdk`, and forwards
//! validated events to the core processing pipeline via an in-memory `mpsc` channel.
//!
//! # Configuration
//!
//! | Env var                  | Description                                          | Default |
//! |--------------------------|------------------------------------------------------|---------|
//! | `GITHUB_WEBHOOK_SECRET`  | HMAC-SHA256 secret shared with GitHub (**required**) | —       |
//! | `ALLOWED_REPOS`          | Comma-separated `owner/repo` values, or `*`          | `*`     |
//! | `EVENT_CHANNEL_CAPACITY` | Bounded channel depth for in-flight events           | `1024`  |
//! | `PORT`                   | TCP port the server listens on                       | `8080`  |
//!
//! # Architecture
//!
//! ```text
//! GitHub HTTPS
//!   └─ POST /webhook  ──►  Axum webhook_handler
//!                               └─ WebhookReceiver (github-bot-sdk)
//!                                       ├─ HMAC-SHA256 signature check
//!                                       └─ ReleaseRegentWebhookHandler
//!                                               └─ mpsc::Sender<ProcessingEvent>
//!                                                            │
//!                                                       WebhookEventSource
//!                                                            └─ run_event_loop
//! ```
//!
//! # Graceful shutdown
//!
//! A `CancellationToken` is shared between the Axum server and the event loop.
//! When `SIGINT` (Ctrl-C) or `SIGTERM` is received, the token is cancelled:
//! - Axum stops accepting new connections after completing in-flight requests.
//! - The event loop finishes processing the current event and then exits.

use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use bytes::Bytes;
use github_bot_sdk::{
    events::{EventProcessor, ProcessorConfig},
    webhook::{WebhookReceiver, WebhookRequest, WebhookResponse},
};
use release_regent_core::run_event_loop;
use std::{collections::HashMap, sync::Arc};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod errors;
mod handler;

use handler::WebhookSecretProvider;

/// Maximum allowed webhook payload size (10 MiB).
///
/// Requests larger than this limit are rejected by the `DefaultBodyLimit` Axum
/// layer before the signature validator even runs.
const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

/// Application state cloned into every Axum request handler.
#[derive(Clone)]
struct AppState {
    receiver: Arc<WebhookReceiver>,
}

/// Health check endpoint.
///
/// Returns `{"status":"healthy","service":"release-regent-webhook"}` with HTTP 200.
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "release-regent-webhook"
    }))
}

/// Receive an incoming GitHub webhook HTTP request.
///
/// Converts the raw Axum headers and body into a [`WebhookRequest`] and
/// delegates signature validation and dispatch to the SDK's
/// [`WebhookReceiver`]. The HTTP response is returned as soon as validation
/// completes; the actual event processing happens asynchronously in the
/// registered [`ReleaseRegentWebhookHandler`] (fire-and-forget).
///
/// | SDK response    | HTTP status |
/// |-----------------|-------------|
/// | `Ok`            | 200         |
/// | `BadRequest`    | 400         |
/// | `Unauthorized`  | 401         |
/// | `InternalError` | 500         |
async fn webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let headers_map: HashMap<String, String> = headers
        .iter()
        .filter_map(|(name, value)| match value.to_str() {
            Ok(v) => Some((name.as_str().to_string(), v.to_string())),
            Err(_) => {
                warn!(header = %name, "Dropping header with non-UTF-8 value");
                None
            }
        })
        .collect();

    let request = WebhookRequest::new(headers_map, body);
    let response = state.receiver.receive_webhook(request).await;

    match response {
        WebhookResponse::Ok { ref event_id, .. } => {
            info!(event_id = %event_id, "Webhook accepted");
            StatusCode::OK
        }
        WebhookResponse::BadRequest { ref message } => {
            warn!(details = %message, "Webhook rejected: bad request");
            StatusCode::BAD_REQUEST
        }
        WebhookResponse::Unauthorized { ref message } => {
            warn!(details = %message, "Webhook rejected: unauthorized");
            StatusCode::UNAUTHORIZED
        }
        WebhookResponse::InternalError { ref message } => {
            error!(details = %message, "Webhook processing error");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Initialise structured logging from `RUST_LOG` or a sensible default filter.
fn setup_logging() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let filter = tracing_subscriber::filter::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            "release_regent_server=debug,release_regent_core=debug,release_regent_github_client=debug,info".into()
        });

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
                .with_level(true),
        )
        .with(filter)
        .init();

    Ok(())
}

/// Main entry point for the Release Regent webhook server.
///
/// # Environment
///
/// See the module-level documentation for the full configuration table.
///
/// # Errors
///
/// Returns an error if:
/// - `GITHUB_WEBHOOK_SECRET` is not set in the environment.
/// - The TCP listener cannot bind to the configured address.
/// - The Axum server exits with an error.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    setup_logging()?;

    info!("Starting Release Regent webhook server");

    // ── Secret / configuration loading ────────────────────────────────────

    // Load webhook secret.
    // Full SecretProvider wiring (Azure Key Vault / AWS Secrets Manager) is task 14.1.
    let github_secret = std::env::var("GITHUB_WEBHOOK_SECRET")
        .map_err(|e| errors::Error::environment("GITHUB_WEBHOOK_SECRET", e.to_string()))?;

    // Allowed repositories: comma-separated "owner/repo" values, or "*" for all.
    let allowed_repos: Vec<String> = std::env::var("ALLOWED_REPOS")
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_else(|_| vec!["*".to_string()]);

    // Bounded channel capacity for in-flight events.
    let channel_capacity: usize = match std::env::var("EVENT_CHANNEL_CAPACITY") {
        Ok(s) => s.parse::<usize>().unwrap_or_else(|_| {
            warn!(
                value = %s,
                variable = "EVENT_CHANNEL_CAPACITY",
                "Invalid value; using default 1024"
            );
            1024
        }),
        Err(_) => 1024,
    };

    // ── Shutdown token ─────────────────────────────────────────────────────

    let shutdown_token = CancellationToken::new();

    // Cancel the token on SIGINT (Ctrl-C) or SIGTERM (sent by Kubernetes / ECS
    // before SIGKILL).  Both signals trigger the same cooperative-shutdown path.
    let signal_token = shutdown_token.clone();
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm =
                signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("Received SIGINT; cancelling");
                }
                _ = sigterm.recv() => {
                    info!("Received SIGTERM; cancelling");
                }
            }
        }
        #[cfg(not(unix))]
        {
            match tokio::signal::ctrl_c().await {
                Ok(()) => info!("Received shutdown signal; cancelling"),
                Err(e) => error!(error = %e, "Failed to install Ctrl-C handler"),
            }
        }
        signal_token.cancel();
    });

    // ── Event source + processing loop ─────────────────────────────────────

    // Build matched handler/source pair sharing a bounded mpsc channel.
    let (webhook_event_handler, event_source) =
        handler::create_webhook_components(allowed_repos, channel_capacity);

    // Spawn the event processing loop.  It runs until the shutdown token is
    // cancelled, processing each `ProcessingEvent` from the mpsc channel.
    let loop_token = shutdown_token.clone();
    let event_loop_handle = tokio::spawn(async move {
        if let Err(e) = run_event_loop(&event_source, loop_token).await {
            error!(error = %e, "Event loop exited with error");
        }
        info!("Event loop stopped");
    });

    // ── HTTP server ────────────────────────────────────────────────────────

    // Build the SDK WebhookReceiver (validates signatures, dispatches to handlers).
    let secret_provider = Arc::new(WebhookSecretProvider::new(github_secret));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let mut receiver = WebhookReceiver::new(secret_provider, processor);
    receiver.add_handler(Arc::new(webhook_event_handler)).await;

    let state = AppState {
        receiver: Arc::new(receiver),
    };

    let app = Router::new()
        .route("/", get(health_check))
        .route("/webhook", post(webhook_handler))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).await?;

    info!(address = %addr, "Server listening");

    // `with_graceful_shutdown` waits for the token to be cancelled before
    // closing the listener and draining in-flight connections.
    let server_token = shutdown_token.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { server_token.cancelled().await })
        .await?;

    // Wait for the event loop to drain any in-flight events before exiting.
    let _ = event_loop_handle.await;

    info!("Shutdown complete");
    Ok(())
}
