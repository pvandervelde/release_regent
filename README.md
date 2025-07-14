# Release Regent

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/pvandervelde/release_regent)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

## Overview

Release Regent automates semantic versioning and changelog generation using conventional commits.
It provides a CLI interface and Azure Functions integration for CI/CD pipelines.

### Core Capabilities

- **Semantic Version Calculation**: Version bumping based on conventional commit analysis
- **Changelog Generation**: Template-based changelog generation with git-cliff-core
- **Conventional Commit Parsing**: Commit parsing with git-conventional library integration
- **Multi-Architecture Support**: CLI tool and Azure Functions integration
- **Configuration**: Configuration system for release management

### Changelog Generation

- **Template Engine**: Tera template support for formatting
- **Commit Categorization**: Grouping by conventional commit types (feat, fix, docs, etc.)
- **Author Attribution**: Commit author information in changelogs
- **Link Generation**: GitHub/GitLab commit and PR link generation
- **Backward Compatibility**: Maintains existing API while adding features
- **Fallback Support**: Fallback from advanced to basic changelog generation

### Version Management

- **Semantic Versioning**: Semantic versioning specification support (MAJOR.MINOR.PATCH)
- **Pre-release Support**: Alpha, beta, and custom pre-release identifiers
- **Build Metadata**: Build metadata in version strings
- **Version Prefix Support**: Optional 'v' prefix handling
- **Breaking Change Detection**: Major version bumps for breaking changes

## Architecture

Release Regent uses a multi-crate workspace architecture:

```text
release_regent/
├── crates/
│   ├── core/           # Internal core components
│   ├── cli/            # Command-line interface
│   ├── az_func/        # Azure Functions integration
│   └── github_client/  # GitHub API integration
└── docs/               # Documentation
```

### Crate Descriptions

- **`release-regent-core`**: Internal core components containing version calculation, changelog generation, and configuration management
- **`release-regent-cli`**: **Published** - Command-line tool for local development and CI/CD integration
- **`release-regent-az-func`**: **Published** - Azure Functions runtime for webhook-based automation
- **`release-regent-github-client`**: Internal GitHub API client with authentication and rate limiting

> **Note**: Only the CLI tool and Azure Function are published for end users. The core and GitHub client components are internal implementation details.

## Installation & Usage

### CLI Installation

#### From Source

```bash
# Clone the repository
git clone https://github.com/pvandervelde/release_regent.git
cd release_regent

# Build the CLI tool
cargo build --release -p release-regent-cli

# Install to cargo bin directory
cargo install --path crates/cli
```

#### Using Cargo

```bash
# Install directly from the repository
cargo install --git https://github.com/pvandervelde/release_regent.git release-regent-cli
```

### CLI Usage

```bash
# Calculate next version based on commits
rr version --current 1.2.0 --commits-since-last-tag

# Generate changelog for current version
rr changelog --version 1.3.0 --output-format markdown

# Analyze commits for version impact
rr analyze --from v1.2.0 --to HEAD

# Interactive mode for version calculation
rr interactive
```

### Programmatic Integration

Since the core components are internal, programmatic access to Release Regent functionality is provided through:

1. **CLI Integration**: Execute the CLI tool from your applications
2. **Azure Function Webhooks**: Automated workflows via HTTP endpoints

#### CLI Integration Example

```rust
use std::process::Command;

// Calculate next version using CLI
let output = Command::new("rr")
    .args(&["version", "--current", "1.2.0"])
    .output()
    .expect("Failed to execute rr command");

let next_version = String::from_utf8(output.stdout)
    .expect("Invalid UTF-8")
    .trim();

// Generate changelog using CLI
let changelog_output = Command::new("rr")
    .args(&["changelog", "--version", next_version])
    .output()
    .expect("Failed to generate changelog");

let changelog = String::from_utf8(changelog_output.stdout)
    .expect("Invalid UTF-8");
```

