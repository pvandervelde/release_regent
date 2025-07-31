# Release Regent Testing Infrastructure

Comprehensive testing infrastructure and mock implementations for Release Regent.

## Overview

This crate provides all the testing utilities, mock implementations, and test fixtures needed to test Release Regent without requiring external dependencies like GitHub API access or real file systems. It enables fast, reliable, and deterministic testing across all components of the Release Regent system.

## Architecture & Design Philosophy

The testing infrastructure follows these core principles:

- **Isolation**: Tests run without external dependencies (no GitHub API calls, no file system access)
- **Determinism**: Same inputs always produce the same outputs for reliable CI/CD
- **Realism**: Test data closely mirrors real-world GitHub API responses and webhook payloads
- **Flexibility**: Easy configuration for different test scenarios and edge cases
- **Performance**: Fast test execution through in-memory mocks rather than network calls

## Features

- **Mock Implementations**: Complete mock implementations of all core traits
- **Test Data Builders**: Builder pattern for creating test data with realistic values
- **Fixture Management**: Deterministic test fixtures for webhooks and API responses
- **Spec Testing Framework**: Support for behavioral assertion testing
- **HTTP Mocking**: Mock GitHub API server for integration testing

## Quick Start

Add this crate as a dev-dependency in your test code:

```toml
[dev-dependencies]
release_regent_testing = { path = "../testing" }
```

## Mock Implementations

Mock implementations provide complete replacements for external services and dependencies, allowing tests to run in complete isolation. Each mock maintains internal state and can be configured to simulate various scenarios.

### Available Mocks

#### `MockGitHubOperations`

**Purpose**: Replaces real GitHub API calls for testing repository operations.

**Why it exists**:

- Eliminates network dependencies in tests
- Provides deterministic responses regardless of actual GitHub state
- Enables testing of error scenarios (rate limits, network failures, etc.)
- Allows testing without requiring valid GitHub credentials

**Key capabilities**:

- Simulates all GitHub API operations (PRs, releases, commits, tags)
- Configurable response behavior and error conditions
- Call tracking and verification for testing interaction patterns
- Realistic response timing simulation

```rust
use release_regent_testing::mocks::MockGitHubOperations;

let mock_github = MockGitHubOperations::new()
    .with_repository_exists(true)
    .with_pull_requests(vec![test_pr])
    .with_rate_limit_error_after(5); // Simulate rate limiting

// Use in place of real GitHub client
let processor = ReleaseRegentProcessor::new(mock_github, ...);
```

#### `MockVersionCalculator`

**Purpose**: Replaces semantic version calculation logic for testing release workflows.

**Why it exists**:

- Provides predictable version calculations for test scenarios
- Enables testing of complex versioning edge cases
- Allows testing without requiring actual commit history analysis
- Supports testing of different versioning strategies (semantic, calendar, etc.)

**Key capabilities**:

- Configurable version calculation strategies
- Support for pre-release and build metadata
- Error simulation for invalid version scenarios
- Deterministic next version predictions

```rust
use release_regent_testing::mocks::MockVersionCalculator;

let mock_calculator = MockVersionCalculator::new()
    .with_next_version("1.2.0")
    .with_version_type(VersionType::Minor);
```

#### `MockConfigurationProvider`

**Purpose**: Replaces configuration loading and validation for testing different repository setups.

**Why it exists**:

- Eliminates file system dependencies in configuration tests
- Enables testing of various configuration scenarios and validation
- Allows testing of configuration inheritance and overrides
- Supports testing of malformed or invalid configurations

**Key capabilities**:

- In-memory configuration storage
- Validation error simulation
- Multiple configuration profile support
- Dynamic configuration updates during tests

```rust
use release_regent_testing::mocks::MockConfigurationProvider;

let mock_config = MockConfigurationProvider::new()
    .with_valid_config(test_config)
    .with_validation_error_for("invalid-repo");
```

## Test Data Builders

Test data builders use the builder pattern to create realistic test data with sensible defaults while allowing customization of specific fields. They ensure test data consistency and reduce boilerplate code in tests.

### Why Builders Matter

