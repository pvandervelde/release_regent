# Configuration Reference

This reference documents all configuration options available in Release Regent. Configuration is specified in a `release-regent.toml` file in your repository root.

## Configuration File Structure

```toml
[versioning]
# Version calculation settings

[release_pr]
# Release PR template settings

[changelog]
# Changelog generation settings

[repository]
# Repository-specific settings

[github]
# GitHub integration settings
```

## Versioning Configuration

Controls how versions are calculated and formatted.

### `versioning.prefix`

**Type**: String
**Default**: `"v"`
**Description**: Prefix added to version tags and displays.

```toml
[versioning]
prefix = "v"          # Creates tags like "v1.2.3"
# prefix = ""         # Creates tags like "1.2.3"
# prefix = "release-" # Creates tags like "release-1.2.3"
```

### `versioning.allow_prerelease`

**Type**: Boolean
**Default**: `true`
**Description**: Whether to support pre-release versions (alpha, beta, rc).

```toml
[versioning]
allow_prerelease = true   # Supports versions like "1.2.3-beta.1"
# allow_prerelease = false # Only stable versions
```

### `versioning.initial_version`

**Type**: String
**Default**: `"0.1.0"`
**Description**: Version to use when no previous releases exist.

```toml
[versioning]
initial_version = "0.1.0"  # Start new projects at 0.1.0
# initial_version = "1.0.0" # Start new projects at 1.0.0
```

## Release PR Configuration

Controls how release PRs are created and formatted.

### `release_pr.title_template`

**Type**: String
**Default**: `"chore(release): prepare version {version}"`
**Description**: Template for release PR titles.

**Available Variables**:

- `{version}` - Semantic version (e.g., "1.2.3")
- `{version_tag}` - Version with prefix (e.g., "v1.2.3")
- `{date}` - Current date in ISO format (e.g., "2025-07-18")

```toml
[release_pr]
title_template = "chore(release): prepare version {version}"
# title_template = "Release {version_tag}"
# title_template = "Prepare release {version} ({date})"
```

### `release_pr.body_template`

**Type**: String
**Default**: See example below
**Description**: Template for release PR body content.

**Available Variables**:

- `{version}` - Semantic version
- `{version_tag}` - Version with prefix
- `{changelog}` - Generated changelog content
- `{commit_count}` - Number of commits since last release
- `{date}` - Current date in ISO format

```toml
[release_pr]
body_template = """
## Release {version}

### Changes

{changelog}

### Release Information

- **Version**: {version}
- **Commits**: {commit_count} commits since last release
- **Generated**: {date}

### Checklist

- [ ] Review changelog for accuracy
- [ ] Verify version bump is appropriate
- [ ] Check that all features are documented
"""
```

### `release_pr.draft`

**Type**: Boolean
**Default**: `false`
**Description**: Whether to create release PRs as drafts initially.

```toml
[release_pr]
draft = false  # Create ready-to-review PRs
# draft = true # Create draft PRs that require manual marking as ready
```

### `release_pr.auto_merge`

**Type**: Boolean
**Default**: `false`
**Description**: Whether to enable auto-merge on release PRs (requires repository settings).

```toml
[release_pr]
auto_merge = false  # Manual merge required
# auto_merge = true # Auto-merge when checks pass
```

## Changelog Configuration

Release Regent uses the git-cliff-core library for advanced changelog generation, providing powerful templating and customization capabilities.

### Basic Changelog Settings

### `changelog.include_authors`

**Type**: Boolean
**Default**: `true`
**Description**: Whether to include commit author information in changelogs.

```toml
[changelog]
include_authors = true   # Show commit authors
# include_authors = false # Hide author information
```

### `changelog.include_commit_links`

**Type**: Boolean
**Default**: `true`
**Description**: Whether to include links to individual commits.

```toml
[changelog]
include_commit_links = true   # Link to commits on GitHub
# include_commit_links = false # Plain text only
```

