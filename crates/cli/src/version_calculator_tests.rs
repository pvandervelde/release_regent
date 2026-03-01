use super::*;
use release_regent_core::{
    traits::version_calculator::{
        CalculationOptions, CommitAnalysis, VersionBump as TraitVersionBump, VersionContext,
        VersioningStrategy,
    },
    versioning::SemanticVersion,
};
use std::collections::HashMap;

/// Build a minimal `VersionContext` for testing.
fn test_context() -> VersionContext {
    VersionContext {
        base_ref: Some("HEAD~3".to_string()),
        current_version: Some(SemanticVersion {
            major: 1,
            minor: 0,
            patch: 0,
            prerelease: None,
            build: None,
        }),
        head_ref: "HEAD".to_string(),
        owner: "test-owner".to_string(),
        repo: "test-repo".to_string(),
        target_branch: "main".to_string(),
    }
}

/// Build a minimal `CommitAnalysis` with the given bump and breaking flag.
fn make_analysis(bump: TraitVersionBump, is_breaking: bool) -> CommitAnalysis {
    CommitAnalysis {
        author: "author".to_string(),
        commit_type: Some("feat".to_string()),
        date: chrono::Utc::now(),
        is_breaking,
        message: "feat: test commit".to_string(),
        metadata: HashMap::new(),
        scope: None,
        sha: "abc123".to_string(),
        version_bump: bump,
    }
}

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
// local_to_trait_bump / trait_to_local_bump round-trip
// ──────────────────────────────────────────────────────────────

#[test]
fn bump_mapping_round_trips_major() {
    use release_regent_core::versioning::VersionBump as LocalBump;
    let trait_bump = DefaultVersionCalculator::local_to_trait_bump(LocalBump::Major);
    let local_back = DefaultVersionCalculator::trait_to_local_bump(&trait_bump);
    assert_eq!(local_back, LocalBump::Major);
}

#[test]
fn bump_mapping_round_trips_minor() {
    use release_regent_core::versioning::VersionBump as LocalBump;
    let trait_bump = DefaultVersionCalculator::local_to_trait_bump(LocalBump::Minor);
    let local_back = DefaultVersionCalculator::trait_to_local_bump(&trait_bump);
    assert_eq!(local_back, LocalBump::Minor);
}

#[test]
fn bump_mapping_round_trips_patch() {
    use release_regent_core::versioning::VersionBump as LocalBump;
    let trait_bump = DefaultVersionCalculator::local_to_trait_bump(LocalBump::Patch);
    let local_back = DefaultVersionCalculator::trait_to_local_bump(&trait_bump);
    assert_eq!(local_back, LocalBump::Patch);
}

#[test]
fn bump_mapping_round_trips_none() {
    use release_regent_core::versioning::VersionBump as LocalBump;
    let trait_bump = DefaultVersionCalculator::local_to_trait_bump(LocalBump::None);
    let local_back = DefaultVersionCalculator::trait_to_local_bump(&trait_bump);
    assert_eq!(local_back, LocalBump::None);
}

// ──────────────────────────────────────────────────────────────
// highest_bump
// ──────────────────────────────────────────────────────────────

#[test]
fn highest_bump_returns_none_for_empty_list() {
    let result = DefaultVersionCalculator::highest_bump(&[]);
    assert_eq!(result, TraitVersionBump::None);
}

#[test]
fn highest_bump_returns_minor_when_only_features() {
    let analyses = vec![
        make_analysis(TraitVersionBump::Minor, false),
        make_analysis(TraitVersionBump::Patch, false),
    ];
    let result = DefaultVersionCalculator::highest_bump(&analyses);
    assert_eq!(result, TraitVersionBump::Minor);
}

#[test]
fn highest_bump_returns_major_when_breaking_change_present() {
    let analyses = vec![
        make_analysis(TraitVersionBump::Minor, false),
        make_analysis(TraitVersionBump::Major, true),
    ];
    let result = DefaultVersionCalculator::highest_bump(&analyses);
    assert_eq!(result, TraitVersionBump::Major);
}

#[test]
fn highest_bump_returns_patch_for_only_fixes() {
    let analyses = vec![
        make_analysis(TraitVersionBump::Patch, false),
        make_analysis(TraitVersionBump::None, false),
    ];
    let result = DefaultVersionCalculator::highest_bump(&analyses);
    assert_eq!(result, TraitVersionBump::Patch);
}

// ──────────────────────────────────────────────────────────────
// to_commit_analysis
// ──────────────────────────────────────────────────────────────

