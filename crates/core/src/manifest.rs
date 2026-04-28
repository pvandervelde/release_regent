//! Version manifest file detection and update logic.
//!
//! This module provides:
//!
//! - [`ManifestFormat`] — the format of a version manifest file.
//! - [`ManifestFileConfig`] — describes a single manifest file to update.
//! - [`update_manifest_content`] — pure function that applies a version string
//!   to a manifest file's content and returns the updated text.
//! - [`detect_standard_manifests`] — inspects a set of file paths that exist
//!   in the repository and returns configurations for the well-known manifest
//!   files that are present.
//!
//! ## Supported languages / formats
//!
//! Auto-detection recognises the following files out of the box:
//!
//! | Language       | File              | Format      | Key                     |
//! |----------------|-------------------|-------------|-------------------------|
//! | Rust           | `Cargo.toml`      | TOML        | `package.version`       |
//! | Node.js / npm  | `package.json`    | JSON        | `version`               |
//! | Python PEP 517 | `pyproject.toml`  | TOML        | `project.version`       |
//! | Python / Poetry| `pyproject.toml`  | TOML        | `tool.poetry.version`   |
//! | PHP / Composer | `composer.json`   | JSON        | `version`               |
//!
//! The [`ManifestFormat::PlainText`] variant serves as an escape hatch for any
//! other language (e.g. .NET `.csproj`, Ruby gemspec) via a user-supplied regex
//! pattern with one capture group.
//!
//! ## Usage
//!
//! ```rust
//! use release_regent_core::manifest::{ManifestFormat, update_manifest_content};
//!
//! let toml = "[package]\nversion = \"0.0.0\"\n";
//! let updated = update_manifest_content(toml, &ManifestFormat::Toml, "package.version", "1.2.3")
//!     .expect("should update");
//! assert!(updated.contains("version = \"1.2.3\""));
//! ```

use crate::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// The serialisation format of a version manifest file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestFormat {
    /// TOML file (e.g. `Cargo.toml`, `pyproject.toml`).
    ///
    /// The `version_key` field of [`ManifestFileConfig`] is a dot-separated
    /// table path such as `"package.version"` or `"tool.poetry.version"`.
    Toml,

    /// JSON file (e.g. `package.json`, `composer.json`).
    ///
    /// The `version_key` field is a top-level JSON object key such as
    /// `"version"`.  Nested paths (e.g. `"info.version"`) are not supported
    /// — use [`ManifestFormat::PlainText`] with a regex for those cases.
    Json,

    /// Arbitrary plain-text file using a regex replacement.
    ///
    /// The `version_key` field is a regex pattern with **exactly one** capture
    /// group (`(...)`) that matches the current version string.  The matched
    /// span (the full match, not just the capture group) is replaced with the
    /// version string substituted in place of the capture group.
    ///
    /// Example pattern: `r#"^version = "(.+)"$"#` applied to
    /// `version = "0.0.0"` produces `version = "1.2.3"`.
    PlainText,
}

/// Configuration for a single version manifest file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFileConfig {
    /// Repo-relative path to the file (e.g. `"Cargo.toml"`, `"package.json"`).
    pub path: String,

    /// The serialisation format to use when reading and updating the file.
    pub format: ManifestFormat,

    /// Format-specific address of the version field.
    ///
    /// - **TOML**: dot-separated table path (e.g. `"package.version"`).
    /// - **JSON**: top-level key (e.g. `"version"`).
    /// - **PlainText**: regex pattern with one capture group (e.g.
    ///   `r#"^version = "(.+)"$"#`).
    pub version_key: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Public functions
// ─────────────────────────────────────────────────────────────────────────────

