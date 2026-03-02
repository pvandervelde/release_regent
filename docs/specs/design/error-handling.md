# Error Handling Design

**Last Updated**: 2025-07-19
**Status**: Complete - Addresses Spec Feedback

## Overview

This document defines comprehensive error handling strategies for Release Regent, addressing the gaps identified in the spec feedback. It provides concrete implementation details for retry strategies, failure handling, and recovery procedures.

## Operational Definitions

### Timing Requirements

**"Complete within 30 seconds"** means the entire workflow from webhook receipt to final GitHub API completion, including:

- Webhook validation and parsing: <2 seconds
- Configuration loading: <3 seconds
- Version calculation: <8 seconds
- GitHub API operations: <15 seconds
- Error handling and logging: <2 seconds

**Measurement**: 95th percentile response time under normal load conditions.

### Retry Strategy Parameters

**Base retry delay**: 100ms with exponential backoff
**Maximum delay**: 30 seconds between retry attempts
**Jitter**: ±25% random variation to prevent thundering herd effects
**Maximum retries**: 5 attempts for transient failures
**Circuit breaker threshold**: 10 consecutive failures before opening circuit

## Error Classification

### Transient Errors (Retry Eligible)

**Network Errors**:

- Connection timeouts
- DNS resolution failures
- HTTP 502, 503, 504 status codes
- Socket connection errors

**GitHub API Errors**:

- Rate limiting (HTTP 429)
- Server errors (HTTP 500, 502, 503)
- Authentication token expiration
- Temporary API unavailability

**Processing Errors**:

- Memory allocation failures
- Temporary file system issues
- Clock synchronization problems

### Permanent Errors (No Retry)

**Configuration Errors**:

- Invalid YAML syntax
- Missing required configuration fields
- Invalid template syntax
- Malformed version specifications

**Authorization Errors**:

- Invalid GitHub App credentials
- Insufficient repository permissions
- Webhook signature validation failures
- Expired GitHub App installation

**Data Validation Errors**:

- Invalid semantic version format
- Malformed webhook payloads
- Invalid repository identifiers
- Missing required webhook fields

### Critical Errors (Immediate Escalation)

**Security Violations**:

- Webhook signature forgery attempts
- Unauthorized access attempts
- Suspicious activity patterns

**System Failures**:

- Unrecoverable memory errors
- Disk space exhaustion
- Critical dependency failures

## Retry Implementation

### Exponential Backoff Algorithm

```rust
pub struct RetryConfig {
    pub base_delay_ms: u64,         // 100
    pub max_delay_ms: u64,          // 30000
    pub max_attempts: u32,          // 5
    pub jitter_percent: f64,        // 0.25
    pub backoff_multiplier: f64,    // 2.0
}

impl RetryConfig {
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let base_delay = self.base_delay_ms as f64;
        let exponential_delay = base_delay * self.backoff_multiplier.powi(attempt as i32);
        let capped_delay = exponential_delay.min(self.max_delay_ms as f64);

        // Add jitter: ±25% random variation
        let jitter_range = capped_delay * self.jitter_percent;
        let jitter = thread_rng().gen_range(-jitter_range..=jitter_range);
        let final_delay = (capped_delay + jitter).max(0.0);

        Duration::from_millis(final_delay as u64)
    }
}
```

### Circuit Breaker Pattern

```rust
pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    failure_threshold: u32,      // 10
    timeout: Duration,           // 60 seconds
    last_failure_time: Option<Instant>,
}

#[derive(Debug, Clone)]
pub enum CircuitState {
    Closed,      // Normal operation
    Open,        // Failing fast
    HalfOpen,    // Testing recovery
}

impl CircuitBreaker {
    pub async fn call<F, T, E>(&mut self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: Future<Output = Result<T, E>>,
    {
        match self.state {
            CircuitState::Open => {
                if self.should_attempt_reset() {
                    self.state = CircuitState::HalfOpen;
                } else {
                    return Err(CircuitBreakerError::CircuitOpen);
                }
            }
            CircuitState::HalfOpen | CircuitState::Closed => {}
        }

        match operation.await {
            Ok(result) => {
                self.on_success();
                Ok(result)
            }
            Err(error) => {
                self.on_failure();
                Err(CircuitBreakerError::OperationFailed(error))
            }
        }
    }
}
```

## Specific Error Handling Strategies

### Malformed Commit Handling