### `changelog.include_pr_links`

**Type**: Boolean
**Default**: `true`
**Description**: Whether to include links to pull requests when available.

```toml
[changelog]
include_pr_links = true   # Link to PRs when detected
# include_pr_links = false # Don't link to PRs
```

### `changelog.group_by`

**Type**: String
**Default**: `"type"`
**Options**: `"type"`, `"scope"`, `"none"`
**Description**: How to group commits in the changelog.

```toml
[changelog]
group_by = "type"   # Group by commit type (feat, fix, etc.)
# group_by = "scope" # Group by commit scope
# group_by = "none"  # No grouping, chronological order
```

### `changelog.sort_commits`

**Type**: String
**Default**: `"date"`
**Options**: `"date"`, `"type"`, `"scope"`
**Description**: How to sort commits within groups.

```toml
[changelog]
sort_commits = "date"  # Sort by commit date
# sort_commits = "type" # Sort by commit type
# sort_commits = "scope" # Sort by commit scope
```

### `changelog.commit_types`

**Type**: Table
**Default**: Standard conventional commit types
**Description**: Custom commit type definitions and display names.

```toml
[changelog.commit_types]
feat = "Features"
fix = "Bug Fixes"
docs = "Documentation"
style = "Styles"
refactor = "Code Refactoring"
perf = "Performance Improvements"
test = "Tests"
build = "Build System"
ci = "Continuous Integration"
chore = "Chores"
revert = "Reverts"

# Custom types
security = "Security"
deprecate = "Deprecations"
```

## Advanced Changelog Templates (git-cliff)

Release Regent leverages git-cliff-core for powerful changelog customization using the Tera template engine.

### `changelog.header`

**Type**: String
**Default**: Standard header
**Description**: Template for the changelog header section.

```toml
[changelog]
header = """
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

"""
```

### `changelog.body`

**Type**: String
**Default**: Standard git-cliff template
**Description**: Main template for changelog content using Tera syntax.

**Available Template Variables**:

- `version` - The release version
- `commits` - Array of commit objects
- `commit_parsers` - Configured commit parsers
- `filter_commits` - Whether to filter commits
- `tag` - Git tag information
- `previous` - Previous release information
- `github` - GitHub-specific data

```toml
[changelog]
body = """
{%- if version -%}
    ## [{{ version | trim_start_matches(pat="v") }}] - {{ timestamp | date(format="%Y-%m-%d") }}
{% else -%}
    ## [Unreleased]
{% endif -%}

{% for group, commits in commits | group_by(attribute="group") %}
### {{ group | striptags | trim | upper_first }}
{% for commit in commits
| filter(attribute="scope")
| sort(attribute="scope") %}
    {%- if commit.scope -%}
        - **{{commit.scope}}**: {{ commit.message | upper_first }} \
          {%- if commit.links %} ([{{ commit.id | truncate(length=7, end="") }}]({{ commit.links[0] }})){% endif %}
    {%- else -%}
        - {{ commit.message | upper_first }} \
          {%- if commit.links %} ([{{ commit.id | truncate(length=7, end="") }}]({{ commit.links[0] }})){% endif %}
    {%- endif -%}
    {%- if commit.breaking -%}
        {% raw %}  {% endraw %}- ⚠️ **BREAKING**: {{ commit.breaking_description }}
    {%- endif -%}
{% endfor -%}
{% for commit in commits %}
{%- if commit.scope -%}
{%- else -%}
    - {{ commit.message | upper_first }} \
      {%- if commit.links %} ([{{ commit.id | truncate(length=7, end="") }}]({{ commit.links[0] }})){% endif %}
    {%- if commit.breaking -%}
        {% raw %}  {% endraw %}- ⚠️ **BREAKING**: {{ commit.breaking_description }}
    {%- endif -%}
{%- endif -%}
{% endfor -%}

{% endfor %}
"""
```

### `changelog.footer`

