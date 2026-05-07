---
title: Use pre-release versions
description: How to configure Release Regent to ship beta and release candidate versions
---

# Use pre-release versions

Release Regent supports pre-release version identifiers such as `-beta.1`, `-rc.1`, and
`-alpha.1`. This page explains how to enable pre-release support and how the workflow differs
from a normal stable release.

## Enable pre-release support

In `.release-regent.toml`:

```toml
[versioning]
allow_prerelease = true
```

With this setting, Release Regent accepts and creates versions such as `1.2.0-beta.1`.

`allow_prerelease` defaults to `true`. Set it to `false` if you want to prevent pre-release
versions entirely and only ever publish stable releases.

## Triggering a pre-release

Pre-release versions are not triggered automatically by commit types. You use the
[`!set-version` PR comment command](../../reference/pr-commands.md) on a release PR to set the
exact pre-release identifier:

1. Let Release Regent create the release PR normally (e.g., for `1.2.0`).
2. Post a comment on the release PR:

   ```
   !set-version 1.2.0-beta.1
   ```

3. Release Regent updates the release PR to target `1.2.0-beta.1` and renames the branch to
   `release/v1.2.0-beta.1`.
4. When you merge, the published GitHub release is tagged `v1.2.0-beta.1`.

## Promoting from pre-release to stable

When you are ready to promote a pre-release to stable:

1. The next time a feature PR is merged, Release Regent recalculates the version.
2. If no new features or fixes have been merged since the pre-release, the calculated version
   will be the stable counterpart (e.g., `1.2.0`).
3. Merge the resulting release PR to publish `v1.2.0`.

Alternatively, if you want to publish the stable release immediately without merging additional
PRs, post `!set-version 1.2.0` on the existing release PR.

## Version comparison and non-downgrade behaviour

Release Regent never downgrades an existing release PR. If a release PR for `1.2.0-beta.1` is
open and a new feature is merged that calculates to `1.2.0`, Release Regent will upgrade the
PR to `1.2.0` (since `1.2.0 > 1.2.0-beta.1` per semantic versioning rules).

---

## Next steps

- [PR comment commands reference](../../reference/pr-commands.md)
- [How versions are calculated](../../explanation/version-calculation.md)