#[test]
fn to_commit_analysis_maps_feat_to_minor_bump() {
    use release_regent_core::versioning::ConventionalCommit;
    let commit = ConventionalCommit {
        commit_type: "feat".to_string(),
        scope: Some("auth".to_string()),
        description: "add OAuth".to_string(),
        breaking_change: false,
        message: "feat(auth): add OAuth".to_string(),
        sha: "deadbeef".to_string(),
    };
    let analysis = DefaultVersionCalculator::to_commit_analysis(commit);
    assert_eq!(analysis.version_bump, TraitVersionBump::Minor);
    assert!(!analysis.is_breaking);
    assert_eq!(analysis.scope, Some("auth".to_string()));
}

#[test]
fn to_commit_analysis_maps_breaking_change_to_major_bump() {
    use release_regent_core::versioning::ConventionalCommit;
    let commit = ConventionalCommit {
        commit_type: "feat".to_string(),
        scope: None,
        description: "remove deprecated API".to_string(),
        breaking_change: true,
        message: "feat!: remove deprecated API".to_string(),
        sha: "cafebabe".to_string(),
    };
    let analysis = DefaultVersionCalculator::to_commit_analysis(commit);
    assert_eq!(analysis.version_bump, TraitVersionBump::Major);
    assert!(analysis.is_breaking);
}

#[test]
fn to_commit_analysis_maps_fix_to_patch_bump() {
    use release_regent_core::versioning::ConventionalCommit;
    let commit = ConventionalCommit {
        commit_type: "fix".to_string(),
        scope: None,
        description: "resolve null pointer".to_string(),
        breaking_change: false,
        message: "fix: resolve null pointer".to_string(),
        sha: "1234567".to_string(),
    };
    let analysis = DefaultVersionCalculator::to_commit_analysis(commit);
    assert_eq!(analysis.version_bump, TraitVersionBump::Patch);
}

// ──────────────────────────────────────────────────────────────
// build_result
// ──────────────────────────────────────────────────────────────

#[test]
fn build_result_excludes_none_bump_commits_from_changelog() {
    use release_regent_core::versioning::ConventionalCommit;
    let ctx = test_context();
    let strategy = VersioningStrategy::ConventionalCommits {
        custom_types: HashMap::new(),
        include_prerelease: false,
    };
    let feat_commit = ConventionalCommit {
        commit_type: "feat".to_string(),
        scope: None,
        description: "new feature".to_string(),
        breaking_change: false,
        message: "feat: new feature".to_string(),
        sha: "aaa".to_string(),
    };
    let chore_commit = ConventionalCommit {
        commit_type: "chore".to_string(),
        scope: None,
        description: "update deps".to_string(),
        breaking_change: false,
        message: "chore: update deps".to_string(),
        sha: "bbb".to_string(),
    };
    let analyses = vec![
        DefaultVersionCalculator::to_commit_analysis(feat_commit),
        DefaultVersionCalculator::to_commit_analysis(chore_commit),
    ];
    let next = SemanticVersion {
        major: 1,
        minor: 1,
        patch: 0,
        prerelease: None,
        build: None,
    };

    let result = DefaultVersionCalculator::build_result(
        &ctx,
        strategy,
        analyses,
        next,
        TraitVersionBump::Minor,
    );

    // Only the feat commit should appear in the changelog.
    assert_eq!(result.changelog_entries.len(), 1);
    assert_eq!(result.changelog_entries[0].commit_sha, "aaa");
    assert_eq!(result.version_bump, TraitVersionBump::Minor);
    assert_eq!(result.next_version.minor, 1);
}

// ──────────────────────────────────────────────────────────────
// apply_version_bump (async trait call, will panic until implemented)
// ──────────────────────────────────────────────────────────────

#[test]
#[should_panic]
fn apply_version_bump_panics_until_implemented() {
    let calc = DefaultVersionCalculator::new();
    let base = SemanticVersion {
        major: 1,
        minor: 0,
        patch: 0,
        prerelease: None,
        build: None,
    };
    // This should todo!() panic in Phase 1.
    let _ = calc.apply_version_bump(base, TraitVersionBump::Minor, None, None);
}

// ──────────────────────────────────────────────────────────────
// parse_conventional_commit (will panic until implemented)
// ──────────────────────────────────────────────────────────────

#[test]
#[should_panic]
fn parse_conventional_commit_panics_until_implemented() {
    let calc = DefaultVersionCalculator::new();
    let _ = calc.parse_conventional_commit("feat: something");
}
