//! Unit tests for TOML configuration format helpers.

use super::*;
use std::path::PathBuf;

#[test]
fn test_parse_config_valid_toml() {
    let toml = "[core]\nversion_prefix = \"v\"";
    assert!(parse_config(toml).is_ok());
}

#[test]
fn test_parse_config_invalid_toml() {
    let bad = "not = valid = toml";
    assert!(parse_config(bad).is_err());
}

#[test]
fn test_serialize_config_roundtrip() {
    let toml = "[core]\nversion_prefix = \"v\"";
    let config = parse_config(toml).unwrap();
    let serialized = serialize_config(&config).unwrap();
    let roundtripped = parse_config(&serialized).unwrap();
    assert_eq!(config.core.version_prefix, roundtripped.core.version_prefix);
}

#[test]
fn test_is_toml_path_returns_true_for_toml() {
    assert!(is_toml_path(&PathBuf::from("config.toml")));
    assert!(is_toml_path(&PathBuf::from("config.TOML"))); // Case insensitive
}

#[test]
fn test_is_toml_path_returns_false_for_other_extensions() {
    assert!(!is_toml_path(&PathBuf::from("config.yaml")));
    assert!(!is_toml_path(&PathBuf::from("config.yml")));
    assert!(!is_toml_path(&PathBuf::from("config.json")));
    assert!(!is_toml_path(&PathBuf::from("config")));
}

#[test]
fn test_validate_toml_path_valid() {
    assert!(validate_toml_path(&PathBuf::from("config.toml")).is_ok());
}

#[test]
fn test_validate_toml_path_wrong_extension() {
    assert!(validate_toml_path(&PathBuf::from("config.json")).is_err());
    assert!(validate_toml_path(&PathBuf::from("config.yaml")).is_err());
    assert!(validate_toml_path(&PathBuf::from("config.yml")).is_err());
}

#[test]
fn test_validate_toml_path_no_extension() {
    assert!(validate_toml_path(&PathBuf::from("config")).is_err());
}
