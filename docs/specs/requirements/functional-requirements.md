# Functional Requirements

**Last Updated**: 2025-07-19
**Status**: Complete

## Core Functional Requirements

### FR-1: Webhook Event Processing

**Requirement**: The system must process GitHub webhook events to trigger release workflows.

**Details**:

- Accept and validate `pull_request.closed` webhook events
- Verify webhook signatures for security
- Queue events for sequential processing
- Support correlation ID tracking across the entire workflow
- Handle webhook delivery retries from GitHub

**Acceptance Criteria**:

- Webhook signature validation using `X-Hub-Signature-256` header
- Events processed within 30 seconds of receipt
- All events logged with unique correlation IDs
- Invalid signatures rejected with appropriate HTTP status codes
- Malformed payloads handled gracefully with error logging

**Priority**: Critical
**Status**: ✅ Complete

### FR-2: Version Calculation

**Requirement**: The system must calculate semantic versions from commit history using conventional commits.

**Details**:

- Parse commit messages using conventional commit format
- Calculate appropriate semantic version bump (major, minor, patch)
- Support external versioning strategies via plugins
- Handle edge cases (no conventional commits, malformed messages)
- Validate calculated versions against semantic versioning rules

**Acceptance Criteria**:

- `feat:` commits trigger minor version bump
- `fix:` commits trigger patch version bump
- `BREAKING CHANGE:` or `!` trigger major version bump
- Invalid semantic versions are rejected with clear error messages
- Malformed commits do not block version calculation for valid commits
- External versioning strategy support via configurable command execution

**Priority**: Critical
**Status**: ✅ Complete

### FR-3: Release Pull Request Management

**Requirement**: The system must create and update release pull requests based on merged changes.

**Details**:

- Create new release PRs when no existing PR for calculated version
- Update existing release PRs with higher versions and new changelog content
- Never downgrade existing PR versions
- Rename branches and PRs when versions change
- Handle branch naming conflicts with fallback strategies

**Acceptance Criteria**:

- Release PR created within 30 seconds of processing merged PR webhook
- PR uses configurable title and body templates
- Branch naming follows consistent pattern: `release/v{major}.{minor}.{patch}`
- Existing PRs updated with higher versions, preserved for lower versions
- Changelog content merged intelligently for same versions
- Branch conflicts resolved with timestamped fallback naming

**Priority**: Critical
**Status**: 🚧 In Progress

### FR-4: Changelog Generation

**Requirement**: The system must generate changelogs from conventional commit messages.

**Details**:

- Group commits by type (Features, Bug Fixes, Breaking Changes)
- Include commit scopes and descriptions
- Support custom changelog templates
- Handle commits without conventional format
- Merge changelog content for PR updates

**Acceptance Criteria**:

- Changelog sections: Features, Bug Fixes, Breaking Changes, Other
- Commit scopes included when available: `feat(auth): add OAuth support`
- Non-conventional commits included in "Other Changes" section
- Custom templates support variable substitution
- Duplicate entries deduplicated when merging changelogs

**Priority**: High
**Status**: ✅ Complete

### FR-5: GitHub Release Creation

**Requirement**: The system must create GitHub releases when release PRs are merged.

**Details**:

- Detect merged release PRs by branch pattern
- Extract version information from PR branch or title
- Create Git tags pointing to merge commit SHA
- Generate release notes from PR body content
- Clean up release branches after successful creation

**Acceptance Criteria**:

- GitHub release created within 30 seconds of release PR merge
- Git tag points to exact merge commit SHA
- Release notes extracted from PR body changelog section
- Pre-release versions marked appropriately in GitHub releases
- Release branch deleted after successful release creation
- Tag creation conflicts handled gracefully

**Priority**: Critical
**Status**: 📋 Planned

### FR-6: Configuration Management

**Requirement**: The system must support flexible configuration at application and repository levels.

**Details**:

- Load application-wide default configuration
- Override with repository-specific configuration
- Validate configuration before processing
- Support template customization for PR titles and bodies
- Provide clear error messages for invalid configuration

**Acceptance Criteria**:

- Configuration loaded from YAML files
- Repository config overrides application defaults
- Template variables supported: `{version}`, `{version_tag}`, `{changelog}`, `{commit_count}`, `{date}`
- Configuration validation prevents runtime errors
- Invalid configuration reports specific field errors with guidance

**Priority**: High
**Status**: ✅ Complete

### FR-7: CLI Operations

**Requirement**: The system must provide command-line tools for testing and configuration.

**Details**:

- Initialize sample configuration files
- Simulate webhook processing locally
- Preview mode shows changes without execution
- Validate configuration files
- Test against real repositories safely

**Acceptance Criteria**:

- `rr init` generates sample configuration with documentation
- `rr run --event-file webhook.json` processes webhook locally
- `rr preview --version X.Y.Z` shows planned changes without execution
- `rr validate` checks configuration for errors
- CLI operations respect same configuration as runtime system

**Priority**: Medium
**Status**: ✅ Complete

## Data Processing Requirements

### DR-1: Repository Information

**Requirement**: Extract and validate repository information from webhook payloads.

**Data Elements**:

- Repository owner and name
- Pull request number and merge status
- Base and head branch information
- Merge commit SHA
- Pull request author and reviewers

**Validation Rules**:

- Repository owner and name must be valid GitHub identifiers
- Pull request must be merged (not just closed)
- Merge commit SHA must be present for release creation
- Base branch must match configured main branch

### DR-2: Commit Data

**Requirement**: Process commit information for version calculation and changelog generation.

**Data Elements**:

- Commit SHA and author information
- Commit message with conventional format parsing
- Commit timestamp and parent relationships
- Files changed in commit (for scope detection)

**Processing Rules**:

- Parse conventional commit format: `type(scope): description`
- Extract breaking change indicators from message body
- Group commits by type for changelog organization
- Preserve original commit messages for audit trail

### DR-3: Version Information

**Requirement**: Manage semantic version data throughout the workflow.

**Data Elements**:

- Current version from latest release or tag
- Calculated next version based on commits
- Version components (major, minor, patch, pre-release)
- Version metadata (calculation method, override source)

**Validation Rules**:

- All versions must follow semantic versioning specification
- Next version must be higher than current version
- Pre-release versions must include appropriate identifiers
- `!set-version` overrides must specify a version strictly greater than the current released version
- `!release` bump-floor overrides raise the effective version to at least the specified bump kind;
  they never lower a version that conventional commits have already calculated to be higher

### DR-4: PR Comment Commands

**Requirement**: Accept and process version-override commands posted as PR comments.

**Recognised Commands**:

| Command | Effect |
|---------|--------|
| `!set-version X.Y.Z` | Only valid on the active release PR (head branch `release/v*`); pins the next release to exactly version `X.Y.Z` and invokes the release orchestrator immediately |
| `!release major` | Applies a minimum-bump floor label (`rr:override-major`) to the PR the comment was posted on; the floor is evaluated when that PR is merged |
| `!release minor` | Applies `rr:override-minor` to the commented-upon PR |
| `!release patch` | Applies `rr:override-patch` to the commented-upon PR |

**Processing Guards**:

- Commands are only processed when `VersioningConfig::allow_override = true`
- Commenter must have `Write`, `Maintain`, or `Admin` permission on the repository;
  commands from users with insufficient permission produce a `❌` rejection comment
  identifying the commenter and explaining the permission requirement
- Commands on closed or merged PRs are silently ignored
- `!set-version` is only accepted when posted on the active release PR (head branch
  matching `release/v*`); if posted on any other open PR, a scope rejection comment is
  posted and the event is acknowledged without modifying any PR

**Persistence of `!release` overrides**:

- The override label is applied to the **commented-upon PR** (the feature PR), not to
  the release PR
- When a PR carrying an `rr:override-*` label is merged (`PullRequestMerged`), the
  floor is read from that PR and applied during orchestration
- If the PR is closed without merging, the label remains on the closed PR and is never
  consumed — it has no effect on future merges
- **When the release PR is merged** (head branch `release/v*`), all open PRs bearing
  `rr:override-*` labels have those labels removed and receive an informational comment
  explaining that the override was cleared because a release was published. Overrides are
  scoped to one release cycle; contributors must re-post `!release` if the intent still
  applies to the next release.
- Posting a new `!release` command on the same PR replaces the previous override label
- Operators may cancel an override early by manually removing the `rr:override-*` label
  from the feature PR in the GitHub UI

**Audit trail**:

- A **confirmation comment** is posted on the feature PR when an override label is
  recorded, explaining the intent and its scope
- An **audit comment** is posted on the **release PR** when a floor is actually applied
  during orchestration, identifying the source PR and explaining the version change
- A **cleanup comment** is posted on each open feature PR whose override label is cleared
  when a release PR merges, explaining that the override was scoped to the completed
  release cycle and instructing the contributor to re-post if still needed

**Validation Rules**:

- `!set-version` version string must be valid semver and strictly greater than the
  current released version (`>= 0.0.1` when no released version exists); `!set-version`
  must be posted on the release PR or it is rejected with a scope rejection comment
