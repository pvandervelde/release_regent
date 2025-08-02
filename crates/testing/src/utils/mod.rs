//! Testing utilities and helper functions
//!
//! This module provides general utilities for testing scenarios.

use std::path::PathBuf;
use tempfile::{NamedTempFile, TempDir};

/// Temporary testing environment
#[derive(Debug)]
pub struct TestEnvironment {
    /// Temporary directory for test files
    temp_dir: TempDir,
    /// Test-specific configuration
    config: TestConfig,
}

/// Configuration for test environment
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Whether to cleanup resources automatically
    pub auto_cleanup: bool,
    /// Whether to enable debug logging
    pub debug_logging: bool,
    /// Default timeout for async operations
    pub default_timeout_ms: u64,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            auto_cleanup: true,
            debug_logging: false,
            default_timeout_ms: 5000,
        }
    }
}

impl TestEnvironment {
    /// Create a new test environment
    ///
    /// # Returns
    /// Initialized test environment
    ///
    /// # Errors
    /// Returns error if temporary directory creation fails
    pub fn new() -> Result<Self, std::io::Error> {
        let temp_dir = TempDir::new()?;
        Ok(Self {
            temp_dir,
            config: TestConfig::default(),
        })
    }

    /// Create a test environment with custom configuration
    ///
    /// # Parameters
    /// - `config`: Test environment configuration
    ///
    /// # Returns
    /// Initialized test environment
    ///
    /// # Errors
    /// Returns error if temporary directory creation fails
    pub fn with_config(config: TestConfig) -> Result<Self, std::io::Error> {
        let temp_dir = TempDir::new()?;
        Ok(Self { temp_dir, config })
    }

    /// Get the temporary directory path
    ///
    /// # Returns
    /// Path to temporary directory
    pub fn temp_dir(&self) -> PathBuf {
        self.temp_dir.path().to_path_buf()
    }

    /// Create a temporary file in the test environment
    ///
    /// # Returns
    /// Named temporary file
    ///
    /// # Errors
    /// Returns error if file creation fails
    pub fn create_temp_file(&self) -> Result<NamedTempFile, std::io::Error> {
        NamedTempFile::new_in(self.temp_dir.path())
    }

    /// Create a subdirectory in the test environment
    ///
    /// # Parameters
    /// - `name`: Directory name
    ///
    /// # Returns
    /// Path to created directory
    ///
    /// # Errors
    /// Returns error if directory creation fails
    pub fn create_subdir(&self, name: &str) -> Result<PathBuf, std::io::Error> {
        let dir_path = self.temp_dir.path().join(name);
        std::fs::create_dir_all(&dir_path)?;
        Ok(dir_path)
    }

    /// Get test configuration
    ///
    /// # Returns
    /// Reference to test configuration
    pub fn config(&self) -> &TestConfig {
        &self.config
    }
}

/// Initialize test logging if debug is enabled
///
/// # Parameters
/// - `config`: Test configuration
pub fn init_test_logging(config: &TestConfig) {
    if config.debug_logging {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("debug")
            .try_init();
    }
}

/// Create a test timeout duration
///
/// # Parameters
/// - `config`: Test configuration
///
/// # Returns
/// Timeout duration
pub fn test_timeout(config: &TestConfig) -> tokio::time::Duration {
    tokio::time::Duration::from_millis(config.default_timeout_ms)
}