**Type**: String
**Default**: Empty
**Description**: Template for the changelog footer section.

```toml
[changelog]
footer = """
---

**Full Changelog**: https://github.com/{{ github.owner }}/{{ github.repo }}/compare/{{ previous.version }}...{{ version }}
"""
```

### `changelog.trim`

**Type**: Boolean
**Default**: `true`
**Description**: Whether to trim whitespace in templates.

```toml
[changelog]
trim = true   # Remove extra whitespace
# trim = false # Preserve all whitespace
```

## Commit Parsing Configuration

Configure how git-cliff parses and categorizes commits.

### `changelog.commit_parsers`

**Type**: Array of Tables
**Default**: Standard conventional commit parsers
**Description**: Rules for parsing and categorizing commits.

```toml
[[changelog.commit_parsers]]
message = "^feat"
group = "⭐ Features"

[[changelog.commit_parsers]]
message = "^fix"
group = "🐛 Bug Fixes"

[[changelog.commit_parsers]]
message = "^doc"
group = "📚 Documentation"

[[changelog.commit_parsers]]
message = "^perf"
group = "🚀 Performance"

[[changelog.commit_parsers]]
message = "^refactor"
group = "🔨 Refactor"

[[changelog.commit_parsers]]
message = "^style"
group = "🎨 Styling"

[[changelog.commit_parsers]]
message = "^test"
group = "🧪 Testing"

[[changelog.commit_parsers]]
message = "^chore\\(release\\): prepare for"
skip = true

[[changelog.commit_parsers]]
message = "^chore"
group = "⚙️ Miscellaneous"

# Custom parser for security fixes
[[changelog.commit_parsers]]
message = "^security"
group = "🔒 Security"

# Skip certain commit patterns
[[changelog.commit_parsers]]
message = "^Merge branch"
skip = true

[[changelog.commit_parsers]]
message = "^Bump version"
skip = true
```

### `changelog.protect_breaking_commits`

**Type**: Boolean
**Default**: `false`
**Description**: Whether to protect breaking change commits from being skipped.

```toml
[changelog]
protect_breaking_commits = true  # Never skip breaking changes
# protect_breaking_commits = false # Allow breaking changes to be filtered
```

### `changelog.filter_unconventional`

**Type**: Boolean
**Default**: `true`
**Description**: Whether to filter out commits that don't follow conventional format.

```toml
[changelog]
filter_unconventional = true   # Hide non-conventional commits
# filter_unconventional = false # Show all commits
```

## Link Generation

Configure how git-cliff generates links to commits and comparisons.

### `changelog.postprocessors`

**Type**: Array of Tables
**Default**: Link generation processors
**Description**: Post-processing rules for generating links and formatting.

```toml
# Replace commit hashes with links
[[changelog.postprocessors]]
pattern = '\b([0-9a-f]{7,40})\b'
replace = "[${1}](https://github.com/{{ github.owner }}/{{ github.repo }}/commit/${1})"

# Replace issue references with links
[[changelog.postprocessors]]
pattern = '\b#(\d+)\b'
replace = "[#${1}](https://github.com/{{ github.owner }}/{{ github.repo }}/issues/${1})"

# Replace PR references with links
[[changelog.postprocessors]]
pattern = '\bPR #(\d+)\b'
replace = "[PR #${1}](https://github.com/{{ github.owner }}/{{ github.repo }}/pull/${1})"
```

## Filtering and Skipping

Control which commits appear in changelogs.

### `changelog.filter_commits`

**Type**: Boolean
**Default**: `false`
**Description**: Whether to enable commit filtering based on patterns.

```toml
[changelog]
filter_commits = true   # Enable filtering
# filter_commits = false # Include all commits
```

### `changelog.tag_pattern`

**Type**: String
**Default**: `"v[0-9]*"`
**Description**: Pattern for matching release tags.

