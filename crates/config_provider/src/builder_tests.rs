//! Unit tests for configuration builder.

use super::*;
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn test_configuration_builder_creation() {
    let builder = ConfigurationBuilder::new();
    assert!(builder.global_config_path.is_none());
    assert!(builder.repository_config_path.is_none());
    assert!(builder.search_directories.is_empty());
    assert!(builder.overrides.is_empty());
}

#[test]
fn test_configuration_builder_with_paths() {
    let builder = ConfigurationBuilder::new()
        .with_global_config_path("/path/to/global.yaml")
        .with_repository_config_path("/path/to/repo.toml");

    assert_eq!(
        builder.global_config_path.unwrap(),
        PathBuf::from("/path/to/global.yaml")
    );
    assert_eq!(
        builder.repository_config_path.unwrap(),
        PathBuf::from("/path/to/repo.toml")
    );
}

#[test]
fn test_configuration_builder_with_search_directories() {
    let builder = ConfigurationBuilder::new()
        .with_search_directory("/path/to/dir1")
        .with_search_directories(vec!["/path/to/dir2", "/path/to/dir3"]);

    assert_eq!(builder.search_directories.len(), 3);
    assert_eq!(
        builder.search_directories[0],
        PathBuf::from("/path/to/dir1")
    );
    assert_eq!(
        builder.search_directories[1],
        PathBuf::from("/path/to/dir2")
    );
    assert_eq!(
        builder.search_directories[2],
        PathBuf::from("/path/to/dir3")
    );
}

#[test]
fn test_configuration_builder_with_overrides() {
    let mut overrides = HashMap::new();
    overrides.insert("key1", "value1");
    overrides.insert("key2", "value2");

    let builder = ConfigurationBuilder::new()
        .with_override("key3", "value3")
        .with_overrides(overrides);

    assert_eq!(builder.overrides.len(), 3);
    assert_eq!(builder.overrides.get("key1"), Some(&"value1".to_string()));
    assert_eq!(builder.overrides.get("key2"), Some(&"value2".to_string()));
    assert_eq!(builder.overrides.get("key3"), Some(&"value3".to_string()));
}

#[test]
fn test_configuration_builder_presets() {
    let dev_builder = ConfigurationBuilder::for_development();
    assert_eq!(dev_builder.search_directories.len(), 2);
    assert!(dev_builder.create_missing);
    assert_eq!(dev_builder.default_format, Some(ConfigFormat::Yaml));

    let prod_builder = ConfigurationBuilder::for_production();
    assert!(prod_builder.strict_validation);

    let test_builder = ConfigurationBuilder::for_testing();
    assert!(test_builder.create_missing);
}

#[tokio::test]
async fn test_determine_base_directory() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    let builder = ConfigurationBuilder::new().with_global_config_path(&config_path);

    let base_dir = builder.determine_base_directory().unwrap();
    assert_eq!(base_dir, temp_dir.path());
}
