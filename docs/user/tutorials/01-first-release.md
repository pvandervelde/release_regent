---
title: Your first automated release
description: A step-by-step tutorial that takes you from zero to a published GitHub release
---

# Your first automated release

In this tutorial you will set up Release Regent from scratch and see it drive a complete
release cycle — from a merged pull request all the way to a published GitHub release. The
whole thing takes about 15 minutes.

## What you will build

By the end of this tutorial you will have:

- The `rr` CLI installed and working
- A GitHub App configured for your repository
- Release Regent processing webhook events
- A published GitHub release created automatically

## Before you start

You need:

- A GitHub account with admin access to at least one repository
- [Rust and Cargo](https://rustup.rs/) installed, **or** the ability to download a pre-built binary
- Basic familiarity with Git and the terminal

You do **not** need to know anything about conventional commits or semantic versioning before
starting — this tutorial explains everything as you go.

---

## Step 1: Install the CLI

The `rr` CLI is the tool you will use throughout this tutorial to test, configure, and simulate
Release Regent workflows.

=== "Cargo (recommended)"

    ```bash
    cargo install --git https://github.com/pvandervelde/release_regent.git release-regent-cli
    ```

=== "Pre-built binary"

    Download the binary for your platform from the
    [releases page](https://github.com/pvandervelde/release_regent/releases), extract it, and
    move it to a directory that is on your `PATH`.

Verify the installation:

```bash
rr --version
```

You should see a version string such as `rr 0.3.0`. If you see a "command not found" error,
check that the binary is on your `PATH`.

---

## Step 2: Prepare a practice repository

You need a Git repository with some history to work with. If you already have a repository you
want to use, skip ahead to [step 3](#step-3-initialise-release-regent). Otherwise, create one now.

=== "Linux/macOS"

    ```bash
    mkdir release-regent-demo
    cd release-regent-demo
    git init

    echo "# Demo project" > README.md
    git add README.md
    git commit -m "feat: initial project setup"

    echo "console.log('hello');" > app.js
    git add app.js
    git commit -m "feat: add hello world script"

    echo "# fix typo" >> README.md
    git add README.md
    git commit -m "fix: correct readme typo"

    echo "# Usage" >> README.md
    git add README.md
    git commit -m "docs: add usage section"
    ```

=== "Windows (PowerShell)"

    ```powershell
    New-Item -ItemType Directory release-regent-demo
    Set-Location release-regent-demo
    git init

    "# Demo project" | Out-File README.md -Encoding utf8
    git add README.md
    git commit -m "feat: initial project setup"

    "console.log('hello');" | Out-File app.js -Encoding utf8
    git add app.js
    git commit -m "feat: add hello world script"

    "# fix typo" | Out-File README.md -Append -Encoding utf8
    git add README.md
    git commit -m "fix: correct readme typo"

    "# Usage" | Out-File README.md -Append -Encoding utf8
    git add README.md
    git commit -m "docs: add usage section"
    ```

The commits follow the [conventional commits](../reference/conventional-commits.md) standard —
that is what Release Regent uses to calculate version bumps.

---

## Step 3: Initialise Release Regent

Inside your repository, run:

```bash
rr init
```

This creates two files:

- `.release-regent.toml` — your configuration file
- `sample-webhook.json` — a sample webhook payload for local testing

Open `.release-regent.toml`. You will see something like this (abbreviated — the actual file
contains all available fields with their defaults):

```toml
[core]
version_prefix = "v"

[core.branches]
main = "main"

[versioning]
strategy = "conventional"
allow_override = true

[release_pr]
title_template = "chore(release): ${version}"
draft = false
```

The defaults work for most repositories. You can leave the file as-is for now.

---

## Step 4: Test commit parsing locally

Before touching GitHub, verify that Release Regent can parse your commits correctly:

```bash
rr test --commits 4
```

You should see output similar to this:

```
Analysing 4 commits...

=== Parsed commits ===
  feat: initial project setup       → feat   (minor bump)
  feat: add hello world script      → feat   (minor bump)
  fix: correct readme typo          → fix    (patch bump)
  docs: add usage section           → docs   (no bump)

=== Version calculation ===
  Current version : (none)
  Next version    : 0.1.0  (initial release, minor changes present)

=== Generated changelog ===
## [0.1.0]

### Features
- Initial project setup
- Add hello world script

### Bug Fixes
- Correct readme typo
```

If any commits show as "unparsed", check that they follow the `type: description` format.
See [conventional commits](../reference/conventional-commits.md) for the full syntax.

---

## Step 5: Set up the GitHub App

Release Regent authenticates to GitHub using a GitHub App. You need to create one.

!!! note
    This step requires admin access to your GitHub account or organisation. If you are working
    inside an organisation where you cannot create GitHub Apps, ask your GitHub admin to carry
    out steps 5.1–5.3 and share the App ID and private key with you.

### 5.1 Create the app

1. Go to **GitHub → Settings → Developer settings → GitHub Apps → New GitHub App**
2. Fill in the basic details:
   - **App name**: `Release Regent - <your name>` (must be globally unique)
   - **Homepage URL**: `https://github.com/pvandervelde/release_regent`
   - **Webhook URL**: leave this blank for now; you will fill it in when you deploy the server
   - **Webhook secret**: generate a random string and copy it somewhere safe:

     ```bash
     openssl rand -hex 32
     ```

3. Under **Repository permissions**, set:

   | Permission | Access |
   | :--- | :--- |
   | Contents | Read & write |
   | Metadata | Read |
   | Pull requests | Read & write |
   | Issues | Read |

4. Under **Subscribe to events**, tick **Pull request**.
5. Set "Where can this GitHub App be installed?" to **Only on this account**.
6. Click **Create GitHub App**.

### 5.2 Generate a private key

1. On the app settings page, scroll to **Private keys** and click **Generate a private key**.
2. A `.pem` file downloads automatically. Store it somewhere secure — you cannot download it
   again.
3. Note the **App ID** shown at the top of the page (a six- or seven-digit number).

### 5.3 Install the app on your repository

1. Go to **Install App** in the app's sidebar.
2. Click **Install** next to your account.
3. Choose **Only select repositories**, pick your demo repository, and click **Install**.

---

## Step 6: Deploy the server

The `rr-server` binary runs a long-lived HTTP server that receives webhooks from GitHub.

For this tutorial, the easiest approach is to run it locally and expose it via a tunnelling
service such as [ngrok](https://ngrok.com/).

### 6.1 Build and run the server

=== "From source"

    ```bash
    cargo build --release --bin rr-server

    export GITHUB_APP_ID=<your-app-id>
    export GITHUB_PRIVATE_KEY="$(cat /path/to/private-key.pem)"
    export GITHUB_WEBHOOK_SECRET=<your-webhook-secret>

    ./target/release/rr-server
    ```

=== "Docker"

    ```bash
    docker run --rm \
      -e GITHUB_APP_ID=<your-app-id> \
      -e GITHUB_PRIVATE_KEY="$(cat /path/to/private-key.pem)" \
      -e GITHUB_WEBHOOK_SECRET=<your-webhook-secret> \
      -p 8080:8080 \
      ghcr.io/pvandervelde/release_regent:latest
    ```

The server starts on port 8080. Verify it is running:

```bash
curl http://localhost:8080/
# {"status":"healthy"}
```

### 6.2 Expose the server with ngrok

```bash
ngrok http 8080
```

ngrok prints a public URL like `https://abc123.ngrok-free.app`. Copy that URL.

### 6.3 Update the GitHub App webhook URL

1. Go back to your GitHub App settings.
2. Under **Webhook URL**, paste `https://abc123.ngrok-free.app/webhook`.
3. Click **Save changes**.

---

## Step 7: Watch Release Regent work

Now trigger the automation by creating and merging a pull request.

1. Create a new branch in your demo repository and push a conventional commit:

   ```bash
   git checkout -b feat/add-greeting
   echo "console.log('Good morning!');" >> app.js
   git add app.js
   git commit -m "feat: add morning greeting"
   git push origin feat/add-greeting
   ```

2. Open a pull request targeting `main` and merge it on GitHub.

3. Within a few seconds, Release Regent receives the webhook and creates a release PR. Open
   your repository's pull requests — you will see a new PR titled something like:

   ```
   chore(release): prepare version 0.2.0
   ```

4. Open the release PR. The body contains the generated changelog, including the new feature
   you just merged.

5. Merge the release PR.

6. Release Regent detects the merge, creates the `v0.2.0` tag, and publishes a GitHub release.
   Go to **Releases** in your repository to confirm.

---

## What just happened?

You just witnessed the full two-phase release workflow:

- **Phase 1**: merging a feature PR caused Release Regent to analyse commits, calculate a
  version bump (`feat` → minor → 0.2.0), and open a release PR.
- **Phase 2**: merging the release PR caused Release Regent to publish the GitHub release.

---

## Next steps

- Customise your changelog: [Customise your changelog tutorial](02-customise-changelog.md)
- Understand why the workflow is designed this way: [The release workflow](../explanation/release-workflow.md)
- Read all configuration options: [Configuration reference](../reference/configuration.md)
- Deploy to production: [Deploy the server](../how-to/setup/install-server.md)
