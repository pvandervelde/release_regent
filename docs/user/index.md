---
title: Release Regent
description: Automated GitHub release management with semantic versioning and changelog generation
---

# Release Regent

Release Regent automates the mechanical parts of the GitHub release process — version bumping,
changelog writing, tag creation — while leaving the decision of *when* to release firmly with
your team.

## What it does

When you merge a pull request to your main branch, Release Regent:

1. Analyses commits since the last release using [conventional commit](reference/conventional-commits.md)
   conventions
2. Calculates the correct semantic version (major, minor, or patch)
3. Opens a **release PR** with a generated changelog and updated manifest files

When you merge that release PR, Release Regent:

1. Creates a Git tag pointing to the merge commit
2. Publishes a GitHub release with the accumulated changelog as release notes
3. Cleans up the release branch

The two-phase design means your team always has the chance to review the calculated version and
changelog before a release is published. See
[The release workflow](explanation/release-workflow.md) for the reasoning behind this approach.

## Where to start

=== "I'm new here"
    Work through the [first-release tutorial](tutorials/01-first-release.md). It takes about
    15 minutes and walks you through every step from installation to your first published release.

=== "I want to set something up"
    Jump straight to a how-to guide:

    - [Install the CLI](how-to/setup/install-cli.md)
    - [Deploy the server](how-to/setup/install-server.md)
    - [Set up the GitHub App](how-to/setup/github-app-setup.md)

=== "I need to look something up"
    Go to the reference section:

    - [CLI commands](reference/cli.md)
    - [Configuration file](reference/configuration.md)
    - [Environment variables](reference/environment-variables.md)
    - [PR comment commands](reference/pr-commands.md)

=== "I want to understand how it works"
    Read the explanation articles:

    - [The release workflow](explanation/release-workflow.md)
    - [How versions are calculated](explanation/version-calculation.md)
    - [GitHub App authentication](explanation/github-app-model.md)

## Deployment modes

Release Regent ships two binaries:

| Binary | Purpose |
| :--- | :--- |
| `rr` | CLI for local testing, simulating webhooks, and analysing git history |
| `rr-server` | Long-running HTTP server that receives live GitHub webhooks |

Both binaries share the same core release logic. The server is the production deployment target;
the CLI is for development and debugging.