/// Apply `version` to the version field in `content` and return the updated text.
///
/// This is a **pure function** — it does not touch the filesystem or the network.
///
/// # Errors
///
/// Returns [`CoreError::InvalidInput`] when:
/// - `format` is [`ManifestFormat::Toml`] and `content` cannot be parsed as TOML,
///   or the dot-separated `key` path does not lead to a string value.
/// - `format` is [`ManifestFormat::Json`] and `content` cannot be parsed as JSON,
///   or the `key` is not a top-level string value.
/// - `format` is [`ManifestFormat::PlainText`] and `key` is not a valid regex,
///   or the regex does not contain exactly one capture group,
///   or no match is found in `content`.
///
/// # Examples
///
/// ```rust
/// use release_regent_core::manifest::{ManifestFormat, update_manifest_content};
///
/// // TOML
/// let toml = "[package]\nversion = \"0.0.0\"\n";
/// let out = update_manifest_content(toml, &ManifestFormat::Toml, "package.version", "1.2.3").unwrap();
/// assert!(out.contains("version = \"1.2.3\""));
///
/// // JSON
/// let json = r#"{"name":"myapp","version":"0.0.0"}"#;
/// let out = update_manifest_content(json, &ManifestFormat::Json, "version", "1.2.3").unwrap();
/// assert!(out.contains(r#""version": "1.2.3""#));
///
/// // PlainText
/// let text = "version = \"0.0.0\"\n";
/// let out = update_manifest_content(text, &ManifestFormat::PlainText, r#"version = "([^"]+)""#, "1.2.3").unwrap();
/// assert!(out.contains("version = \"1.2.3\""));
/// ```
// CoreError is intentionally large; this is the established pattern throughout the codebase.
#[allow(clippy::result_large_err)]
pub fn update_manifest_content(
    content: &str,
    format: &ManifestFormat,
    key: &str,
    version: &str,
) -> CoreResult<String> {
    match format {
        ManifestFormat::Toml => update_toml(content, key, version),
        ManifestFormat::Json => update_json(content, key, version),
        ManifestFormat::PlainText => update_plain_text(content, key, version),
    }
}