**Strategy**: Continue processing with fallback formatting

**Implementation**:

1. Parse commits using conventional commit parser
2. For malformed commits, extract basic information (SHA, message, author)
3. Include in changelog under "Other Changes" section
4. Log warning with commit details for user awareness
5. Continue processing remaining commits

**Changelog Format for Malformed Commits**:

```markdown
### Other Changes
- [abc1234] Fix the thing (by @username)
- [def5678] Update docs (by @maintainer)
```

**Warning Log Example**:

```json
{
  "level": "WARN",
  "message": "Malformed conventional commit",
  "commit_sha": "abc123def456",
  "commit_message": "fix the thing",
  "expected_format": "type(scope): description",
  "correlation_id": "req_xyz789"
}
```

### Changelog Merging Strategy

**For Same Version Updates**:

1. Parse existing changelog from PR body
2. Extract sections (Features, Bug Fixes, Breaking Changes, Other)
3. Append new content to appropriate sections
4. Deduplicate identical entries by commit SHA
5. Preserve chronological order within sections

**Deduplication Logic**:

```rust
fn deduplicate_changelog_entries(
    existing: &[ChangelogEntry],
    new: &[ChangelogEntry]
) -> Vec<ChangelogEntry> {
    let mut seen_shas = HashSet::new();
    let mut result = Vec::new();

    // Add existing entries first
    for entry in existing {
        if seen_shas.insert(entry.commit_sha.clone()) {
            result.push(entry.clone());
        }
    }

    // Add new entries, skipping duplicates
    for entry in new {
        if seen_shas.insert(entry.commit_sha.clone()) {
            result.push(entry.clone());
        }
    }

    result
}
```

### Branch Cleanup Failure Handling

**Strategy**: Log error but continue with release creation

**Implementation**:

1. Create GitHub release and tag successfully
2. Attempt to delete release branch
3. If branch deletion fails:
   - Log error with correlation ID
   - Include branch name and repository for manual cleanup
   - Continue with success response (release was created)
   - Optionally notify maintainers of cleanup failure

**Error Handling**:

```rust
async fn cleanup_release_branch(&self, repo: &str, branch: &str) -> Result<(), GitHubError> {
    match self.delete_branch(repo, branch).await {
        Ok(_) => {
            info!("Release branch deleted successfully",
                  repository = repo, branch = branch);
            Ok(())
        }
        Err(error) => {
            warn!("Failed to delete release branch, manual cleanup required",
                  repository = repo,
                  branch = branch,
                  error = ?error,
                  cleanup_required = true);
            // Don't propagate error - release creation succeeded
            Ok(())
        }
    }
}
```

### Template Rendering Failure

**Strategy**: Use fallback template or fail fast with clear error

**Fallback Templates**:

```rust
const FALLBACK_PR_TITLE: &str = "chore(release): {version}";
const FALLBACK_PR_BODY: &str = r#"
## Release {version}

{changelog}

---
*This release was automatically generated*
"#;

async fn render_pr_template(
    template: &str,
    variables: &TemplateVariables
) -> Result<String, TemplateError> {
    match render_template(template, variables) {
        Ok(rendered) => Ok(rendered),
        Err(error) => {
            warn!("Template rendering failed, using fallback",
                  template = template,
                  error = ?error);

            // Use fallback template
            render_template(FALLBACK_PR_TITLE, variables)
                .map_err(|fallback_error| {
                    TemplateError::FallbackFailed {
                        original_error: Box::new(error),
                        fallback_error: Box::new(fallback_error),
                    }
                })
        }
    }
}
```

### Configuration Validation Failure

**Strategy**: Fail fast with detailed validation errors

**Validation Error Format**:

```rust
#[derive(Debug)]
pub struct ConfigValidationError {
    pub field_path: String,
    pub error_type: ValidationErrorType,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug)]
pub enum ValidationErrorType {
    Missing,
    InvalidFormat,
    InvalidValue,
    InvalidReference,
}

// Example validation errors
vec![
    ConfigValidationError {
        field_path: "branches.main".to_string(),
        error_type: ValidationErrorType::Missing,
        message: "Main branch name is required".to_string(),
        suggestion: Some("Add 'branches.main: \"main\"' to your configuration".to_string()),
    },
    ConfigValidationError {
        field_path: "versioning.external.command".to_string(),
        error_type: ValidationErrorType::InvalidReference,
        message: "External command does not exist or is not executable".to_string(),
        suggestion: Some("Ensure the script exists and has execute permissions".to_string()),
    }
]
```