```toml
[changelog]
tag_pattern = "v[0-9]*"           # Standard versioning
# tag_pattern = "release-[0-9]*"  # Custom tag format
# tag_pattern = "[0-9]*\\.[0-9]*" # No prefix
```

### `changelog.skip_tags`

**Type**: String
**Default**: Empty
**Description**: Pattern for tags to skip in changelog generation.

```toml
[changelog]
skip_tags = "v.*-beta.*|v.*-alpha.*"  # Skip pre-release tags
# skip_tags = "v.*-rc.*"              # Skip release candidates
```

### `changelog.ignore_tags`

**Type**: String
**Default**: Empty
**Description**: Pattern for tags to completely ignore.

```toml
[changelog]
ignore_tags = "v0\\..*"  # Ignore all v0.x releases
```

## Advanced Template Examples

### Emoji-Rich Template

```toml
[changelog]
body = """
{% if version -%}
## 🚀 [{{ version | trim_start_matches(pat="v") }}] - {{ timestamp | date(format="%Y-%m-%d") }}
{%- else -%}
## 🔮 [Unreleased]
{%- endif %}

{% for group, commits in commits | group_by(attribute="group") %}
### {{ group }}
{% for commit in commits -%}
- {{ commit.message | upper_first }}
  {%- if commit.author %} by @{{ commit.author.name }}{% endif -%}
  {%- if commit.links %} ([{{ commit.id | truncate(length=7, end="") }}]({{ commit.links[0] }})){% endif %}
  {%- if commit.breaking %}

  ⚠️ **BREAKING CHANGE**: {{ commit.breaking_description }}
  {%- endif %}
{% endfor %}
{% endfor -%}
"""
```

### Detailed Template with Scopes

```toml
[changelog]
body = """
## {% if version %}{{ version }}{% else %}Unreleased{% endif %} ({{ timestamp | date(format="%Y-%m-%d") }})

{% for group, commits in commits | group_by(attribute="group") -%}
### {{ group }}

{% for scope, commits in commits | group_by(attribute="scope") -%}
{% if scope -%}
#### {{ scope | title }}
{% endif -%}
{% for commit in commits -%}
- {{ commit.message | split(pat=":") | last | trim | upper_first }}
  {%- if commit.links %} ([{{ commit.id | truncate(length=7, end="") }}]({{ commit.links[0] }})){% endif -%}
  {%- if commit.author and include_authors %} - {{ commit.author.name }}{% endif %}
  {%- if commit.breaking %}

  **BREAKING CHANGE**: {{ commit.breaking_description }}
  {%- endif %}
{% endfor -%}
{% endfor -%}
{% endfor -%}
"""
```

### Minimal Template

```toml
[changelog]
body = """
## {{ version }} ({{ timestamp | date(format="%Y-%m-%d") }})

{% for commit in commits -%}
- {{ commit.message | upper_first }}{% if commit.links %} ({{ commit.id | truncate(length=7, end="") }}){% endif %}
{% endfor -%}
"""
```

## Template Context Variables

When writing custom templates, these variables are available:

### Release Information

- `version` - Current version being released
- `previous` - Previous release information
- `timestamp` - Release timestamp

### Commits Array

Each commit object contains:

- `id` - Commit SHA
- `message` - Commit message
- `group` - Parsed group (from commit_parsers)
- `scope` - Commit scope
- `breaking` - Whether this is a breaking change
- `breaking_description` - Description of breaking change
- `author` - Author information (name, email)
- `links` - Array of related links

### GitHub Context

- `github.owner` - Repository owner
- `github.repo` - Repository name

### Template Functions

- `upper_first` - Capitalize first letter
- `trim_start_matches` - Remove prefix
- `truncate` - Limit string length
- `group_by` - Group array by attribute
- `filter` - Filter array
- `sort` - Sort array
- `date` - Format timestamp

## Repository Configuration

Settings specific to your repository and Git hosting.

### `repository.remote_url`

**Type**: String
**Required**: Yes (for link generation)
**Description**: Base URL of your repository for generating links.

