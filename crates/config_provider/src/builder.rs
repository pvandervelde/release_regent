//! Configuration builder for complex configuration setups.

use crate::errors::{ConfigProviderError, ConfigProviderResult};
use crate::file_provider::FileConfigurationProvider;
use crate::formats::{ConfigFormat, FormatDetector};
use crate::validation::{ConfigValidator, ValidationRule};
use release_regent_core::{
    config::ReleaseRegentConfig,
    traits::{configuration_provider::LoadOptions, ConfigurationProvider},
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Builder for creating complex configuration setups
pub struct ConfigurationBuilder {
    /// Global configuration file path
    global_config_path: Option<PathBuf>,
    /// Repository configuration file path
    repository_config_path: Option<PathBuf>,
    /// Configuration search directories
    search_directories: Vec<PathBuf>,
    /// Custom configuration values to override
    overrides: HashMap<String, String>,
    /// Configuration validator
    validator: Option<ConfigValidator>,
    /// Whether to enable strict validation
    strict_validation: bool,
    /// Custom format for files without clear extensions
    default_format: Option<ConfigFormat>,
    /// Whether to create missing configuration files
    create_missing: bool,
}

impl ConfigurationBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self {
            global_config_path: None,
            repository_config_path: None,
            search_directories: Vec::new(),
            overrides: HashMap::new(),
            validator: None,
            strict_validation: false,
            default_format: None,
            create_missing: false,
        }
    }

    /// Set the global configuration file path
    pub fn with_global_config_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.global_config_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set the repository configuration file path
    pub fn with_repository_config_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.repository_config_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Add a directory to search for configuration files
    pub fn with_search_directory<P: AsRef<Path>>(mut self, directory: P) -> Self {
        self.search_directories
            .push(directory.as_ref().to_path_buf());
        self
    }

    /// Add multiple search directories
    pub fn with_search_directories<P: AsRef<Path>>(mut self, directories: Vec<P>) -> Self {
        for dir in directories {
            self.search_directories.push(dir.as_ref().to_path_buf());
        }
        self
    }

    /// Add a configuration override
    pub fn with_override<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.overrides.insert(key.into(), value.into());
        self
    }

    /// Add multiple configuration overrides
    pub fn with_overrides<K: Into<String>, V: Into<String>>(
        mut self,
        overrides: HashMap<K, V>,
    ) -> Self {
        for (key, value) in overrides {
            self.overrides.insert(key.into(), value.into());
        }
        self
    }

    /// Set a custom validator
    pub fn with_validator(mut self, validator: ConfigValidator) -> Self {
        self.validator = Some(validator);
        self
    }

    /// Add a custom validation rule
    pub fn with_validation_rule(mut self, rule: Box<dyn ValidationRule>) -> Self {
        if let Some(validator) = self.validator.take() {
            self.validator = Some(validator.with_rule(rule));
        } else {
            self.validator = Some(ConfigValidator::new().with_rule(rule));
        }
        self
    }

    /// Enable strict validation (warnings become errors)
    pub fn with_strict_validation(mut self) -> Self {
        self.strict_validation = true;
        self
    }

    /// Set the default format for files without clear extensions
    pub fn with_default_format(mut self, format: ConfigFormat) -> Self {
        self.default_format = Some(format);
        self
    }

    /// Enable creation of missing configuration files
    pub fn with_create_missing(mut self) -> Self {
        self.create_missing = true;
        self
    }

    /// Build the configuration provider
    pub async fn build(self) -> ConfigProviderResult<FileConfigurationProvider> {
        // Determine the base directory for the configuration provider
        let base_dir = self.determine_base_directory()?;

        // Create the file configuration provider
        let mut provider = FileConfigurationProvider::new(&base_dir).await?;

        // Configure search directories
        for dir in self.search_directories {
            provider.add_search_directory(dir);
        }

        // Set default format if specified
        if let Some(format) = self.default_format {
            provider.set_default_format(format);
        }

        // Configure validator
        let validator = if self.strict_validation {
            self.validator.unwrap_or_else(ConfigValidator::strict)
        } else {
            self.validator.unwrap_or_default()
        };
        provider.set_validator(validator);

        // Apply configuration overrides if any
        if !self.overrides.is_empty() {
            provider.set_overrides(self.overrides);
        }

        // Set specific configuration file paths if provided
        if let Some(global_path) = self.global_config_path {
            provider.set_global_config_path(global_path);
        }

        if let Some(repo_path) = self.repository_config_path {
            provider.set_repository_config_path(repo_path);
        }

        // Enable creation of missing files if requested
        if self.create_missing {
            provider.enable_create_missing();
        }

        Ok(provider)
    }

    /// Build and load a merged configuration
    pub async fn build_and_load(
        self,
        owner: Option<&str>,
        repo: Option<&str>,
    ) -> ConfigProviderResult<ReleaseRegentConfig> {
        let provider = self.build().await?;
        provider
            .get_merged_config(
                owner.ok_or_else(|| ConfigProviderError::Builder { message: "Owner is required".to_string() })?,
                repo.ok_or_else(|| ConfigProviderError::Builder { message: "Repository is required".to_string() })?,
                LoadOptions::default()
            )
            .await
            .map_err(ConfigProviderError::Core)
    }

    /// Build and load just the global configuration
    pub async fn build_and_load_global(self) -> ConfigProviderResult<ReleaseRegentConfig> {
        let provider = self.build().await?;
        provider.load_global_config(LoadOptions::default())
            .await
            .map_err(ConfigProviderError::Core)
    }

    /// Determine the base directory for the configuration provider
    fn determine_base_directory(&self) -> ConfigProviderResult<PathBuf> {
        // If global config path is specified, use its parent directory
        if let Some(global_path) = &self.global_config_path {
            if let Some(parent) = global_path.parent() {
                return Ok(parent.to_path_buf());
            }
        }

        // If repository config path is specified, use its parent directory
        if let Some(repo_path) = &self.repository_config_path {
            if let Some(parent) = repo_path.parent() {
                return Ok(parent.to_path_buf());
            }
        }

        // If search directories are specified, use the first one
        if let Some(first_dir) = self.search_directories.first() {
            return Ok(first_dir.clone());
        }

        // Default to current directory
        std::env::current_dir()
            .map_err(|e| ConfigProviderError::io_error("Failed to get current directory", e))
    }
}

