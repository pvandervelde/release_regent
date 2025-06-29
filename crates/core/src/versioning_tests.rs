use super::*;

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
fn test_parse_invalid_version() {
    let result = VersionCalculator::parse_version("invalid");
    assert!(result.is_err());
}

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
