# Release Regent Architecture Refactor for Testable Spec Compliance

## Current Architecture Issues

The current architecture makes spec testing difficult because:

1. **Tight coupling**: Spec tests directly import internal modules
2. **No clear boundaries**: Business logic mixed with infrastructure concerns
3. **Hard to test**: No standardized interfaces for end-to-end testing

## Proposed Architecture

### 1. Core Business Logic (release-regent-engine)

**New crate**: `crates/engine/` - Pure business logic with no external dependencies

```rust
// crates/engine/src/lib.rs
pub struct ReleaseEngine {
    config: EngineConfig,
}

impl ReleaseEngine {
    /// Process a webhook event and return processing instructions
    pub async fn process_webhook_event(
        &self,
        event: WebhookEventData,
    ) -> EngineResult<Option<ReleaseInstruction>> {
        // Pure business logic - no I/O, no side effects
    }

    /// Calculate the next version from commit data
    pub async fn calculate_next_version(
        &self,
        commits: &[CommitData],
        current_version: Option<&str>,
    ) -> EngineResult<SemanticVersion> {
        // Pure version calculation logic
    }

    /// Generate changelog from commits
    pub fn generate_changelog(
        &self,
        commits: &[CommitData],
        version: &SemanticVersion,
    ) -> EngineResult<String> {
        // Pure changelog generation
    }
}

/// Instructions for what actions to take (returned by engine)
#[derive(Debug, Clone, PartialEq)]
pub enum ReleaseInstruction {
    CreateReleasePR {
        version: SemanticVersion,
        changelog: String,
        branch_name: String,
        pr_title: String,
        pr_body: String,
    },
    UpdateReleasePR {
        pr_number: u64,
        version: SemanticVersion,
        changelog: String,
        should_rename_branch: bool,
        new_branch_name: Option<String>,
    },
    CreateGitHubRelease {
        version: SemanticVersion,
        tag_name: String,
        release_notes: String,
        target_sha: String,
        is_prerelease: bool,
    },
    NoAction {
        reason: String,
    },
}

/// Input data structures (no external dependencies)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEventData {
    pub event_type: String,
    pub action: String,
    pub repository: RepositoryData,
    pub pull_request: Option<PullRequestData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitData {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
```

### 2. I/O Adapters (release-regent-adapters)

**New crate**: `crates/adapters/` - Handles all external I/O

```rust
// crates/adapters/src/lib.rs
pub struct GitHubAdapter {
    client: GitHubClient,
}

impl GitHubAdapter {
    /// Fetch commits since last release
    pub async fn fetch_commits_since_release(
        &self,
        repo: &RepositoryData,
        since_sha: Option<&str>,
    ) -> AdapterResult<Vec<CommitData>> {
        // GitHub API calls, convert to engine data structures
    }

    /// Execute release instruction
    pub async fn execute_instruction(
        &self,
        instruction: ReleaseInstruction,
        repo: &RepositoryData,
    ) -> AdapterResult<ExecutionResult> {
        match instruction {
            ReleaseInstruction::CreateReleasePR { .. } => {
                // Create PR via GitHub API
            }
            ReleaseInstruction::UpdateReleasePR { .. } => {
                // Update PR via GitHub API
            }
            // etc.
        }
    }
}

pub struct WebhookAdapter;

impl WebhookAdapter {
    /// Convert raw webhook payload to engine data
    pub fn parse_webhook_event(
        event_type: &str,
        payload: &serde_json::Value,
        headers: &HashMap<String, String>,
    ) -> AdapterResult<Option<WebhookEventData>> {
        // Parse GitHub webhook format into engine data structures
    }

    /// Validate webhook signature
    pub fn validate_signature(
        payload: &[u8],
        signature: &str,
        secret: &str,
    ) -> AdapterResult<()> {
        // HMAC validation logic
    }
}
```

### 3. Orchestrator (release-regent-orchestrator)

**New crate**: `crates/orchestrator/` - Coordinates engine + adapters

```rust
// crates/orchestrator/src/lib.rs
pub struct ReleaseOrchestrator {
    engine: ReleaseEngine,
    github_adapter: GitHubAdapter,
    config: OrchestratorConfig,
}

impl ReleaseOrchestrator {
    /// Complete end-to-end webhook processing
    pub async fn process_webhook(
        &self,
        event_type: &str,
        payload: &serde_json::Value,
        headers: &HashMap<String, String>,
    ) -> OrchestratorResult<ProcessingResult> {
        // 1. Parse webhook via adapter
        let event_data = self.webhook_adapter.parse_webhook_event(event_type, payload, headers)?;

        // 2. Get business logic decision from engine
        let instruction = self.engine.process_webhook_event(event_data).await?;

        // 3. Execute instruction via adapter
        if let Some(instruction) = instruction {
            let result = self.github_adapter.execute_instruction(instruction, &repo).await?;
            Ok(ProcessingResult::Executed(result))
        } else {
            Ok(ProcessingResult::NoAction)
        }
    }
}
```

### 4. Thin Entry Points

#### CLI Tool

```rust
// crates/cli/src/main.rs
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.command {
        Commands::Run { event_file } => {
            // Read webhook file
            let payload = fs::read_to_string(event_file)?;
            let webhook_data: serde_json::Value = serde_json::from_str(&payload)?;

            // Create orchestrator
            let orchestrator = create_orchestrator_from_config().await?;

            // Process webhook
            let result = orchestrator.process_webhook(
                "pull_request",
                &webhook_data,
                &HashMap::new(),
            ).await?;

            println!("Result: {:?}", result);
        }
        // Other commands...
    }
}
```

#### Azure Function

