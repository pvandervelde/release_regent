use super::*;

#[test]
fn test_initial_version_calculation() {
    let calculator = VersionCalculator::new(None);
    let commits = vec![ConventionalCommit {
        commit_type: "feat".to_string(),
        scope: None,
        description: "initial feature".to_string(),
        breaking_change: false,
        message: "feat: initial feature".to_string(),
        sha: "abc123".to_string(),
    }];

    let version = calculator.calculate_next_version(&commits).unwrap();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 2); // 0.1.0 -> 0.2.0 for minor bump
    assert_eq!(version.patch, 0);
}

#[test]
fn test_parse_invalid_version() {
    let result = VersionCalculator::parse_version("invalid");
    assert!(result.is_err());
}

#[test]
fn test_parse_simple_version() {
    let version = VersionCalculator::parse_version("1.2.3").unwrap();
    assert_eq!(version.major, 1);
    assert_eq!(version.minor, 2);
    assert_eq!(version.patch, 3);
}

#[test]
fn test_parse_version_with_prefix() {
    let version = VersionCalculator::parse_version("v1.2.3").unwrap();
    assert_eq!(version.major, 1);
    assert_eq!(version.minor, 2);
    assert_eq!(version.patch, 3);
}

#[test]
fn test_semantic_version_display() {
    let version = SemanticVersion {
        major: 1,
        minor: 2,
        patch: 3,
        prerelease: None,
        build: None,
    };
    assert_eq!(version.to_string(), "1.2.3");
}

#[test]
fn test_semantic_version_display_full() {
    let version = SemanticVersion {
        major: 1,
        minor: 2,
        patch: 3,
        prerelease: Some("beta.2".to_string()),
        build: Some("build.123".to_string()),
    };
    assert_eq!(version.to_string(), "1.2.3-beta.2+build.123");
}

#[test]
fn test_semantic_version_display_with_build() {
    let version = SemanticVersion {
        major: 1,
        minor: 2,
        patch: 3,
        prerelease: None,
        build: Some("20220101".to_string()),
    };
    assert_eq!(version.to_string(), "1.2.3+20220101");
}

#[test]
fn test_semantic_version_display_with_prerelease() {
    let version = SemanticVersion {
        major: 1,
        minor: 2,
        patch: 3,
        prerelease: Some("alpha.1".to_string()),
        build: None,
    };
    assert_eq!(version.to_string(), "1.2.3-alpha.1");
}

#[test]
fn test_version_bump_application() {
    let calculator = VersionCalculator::new(None);
    let base = SemanticVersion {
        major: 1,
        minor: 2,
        patch: 3,
        prerelease: None,
        build: None,
    };

    // Major bump
    let major_result = calculator.apply_version_bump(&base, VersionBump::Major);
    assert_eq!(major_result.major, 2);
    assert_eq!(major_result.minor, 0);
    assert_eq!(major_result.patch, 0);

    // Minor bump
    let minor_result = calculator.apply_version_bump(&base, VersionBump::Minor);
    assert_eq!(minor_result.major, 1);
    assert_eq!(minor_result.minor, 3);
    assert_eq!(minor_result.patch, 0);

    // Patch bump
    let patch_result = calculator.apply_version_bump(&base, VersionBump::Patch);
    assert_eq!(patch_result.major, 1);
    assert_eq!(patch_result.minor, 2);
    assert_eq!(patch_result.patch, 4);

    // No bump
    let none_result = calculator.apply_version_bump(&base, VersionBump::None);
    assert_eq!(none_result, base);
}

