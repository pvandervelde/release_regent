use super::*;
use release_regent_core::{
    traits::version_calculator::{VersionBump as TraitVersionBump, VersioningStrategy},
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

// Full apply_version_bump and parse_conventional_commit coverage lives in
// crates/core/src/default_version_calculator_tests.rs where the type is defined.
