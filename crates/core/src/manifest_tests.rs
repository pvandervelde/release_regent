use super::*;

// ─────────────────────────────────────────────────────────────────────────────
// update_manifest_content — TOML format
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn update_manifest_content_toml_replaces_package_version() {
    let content = "[package]\nname = \"myapp\"\nversion = \"0.0.0\"\n";
    let result =
        update_manifest_content(content, &ManifestFormat::Toml, "package.version", "1.2.3");
    assert!(result.is_ok(), "unexpected error: {:?}", result);
    let updated = result.unwrap();
    assert!(
        updated.contains("version = \"1.2.3\""),
        "expected version = \"1.2.3\" in:\n{updated}"
    );
    assert!(!updated.contains("0.0.0"), "old version should be gone");
}

#[test]
fn update_manifest_content_toml_replaces_tool_poetry_version() {
    let content = "[tool.poetry]\nname = \"mypkg\"\nversion = \"0.1.0\"\n";
    let result = update_manifest_content(
        content,
        &ManifestFormat::Toml,
        "tool.poetry.version",
        "2.0.0",
    );
    assert!(
        result.is_ok(),
        "three-segment TOML key should succeed: {:?}",
        result
    );
    let updated = result.unwrap();
    assert!(
        updated.contains("2.0.0"),
        "updated content should contain the new version"
    );
    assert!(
        !updated.contains("0.1.0"),
        "updated content should not contain the old version"
    );
}

#[test]
fn update_manifest_content_toml_key_not_found_returns_error() {
    let content = "[package]\nname = \"myapp\"\n";
    let result =
        update_manifest_content(content, &ManifestFormat::Toml, "package.version", "1.0.0");
    assert!(result.is_err(), "missing key should return an error");
    let msg = format!("{:?}", result.unwrap_err());
    assert!(
        msg.contains("version"),
        "error should mention the missing key"
    );
}

#[test]
fn update_manifest_content_toml_malformed_returns_error() {
    let content = "this is not toml @@@ {{{";
    let result =
        update_manifest_content(content, &ManifestFormat::Toml, "package.version", "1.0.0");
    assert!(result.is_err(), "malformed TOML should return an error");
}

