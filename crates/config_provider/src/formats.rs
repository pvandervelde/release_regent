//! Configuration format utilities.
//!
//! All configuration files use TOML format. This module provides helpers for
//! parsing, serializing, and validating TOML configuration files.

use crate::errors::{ConfigProviderError, ConfigProviderResult};
use release_regent_core::config::ReleaseRegentConfig;
use std::path::Path;

/// Parse TOML configuration content into a [`ReleaseRegentConfig`].
///
/// # Errors
/// - `ConfigProviderError::ParseError` — content could not be parsed as valid TOML
#[allow(clippy::result_large_err)] // ConfigProviderError is intentionally large
pub fn parse_config(content: &str) -> ConfigProviderResult<ReleaseRegentConfig> {
    toml::from_str(content).map_err(|e| {
        ConfigProviderError::parse_error_with_source(
            std::path::PathBuf::new(),
            format!("Failed to parse TOML: {e}"),
            e,
        )
    })
}

/// Serialize a [`ReleaseRegentConfig`] to a TOML string.
///
/// # Errors
/// - `ConfigProviderError::ParseError` — configuration could not be serialized
#[allow(clippy::result_large_err)] // ConfigProviderError is intentionally large
pub fn serialize_config(config: &ReleaseRegentConfig) -> ConfigProviderResult<String> {
    toml::to_string_pretty(config).map_err(|e| {
        ConfigProviderError::parse_error_with_source(
            std::path::PathBuf::new(),
            format!("Failed to serialize to TOML: {e}"),
            e,
        )
    })
}

/// Return `true` when `path` has a `.toml` extension (case-insensitive).
#[must_use]
pub fn is_toml_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("toml"))
        .unwrap_or(false)
}

/// Validate that `path` ends with `.toml`, returning an error for any other
/// extension or when no extension is present.
///
/// # Errors
/// - `ConfigProviderError::UnsupportedFormat` — extension is present but not `toml`
/// - `ConfigProviderError::InvalidFormat` — path has no extension
#[allow(clippy::result_large_err)] // ConfigProviderError is intentionally large
pub fn validate_toml_path(path: &Path) -> ConfigProviderResult<()> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase);

    match extension.as_deref() {
        Some("toml") => Ok(()),
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

#[cfg(test)]
#[path = "formats_tests.rs"]
mod tests;
