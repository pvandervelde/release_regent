# User Stories & Personas

**Last Updated**: 2025-07-19
**Status**: Complete

## User Personas

### Sarah - Repository Maintainer

**Profile**:

- Maintains 3-5 open source projects
- Values consistency and predictability in release processes
- Wants automation without losing control over release timing
- Needs to provide clear guidance to contributors

**Pain Points**:

- Manual release creation is time-consuming and error-prone
- Different contributors follow different release procedures
- Hard to maintain consistency across multiple repositories
- Release timing often doesn't align with availability

**Goals**:

- Automated release mechanics with human timing control
- Consistent release processes across all repositories
- Clear audit trail for release decisions
- Minimal maintenance overhead

### DevOps Team - Enterprise

**Profile**:

- Manages 200+ internal repositories
- Subject to compliance and audit requirements
- Needs integration with existing CI/CD workflows
- Values reliability and error visibility

**Pain Points**:

- Manual releases don't scale across many repositories
- Inconsistent release processes create compliance risks
- Need detailed audit trails for regulatory requirements
- Difficult to troubleshoot release failures across teams

**Goals**:

- Standardized release automation across all repositories
- Comprehensive logging and audit capabilities
- Integration with existing monitoring and alerting
- Self-service capability for development teams

## User Stories

### US-1: Automated Release PR Creation

**As** Sarah, **I want** Release Regent to automatically create release PRs when changes are merged **so that** I don't have to manually track and create releases.

**Acceptance Criteria**:

- Release PR created within 30 seconds of merge webhook
- PR includes correct semantic version based on conventional commits
- PR contains generated changelog with all changes since last release
- PR targets the correct main branch
- PR uses configurable title and body templates

**Priority**: High
**Status**: In Progress

### US-2: Release PR Management

**As** Sarah, **I want** Release Regent to update existing release PRs with new changes **so that** I have a single PR tracking all unreleased changes.

**Acceptance Criteria**:

- Existing release PR updated with new version if higher
- Changelog updated with new entries
- PR title and branch renamed if version changes
- Never downgrades version in existing PR
- Handles concurrent updates gracefully

**Priority**: High
**Status**: In Progress

### US-3: GitHub Release Creation

**As** Sarah, **I want** GitHub releases to be created automatically when I merge release PRs **so that** the final release publication is seamless.

**Acceptance Criteria**:

- GitHub release created within 30 seconds of release PR merge
- Git tag created pointing to the correct merge commit
- Release notes extracted from release PR body
- Release marked appropriately (draft/prerelease based on config)
- Release branch cleaned up after successful creation

**Priority**: High
**Status**: Planned

### US-4: Version Control

**As** Sarah, **I want** to override the calculated version via PR comments **so that** I can control release timing and version bumps.

**Acceptance Criteria**:

**`!set-version X.Y.Z` (explicit pin):**

- A comment containing `!set-version X.Y.Z` on the **active release PR** (head branch
  matching `release/v*`) updates that release PR to exactly version `X.Y.Z`
- If the command is posted on any other open PR, a scope rejection comment is posted
  explaining that `!set-version` must be re-posted on the release PR; no PR is modified
- Only valid semantic version strings are accepted; malformed strings produce a rejection comment
- The specified version must be strictly greater than the current released version (latest
  semver tag); violations produce a rejection comment with the reason
- Only collaborators with Write access or above may issue commands

**`!release major|minor|patch` (bump-floor override):**

- A comment containing `!release major`, `!release minor`, or `!release patch` applies a
  `rr:override-major/minor/patch` label to the **PR the comment was posted on** (the feature PR)
- A confirmation comment is posted on the feature PR confirming the override and its scope
- When the feature PR is **merged**, the override label is read from that PR and used as a
  minimum-bump floor during orchestration: `effective_version = max(calculated_version, floor_version)`
- If the feature PR is **closed without merging**, the label remains on the closed PR and
  has no effect on any future merges — version decisions are only made based on merged work
- When the **release PR is merged** (a release is published), any `rr:override-*` labels
  on open feature PRs are **automatically cleared** and each affected PR receives an
  informational comment. Overrides are valid for one release cycle only; contributors must
  re-post `!release` if the intent still applies to the next release.
- Posting a new `!release` command on the same PR replaces the previous override label
  and posts an updated confirmation comment
- The floor is applied as a minimum; it can never reduce a version that conventional
  commits have computed to be higher
- A `BREAKING CHANGE:` commit always produces a major bump and cannot be reduced by a
  `!release minor` or `!release patch` override
