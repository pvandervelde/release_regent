//! Structured logging and span propagation tests for [`ReleaseOrchestrator`].
//!
//! These tests verify that `orchestrate` emits all required structured fields
//! so that log lines are filterable and traceable in production monitoring.

use super::super::ReleaseOrchestrator;
use super::{default_config, ver, TestGitHub};
use tracing_test::traced_test;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Structured logging / span propagation tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// `orchestrate` must include the `correlation_id` as a structured field in its
/// span so that ALL log lines emitted during orchestration are correlated.
///
/// This test will fail if the function uses `span.enter()` without also
/// including `correlation_id` in a log event, or if `#[tracing::instrument]`
/// is absent and the span is never opened.
#[tokio::test]
#[traced_test]
async fn test_orchestrate_logs_contain_correlation_id() {
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 0, 0),
            "- feat: something",
            "main",
            "sha-001",
            "unique-corr-id-ALPHA",
        )
        .await
        .expect("orchestrate should succeed");

    assert!(
        logs_contain("unique-corr-id-ALPHA"),
        "correlation_id must appear in structured log output"
    );
}

/// `orchestrate` must record `owner` and `repo` as structured fields so that
/// log lines are filterable by repository in production monitoring.
#[tokio::test]
#[traced_test]
async fn test_orchestrate_logs_contain_owner_and_repo() {
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    orchestrator
        .orchestrate(
            "unique-owner-BETA",
            "unique-repo-BETA",
            &ver(2, 0, 0),
            "- fix: something",
            "main",
            "sha-002",
            "corr-002",
        )
        .await
        .expect("orchestrate should succeed");

    assert!(
        logs_contain("unique-owner-BETA"),
        "owner must appear in structured log output"
    );
    assert!(
        logs_contain("unique-repo-BETA"),
        "repo must appear in structured log output"
    );
}

/// `orchestrate` must record the version as a structured field.
#[tokio::test]
#[traced_test]
async fn test_orchestrate_logs_contain_version() {
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(9, 8, 7),
            "- chore: bump",
            "main",
            "sha-003",
            "corr-003",
        )
        .await
        .expect("orchestrate should succeed");

    assert!(
        logs_contain("9.8.7"),
        "version must appear in structured log output"
    );
}
