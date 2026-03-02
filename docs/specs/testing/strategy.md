# Testing Strategy

**Last Updated**: 2025-07-19
**Status**: New - Addresses Testability Requirements

## Overview

This document defines the comprehensive testing strategy for Release Regent, designed to enable thorough testing without requiring full infrastructure deployment. The strategy emphasizes testability through thin wrapper architecture and dependency injection.

## Testing Challenges in Serverless Environment

### Infrastructure Dependencies

**GitHub API Integration**: Tests need to work without live GitHub API access
**Serverless Runtime**: Tests should not require Azure Functions or AWS Lambda deployment
**External Queues**: Tests need to work without Azure Service Bus or AWS SQS
**Configuration Management**: Tests need predictable, isolated configuration

### Test Data Management

**Webhook Payloads**: Need realistic GitHub webhook event data for testing
**Repository State**: Need to simulate various repository configurations and histories
**API Responses**: Need to mock GitHub API responses for all scenarios
**Error Conditions**: Need to simulate network failures, rate limits, and conflicts

## Testability Architecture

### Thin Wrapper Design

The core testing strategy uses dependency injection and thin wrappers around external services:

```rust
// Trait-based abstraction for GitHub operations
#[async_trait]
pub trait GitHubOperations {
    async fn get_pull_request(&self, number: u64) -> Result<PullRequest, GitHubError>;
    async fn create_pull_request(&self, request: CreatePullRequestRequest) -> Result<PullRequest, GitHubError>;
    async fn update_pull_request(&self, number: u64, request: UpdatePullRequestRequest) -> Result<PullRequest, GitHubError>;
    async fn create_branch(&self, name: &str, sha: &str) -> Result<(), GitHubError>;
    async fn get_commits(&self, base: &str, head: &str) -> Result<Vec<Commit>, GitHubError>;
    async fn create_release(&self, request: CreateReleaseRequest) -> Result<Release, GitHubError>;
}

// Production implementation
pub struct GitHubApiClient {
    octocrab: octocrab::Octocrab,
    owner: String,
    repo: String,
}

#[async_trait]
impl GitHubOperations for GitHubApiClient {
    async fn get_pull_request(&self, number: u64) -> Result<PullRequest, GitHubError> {
        // Real GitHub API implementation
        self.octocrab.pulls(&self.owner, &self.repo).get(number).await
            .map_err(GitHubError::from)
    }

    // ... other implementations
}

// Test implementation
pub struct MockGitHubOperations {
    pull_requests: std::collections::HashMap<u64, PullRequest>,
    branches: std::collections::HashMap<String, String>,
    releases: Vec<Release>,
    pub call_log: Vec<GitHubApiCall>,
}

#[async_trait]
impl GitHubOperations for MockGitHubOperations {
    async fn get_pull_request(&self, number: u64) -> Result<PullRequest, GitHubError> {
        self.call_log.push(GitHubApiCall::GetPullRequest(number));
        self.pull_requests.get(&number)
            .cloned()
            .ok_or(GitHubError::NotFound)
    }

    // ... other mock implementations
}
```

### Core Service Abstraction

The main Release Regent processor uses dependency injection for all external services:

```rust
pub struct ReleaseRegentProcessor<G, C, V>
where
    G: GitHubOperations,
    C: ConfigurationProvider,
    V: VersionCalculator,
{
    github: G,
    config: C,
    version_calculator: V,
    correlation_id: String,
}

impl<G, C, V> ReleaseRegentProcessor<G, C, V>
where
    G: GitHubOperations,
    C: ConfigurationProvider,
    V: VersionCalculator,
{
    pub fn new(github: G, config: C, version_calculator: V, correlation_id: String) -> Self {
        Self {
            github,
            config,
            version_calculator,
            correlation_id,
        }
    }

    pub async fn process_merged_pr(&self, event: WebhookEvent) -> Result<ProcessingResult, ProcessingError> {
        // Business logic that can be tested with mocked dependencies
        let config = self.config.get_repository_config(&event.repository).await?;

        if let Some(existing_pr) = self.find_existing_release_pr(&event).await? {
            return self.update_existing_release_pr(existing_pr, &event, &config).await;
        }

        let version = self.version_calculator.calculate_version(&event).await?;
        let branch_name = self.create_release_branch(&event, &version, &config).await?;
        let pr = self.create_release_pr(&event, &version, &branch_name, &config).await?;

        Ok(ProcessingResult::Created {
            pr_number: pr.number,
            version: version.to_string(),
            branch_name,
            correlation_id: self.correlation_id.clone(),
        })
    }
}
```

### Configuration Testing

