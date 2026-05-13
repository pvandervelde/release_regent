---
title: Customise your changelog
description: Learn to shape changelog output using commit groups, emoji headers, and Tera templates
---

# Customise your changelog

This tutorial walks through progressively more advanced changelog customisation. At the end you
will have a changelog configuration that groups commits the way your project works, uses headers
your team prefers, and skips noise commits such as merge commits and dependency bumps.

## Before you start

You need:

- The `rr` CLI installed (see [Your first automated release](01-first-release.md))
- A Git repository with conventional commits
- A `.release-regent.toml` configuration file (run `rr init` if you do not have one yet)

---

## Step 1: Inspect the default output

First, see what the default changelog looks like for your repository:

```bash
rr test --commits 20
```

The last section of the output shows the generated changelog. Note which commits appear, how
they are grouped, and whether there are any commits you would like to hide.

---

## Step 2: Change the grouping header labels

Open `.release-regent.toml` and add a `commit_types` table to the `[changelog]` section. This
overrides the display label for each conventional commit type.

```yaml
changelog:
  commit_types:
    feat: "New features"
    fix: "Bug fixes"
    perf: "Performance improvements"
    refactor: "Internal changes"
    docs: "Documentation"
    chore: "Maintenance"
```

Run `rr test` again to see the new headers in the output.

---

## Step 3: Add emoji to headers

You do not have to use plain text for group headers. Edit the labels to include emoji:

```yaml
changelog:
  commit_types:
    feat: "🚀 New features"
    fix: "🐛 Bug fixes"
    perf: "⚡ Performance improvements"
    refactor: "🔨 Internal changes"
    docs: "📚 Documentation"
    chore: "🔧 Maintenance"
```

---

## Step 4: Skip noisy commits

Most projects generate commits that have no value in a user-facing changelog — automated
dependency bumps, merge commits, and changelog update commits from Release Regent itself.

Use `commit_parsers` to skip them. Add to your `changelog` section:

```yaml
changelog:
  commit_parsers:
    # Hide Release Regent's own release preparation commits
    - message: "^chore\\(release\\): prepare"
      skip: true
    # Hide automated dependency update commits (e.g., from Renovate or Dependabot)
    - message: "^chore\\(deps\\)"
      skip: true
    # Hide bare merge commits
    - message: "^Merge (branch|pull request)"
      skip: true
    # Everything else: use the default type-based grouping
    - message: "^feat"
      group: "🚀 New features"
    - message: "^fix"
      group: "🐛 Bug fixes"
    - message: "^perf"
      group: "⚡ Performance improvements"
    - message: "^docs"
      group: "📚 Documentation"
```

Run `rr test` to confirm the skipped commits no longer appear.

!!! note
    `commit_parsers` rules are evaluated in order. The first matching rule wins. Always put
    `skip` rules before the grouping rules so that skipped patterns are caught first.

---

## Step 5: Group by scope instead of type

If your project uses scopes heavily (e.g. `feat(auth):`, `fix(api):`), you may prefer the
changelog to be organised by scope rather than type.

```yaml
changelog:
  group_by: "scope"
```

Run `rr test` to see the difference. Commits without a scope are grouped under an empty group
label, which can look odd. Combine this with a Tera template (step 6) to handle that case.

---

## Step 6: Use a custom Tera template

For full control, provide a custom `body` template written in
[Tera](https://keats.github.io/tera/) — the same templating language used by git-cliff.

The following template reproduces a lean Keep a Changelog layout:

```yaml
changelog:
  body: |
    {%- if version -%}
    ## [{{ version | trim_start_matches(pat="v") }}] — {{ timestamp | date(format="%Y-%m-%d") }}
    {% else -%}
    ## [Unreleased]
    {% endif -%}

    {% for group, commits in commits | group_by(attribute="group") -%}
    ### {{ group | striptags | trim | upper_first }}

    {% for commit in commits | sort(attribute="scope") -%}
    - {% if commit.scope %}**{{ commit.scope }}**: {% endif -%}
      {{ commit.message | upper_first -}}
      {% if commit.links %} ([`{{ commit.id | truncate(length=7, end="") }}`]({{ commit.links[0] }})){% endif %}
      {%- if commit.breaking %}
        > ⚠️ **Breaking change**: {{ commit.breaking_description }}
      {%- endif %}
    {% endfor %}
    {% endfor -%}
```

!!! tip
    Test template changes quickly with `rr test` before committing them. The CLI prints the
    full rendered changelog for your current branch.

---

## Step 7: Customise the release PR body

The changelog appears inside the release PR body. You can control the surrounding text with the
`release_pr.body_template` setting:

```yaml
release_pr:
  body_template: |
    ## Release {version}

    {changelog}

    ---

    **Commits since last release**: {commit_count}
    **Release date**: {date}

    ### Release checklist

    - [ ] Changelog is accurate
    - [ ] Version bump is appropriate
    - [ ] Documentation is updated
```

Available variables: `{version}`, `{version_tag}`, `{changelog}`, `{commit_count}`, `{date}`.

---

## Verify the full configuration

When you are happy with the output of `rr test`, commit `.release-regent.toml` to your
repository. The next time a PR is merged, Release Regent will use the updated configuration.

---

## Next steps

- See all changelog options: [Configuration reference — changelog section](../reference/configuration.md#changelog-configuration)
- Go deeper on Tera templates: [Customise changelog templates how-to](../how-to/configuration/custom-changelog-template.md)
- Understand how version bumps are calculated: [How versions are calculated](../explanation/version-calculation.md)
