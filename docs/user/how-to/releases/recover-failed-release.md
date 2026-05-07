---
title: Recover from a failed release
description: What to do when Release Regent fails mid-release and how to resume or clean up
---

# Recover from a failed release

Release Regent is designed to handle transient failures gracefully, but sometimes a release
gets stuck in a partial state — for example, a branch was created but the PR was never opened,
or a PR was merged but the GitHub release was not created. This guide explains how to diagnose
and recover from each failure scenario.

## Check the server logs first

Before taking manual action, look at the server logs. The log entries include structured context
that identifies exactly where processing stopped:

```bash
# Docker
docker logs release-regent --tail 100

# Kubernetes
kubectl logs deployment/release-regent --tail 100
```

Look for log lines at `ERROR` or `WARN` level that contain `repository`, `pull_request`, or
`release` fields. These tell you what operation failed and why.

## Scenario 1: Release PR was not created

**Symptom**: A feature PR was merged but no release PR appeared.

**Diagnose**:

1. Check server logs for errors around the time of the merge.
2. Verify the server received the webhook: in GitHub App settings, check
   **Advanced → Recent deliveries**.
3. Run the event locally to see what happens:

   ```bash
   rr run --event-file ./missed-event.json --dry-run
   ```

**Fix**:

- If the webhook was never delivered, use GitHub's **Redeliver** button.
- If the webhook was delivered but processing failed (e.g., GitHub API rate limit), wait and
  redeliver.
- If the configuration caused the failure (e.g., a syntax error in `.release-regent.toml`),
  fix the config and redeliver.

## Scenario 2: Release PR branch exists but the PR is missing

**Symptom**: A `release/v*` branch exists but there is no open pull request for it.

**Fix**:

Delete the orphaned branch and redeliver the original webhook event:

```bash
git push origin --delete release/v1.2.3
```

Then redeliver the original feature PR merged event from GitHub App settings. Release Regent
will recreate the release branch and open a new PR.

## Scenario 3: Release PR was merged but no GitHub release was created

**Symptom**: The `release/v*` branch is gone (or the PR is closed), but no GitHub release
appears under the repository's **Releases** section.

**Diagnose**: Check logs for errors that occurred after the release PR merge was received.

**Fix**:

If the `v*` Git tag does not exist yet, Release Regent can recreate the release if you
redeliver the release PR merged event from GitHub App settings.

If the tag *does* exist but the GitHub release is absent, you can create the release manually:

1. Open the repository on GitHub.
2. Go to **Releases → Draft a new release**.
3. Choose the existing tag (e.g., `v1.2.3`).
4. Paste the changelog from the release PR body as the release description.
5. Publish the release.

## Scenario 4: Wrong version was released

**Symptom**: A GitHub release was published with the wrong version number.

There is no automated rollback for published GitHub releases. The safest approach is:

1. **Do not delete the published release** unless it contained broken code — deleting releases
   confuses package managers and downstream users who have already consumed it.
2. Create a corrected release at the right version:
   - If the intended version is higher, merge any pending change (even an empty commit with
     `chore: bump version`) to trigger a new release PR and use
     [`!set-version`](../../reference/pr-commands.md) to set the correct version.
   - If the intended version is lower, use `!set-version` on the release PR before it is
     merged.
3. If the published release really must be removed (e.g., it published credentials by
   accident), delete it on GitHub and delete the associated tag, then create the correct
   release manually as described above.

## General tips

- **Idempotency**: Most Release Regent operations are safe to retry. Creating a release PR
  when one already exists at a lower version causes an upgrade; at the same version, it
  updates the changelog. Only the GitHub release creation step is not idempotent because GitHub
  does not allow duplicate tags.
- **Rate limits**: If you see `rate limit` errors in logs, wait for the limit to reset (GitHub
  shows the reset time in the API response headers) and redeliver the event.
- **Check branch protection**: If branch protection rules require status checks before merging,
  confirm those checks passed. Release Regent cannot merge PRs on its own if branch protection
  blocks the merge.

---

## Next steps

- [Troubleshooting](../troubleshooting.md) — broader diagnosis guide
- [Trigger a release manually](trigger-release-manually.md) — replay events with the CLI
