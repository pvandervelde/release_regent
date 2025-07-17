# Release Automation Guide

This guide explains how Release Regent's automated workflow works and the concepts behind its two-phase approach to release management.

## Understanding the Workflow

Release Regent implements a **two-phase automated release workflow** that separates preparation from publication.

### Why Two Phases?

The two-phase approach addresses a common challenge in automated releases: finding the right balance between automation and human oversight.

**Phase 1** (Release PR Management) handles version calculation and changelog generation, while **Phase 2** (Release Creation) provides a review point before releases go live.

## Phase 1: Release PR Management

### What Triggers It

Every time a pull request is merged into your main branch, Release Regent analyzes the changes and determines if a release is needed.

### The Decision Process

1. **Commit Analysis**: Release Regent examines all commits since the last release
2. **Convention Parsing**: Uses conventional commit standards to understand change types
3. **Version Calculation**: Applies semantic versioning rules to determine the next version
4. **Release PR Management**: Creates or updates a release PR with the calculated changes

### Version Logic

Release Regent follows semantic versioning strictly:

- **MAJOR**: Breaking changes (`feat!:` or `BREAKING CHANGE:` in commit body)
- **MINOR**: New features (`feat:` commits)
- **PATCH**: Bug fixes (`fix:` commits)

Other commit types (`docs:`, `style:`, `refactor:`, `test:`, `chore:`) don't trigger version bumps but appear in changelogs.

### PR Updates

When Release Regent finds an existing release PR, it handles updates based on version comparison:

**Higher Version Calculated**: Updates the PR with the new version, renames the branch, and merges changelogs. This happens when new features or breaking changes are merged.

**Same Version**: Updates only the changelog content, preserving the existing version. This happens when only patches or non-version-affecting commits are merged.

**Lower Version**: Doesn't downgrade versions. Logs a warning and updates only the changelog. This prevents version conflicts when the automation runs on older commits.

## Phase 2: Release Creation

### What Triggers Release Creation

When you merge a release PR (identified by the `release/v{version}` branch pattern), Release Regent automatically creates the GitHub release.

### The Publication Process

1. **Release Detection**: Identifies merged release PRs by branch naming convention
2. **Version Extraction**: Extracts the version from the branch name or PR title
3. **Release Notes**: Uses the accumulated changelog from the PR body as release notes
4. **Git Tag Creation**: Creates a Git tag pointing to the exact merge commit
5. **GitHub Release**: Creates the GitHub release with proper metadata
6. **Cleanup**: Removes the release branch after successful creation

### Why This Design Works

This two-phase approach has several practical benefits:

**Predictable Timing**: Releases happen when you merge the release PR, not when the automation runs.

**Review Opportunity**: You can examine the calculated version and changelog before merging.

**Straightforward Rollback**: If you need to make changes, you can update the release PR or not merge it.

**Clear Paper Trail**: Every release has a corresponding PR showing what was included and when it was approved.

## Branch Management

### Naming Strategy

Release Regent uses a consistent naming pattern for release branches:

- **Primary**: `release/v{major}.{minor}.{patch}` (e.g., `release/v1.2.3`)
- **Conflict Resolution**: `release/v{major}.{minor}.{patch}-{timestamp}` (e.g., `release/v1.2.3-20250717T143052Z`)
- **Pre-releases**: `release/v{major}.{minor}.{patch}-{prerelease}` (e.g., `release/v1.2.3-beta.1`)

### Conflict Handling

When a branch name already exists (rare but possible), Release Regent automatically appends a timestamp to ensure uniqueness. The PR title and content remain focused on the version, not the branch name.

### Cleanup Process

After successful release creation, Release Regent removes the release branch to keep the repository clean. This happens automatically and doesn't affect the Git tag or GitHub release.

## Configuration and Customization

### Template System

Release Regent uses templates to customize how release PRs look and what information they contain:

```toml
[release_pr]
title_template = "chore(release): prepare version {version}"
body_template = """
## Release {version}

### Changes
{changelog}

### Commits
- {commit_count} commits since last release
- Generated on {date}
"""
```

### Template Variables

Templates support several variables that are populated automatically:

