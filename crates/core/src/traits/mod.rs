//! Core trait abstractions for Release Regent
//!
//! This module defines the fundamental contracts for external services and operations
//! that Release Regent depends on. These traits enable dependency injection and
//! comprehensive testing by providing abstraction layers over external systems.
//!
//! The traits are designed to:
//! - Support async operations with proper error handling
//! - Enable mock implementations for testing
//! - Provide clear contracts for business logic
//! - Support behavioral assertion testing
//!
//! # Architecture
//!
//! The trait abstractions follow a layered approach:
//! - `GitHubOperations`: GitHub API interactions
//! - `ConfigurationProvider`: Configuration loading and validation
//! - `VersionCalculator`: Version calculation strategies
//!
//! # Usage
//!
//! These traits are intended to be implemented by:
//! - Production services (in separate crates)
//! - Mock implementations (in testing crate)
//! - Test fixtures (for deterministic testing)

pub mod configuration_provider;
pub mod github_operations;
pub mod version_calculator;

pub use configuration_provider::ConfigurationProvider;
pub use github_operations::GitHubOperations;
pub use version_calculator::VersionCalculator;

#[cfg(test)]
mod tests;