```rust
#[async_trait]
pub trait ConfigurationProvider {
    async fn get_repository_config(&self, repository: &Repository) -> Result<RepositoryConfig, ConfigError>;
    async fn get_application_config(&self) -> Result<ApplicationConfig, ConfigError>;
}

// Production implementation - loads from files/environment
pub struct FileConfigurationProvider {
    app_config_path: PathBuf,
}

// Test implementation - uses in-memory configuration
pub struct MockConfigurationProvider {
    pub repository_configs: std::collections::HashMap<String, RepositoryConfig>,
    pub application_config: ApplicationConfig,
}

#[async_trait]
impl ConfigurationProvider for MockConfigurationProvider {
    async fn get_repository_config(&self, repository: &Repository) -> Result<RepositoryConfig, ConfigError> {
        let key = format!("{}/{}", repository.owner, repository.name);
        self.repository_configs.get(&key)
            .cloned()
            .or_else(|| Some(RepositoryConfig::default()))
            .ok_or(ConfigError::NotFound)
    }

    async fn get_application_config(&self) -> Result<ApplicationConfig, ConfigError> {
        Ok(self.application_config.clone())
    }
}
```

## Test Levels and Strategies

### Unit Tests

**Scope**: Individual functions and methods
**Dependencies**: All external dependencies mocked
**Test Data**: Minimal, focused on specific scenarios

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_version_calculation_conventional_commits() {
        let mock_github = MockGitHubOperations::new();
        let mock_config = MockConfigurationProvider::default();
        let version_calculator = ConventionalCommitVersionCalculator::new();

        let processor = ReleaseRegentProcessor::new(
            mock_github,
            mock_config,
            version_calculator,
            "test-correlation-id".to_string(),
        );

        // Add test commits to mock
        let commits = vec![
            create_test_commit("feat: add new feature", "abc123"),
            create_test_commit("fix: resolve bug", "def456"),
        ];
        mock_github.set_commits("main", "feature-branch", commits);

        let event = create_test_webhook_event();
        let result = processor.calculate_next_version(&event).await.unwrap();

        assert_eq!(result.version, semver::Version::new(1, 1, 0)); // Minor bump for 'feat'
    }
}
```

### Integration Tests

**Scope**: Component interactions and workflows
**Dependencies**: Some mocked, some real (non-external)
**Test Data**: Realistic webhook events and repository states

```rust
#[tokio::test]
async fn test_full_release_pr_workflow() {
    let mut mock_github = MockGitHubOperations::new();
    let mut mock_config = MockConfigurationProvider::default();

    // Set up realistic repository state
    mock_config.set_repository_config("owner/repo", RepositoryConfig {
        version_prefix: "v".to_string(),
        branches: BranchConfig { main: "main".to_string() },
        release_pr: ReleasePrConfig::default(),
        ..Default::default()
    });

    // Set up existing repository state
    mock_github.set_existing_releases(vec![
        create_test_release("v1.0.0", "abc123"),
    ]);

    let processor = ReleaseRegentProcessor::new(
        mock_github,
        mock_config,
        ConventionalCommitVersionCalculator::new(),
        "integration-test-id".to_string(),
    );

    let webhook_event = create_realistic_webhook_event();
    let result = processor.process_merged_pr(webhook_event).await.unwrap();

    // Verify the complete workflow
    match result {
        ProcessingResult::Created { pr_number, version, branch_name, .. } => {
            assert_eq!(version, "1.1.0");
            assert_eq!(branch_name, "release/v1.1.0");
            assert!(pr_number > 0);
        }
        _ => panic!("Expected ProcessingResult::Created"),
    }

    // Verify GitHub API calls were made correctly
    let calls = mock_github.get_call_log();
    assert!(calls.contains(&GitHubApiCall::GetCommits("main".to_string(), "feature-branch".to_string())));
    assert!(calls.contains(&GitHubApiCall::CreateBranch("release/v1.1.0".to_string(), "commit-sha".to_string())));
}
```

### Contract Tests

**Scope**: External API contracts and data formats
**Dependencies**: Real external services in test environment
**Test Data**: Production-like data with known outputs

```rust
#[tokio::test]
#[ignore] // Only run in CI with proper GitHub token
async fn test_github_api_contract() {
    let github_client = GitHubApiClient::new(
        std::env::var("GITHUB_TEST_TOKEN").expect("GITHUB_TEST_TOKEN required"),
        "release-regent".to_string(),
        "test-repo".to_string(),
    );

    // Test actual GitHub API contract
    let pr = github_client.get_pull_request(1).await.unwrap();
    assert!(pr.number == 1);
    assert!(!pr.title.is_empty());
}
```

### End-to-End Tests

**Scope**: Complete system behavior
**Dependencies**: Test environment with isolated resources
**Test Data**: Full realistic scenarios

```rust
#[tokio::test]
#[ignore] // Only run in dedicated test environment
async fn test_complete_webhook_to_release_flow() {
    // This test would use a real test repository and webhook
    // but in an isolated test environment
    let test_repo = TestRepositorySetup::new("release-regent-e2e-test").await;

    // Create a test PR and merge it
    let pr = test_repo.create_and_merge_pr_with_commits(vec![
        "feat: add amazing feature",
        "fix: resolve critical bug",
    ]).await;

    // Wait for webhook processing
    let release_pr = test_repo.wait_for_release_pr(Duration::from_secs(60)).await.unwrap();

    // Verify the release PR was created correctly
    assert!(release_pr.title.contains("1.1.0"));
    assert!(release_pr.body.contains("add amazing feature"));
    assert!(release_pr.body.contains("resolve critical bug"));
}
```

## Test Data Management

### Webhook Event Fixtures

```rust
pub struct TestWebhookEventBuilder {
    event: WebhookEvent,
}

