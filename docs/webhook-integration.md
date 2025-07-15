# Webhook Integration Guide

This guide covers how Release Regent processes GitHub webhooks with secure signature validation.

## Overview

Release Regent processes GitHub webhooks to automatically manage releases when pull requests are merged. The webhook processing includes:

- **Signature Validation**: Verifies webhook authenticity using HMAC-SHA256
- **Event Filtering**: Processes only relevant events (merged pull requests)
- **Data Extraction**: Extracts repository and pull request information
- **Release Triggering**: Initiates release processing workflow

## Architecture

### Components

1. **WebhookProcessor** (`core::webhook`): Core webhook processing logic
2. **GitHubAuthManager** (`github_client::auth`): Signature validation and authentication
3. **Azure Function** (`az_func`): HTTP endpoint for receiving webhooks

### Flow

```mermaid
sequenceDiagram
    participant GitHub
    participant AzureFunction
    participant WebhookProcessor
    participant GitHubAuth
    participant ReleaseManager

    GitHub->>AzureFunction: POST /webhook (signed payload)
    AzureFunction->>WebhookProcessor: process_event()
    WebhookProcessor->>GitHubAuth: verify_webhook_signature()
    GitHubAuth-->>WebhookProcessor: validation result

    alt signature valid
        WebhookProcessor->>WebhookProcessor: extract_pull_request_info()
        WebhookProcessor->>ReleaseManager: trigger_release_process()
        WebhookProcessor-->>AzureFunction: ProcessingResult
        AzureFunction-->>GitHub: 200 OK
    else signature invalid
        WebhookProcessor-->>AzureFunction: ValidationError
        AzureFunction-->>GitHub: 401 Unauthorized
    end
```

## Signature Validation

### Security Model

Release Regent uses GitHub's webhook signature validation to ensure webhooks are authentic:

1. **HMAC-SHA256**: Uses shared secret to generate signature
2. **Constant-time Comparison**: Prevents timing attacks
3. **Header Validation**: Checks `X-Hub-Signature-256` header
4. **Payload Integrity**: Validates entire request body

### Implementation

```rust
use release_regent_core::webhook::{WebhookProcessor, WebhookEvent};

// Create processor with webhook secret
let processor = WebhookProcessor::new(Some("your-webhook-secret".to_string()));

// Process incoming webhook
let result = processor.process_event(&webhook_event).await?;
```

The signature validation automatically:

- Extracts signature from `X-Hub-Signature-256` header
- Serializes payload for consistent validation
- Uses GitHub client's `verify_webhook_signature()` method
- Returns detailed error on validation failure

## Event Processing

### Supported Events

Currently supports:

- **Pull Request Events**: `action: "closed"` with `merged: true`

Future support planned for:

- Push events to main branch
- Release events
- Repository configuration changes

### Event Structure

```rust
pub struct WebhookEvent {
    pub event_type: String,    // "pull_request"
    pub action: String,        // "closed", "opened", etc.
    pub payload: Value,        // GitHub webhook payload
    pub headers: HashMap<String, String>, // HTTP headers
}
```

### Processing Results

```rust
pub enum ProcessingResult {
    MergedPullRequest {
        repository: RepositoryInfo,
        pull_request: PullRequestInfo,
    },
}
```

## Configuration

### Webhook Setup

1. **Repository Settings**: Go to Settings → Webhooks
2. **Payload URL**: `https://your-function.azurewebsites.net/api/webhook`
3. **Content Type**: `application/json`
4. **Secret**: Generate and store securely
5. **Events**: Select "Pull requests" and "Pushes"

### Environment Variables

```bash
# Azure Function
WEBHOOK_SECRET=your-webhook-secret-here
GITHUB_APP_ID=123456
GITHUB_APP_PRIVATE_KEY="-----BEGIN RSA PRIVATE KEY-----..."
```

### Configuration File

```toml
# release-regent.toml
[webhook]
secret = "your-webhook-secret"
validate_signatures = true

[github]
app_id = 123456
private_key_path = "path/to/private-key.pem"
```

## Error Handling

### Validation Errors

| Error | Cause | Response |
|-------|-------|----------|
| Missing signature header | No `X-Hub-Signature-256` header | 400 Bad Request |
| Invalid signature format | Malformed signature | 400 Bad Request |
| Signature mismatch | Wrong secret or payload tampering | 401 Unauthorized |
| Payload serialization error | Invalid JSON structure | 400 Bad Request |

### Processing Errors

| Error | Cause | Response |
|-------|-------|----------|
| Unsupported event type | Non-PR events | 200 OK (ignored) |
| Missing PR data | Malformed webhook payload | 400 Bad Request |
| GitHub API failure | Network or auth issues | 500 Internal Server Error |

## Testing

### Unit Tests

```bash
# Run webhook tests
cargo test webhook

# Run signature validation tests
cargo test webhook_signature_validation
```

### Integration Testing

```bash
# Test with sample webhook
cargo run --bin rr webhook --sample-payload
```

### Manual Testing

Use the CLI to test webhook processing:

```bash
# Generate sample webhook payload
rr generate-webhook --event pull_request --action closed

# Process webhook locally
rr process-webhook --payload webhook.json --secret "test-secret"
```

## Security Considerations

### Best Practices

1. **Secret Management**: Store webhook secrets securely (Azure Key Vault, etc.)
2. **HTTPS Only**: Always use HTTPS endpoints
3. **Signature Validation**: Never skip signature validation in production
4. **Error Handling**: Don't expose internal errors in responses
5. **Rate Limiting**: Implement rate limiting for webhook endpoints

### Threat Model

**Protected Against**:

- Forged webhooks from untrusted sources
- Payload tampering during transit
- Timing attacks on signature comparison
- Information disclosure through error messages

**Not Protected Against**:

- Compromise of webhook secret
- GitHub account compromise
- Network-level attacks (DDoS)

## Monitoring

### Logging

Release Regent logs webhook processing at multiple levels:

```rust
// Debug: Detailed processing steps
debug!("Validating webhook signature");
debug!("Processing pull request event with action: {}", action);

// Info: Important events
info!("Processing webhook event: {} - {}", event_type, action);
info!("Processing merged PR #{} in {}/{}", pr_number, owner, repo);

// Error: Processing failures
error!("Webhook processing failed: {}", error);
```

### Metrics

Key metrics to monitor:

- Webhook processing rate
- Signature validation success rate
- Event type distribution
- Processing latency
- Error rates by type

## Troubleshooting

### Common Issues

**Signature Validation Fails**:

1. Check webhook secret configuration
2. Verify payload content-type is `application/json`
3. Ensure no middleware modifies the request body
4. Check for trailing whitespace in secret

**Events Not Processing**:

1. Verify event type is supported
2. Check webhook event configuration in GitHub
3. Review application logs for errors
4. Test with sample payloads

**Performance Issues**:

1. Monitor Azure Function cold starts
2. Check GitHub API rate limits
3. Review payload sizes and processing time
4. Consider webhook filtering at GitHub level

### Debug Mode

Enable detailed logging:

```bash
RUST_LOG=release_regent=debug cargo run
```

This provides detailed information about webhook processing steps.