**Consistency**: All builders generate data that conforms to real GitHub API response structures
**Maintainability**: Changes to data structures only require updates in one place
**Readability**: Test intent is clear through fluent builder APIs
**Relationships**: Builders handle complex relationships between objects (PRs have commits, releases have authors, etc.)

### Available Builders

#### `CommitBuilder`

**Purpose**: Creates realistic Git commit objects for testing version calculation and change detection.

**Why it exists**:

- Provides commits with proper conventional commit message formats
- Generates realistic commit metadata (timestamps, SHAs, authors)
- Supports testing of commit parsing and analysis logic

```rust
use release_regent_testing::builders::CommitBuilder;

let feature_commit = CommitBuilder::new()
    .with_conventional_message("feat: add user authentication")
    .with_author("developer@example.com")
    .with_timestamp("2024-01-15T10:30:00Z")
    .build();

let breaking_commit = CommitBuilder::new()
    .with_conventional_message("feat!: redesign user API")
    .with_breaking_change("User.login field removed")
    .build();
```

#### `PullRequestBuilder`

**Purpose**: Creates GitHub pull request objects for testing PR processing and merge workflows.

**Why it exists**:

- Models complete PR lifecycle (draft → open → merged/closed)
- Includes realistic branch relationships and commit history
- Supports testing of PR validation and merge decision logic

```rust
use release_regent_testing::builders::PullRequestBuilder;

let pr = PullRequestBuilder::new()
    .with_title("Add user authentication system")
    .with_base_branch("main")
    .with_head_branch("feature/auth")
    .with_draft(false)
    .with_commits(vec![feature_commit])
    .build();
```

#### `ReleaseBuilder`

**Purpose**: Creates GitHub release objects for testing release creation and publication workflows.

**Why it exists**:

- Models different release types (stable, pre-release, draft)
- Includes proper semantic version formatting and metadata
- Supports testing of release notes generation and publication logic

```rust
use release_regent_testing::builders::ReleaseBuilder;

let release = ReleaseBuilder::new()
    .with_tag_name("v1.2.0")
    .with_name("Authentication Update")
    .with_body("## What's Changed\n- Added user authentication")
    .with_prerelease(false)
    .build();
```

#### `ConfigurationBuilder`

**Purpose**: Creates repository configuration objects for testing different configuration scenarios.

**Why it exists**:

- Models complete Release Regent configuration with all sections
- Enables testing of configuration validation and inheritance
- Supports testing of different versioning strategies and workflows

```rust
use release_regent_testing::builders::ConfigurationBuilder;

let config = ConfigurationBuilder::new()
    .with_versioning_strategy("semantic")
    .with_branch_patterns(vec!["main", "release/*"])
    .with_notification_settings(notification_config)
    .build();
```

#### Other Builders

- **`RepositoryBuilder`**: Creates GitHub repository metadata
- **`VersionBuilder`**: Creates semantic version objects with different formats
- **`VersionContextBuilder`**: Creates version calculation context with commit history
- **`WebhookBuilder`**: Creates GitHub webhook payload objects for event testing

## Fixtures

Fixtures provide pre-built, realistic test data that matches actual GitHub API responses and events. They eliminate the need to construct complex nested data structures in every test.

### Why Fixtures Are Important

**Realism**: Based on actual GitHub API responses and event payloads
**Consistency**: Same data structures used across different tests
**Speed**: Pre-built objects avoid repeated construction overhead
**Maintenance**: Updates to GitHub API changes centralized in fixture definitions

### Available Fixtures

#### GitHub API Response Fixtures

Located in `fixtures/github_api_fixtures.rs`

**Purpose**: Provides realistic GitHub API response data for testing API integration logic.

Examples:

- Repository metadata responses
- Pull request API responses
- Release API responses
- Commit API responses
- Error responses for various failure scenarios

```rust
use release_regent_testing::fixtures::GitHubApiFixtures;

// Get a complete repository response
let repo_response = GitHubApiFixtures::repository_response()
    .with_name("my-project")
    .with_default_branch("main")
    .build();

// Get a pull request response with realistic metadata
let pr_response = GitHubApiFixtures::pull_request_response()
    .with_number(42)
    .with_state("open")
    .build();
```