#[test]
fn test_version_bump_determination() {
    let calculator = VersionCalculator::new(None);

    // Test breaking change
    let breaking_commits = vec![ConventionalCommit {
        commit_type: "feat".to_string(),
        scope: None,
        description: "add new feature".to_string(),
        breaking_change: true,
        message: "feat: add new feature\n\nBREAKING CHANGE: API changed".to_string(),
        sha: "abc123".to_string(),
    }];
    assert_eq!(
        calculator.determine_version_bump(&breaking_commits),
        VersionBump::Major
    );

    // Test feature
    let feature_commits = vec![ConventionalCommit {
        commit_type: "feat".to_string(),
        scope: None,
        description: "add new feature".to_string(),
        breaking_change: false,
        message: "feat: add new feature".to_string(),
        sha: "def456".to_string(),
    }];
    assert_eq!(
        calculator.determine_version_bump(&feature_commits),
        VersionBump::Minor
    );

    // Test fix
    let fix_commits = vec![ConventionalCommit {
        commit_type: "fix".to_string(),
        scope: None,
        description: "fix bug".to_string(),
        breaking_change: false,
        message: "fix: fix bug".to_string(),
        sha: "ghi789".to_string(),
    }];
    assert_eq!(
        calculator.determine_version_bump(&fix_commits),
        VersionBump::Patch
    );

    // Test no relevant changes
    let chore_commits = vec![ConventionalCommit {
        commit_type: "chore".to_string(),
        scope: None,
        description: "update dependencies".to_string(),
        breaking_change: false,
        message: "chore: update dependencies".to_string(),
        sha: "jkl012".to_string(),
    }];
    assert_eq!(
        calculator.determine_version_bump(&chore_commits),
        VersionBump::None
    );
}

#[test]
fn test_version_comparison() {
    let v1_0_0 = SemanticVersion {
        major: 1,
        minor: 0,
        patch: 0,
        prerelease: None,
        build: None,
    };

    let v1_0_1 = SemanticVersion {
        major: 1,
        minor: 0,
        patch: 1,
        prerelease: None,
        build: None,
    };

    let v1_1_0 = SemanticVersion {
        major: 1,
        minor: 1,
        patch: 0,
        prerelease: None,
        build: None,
    };

    assert!(v1_0_0 < v1_0_1);
    assert!(v1_0_1 < v1_1_0);
    assert_eq!(v1_0_0, v1_0_0);
}

#[test]
fn test_parse_conventional_commit_basic() {
    let commits = vec![
        (
            "abc123".to_string(),
            "feat: add user authentication".to_string(),
        ),
        ("def456".to_string(), "fix: resolve login bug".to_string()),
        ("ghi789".to_string(), "docs: update README".to_string()),
    ];

    let parsed = VersionCalculator::parse_conventional_commits(&commits);

    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed[0].commit_type, "feat");
    assert_eq!(parsed[0].description, "add user authentication");
    assert_eq!(parsed[0].breaking_change, false);

    assert_eq!(parsed[1].commit_type, "fix");
    assert_eq!(parsed[1].description, "resolve login bug");
    assert_eq!(parsed[1].breaking_change, false);

    assert_eq!(parsed[2].commit_type, "docs");
    assert_eq!(parsed[2].description, "update README");
    assert_eq!(parsed[2].breaking_change, false);
}

#[test]
fn test_parse_conventional_commit_with_scope() {
    let commits = vec![
        (
            "abc123".to_string(),
            "feat(auth): add OAuth support".to_string(),
        ),
        (
            "def456".to_string(),
            "fix(ui): button alignment issue".to_string(),
        ),
    ];

    let parsed = VersionCalculator::parse_conventional_commits(&commits);

    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].commit_type, "feat");
    assert_eq!(parsed[0].scope, Some("auth".to_string()));
    assert_eq!(parsed[0].description, "add OAuth support");

    assert_eq!(parsed[1].commit_type, "fix");
    assert_eq!(parsed[1].scope, Some("ui".to_string()));
    assert_eq!(parsed[1].description, "button alignment issue");
}

#[test]
fn test_parse_conventional_commit_breaking_change_exclamation() {
    let commits = vec![
        (
            "abc123".to_string(),
            "feat!: remove deprecated API".to_string(),
        ),
        (
            "def456".to_string(),
            "fix(auth)!: change authentication flow".to_string(),
        ),
    ];

    let parsed = VersionCalculator::parse_conventional_commits(&commits);

    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].commit_type, "feat");
    assert_eq!(parsed[0].breaking_change, true);
    assert_eq!(parsed[0].description, "remove deprecated API");

    assert_eq!(parsed[1].commit_type, "fix");
    assert_eq!(parsed[1].scope, Some("auth".to_string()));
    assert_eq!(parsed[1].breaking_change, true);
    assert_eq!(parsed[1].description, "change authentication flow");
}