impl TestWebhookEventBuilder {
    pub fn new() -> Self {
        Self {
            event: WebhookEvent {
                action: "closed".to_string(),
                pull_request: PullRequest::default(),
                repository: Repository::default(),
                correlation_id: uuid::Uuid::new_v4().to_string(),
            }
        }
    }

    pub fn with_merged_pr(mut self) -> Self {
        self.event.pull_request.merged = Some(true);
        self.event.pull_request.merge_commit_sha = Some("abc123def456".to_string());
        self
    }

    pub fn with_conventional_commits(mut self, commits: Vec<&str>) -> Self {
        self.event.pull_request.commits = commits.iter()
            .enumerate()
            .map(|(i, message)| Commit {
                sha: format!("commit-{}", i),
                message: message.to_string(),
                author: Author::default(),
            })
            .collect();
        self
    }

    pub fn with_repository(mut self, owner: &str, name: &str) -> Self {
        self.event.repository.owner = owner.to_string();
        self.event.repository.name = name.to_string();
        self.event.repository.full_name = format!("{}/{}", owner, name);
        self
    }

    pub fn build(self) -> WebhookEvent {
        self.event
    }
}
```

### GitHub API Response Fixtures

```rust
pub struct GitHubApiResponseFixtures;

impl GitHubApiResponseFixtures {
    pub fn pull_request() -> PullRequest {
        serde_json::from_str(include_str!("fixtures/pull_request.json"))
            .expect("Valid PR fixture")
    }

    pub fn release() -> Release {
        serde_json::from_str(include_str!("fixtures/release.json"))
            .expect("Valid release fixture")
    }

    pub fn commit_with_conventional_message(message: &str) -> Commit {
        Commit {
            sha: "abc123def456".to_string(),
            message: message.to_string(),
            author: Author {
                name: "Test Author".to_string(),
                email: "test@example.com".to_string(),
            },
        }
    }
}
```

## Continuous Integration Testing

### Test Execution Matrix

```yaml
# GitHub Actions test matrix
strategy:
  matrix:
    test-type: [unit, integration, contract]
    rust-version: [stable, beta]
    exclude:
      - test-type: contract
        rust-version: beta

steps:
  - name: Run Unit Tests
    if: matrix.test-type == 'unit'
    run: cargo test --lib

  - name: Run Integration Tests
    if: matrix.test-type == 'integration'
    run: cargo test --test integration_tests

  - name: Run Contract Tests
    if: matrix.test-type == 'contract'
    env:
      GITHUB_TEST_TOKEN: ${{ secrets.GITHUB_TEST_TOKEN }}
    run: cargo test contract_tests -- --ignored
```

### Test Coverage Requirements

**Unit Test Coverage**: Minimum 90% line coverage for core business logic
**Integration Test Coverage**: All major workflow paths covered
**Contract Test Coverage**: All external API interactions verified
**End-to-End Coverage**: Critical user scenarios validated

### Performance Testing

```rust
#[tokio::test]
async fn test_processing_performance() {
    let mock_github = MockGitHubOperations::new();
    let processor = ReleaseRegentProcessor::new(/* ... */);

    let start = std::time::Instant::now();
    let result = processor.process_merged_pr(create_test_event()).await.unwrap();
    let duration = start.elapsed();

    assert!(duration < Duration::from_secs(5), "Processing took too long: {:?}", duration);
}
```

This testing strategy enables comprehensive validation of Release Regent's behavior without requiring full infrastructure deployment, while maintaining confidence in production reliability through realistic test scenarios.