#### Event Payload Fixtures

Located in `fixtures/event_fixtures.rs`

**Purpose**: Provides realistic GitHub event payloads for testing automation triggers and processing logic.

**Why they exist**:

- GitHub events have complex nested structures that are tedious to construct manually
- Different event types have different required fields and metadata
- Events must match GitHub's exact payload format for accurate testing
- Enable testing of event processing logic without requiring actual GitHub events

```rust
use release_regent_testing::fixtures::EventFixtures;

// GitHub push event with commits
let push_event = EventFixtures::push_event()
    .with_branch("main")
    .with_commits(3)
    .with_repository("owner/repo")
    .build();

// Pull request opened event
let pr_event = EventFixtures::pull_request_event()
    .with_action("opened")
    .with_pr_number(42)
    .build();

// Release published event
let release_event = EventFixtures::release_event()
    .with_action("published")
    .with_tag_name("v1.2.0")
    .build();
```

### Using Fixtures in Tests

Fixtures integrate seamlessly with mocks and builders:

```rust
#[tokio::test]
async fn test_release_processing() {
    // Use fixture for realistic GitHub API response
    let release_response = GitHubApiFixtures::release_response()
        .with_tag_name("v1.0.0")
        .build();

    // Configure mock to return fixture data
    let mock_github = MockGitHubOperations::new()
        .with_get_release_response(release_response);

    // Test with realistic data
    let processor = ReleaseRegentProcessor::new(mock_github);
    let result = processor.process_release("owner", "repo").await;

    assert!(result.is_ok());
}
```

## Spec Testing Framework

The spec testing framework enables behavioral verification against formal specifications. It supports testing that implementations comply with expected behaviors rather than just checking outputs.

### Why Spec Testing Matters

**Behavioral Verification**: Tests verify that code behaves according to specifications, not just that it produces expected outputs
**Living Documentation**: Specs serve as executable documentation of system behavior
**Regression Prevention**: Changes that break behavioral contracts are caught immediately
**Domain Alignment**: Tests express business logic and domain rules in clear terms

### Components

#### `SpecRunner`

**Purpose**: Executes specification tests and provides detailed reporting.

**Key capabilities**:

- Runs specification test suites with detailed results
- Provides clear failure messages with expected vs actual behavior
- Supports parameterized specs for testing multiple scenarios
- Generates compliance reports for audit and documentation

```rust
use release_regent_testing::assertions::SpecRunner;

let spec_runner = SpecRunner::new("Version Calculation Compliance")
    .with_specification("semantic_versioning_spec")
    .with_test_cases(vec![
        ("patch_increment", patch_test_case),
        ("minor_increment", minor_test_case),
        ("major_increment", major_test_case),
    ]);

let results = spec_runner.run().await;
assert!(results.all_passed());
```

#### `ComplianceChecker`

**Purpose**: Verifies that implementations comply with behavioral specifications.

**Key capabilities**:

- Validates that implementations meet specification requirements
- Provides detailed compliance reports with pass/fail status
- Supports custom compliance rules and validation logic
- Enables continuous compliance monitoring

```rust
use release_regent_testing::assertions::ComplianceChecker;

let checker = ComplianceChecker::new("Release Process Compliance")
    .add_rule("must_increment_version", version_increment_rule)
    .add_rule("must_generate_changelog", changelog_generation_rule)
    .add_rule("must_validate_semver", semver_validation_rule);

let compliance = checker.verify(&release_processor).await;
assert!(compliance.is_compliant());
```

#### `BehaviorVerifier`

**Purpose**: Provides utilities for verifying specific behavioral patterns.

**Key capabilities**:

- Verifies state transitions and workflow compliance
- Checks invariant conditions and business rules
- Validates interaction patterns between components
- Supports temporal behavior verification (sequences, ordering)

```rust
use release_regent_testing::assertions::BehaviorVerifier;

let verifier = BehaviorVerifier::new()
    .expect_state_transition("draft", "published")
    .expect_invariant("version_always_increases")
    .expect_interaction_pattern("calculate_then_publish");

verifier.verify_behavior(&release_workflow).await;
```

