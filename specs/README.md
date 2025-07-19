# Release Regent Specification

**Version**: 2.0
**Last Updated**: 2025-07-19
**Status**: Active

## Overview

Release Regent is a GitHub App that automates release management by creating and updating release pull requests, determining semantic versions, and publishing GitHub releases. It's designed for teams who want automated releases without the complexity of full continuous deployment.

### What It Does

Release Regent watches for merged pull requests and automatically:

- Creates release PRs with calculated semantic versions
- Updates existing release PRs with new changes and higher versions if needed
- Generates changelogs based on conventional commits
- Creates GitHub releases and tags when release PRs are merged
- Provides a CLI for local testing and configuration

### Who It's For

**Repository Maintainers** who want consistent, automated releases without manual overhead.

**Enterprise Teams** who need controlled, auditable release processes that integrate with existing workflows.

## Problem & Solution

### The Problem

Most teams fall into one of two camps: either they deploy every merge to main (full CD), or they manage releases manually. There's a gap for teams who want automated release management but need control over timing.

**Current pain points**:

- **Manual overhead**: Creating releases manually is time-consuming and error-prone
- **Inconsistent processes**: Different team members follow different procedures
- **Timing mismatch**: Need to release when ready, not on every merge
- **Tool limitations**: Existing tools are either unreliable or do too much

### Our Solution

Release Regent sits in the sweet spot between manual releases and full continuous deployment. It automates all the mechanical parts (version calculation, PR creation, changelog generation) while letting developers control when releases actually happen.

**How it works**:

1. Developer merges a PR with conventional commit messages
2. Release Regent creates or updates a release PR with the calculated version
3. Developer reviews and merges the release PR when ready
4. Release Regent creates the GitHub release and tag automatically

## Navigation

### 📋 Requirements & Planning

- [User Stories & Personas](requirements/user-stories.md) - Who uses Release Regent and why
- [Functional Requirements](requirements/functional-requirements.md) - What the system must do
- [Non-Functional Requirements](requirements/non-functional-requirements.md) - Performance, security, reliability

### 🏗️ Architecture & Design

- [System Architecture](architecture/overview.md) - High-level system design
- [Core Components](architecture/components.md) - Modules and responsibilities
- [Data Flow](architecture/data-flow.md) - Request processing workflows
- [Integration Points](architecture/integration-points.md) - External system interactions

### 🔧 Feature Design

- [Release PR Management](design/release-pr-management.md) - Release PR creation and updates
- [Release Automation](design/release-automation.md) - GitHub release creation workflow
- [Versioning Strategy](design/versioning-strategy.md) - Version calculation and strategies
- [Error Handling](design/error-handling.md) - Comprehensive error handling
- [Concurrency Control](design/concurrency-control.md) - Race conditions and locking

### 🚀 Operations

- [Configuration](operations/configuration.md) - Schema, validation, and templates
- [Logging & Monitoring](operations/logging-monitoring.md) - Observability and debugging
- [Deployment](operations/deployment.md) - Infrastructure and CI/CD
- [Maintenance](operations/maintenance.md) - Operational procedures

### 🔒 Security

- [Authentication](security/authentication.md) - GitHub App auth and tokens
- [Webhook Security](security/webhook-security.md) - Signature validation and protection
- [Secrets Management](security/secrets-management.md) - Credential storage and rotation

### 🧪 Testing

- [Testing Strategy](testing/strategy.md) - Testing approach and testability architecture
- [Behavioral Assertions](testing/behavioral-assertions.md) - Testable system behaviors

## Key Design Principles

- **Reliability first**: Consistent operation over performance optimization
- **Developer control**: Automate the mechanics while preserving timing control
- **Audit-friendly**: Clear logs and traceability for compliance
- **Simple to start**: Sensible defaults with customization options

## Current Implementation Status

| Feature | Status | Documentation |
|---------|--------|---------------|
| Webhook Processing | ✅ Complete | [Webhook Security](security/webhook-security.md) |
| Version Calculation | ✅ Complete | [Versioning Strategy](design/versioning-strategy.md) |
| Configuration System | ✅ Complete | [Configuration](operations/configuration.md) |
| Release PR Management | 🚧 In Progress | [Release PR Management](design/release-pr-management.md) |
| Release Automation | 📋 Planned | [Release Automation](design/release-automation.md) |
| CLI Tools | ✅ Complete | [CLI Reference](../docs/cli-reference.md) |

## Quick Start

For getting started with Release Regent, see our [Getting Started Guide](../docs/getting-started.md).

For detailed setup instructions, see the [GitHub App Setup Guide](../docs/github-app-setup.md).

---

*This specification is a living document that evolves with the application. All changes should be reflected here to maintain a single source of truth for requirements, design, and implementation guidance.*
