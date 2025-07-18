# Getting Started Guide

A step-by-step tutorial to get you up and running with Release Regent

## Overview

This guide will walk you through setting up Release Regent for your project from scratch. By the end of this tutorial, you'll have:

- Release Regent installed and configured
- A working configuration file
- Your first automated release workflow
- Knowledge of how to test and validate your setup

**Prerequisites**:

- Git repository with at least one commit
- GitHub repository (for production use)
- Basic understanding of conventional commits
- Node.js or Rust installed (for installation)

**Time needed**: 15-20 minutes

## Step 1: Install Release Regent

### Option A: Using Cargo (Recommended)

```bash
cargo install release-regent
```

### Option B: Download Pre-built Binary

1. Go to the [releases page](https://github.com/pvandervelde/release_regent/releases)
2. Download the binary for your platform
3. Extract to a directory in your PATH

### Verify Installation

```bash
rr --version
```

You should see output like:

```text
release-regent 0.1.0
```

## Step 2: Initialize Your Project

Navigate to your Git repository and initialize Release Regent:

```bash
cd your-project
rr init
```

This creates two files:

- `release-regent.yml` - Your configuration file
- `sample-webhook.json` - Sample webhook payload for testing

**What just happened?**

Release Regent analyzed your repository and created a basic configuration that should work for most projects. The configuration includes:

- Version prefix detection (if you use "v" tags)
- Basic changelog settings
- Conventional commit parsing rules

## Step 3: Review Your Configuration

Open the generated `release-regent.yml` file:

```yaml
# Version management
versioning:
  prefix: "v"              # Detected from existing tags
  initial_version: "0.1.0" # Starting version if no tags exist
  
# Changelog generation
changelog:
  include_authors: true
  group_by_type: true
  
# Commit parsing
parsing:
  conventional_commits: true
  merge_commits: false
```

**Key settings to review**:

- `versioning.prefix`: Should match your existing tag format
- `versioning.initial_version`: Starting point for version calculations
- `changelog.include_authors`: Whether to show commit authors in changelog

## Step 4: Test Your Setup

Before going live, test your configuration with your existing Git history:

```bash
rr test --commits 10
```

This analyzes your last 10 commits and shows you:
- How commits are parsed
- What version bump would occur
- Generated changelog content

**Example output**:
```
Analyzing 10 commits...

=== Parsed Commits ===
• feat: add user authentication
  Type: feat, Scope: none, Breaking: false
  
• fix(auth): handle invalid tokens
  Type: fix, Scope: auth, Breaking: false

=== Version Calculation ===
Current version: 1.2.0
Next version: 1.3.0 (minor bump from 'feat' commit)

=== Generated Changelog ===
## [1.3.0] - 2024-01-15

### Features
- Add user authentication

### Bug Fixes
- **auth**: Handle invalid tokens
```

## Step 5: Fine-tune Your Configuration

Based on the test output, you might want to adjust your configuration:

### Common Adjustments

**If you want more detailed changelogs**:
```yaml
changelog:
  include_authors: true
  include_commit_links: true
  group_by_type: true
  type_headers:
    feat: "🚀 Features"
    fix: "🐛 Bug Fixes"
    docs: "📚 Documentation"
```

**If you use different commit types**:
```yaml
parsing:
  conventional_commits: true
  additional_types:
    - "perf"
    - "style"
    - "refactor"
```

**If you want to ignore certain commits**:
```yaml
parsing:
  ignore_patterns:
    - "^chore\\(deps\\):"
    - "^docs\\(readme\\):"
```

## Step 6: Test with Sample Data

Test the webhook processing functionality:

```bash
rr run --event-file sample-webhook.json --dry-run
```

This simulates receiving a GitHub webhook and shows you what would happen without making any actual changes.

**Expected output**:
```
Processing webhook event...
Event: pull_request.closed (merged)
Repository: your-org/your-repo
PR #123: Add new feature

=== Analysis Results ===
Version bump: minor (feat commit detected)
Current version: 1.2.0
Next version: 1.3.0

=== Would Create ===
- Git tag: v1.3.0
- GitHub release with changelog
- Release notes from conventional commits

[DRY RUN] No changes made
```

## Step 7: Create Your First Release (Optional)

If you want to test creating an actual release:

1. **Commit your configuration**:
   ```bash
   git add release-regent.yml
   git commit -m "feat: add release automation configuration"
   ```

2. **Test the release process**:
   ```bash
   rr test --commits 1
   ```

3. **Create a manual release** (if confident):
   ```bash
   # This would create an actual tag and release
   # Only run this if you're ready!
   rr run --event-file sample-webhook.json
   ```

## Step 8: Set Up GitHub Integration

For production use, you'll want to integrate with GitHub:

1. **Follow the [GitHub App Setup Guide](github-app-setup.md)** to create a GitHub App
2. **Configure webhook endpoints** using the [Webhook Integration Guide](webhook-integration.md)
3. **Set up your deployment** (Azure Functions, AWS Lambda, etc.)

## Common Issues and Solutions

### "Configuration file not found"
- Make sure you ran `rr init` in your project directory
- Check that `release-regent.yml` exists in your current directory

### "No commits found matching criteria"
- Verify you're in a Git repository with commit history
- Check your `parsing.ignore_patterns` - they might be too restrictive
- Try increasing the `--commits` count in `rr test`

### "Invalid version format"
- Ensure your existing tags follow semantic versioning (e.g., "v1.2.3")
- Check the `versioning.prefix` setting matches your tag format
- Use `git tag -l` to see your current tags

### Unexpected version bumps
- Review your commit messages for conventional commit format
- Check the `parsing.conventional_commits` setting
- Use `rr test --verbose` to see detailed commit parsing

## Next Steps

Now that you have Release Regent working locally:

1. **Read the [Release Automation Guide](release-automation-guide.md)** to understand the workflow
2. **Explore the [Configuration Reference](configuration-reference.md)** for advanced options
3. **Set up production deployment** using the integration guides
4. **Customize your changelog** with templates and formatting options

## Learning Resources

- [Conventional Commits](https://www.conventionalcommits.org/) - Learn the commit message format
- [Semantic Versioning](https://semver.org/) - Understand version numbering
- [GitHub Webhooks](https://docs.github.com/en/developers/webhooks-and-events/webhooks) - How GitHub sends events
- [CLI Reference](cli-reference.md) - Complete command documentation

## Need Help?

- **Configuration questions**: See [Configuration Reference](configuration-reference.md)
- **Command usage**: See [CLI Reference](cli-reference.md)
- **Troubleshooting**: See [Troubleshooting Guide](troubleshooting-guide.md)
- **Issues**: Check the [GitHub Issues](https://github.com/pvandervelde/release_regent/issues)

---

*This guide is part of the Release Regent documentation. For the complete documentation, see the [README](../README.md).*
