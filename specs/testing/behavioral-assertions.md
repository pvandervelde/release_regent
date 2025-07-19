# Behavioral Assertions

**Last Updated**: 2025-07-19
**Status**: Complete - Addresses Spec Feedback

## Overview

This document defines testable behavioral assertions for Release Regent that serve as the foundation for black-box testing, implementation validation, and system verification. Each assertion is implementation-agnostic and defines expected system behavior.

## Release PR Management Assertions

### Timing and Performance

**BA-1**: Release PR creation must complete within 30 seconds of receiving a merged PR webhook event.

*Operational Definition*: "Complete" means the entire workflow from webhook receipt to GitHub API completion, including validation, configuration loading, version calculation, and PR creation/update operations.

*Measurement*: 95th percentile response time under normal load conditions.

*Implementation Note*: Includes error handling and logging time but excludes GitHub API propagation delays.

### Version Management

**BA-2**: Version calculations must never downgrade an existing release PR's version.

*Behavior*: If calculated version is lower than existing PR version, preserve existing version and update changelog only. Log warning about version downgrade attempt.

*Edge Case*: Manual version overrides that specify lower versions should be rejected with clear error message.

**BA-3**: PR updates must preserve existing version if calculated version is lower or equal.

*Lower Version*: Preserve existing version, update changelog, log warning.
*Equal Version*: Preserve version, merge changelog content, no warning.
*Higher Version*: Update to new version, replace changelog, update branch if needed.

### Branch and Naming

**BA-4**: Branch naming must be consistent across all operations using pattern `release/v{major}.{minor}.{patch}`.

*Standard Format*: `release/v1.2.3` for regular versions.
*Pre-release Format*: `release/v1.2.3-alpha.1` for pre-release versions.
*Conflict Resolution*: Append timestamp if branch already exists: `release/v1.2.3-1642674000`.

**BA-5**: Branch conflicts must be resolved with timestamped fallback naming.

*Primary Attempt*: Use standard branch name format.
*Conflict Resolution*: Append Unix timestamp: `release/v{version}-{timestamp}`.
*Maximum Attempts*: Try up to 5 different names before failing.

### Changelog Management

**BA-6**: Changelog updates must merge new content with existing content for same versions.

*Merging Strategy*:

- Parse existing changelog by sections (Features, Bug Fixes, Breaking Changes, Other)
- Append new content to appropriate sections
- Deduplicate identical entries by commit SHA
- Preserve chronological order within sections

*Deduplication Logic*:

```markdown
# Existing changelog has:
### Features
- feat(auth): add OAuth support (abc123)

# New changelog has:
### Features
- feat(auth): add OAuth support (abc123)  # Duplicate - skip
- feat(api): add rate limiting (def456)   # New - include

# Result:
### Features
- feat(auth): add OAuth support (abc123)
- feat(api): add rate limiting (def456)
```

### Configuration and Templates

**BA-7**: Configuration templates must support all documented template variables.

*Required Variables*:

- `{version}`: Semantic version (e.g., "1.2.3")
- `{version_tag}`: Version with prefix (e.g., "v1.2.3")
- `{changelog}`: Generated changelog content
- `{commit_count}`: Number of commits since last release
- `{date}`: Current date in ISO 8601 format

*Error Handling*: Missing or invalid variables should use fallback template or fail fast with clear error message.

**BA-8**: Release branches must be created from the correct base branch specified in configuration.

*Default Base*: "main" branch unless overridden in configuration.
*Validation*: Verify base branch exists before creating release branch.
*Error Handling*: Fail with clear error if base branch doesn't exist.

### Search and Matching

**BA-9**: PR searches must only match release branches owned by the application.

*Search Pattern*: Use head branch filter `release/*` to find release PRs.
*Ownership Verification*: Verify branch was created by Release Regent (not manual branches).
*Conflict Avoidance*: Skip PRs created manually with similar naming patterns.

## Release Automation Assertions

### Timing and Performance

**BA-10**: GitHub releases must be created within 30 seconds of merging a release PR.

*Operational Definition*: From release PR merge webhook receipt to GitHub release creation completion.

*Includes*: Version extraction, release notes generation, tag creation, and release publication.

*Excludes*: GitHub notification propagation and branch cleanup (which can happen asynchronously).

### Version and Metadata

**BA-11**: Git tags must point to the exact merge commit SHA from the release PR.

*Tag Target*: Use `merge_commit_sha` from pull request webhook payload.
*Validation*: Verify merge commit SHA is present before creating tag.
*Error Handling*: Fail release creation if merge commit SHA is missing.

**BA-12**: Release notes must be extracted from the release PR body content.

*Extraction Strategy*:

1. Search for changelog section markers: `### Changes`, `## Changes`, `# Changes`, `## Changelog`, `### Changelog`
2. Extract content from marker to end of section (before `### Metadata` or `---`)
3. Use extracted content as GitHub release notes
4. Fallback to full PR body if no changelog section found

*PR Body Format*:

```markdown
## Release v1.2.3

### Changes
#### Features
- feat(auth): add OAuth support
#### Bug Fixes
- fix(parser): handle malformed commits

### Metadata
- Commits: 15 changes since v1.2.0
- Generated: 2025-07-19T10:30:00Z
```

**BA-13**: Version extraction from release PR must handle both branch names and PR titles.

*Priority Order*:

1. Extract from branch name: `release/v1.2.3` → `1.2.3`
2. Fallback to PR title parsing: `chore(release): v1.2.3` → `1.2.3`
3. Fallback to PR body parsing for version information
4. Fail if no valid version found in any location

*Validation*: All extracted versions must pass semantic versioning validation.

