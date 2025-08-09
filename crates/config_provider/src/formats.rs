//! Configuration format detection and handling utilities.

use crate::errors::{ConfigProviderError, ConfigProviderResult};
use release_regent_core::config::ReleaseRegentConfig;
use std::path::Path;

/// Supported configuration formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    /// YAML format (.yaml, .yml)
    Yaml,
    /// TOML format (.toml)
    Toml,
}

impl ConfigFormat {
    /// Get the primary file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            ConfigFormat::Yaml => "yaml",
            ConfigFormat::Toml => "toml",
        }
    }

    /// Get all supported file extensions for this format
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            ConfigFormat::Yaml => &["yaml", "yml"],
            ConfigFormat::Toml => &["toml"],
        }
    }

    /// Get the format name as a string
    pub fn name(&self) -> &'static str {
        match self {
            ConfigFormat::Yaml => "YAML",
            ConfigFormat::Toml => "TOML",
        }
    }

    /// Parse configuration content in this format
    pub fn parse(&self, content: &str) -> ConfigProviderResult<ReleaseRegentConfig> {
        match self {
            ConfigFormat::Yaml => serde_yaml::from_str(content).map_err(|e| {
                ConfigProviderError::parse_error_with_source(
                    std::path::PathBuf::new(),
                    format!("Failed to parse YAML: {}", e),
                    e,
                )
            }),
            ConfigFormat::Toml => toml::from_str(content).map_err(|e| {
                ConfigProviderError::parse_error_with_source(
                    std::path::PathBuf::new(),
                    format!("Failed to parse TOML: {}", e),
                    e,
                )
            }),
        }
    }

    /// Serialize configuration to this format
    pub fn serialize(&self, config: &ReleaseRegentConfig) -> ConfigProviderResult<String> {
        match self {
            ConfigFormat::Yaml => serde_yaml::to_string(config).map_err(|e| {
                ConfigProviderError::parse_error_with_source(
                    std::path::PathBuf::new(),
                    format!("Failed to serialize to YAML: {}", e),
                    e,
                )
            }),
            ConfigFormat::Toml => toml::to_string_pretty(config).map_err(|e| {
                ConfigProviderError::parse_error_with_source(
                    std::path::PathBuf::new(),
                    format!("Failed to serialize to TOML: {}", e),
                    e,
                )
            }),
        }
    }
}

/// Utility for detecting configuration file formats
pub struct FormatDetector;

impl FormatDetector {
    /// Detect format from file extension
    pub fn detect_from_path(path: &Path) -> ConfigProviderResult<ConfigFormat> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_lowercase());

        match extension.as_deref() {
            Some("yaml") | Some("yml") => Ok(ConfigFormat::Yaml),
            Some("toml") => Ok(ConfigFormat::Toml),
            Some(ext) => Err(ConfigProviderError::UnsupportedFormat {
                format: ext.to_string(),
                path: path.to_path_buf(),
            }),
            None => Err(ConfigProviderError::InvalidFormat {
                path: path.to_path_buf(),
                reason: "No file extension found".to_string(),
            }),
        }
    }

    /// Detect format from file content (fallback when extension detection fails)
    pub fn detect_from_content(content: &str) -> ConfigProviderResult<ConfigFormat> {
        // Try parsing as YAML first (more common)
        if serde_yaml::from_str::<serde_yaml::Value>(content).is_ok() {
            return Ok(ConfigFormat::Yaml);
        }

        // Try parsing as TOML
        if toml::from_str::<toml::Value>(content).is_ok() {
            return Ok(ConfigFormat::Toml);
        }

        Err(ConfigProviderError::InvalidFormat {
            path: std::path::PathBuf::new(),
            reason: "Could not detect format from content".to_string(),
        })
    }

    /// Get all supported formats
    pub fn supported_formats() -> Vec<ConfigFormat> {
        vec![ConfigFormat::Yaml, ConfigFormat::Toml]
    }

    /// Get all supported file extensions
    pub fn supported_extensions() -> Vec<&'static str> {
        Self::supported_formats()
            .iter()
            .flat_map(|f| f.extensions().iter().copied())
            .collect()
    }

    /// Check if a file extension is supported
    pub fn is_supported_extension(extension: &str) -> bool {
        let extension = extension.to_lowercase();
        Self::supported_extensions()
            .iter()
            .any(|&ext| ext == extension)
    }
}

#[cfg(test)]
#[path = "formats_tests.rs"]
mod tests;
