---
title: Trigger a release manually
description: How to use the CLI to replay a webhook event without a live GitHub event
---

# Trigger a release manually

The `rr run` command lets you feed a webhook event file into Release Regent's processing
pipeline without waiting for a real GitHub event. This is useful for:

- Testing a new configuration before it goes live
- Replaying a missed event (e.g., the server was down when a PR was merged)
- Developing and debugging release logic locally

## Generate a sample webhook file

If you do not have a webhook file to hand, generate one:

```bash
rr generate --kind webhook --output-dir ./test-events
```

This creates a `test-events/sample-webhook.json` that represents a merged pull request.

You can also use `rr init` which creates a `sample-webhook.json` alongside the configuration
file.

## Run against a local repository (mock mode)

In `--mock` mode, Release Regent uses in-process mocks instead of real GitHub API calls. No
credentials are required:

```bash
rr run --event-file ./test-events/sample-webhook.json --mock
```

Use this to verify that your configuration parses the event correctly and generates the right
version and changelog, without creating any real PRs.

## Run in dry-run mode

Dry-run mode connects to the real GitHub API but does not write anything — no PRs are created,
no branches are modified:

```bash
rr run \
  --event-file ./test-events/sample-webhook.json \
  --dry-run \
  --config-path .release-regent.toml
```

This requires `GITHUB_APP_ID`, `GITHUB_PRIVATE_KEY`, and `GITHUB_WEBHOOK_SECRET` to be set in
your environment (the same variables as the server).

## Run a specific event type

The `--event-type` flag tells Release Regent which processing path to take. The valid values
are:

| Value | When to use |
| :--- | :--- |
| `pull_request_merged` | A regular feature PR was merged (default) |
| `release_pr_merged` | A release PR was merged |
| `pull_request_comment_received` | A PR comment was posted (e.g., `!set-version`) |
| `pull_request_opened` | A PR was opened |
| `pull_request_updated` | A PR was updated |

Example — replay a release PR merge:

```bash
rr run \
  --event-file ./events/release-pr-merged.json \
  --event-type release_pr_merged
```

## Replay a real event from GitHub logs

GitHub records every webhook delivery in the app settings:

1. Go to **GitHub → Settings → Developer settings → GitHub Apps → \<your app\> → Advanced**.
2. Find the failed or missed delivery.
3. Click **Redeliver** to replay it directly to your live server, or click the delivery to
   copy the payload and save it as a JSON file for local testing.

---

## Next steps

- [`rr run` command reference](../../reference/cli.md#rr-run)
- [Recover from a failed release](recover-failed-release.md)