```toml
[repository]
remote_url = "https://github.com/owner/repo"
# remote_url = "https://github.com/myorg/myproject"
```

### `repository.main_branch`

**Type**: String
**Default**: `"main"`
**Description**: Name of the default branch that releases are based on.

```toml
[repository]
main_branch = "main"
# main_branch = "master"
# main_branch = "develop"
```

### `repository.release_branch_pattern`

**Type**: String
**Default**: `"release/v{version}"`
**Description**: Pattern for release branch names.

**Available Variables**:

- `{version}` - Semantic version
- `{major}` - Major version number
- `{minor}` - Minor version number
- `{patch}` - Patch version number

```toml
[repository]
release_branch_pattern = "release/v{version}"
# release_branch_pattern = "release/{version}"
# release_branch_pattern = "releases/v{major}.{minor}"
```

### `repository.tag_pattern`

**Type**: String
**Default**: `"v{version}"`
**Description**: Pattern for Git tags created during releases.

```toml
[repository]
tag_pattern = "v{version}"
# tag_pattern = "{version}"
# tag_pattern = "release-{version}"
```

## GitHub Integration

Settings for GitHub API integration and authentication.

### `github.app_id`

**Type**: Integer
**Required**: Yes (for GitHub App authentication)
**Description**: GitHub App ID for authentication.

```toml
[github]
app_id = 123456
```

**Note**: This can also be provided via the `GITHUB_APP_ID` environment variable.

### `github.installation_id`

**Type**: Integer
**Required**: Yes (for GitHub App authentication)
**Description**: GitHub App installation ID for the target repository.

```toml
[github]
installation_id = 789012
```

**Note**: This can also be provided via the `GITHUB_INSTALLATION_ID` environment variable.

### `github.private_key_path`

**Type**: String
**Required**: Yes (for GitHub App authentication)
**Description**: Path to the GitHub App private key file.

```toml
[github]
private_key_path = "/path/to/private-key.pem"
# private_key_path = "./github-app-key.pem"
```

**Note**: The private key can also be provided via the `GITHUB_PRIVATE_KEY` environment variable (as the key content, not a path).

### `github.webhook_secret`

**Type**: String
**Required**: Yes (for webhook signature validation)
**Description**: Secret used to validate webhook signatures.

```toml
[github]
webhook_secret = "your-webhook-secret"
```

**Note**: This should be provided via the `GITHUB_WEBHOOK_SECRET` environment variable in production for security.

### `github.api_base_url`

**Type**: String
**Default**: `"https://api.github.com"`
**Description**: Base URL for GitHub API (useful for GitHub Enterprise).

```toml
[github]
api_base_url = "https://api.github.com"
# api_base_url = "https://github.enterprise.com/api/v3"
```

## Template Variables Reference

All template strings support these variables:

### Version Variables

- **`{version}`**: Semantic version without prefix (e.g., "1.2.3")
- **`{version_tag}`**: Version with configured prefix (e.g., "v1.2.3")
- **`{major}`**: Major version number (e.g., "1")
- **`{minor}`**: Minor version number (e.g., "2")
- **`{patch}`**: Patch version number (e.g., "3")
- **`{prerelease}`**: Pre-release identifier (e.g., "beta.1", empty for stable)

### Content Variables

- **`{changelog}`**: Generated changelog content in Markdown format
- **`{commit_count}`**: Number of commits included since the last release
- **`{commit_list}`**: Bulleted list of commit messages

### Date Variables

- **`{date}`**: Current date in ISO format (e.g., "2025-07-18")
- **`{datetime}`**: Current date and time in ISO format (e.g., "2025-07-18T10:30:00Z")
- **`{year}`**: Current year (e.g., "2025")
- **`{month}`**: Current month (e.g., "07")
- **`{day}`**: Current day (e.g., "18")

## Environment Variables

Release Regent supports environment variables for sensitive configuration:

### Required Environment Variables

