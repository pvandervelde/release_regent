---
title: Local testing samples
description: Scripts and configuration for running Release Regent locally with Docker, Smee, and a generated test repository.
---

# Local testing samples

This directory contains scripts and configuration for running and testing Release Regent
locally without a public server.

## Scripts

| Script | Purpose |
| :----- | :------ |
| [run-local.ps1](run-local.ps1) | Start Release Regent in Docker and proxy GitHub webhooks via Smee |
| [create-test-repo.ps1](create-test-repo.ps1) | Create a disposable GitHub test repository with branches ready to merge |

## End-to-end testing workflow

The two scripts work together for a complete local test loop:

```text
create-test-repo.ps1
  └── Creates a GitHub repo with pre-built branches and a release-regent.toml

run-local.ps1
  └── Starts rr-server in Docker + smee proxy

You merge a branch (PR) in the test repo
  └── GitHub fires webhook → smee → rr-server → Release Regent logic runs
```

See the individual sections below for detailed usage of each script.

---

## run-local.ps1 — Docker + Smee proxy

Runs Release Regent locally in Docker and forwards GitHub App webhook events
via [Smee.io](https://smee.io).

## How it works

```text
GitHub App webhook
       │
       ▼
https://smee.io/YOUR_CHANNEL   (public relay, created automatically)
       │
       ▼ smee-client (runs locally via npx)
       │
       ▼
http://localhost:PORT/webhook
       │
       ▼
Docker container  →  rr-server
```

## Prerequisites

| Requirement | Notes |
| :---------- | :---- |
| [Docker Desktop](https://www.docker.com/products/docker-desktop/) | Engine must be running |
| [Node.js](https://nodejs.org/) | Provides `npx`, used to run the smee client |
| GitHub App | See [GitHub App setup guide](../docs/github-app-setup.md) |

## Quick start

### 1. Build the Docker image

From the repository root:

```powershell
docker build --tag release-regent:local .
```

Or let the script build it for you by passing `-Build` (step 4).

### 2. Create your .env file

```powershell
Copy-Item samples\.env.example samples\.env
```

Open `samples\.env` and fill in the four required values:

| Variable | Where to find it |
| :------- | :--------------- |
| `GITHUB_APP_ID` | GitHub Settings → Developer settings → GitHub Apps → your app |
| `GITHUB_INSTALLATION_ID` | URL after installing the app: `github.com/settings/installations/XXXXXXXX` |
| `GITHUB_WEBHOOK_SECRET` | The secret you entered when configuring the app's webhook |
| `GITHUB_PRIVATE_KEY_FILE` | Path to the `.pem` file downloaded from the app's *Private keys* section |

### 3. Update the sample config (optional)

Edit `samples/config/release-regent.toml` and set `repository.remote_url` to
the GitHub repository you want to test against. All other settings have
working defaults.

### 4. Start the stack

```powershell
# Use a pre-built image and auto-create a Smee channel
.\samples\run-local.ps1

# Build the image first, then start
.\samples\run-local.ps1 -Build

# Use a specific Smee channel from a previous session
.\samples\run-local.ps1 -SmeeUrl https://smee.io/abc123
```

The script prints the Smee channel URL when it starts:

```text
  Configure your GitHub App webhook URL to:
    https://smee.io/abc123
```

### 5. Point your GitHub App at the Smee channel

1. Open your GitHub App settings.
2. Under **Webhook URL**, enter the Smee channel URL printed by the script.
3. Save the changes.

GitHub will now deliver all webhook events to your local container.

### 6. Trigger a test event

Merge a pull request in a repository where the app is installed, or use the
**Redeliver** button in the GitHub App's *Recent deliveries* tab to replay an
existing event.

The script streams both server logs and smee proxy output to the console:

```text
[smee]   POST http://localhost:8080/webhook - 200
[server] 2026-04-18T12:00:00Z  INFO release_regent_core: processing pull_request event
```

### 7. Stop the stack

Press **Ctrl+C**. The script stops the smee proxy, stops the container, and
removes it automatically.

## Script parameters

| Parameter | Default | Description |
| :-------- | :------ | :---------- |
| `-SmeeUrl` | auto-created | Smee.io channel URL. Reuse across sessions to keep the same webhook URL in your GitHub App. |
| `-EnvFile` | `samples\.env` | Path to the `.env` file with credentials. |
| `-PrivateKeyFile` | from `.env` | Overrides `GITHUB_PRIVATE_KEY_FILE` in the `.env` file. |
| `-ConfigDir` | `samples\config` | Directory containing `release-regent.toml`, mounted read-only at `/config` inside the container. |
| `-ImageName` | `release-regent:local` | Docker image to start. |
| `-Port` | `8080` | Host port mapped to the container. Change if 8080 is already in use. |
| `-Build` | off | (Re)build the Docker image from source before starting. |

## Troubleshooting

### Container exits immediately

Run the following to inspect the last log lines:

```powershell
docker logs release-regent-local
```

Common causes:

- A required environment variable is missing or empty in `.env`.
- `GITHUB_APP_ID` or `GITHUB_INSTALLATION_ID` is not a number.
- The private key file does not contain a valid PEM-encoded key.

### Webhooks are not arriving

- Confirm the Smee channel URL in the script output matches the one set in
  the GitHub App webhook settings.
- Check the **Recent deliveries** tab in the GitHub App for delivery errors.
- Ensure the container's health endpoint responds: `http://localhost:8080/health`.

### Port already in use

Pass a different port with `-Port`:

```powershell
.\samples\run-local.ps1 -Port 9090
```

Then update the smee target accordingly — the script handles this automatically.

### Smee channel expires

Smee channels do not expire while the proxy is connected. If you restart the
script without `-SmeeUrl`, a new channel is created and you will need to update
the GitHub App webhook URL again. Pass `-SmeeUrl https://smee.io/abc123` to
reuse an existing channel across sessions.

---

## create-test-repo.ps1 — Test repository generator

Creates a disposable GitHub repository pre-loaded with conventional commits and
branches that exercise every Release Regent code path.

### Prerequisites

| Requirement | Notes |
| :---------- | :---- |
| [gh CLI](https://cli.github.com/) | Must be authenticated (`gh auth login`) |
| [Git](https://git-scm.com/) | Must be on PATH |
| GitHub account | Repository is created under the authenticated user/org |

### Quick start

```powershell
# Create a private repo with a random suffix (avoids name collisions)
.\samples\create-test-repo.ps1 -RandomSuffix

# Also open a draft PR for every branch
.\samples\create-test-repo.ps1 -RandomSuffix -CreatePRs

# Public repo, cloned into a specific directory
.\samples\create-test-repo.ps1 -Visibility public -WorkDir C:\dev\scratch -CreatePRs
```

The script prints a summary and step-by-step instructions when it finishes:

```text
  Repository : https://github.com/you/rr-test-a3f9
  Local clone: C:\Users\you\AppData\Local\Temp\rr-test-a3f9

  Next steps
  ──────────
  1. Install your Release Regent GitHub App on this repository
  2. Start Release Regent locally with run-local.ps1
  3. Merge branches in this order...
```

### What is created

The script creates six branches off the tagged `v0.1.0` baseline on `main`:

| Branch | Commit type | Expected Release Regent outcome |
| :----- | :---------- | :------------------------------ |
| `fix/handle-empty-input` | `fix:` | `release/v0.1.1` PR created |
| `feat/add-greeting-styles` | `feat:` | `release/v0.2.0` PR created, replaces v0.1.1 |
| `feat/add-language-support` | `feat:` | `release/v0.2.0` changelog updated only |
| `docs/update-api-docs` | `docs:` | No version bump |
| `chore/update-ci` | `chore:` | No version bump |
| `feat/breaking-rename-endpoint` | `feat!:` + `BREAKING CHANGE:` | `release/v1.0.0` PR created |

Merge the `release/v0.2.0` PR between steps 5 and 6 to trigger GitHub release creation.

### Script parameters

| Parameter | Default | Description |
| :-------- | :------ | :---------- |
| `-RepoName` | `rr-test` | Base name for the GitHub repository. |
| `-Owner` | authenticated user | GitHub user or organisation. |
| `-Visibility` | `private` | `private`, `public`, or `internal`. |
| `-WorkDir` | `$env:TEMP` | Parent directory for the local clone. |
| `-RandomSuffix` | off | Append a 4-char hex suffix to avoid naming conflicts. |
| `-CreatePRs` | off | Open a draft PR on GitHub for each branch. |
| `-SkipTagV0` | off | Skip the `v0.1.0` baseline tag (Release Regent uses `initial_version`). |

### Cleanup

```powershell
gh repo delete owner/rr-test-a3f9 --yes
Remove-Item -Recurse -Force "$env:TEMP\rr-test-a3f9"
```

---

## File reference

```text
samples/
  run-local.ps1              Docker + Smee orchestration script
  create-test-repo.ps1       Test repository generator
  .env.example               Credential template (copy to .env and fill in values)
  config/
    release-regent.toml      Sample server configuration mounted into Docker
  README.md                  This file
```