#[test]
fn test_parse_conventional_commit_breaking_change_footer() {
    let commits = vec![
        (
            "abc123".to_string(),
            "feat: add new feature\n\nBREAKING CHANGE: This removes the old API".to_string(),
        ),
        (
            "def456".to_string(),
            "fix: bug fix\n\nBREAKING-CHANGE: Changes behavior".to_string(),
        ),
    ];

    let parsed = VersionCalculator::parse_conventional_commits(&commits);

    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].commit_type, "feat");
    assert_eq!(parsed[0].breaking_change, true);
    assert_eq!(parsed[0].description, "add new feature");

    assert_eq!(parsed[1].commit_type, "fix");
    assert_eq!(parsed[1].breaking_change, true);
    assert_eq!(parsed[1].description, "bug fix");
}

#[test]
fn test_parse_non_conventional_commit() {
    let commits = vec![
        ("abc123".to_string(), "Update README file".to_string()),
        ("def456".to_string(), "random commit message".to_string()),
        ("ghi789".to_string(), "".to_string()),
    ];

    let parsed = VersionCalculator::parse_conventional_commits(&commits);

    assert_eq!(parsed.len(), 3);
    // Non-conventional commits should be treated as "chore"
    assert_eq!(parsed[0].commit_type, "chore");
    assert_eq!(parsed[0].description, "Update README file");
    assert_eq!(parsed[0].breaking_change, false);

    assert_eq!(parsed[1].commit_type, "chore");
    assert_eq!(parsed[1].description, "random commit message");
    assert_eq!(parsed[1].breaking_change, false);

    assert_eq!(parsed[2].commit_type, "chore");
    assert_eq!(parsed[2].description, "");
    assert_eq!(parsed[2].breaking_change, false);
}

// Enhanced semantic version parsing tests

#[test]
fn test_parse_version_with_prerelease() {
    let version = VersionCalculator::parse_version("1.2.3-alpha.1").unwrap();
    assert_eq!(version.major, 1);
    assert_eq!(version.minor, 2);
    assert_eq!(version.patch, 3);
    assert_eq!(version.prerelease, Some("alpha.1".to_string()));
    assert_eq!(version.build, None);
}

#[test]
fn test_parse_version_with_build_metadata() {
    let version = VersionCalculator::parse_version("1.2.3+20210101.abcd123").unwrap();
    assert_eq!(version.major, 1);
    assert_eq!(version.minor, 2);
    assert_eq!(version.patch, 3);
    assert_eq!(version.prerelease, None);
    assert_eq!(version.build, Some("20210101.abcd123".to_string()));
}

#[test]
fn test_parse_version_with_prerelease_and_build() {
    let version = VersionCalculator::parse_version("1.2.3-beta.2+exp.sha.5114f85").unwrap();
    assert_eq!(version.major, 1);
    assert_eq!(version.minor, 2);
    assert_eq!(version.patch, 3);
    assert_eq!(version.prerelease, Some("beta.2".to_string()));
    assert_eq!(version.build, Some("exp.sha.5114f85".to_string()));
}

#[test]
fn test_parse_version_with_prefix_and_metadata() {
    let version = VersionCalculator::parse_version("v2.0.0-rc.1+build.1").unwrap();
    assert_eq!(version.major, 2);
    assert_eq!(version.minor, 0);
    assert_eq!(version.patch, 0);
    assert_eq!(version.prerelease, Some("rc.1".to_string()));
    assert_eq!(version.build, Some("build.1".to_string()));
}

#[test]
fn test_parse_version_invalid_leading_zeros() {
    assert!(VersionCalculator::parse_version("01.2.3").is_err());
    assert!(VersionCalculator::parse_version("1.02.3").is_err());
    assert!(VersionCalculator::parse_version("1.2.03").is_err());
}

#[test]
fn test_parse_version_invalid_prerelease_leading_zeros() {
    assert!(VersionCalculator::parse_version("1.2.3-01").is_err());
    assert!(VersionCalculator::parse_version("1.2.3-alpha.01").is_err());
}

#[test]
fn test_parse_version_empty_components() {
    assert!(VersionCalculator::parse_version("").is_err());
    assert!(VersionCalculator::parse_version("1..3").is_err());
    assert!(VersionCalculator::parse_version("1.2.").is_err());
    assert!(VersionCalculator::parse_version("1.2.3-").is_err());
    assert!(VersionCalculator::parse_version("1.2.3+").is_err());
}