### Release Management

**BA-14**: Release branch cleanup must occur after successful release creation.

*Cleanup Order*:

1. Create GitHub release and tag successfully
2. Attempt to delete release branch
3. If cleanup fails, log error but don't fail the release process
4. Continue with success response since release was created

*Error Handling*: Branch cleanup failures should be logged but not block release creation.

**BA-15**: Pre-release versions must be marked appropriately in GitHub releases.

*Detection Logic*: Version contains pre-release identifiers (e.g., "1.2.3-alpha.1", "1.2.3-beta.2")
*GitHub Release Flag*: Set `prerelease: true` for versions with pre-release identifiers
*Draft Flag*: Respect configuration setting for draft releases

**BA-16**: Release creation must not proceed if version extraction fails.

*Validation*: Version extraction must succeed before any GitHub operations
*Error Response*: Provide clear error message indicating why version extraction failed
*No Partial State*: Don't create tags or releases if version is invalid

**BA-17**: Tag creation conflicts must be handled gracefully with clear error messages.

*Conflict Detection*: GitHub API returns 422 if tag already exists
*Error Message*: "Tag v1.2.3 already exists in repository owner/repo"
*Recovery*: Don't attempt to overwrite existing tags
*Logging*: Log detailed context for troubleshooting duplicate tag scenarios

## System-Wide Assertions

### Error Handling and Reliability

**BA-18**: Failed GitHub API calls must retry with exponential backoff up to 5 times.

*Retry Parameters*:

- Base delay: 100ms
- Backoff multiplier: 2x
- Maximum delay: 30 seconds
- Jitter: ±25% random variation
- Maximum attempts: 5

*Eligible Errors*: Network timeouts, HTTP 429 (rate limited), HTTP 502/503 (server errors)
*Non-Eligible Errors*: HTTP 401/403 (auth), HTTP 404 (not found), HTTP 422 (validation)

**BA-19**: Malformed commit messages must not block release PR creation for valid commits.

*Processing Strategy*:

1. Parse all commits using conventional commit parser
2. Successfully parsed commits go into appropriate changelog sections
3. Malformed commits go into "Other Changes" section with basic formatting
4. Continue processing with warning log about malformed commits

*Other Changes Format*:

```markdown
### Other Changes
- [abc1234] Fix the thing (by @username)
- [def5678] Update docs (by @maintainer)
```

**BA-20**: Concurrent webhook processing must not create duplicate release PRs for the same version.

*Concurrency Control*:

- Process webhooks sequentially per repository
- Use optimistic locking for PR updates with ETags
- Check for existing PRs before creating new ones
- Handle race conditions with conflict detection and retry

### Data Integrity

**BA-21**: All operations must be idempotent and safe to retry on failure.

*PR Creation*: Check for existing PR before creating new one
*PR Updates*: Use conditional updates with ETags when possible
*Tag Creation*: Verify tag doesn't exist before creation
*Release Creation*: Check for existing release before creation

**BA-22**: Error messages must include correlation IDs for troubleshooting.

*Correlation ID Format*: `req_{uuid}` (e.g., "req_abc123def456")
*Propagation*: Pass correlation ID through all operations and log entries
*Error Context*: Include correlation ID in all error responses and notifications

### Validation and Security

**BA-23**: Version parsing must strictly follow semantic versioning specification.

*Valid Formats*:

- `1.2.3` (release)
- `1.2.3-alpha.1` (pre-release)
- `1.2.3+build.1` (build metadata)

*Invalid Formats*:

- `v1.2.3` (prefix not allowed in parsing)
- `1.2` (missing patch version)
- `1.2.3.4` (too many components)

**BA-24**: Webhook signature validation must be enforced for all incoming requests.

*Validation Method*: HMAC-SHA256 using configured webhook secret
*Header*: Verify `X-Hub-Signature-256` header matches computed signature
*Timing Attack Prevention*: Use constant-time comparison for signature verification
*Rejection*: Return HTTP 401 for invalid signatures

**BA-25**: Repository configuration must be validated before processing begins.

*Validation Scope*:

- Required fields present (branches.main, version_prefix)
- Template syntax valid (no undefined variables)
- External commands exist and are executable
- URLs and paths properly formatted

*Error Handling*:

- Fail fast with detailed validation errors
- Provide field-specific guidance for fixes
- Include examples of correct configuration

## Testing Implementation

### Verification Methods

Each behavioral assertion can be verified through:

**Black-box Testing**: External observation of system behavior without implementation knowledge

**Integration Testing**: Testing against real GitHub API with test repositories

**Property-based Testing**: Generate random valid inputs and verify behavior properties hold

**Performance Testing**: Measure timing assertions under various load conditions

### Test Scenarios

**Happy Path Tests**: Verify normal operation under expected conditions

**Edge Case Tests**: Test boundary conditions and unusual but valid inputs

**Error Condition Tests**: Verify proper error handling and recovery

**Concurrency Tests**: Test behavior under concurrent access patterns

**Security Tests**: Verify security assertions and attack resistance

### Measurement Infrastructure

**Timing Measurement**:

- Start timer at webhook receipt
- End timer at final GitHub API completion
- Measure 95th percentile across multiple test runs

**Correlation Tracking**:

- Generate correlation IDs for all test operations
- Verify correlation ID appears in all related log entries
- Trace operations across system boundaries

**State Verification**:

- Verify GitHub state matches expected outcomes
- Check PR content, branch names, tag creation
- Validate release metadata and notes

This comprehensive set of behavioral assertions provides clear, testable specifications for Release Regent that address all the gaps identified in the spec feedback while maintaining implementation independence.