impl Default for ConfigurationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenient builder methods for common configurations
impl ConfigurationBuilder {
    /// Create a builder for development/testing with common defaults
    pub fn for_development() -> Self {
        Self::new()
            .with_search_directory("./config")
            .with_search_directory(".")
            .with_default_format(ConfigFormat::Yaml)
            .with_create_missing()
    }

    /// Create a builder for production with strict validation
    pub fn for_production() -> Self {
        Self::new()
            .with_strict_validation()
            .with_default_format(ConfigFormat::Yaml)
    }

    /// Create a builder for testing with minimal configuration
    pub fn for_testing() -> Self {
        Self::new()
            .with_default_format(ConfigFormat::Yaml)
            .with_create_missing()
    }

    /// Create a builder with automatic directory detection
    pub fn auto_detect() -> ConfigProviderResult<Self> {
        let mut builder = Self::new();

        // Common configuration directories to check
        let release_regent_config_path = format!("{}/.config/release-regent", std::env::var("HOME").unwrap_or_default());
        let common_dirs = vec![
            "./config",
            "./.config",
            "./configs",
            ".",
            release_regent_config_path.as_str(),
            "/etc/release-regent",
        ];

        for dir_str in common_dirs {
            let dir = PathBuf::from(dir_str);
            if dir.exists() && dir.is_dir() {
                builder = builder.with_search_directory(dir);
            }
        }

        // Try to find existing configuration files
        if let Ok(current_dir) = std::env::current_dir() {
            for entry in
                std::fs::read_dir(&current_dir).unwrap_or_else(|_| std::fs::read_dir(".").unwrap())
            {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                        // Look for common configuration file names
                        if (file_name.starts_with("release-regent")
                            || file_name.starts_with("release_regent")
                            || file_name == "config.yaml"
                            || file_name == "config.yml" || file_name == "config.toml") && FormatDetector::detect_from_path(&path).is_ok() {
                            builder = builder.with_global_config_path(path);
                            break;
                        }
                    }
                }
            }
        }

        Ok(builder.with_default_format(ConfigFormat::Yaml))
    }
}

#[cfg(test)]
#[path = "builder_tests.rs"]
mod tests;