## Concurrency Control

### Webhook Queue Processing

**Strategy**: Process webhooks sequentially with correlation ID tracking

**Implementation**:

```rust
pub struct WebhookQueue {
    queue: Arc<Mutex<VecDeque<WebhookEvent>>>,
    processing: Arc<AtomicBool>,
    processor: Arc<dyn WebhookProcessor>,
}

impl WebhookQueue {
    pub async fn enqueue(&self, event: WebhookEvent) -> Result<(), QueueError> {
        let mut queue = self.queue.lock().await;

        if queue.len() >= MAX_QUEUE_SIZE {
            return Err(QueueError::QueueFull);
        }

        queue.push_back(event);

        // Start processing if not already running
        if !self.processing.load(Ordering::Acquire) {
            tokio::spawn(self.process_queue());
        }

        Ok(())
    }

    async fn process_queue(&self) {
        self.processing.store(true, Ordering::Release);

        while let Some(event) = {
            let mut queue = self.queue.lock().await;
            queue.pop_front()
        } {
            if let Err(error) = self.processor.process_event(event).await {
                error!("Failed to process webhook event", error = ?error);
            }
        }

        self.processing.store(false, Ordering::Release);
    }
}
```

### Version Conflict Resolution

**Strategy**: Use optimistic locking with GitHub API ETags

**Implementation**:

```rust
async fn update_pr_with_etag(
    &self,
    pr_number: u64,
    updates: &PullRequestUpdate,
    expected_etag: Option<&str>
) -> Result<PullRequest, GitHubError> {
    let mut request = self.client
        .pulls(&self.owner, &self.repo)
        .update(pr_number)
        .title(&updates.title)
        .body(&updates.body);

    // Add If-Match header for optimistic locking
    if let Some(etag) = expected_etag {
        request = request.header("If-Match", etag);
    }

    match request.send().await {
        Ok(pr) => Ok(pr),
        Err(octocrab::Error::GitHub { source, .. }) if source.status_code == 412 => {
            // Precondition failed - conflict detected
            Err(GitHubError::ConflictDetected {
                resource: "pull_request".to_string(),
                identifier: pr_number.to_string(),
            })
        }
        Err(error) => Err(GitHubError::from(error)),
    }
}
```

### Duplicate Detection

**Strategy**: Check for existing PRs before creation, update if found

**Implementation**:

```rust
async fn find_or_create_release_pr(
    &self,
    version: &SemanticVersion,
    changelog: &str
) -> Result<ReleasePROperation, GitHubError> {
    // Search for existing release PRs
    let existing_prs = self.search_release_prs().await?;

    match existing_prs.into_iter().find(|pr| pr.is_release_pr()) {
        Some(existing_pr) => {
            let existing_version = self.extract_version_from_pr(&existing_pr)?;

            match version.cmp(&existing_version) {
                Ordering::Greater => {
                    // Update with higher version
                    self.update_pr_with_higher_version(&existing_pr, version, changelog).await
                }
                Ordering::Equal => {
                    // Merge changelog for same version
                    self.update_pr_changelog(&existing_pr, changelog).await
                }
                Ordering::Less => {
                    // Never downgrade, just update changelog
                    warn!("Calculated version {} lower than existing PR version {}",
                          version, existing_version);
                    self.update_pr_changelog(&existing_pr, changelog).await
                }
            }
        }
        None => {
            // Create new release PR
            self.create_new_release_pr(version, changelog).await
        }
    }
}
```

## PR Body Format Specification

### Required Structure

Release PR bodies must contain a changelog section that can be extracted for GitHub release notes:

```markdown
## Release {version}

Brief description of the release (optional).

### Changes

#### Features
- feat(auth): add OAuth support (#123)
- feat(api): implement rate limiting (#124)

#### Bug Fixes
- fix(parser): handle malformed commit messages (#125)
- fix(config): validate template syntax (#126)

#### Breaking Changes
- feat!: change API response format (#127)

#### Other Changes
- [abc1234] Update documentation (by @maintainer)
- [def5678] Fix typo in readme (by @contributor)

### Metadata
- **Commits**: 15 changes since v1.2.0
- **Generated**: 2025-07-19T10:30:00Z
- **Correlation ID**: req_abc123def456
```

