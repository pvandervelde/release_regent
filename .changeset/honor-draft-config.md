---
"release-regent-core": patch
---

`[releases] draft = true` and `[release_pr] draft = true` are no longer silently ignored.
Previously, `ReleaseAutomator` always created the GitHub release as published and
`ReleaseOrchestrator` always opened the release PR as ready-for-review, regardless of
these settings — the parsed TOML value was never passed through to the GitHub API call.
Both settings now take effect as documented: setting `draft = true` under `[releases]`
creates the GitHub release as a draft, and setting `draft = true` under `[release_pr]`
opens the release PR as a draft.

Configs that leave `draft` unset or set it to `false` (the default for both fields) see
no behavior change.
