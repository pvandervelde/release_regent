---
"release-regent-core": patch
---

Fixed GitHub API calls not honoring the `error_handling` configuration (`max_retries`,
`backoff_multiplier`, `initial_delay_ms`). Transient failures — network errors, rate limits,
timeouts, and optimistic-lock conflicts — are now retried with exponential backoff as
documented, up to the configured `max_retries`. As a safety measure, the delay before each
retry attempt is capped at 30 seconds regardless of the configured `backoff_multiplier` and
`initial_delay_ms`. Permanent failures (e.g. 404 Not Found, authentication failures, validation
errors) continue to fail immediately without retry.
