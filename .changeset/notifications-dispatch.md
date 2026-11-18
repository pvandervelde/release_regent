---
"release-regent-core": minor
"release-regent-github-client": minor
---

The `[notifications]` configuration section is no longer inert. Release Regent now
dispatches a notification when a release PR merge successfully creates a release, and
whenever a processing error occurs in any of the event handlers (merged PR, release
automation, PR comment command, PR status comment). Delivery is routed through one of
three strategies selected by `notifications.strategy` — `github_issue` (idempotent:
comments on a matching open issue instead of opening a duplicate, using a hidden marker
embedded in the issue body), `webhook` (POSTs a JSON payload describing the event), or
`slack` (posts a formatted message to an incoming webhook) — or disabled entirely via
`enabled = false` / `strategy = "none"`.

Notification delivery is best-effort and never affects the outcome of the triggering
operation: a delivery failure is logged and swallowed, so a broken notification channel
can never fail a release or mask a processing error. Webhook and Slack URLs are treated
as secrets and are never written to logs; the underlying HTTP client disables redirect
following, applies a 10-second timeout, and uses the `rustls` TLS backend. HTTPS
endpoints are strongly recommended — a non-HTTPS endpoint is still accepted but logs a
warning containing only the scheme and host.