- When a floor is applied during orchestration, an audit comment is posted on the release
  PR identifying the source feature PR and the version change

**General:**

- Commands are only processed when `VersioningConfig::allow_override = true`
- Commands on closed or merged PRs are silently ignored
- Commands from collaborators with Triage or Read access produce a `❌` rejection comment
  identifying the commenter and explaining the permission requirement; the command has no
  other effect

**Priority**: Medium
**Status**: In Progress

### US-5: Error Visibility

**As** the DevOps team, **I want** to be notified when Release Regent encounters persistent failures **so that** I can address issues preventing releases.

**Acceptance Criteria**:

- Transient failures retried up to 5 times
- Persistent failures trigger configurable notifications
- All failures logged with correlation IDs
- Clear error context for troubleshooting
- Integration with existing alerting systems

**Priority**: Medium
**Status**: Future Enhancement

### US-6: Configuration Management

**As** the DevOps team, **I want** to configure Release Regent behavior per repository **so that** different projects can have appropriate release processes.

**Acceptance Criteria**:

- Repository-specific configuration overrides
- Template customization for PR titles and bodies
- Configurable versioning strategies
- Validation of configuration before processing
- Clear error messages for invalid configuration

**Priority**: Medium
**Status**: Complete

### US-7: CLI Testing

**As** Sarah, **I want** to test Release Regent behavior locally **so that** I can validate configuration and debug issues.

**Acceptance Criteria**:

- CLI can simulate webhook processing
- Preview mode shows what would happen without executing
- Local configuration validation
- Test against real repositories safely
- Debug logging for troubleshooting

**Priority**: Medium
**Status**: Complete

### US-8: Audit Trail

**As** the DevOps team, **I want** comprehensive logging of all Release Regent operations **so that** we can meet compliance requirements.

**Acceptance Criteria**:

- All operations logged with correlation IDs
- Structured logging for programmatic analysis
- Audit trail from webhook to release creation
- No sensitive information in logs
- Log retention aligned with compliance requirements

**Priority**: Medium
**Status**: Complete

## User Journey Maps

### Release PR Creation Journey

```mermaid
journey
    title Sarah's Release PR Creation Experience
    section Development
      Merge feature PR: 5: Sarah
      Receive notification: 4: Sarah
      Review release PR: 5: Sarah
    section Release Decision
      Decide to release: 5: Sarah
      Review changelog: 4: Sarah
      Merge release PR: 5: Sarah
    section Release Creation
      Receive release notification: 5: Sarah
      Verify GitHub release: 5: Sarah
      Share with team: 5: Sarah
```

### Error Recovery Journey

```mermaid
journey
    title DevOps Team's Error Recovery Experience
    section Detection
      Receive alert: 3: DevOps
      Check logs: 2: DevOps
      Identify issue: 3: DevOps
    section Resolution
      Fix configuration: 4: DevOps
      Retry failed operation: 4: DevOps
      Verify success: 5: DevOps
    section Prevention
      Update monitoring: 4: DevOps
      Document solution: 4: DevOps
      Share with team: 5: DevOps
```

## Success Metrics

### Primary Metrics

- **Release PR Creation Success Rate**: >95% of merged PRs result in successful release PR creation
- **End-to-End Processing Time**: <30 seconds from webhook to PR creation/update
- **Version Calculation Accuracy**: >99% of version calculations are semantically correct
- **Error Recovery Rate**: >90% of transient failures resolve within 5 retries

### Secondary Metrics

- **User Satisfaction**: Repository maintainers report improved release workflow efficiency
- **Configuration Adoption**: >80% of repositories use custom configuration templates
- **Support Request Volume**: <5 support requests per month per 100 repositories
- **Audit Compliance**: 100% of operations have complete audit trails

## Constraints & Assumptions

### Technical Constraints

- GitHub API rate limits may affect processing speed during high-traffic periods
- Webhook delivery reliability depends on GitHub's infrastructure
- Serverless cold starts may impact initial response times
- Repository size affects commit fetching performance

### Business Constraints

- Must integrate with existing GitHub workflow without disruption
- Configuration complexity should not exceed current CI/CD setup complexity
- Error messages must be actionable by repository maintainers
- Audit trail requirements vary by organization compliance needs

### Assumptions

- Repository maintainers understand conventional commit message format
- Teams are comfortable with semantic versioning principles
- GitHub App permissions model is sufficient for required operations
- Repository configuration can be managed through standard Git workflows
