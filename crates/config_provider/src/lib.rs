//! # Configuration Provider Crate
//!
//! This crate provides file-based configuration loading and management for Release Regent.
//! It implements the `ConfigurationProvider` trait with support for YAML and TOML formats,
//! comprehensive validation, and a builder pattern for complex setups.
//!
//! ## Features
//!
//! - File-based configuration loading (YAML and TOML)
//! - Configuration validation with JSON Schema
//! - Builder pattern for complex configuration setups
//! - Comprehensive error handling
//! - Async/await support
//! - Hierarchical configuration merging (global + repository-specific)
//!
//! ## Usage
//!
//! ```rust
//! use release_regent_config_provider::{FileConfigurationProvider, ConfigurationBuilder};
//! use release_regent_core::traits::ConfigurationProvider;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a file-based configuration provider
//! let provider = FileConfigurationProvider::new("/path/to/config/dir").await?;
//!
//! // Load global configuration
//! let global_config = provider.load_global_config(Default::default()).await?;
//!
//! // Load repository-specific configuration
//! let repo_config = provider.load_repository_config("owner", "repo", Default::default()).await?;
//!
//! // Use builder for complex setups
//! let config = ConfigurationBuilder::new()
//!     .with_global_config_path("/path/to/global.yaml")
//!     .with_repository_config_path("/path/to/repo.toml")
//!     .build()
//!     .await?;
//! # Ok(())
//! # }
//! ```

pub mod builder;
pub mod errors;
pub mod file_provider;
pub mod formats;
pub mod validation;

pub use builder::ConfigurationBuilder;
pub use errors::{ConfigProviderError, ConfigProviderResult};
pub use file_provider::FileConfigurationProvider;
pub use formats::{ConfigFormat, FormatDetector};
pub use validation::{ConfigValidator, ValidationResult as ConfigValidationResult};

// Re-export core types for convenience
pub use release_regent_core::{
    config::ReleaseRegentConfig,
    traits::{configuration_provider::LoadOptions, ConfigurationProvider},
};
