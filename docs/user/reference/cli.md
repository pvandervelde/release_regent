---
title: CLI reference
description: Complete reference for every rr command, flag, and option
---

# CLI reference

The `rr` binary provides local testing and configuration tools for Release Regent. It does not
connect to a live server — all processing happens in-process on your machine.

## Installation

See [Install the CLI](../how-to/setup/install-cli.md).

## Global options

These options are available for every command.

### `-v, --verbose`

Enable debug-level logging.

```bash
rr --verbose test --commits 5
```

### `-c, --config <PATH>`

Path to a configuration file. Defaults to `.release-regent.toml` in the current directory.

```bash
rr --config /path/to/release-regent.toml test
```

### `--version`

Print the installed version and exit.

```bash
rr --version
```

### `--help`

Print help text for the command or subcommand and exit.

```bash
rr --help
rr init --help
```

---

## `rr init`

Generate sample configuration files in your project.

```
rr init [OPTIONS]
```

### Options

| Flag | Default | Description |
| :--- | :--- | :--- |
| `-o, --output-dir <PATH>` | `.` (current directory) | Directory where generated files are written |
| `-t, --template <TYPE>` | `basic` | Template type: `basic`, `comprehensive`, or `minimal` |
| `--overwrite` | false | Overwrite existing files without prompting |

### Templates

| Template | Description |
| :--- | :--- |
| `basic` | Common options with sensible defaults — good for most projects |
| `comprehensive` | All available options with documentation comments |
| `minimal` | Only required fields — smallest possible configuration |

### Output files

| File | Description |
| :--- | :--- |
| `.release-regent.toml` | Main configuration file |
| `sample-webhook.json` | Sample pull request merged webhook payload for local testing |

### Examples

```bash
# Initialise in the current directory
rr init

# Generate a comprehensive configuration with all options shown
rr init --template comprehensive

# Generate into a subdirectory, replacing any existing files
rr init --output-dir ./config --overwrite
```

---

## `rr run`

Process a GitHub webhook event locally.

```
rr run --event-file <FILE> [OPTIONS]
```

### Options

| Flag | Default | Description |
| :--- | :--- | :--- |
| `-e, --event-file <FILE>` | (required) | Path to a JSON webhook payload file |
| `--event-type <TYPE>` | `pull_request_merged` | Internal event type (see table below) |
| `-d, --dry-run` | false | Exit immediately without processing the event or calling the GitHub API |
| `--mock` | false | Use in-process mocks instead of real GitHub credentials |
| `-c, --config-path <PATH>` | (uses global `-c`) | Configuration file path |

### Event types

| Value | When to use |
| :--- | :--- |
| `pull_request_merged` | A regular (non-release) PR was merged to the default branch |
| `release_pr_merged` | A `release/v*` PR was merged |
| `pull_request_comment_received` | A comment was posted on a PR (e.g., `!set-version`) |
| `pull_request_opened` | A PR was opened |
| `pull_request_updated` | A PR's head commit changed |

### Examples

```bash
# Process a sample event with mocks (no credentials required)
rr run --event-file sample-webhook.json --mock

# Exit without processing (confirms the event file is readable)
rr run --event-file sample-webhook.json --dry-run

# Replay a release PR merge event
rr run --event-file release-merged.json --event-type release_pr_merged

# Verbose output with a custom config file
rr --verbose run --event-file webhook.json --config-path ./config/release-regent.toml
```

### Exit codes

| Code | Meaning |
| :--- | :--- |
| `0` | Event processed successfully |
| `1` | Configuration error |
| `2` | Webhook event file not found or unreadable |
| `3` | GitHub API error (non-dry-run only) |

---

## `rr test`

Analyse commits in the current Git repository and show version calculation and changelog output.

```
rr test [OPTIONS]
```

### Options

| Flag | Default | Description |
| :--- | :--- | :--- |
| `-n, --commits <NUMBER>` | `10` | Number of commits to analyse from HEAD |
| `-f, --from <SHA>` | (HEAD) | Starting commit SHA |
| `--current-version <VERSION>` | (auto-detected from tags) | Base version for calculation |
| `-v, --verbose` | false | Show per-commit parsing detail |

### Output sections

The command prints three sections:

1. **Parsed commits** — each commit with its parsed type, scope, and bump contribution
2. **Version calculation** — current version, calculated next version, and the reason
3. **Generated changelog** — the rendered changelog using your configuration template

### Examples

```bash
# Analyse the last 10 commits
rr test

# Analyse 30 commits, starting the calculation from v1.5.0
rr test --commits 30 --current-version 1.5.0

# Show detailed per-commit parsing
rr test --verbose

# Analyse from a specific commit SHA
rr test --from abc123def456
```

---

## `rr generate`

Generate test data files.

```
rr generate [OPTIONS]
```

### Options

| Flag | Default | Description |
| :--- | :--- | :--- |
| `-o, --output-dir <PATH>` | `.` | Directory where generated files are written |
| `-k, --kind <TYPE>` | `all` | What to generate: `webhook`, `config`, or `all` |
| `--overwrite` | false | Overwrite existing files without prompting |

### Generated files

| Kind | Files created |
| :--- | :--- |
| `webhook` | `sample-webhook.json` |
| `config` | `sample-config.toml` |
| `all` | Both of the above |

### Examples

```bash
# Generate all test files in the current directory
rr generate

# Generate only a sample webhook in a specific directory
rr generate --kind webhook --output-dir ./test-fixtures

# Regenerate, overwriting existing files
rr generate --overwrite
```