### Extraction Logic

```rust
fn extract_changelog_from_pr_body(body: &str) -> Result<String, ExtractionError> {
    // Look for changelog section markers
    let changelog_markers = [
        "### Changes",
        "## Changes",
        "# Changes",
        "## Changelog",
        "### Changelog"
    ];

    for marker in &changelog_markers {
        if let Some(start_index) = body.find(marker) {
            // Find the end of the changelog section
            let changelog_start = start_index + marker.len();
            let end_index = body[changelog_start..]
                .find("\n### Metadata")
                .or_else(|| body[changelog_start..].find("\n---"))
                .map(|i| changelog_start + i)
                .unwrap_or(body.len());

            let changelog = body[changelog_start..end_index].trim();
            return Ok(changelog.to_string());
        }
    }

    Err(ExtractionError::NoChangelogSection)
}
```

## Error Communication

### User-Facing Error Messages

**Configuration Validation Errors**:

```
❌ Configuration validation failed for repository owner/repo:

1. branches.main (MISSING): Main branch name is required
   Suggestion: Add 'branches.main: "main"' to your configuration

2. versioning.external.command (INVALID_REFERENCE): External command does not exist
   Suggestion: Ensure ./scripts/version.sh exists and has execute permissions

3. release_pr.title_template (INVALID_FORMAT): Template contains undefined variable
   Suggestion: Use {version} instead of {release_version}

Fix these issues in your .release-regent.yml file and try again.
```

**Processing Errors**:

```
⚠️ Release PR creation completed with warnings for owner/repo:

✅ Created release PR #42 for version v1.2.3
⚠️ Failed to parse 2 commit messages (see logs for details)
⚠️ Could not delete release branch (manual cleanup required)

Correlation ID: req_abc123def456
```

### Operational Error Logs

**Structured Error Logging**:

```json
{
  "timestamp": "2025-07-19T10:30:00Z",
  "level": "ERROR",
  "correlation_id": "req_abc123def456",
  "repository": "owner/repo",
  "operation": "create_release_pr",
  "error_type": "github_api_error",
  "error_code": "rate_limited",
  "retry_attempt": 3,
  "next_retry_in_ms": 4000,
  "context": {
    "pr_number": 42,
    "version": "1.2.3",
    "github_status_code": 429,
    "rate_limit_reset": "2025-07-19T10:35:00Z"
  }
}
```

## Recovery Procedures

### Dead Letter Queue Processing

**Failed Event Storage**:

```rust
pub struct DeadLetterQueue {
    storage: Box<dyn DeadLetterStorage>,
    max_retention_days: u32,
}

impl DeadLetterQueue {
    pub async fn store_failed_event(
        &self,
        event: &WebhookEvent,
        error: &ProcessingError,
        attempt_count: u32
    ) -> Result<(), StorageError> {
        let dead_letter = DeadLetter {
            id: Uuid::new_v4(),
            original_event: event.clone(),
            failure_reason: error.to_string(),
            attempt_count,
            first_attempt: event.received_at,
            last_attempt: Utc::now(),
            correlation_id: event.correlation_id.clone(),
        };

        self.storage.store(dead_letter).await
    }

    pub async fn replay_events(
        &self,
        filter: &DeadLetterFilter
    ) -> Result<Vec<ReplayResult>, StorageError> {
        let events = self.storage.list_events(filter).await?;
        let mut results = Vec::new();

        for event in events {
            match self.processor.process_event(&event.original_event).await {
                Ok(_) => {
                    self.storage.mark_resolved(&event.id).await?;
                    results.push(ReplayResult::Success(event.id));
                }
                Err(error) => {
                    results.push(ReplayResult::Failed(event.id, error));
                }
            }
        }

        Ok(results)
    }
}
```

### Manual Recovery Tools

**CLI Recovery Commands**:

```bash
# Replay failed webhook events
rr recover replay --since=2025-07-19 --repository=owner/repo

# Fix corrupted release PR
rr recover fix-pr --pr=42 --version=1.2.3 --repository=owner/repo

# Clean up orphaned release branches
rr recover cleanup-branches --dry-run --repository=owner/repo

# Validate configuration before applying
rr recover validate-config --file=.release-regent.yml
```

This comprehensive error handling design addresses all the gaps identified in the spec feedback, providing concrete implementation details for reliable operation of Release Regent.
