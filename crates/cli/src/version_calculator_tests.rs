use super::*;
use release_regent_core::{
    traits::version_calculator::{VersionBump as TraitVersionBump, VersioningStrategy},
    versioning::SemanticVersion,
    VersionCalculator,
};

// ──────────────────────────────────────────────────────────────
// DefaultVersionCalculator::new
// ──────────────────────────────────────────────────────────────

#[test]
fn new_creates_default_calculator() {
    let calc = DefaultVersionCalculator::new();
    // The type exists and can be constructed.
    let _ = calc;
}

// ──────────────────────────────────────────────────────────────
// supported_strategies / default_strategy
// ──────────────────────────────────────────────────────────────

#[test]
fn supported_strategies_includes_conventional_commits() {
    let calc = DefaultVersionCalculator::new();
    let strategies = calc.supported_strategies();
    assert!(
        strategies.contains_key("conventional_commits"),
        "Expected 'conventional_commits' key, got: {:?}",
        strategies.keys().collect::<Vec<_>>()
    );
}

#[test]
fn default_strategy_is_conventional_commits() {
    let calc = DefaultVersionCalculator::new();
    match calc.default_strategy() {
        VersioningStrategy::ConventionalCommits { .. } => {}
        other => panic!("Unexpected default strategy: {:?}", other),
    }
}

// ──────────────────────────────────────────────────────────────
// apply_version_bump (public trait method)
// ──────────────────────────────────────────────────────────────

#[test]
fn apply_version_bump_increments_minor_version() {
    let calc = DefaultVersionCalculator::new();
    let base = SemanticVersion {
        major: 1,
        minor: 0,
        patch: 0,
        prerelease: None,
        build: None,
    };
    let result = calc.apply_version_bump(base, TraitVersionBump::Minor, None, None);
    assert!(result.is_ok());
    let next = result.unwrap();
    assert_eq!(next.major, 1);
    assert_eq!(next.minor, 1);
    assert_eq!(next.patch, 0);
}

#[test]
fn apply_version_bump_increments_major_and_resets_minor_patch() {
    let calc = DefaultVersionCalculator::new();
    let base = SemanticVersion {
        major: 1,
        minor: 2,
        patch: 3,
        prerelease: None,
        build: None,
    };
    let result = calc.apply_version_bump(base, TraitVersionBump::Major, None, None);
    assert!(result.is_ok());
    let next = result.unwrap();
    assert_eq!(next.major, 2);
    assert_eq!(next.minor, 0);
    assert_eq!(next.patch, 0);
}

#[test]
fn apply_version_bump_with_none_leaves_version_unchanged() {
    let calc = DefaultVersionCalculator::new();
    let base = SemanticVersion {
        major: 3,
        minor: 4,
        patch: 5,
        prerelease: None,
        build: None,
    };
    let result = calc.apply_version_bump(base.clone(), TraitVersionBump::None, None, None);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), base);
}

#[test]
fn apply_version_bump_sets_prerelease_and_build() {
    let calc = DefaultVersionCalculator::new();
    let base = SemanticVersion {
        major: 1,
        minor: 0,
        patch: 0,
        prerelease: None,
        build: None,
    };
    let result = calc.apply_version_bump(
        base,
        TraitVersionBump::Patch,
        Some("alpha.1".to_string()),
        Some("build.42".to_string()),
    );
    assert!(result.is_ok());
    let next = result.unwrap();
    assert_eq!(next.patch, 1);
    assert_eq!(next.prerelease, Some("alpha.1".to_string()));
    assert_eq!(next.build, Some("build.42".to_string()));
}

// ──────────────────────────────────────────────────────────────
// parse_conventional_commit (public trait method)
// ──────────────────────────────────────────────────────────────

#[test]
fn parse_conventional_commit_returns_analysis_for_feat_commit() {
    let calc = DefaultVersionCalculator::new();
    let result = calc.parse_conventional_commit("feat: add OAuth login");
    assert!(result.is_ok());
    let opt = result.unwrap();
    assert!(opt.is_some(), "Expected Some for conventional commit");
    let analysis = opt.unwrap();
    assert_eq!(analysis.version_bump, TraitVersionBump::Minor);
    assert!(!analysis.is_breaking);
}

#[test]
fn parse_conventional_commit_returns_none_for_non_conventional() {
    let calc = DefaultVersionCalculator::new();
    let result = calc.parse_conventional_commit("Update README.md");
    assert!(result.is_ok());
    assert!(
        result.unwrap().is_none(),
        "Expected None for non-conventional commit"
    );
}

#[test]
fn parse_conventional_commit_identifies_breaking_change() {
    let calc = DefaultVersionCalculator::new();
    let result = calc.parse_conventional_commit("feat!: remove deprecated endpoint");
    assert!(result.is_ok());
    let opt = result.unwrap();
    assert!(opt.is_some());
    let analysis = opt.unwrap();
    assert_eq!(analysis.version_bump, TraitVersionBump::Major);
    assert!(analysis.is_breaking);
}
