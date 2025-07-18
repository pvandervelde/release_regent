# CLI Reference

This reference documents all commands and options available in the Release Regent CLI tool (`rr`).

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/pvandervelde/release_regent.git
cd release_regent

# Build and install the CLI
cargo install --path crates/cli
```

### From Git Repository

```bash
# Install directly from Git
cargo install --git https://github.com/pvandervelde/release_regent.git release-regent-cli
```

## Global Options

These options are available for all commands:

### `-v, --verbose`

**Type**: Flag
**Description**: Enable verbose logging output. Shows debug-level information including detailed parsing and processing steps.

```bash
rr --verbose test --commits 5
rr -v init --template comprehensive
```

### `-c, --config <PATH>`

**Type**: Path
**Description**: Specify a custom configuration file path. If not provided, defaults to `.release-regent.yml` in the current directory.

```bash
rr --config /path/to/config.yml test
rr -c ./custom-config.yml run --event-file webhook.json
```

### `--help`

**Type**: Flag
**Description**: Display help information for the command or subcommand.

```bash
rr --help          # Show main help
rr init --help     # Show help for init command
```

### `--version`

**Type**: Flag
**Description**: Display the version of Release Regent CLI.

```bash
rr --version
```

## Commands

### `init` - Generate Configuration Files

Initialize Release Regent configuration files in your project.

**Usage**:

```bash
rr init [OPTIONS]
```

**Description**:
Generates sample configuration files to get started with Release Regent. Creates a configuration file and sample webhook payload for testing.

#### Options

##### `-o, --output-dir <PATH>`

**Type**: Path
**Default**: `.` (current directory)
**Description**: Directory where configuration files will be created.

```bash
rr init --output-dir ./config
rr init -o /path/to/project
```

##### `-t, --template <TYPE>`

**Type**: String
**Default**: `basic`
**Options**: `basic`, `comprehensive`, `minimal`
**Description**: Configuration template type to generate.

- `basic` - Standard configuration with common options
- `comprehensive` - Full configuration with all available options and examples
- `minimal` - Minimal configuration with only required fields

```bash
rr init --template basic
rr init --template comprehensive
rr init -t minimal
```

##### `--overwrite`

**Type**: Flag
**Description**: Overwrite existing configuration files without prompting.

```bash
rr init --overwrite
rr init --template comprehensive --overwrite
```

#### Examples

```bash
# Basic setup in current directory
rr init

# Comprehensive setup in config directory
rr init --output-dir ./config --template comprehensive

# Minimal setup, overwriting existing files
rr init --template minimal --overwrite
```

#### Output Files

- `.release-regent.yml` - Main configuration file
- `sample-webhook.json` - Sample webhook payload for testing

### `run` - Process Webhook Events

Process GitHub webhook events locally for testing and development.

**Usage**:

```bash
rr run [OPTIONS] --event-file <FILE>
```

**Description**:

Simulates webhook processing by loading a webhook event from a JSON file and processing it according to your configuration. Useful for testing release workflows locally.

#### Run Command Options

##### `-e, --event-file <FILE>`

**Type**: Path
**Required**: Yes
**Description**: Path to JSON file containing webhook event payload.

```bash
rr run --event-file webhook.json
rr run -e ./events/pr-merged.json
```

##### `-d, --dry-run`

**Type**: Flag
**Description**: Run in dry-run mode. Processes the webhook but doesn't perform any actual operations (no API calls, no file changes).

```bash
rr run --event-file webhook.json --dry-run
rr run -e webhook.json -d
```

##### `-c, --config-path <PATH>`

**Type**: Path
**Description**: Path to configuration file. Overrides the global `--config` option for this command.

```bash
rr run --event-file webhook.json --config-path ./test-config.yml
rr run -e webhook.json -c ./configs/test.yml
```

#### Run Command Examples

```bash
# Process webhook event with default configuration
rr run --event-file sample-webhook.json

# Dry run with custom configuration
rr run --event-file webhook.json --config-path ./test-config.yml --dry-run