- `!release` bump kind must be one of `major`, `minor`, or `patch` (case-insensitive)

### DR-5: Override Floor Computation

**Requirement**: When a `rr:override-*` label is present on the merged PR, compute the
effective version as the maximum of the conventionally-calculated version and the floor
version.

**Computation**:

Given `current_version` (latest semver tag) and `calculated_version` (from conventional
commits on the merged PR):

- `rr:override-major` floor → `floor_version = current_version.next_major()`
- `rr:override-minor` floor → `floor_version = current_version.next_minor()`
- `rr:override-patch` floor → `floor_version = current_version.next_patch()`
- `effective_version = max(calculated_version, floor_version)`

**Precedence Rules**:

1. The floor is a minimum — it can never reduce a version that conventional commits have
   computed to be higher
2. `BREAKING CHANGE:` commits always produce a major increment regardless of floor
3. `!set-version` is restricted to the release PR: it invokes the orchestrator with a
   specific version immediately when posted on a `release/v*` branch PR; it is rejected
   with a scope rejection comment when posted on any other PR; any `rr:override-*`
   labels on other open PRs are unaffected and will still apply their floors when those
   PRs eventually merge

## Integration Requirements

### IR-1: GitHub API Integration

**Requirement**: Integrate with GitHub API for all repository operations.

**Capabilities**:

- Authenticate using GitHub App installation tokens
- Create, update, and search pull requests
- Create Git tags and GitHub releases
- Fetch commit history and repository information
- Handle API rate limits and retries

**Error Handling**:

- Exponential backoff for transient failures (max 5 retries)
- Circuit breaker for persistent API failures
- Rate limit respect with appropriate delays
- Clear error messages for API permission issues

### IR-2: Git Operations

**Requirement**: Perform Git operations for branch and tag management.

**Capabilities**:

- Create and update release branches
- Rename branches when versions change
- Create Git tags with proper metadata
- Handle branch conflicts and naming collisions
- Clean up branches after release creation

**Error Handling**:

- Fallback naming strategies for branch conflicts
- Graceful handling of missing merge commit SHAs
- Recovery from partial branch operations
- Validation of Git references before operations

## Performance Requirements

### PR-1: Processing Time

**Requirement**: Complete webhook processing within defined time limits.

**Targets**:

- Webhook to Release PR creation: <30 seconds
- Release PR merge to GitHub release: <30 seconds
- Configuration loading and validation: <5 seconds
- Version calculation from commits: <10 seconds

**Considerations**:

- Large repository commit history may affect performance
- GitHub API rate limits may introduce delays
- Serverless cold starts impact initial response time
- Network latency affects external system interactions

### PR-2: Concurrent Processing

**Requirement**: Handle concurrent webhook events safely without data corruption.

**Capabilities**:

- Sequential processing of events per repository
- Optimistic locking for GitHub API operations
- Correlation ID tracking across concurrent operations
- Race condition detection and resolution

**Error Handling**:

- Queue overflow protection with dead letter handling
- Duplicate event detection and deduplication
- Conflict resolution for version race conditions
- Graceful degradation under high load

## Error Handling Requirements

### EH-1: Transient Error Recovery

**Requirement**: Automatically recover from temporary failures.

**Retry Strategy**:

- Base delay: 100ms with exponential backoff
- Maximum delay: 30 seconds
- Jitter: ±25% random variation
- Maximum retries: 5 attempts
- Circuit breaker after consecutive failures

**Covered Scenarios**:

- GitHub API rate limiting
- Network connectivity issues
- Temporary authentication failures
- Webhook delivery retries

### EH-2: Permanent Error Handling

**Requirement**: Handle non-recoverable errors gracefully.

**Response Actions**:

- Log detailed error information with correlation ID
- Send notification if configured
- Skip processing and continue with next event
- Provide actionable error messages for users

**Covered Scenarios**:

- Invalid repository configuration
- Insufficient GitHub permissions
- Malformed webhook payloads
- Invalid semantic version specifications

### EH-3: Partial Failure Recovery

**Requirement**: Continue processing when non-critical operations fail.

**Recovery Strategies**:

- Release creation continues if branch cleanup fails
- Changelog generation continues with fallback for malformed commits
- PR updates continue if template rendering fails using defaults
- Version calculation continues if external strategy fails with fallback

**Priority Levels**:

- **Critical**: Webhook processing, version calculation, PR creation
- **Important**: Changelog generation, template rendering, error notifications
- **Optional**: Branch cleanup, external strategy integration, metrics collection