#### Azure Function Integration

For automated workflows, deploy the Azure Function and configure webhooks:

```bash
# Deploy the Azure Function
cd crates/az_func
func azure functionapp publish YourReleaseRegentApp

# Configure webhook in your repository
# POST https://your-function-app.azurewebsites.net/api/webhook
```

## ⚙️ Configuration

### CLI Configuration

Create a `release-regent.toml` configuration file:

```toml
[versioning]
# Version prefix for tags (e.g., "v1.0.0")
prefix = "v"
# Pre-release suffix handling
allow_prerelease = true

[changelog]
# Use advanced git-cliff-core features
use_advanced_generation = true
# Include commit authors in changelog
include_authors = true
# Include commit SHAs
include_shas = true
# Include links to commits and PRs
include_links = true

# Custom template for changelog sections
section_template = """
### {title}

{entries}

"""

# Custom template for commit entries
commit_template = "- {description} [{sha}]"

[repository]
# Repository URL for link generation
remote_url = "https://github.com/owner/repo"
# Main branch name
main_branch = "main"
```

### Advanced Changelog Templates

Release Regent supports Tera templates for changelog customization:

```toml
[changelog.templates]
# Custom header template
header = """
# Changelog

All notable changes to this project will be documented in this file.
"""

# Advanced body template with conditional formatting
body = """
{%- for group, commits in commits | group_by(attribute="group") %}
### {{ group | title }}

{%- for commit in commits %}
{%- if commit.scope %}
- **{{ commit.scope }}**: {{ commit.description }}
{%- else %}
- {{ commit.description }}
{%- endif %}
{%- if include_links %} ([{{ commit.id | truncate(length=7, end="") }}]({{ remote_url }}/commit/{{ commit.id }})){% endif %}
{%- if commit.breaking_change %} ⚠️ **BREAKING**{% endif %}
{%- endfor %}

{%- endfor %}
"""
```

## 🔗 CI/CD Integration

### GitHub Actions

Create `.github/workflows/release.yml`:

```yaml
name: Release Management

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  version-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install Release Regent
        run: cargo install --git https://github.com/pvandervelde/release_regent.git release-regent-cli

      - name: Calculate Next Version
        run: |
          CURRENT_VERSION=$(git describe --tags --abbrev=0 2>/dev/null || echo "0.0.0")
          NEXT_VERSION=$(rr version --current $CURRENT_VERSION)
          echo "next_version=$NEXT_VERSION" >> $GITHUB_OUTPUT

      - name: Generate Changelog
        run: |
          rr changelog --version ${{ steps.version-check.outputs.next_version }} > CHANGELOG.md

      - name: Create Release PR
        if: github.event_name == 'push' && github.ref == 'refs/heads/main'
        uses: actions/create-pull-request@v5
        with:
          title: "chore(release): prepare version ${{ steps.version-check.outputs.next_version }}"
          body: |
            ## Release ${{ steps.version-check.outputs.next_version }}

            $(cat CHANGELOG.md)
          branch: release/${{ steps.version-check.outputs.next_version }}
```

### Azure DevOps

```yaml
# azure-pipelines.yml
trigger:
  branches:
    include: [main]

pool:
  vmImage: 'ubuntu-latest'

steps:
- task: Bash@3
  displayName: 'Install Release Regent'
  inputs:
    targetType: 'inline'
    script: |
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      source ~/.cargo/env
      cargo install --git https://github.com/pvandervelde/release_regent.git release-regent-cli

- task: Bash@3
  displayName: 'Calculate Version and Generate Changelog'
  inputs:
    targetType: 'inline'
    script: |
      source ~/.cargo/env
      CURRENT_VERSION=$(git describe --tags --abbrev=0 2>/dev/null || echo "0.0.0")
      NEXT_VERSION=$(rr version --current $CURRENT_VERSION)
      rr changelog --version $NEXT_VERSION > CHANGELOG.md
      echo "##vso[task.setvariable variable=NextVersion]$NEXT_VERSION"
```

