---
title: Explanation
description: Background reading that helps you understand how and why Release Regent works
---

# Explanation

Explanation articles provide background understanding. They are not instructions — they are
reading material that helps you form a mental model of the system so that you can use it more
effectively and make better decisions.

## Articles in this section

- [The release workflow](release-workflow.md) — the two-phase approach and why it is designed
  that way
- [How versions are calculated](version-calculation.md) — the logic behind semantic version
  bumps, label-based overrides, and the non-downgrade guarantee
- [GitHub App authentication](github-app-model.md) — why Release Regent uses a GitHub App, how
  JWT-based authentication works, and what the required permissions allow
- [Release branch ownership](release-branch-ownership.md) — why Release Regent takes exclusive
  ownership of `release/v*` branches and what that means for your workflow
- [Architecture](architecture.md) — how the crates fit together and what deployment shapes are
  possible
- [Configuration hierarchy](configuration-hierarchy.md) — how the five configuration levels
  merge, how the per-org metadata repository works, and how platform teams enforce policy