### Spec Testing Patterns

#### Testing Version Calculation Compliance

```rust
#[tokio::test]
async fn test_semantic_versioning_compliance() {
    let calculator = MockVersionCalculator::new();
    let spec = SemanticVersioningSpec::new();

    assert_spec_compliance!(calculator, spec);
}
```

#### Testing Release Process Workflow

```rust
#[tokio::test]
async fn test_release_workflow_compliance() {
    let processor = create_test_processor();
    let workflow_spec = ReleaseWorkflowSpec::new()
        .with_required_steps(vec!["validate", "calculate", "publish"])
        .with_failure_recovery("rollback_on_error");

    let compliance = ComplianceChecker::new("Release Workflow")
        .verify_against_spec(&processor, &workflow_spec)
        .await;

    assert!(compliance.is_compliant());
}
```

### Integration with Other Testing Components

Spec testing works seamlessly with mocks, builders, and fixtures:

```rust
#[tokio::test]
async fn test_end_to_end_compliance() {
    // Use builders for test data
    let config = ConfigurationBuilder::new()
        .with_versioning_strategy("semantic")
        .build();

    // Use mocks for external dependencies
    let mock_github = MockGitHubOperations::new()
        .with_repository_config(config);

    // Use fixtures for realistic event data
    let push_event = EventFixtures::push_event()
        .with_conventional_commits()
        .build();

    // Verify complete workflow compliance
    let processor = ReleaseRegentProcessor::new(mock_github, mock_calculator);
    let compliance = ComplianceChecker::new("Full Release Cycle")
        .verify_event_processing(&processor, &push_event)
        .await;

    assert!(compliance.is_compliant());
}
```

## Testing Best Practices

### Test Organization Patterns

#### Unit Tests with Mocks

Use mocks to isolate the unit under test from external dependencies:

```rust
#[tokio::test]
async fn test_version_calculation_logic() {
    // Arrange: Use mocks for dependencies
    let mock_github = MockGitHubOperations::new()
        .with_commits(test_commits);

    let calculator = VersionCalculator::new(mock_github);

    // Act: Test the specific logic
    let version = calculator.calculate_next_version("1.0.0").await?;

    // Assert: Verify expected behavior
    assert_eq!(version.to_string(), "1.1.0");
}
```

#### Integration Tests with Fixtures

Use fixtures for realistic data in integration tests:

```rust
#[tokio::test]
async fn test_complete_release_workflow() {
    // Arrange: Use realistic fixture data
    let push_event = EventFixtures::push_event()
        .with_conventional_commits()
        .build();

    let mock_github = MockGitHubOperations::new()
        .with_realistic_responses();

    // Act: Test complete workflow
    let processor = ReleaseRegentProcessor::new(mock_github, config);
    let result = processor.handle_push_event(&push_event).await;

    // Assert: Verify end-to-end behavior
    assert!(result.release_created);
    assert_eq!(result.version, "1.1.0");
}
```

#### Spec Tests for Compliance

Use spec testing for behavioral verification:

```rust
#[tokio::test]
async fn test_semantic_versioning_compliance() {
    let calculator = create_version_calculator();
    let spec = SemanticVersioningSpec::default();

    let compliance = ComplianceChecker::new("Semantic Versioning")
        .verify_against_spec(&calculator, &spec)
        .await;

    assert!(compliance.is_compliant());
}
```

### Error Scenario Testing

Test error conditions systematically:

```rust
#[tokio::test]
async fn test_github_rate_limit_handling() {
    let mock_github = MockGitHubOperations::new()
        .with_rate_limit_error_after(3);

    let processor = ReleaseRegentProcessor::new(mock_github);

    // Should handle rate limiting gracefully
    let result = processor.process_repository("owner", "repo").await;

    match result {
        Err(CoreError::RateLimitExceeded { retry_after }) => {
            assert!(retry_after > Duration::ZERO);
        }
        _ => panic!("Expected rate limit error"),
    }
}
```

### Performance Testing

The testing infrastructure supports performance testing scenarios:

```rust
#[tokio::test]
async fn test_high_volume_processing() {
    let mock_github = MockGitHubOperations::new()
        .with_response_latency(Duration::from_millis(100))
        .with_concurrent_request_limit(10);

    let processor = ReleaseRegentProcessor::new(mock_github);

    // Process multiple events concurrently
    let events: Vec<_> = (0..100)
        .map(|i| EventFixtures::push_event().with_id(i).build())
        .collect();

    let start = Instant::now();
    let results = future::join_all(
        events.iter().map(|event| processor.handle_event(event))
    ).await;
    let duration = start.elapsed();

    // Verify performance characteristics
    assert!(duration < Duration::from_secs(5));
    assert!(results.iter().all(|r| r.is_ok()));
}
```

## Advanced Testing Scenarios

### Testing State Transitions

```rust
#[tokio::test]
async fn test_release_state_machine() {
    let mock_github = MockGitHubOperations::new();
    let processor = ReleaseRegentProcessor::new(mock_github);

    // Test draft → published transition
    let draft_release = ReleaseBuilder::new()
        .with_draft(true)
        .build();

    let published = processor.publish_release(draft_release).await?;

    assert!(!published.draft);
    assert!(published.published_at.is_some());
}
```

### Testing Configuration Variations

```rust
#[tokio::test]
async fn test_different_versioning_strategies() {
    let test_cases = vec![
        ("semantic", "1.2.3", "1.2.4"),
        ("calendar", "2024.1.1", "2024.1.2"),
        ("incremental", "42", "43"),
    ];

    for (strategy, current, expected) in test_cases {
        let config = ConfigurationBuilder::new()
            .with_versioning_strategy(strategy)
            .build();

        let calculator = VersionCalculator::new(config);
        let next = calculator.calculate_next_version(current).await?;

        assert_eq!(next.to_string(), expected);
    }
}
```

### Testing Concurrent Operations

```rust
#[tokio::test]
async fn test_concurrent_release_processing() {
    let mock_github = Arc::new(MockGitHubOperations::new()
        .with_thread_safe_responses());

    let processors: Vec<_> = (0..10)
        .map(|_| ReleaseRegentProcessor::new(Arc::clone(&mock_github)))
        .collect();

    // Process releases concurrently
    let tasks: Vec<_> = processors
        .into_iter()
        .enumerate()
        .map(|(i, processor)| {
            tokio::spawn(async move {
                processor.process_repository("owner", &format!("repo-{}", i)).await
            })
        })
        .collect();

    let results = future::try_join_all(tasks).await?;

    // Verify all succeeded
    assert!(results.iter().all(|r| r.is_ok()));
}
```

## Contributing to Testing Infrastructure

When adding new test types or utilities:

1. **Follow existing patterns**: Use builder pattern for data creation, mock pattern for dependencies
2. **Add comprehensive documentation**: Explain why the test type exists and how to use it
3. **Include realistic examples**: Show common usage patterns in documentation
4. **Maintain thread safety**: All mocks and builders should be thread-safe
5. **Support error simulation**: Include ways to test error conditions
6. **Write tests for test code**: Test utilities should themselves be tested

## Troubleshooting

### Common Issues

- **Mock not returning expected data**: Verify mock configuration matches test expectations
- **Flaky tests**: Check for non-deterministic behavior in mocks or builders
- **Performance test failures**: Verify mock response timing configuration
- **Compilation errors**: Ensure test dependencies match production trait definitions

### Debug Tools

The testing infrastructure includes debug utilities:

```rust
// Enable detailed mock call logging
let mock_github = MockGitHubOperations::new()
    .with_call_logging(true);

// Inspect mock state
println!("Mock call history: {:#?}", mock_github.call_history());

// Verify expected interactions
mock_github.verify_called("get_pull_request", times(3));
```

        .build();

    // Verify complete workflow compliance
    let processor = ReleaseRegentProcessor::new(mock_github, mock_calculator);
    let compliance = ComplianceChecker::new("Full Release Cycle")
        .verify_event_processing(&processor, &push_event)
        .await;

    assert!(compliance.is_compliant());
}

```
