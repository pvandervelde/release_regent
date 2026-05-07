---
title: How-to guides
description: Practical recipes for common tasks with Release Regent
---

# How-to guides

How-to guides are concise, task-focused recipes. Each one tells you how to accomplish a
specific goal — they assume you already understand the basics.

If you are new to Release Regent, start with the
[tutorials](../tutorials/index.md) first.

## Set up

- [Install the CLI](setup/install-cli.md) — get the `rr` binary onto your machine
- [Deploy the server](setup/install-server.md) — run the webhook server with Docker, Docker
  Compose, or Kubernetes
- [Set up the GitHub App](setup/github-app-setup.md) — create and configure the GitHub App
  that Release Regent uses to authenticate
- [Configure multiple repositories](setup/configure-multiple-repos.md) — manage several
  repositories with a single server instance

## Manage releases

- [Override the version](releases/override-version.md) — use PR comments to set an exact
  version or raise the version floor
- [Trigger a release manually](releases/trigger-release-manually.md) — use the CLI to replay
  a webhook event without waiting for a real GitHub event
- [Recover from a failed release](releases/recover-failed-release.md) — what to do when
  something goes wrong mid-release

## Configure

- [Update manifest files](configuration/update-manifest-files.md) — keep `Cargo.toml`,
  `package.json`, and other version files in sync with each release
- [Use pre-release versions](configuration/pre-release-versions.md) — ship beta and release
  candidate versions
- [Customise changelog templates](configuration/custom-changelog-template.md) — full Tera
  template reference for changelog bodies

## Troubleshooting

- [Troubleshooting](troubleshooting.md) — diagnose common problems with webhooks, authentication,
  version calculation, and release PR creation
