---
title: Troubleshooting
description: Diagnosing common problems with webhooks, authentication, version calculation, and release PR creation
---

# Troubleshooting

This guide covers the most common problems encountered when setting up and operating Release
Regent.

## Webhooks are not being received

**Symptom**: PRs are merged but no release PR is created and the server logs show no activity.

**Check 1 — Webhook deliveries in GitHub**

Go to **GitHub App settings → Advanced → Recent Deliveries**. If deliveries show a non-`200`
response code or a timeout, the server is not reachable from GitHub.

- Confirm the server is running: `curl http://your-server:8080/` should return
  `{"status":"healthy"}`.
- Confirm the webhook URL in the GitHub App settings includes the `/webhook` path:
  `https://your-server.example.com/webhook`.
- Check that your network/firewall allows inbound HTTPS on the configured port.

**Check 2 — Wrong event types subscribed**

In GitHub App settings → **Permissions & events**, confirm **Pull request** is checked under
webhooks. Without it, GitHub will not send pull request events.

**Check 3 — Repository not allowed**

If `ALLOWED_REPOS` is set, confirm the repository is listed. Events from repositories not in
`ALLOWED_REPOS` are silently rejected with `403 Forbidden`.

---

## Webhook signature validation failures

**Symptom**: Server logs show `401 Unauthorized` or `signature mismatch` errors.

- Confirm the `GITHUB_WEBHOOK_SECRET` environment variable matches the secret configured in
  the GitHub App settings exactly (no extra spaces or newlines).
- If you recently rotated the webhook secret in GitHub, restart the server so it picks up the
  new value.
- Confirm the webhook content type is set to `application/json` in GitHub App settings. Other
  content types alter the payload format and invalidate the signature.

---

## Authentication failures (GitHub API calls failing)

**Symptom**: Logs contain errors like `401 Bad credentials`, `invalid JWT`, or
`installation not found`.

- Confirm `GITHUB_APP_ID` is the numeric ID shown at the top of your GitHub App settings page.
- Confirm `GITHUB_PRIVATE_KEY` contains the full PEM file, including the `-----BEGIN` and
  `-----END` lines, with proper line breaks. Multi-line values in environment variables can
  lose newlines depending on how they are injected.
- Confirm the GitHub App is installed on the repository that sent the event. Go to
  **App settings → Install App** to check.
- Check that the private key has not expired or been revoked. Under **Private keys**, an
  expired key shows a warning.

---

## No release PR is created after a PR merge

**Symptom**: The webhook was received and acknowledged (200 OK) but no release PR appeared.

**Check 1 — All commits are non-bumping**

If every commit since the last release is a `docs:`, `chore:`, `style:`, or other
non-version-bumping type, Release Regent intentionally skips release PR creation. Run
`rr test --commits 20` to see which commits were analysed and what version bump (if any) was
calculated.

**Check 2 — Configuration error**

Check logs for `WARN` entries about `.release-regent.toml`. A configuration parse error can
cause Release Regent to fall back to defaults or skip processing. Run
`rr run --event-file sample-webhook.json --dry-run` locally to surface config errors.

**Check 3 — Merge was to a non-default branch**

Release Regent only processes merges into the repository's default branch (`main` or
`master`). Merges to other branches are ignored.

---

## Release PR has the wrong version

**Symptom**: The release PR was created at a version that seems wrong (too low, too high, or
at an unexpected increment).

Run `rr test --commits 30 --current-version <current-tag>` to see exactly which commits were
analysed and which bump was applied. Cross-check the output against the
[conventional commits version rules](../reference/conventional-commits.md).

To correct the version on an already-open release PR, use the
[`!set-version` command](../reference/pr-commands.md):

```
!set-version 2.0.0
```

---

## Release was not published after merging the release PR

**Symptom**: The release PR is merged and closed but no GitHub release appears.

**Check 1 — Branch naming mismatch**

Release Regent identifies release PRs by the `release/v*` branch pattern. If the release PR
branch was renamed or does not match this pattern, Release Regent does not recognise the merge
as a release trigger. Check the branch name of the merged PR.

**Check 2 — Tag already exists**

If the Git tag for this version was already created manually, GitHub rejects the duplicate and
the release fails. Check `git tag --list` and remove any conflicting tag before redelivering
the event.

**Check 3 — Insufficient permissions**

Confirm the GitHub App has **Contents: Read & write** permission. Without write access to
contents, it cannot create tags or releases.

---

## `rr` CLI command is not found

Ensure the binary directory is on your `PATH`:

```bash
# Cargo install default
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

---

## Getting more detail from the server

Set the log level to `debug` to see every GitHub API call, event parse step, and version
calculation decision:

```bash
RUST_LOG=debug rr-server
```

Or for Release Regent modules only (less noisy):

```bash
RUST_LOG=release_regent=debug rr-server
```

---

## Still stuck?

Open an issue on [GitHub](https://github.com/pvandervelde/release_regent/issues) with:

- The server log output (redact your private key and webhook secret)
- The GitHub App settings (Permissions and Events tab)
- The `.release-regent.toml` content
- The output of `rr test --commits 20`