#[test]
fn test_parse_version_invalid_characters() {
    assert!(VersionCalculator::parse_version("1.2.3-alpha@beta").is_err());
    assert!(VersionCalculator::parse_version("1.2.3+build$123").is_err());
    assert!(VersionCalculator::parse_version("1.2.3-α").is_err()); // Non-ASCII
}

#[test]
fn test_version_formatting_with_prefix() {
    let version = SemanticVersion {
        major: 1,
        minor: 2,
        patch: 3,
        prerelease: Some("alpha.1".to_string()),
        build: Some("build.123".to_string()),
    };
    
    assert_eq!(version.to_string_with_prefix(false), "1.2.3-alpha.1+build.123");
    assert_eq!(version.to_string_with_prefix(true), "v1.2.3-alpha.1+build.123");
}

#[test]
fn test_version_prerelease_detection() {
    let normal = SemanticVersion {
        major: 1, minor: 0, patch: 0,
        prerelease: None, build: None,
    };
    let prerelease = SemanticVersion {
        major: 1, minor: 0, patch: 0,
        prerelease: Some("alpha.1".to_string()), build: None,
    };
    
    assert!(!normal.is_prerelease());
    assert!(prerelease.is_prerelease());
}

#[test]
fn test_version_build_metadata_detection() {
    let without_build = SemanticVersion {
        major: 1, minor: 0, patch: 0,
        prerelease: None, build: None,
    };
    let with_build = SemanticVersion {
        major: 1, minor: 0, patch: 0,
        prerelease: None, build: Some("build.123".to_string()),
    };
    
    assert!(!without_build.has_build_metadata());
    assert!(with_build.has_build_metadata());
}

#[test]
fn test_version_precedence_comparison() {
    let v1_0_0 = SemanticVersion {
        major: 1, minor: 0, patch: 0,
        prerelease: None, build: None,
    };
    let v1_0_0_alpha = SemanticVersion {
        major: 1, minor: 0, patch: 0,
        prerelease: Some("alpha".to_string()), build: None,
    };
    let v1_0_0_beta = SemanticVersion {
        major: 1, minor: 0, patch: 0,
        prerelease: Some("beta".to_string()), build: None,
    };
    let v1_0_0_with_build = SemanticVersion {
        major: 1, minor: 0, patch: 0,
        prerelease: None, build: Some("build.123".to_string()),
    };
    
    use std::cmp::Ordering;
    
    // Pre-release versions have lower precedence than normal versions
    assert_eq!(v1_0_0_alpha.compare_precedence(&v1_0_0), Ordering::Less);
    assert_eq!(v1_0_0.compare_precedence(&v1_0_0_alpha), Ordering::Greater);
    
    // Compare pre-release versions alphabetically
    assert_eq!(v1_0_0_alpha.compare_precedence(&v1_0_0_beta), Ordering::Less);
    
    // Build metadata is ignored in precedence comparison
    assert_eq!(v1_0_0.compare_precedence(&v1_0_0_with_build), Ordering::Equal);
}

#[test]
fn test_complex_prerelease_versions() {
    let version1 = VersionCalculator::parse_version("1.0.0-alpha").unwrap();
    let version2 = VersionCalculator::parse_version("1.0.0-alpha.1").unwrap();
    let version3 = VersionCalculator::parse_version("1.0.0-alpha.beta").unwrap();
    let version4 = VersionCalculator::parse_version("1.0.0-beta").unwrap();
    let version5 = VersionCalculator::parse_version("1.0.0-beta.2").unwrap();
    let version6 = VersionCalculator::parse_version("1.0.0-beta.11").unwrap();
    let version7 = VersionCalculator::parse_version("1.0.0-rc.1").unwrap();
    let version8 = VersionCalculator::parse_version("1.0.0").unwrap();
    
    // Verify all parsed correctly
    assert!(version1.is_prerelease());
    assert!(version2.is_prerelease());
    assert!(version3.is_prerelease());
    assert!(version4.is_prerelease());
    assert!(version5.is_prerelease());
    assert!(version6.is_prerelease());
    assert!(version7.is_prerelease());
    assert!(!version8.is_prerelease());
}
