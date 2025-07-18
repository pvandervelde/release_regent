# Release Regent

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/pvandervelde/release_regent)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

## Overview

Release Regent automates the release process for projects using conventional commits. It handles semantic versioning, changelog generation, and GitHub release creation through a two-phase workflow triggered by pull request merges.

### What Release Regent Does

When you merge a regular pull request, Release Regent creates or updates a release PR with a calculated semantic version and generated changelog. When you merge that release PR, it creates a GitHub release with proper tags and release notes.

The tool uses conventional commit analysis to determine version bumps and handles version conflicts by never downgrading existing release PRs. Templates allow customization of release PR titles, bodies, and changelog formatting.

## How It Works

Release Regent implements a two-phase automated release workflow:

### Phase 1: Release PR Management

When a regular pull request is merged to your main branch:

1. **Webhook Processing**: Release Regent receives the merge event
2. **Version Calculation**: Analyzes commits since the last release using conventional commit standards
3. **Release PR Creation**: Creates or updates a release PR with the calculated version and generated changelog
4. **Smart Updates**: If a release PR already exists, intelligently updates it based on version comparison

### Phase 2: Release Creation

When a release PR is merged:

1. **Release Detection**: Automatically detects merged release PRs by branch pattern
2. **GitHub Release Creation**: Creates a GitHub release with proper Git tags pointing to the merge commit
3. **Release Notes**: Uses the accumulated changelog from the release PR as release notes
4. **Cleanup**: Removes the release branch after successful release creation

### Core Components

- **CLI Tool**: Local development and testing of release workflows
- **Azure Functions Integration**: Webhook-based automation for GitHub repositories
- **GitHub API Client**: Handles all GitHub interactions with proper authentication and rate limiting
- **Configuration System**: Template-based customization of release PRs and changelog formatting

## Getting Started

### For New Users

- 📚 **[Tutorial](docs/tutorial.md)** - 15-minute hands-on introduction
- 📚 **[Getting Started Guide](docs/getting-started-guide.md)** - Step-by-step setup tutorial
- 🔧 **[GitHub App Setup](docs/github-app-setup.md)** - Tutorial for configuring GitHub integration
- 🌐 **[Webhook Integration](docs/webhook-integration.md)** - Tutorial for deploying webhook processing

### For Existing Users

- 📖 **[Configuration Reference](docs/configuration-reference.md)** - Complete configuration options
- 🛠️ **[CLI Reference](docs/cli-reference.md)** - Command-line tool documentation
- 🔍 **[Troubleshooting Guide](docs/troubleshooting.md)** - Common issues and solutions

### Understanding Release Regent

- 💡 **[Release Automation Guide](docs/release-automation-guide.md)** - How the automated workflow works
- 🏗️ **[Architecture Overview](#architecture)** - System design and component relationships

## Quick Start

### CLI Installation

```bash
# Install from source
cargo install --git https://github.com/pvandervelde/release_regent.git release-regent-cli

# Test installation
rr --version
```

### Basic Usage

```bash
# Calculate next version from commits
rr version --current 1.2.0

# Generate changelog for a version
rr changelog --version 1.3.0

# Test complete workflow locally
rr test-workflow --dry-run
```

For complete setup instructions, see the **[Getting Started Guide](docs/getting-started.md)**.

## Architecture

Release Regent uses a multi-crate workspace architecture designed for modularity and deployment flexibility:

```text
release_regent/
├── crates/
│   ├── core/           # Core logic and workflows
│   ├── cli/            # Command-line interface
│   ├── az_func/        # Azure Functions runtime
│   └── github_client/  # GitHub API integration
└── docs/               # Documentation
```

### Component Relationships

**Core Engine** (`release-regent-core`): Contains the release orchestration logic, version calculation algorithms, and configuration management. This is where the main workflow intelligence resides.

**CLI Tool** (`release-regent-cli`): **Published** - Provides local testing capabilities and development workflow integration. Essential for validating configurations and testing release logic.

**Azure Function** (`release-regent-az-func`): **Published** - Webhook processor that connects GitHub events to the core release workflows. Handles authentication, signature validation, and async processing.

**GitHub Client** (`release-regent-github-client`): Internal API client that handles all GitHub interactions with proper rate limiting, retry logic, and error handling.

> **Publication Model**: Only the CLI tool and Azure Function are published for end users. The core and GitHub client components are internal implementation details that ensure clean separation of concerns.

## Configuration Example

Release Regent uses template-based configuration to customize the automated workflow:

```toml
[release_pr]
# Template for release PR titles
title_template = "chore(release): prepare version {version}"

# Template for release PR bodies
body_template = """
## Release {version}

### Changes

{changelog}

### Commits
- {commit_count} commits since last release
- Generated on {date}
"""

[versioning]
prefix = "v"           # Creates tags like "v1.2.3"
allow_prerelease = true

[repository]
main_branch = "main"
release_branch_pattern = "release/v{version}"
```

**Template Variables Available**:

- `{version}` - Calculated semantic version (e.g., "1.2.3")
- `{version_tag}` - Version with prefix (e.g., "v1.2.3")
- `{changelog}` - Generated changelog content
- `{commit_count}` - Number of commits since last release
- `{date}` - Current date in ISO format

See **[Configuration Reference](docs/configuration-reference.md)** for complete options.

## Current Status & Roadmap

Release Regent is in **active development** implementing the complete automation workflow:

### ✅ Completed (v0.2)

- Core version calculation with conventional commits
- CLI tool with basic workflows
- GitHub API client with authentication
- Configuration system foundation

### 🔄 In Progress (v0.3-0.4)

- **Release PR Management**: Automated PR creation and updates
- **Release Automation**: GitHub release creation from merged release PRs
- **Webhook Processing**: Complete Azure Function integration
- **Template System**: Full customization capabilities

### 🎯 Upcoming (v1.0)

- Complete documentation and guides
- Production deployment templates
- Comprehensive error handling and monitoring
- CLI tool publication to crates.io

**Target Timeline**: Version 1.0 planned for Q3 2025

## Contributing

Release Regent welcomes contributions and follows standard Rust development practices:

### Development Setup

```bash
# Clone and build
git clone https://github.com/pvandervelde/release_regent.git
cd release_regent
cargo build --workspace

# Run tests
cargo test --workspace

# Format and lint
cargo fmt && cargo clippy
```

### Contributing Guidelines

- Use conventional commit format for all commits
- Maintain test coverage above 90%
- Update documentation for new features
- All pull requests require CI/CD checks to pass

For detailed contribution guidelines, see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

---

**Made with Rust** | **Semantic Versioning** | **Conventional Commits**