# Verbose processing to see detailed logs
rr --verbose run --event-file webhook.json
```

#### Webhook Event Format

The event file should contain a GitHub webhook payload in JSON format:

```json
{
  "action": "closed",
  "number": 42,
  "pull_request": {
    "id": 123456789,
    "number": 42,
    "state": "closed",
    "title": "feat: add new feature",
    "merged": true,
    "merge_commit_sha": "abc123def456789",
    "base": {
      "ref": "main",
      "sha": "def456789abc123"
    },
    "head": {
      "ref": "feature/new-feature",
      "sha": "789abc123def456"
    }
  },
  "repository": {
    "name": "test-repo",
    "full_name": "owner/test-repo",
    "owner": {
      "login": "owner"
    },
    "default_branch": "main"
  }
}
```

### `test` - Analyze Git History

Test conventional commit parsing and changelog generation using your Git history.

**Usage**:

```bash
rr test [OPTIONS]
```

**Description**:

Analyzes recent commits in your Git repository, parses them according to conventional commit standards, calculates version bumps, and generates changelog content. Perfect for testing your configuration and understanding how your commits will be processed.

#### Test Command Options

##### `-n, --commits <NUMBER>`

**Type**: Integer
**Default**: `10`
**Description**: Number of commits to analyze from current HEAD.

```bash
rr test --commits 20
rr test -n 5
```

##### `-f, --from <SHA>`

**Type**: String
**Description**: Starting commit SHA. Analyzes commits from this SHA to HEAD. If not provided, starts from HEAD.

```bash
rr test --from abc123def
rr test --commits 50 --from v1.2.0
```

##### `--current-version <VERSION>`

**Type**: String
**Description**: Current version to calculate the next version from. Must be a valid semantic version.

```bash
rr test --current-version 1.2.3
rr test --current-version v2.0.0-beta.1
```

##### `-v, --verbose` (Test Command)

**Type**: Flag
**Description**: Show detailed commit parsing information including individual commit analysis.

```bash
rr test --verbose
rr test -v --commits 5
```

#### Test Command Examples

```bash
# Analyze last 10 commits
rr test

# Analyze specific number of commits with current version
rr test --commits 20 --current-version 1.5.0

# Detailed analysis from specific commit
rr test --from v1.0.0 --verbose --current-version 1.0.0

# Quick test of recent changes
rr test --commits 3 --verbose
```

#### Sample Output

```text
Analyzing 5 commits...

=== Parsed Commits ===
• feat (auth): add OAuth2 integration
  SHA: abc123def456

• fix (ui): resolve button alignment issue
  SHA: def456789abc

• docs (no scope): update installation guide
  SHA: 789abcdef123

=== Version Calculation ===
Current version: 1.2.3
Next version: 1.3.0

=== Generated Changelog ===
## Features
- **auth**: add OAuth2 integration

## Bug Fixes
- **ui**: resolve button alignment issue

## Documentation
- update installation guide
```

## Exit Codes

The CLI uses standard exit codes:

- **0** - Success
- **1** - General error
- **2** - Configuration error
- **3** - Invalid arguments
- **4** - File not found
- **5** - Command execution failed

## Environment Variables

### `RUST_LOG`

**Type**: String
**Default**: `info`
**Description**: Controls logging level for the CLI. Overrides the `--verbose` flag.

**Values**:

- `error` - Only error messages
- `warn` - Warning and error messages
- `info` - Informational, warning, and error messages
- `debug` - Debug and all higher level messages
- `trace` - All messages including trace information

```bash
RUST_LOG=debug rr test --commits 5
RUST_LOG=error rr run --event-file webhook.json
```

### `RELEASE_REGENT_CONFIG`

**Type**: Path
**Description**: Default configuration file path. Overridden by the `--config` option.

```bash
export RELEASE_REGENT_CONFIG=/path/to/config.yml
rr test
```

## Configuration File

The CLI looks for configuration files in this order:

1. Path specified by `--config` option
2. Path specified by `RELEASE_REGENT_CONFIG` environment variable
3. `.release-regent.yml` in current directory
4. `.release-regent.yaml` in current directory

### Configuration Format

The configuration file uses YAML format. See the [Configuration Reference](configuration-reference.md) for complete documentation.

**Basic Example**:

```yaml
versioning:
  prefix: "v"
  allow_prerelease: true

