//! Testing infrastructure for Release Regent
//!
//! This crate provides comprehensive testing utilities, mock implementations,
//! and test fixtures for testing Release Regent without external dependencies.
//!
//! # Architecture
//!
//! The testing infrastructure is organized into several modules:
//! - `mocks`: Mock implementations of all core traits
//! - `builders`: Test data builders using the builder pattern
//! - `fixtures`: Pre-built test data and webhook payloads
//! - `assertions`: Spec testing framework for behavioral assertions
//! - `utils`: General testing utilities and helpers
//!
//! # Design Principles
//!
//! All mock implementations follow these principles:
//! - **Deterministic**: Same inputs always produce same outputs
//! - **Comprehensive**: Support all production use cases
//! - **Configurable**: Allow test-specific behavior configuration
//! - **Error Simulation**: Support testing error scenarios
//! - **Performance**: Enable performance and load testing
//!
//! # Usage Patterns
//!
//! ## Mock Trait Implementations
//!
//! Use mock implementations for unit testing:
//!
//! ```rust
//! use release_regent_testing::mocks::MockGitHubOperations;
//! use release_regent_core::traits::GitHubOperations;
//!
//! let mock_github = MockGitHubOperations::new()
//!     .with_repository_exists(true)
//!     .with_default_branch("main");
//! ```
//!
//! ## Test Data Builders
//!
//! Create realistic test data using builders:
//!
//! ```rust
//! use release_regent_testing::builders::CommitBuilder;
//!
//! let commit = CommitBuilder::new()
//!     .with_conventional_message("feat: add authentication")
//!     .with_author("developer@example.com")
//!     .build();
//! ```
//!
//! ## Webhook Fixtures
//!
//! Use pre-built webhook payloads for integration testing:
//!
//! ```rust
//! use release_regent_testing::fixtures::WebhookFixtures;
//!
//! let push_event = WebhookFixtures::github_push_event()
//!     .with_branch("main")
//!     .with_commits(3)
//!     .build();
//! ```
//!
//! ## Spec Testing
//!
//! Verify behavioral compliance using spec testing:
//!
//! ```rust
//! use release_regent_testing::assertions::SpecAssertion;
//!
//! SpecAssertion::new()
//!     .verify_version_calculation(&calculator, &spec)
//!     .assert_compliance();
//! ```
//!
//! # Error Handling
//!
//! All mock implementations properly handle and simulate errors:
//! - Network timeouts and connection failures
//! - Authentication and authorization errors
//! - Rate limiting and quota exceeded scenarios
//! - Invalid input validation errors
//! - Service unavailable conditions
//!
//! # Performance Testing
//!
//! Mock implementations support performance testing:
//! - Configurable response latency simulation
//! - Memory usage tracking and limits
//! - Concurrent request handling verification
//! - Resource cleanup validation
//!
//! # Thread Safety
//!
//! All mock implementations are thread-safe and support:
//! - Concurrent test execution
//! - Shared state management
//! - Atomic operation counting
//! - Safe cleanup in multi-threaded scenarios

pub mod assertions;
pub mod builders;
pub mod fixtures;
pub mod mocks;
pub mod utils;

// Re-export commonly used items for convenience
pub use assertions::*;
pub use builders::*;
pub use fixtures::*;
pub use mocks::*;
pub use utils::*;
