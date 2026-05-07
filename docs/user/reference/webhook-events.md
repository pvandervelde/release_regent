---
title: Webhook events
description: GitHub webhook events supported by Release Regent and their processing behaviour
---

# Webhook events

Release Regent processes a subset of GitHub webhook events. This page describes which events
are handled, how they are validated, and which processing path each one triggers.

## Security: signature validation

Every incoming webhook request is validated before any processing occurs:

1. GitHub includes an `X-Hub-Signature-256` header containing an HMAC-SHA256 signature of
   the request body, computed using the webhook secret.
2. Release Regent recomputes the HMAC using `GITHUB_WEBHOOK_SECRET` and compares the result to
   the header using a constant-time comparison to prevent timing attacks.
3. Requests with missing, malformed, or mismatched signatures are rejected with `401
   Unauthorized`. No release logic runs.

## Supported events

### `pull_request` — action: `closed`, `merged: true`

**Trigger**: A pull request is merged into the repository's default branch.

**What Release Regent does**:

- Determines whether this is a regular feature PR or a release PR (by branch name pattern
  `release/v*`).
- **Regular PR**: Analyses commits since the last release, calculates the next semantic
  version, and creates or updates a release PR.
- **Release PR**: Creates the GitHub release, tags the merge commit, and deletes the release
  branch.

**Response**: `200 OK` with JSON body `{"status": "processing"}`.

### `pull_request` — action: `created` (comment)

**Trigger**: A comment is posted on a pull request.

**What Release Regent does**:

- Checks whether the comment body starts with `!set-version` or `!release`.
- If yes, and the posting user has write access, processes the
  [PR comment command](pr-commands.md).
- If no recognised command is found, ignores the event.

**Response**: `200 OK`.

### All other events

All other webhook event types are accepted (return `200 OK`) but not processed. This prevents
GitHub from treating unhandled events as failures and retrying them.

## Error responses

| HTTP status | Cause |
| :--- | :--- |
| `200 OK` | Event processed successfully, or event type not handled (intentional no-op) |
| `400 Bad Request` | Malformed JSON payload or missing required fields |
| `401 Unauthorized` | Webhook signature missing or invalid |
| `403 Forbidden` | Repository not in `ALLOWED_REPOS` |
| `413 Payload Too Large` | Request body exceeds 10 MiB |
| `500 Internal Server Error` | Unexpected error during GitHub API calls or release processing |

## Delivery timeout

GitHub considers a webhook delivery failed if the server does not respond within 10 seconds.
Release Regent acknowledges the HTTP request immediately and processes the event
asynchronously, so it always responds within the timeout window regardless of how long the
release workflow takes.

Failed deliveries can be redelivered from the GitHub App settings:
**App settings → Advanced → Recent Deliveries → Redeliver**.
