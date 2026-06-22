---
title: Set up the metadata repository
description: How platform teams create and maintain the per-org metadata repository to
  enforce organisation-wide Release Regent policy
---

# Set up the metadata repository

The metadata repository is a GitHub repository named `.release-regent` within your GitHub
organisation. Platform teams use it to enforce organisation-wide policy and group-level
defaults without modifying individual repository dotfiles.

This guide is for **platform or DevOps teams**. Individual repository owners do not need to
read this page.

## Prerequisites

- A GitHub organisation (or personal account) with at least one repository already using
  Release Regent.
- Admin access to the organisation to install GitHub Apps.
- The Release Regent GitHub App installed at the organisation level (see
  [Set up the GitHub App](github-app-setup.md)).

!!! important "Organisation-level App installation required"
    The GitHub App must be installed at the **organisation level** — not scoped to individual
    repositories. A repository-scoped installation cannot read the metadata repository from
    the context of other repositories' events and will cause `403` errors when fetching policy
    files.

---

## Step 1 — Create the repository

Create a new GitHub repository named exactly `.release-regent` in your organisation:

1. Go to **GitHub → Your organisation → Repositories → New**.
2. Set the repository name to `.release-regent`.
3. Choose **Private** (recommended — policy files may contain sensitive settings).
4. Do **not** initialise with a README; you will add files manually.
5. Click **Create repository**.

---

## Step 2 — Install the GitHub App on the metadata repository

1. Go to **GitHub → Settings → Installations → \<your app\> → Configure**.
2. Under **Repository access**, ensure **All repositories** is selected.
3. Click **Save**.

!!! warning "Do not use a repository-scoped installation"
    Do not restrict the installation to only the `.release-regent` repository. Release Regent
    uses the metadata-repo installation to fetch dotfiles from *other* repositories in the
    org. A repository-scoped installation will return `403` for those fetches.

Release Regent discovers the App installation automatically when it processes the first event
for a repository in your organisation.

---

## Step 3 — Add global policy

Create a `global.toml` file at the root of the metadata repository. This file applies to
every repository in your organisation.

```toml
# myorg/.release-regent/global.toml

# Prevent any repository from switching to an external versioning script.
locked_fields = ["versioning.strategy"]

[versioning]
strategy = "conventional"
```

Commit and push to the default branch.

!!! tip "Start with no locks"
    It is safer to start with an unlocked `global.toml` that sets default values only, then
    add `locked_fields` once you are confident in the policy. An invalid `global.toml` causes
    a hard failure for **all events** in your organisation until it is corrected.

---

## Step 4 — Add group policies (optional)

If you want different defaults for subsets of repositories, create group policy files under
`groups/`.

```
myorg/.release-regent/
  global.toml
  groups/
    backend.toml
    mobile.toml
```

Example group file:

```toml
# myorg/.release-regent/groups/backend.toml

# Lock draft so that backend services never accidentally publish non-draft releases.
locked_fields = ["releases.draft"]

[releases]
draft = true

[release_pr]
title_template = "chore(release): ${version} [backend]"
```

Repositories opt in to a group by adding a `group` field to their own dotfile:

```toml
# myorg/platform-api/release-regent.toml
group = "backend"

[versioning]
excluded_pr_authors = ["dependabot[bot]"]
```

---

## Step 5 — Verify the configuration is picked up

After pushing the policy files, trigger a Release Regent event in one of the managed
repositories (for example, open a pull request). Check the server logs for entries such as:

```
INFO  release_regent::config: loaded global policy from myorg/.release-regent/global.toml
INFO  release_regent::config: loaded group policy from myorg/.release-regent/groups/backend.toml
```

If the metadata repository is unreachable or the App is not installed correctly, you will see
a `WARN` entry and the global and group levels will be skipped.

---

## Managing policy files over time

### Branch protection and CODEOWNERS

Treat the metadata repository like any other production configuration repository:

- Enable branch protection on the default branch (require PR reviews, status checks).
- Add a `CODEOWNERS` file to require review from the platform team for all policy changes.

```
# myorg/.release-regent/.github/CODEOWNERS
* @myorg/platform-team
```

Release Regent does not validate who made changes to the metadata repository — that
responsibility belongs to branch protection rules and CODEOWNERS.

### Cache TTL considerations

Release Regent caches policy files to reduce API calls:

| Level | TTL |
| :--- | :--- |
| Global policy | 10 minutes |
| Group policy | 5 minutes |

After pushing a policy change, allow up to one TTL window before all running server instances
pick up the new values. A server restart clears all caches immediately.

### Error handling for invalid files

If a policy file is pushed with a TOML syntax error or an invalid field value:

- **`global.toml` invalid** — all events for every repository in the organisation hard-fail
  until the file is corrected. Fix the file and push the correction; the cache entry is
  evicted on the next event attempt.
- **Group file invalid** — events for repositories in that group hard-fail. Repositories
  not in the group are unaffected.

---

## Repository layout reference

```
{org}/.release-regent/
  global.toml                     ← org-wide defaults and locks
  groups/
    {group-name}.toml             ← per-group defaults and locks
```

Both files use the same TOML schema as a repository dotfile, with two additions:

| Field | Valid in | Description |
| :--- | :--- | :--- |
| `locked_fields` | `global.toml`, group files | List of dotted field paths that lower levels cannot override |
| `group` | Repository dotfile only | Declares which group policy applies to this repository |

---

## Next steps

- [Configuration hierarchy explained](../../explanation/configuration-hierarchy.md) — detailed
  mental model of the five levels, locking rules, and caching
- [Configuration file reference](../../reference/configuration.md) — all settings available in
  a repository dotfile, including `group` and `locked_fields`
