---
"release-regent-core": patch
"release-regent-github-client": patch
---

`rr:override-*` labels from `!release major|minor|patch` are now honored in every release
lifecycle event, not just when the exact labelled PR merges. If a later feature PR merges
without its own override label, Release Regent scans the commit history for the current release
cycle and still applies the highest outstanding override as the version floor. `!set-version` on
the release PR now rejects a pinned version that falls below an outstanding override floor,
naming the required minimum version and the PR the override came from. And if the release PR is
deleted before merging and a new one is created by a later feature PR merge, the new release
PR's version also honors any outstanding override found in the commit history instead of
recalculating from scratch and silently losing it.

Previously, an outstanding override label only took effect if the exact PR it was applied to was
the one that merged and triggered the release PR update; in every other scenario the label was
silently ignored. There is no new configuration or command syntax — existing `!release` and
`!set-version` usage is unchanged except that it now behaves correctly in these edge cases.