- **`GITHUB_APP_ID`**: GitHub App ID (alternative to config file)
- **`GITHUB_INSTALLATION_ID`**: GitHub App installation ID (alternative to config file)
- **`GITHUB_PRIVATE_KEY`**: GitHub App private key content (alternative to file path)
- **`GITHUB_WEBHOOK_SECRET`**: Webhook signature validation secret

### Optional Environment Variables

- **`GITHUB_API_BASE_URL`**: GitHub API base URL (defaults to public GitHub)
- **`RUST_LOG`**: Logging level configuration (e.g., "info", "debug")
- **`RELEASE_REGENT_CONFIG`**: Path to configuration file (defaults to "./release-regent.toml")

## Configuration Examples

### Minimal Configuration

```toml
[repository]
remote_url = "https://github.com/owner/repo"

[github]
app_id = 123456
installation_id = 789012
private_key_path = "./github-app-key.pem"
webhook_secret = "your-webhook-secret"
```

### Comprehensive Configuration

```toml
[versioning]
prefix = "v"
allow_prerelease = true
initial_version = "0.1.0"

[release_pr]
title_template = "chore(release): prepare version {version}"
body_template = """
## Release {version_tag}

### Changes
{changelog}

### Release Details
- **Version**: {version}
- **Commits**: {commit_count} since last release
- **Date**: {date}

### Pre-release Checklist
- [ ] All tests passing
- [ ] Documentation updated
- [ ] Breaking changes documented
"""
draft = false
auto_merge = false

[changelog]
include_authors = true
include_commit_links = true
include_pr_links = true
group_by = "type"
sort_commits = "date"

[changelog.commit_types]
feat = "✨ Features"
fix = "🐛 Bug Fixes"
docs = "📚 Documentation"
style = "💄 Styles"
refactor = "♻️ Code Refactoring"
perf = "⚡ Performance Improvements"
test = "✅ Tests"
build = "📦 Build System"
ci = "👷 Continuous Integration"
chore = "🔧 Chores"
security = "🔒 Security"

[repository]
remote_url = "https://github.com/owner/repo"
main_branch = "main"
release_branch_pattern = "release/v{version}"
tag_pattern = "v{version}"

[github]
app_id = 123456
installation_id = 789012
private_key_path = "./github-app-key.pem"
webhook_secret = "your-webhook-secret"
```

### Enterprise GitHub Configuration

```toml
[repository]
remote_url = "https://github.enterprise.com/owner/repo"
main_branch = "master"

[github]
app_id = 123456
installation_id = 789012
private_key_path = "./github-app-key.pem"
webhook_secret = "your-webhook-secret"
api_base_url = "https://github.enterprise.com/api/v3"
```

## Configuration Validation

Release Regent validates configuration at startup and provides helpful error messages:

### Required Fields

These fields must be present:

- `repository.remote_url`
- `github.app_id`
- `github.installation_id`
- GitHub private key (via file or environment variable)
- `github.webhook_secret`

### Template Validation

Templates are validated for:

- **Syntax**: Must be valid template syntax
- **Variables**: Unknown variables generate warnings
- **Output**: Templates must produce non-empty output

### URL Validation

Repository and API URLs are validated for:

- **Format**: Must be valid HTTP/HTTPS URLs
- **GitHub compatibility**: Must point to GitHub or GitHub Enterprise instances

## Troubleshooting Configuration

### Common Issues

**"Repository URL required"**: Set `repository.remote_url` in your configuration file.

**"GitHub App authentication failed"**: Check that `app_id`, `installation_id`, and private key are correct.

**"Webhook signature validation failed"**: Verify that `webhook_secret` matches your GitHub webhook configuration.

**"Template rendering failed"**: Check template syntax and ensure all variables are spelled correctly.

### Debug Mode

Enable debug logging to see detailed configuration information:

```bash
RUST_LOG=debug rr --config-check
```

This will validate your configuration and show exactly what values are being used.
