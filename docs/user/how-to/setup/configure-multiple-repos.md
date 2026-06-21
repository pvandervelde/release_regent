---
title: Configure multiple repositories
description: How to manage several GitHub repositories with a single Release Regent server
---

# Configure multiple repositories

A single Release Regent server instance can manage webhooks from multiple repositories. This
page explains how to control which repositories the server accepts events from and how to
provide per-repository configuration.

## Allow specific repositories

By default the server accepts events from any repository that has the GitHub App installed.
To restrict it to a specific set, set the `ALLOWED_REPOS` environment variable:

```bash
ALLOWED_REPOS=myorg/backend,myorg/frontend,myorg/docs
```

Events from repositories not in this list are rejected with `403 Forbidden`. This is the
recommended setting for production deployments.

To restore the default (accept any installed repository), set `ALLOWED_REPOS=*` or leave the
variable unset.

## Install the GitHub App on additional repositories

1. Go to **GitHub → Settings → Installations → \<your app\> → Configure**.
2. Under **Repository access**, add the new repository.
3. Click **Save**.

Release Regent starts processing events from the new repository immediately, as long as it is
in `ALLOWED_REPOS` (or `ALLOWED_REPOS` is `*`).

## Per-repository configuration

Release Regent reads configuration from a `.release-regent.toml` file at the root of each
repository. Each repository controls its own versioning rules, changelog format, and PR
templates for any settings that have not been locked by a higher level.

The server finds the app-level baseline configuration using the `CONFIG_DIR` environment
variable. If `CONFIG_DIR` is unset, the server looks in the current working directory.

## Organisation-wide policy (metadata repository)

For larger deployments you can enforce settings across every repository in your organisation
without modifying individual dotfiles. Create a repository named `.release-regent` in your
GitHub organisation and add a `global.toml` file. Release Regent auto-discovers this
repository and merges its settings over the app-level config for every event in the org.

You can also apply per-group defaults by adding files under `groups/` and having repositories
declare `group = "name"` in their dotfiles.

!!! tip
    See [Set up the metadata repository](metadata-repository.md) for step-by-step
    instructions and [Configuration hierarchy](../../explanation/configuration-hierarchy.md)
    for the full mental model.

## Webhook routing

The server uses the `installation.id` field present in every GitHub App webhook payload to
identify which GitHub installation credentials to use for each event. You do not need to
configure this — it is automatic.

---

## Next steps

- [Environment variables reference](../../reference/environment-variables.md) — full list of
  server settings
- [Configuration file reference](../../reference/configuration.md)
