---
"release-regent-server": patch
---

The server now automatically drops webhook events that originate from the metadata
repository `{owner}/.release-regent` (matched case-insensitively). The drop happens in the
webhook handler before the `ALLOWED_REPOS` / `EXCLUDED_REPOS` check and is logged at `debug`
level.

Because the GitHub App installation must include the metadata repository to read policy
files, GitHub delivers its events (such as pushes editing `global.toml`) to the server.
Previously these were processed like any other repository's events when allowed by
`ALLOWED_REPOS`. No configuration is required, and operators no longer need to exclude
`.release-regent` from `ALLOWED_REPOS`.