## 🔧 Development

### Building from Source

```bash
# Clone the repository
git clone https://github.com/pvandervelde/release_regent.git
cd release_regent

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Build release version
cargo build --release --workspace
```

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for specific crate
cargo test -p release-regent-core

# Run tests with coverage
cargo tarpaulin --workspace
```

### Development Dependencies

- Rust 1.70 or later
- Git 2.0 or later
- Optional: Docker for containerized testing

## Current Status

Release Regent is in active development with the following completion status:

- ✅ **Core Version Calculation** - Implemented with semantic versioning support
- ✅ **Conventional Commit Parsing** - Parsing with git-conventional library integration
- ✅ **Enhanced Changelog Generation** - git-cliff-core integration with Tera templates
- ✅ **CLI Interface** - Basic structure implemented, features in progress
- ✅ **GitHub Client Components** - Authentication and API integration (internal)
- 🔄 **CI/CD Templates** - GitHub Actions and Azure DevOps examples
- 🔄 **Azure Functions Integration** - Webhook handling for automated workflows
- ⏳ **Publication & Distribution** - CLI and Azure Function packaging in progress

### Test Coverage

- **161+ tests passing** across all crates
- Core functionality tested
- Integration tests for CLI workflows
- Error handling and edge case coverage

### Publication Status

- 🚀 **CLI Tool (`release-regent-cli`)** - Ready for publication to crates.io
- 🚀 **Azure Function (`release-regent-az-func`)** - Ready for deployment
- 🔒 **Core Components** - Internal only, not published separately

## Roadmap

### Version 1.0 Goals

- [ ] Complete CLI interface with all commands
- [ ] Publish CLI tool to crates.io
- [ ] Azure Function deployment templates and documentation
- [ ] Full GitHub integration for automated releases
- [ ] Comprehensive configuration management
- [ ] Complete documentation and examples
- [ ] CI/CD pipeline templates for major platforms

### Future Enhancements

- [ ] GitLab and Bitbucket support
- [ ] Advanced template gallery
- [ ] Multi-repository support
- [ ] Plugin system for custom integrations
- [ ] Web UI for configuration management
- [ ] Docker containers for easy deployment

## Related Projects

Release Regent is part of a suite of GitHub automation tools for software development workflows.

## Contributing

Contributions are welcome. Release Regent uses standard Rust practices and testing.

### Getting Started

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes with tests
4. Ensure all tests pass (`cargo test --workspace`)
5. Run formatting and linting (`cargo fmt && cargo clippy`)
6. Commit your changes (`git commit -m 'feat: add amazing feature'`)
7. Push to your branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

### Development Guidelines

- Follow conventional commit format for all commits
- Maintain test coverage above 90%
- Update documentation for new features
- Use `cargo fmt` for consistent formatting
- Address all `cargo clippy` warnings

### Testing

```bash
# Run all tests
cargo test --workspace

# Run tests with output
cargo test --workspace -- --nocapture

# Run specific test
cargo test test_name -p crate_name
```

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Project Status

Release Regent is in **active development** with a focus on stability and feature coverage for version 1.0.

**Current Version**: 0.1.0 (Pre-release)
**Target 1.0 Release**: Q3 2025
**Stability**: Core features stable, API may evolve

## Future Vision

Release Regent aims to provide automated semantic versioning and changelog generation with:

- **Git Platform Support**: GitHub, GitLab, Bitbucket, and self-hosted solutions
- **Template System**: Changelog templates and formatting options
- **Enterprise Features**: Configuration, audit logging, and compliance reporting
- **Multi-repository Management**: Support for repository structures and dependencies
- **Plugin Architecture**: System for custom integrations and workflows
- **Web Interface**: Optional UI for configuration and monitoring

---

**Made with Rust** | **Semantic Versioning** | **Conventional Commits**