changelog:
  include_authors: true
  include_commit_links: true

repository:
  remote_url: "https://github.com/owner/repo"
  main_branch: "main"
```

## Error Handling

### Common Errors

**"Configuration file not found"**:

- Run `rr init` to create a configuration file
- Check the file path with `--config` option
- Verify the file exists and is readable

**"Git command failed"**:

- Ensure you're in a Git repository
- Check that Git is installed and in your PATH
- Verify the commit SHA exists (when using `--from`)

**"Invalid webhook JSON"**:

- Validate JSON syntax in your webhook file
- Ensure required fields are present (action, pull_request, repository)
- Use `rr init` to generate a valid sample

**"Invalid version format"**:

- Use semantic versioning format (e.g., "1.2.3")
- Optionally include "v" prefix (e.g., "v1.2.3")
- Check pre-release format (e.g., "1.2.3-beta.1")

### Debug Mode

Enable debug logging to troubleshoot issues:

```bash
# Using verbose flag
rr --verbose test

# Using environment variable
RUST_LOG=debug rr test

# Both for maximum detail
RUST_LOG=trace rr --verbose test
```

## Integration Examples

### CI/CD Pipeline Testing

```bash
#!/bin/bash
# Test Release Regent configuration in CI

# Install CLI
cargo install --git https://github.com/pvandervelde/release_regent.git release-regent-cli

# Initialize configuration
rr init --template basic

# Test with current repository
rr test --commits 10 --current-version "${CURRENT_VERSION}"

# Validate webhook processing (if webhook file exists)
if [ -f "webhook-test.json" ]; then
    rr run --event-file webhook-test.json --dry-run
fi
```

### Local Development Workflow

```bash
#!/bin/bash
# Local development testing script

# Test recent changes
echo "Testing recent commits..."
rr test --commits 5 --verbose

# Generate changelog for current changes
echo "Generating changelog..."
rr test --commits 20 --current-version "$(git describe --tags --abbrev=0)"

# Test webhook processing
if [ -f "sample-webhook.json" ]; then
    echo "Testing webhook processing..."
    rr run --event-file sample-webhook.json --dry-run
fi
```

### Configuration Validation

```bash
# Validate configuration by running a simple test
rr test --commits 1

# Check configuration with verbose output
rr --verbose test --commits 1

# Test specific webhook scenario
rr run --event-file test-webhook.json --dry-run --verbose
```

## Tips and Best Practices

### Testing Configuration Changes

1. **Start with dry runs**: Always use `--dry-run` when testing webhook processing
2. **Use verbose output**: Add `--verbose` to understand what's happening
3. **Test incrementally**: Start with a few commits, then increase the scope
4. **Validate webhook format**: Use the sample webhook as a template

### Debugging Issues

1. **Check Git repository**: Ensure you're in a valid Git repository
2. **Verify configuration**: Use `rr init` to generate a known-good configuration
3. **Test with samples**: Use generated sample files before testing with real data
4. **Enable debug logging**: Use `RUST_LOG=debug` for detailed output

### Performance Considerations

1. **Limit commit analysis**: Use `--commits` to avoid analyzing large histories
2. **Use specific ranges**: Use `--from` to analyze specific commit ranges
3. **Avoid verbose mode for large datasets**: Verbose output can be overwhelming for many commits

## Troubleshooting

### Git Repository Issues

```bash
# Ensure you're in a Git repository
git status

# Check that commits exist
git log --oneline -5

# Verify specific commit exists
git show <commit-sha>
```

### Configuration Issues

```bash
# Generate fresh configuration
rr init --overwrite

# Test with minimal configuration
rr init --template minimal --overwrite
rr test --commits 3
```

### Webhook Processing Issues

```bash
# Validate JSON format
python -m json.tool webhook.json

# Test with sample webhook
rr init  # Generates sample-webhook.json
rr run --event-file sample-webhook.json --dry-run
```
