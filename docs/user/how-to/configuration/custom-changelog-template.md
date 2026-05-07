---
title: Customise changelog templates
description: Full guide to using Tera templates for changelog body, header, and footer
---

# Customise changelog templates

Release Regent uses [git-cliff-core](https://git-cliff.org/) for changelog generation, which
means changelog templates are written in the [Tera](https://keats.github.io/tera/) templating
language. This page is a practical reference for writing custom templates.

For a step-by-step guided approach, work through the
[Customise your changelog tutorial](../../tutorials/02-customise-changelog.md) first.

## Template sections

The changelog is built from three template strings configured in `.release-regent.toml`:

| Setting | Purpose |
| :--- | :--- |
| `changelog.header` | Prepended once at the top of the entire changelog document |
| `changelog.body` | Rendered once per release version; this is the main template |
| `changelog.footer` | Appended once at the bottom of the entire changelog document |

## Available variables in `body`

| Variable | Type | Description |
| :--- | :--- | :--- |
| `version` | `string \| null` | The release version, e.g. `"1.2.3"`. `null` for unreleased. |
| `timestamp` | `integer` | Unix timestamp of the release |
| `commits` | `array` | Array of commit objects (see below) |
| `previous` | `object \| null` | Previous release info: `.version`, `.timestamp` |
| `github` | `object` | Repository info: `.owner`, `.repo` |

### Commit object fields

| Field | Type | Description |
| :--- | :--- | :--- |
| `id` | `string` | Full commit SHA |
| `message` | `string` | Parsed subject line |
| `group` | `string` | Group name assigned by `commit_parsers` |
| `scope` | `string \| null` | Conventional commit scope, e.g. `"auth"` |
| `breaking` | `bool` | Whether this is a breaking change |
| `breaking_description` | `string \| null` | Content of the `BREAKING CHANGE:` footer |
| `links` | `array` | List of URLs associated with the commit |
| `author` | `object` | `.name` and `.email` |

## Minimal example

```toml
[changelog]
body = """
{%- if version -%}
## {{ version }} — {{ timestamp | date(format="%Y-%m-%d") }}
{% else -%}
## Unreleased
{% endif -%}

{% for group, commits in commits | group_by(attribute="group") -%}
### {{ group }}

{% for commit in commits -%}
- {% if commit.scope %}**{{ commit.scope }}**: {% endif %}{{ commit.message | upper_first }}
{% endfor %}
{% endfor -%}
"""
```

## Full example with commit links and breaking change callouts

```toml
[changelog]
header = """
# Changelog

All notable changes to this project will be documented in this file.
"""

body = """
{%- if version -%}
## [{{ version | trim_start_matches(pat="v") }}] — {{ timestamp | date(format="%Y-%m-%d") }}
{% else -%}
## [Unreleased]
{% endif -%}

{% for group, commits in commits | group_by(attribute="group") %}
### {{ group | striptags | trim | upper_first }}

{% for commit in commits | sort(attribute="scope") -%}
- {% if commit.scope -%}**{{ commit.scope }}**: {% endif -%}
  {{ commit.message | upper_first -}}
  {% if commit.links %} \
  ([`{{ commit.id | truncate(length=7, end="") }}`]({{ commit.links[0] }})){% endif %}
  {%- if commit.breaking %}
  > ⚠️ **Breaking**: {{ commit.breaking_description }}
  {%- endif %}
{% endfor %}
{% endfor -%}
"""

footer = """
---
[Full diff](https://github.com/{{ github.owner }}/{{ github.repo }}/compare/\
{{ previous.version }}...{{ version }})
"""

trim = true
```

## Skipping unwanted commits

Use `commit_parsers` to skip commits before they reach the template. Rules are matched in
order; the first match wins.

```toml
[[changelog.commit_parsers]]
message = "^chore\\(release\\): prepare"
skip = true

[[changelog.commit_parsers]]
message = "^chore\\(deps\\)"
skip = true

[[changelog.commit_parsers]]
message = "^Merge (branch|pull request)"
skip = true

[[changelog.commit_parsers]]
message = "^feat"
group = "🚀 Features"

[[changelog.commit_parsers]]
message = "^fix"
group = "🐛 Bug fixes"
```

## Tera filter reference

Useful Tera built-in filters and functions for changelog templates:

| Expression | Effect |
| :--- | :--- |
| `value \| trim_start_matches(pat="v")` | Remove leading `v` from version strings |
| `value \| upper_first` | Capitalise first letter |
| `value \| striptags` | Remove HTML tags |
| `value \| truncate(length=7, end="")` | Trim a SHA to 7 characters |
| `timestamp \| date(format="%Y-%m-%d")` | Format a Unix timestamp as a date |
| `commits \| group_by(attribute="group")` | Group commits into a map by `group` field |
| `commits \| sort(attribute="scope")` | Sort commits by scope |
| `commits \| filter(attribute="scope")` | Keep only commits that have a scope |

Full Tera documentation: <https://keats.github.io/tera/docs/>

## Testing your template

After editing `.release-regent.toml`, run:

```bash
rr test --commits 20
```

The last section of the output shows the rendered changelog with your current template. Iterate
until the output looks right before committing.

---

## Next steps

- [Configuration reference — changelog section](../../reference/configuration.md#changelog-configuration)