- **`{version}`**: The calculated semantic version (e.g., "1.2.3")
- **`{version_tag}`**: Version with prefix (e.g., "v1.2.3")
- **`{changelog}`**: The generated changelog content
- **`{commit_count}`**: Number of commits since the last release
- **`{date}`**: Current date in ISO format (e.g., "2025-07-17")

### Changelog Generation

The changelog generation respects conventional commit categories:

- **Features**: New functionality (`feat:` commits)
- **Bug Fixes**: Error corrections (`fix:` commits)
- **Documentation**: Documentation changes (`docs:` commits)
- **Performance**: Performance improvements (`perf:` commits)
- **Breaking Changes**: Highlighted separately regardless of category

## Error Handling and Edge Cases

### Malformed Commits

When Release Regent encounters commits that don't follow conventional commit format, it:

1. **Logs a warning** with details about the malformed commit
2. **Continues processing** other commits that are properly formatted
3. **Includes malformed commits** in the changelog under "Other Changes"
4. **Never fails the process** due to commit format issues

### No Version Bump Needed

If all commits since the last release are non-version-affecting (like `docs:`, `style:`, `chore:`), Release Regent:

1. **Skips release PR creation** since no version bump is needed
2. **Logs the decision** for transparency
3. **Waits for the next version-affecting commit** to trigger the workflow

### Rate Limiting and Failures

Release Regent handles GitHub API limitations:

- **Rate Limiting**: Uses exponential backoff and respects GitHub's rate limits
- **Network Failures**: Retries failed operations with backoff patterns
- **Partial Failures**: Can recover from partial operations (e.g., if PR creation succeeds but branch creation fails)

### Concurrent Operations

When multiple PRs are merged quickly, Release Regent:

1. **Processes events sequentially** to avoid conflicts
2. **Uses the latest calculated version** if multiple version calculations happen
3. **Updates existing release PRs** rather than creating duplicates
4. **Handles race conditions** with optimistic locking on GitHub API operations

## Integration Points

### Webhook Processing

Release Regent integrates with your repository through GitHub webhooks:

- **Event Types**: Listens for `pull_request` events with `closed` action and `merged: true`
- **Signature Validation**: Verifies webhook signatures to ensure security
- **Filtering**: Only processes merges to the configured main branch
- **Async Processing**: Handles webhook events asynchronously to avoid timeouts

### GitHub Permissions

The GitHub App requires specific permissions:

- **Repository**: Read access to analyze commits and repository structure
- **Pull Requests**: Write access to create and update release PRs
- **Issues**: Read access to link related issues in changelogs
- **Metadata**: Read access to repository information
- **Contents**: Read access to configuration files

### Configuration Loading

Release Regent loads configuration from your repository:

1. **Primary**: Looks for `release-regent.toml` in the repository root
2. **Fallback**: Uses sensible defaults if no configuration file exists
3. **Validation**: Validates configuration and provides helpful error messages
4. **Template Parsing**: Pre-validates templates to catch errors early

## Benefits and Trade-offs

### Benefits

**Consistency**: Every release follows the same process and format.

**Time Savings**: Automates version calculation, changelog writing, and release note creation.

**Paper Trail**: Every release has a clear record showing what was included and when it was approved.

**Convention Encouragement**: Helps teams adopt conventional commit standards for better change tracking.

### Trade-offs

**Learning Curve**: Teams need to understand conventional commits and the two-phase workflow.

**Convention Dependency**: Works best when teams consistently use conventional commit formats.

**GitHub Specific**: Currently designed for GitHub repositories.

**Extra PR**: Adds a release PR to the workflow, though this is automatically generated and managed.

## Getting Started

To implement this workflow in your project:

1. **Set up the GitHub App** following the [GitHub App Setup Guide](github-app-setup.md)
2. **Deploy webhook processing** using the [Webhook Integration Guide](webhook-integration.md)
3. **Configure templates and settings** as described in the [Configuration Reference](configuration-reference.md)
4. **Test the workflow** using the [Getting Started Guide](getting-started.md)

The automation will start working immediately once configured, processing new PR merges according to the workflow described in this guide.
