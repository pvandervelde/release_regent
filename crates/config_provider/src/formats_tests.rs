//! Unit tests for configuration format detection and handling.

use super::*;
use std::path::PathBuf;

#[test]
fn test_format_extensions() {
    assert_eq!(ConfigFormat::Yaml.extension(), "yaml");
    assert_eq!(ConfigFormat::Toml.extension(), "toml");

    assert_eq!(ConfigFormat::Yaml.extensions(), &["yaml", "yml"]);
    assert_eq!(ConfigFormat::Toml.extensions(), &["toml"]);
}

#[test]
fn test_format_names() {
    assert_eq!(ConfigFormat::Yaml.name(), "YAML");
    assert_eq!(ConfigFormat::Toml.name(), "TOML");
}

#[test]
fn test_detect_from_path() {
    assert_eq!(
        FormatDetector::detect_from_path(&PathBuf::from("config.yaml")).unwrap(),
        ConfigFormat::Yaml
    );
    assert_eq!(
        FormatDetector::detect_from_path(&PathBuf::from("config.yml")).unwrap(),
        ConfigFormat::Yaml
    );
    assert_eq!(
        FormatDetector::detect_from_path(&PathBuf::from("config.toml")).unwrap(),
        ConfigFormat::Toml
    );

    // Unsupported extension
    assert!(FormatDetector::detect_from_path(&PathBuf::from("config.json")).is_err());

    // No extension
    assert!(FormatDetector::detect_from_path(&PathBuf::from("config")).is_err());
}

#[test]
fn test_supported_extensions() {
    let extensions = FormatDetector::supported_extensions();
    assert!(extensions.contains(&"yaml"));
    assert!(extensions.contains(&"yml"));
    assert!(extensions.contains(&"toml"));
}

#[test]
fn test_is_supported_extension() {
    assert!(FormatDetector::is_supported_extension("yaml"));
    assert!(FormatDetector::is_supported_extension("yml"));
    assert!(FormatDetector::is_supported_extension("toml"));
    assert!(FormatDetector::is_supported_extension("YAML")); // Case insensitive

    assert!(!FormatDetector::is_supported_extension("json"));
    assert!(!FormatDetector::is_supported_extension("xml"));
}