#[test]
fn update_manifest_content_toml_preserves_other_fields() {
    let content = "[package]\nname = \"myapp\"\nversion = \"0.0.0\"\nedition = \"2021\"\n";
    let updated =
        update_manifest_content(content, &ManifestFormat::Toml, "package.version", "1.5.0")
            .expect("update should succeed");
    assert!(
        updated.contains("name = \"myapp\""),
        "name field must be preserved"
    );
    assert!(
        updated.contains("edition = \"2021\""),
        "edition field must be preserved"
    );
    assert!(
        updated.contains("version = \"1.5.0\""),
        "version must be updated"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// update_manifest_content — JSON format
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn update_manifest_content_json_replaces_version_field() {
    let content = r#"{"name":"myapp","version":"0.0.0"}"#;
    let result = update_manifest_content(content, &ManifestFormat::Json, "version", "1.2.3");
    assert!(result.is_ok(), "unexpected error: {:?}", result);
    let updated = result.unwrap();
    assert!(
        updated.contains(r#""version": "1.2.3""#),
        "expected updated version in:\n{updated}"
    );
    assert!(!updated.contains("0.0.0"), "old version should be gone");
}

#[test]
fn update_manifest_content_json_key_not_found_returns_error() {
    let content = r#"{"name":"myapp"}"#;
    let result = update_manifest_content(content, &ManifestFormat::Json, "version", "1.0.0");
    assert!(result.is_err(), "missing key should return an error");
}

#[test]
fn update_manifest_content_json_malformed_returns_error() {
    let content = "{ not valid json }}}";
    let result = update_manifest_content(content, &ManifestFormat::Json, "version", "1.0.0");
    assert!(result.is_err(), "malformed JSON should return an error");
}

#[test]
fn update_manifest_content_json_preserves_other_fields() {
    let content = r#"{"name":"myapp","version":"0.0.0","private":true}"#;
    let updated = update_manifest_content(content, &ManifestFormat::Json, "version", "2.0.0")
        .expect("update should succeed");
    assert!(updated.contains("\"name\""), "name field must be preserved");
    assert!(
        updated.contains("\"private\""),
        "private field must be preserved"
    );
    assert!(updated.contains("\"2.0.0\""), "version must be updated");
}

// ─────────────────────────────────────────────────────────────────────────────
// update_manifest_content — PlainText format
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn update_manifest_content_plaintext_replaces_regex_match() {
    let content = "version = \"0.0.0\"\n";
    let pattern = r#"version = "([^"]+)""#;
    let result = update_manifest_content(content, &ManifestFormat::PlainText, pattern, "1.2.3");
    assert!(result.is_ok(), "unexpected error: {:?}", result);
    let updated = result.unwrap();
    assert!(
        updated.contains("version = \"1.2.3\""),
        "expected updated version in:\n{updated}"
    );
    assert!(!updated.contains("0.0.0"), "old version should be gone");
}

#[test]
fn update_manifest_content_plaintext_no_match_returns_error() {
    let content = "completely unrelated content\n";
    let pattern = r#"version = "([^"]+)""#;
    let result = update_manifest_content(content, &ManifestFormat::PlainText, pattern, "1.0.0");
    assert!(result.is_err(), "no match should return an error");
}

#[test]
fn update_manifest_content_plaintext_invalid_regex_returns_error() {
    let content = "version = \"0.0.0\"\n";
    let pattern = r"[invalid regex(((";
    let result = update_manifest_content(content, &ManifestFormat::PlainText, pattern, "1.0.0");
    assert!(result.is_err(), "invalid regex should return an error");
}

#[test]
fn update_manifest_content_plaintext_no_capture_group_returns_error() {
    let content = "version = \"0.0.0\"\n";
    // Pattern with no capture group — must be rejected.
    let pattern = r#"version = "[^"]+""#;
    let result = update_manifest_content(content, &ManifestFormat::PlainText, pattern, "1.0.0");
    assert!(
        result.is_err(),
        "pattern without capture group should return an error"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// detect_standard_manifests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn detect_standard_manifests_empty_list_returns_empty() {
    let result = detect_standard_manifests(&[]);
    assert!(result.is_empty());
}

#[test]
fn detect_standard_manifests_cargo_toml_detected() {
    let result = detect_standard_manifests(&["Cargo.toml"]);
    assert_eq!(
        result.len(),
        2,
        "root Cargo.toml should produce two entries (workspace + package)"
    );
    // First entry: plain-package key (emitted first so workspace key wins deduplication)
    assert_eq!(result[0].path, "Cargo.toml");
    assert_eq!(result[0].format, ManifestFormat::Toml);
    assert_eq!(result[0].version_key, "package.version");
    // Second entry: workspace root key (emitted last so it wins deduplication)
    assert_eq!(result[1].path, "Cargo.toml");
    assert_eq!(result[1].format, ManifestFormat::Toml);
    assert_eq!(result[1].version_key, "workspace.package.version");
}

#[test]
fn detect_standard_manifests_package_json_detected() {
    let result = detect_standard_manifests(&["package.json"]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, "package.json");
    assert_eq!(result[0].format, ManifestFormat::Json);
    assert_eq!(result[0].version_key, "version");
}

#[test]
fn detect_standard_manifests_pyproject_toml_returns_two_entries() {
    // Both PEP 517 and Poetry keys are returned; the orchestrator tries both.
    let result = detect_standard_manifests(&["pyproject.toml"]);
    assert_eq!(result.len(), 2, "expected two entries for pyproject.toml");
    let keys: Vec<&str> = result.iter().map(|m| m.version_key.as_str()).collect();
    assert!(
        keys.contains(&"project.version"),
        "PEP 517 key must be present"
    );
    assert!(
        keys.contains(&"tool.poetry.version"),
        "Poetry key must be present"
    );
}

#[test]
fn detect_standard_manifests_composer_json_detected() {
    let result = detect_standard_manifests(&["composer.json"]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, "composer.json");
    assert_eq!(result[0].format, ManifestFormat::Json);
    assert_eq!(result[0].version_key, "version");
}

#[test]
fn detect_standard_manifests_unknown_file_not_detected() {
    let result = detect_standard_manifests(&["my-custom-file.txt", "README.md"]);
    assert!(result.is_empty(), "unrecognised files must not be detected");
}

/// Verify that a workspace member `Cargo.toml` (non-root path ending in
/// `/Cargo.toml`) is detected with the `package.version` key.
#[test]
fn detect_standard_manifests_member_cargo_toml_detected() {
    let result = detect_standard_manifests(&["crates/my-crate/Cargo.toml"]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, "crates/my-crate/Cargo.toml");
    assert_eq!(result[0].format, ManifestFormat::Toml);
    assert_eq!(result[0].version_key, "package.version");
}

/// Verify that the root `Cargo.toml` (two entries) and each workspace member
/// `Cargo.toml` (one entry each) are all detected correctly when passed together,
/// and that `workspace.package.version` is the second (last-emitted) root entry
/// so that `dedup_file_updates_by_path` keeps it when both root keys collide.
#[test]
fn detect_standard_manifests_root_and_member_cargo_tomls() {
    let result = detect_standard_manifests(&[
        "Cargo.toml",
        "crates/foo/Cargo.toml",
        "crates/bar/Cargo.toml",
    ]);
    // root → 2, foo → 1, bar → 1 = 4 total
    assert_eq!(result.len(), 4);
    let root: Vec<&ManifestFileConfig> = result.iter().filter(|m| m.path == "Cargo.toml").collect();
    assert_eq!(root.len(), 2);
    let root_keys: Vec<&str> = root.iter().map(|m| m.version_key.as_str()).collect();
    assert!(
        root_keys.contains(&"workspace.package.version"),
        "workspace key must be present for root"
    );
    assert!(
        root_keys.contains(&"package.version"),
        "package key must be present for root"
    );
    let members: Vec<&ManifestFileConfig> =
        result.iter().filter(|m| m.path != "Cargo.toml").collect();
    assert_eq!(members.len(), 2);
    assert!(members.iter().all(|m| m.version_key == "package.version"));
}

// ─────────────────────────────────────────────────────────────────────────────
// update_manifest_content — TOML workspace support
// ─────────────────────────────────────────────────────────────────────────────

/// Verify that `workspace.package.version` (the workspace-level version key
/// used by Cargo workspace roots) is updated correctly.
#[test]
fn update_manifest_content_toml_replaces_workspace_package_version() {
    let content = "[workspace.package]\nversion = \"0.1.0\"\n";
    let result = update_manifest_content(
        content,
        &ManifestFormat::Toml,
        "workspace.package.version",
        "2.0.0",
    );
    assert!(
        result.is_ok(),
        "workspace.package.version update should succeed: {:?}",
        result
    );
    let updated = result.unwrap();
    assert!(
        updated.contains("2.0.0"),
        "updated content should contain the new version"
    );
    assert!(
        !updated.contains("0.1.0"),
        "updated content should not contain the old version"
    );
}

/// Verify that `version.workspace = true` (dotted-key form of workspace
/// inheritance) returns a clear error rather than a generic "not a string" error.
#[test]
fn update_manifest_content_toml_workspace_inherited_returns_error() {
    let content = "[package]\nname = \"my-crate\"\nversion.workspace = true\n";
    let result =
        update_manifest_content(content, &ManifestFormat::Toml, "package.version", "1.0.0");
    assert!(
        result.is_err(),
        "workspace-inherited version should return an error"
    );
    let msg = format!("{:?}", result.unwrap_err());
    assert!(
        msg.contains("workspace inheritance"),
        "error message should mention workspace inheritance, got: {msg}"
    );
}

/// Verify that all well-known manifests present in the same repository are
/// detected in one call: root `Cargo.toml` produces two entries (package key
/// first, workspace key last), `package.json` and `composer.json` one each
/// — four entries in total.
#[test]
fn detect_standard_manifests_multiple_files_all_detected() {
    let result = detect_standard_manifests(&["Cargo.toml", "package.json", "composer.json"]);
    // Cargo.toml → 2 (workspace + package), package.json → 1, composer.json → 1 = 4 total
    assert_eq!(result.len(), 4);
    let paths: Vec<&str> = result.iter().map(|m| m.path.as_str()).collect();
    assert!(paths.contains(&"Cargo.toml"));
    assert!(paths.contains(&"package.json"));
    assert!(paths.contains(&"composer.json"));
}

/// Verify that `version = { workspace = true }` (inline-table form) is
/// recognised as workspace inheritance and returns a clear error rather
/// than the generic "not a string value" error.
///
/// Regression guard for the inline-table detection gap: `item.as_table()`
/// returns `None` for inline tables, so this form was previously undetected.
#[test]
fn update_manifest_content_toml_workspace_inherited_inline_table_returns_error() {
    let content = "[package]\nname = \"my-crate\"\nversion = { workspace = true }\n";
    let result =
        update_manifest_content(content, &ManifestFormat::Toml, "package.version", "1.0.0");
    assert!(
        result.is_err(),
        "inline-table workspace-inherited version should return an error"
    );
    let msg = format!("{:?}", result.unwrap_err());
    assert!(
        msg.contains("workspace inheritance"),
        "error message should mention workspace inheritance, got: {msg}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// update_cargo_lock_workspace_version
// ─────────────────────────────────────────────────────────────────────────────

const SAMPLE_CARGO_LOCK: &str = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "async-trait"
version = "0.1.89"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "deadbeef"
dependencies = [
 "proc-macro2",
]

[[package]]
name = "my-app"
version = "0.1.0"
dependencies = [
 "async-trait",
 "my-lib",
]

[[package]]
name = "my-lib"
version = "0.1.0"
dependencies = [
 "async-trait",
]
"#;

#[test]
fn update_cargo_lock_workspace_version_bumps_workspace_packages() {
    let updated = update_cargo_lock_workspace_version(SAMPLE_CARGO_LOCK, "1.2.3")
        .expect("should succeed");

    // Workspace packages (no source field) are updated.
    assert!(
        updated.contains("name = \"my-app\"\nversion = \"1.2.3\""),
        "my-app should be updated; got:\n{updated}"
    );
    assert!(
        updated.contains("name = \"my-lib\"\nversion = \"1.2.3\""),
        "my-lib should be updated; got:\n{updated}"
    );
}

#[test]
fn update_cargo_lock_workspace_version_leaves_external_crates_unchanged() {
    let updated = update_cargo_lock_workspace_version(SAMPLE_CARGO_LOCK, "1.2.3")
        .expect("should succeed");

    // External crate (has source field) must not be changed.
    assert!(
        updated.contains("name = \"async-trait\"\nversion = \"0.1.89\""),
        "async-trait version must stay at 0.1.89; got:\n{updated}"
    );
}

#[test]
fn update_cargo_lock_workspace_version_no_packages_returns_content_unchanged() {
    let lock = "# This file is automatically @generated by Cargo.\nversion = 4\n";
    let result = update_cargo_lock_workspace_version(lock, "9.9.9").expect("should succeed");
    assert_eq!(result, lock);
}

#[test]
fn update_cargo_lock_workspace_version_malformed_toml_returns_error() {
    let result = update_cargo_lock_workspace_version("[[package\nname = \"broken\"", "1.0.0");
    assert!(result.is_err(), "malformed TOML should return an error");
}