/// Return [`ManifestFileConfig`] entries for every well-known manifest file
/// whose path appears in `existing_paths`.
///
/// This function is deterministic and pure — it does not read from disk.
/// The caller is responsible for providing the set of paths that actually
/// exist in the repository (e.g. by calling
/// [`GitHubOperations::get_file_content`] for each candidate and collecting
/// the `Some(_)` results).
///
/// When `pyproject.toml` is present, **two** entries are returned — one for
/// the PEP 517 key (`project.version`) and one for the Poetry key
/// (`tool.poetry.version`).  The orchestrator will attempt both; the one
/// whose key is absent in the file will produce a
/// [`CoreError::InvalidInput`] that is caught and skipped with a `warn!`.
///
/// Explicit entries in [`OrchestratorConfig::manifest_files`] take
/// precedence: if the caller has already provided a config entry for a given
/// path, it should remove that path from `existing_paths` before calling this
/// function (or filter the results).
///
/// [`GitHubOperations::get_file_content`]: crate::traits::github_operations::GitHubOperations::get_file_content
/// [`OrchestratorConfig::manifest_files`]: crate::release_orchestrator::OrchestratorConfig
pub fn detect_standard_manifests(existing_paths: &[&str]) -> Vec<ManifestFileConfig> {
    let path_set: std::collections::HashSet<&str> = existing_paths.iter().copied().collect();
    let mut result = Vec::new();

    // Rust
    if path_set.contains("Cargo.toml") {
        result.push(ManifestFileConfig {
            path: "Cargo.toml".to_string(),
            format: ManifestFormat::Toml,
            version_key: "package.version".to_string(),
        });
    }

    // Node.js / npm
    if path_set.contains("package.json") {
        result.push(ManifestFileConfig {
            path: "package.json".to_string(),
            format: ManifestFormat::Json,
            version_key: "version".to_string(),
        });
    }

    // Python — pyproject.toml: try PEP 517 key first, then Poetry key.
    if path_set.contains("pyproject.toml") {
        result.push(ManifestFileConfig {
            path: "pyproject.toml".to_string(),
            format: ManifestFormat::Toml,
            version_key: "project.version".to_string(),
        });
        result.push(ManifestFileConfig {
            path: "pyproject.toml".to_string(),
            format: ManifestFormat::Toml,
            version_key: "tool.poetry.version".to_string(),
        });
    }

    // PHP / Composer
    if path_set.contains("composer.json") {
        result.push(ManifestFileConfig {
            path: "composer.json".to_string(),
            format: ManifestFormat::Json,
            version_key: "version".to_string(),
        });
    }

    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Update the version in a TOML document at the given dot-separated key path.
///
/// The key path may have any number of dot-separated segments, e.g.:
/// - `"version"` — top-level key
/// - `"package.version"` — one table level
/// - `"tool.poetry.version"` — two table levels (Poetry)
///
/// Each intermediate segment must be a TOML table.  The final segment must
/// be an existing string value.
#[allow(clippy::result_large_err)]
fn update_toml(content: &str, key: &str, version: &str) -> CoreResult<String> {
    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .map_err(|e| CoreError::invalid_input("manifest", format!("Failed to parse TOML: {e}")))?;

    let segments: Vec<&str> = key.split('.').collect();
    let (table_segments, field_key) = match segments.split_last() {
        Some((last, rest)) => (rest, *last),
        None => {
            return Err(CoreError::invalid_input(
                "manifest",
                format!("TOML key path '{key}' is empty"),
            ));
        }
    };

    // Navigate all intermediate table segments.
    let mut current: &mut toml_edit::Item = doc.as_item_mut();
    for segment in table_segments {
        current = current
            .as_table_mut()
            .ok_or_else(|| {
                CoreError::invalid_input(
                    "manifest",
                    format!("TOML path '{key}': '{segment}' is not a table"),
                )
            })?
            .get_mut(segment)
            .ok_or_else(|| {
                CoreError::invalid_input(
                    "manifest",
                    format!("TOML path '{key}': table '{segment}' not found"),
                )
            })?;
    }

    // Now `current` points at the innermost table; find the field.
    let table = current.as_table_mut().ok_or_else(|| {
        CoreError::invalid_input(
            "manifest",
            format!("TOML path '{key}': intermediate segment is not a table"),
        )
    })?;

    let item = table.get_mut(field_key).ok_or_else(|| {
        CoreError::invalid_input(
            "manifest",
            format!("TOML key '{key}': field '{field_key}' not found"),
        )
    })?;

    if !item.is_str() {
        return Err(CoreError::invalid_input(
            "manifest",
            format!("TOML key '{key}' is not a string value"),
        ));
    }

    *item = toml_edit::value(version);

    Ok(doc.to_string())
}

/// Update a top-level string key in a JSON object.
#[allow(clippy::result_large_err)]
fn update_json(content: &str, key: &str, version: &str) -> CoreResult<String> {
    let mut value: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| CoreError::invalid_input("manifest", format!("Failed to parse JSON: {e}")))?;

    let obj = value.as_object_mut().ok_or_else(|| {
        CoreError::invalid_input("manifest", "JSON content is not an object".to_string())
    })?;

    match obj.get(key) {
        None => {
            return Err(CoreError::invalid_input(
                "manifest",
                format!("JSON key '{key}' not found in object"),
            ));
        }
        Some(existing) if !existing.is_string() => {
            return Err(CoreError::invalid_input(
                "manifest",
                format!("JSON key '{key}' is not a string value"),
            ));
        }
        _ => {}
    }

    obj.insert(
        key.to_string(),
        serde_json::Value::String(version.to_string()),
    );

    serde_json::to_string_pretty(&value)
        .map_err(|e| CoreError::invalid_input("manifest", format!("Failed to serialise JSON: {e}")))
}

/// Update a version string matched by a regex pattern with one capture group.
#[allow(clippy::result_large_err)]
fn update_plain_text(content: &str, pattern: &str, version: &str) -> CoreResult<String> {
    let re = regex::Regex::new(pattern).map_err(|e| {
        CoreError::invalid_input(
            "manifest",
            format!("PlainText pattern '{pattern}' is not a valid regex: {e}"),
        )
    })?;

    if re.captures_len() != 2 {
        // captures_len() == 1 means the regex has no capture groups;
        // == 2 means exactly one explicit group plus the implicit whole-match group.
        return Err(CoreError::invalid_input(
            "manifest",
            format!("PlainText pattern '{pattern}' must contain exactly one capture group"),
        ));
    }

    let caps = re.captures(content).ok_or_else(|| {
        CoreError::invalid_input(
            "manifest",
            format!("PlainText pattern '{pattern}' did not match any text in the file"),
        )
    })?;

    let full_match = caps.get(0).expect("captures(0) always exists").as_str();
    let capture = caps
        .get(1)
        .expect("captures(1) exists; checked above")
        .as_str();

    // Replace the captured version substring within the full match.
    let new_match = full_match.replacen(capture, version, 1);
    Ok(content.replacen(full_match, &new_match, 1))
}
