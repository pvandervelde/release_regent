# Tutorial: Your First Release with Release Regent

*A hands-on tutorial to get you started with Release Regent in 15 minutes*

## What You'll Build

In this tutorial, you'll:

- Set up Release Regent in a sample repository
- Create your first configuration
- Generate a changelog from commits
- Process a simulated webhook event
- Understand how automated releases work

## Before You Start

You'll need:

- A computer with a terminal
- Git installed
- 15 minutes of your time

No prior knowledge of Release Regent is required!

## Step 1: Download and Install

First, let's get Release Regent on your system.

### Download the Tool

Download the latest release from [GitHub Releases](https://github.com/pvandervelde/release_regent/releases) and extract it to a directory in your PATH.

### Verify Installation

Open a terminal and run:

**Linux/macOS (bash)**:

```bash
rr --version
```

**Windows (PowerShell)**:

```powershell
rr --version
```

You should see version information. If you get a "command not found" error, make sure the `rr` executable is in your PATH.

## Step 2: Create a Practice Repository

Let's create a sample repository to practice with:

**Linux/macOS (bash)**:

```bash
# Create a new directory
mkdir release-regent-tutorial
cd release-regent-tutorial

# Initialize git
git init

# Create a sample file
echo "# My Project" > README.md
git add README.md
git commit -m "feat: initial project setup"

# Add a few more commits to have some history
echo "console.log('Hello, world!');" > index.js
git add index.js
git commit -m "feat: add hello world function"

echo "console.log('Goodbye, world!');" >> index.js
git add index.js
git commit -m "fix: add goodbye message"

echo "# Installation" >> README.md
git add README.md
git commit -m "docs: add installation section"
```

**Windows (PowerShell)**:

```powershell
# Create a new directory
mkdir release-regent-tutorial
cd release-regent-tutorial

# Initialize git
git init

# Create a sample file
"# My Project" | Out-File -FilePath README.md -Encoding utf8
git add README.md
git commit -m "feat: initial project setup"

# Add a few more commits to have some history
"console.log('Hello, world!');" | Out-File -FilePath index.js -Encoding utf8
git add index.js
git commit -m "feat: add hello world function"

"console.log('Goodbye, world!');" | Out-File -FilePath index.js -Append -Encoding utf8
git add index.js
git commit -m "fix: add goodbye message"

"# Installation" | Out-File -FilePath README.md -Append -Encoding utf8
git add README.md
git commit -m "docs: add installation section"
```

Perfect! You now have a repository with some conventional commits to work with.

## Step 3: Initialize Release Regent

Now let's set up Release Regent in your repository:

```bash
rr init
```

This creates two files:

- `release-regent.yml` - Your configuration file
- `sample-webhook.json` - A sample webhook for testing

Let's see what was created:

**Linux/macOS (bash)**:

```bash
ls -la release-regent.yml sample-webhook.json
```

**Windows (PowerShell)**:

```powershell
Get-ChildItem release-regent.yml, sample-webhook.json
```

## Step 4: Configure for Your Repository

Open `release-regent.yml` in your text editor. You'll see something like this:

```yaml
repository:
  owner: "your-username"
  name: "your-repo"

versioning:
  scheme: "semantic"
  initial_version: "0.1.0"

release_notes:
  enabled: true
  template: |
    ## What's Changed

    {{#each commits}}
    - {{this.message}} ({{this.sha}})
    {{/each}}

branches:
  main: "main"
  develop: "develop"
```

**Update the repository section** with your information:

```yaml
repository:
  owner: "your-github-username"
  name: "release-regent-tutorial"
```

Save the file.

## Step 5: Test Commit Analysis

Let's see how Release Regent analyzes your commits:

**Linux/macOS (bash)**:

```bash
rr test --commits 4
```

**Windows (PowerShell)**:

```powershell
rr test --commits 4
```

You should see output like:

```
Analyzing 4 commits...
Found 3 conventional commits:
- feat: initial project setup (minor bump)
- feat: add hello world function (minor bump)
- fix: add goodbye message (patch bump)
- docs: add installation section (no version bump)

Calculated version bump: minor
Current version: 0.1.0
Next version: 0.2.0

Generated changelog:
## What's Changed
- feat: initial project setup (abc123)
- feat: add hello world function (def456)
- fix: add goodbye message (ghi789)
- docs: add installation section (jkl012)
```

**What just happened?**

- Release Regent analyzed your 4 commits
- It found 3 that follow conventional commit format
- It calculated that you'd get a minor version bump (0.1.0 → 0.2.0)
- It generated a changelog

## Step 6: Test Webhook Processing

Now let's simulate how Release Regent processes GitHub webhooks:

**Linux/macOS (bash)**:

```bash
rr run --event-file sample-webhook.json --dry-run
```

**Windows (PowerShell)**:

```powershell
rr run --event-file sample-webhook.json --dry-run
```

The `--dry-run` flag means no actual API calls will be made. You should see output showing how the webhook would be processed.

## Step 7: Customize Your Release Notes

Let's make your release notes more interesting. Edit `release-regent.yml` and replace the `release_notes` section:

```yaml
release_notes:
  enabled: true
  template: |
    ## 🚀 What's New in v{{version}}

    {{#if features}}
    ### ✨ New Features
    {{#each features}}
    - {{this.message}}
    {{/each}}
    {{/if}}

    {{#if fixes}}
    ### 🐛 Bug Fixes
    {{#each fixes}}
    - {{this.message}}
    {{/each}}
    {{/if}}

    {{#if others}}
    ### 📚 Other Changes
    {{#each others}}
    - {{this.message}}
    {{/each}}
    {{/if}}
```

Save the file and test it:

```bash
rr test --commits 4 --verbose
```

Notice how the output now categorizes your commits and uses emojis!

## Step 8: Add More Commits

Let's add some different types of commits to see how they're handled:

**Linux/macOS (bash)**:

```bash
# Add a breaking change
echo "export function hello() { return 'Hello!'; }" > index.js
git add index.js
git commit -m "feat!: convert to ES6 module

BREAKING CHANGE: Changed from console.log to exported function"

# Add a performance improvement
echo "// Optimized version" >> index.js
git add index.js
git commit -m "perf: optimize hello function"

# Add a test
echo "// TODO: Add tests" >> index.js
git add index.js
git commit -m "test: add placeholder for tests"
```

**Windows (PowerShell)**:

```powershell
# Add a breaking change
"export function hello() { return 'Hello!'; }" | Out-File -FilePath index.js -Encoding utf8
git add index.js
git commit -m "feat!: convert to ES6 module

BREAKING CHANGE: Changed from console.log to exported function"

# Add a performance improvement
"// Optimized version" | Out-File -FilePath index.js -Append -Encoding utf8
git add index.js
git commit -m "perf: optimize hello function"

# Add a test
"// TODO: Add tests" | Out-File -FilePath index.js -Append -Encoding utf8
git add index.js
git commit -m "test: add placeholder for tests"
```

Now test with your new commits:

```bash
rr test --commits 7
```

You should see that the breaking change bumps the major version (0.2.0 → 1.0.0)!

## Step 9: Create a Custom Webhook

Let's create a custom webhook payload. Create a file called `my-webhook.json`:

```json
{
  "action": "closed",
  "pull_request": {
    "merged": true,
    "base": {
      "ref": "main"
    },
    "head": {
      "ref": "feature/new-feature"
    }
  },
  "repository": {
    "name": "release-regent-tutorial",
    "owner": {
      "login": "your-github-username"
    }
  }
}
```

Test it:

**Linux/macOS (bash)**:

```bash
rr run --event-file my-webhook.json --dry-run
```

**Windows (PowerShell)**:

```powershell
rr run --event-file my-webhook.json --dry-run
```

## Step 10: Understanding the Results

Let's run one more comprehensive test to understand everything:

```bash
rr test --commits 10 --verbose --current-version 0.1.0
```

This shows you:

- **Commit parsing**: How each commit is categorized
- **Version calculation**: Why the version changes
- **Changelog generation**: What the release notes will look like

## What You've Learned

Congratulations! You've successfully:

- ✅ Installed Release Regent
- ✅ Created a configuration file
- ✅ Analyzed commits with conventional commit standards
- ✅ Generated changelogs with custom templates
- ✅ Processed webhook events
- ✅ Understood version calculation rules

## Understanding the Workflow

Release Regent follows this workflow:

1. **Trigger**: A webhook event (PR merge, direct push)
2. **Analysis**: Parse commits since last release
3. **Calculation**: Determine version bump based on commit types
4. **Generation**: Create changelog from commits
5. **Release**: Create GitHub release (in production)

## Next Steps

Now that you understand the basics, you can:

### For Local Development

- **Integrate with your real repositories**: Use `rr init` in your actual projects
- **Customize configurations**: Explore the [Configuration Reference](configuration-reference.md)
- **Learn advanced features**: Check the [CLI Reference](cli-reference.md)

### For Production Use

- **Set up GitHub App**: Follow [GitHub App Setup Guide](github-app-setup.md)
- **Deploy webhook processing**: Use the [Webhook Integration Guide](webhook-integration.md)
- **Understand the concepts**: Read the [Release Automation Guide](release-automation-guide.md)

### If You Have Issues

- **Troubleshoot problems**: Check the [Troubleshooting Guide](troubleshooting-guide.md)
- **Get help**: Review the documentation or open an issue

## Clean Up

If you want to clean up the tutorial files:

```bash
cd ..
rm -rf release-regent-tutorial
```

## Summary

You've learned how Release Regent automates releases by:

- Analyzing conventional commits
- Calculating semantic versions
- Generating formatted changelogs
- Processing GitHub webhooks

The tool bridges the gap between your development workflow and automated releases, making it easy to maintain consistent versioning and clear release notes.

Happy releasing! 🚀
