# GitHub Client

Internal GitHub API client for Release Regent, implementing trait-based operations for Git and GitHub-specific functionality.

## Architecture

This crate provides implementations of two core trait interfaces:

- **`GitOperations`**: Platform-agnostic Git operations (commits, tags, branches, repositories)
- **`GitHubOperations`**: GitHub-specific operations (pull requests, releases, issues)

The implementation uses [github-bot-sdk](https://github.com/pvandervelde/github-bot-sdk) for GitHub API interactions, providing a clean separation between domain logic and GitHub API specifics.

## SDK Migration (Phase 1 Complete)

**Status**: ✅ Phase 1 implementation complete - code compiles and SDK integration functional

### Migration from octocrab to github-bot-sdk

Release Regent has migrated from octocrab to github-bot-sdk to improve architecture, reduce dependencies, and provide better control over GitHub App authentication. The migration follows a three-phase approach:

#### Phase 1: Core Integration (✅ Complete)

- All GitHub operations implemented using github-bot-sdk
- Trait interfaces maintained for clean architecture
- Compilation successful with zero errors
- **Stubbed Operations**:
  - `get_commit()` - Returns `NotSupported` error
  - `get_commits_between()` - Returns empty Vec (for changelog generation)
  - See [SDK Feature Request: Commit Operations](../../.github/SDK_FEATURE_REQUEST_COMMIT_OPERATIONS.md)

#### Phase 2: SDK Enhancement (🚧 In Progress)

- SDK team implementing commit operations:
  - `get_commit(owner, repo, sha)` - Get single commit details
  - `list_commits(owner, repo, ...)` - List commits with filtering
  - `compare_commits(owner, repo, base, head)` - **Critical for changelog**
- Optional field additions:
  - `generate_release_notes` for CreateReleaseRequest
  - `maintainer_can_modify` for CreatePullRequestRequest
  - See [SDK Feature Request: API Enhancements](../../.github/SDK_FEATURE_REQUEST_API_ENHANCEMENTS.md)

#### Phase 3: Full Implementation (⏳ Planned)

- Replace stubbed commit operations with real SDK calls
- Update tests to validate commit operations
- Enable full changelog generation with commit details

### What Works Now (Phase 1)

✅ **Fully Functional:**

- Repository operations (get, branch management)
- Tag operations (list, create, get)
- Release operations (create, update, get, list)
- Pull request operations (create, update, get)
- Authentication via Azure Key Vault
- Rate limiting and retry logic

⚠️ **Temporarily Stubbed:**

- Commit retrieval (returns error)
- Commit comparison for changelog (returns empty list)

### SDK Advantages

- **Reduced Dependencies**: Removed 10 direct dependencies (jsonwebtoken, secrecy, hmac, sha2, hex, url, http, uuid, fastrand, octocrab)
- **Better Authentication**: Native GitHub App support with installation tokens
- **Type Safety**: Strongly-typed API models
- **Maintainability**: Active development and responsive maintainer

## Usage

```rust
use release_regent_github_client::{GitHubClient, AuthConfig, AzureKeyVaultSecretProvider};
use github_bot_sdk::auth::AuthenticationProvider;

// Configure authentication
let auth_config = AuthConfig {
    app_id: 12345,
    private_key: "-----BEGIN RSA PRIVATE KEY-----...".to_string(),
    webhook_secret: "webhook-secret".to_string(),
};

let secret_provider = AzureKeyVaultSecretProvider::new(auth_config)?;

// Create client
let client = GitHubClient::new(
    secret_provider,
    installation_id,
)?;

// Use trait methods
use release_regent_core::traits::git_operations::GitOperations;
let tags = client.list_tags("owner", "repo", Default::default()).await?;
```

## Error Handling

Errors are converted from SDK `ApiError` to Release Regent's `CoreError` with proper context:

```rust
pub enum Error {
    Api { message, source },
    Auth { message, source },
    Network { message, source },
    NotFound { resource },
    InvalidInput { message },
    RateLimit,
    Other { message, source },
}
```

All errors implement proper source chaining for debugging.

## Testing

Tests are being updated to work with github-bot-sdk. The old octocrab-based tests are disabled during the migration.

**Test Status**: 🚧 Tests being updated for SDK (see [#67](https://github.com/pvandervelde/release_regent/pull/67))

## Feature Requests

Active feature requests for github-bot-sdk:

1. **[HIGH Priority] Commit Operations** - Required for Phase 3
   - File: `.github/SDK_FEATURE_REQUEST_COMMIT_OPERATIONS.md`
   - Status: Submitted to SDK team

2. **[MEDIUM Priority] API Enhancements** - Optional field additions
   - File: `.github/SDK_FEATURE_REQUEST_API_ENHANCEMENTS.md`
   - Status: Submitted to SDK team

## Contributing

When contributing to the GitHub client:

1. Maintain trait interface contracts
2. Convert SDK errors to CoreError with context
3. Add comprehensive logging with tracing
4. Document any SDK limitations
5. Update feature requests as needed

For SDK enhancements, contribute to [github-bot-sdk](https://github.com/pvandervelde/github-bot-sdk).
