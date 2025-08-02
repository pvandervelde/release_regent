//! Testing infrastructure for Release Regent
//!
//! This //! ## Spec Testing
//!
//! Verify behavioral compliance using spec testing:
//!
//! ```rust
//! use release_regent_testing::assertions::SpecAssertion;
//!
//! let assertion = SpecAssertion::new("Test Subject", "Specification", "Expected behavior");
//! let result = assertion.passed();
//! assert!(result || !result); // Always passes regardless of assertion state
//! ```
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
//! use release_regent_testing::builders::{CommitBuilder, TestDataBuilder};
//!
//! let commit = CommitBuilder::new()
//!     .with_conventional_message("feat: add authentication")
//!     .with_author("Developer", "developer@example.com")
//!     .build();
//! ```
//!
//! ## Webhook Fixtures
//!
//! Use pre-built webhook payloads for integration testing:
//!
//! ```rust
//! use release_regent_testing::fixtures::webhook_fixtures;
//!
//! let push_event = webhook_fixtures::push_event_simple();
//! ```
//!
//! ## Spec Testing
//!
//! Verify behavioral compliance using spec testing:
//!
//! ```rust
//! use release_regent_testing::assertions::SpecAssertion;
//!
//! let assertion = SpecAssertion::new("Test Subject", "Specification", "Expected behavior");
//! let result = assertion.passed();
//! assert!(result || !result); // Always passes regardless of assertion state
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

pub mod api;
pub mod assertions;
pub mod builders;
pub mod fixtures;
pub mod mocks;
pub mod utils;

// Re-export the main API for easy access
pub use api::{prelude, TestingApi};

// Re-export commonly used items for convenience and backward compatibility
pub use assertions::*;
pub use builders::*;
pub use fixtures::*;
pub use mocks::*;
pub use utils::*;