```rust
// crates/az_func/src/main.rs
async fn webhook_handler(
    payload: String,
    headers: HeaderMap,
) -> Result<Json<WebhookResponse>, StatusCode> {
    // Parse headers
    let headers_map = extract_headers(&headers);
    let event_type = headers_map.get("x-github-event").unwrap_or("unknown");

    // Parse payload
    let webhook_data: serde_json::Value = serde_json::from_str(&payload)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Create orchestrator
    let orchestrator = create_orchestrator_from_env().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Process webhook
    match orchestrator.process_webhook(event_type, &webhook_data, &headers_map).await {
        Ok(result) => Ok(Json(WebhookResponse::from(result))),
        Err(e) => {
            error!("Processing failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
```

## Spec Testing Approach

### 1. Create Spec Testing Crate

**New crate**: `crates/spec_tests/` - Black-box testing through public APIs

```rust
// crates/spec_tests/src/lib.rs
use release_regent_orchestrator::{ReleaseOrchestrator, ProcessingResult};
use std::time::Duration;

/// Spec test utilities
pub struct SpecTestHarness {
    orchestrator: ReleaseOrchestrator,
}

impl SpecTestHarness {
    /// Create test harness with mock/test configuration
    pub async fn new() -> SpecTestResult<Self> {
        let orchestrator = create_test_orchestrator().await?;
        Ok(Self { orchestrator })
    }

    /// Process webhook and verify timing requirements
    pub async fn process_webhook_with_timing(
        &self,
        event_type: &str,
        payload: serde_json::Value,
    ) -> SpecTestResult<(ProcessingResult, Duration)> {
        let start = Instant::now();

        let result = tokio::time::timeout(
            Duration::from_secs(30),
            self.orchestrator.process_webhook(event_type, &payload, &HashMap::new())
        ).await??;

        let elapsed = start.elapsed();
        Ok((result, elapsed))
    }

    /// Verify behavioral assertions
    pub async fn verify_release_pr_creation(&self, commits: Vec<CommitData>) -> SpecTestResult<()> {
        // Create webhook payload from commits
        let payload = create_merged_pr_webhook(commits);

        // Process and verify result
        let (result, timing) = self.process_webhook_with_timing("pull_request", payload).await?;

        // Behavioral assertions
        assert!(timing <= Duration::from_secs(30), "Must complete within 30 seconds");

        match result {
            ProcessingResult::Executed(execution_result) => {
                // Verify PR was created with correct data
                verify_pr_creation_result(execution_result)?;
            }
            _ => panic!("Expected PR creation result"),
        }

        Ok(())
    }
}

/// Test the complete workflow without external dependencies
async fn create_test_orchestrator() -> SpecTestResult<ReleaseOrchestrator> {
    // Use test/mock adapters that don't make real API calls
    let github_adapter = MockGitHubAdapter::new();
    let engine = ReleaseEngine::new(test_config());

    Ok(ReleaseOrchestrator::new(engine, github_adapter, test_orchestrator_config()))
}
```

### 2. Clean Spec Tests

```rust
// crates/spec_tests/src/behavioral_assertions.rs

/// Behavioral Assertion #1: Release PR creation must complete within 30 seconds
#[tokio::test]
async fn test_release_pr_creation_timing() {
    let harness = SpecTestHarness::new().await.unwrap();

    let commits = vec![
        create_feat_commit("add new feature"),
        create_fix_commit("resolve bug"),
    ];

    // This tests the complete workflow through public APIs only
    harness.verify_release_pr_creation(commits).await.unwrap();
}

/// Behavioral Assertion #2: Version calculations must never downgrade
#[tokio::test]
async fn test_version_downgrade_prevention() {
    let harness = SpecTestHarness::new().await.unwrap();

    // Create scenario with existing higher version
    let existing_pr = create_existing_release_pr("1.5.0");
    let new_commits = vec![create_fix_commit("small fix")]; // Would calculate 1.2.1

    let result = harness.process_release_pr_update(existing_pr, new_commits).await.unwrap();

    // Should preserve existing version, only update changelog
    match result {
        ProcessingResult::Executed(execution_result) => {
            assert_eq!(execution_result.version, "1.5.0", "Should not downgrade version");
            assert!(execution_result.changelog_updated, "Should update changelog");
        }
        _ => panic!("Expected PR update result"),
    }
}
```

### 3. CLI Integration for Spec Tests

```bash
# Add spec test command to CLI
rr spec-test run --behavioral-assertions
rr spec-test run --assertion "release-pr-timing"
rr spec-test verify --config my-config.yml
```

```rust
// crates/cli/src/commands/spec_test.rs
pub async fn run_spec_tests(args: SpecTestArgs) -> Result<(), Box<dyn std::error::Error>> {
    let harness = SpecTestHarness::new().await?;

    match args.assertion {
        Some(assertion) => {
            run_single_assertion(&harness, &assertion).await?;
        }
        None => {
            run_all_behavioral_assertions(&harness).await?;
        }
    }

    Ok(())
}
```

## Benefits of This Approach

### 1. True Black-Box Testing

- Spec tests only use public APIs
- No coupling to internal implementation details
- Tests remain valid through refactoring

### 2. Clean Separation of Concerns

- **Engine**: Pure business logic, easily testable
- **Adapters**: I/O concerns, mockable for testing
- **Orchestrator**: Coordination logic
- **Entry Points**: Thin wrappers

### 3. Multiple Testing Strategies

- **Unit tests**: Test engine logic in isolation
- **Integration tests**: Test orchestrator with mock adapters
- **Spec tests**: End-to-end behavior through public APIs
- **Contract tests**: Verify adapter implementations

### 4. Maintainable and Extensible

- Easy to add new entry points (web UI, etc.)
- Clear boundaries for team development
- Testable architecture supports TDD

Would you like me to start implementing this architecture refactor? I can begin with the engine crate and show how the spec tests would work with this cleaner design.
