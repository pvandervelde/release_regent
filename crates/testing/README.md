# Release Regent Testing Infrastructure

Comprehensive testing infrastructure and mock implementations for Release Regent.

## Overview

This crate provides all the testing utilities, mock implementations, and test fixtures needed to test Release Regent without requiring external dependencies like GitHub API access or real file systems.

## Features

- **Mock Implementations**: Complete mock implementations of all core traits
- **Test Data Builders**: Builder pattern for creating test data with realistic values
- **Fixture Management**: Deterministic test fixtures for webhooks and API responses
- **Spec Testing Framework**: Support for behavioral assertion testing
- **HTTP Mocking**: Mock GitHub API server for integration testing

## Usage

Add this crate as a dev-dependency in your test code:

```toml
[dev-dependencies]
release_regent_testing = { path = "../testing" }
```

## Mock Implementations

All core traits have mock implementations that support:

- Deterministic behavior for reproducible tests
- Configurable responses for different test scenarios
- Comprehensive error simulation
- Performance testing support

## Test Data Builders

Use builders to create test data:

```rust
use release_regent_testing::builders::*;

let commit = CommitBuilder::new()
    .with_conventional_message("feat: add new feature")
    .with_author("test@example.com")
    .build();
```

## Fixtures

Pre-built webhook payloads and API responses:

```rust
use release_regent_testing::fixtures::*;

let push_event = webhook_fixtures::github_push_event();
```

## Spec Testing

Support for behavioral assertions:

```rust
use release_regent_testing::assertions::*;

assert_spec_compliance!(calculator, version_calculation_spec);
```
