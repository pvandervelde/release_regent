#Requires -Version 5.1
<#
.SYNOPSIS
    Creates a test GitHub repository pre-loaded with commits and branches
    that exercise the full Release Regent workflow.

.DESCRIPTION
    This script bootstraps a disposable test repository on GitHub so you can
    verify Release Regent end-to-end without touching a real project.

    What the script creates
    -----------------------
    After the script finishes you have a GitHub repository with:

    main (tagged v0.1.0, with .release-regent.yml dotfile declaring group = "backend")
    ├── fix/handle-empty-input       PATCH bump → v0.1.1
    ├── feat/add-greeting-styles     MINOR bump → v0.2.0
    ├── feat/add-language-support    MINOR bump → v0.2.0 (changelog only update)
    ├── docs/update-api-docs         no version bump
    ├── chore/update-ci              no version bump
    └── feat/breaking-rename-endpoint  MAJOR bump → v1.0.0

    A companion metadata repository ({Owner}/.release-regent) is also created
    with:
      global.toml     — org-wide policy: version_prefix = "vGLOBAL-" (not locked)
      groups/backend.toml — group policy for the "backend" group:
                            version_prefix = "v" and locks versioning.strategy

    This exercises the full five-level configuration hierarchy:
      Level 2 (app)   → version_prefix = "vAPP-"   (samples/config/release-regent.toml)
      Level 3 (global)→ version_prefix = "vGLOBAL-" (overrides app level)
      Level 4 (group) → version_prefix = "v"        (overrides global)
                        locks versioning.strategy = "conventional"
      Level 5 (repo)  → inherits "v" from group (dotfile sets only allow_override)

    Expected effective config: version_prefix = "v", strategy locked to "conventional".

    Suggested merge order
    ---------------------
    Merge in this order to exercise each Release Regent code path:

    1. fix/handle-empty-input        → expect: release/v0.1.1 PR created
    2. feat/add-greeting-styles      → expect: release/v0.2.0 PR, replaces v0.1.1 PR
    3. feat/add-language-support     → expect: release/v0.2.0 PR changelog updated
    4. docs/update-api-docs          → expect: no change (docs-only, no bump)
    5. chore/update-ci               → expect: no change (chore, no bump)
       Merge the release/v0.2.0 PR  → expect: GitHub release v0.2.0 created
    6. feat/breaking-rename-endpoint → expect: release/v1.0.0 PR created

.PARAMETER RepoName
    Name of the GitHub repository to create. A random suffix is appended when
    -RandomSuffix is used to avoid conflicts between test runs.
    Defaults to "rr-test".

.PARAMETER Owner
    GitHub user or organisation that owns the repository. Defaults to the
    authenticated gh CLI user (`gh api user`).

.PARAMETER Visibility
    Repository visibility: "private" (default), "public", or "internal".

.PARAMETER WorkDir
    Local directory under which the repository is cloned. Defaults to
    $env:TEMP. The repository is cloned as $WorkDir\$RepoName.

.PARAMETER RandomSuffix
    Append a random 4-character hex suffix to -RepoName so that multiple
    test runs do not collide (e.g. "rr-test-a3f9").

.PARAMETER CreatePRs
    Open a draft pull request on GitHub for each branch after pushing.
    Requires the gh CLI to be authenticated with the "repo" scope.

.PARAMETER SkipTagV0
    Skip creating the initial v0.1.0 Git tag. Release Regent will treat
    this as a brand-new project and use the configured initial_version.

.PARAMETER SkipMetadataRepo
    Skip creating the {Owner}/.release-regent metadata repository and its
    global/group policy files. Use this when the metadata repo already exists
    or when you want to test the two-level fallback path (app-level + repo
    dotfile only).

.EXAMPLE
    # Private repo, no PRs
    .\samples\create-test-repo.ps1

.EXAMPLE
    # Public repo, open draft PRs, random suffix to avoid naming conflicts
    .\samples\create-test-repo.ps1 -Visibility public -CreatePRs -RandomSuffix

.EXAMPLE
    # Clone into a specific directory and prefix the repo name
    .\samples\create-test-repo.ps1 -RepoName rr-integration-test `
                                    -WorkDir C:\dev\scratch `
                                    -CreatePRs
#>
[CmdletBinding()]
param (
    [string]$RepoName = 'rr-test',

    [string]$Owner,

    [ValidateSet('private', 'public', 'internal')]
    [string]$Visibility = 'private',

    [string]$WorkDir = $env:TEMP,

    [switch]$RandomSuffix,

    [switch]$CreatePRs,

    [switch]$SkipTagV0,

    [switch]$SkipMetadataRepo
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ─────────────────────────────────────────────────────────────────────────────
# Helpers
# ─────────────────────────────────────────────────────────────────────────────

function Write-Step
{
    param ([string]$Message)
    Write-Host ''
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Write-Info
{
    param ([string]$Message)
    Write-Host "    $Message"
}

function Write-Success
{
    param ([string]$Message)
    Write-Host "    OK  $Message" -ForegroundColor Green
}

function Write-Fatal
{
    param ([string]$Message)
    Write-Host ''
    Write-Host "ERROR: $Message" -ForegroundColor Red
    exit 1
}

# Run a git command rooted at $RepoDir. Throws on non-zero exit code.
function Invoke-Git
{
    param ([string[]]$Arguments)
    & git -C $script:RepoDir @Arguments 2>&1 | ForEach-Object { Write-Verbose $_ }
    if ($LASTEXITCODE -ne 0)
    {
        Write-Fatal "git $($Arguments -join ' ') failed (exit $LASTEXITCODE)"
    }
}

# Create a file (and any missing parent directories) relative to $RepoDir.
function New-RepoFile
{
    param (
        [string]$RelPath,
        [string]$Content
    )
    $fullPath = Join-Path $script:RepoDir $RelPath
    $dir = Split-Path $fullPath
    if (-not (Test-Path $dir))
    {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    Set-Content -Path $fullPath -Value $Content -Encoding UTF8 -NoNewline
}

# Stage all pending changes and create a commit.
function New-Commit
{
    param (
        [string]$Message,
        [string]$Body = ''
    )
    Invoke-Git @('add', '--all')
    $fullMessage = if ($Body)
    {
        "$Message`n`n$Body"
    }
    else
    {
        $Message
    }
    Invoke-Git @('commit', '--message', $fullMessage)
}

# Create a branch from main, commit files, push, and optionally open a PR.
function New-Branch
{
    param (
        [string]  $BranchName,
        [string]  $PrTitle,
        [string]  $PrBody,
        [scriptblock]$FilesBlock   # Called with no args; should call New-RepoFile
    )

    Write-Info "Creating branch: $BranchName"
    Invoke-Git @('checkout', '-b', $BranchName, 'main')

    & $FilesBlock

    Invoke-Git @('push', '--set-upstream', 'origin', $BranchName)

    if ($CreatePRs)
    {
        $null = & gh pr create `
            --repo   "$script:FullRepoName" `
            --head   $BranchName `
            --base   main `
            --title  $PrTitle `
            --body   $PrBody `
            --draft  `
            2>&1
        if ($LASTEXITCODE -ne 0)
        {
            Write-Host "    WARN: Could not create PR for $BranchName" -ForegroundColor Yellow
        }
        else
        {
            Write-Info "    Draft PR opened."
        }
    }

    # Return to main for the next branch.
    Invoke-Git @('checkout', 'main')
}

# ─────────────────────────────────────────────────────────────────────────────
# 1. Prerequisites
# ─────────────────────────────────────────────────────────────────────────────

Write-Step 'Checking prerequisites'

foreach ($cmd in 'gh', 'git')
{
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue))
    {
        Write-Fatal "'$cmd' was not found on PATH.$(
            if ($cmd -eq 'gh') { ' Install from https://cli.github.com/' }
            else { ' Install Git from https://git-scm.com/' }
        )"
    }
}

# Confirm the gh CLI is authenticated.
$null = gh auth status 2>&1
if ($LASTEXITCODE -ne 0)
{
    Write-Fatal "gh is not authenticated. Run 'gh auth login' first."
}

Write-Success 'gh and git are available and authenticated.'

# ─────────────────────────────────────────────────────────────────────────────
# 2. Resolve owner and repository name
# ─────────────────────────────────────────────────────────────────────────────

if (-not $Owner)
{
    $Owner = (gh api user --jq '.login' 2>&1)
    if ($LASTEXITCODE -ne 0 -or -not $Owner)
    {
        Write-Fatal 'Could not determine the authenticated GitHub user. Use -Owner to specify one.'
    }
}

if ($RandomSuffix)
{
    $suffix = ([System.IO.Path]::GetRandomFileName() -replace '[^a-z0-9]', '').Substring(0, 4)
    $RepoName = "$RepoName-$suffix"
}

$FullRepoName = "$Owner/$RepoName"
$script:FullRepoName = $FullRepoName

Write-Info "Repository : $FullRepoName ($Visibility)"
Write-Info "Clone dir  : $WorkDir"

# Metadata repo name — always {Owner}/.release-regent regardless of the test repo name.
$MetaRepoName  = "$Owner/.release-regent"
$script:MetaRepoName = $MetaRepoName

# ─────────────────────────────────────────────────────────────────────────────
# 3. Create the GitHub repository
# ─────────────────────────────────────────────────────────────────────────────

Write-Step "Creating GitHub repository: $FullRepoName"

# Check whether the repo already exists before trying to create it.
$existing = gh repo view $FullRepoName 2>&1
if ($LASTEXITCODE -eq 0)
{
    Write-Fatal "Repository '$FullRepoName' already exists. Use -RandomSuffix or choose a different -RepoName."
}

$repoUrl = gh repo create $FullRepoName --$Visibility --description 'Release Regent test repository' 2>&1

if ($LASTEXITCODE -ne 0)
{
    Write-Fatal "gh repo create failed: $repoUrl"
}

Write-Success "Repository created: https://github.com/$FullRepoName"

# ─────────────────────────────────────────────────────────────────────────────
# 4. Create (or reuse) the metadata repository
# ─────────────────────────────────────────────────────────────────────────────

if (-not $SkipMetadataRepo)
{
    Write-Step "Setting up metadata repository: $MetaRepoName"

    $metaExists = gh repo view $MetaRepoName 2>&1
    if ($LASTEXITCODE -eq 0)
    {
        Write-Info "Metadata repo '$MetaRepoName' already exists — reusing it."
        Write-Info "Existing global.toml / group files will be overwritten."
    }
    else
    {
        $null = gh repo create $MetaRepoName --$Visibility `
            --description 'Release Regent metadata repository (global and group policy)' 2>&1
        if ($LASTEXITCODE -ne 0)
        {
            Write-Host "    WARN: Could not create metadata repo $MetaRepoName" -ForegroundColor Yellow
            Write-Host "    The server will fall back to the app-level config (Level 2 only)." -ForegroundColor Yellow
            $SkipMetadataRepo = $true
        }
        else
        {
            Write-Success "Metadata repo created: https://github.com/$MetaRepoName"
        }
    }

    if (-not $SkipMetadataRepo)
    {
        # Clone the metadata repo into a sibling temp directory.
        $MetaCloneDir = Join-Path $WorkDir '.release-regent'
        if (Test-Path $MetaCloneDir) { Remove-Item -Recurse -Force $MetaCloneDir }

        & git clone "https://github.com/$MetaRepoName.git" $MetaCloneDir 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0)
        {
            Write-Host "    WARN: Could not clone metadata repo — skipping policy files." -ForegroundColor Yellow
            $SkipMetadataRepo = $true
        }
    }

    if (-not $SkipMetadataRepo)
    {
        # Save current RepoDir and switch to the metadata clone.
        $savedRepoDir   = $script:RepoDir
        $script:RepoDir = $MetaCloneDir

        # Configure git identity and disable signing in the meta clone.
        $gitUserName  = (& git config --global user.name  2>&1) | Out-String
        $gitUserEmail = (& git config --global user.email 2>&1) | Out-String
        if (-not $gitUserName.Trim())  { Invoke-Git @('config', 'user.name',  'Release Regent Test') }
        if (-not $gitUserEmail.Trim()) { Invoke-Git @('config', 'user.email', 'rr-test@example.com') }
        Invoke-Git @('config', 'tag.gpgsign',    'false')
        Invoke-Git @('config', 'commit.gpgsign', 'false')

        # If the metadata repo was brand-new it will have an unborn HEAD;
        # pin to main for consistency.
        $headRef = (& git -C $MetaCloneDir symbolic-ref HEAD 2>&1) | Out-String
        if ($LASTEXITCODE -ne 0 -or $headRef.Trim() -eq '')
        {
            Invoke-Git @('symbolic-ref', 'HEAD', 'refs/heads/main')
        }

        # ── global.toml (Level 3) ───────────────────────────────────────────
        # Overrides the app-level version_prefix ("vAPP-") with "vGLOBAL-" so
        # you can see Level 3 winning when the Group level is absent.
        # Does NOT lock the field, allowing the group policy below to override it.
        New-RepoFile 'global.toml' @"
# ==========================================================================
# Release Regent — global policy (Level 3 of 5)
# Applies to ALL repositories in the $Owner organisation.
# ==========================================================================

[core]
# Overrides the app-level version_prefix ("vAPP-") for all org repos.
# The backend group policy below overrides this further to the real "v".
version_prefix = "vGLOBAL-"
"@

        # ── groups/backend.toml (Level 4) ──────────────────────────────────
        # The test repo's .release-regent.yml declares group = "backend",
        # so this policy applies. It sets the real version_prefix and
        # locks versioning.strategy so no per-repo override can change it.
        New-RepoFile 'groups/backend.toml' @"
# ==========================================================================
# Release Regent — group policy for 'backend' (Level 4 of 5)
# Applies to all repositories that declare group = "backend" in their dotfile.
# ==========================================================================

# Lock versioning.strategy for all backend repos so individual repos cannot
# switch away from conventional commits.
locked_fields = ["versioning.strategy"]

[core]
# Use the normal "v" prefix for all backend repos,
# overriding the org-wide "vGLOBAL-" from global.toml.
version_prefix = "v"

[versioning]
# Force conventional-commit versioning for every backend repo.
# This value is locked by locked_fields above.
strategy = "conventional"
"@

        Invoke-Git @('add', '--all')

        # Detect whether there are any staged changes to avoid a no-op commit
        # on an already-populated metadata repo.
        $status = (& git -C $MetaCloneDir status --porcelain) | Out-String
        if ($status.Trim())
        {
            Invoke-Git @('commit', '--message', 'chore: add global and backend group policy for Release Regent testing')
            Invoke-Git @('push', '--set-upstream', 'origin', 'main')
            Write-Success 'Metadata repo policy files committed and pushed.'
        }
        else
        {
            Write-Info 'Metadata repo already up to date — no new commit needed.'
        }

        # Restore the original repo directory for the remaining steps.
        $script:RepoDir = $savedRepoDir
    }
}

# ─────────────────────────────────────────────────────────────────────────────
# 5. Clone the test repository
# ─────────────────────────────────────────────────────────────────────────────

$CloneDir = Join-Path $WorkDir $RepoName
$script:RepoDir = $CloneDir

Write-Step "Cloning into: $CloneDir"

if (Test-Path $CloneDir)
{
    Write-Fatal "Local directory already exists: $CloneDir. Remove it or choose a different -WorkDir."
}

& git clone "https://github.com/$FullRepoName.git" $CloneDir 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0)
{
    Write-Fatal "git clone failed."
}

# Set local git identity when the global identity is not configured.
$gitUserName = (& git config --global user.name  2>&1) | Out-String
$gitUserEmail = (& git config --global user.email 2>&1) | Out-String
if (-not $gitUserName.Trim())
{
    Invoke-Git @('config', 'user.name', 'Release Regent Test')
}
if (-not $gitUserEmail.Trim())
{
    Invoke-Git @('config', 'user.email', 'rr-test@example.com')
}

# Disable GPG/SSH signing for this throwaway test repo. Developer machines
# often have tag.gpgsign=true (or commit.gpgsign=true) in global git config;
# if GPG is unavailable or misconfigured that causes `git tag --annotate` to
# exit 128 (fatal). These local overrides ensure the script always works.
Invoke-Git @('config', 'tag.gpgsign',    'false')
Invoke-Git @('config', 'commit.gpgsign', 'false')

# Cloning an empty repo leaves HEAD in an "unborn" state. The branch name used
# for the first commit comes from the local init.defaultBranch git setting,
# which varies from system to system (commonly 'master' on older git installs).
# Pin it explicitly to 'main' before making any commits so that the branch
# name is predictable and consistent with what Release Regent expects.
Invoke-Git @('symbolic-ref', 'HEAD', 'refs/heads/main')

Write-Success 'Repository cloned.'

# ─────────────────────────────────────────────────────────────────────────────
# 6. Initial commit on main
# ─────────────────────────────────────────────────────────────────────────────

Write-Step 'Creating initial commit on main'

New-RepoFile 'README.md' @"
# $RepoName

A disposable test repository for [Release Regent](https://github.com/pvandervelde/release_regent).

## Purpose

This repository was generated by `create-test-repo.ps1` so that the Release Regent
webhook integration can be tested end-to-end without affecting real projects.

## Configuration hierarchy in use

This repo exercises the full five-level configuration hierarchy:

| Level | Source | version_prefix | Notes |
| :---: | :----- | :------------- | :---- |
| 2 | App-level (`samples/config/release-regent.toml`) | `vAPP-` | Server baseline |
| 3 | Global policy (`$Owner/.release-regent/global.toml`) | `vGLOBAL-` | Org-wide override |
| 4 | Group policy (`$Owner/.release-regent/groups/backend.toml`) | `v` | Locks `versioning.strategy` |
| 5 | Repo dotfile (`.release-regent.yml` in this repo) | *(inherits `v`)* | Sets `allow_override = true` |

Effective config: `version_prefix = "v"`, `versioning.strategy = "conventional"` (locked).

## Suggested merge order

Merge the branches in the following order to exercise each Release Regent code path:

| Order | Branch | Conventional commit type | Expected outcome |
| :---: | :----- | :----------------------- | :--------------- |
| 1 | \`fix/handle-empty-input\` | \`fix:\` | \`release/v0.1.1\` PR created |
| 2 | \`feat/add-greeting-styles\` | \`feat:\` | \`release/v0.2.0\` PR created, replaces v0.1.1 |
| 3 | \`feat/add-language-support\` | \`feat:\` | \`release/v0.2.0\` changelog updated |
| 4 | \`docs/update-api-docs\` | \`docs:\` | No version bump |
| 5 | \`chore/update-ci\` | \`chore:\` | No version bump |
|   | _Merge \`release/v0.2.0\` PR_ | — | GitHub release v0.2.0 created |
| 6 | \`feat/breaking-rename-endpoint\` | \`feat!:\` | \`release/v1.0.0\` PR created |

## PR comment commands

The dotfile sets \`allow_override: true\`, so you can test both PR comment commands at any
point while Release Regent is running:

| Command | Where to post it | What it does |
| :------ | :--------------- | :----------- |
| \`!release minor\` | Any open **feature** PR | Raises the bump floor to \`minor\` for that PR, regardless of commit types |
| \`!release major\` | Any open **feature** PR | Raises the bump floor to \`major\` for that PR |
| \`!set-version 3.0.0\` | The open **release** PR | Overrides the calculated version; renames branch and updates changelog heading |
| \`!set-version 1.0.0-rc.1\` | The open **release** PR | Same as above, with a pre-release suffix |

Post the command as a PR comment. Only users with **Write** or higher access on the
repository can issue commands — comments from Read-only users are silently ignored.
"@

New-RepoFile '.release-regent.yml' @"
# ==========================================================================
# Release Regent - repository dotfile (Level 5 of 5)
# This file is specific to the $RepoName repository.
# ==========================================================================

# Assign this repo to the 'backend' group so that
# groups/backend.toml (Level 4) is applied on top of global.toml (Level 3).
group: "backend"

versioning:
  # Allow contributors to override the calculated bump via PR comments.
  # versioning.strategy is locked by the group policy and cannot be changed here.
  allow_override: true
"@

New-RepoFile 'release-regent.toml' @"
# Release Regent configuration for this test repository.
# See https://github.com/pvandervelde/release_regent/blob/master/docs/configuration-reference.md

[repository]
remote_url = "https://github.com/$FullRepoName"
main_branch = "main"
release_branch_pattern = "release/v{version}"
tag_pattern = "v{version}"

[versioning]
prefix = "v"
allow_prerelease = false
initial_version = "0.1.0"

[release_pr]
title_template = "chore(release): prepare version {version}"
draft = false
auto_detect_manifests = false

[[release_pr.manifest_files]]
path = "package.json"
format = "json"
version_key = "version"

[[release_pr.manifest_files]]
path = "version.txt"
format = "plain_text"
version_key = 'version = "([^"]+)"'

[changelog]
include_authors = false
include_commit_links = true
include_pr_links = true
group_by = "type"
sort_commits = "date"

[changelog.commit_types]
feat     = "Features"
fix      = "Bug Fixes"
docs     = "Documentation"
refactor = "Code Refactoring"
perf     = "Performance Improvements"
test     = "Tests"
chore    = "Chores"
ci       = "Continuous Integration"
"@

New-RepoFile 'src/greeting.md' @"
# Greeting Service API

## Endpoints

### POST /greet

Returns a greeting for the given name.

**Request body**
``````json
{ "name": "Alice" }
``````

**Response**
``````json
{ "message": "Hello, Alice!" }
``````
"@

New-RepoFile 'package.json' @"
{
  "name": "greeting-service",
  "version": "0.1.0",
  "description": "A simple greeting service API",
  "license": "MIT"
}
"@

New-RepoFile 'version.txt' 'version = "0.1.0"'

New-Commit -Message 'chore: initial repository setup'

Write-Success 'Initial commit created.'

# ─────────────────────────────────────────────────────────────────────────────
# 7. Tag v0.1.0 as the baseline release
# ─────────────────────────────────────────────────────────────────────────────

if (-not $SkipTagV0)
{
    Write-Step 'Tagging baseline as v0.1.0'

    Invoke-Git @('tag', '--annotate', 'v0.1.0', '--message', 'chore(release): v0.1.0')
    # --set-upstream establishes the tracking reference on this first push.
    # Without it, subsequent git commands that query the upstream fail on
    # some git versions.
    Invoke-Git @('push', '--set-upstream', 'origin', 'main', '--tags')

    Write-Success 'Tag v0.1.0 pushed.'
}
else
{
    Invoke-Git @('push', '--set-upstream', 'origin', 'main')
    Write-Info 'Skipped v0.1.0 tag (Release Regent will start from initial_version).'
}

# ─────────────────────────────────────────────────────────────────────────────
# 8. Feature branches
# ─────────────────────────────────────────────────────────────────────────────

Write-Step 'Creating feature branches'

# ── Branch 1: patch bump ──────────────────────────────────────────────────────

New-Branch `
    -BranchName 'fix/handle-empty-input' `
    -PrTitle    'fix(api): return 400 when input name is empty' `
    -PrBody     @'
## Summary

Guards the `/greet` endpoint against requests that supply an empty or
whitespace-only `name` field, which previously caused a 500 response.

## Changes

- Validate `name` field presence before processing
- Return HTTP 400 with a descriptive error message for blank input
- Add test case for empty-name scenario

## Testing

Manually tested with `curl -X POST /greet -d '{"name":""}'`.
'@ `
    -FilesBlock {
    New-RepoFile 'src/validation.md' @"
# Input Validation

## POST /greet

| Field | Type   | Required | Rules                         |
| :---- | :----- | :------: | :---------------------------- |
| name  | string | yes      | Non-blank; max 200 characters |

When the name field is missing or blank the endpoint returns HTTP 400
with body: { "error": "name must not be blank" }
"@
    New-Commit -Message 'fix(api): return 400 when input name is empty'
}

# ── Branch 2: minor bump ──────────────────────────────────────────────────────

New-Branch `
    -BranchName 'feat/add-greeting-styles' `
    -PrTitle    'feat(api): add formal and casual greeting styles' `
    -PrBody     @'
## Summary

Adds an optional `style` field that lets callers choose between different
greeting registers without changing the endpoint URL.

## Changes

- Accept `style` field: `"formal"` | `"casual"` (default: `"casual"`)
- Formal style: `"Good day, Alice."`
- Casual style: `"Hey, Alice!"`
- Document the new field in the API reference

## Testing

Tested all three style values (explicit formal, explicit casual, omitted).
'@ `
    -FilesBlock {
    New-RepoFile 'src/styles.md' @"
# Greeting Styles

Greeting style is controlled by the optional style field in the request body.

| Style  | Description          | Example output      |
| :----- | :------------------- | :------------------ |
| casual | Friendly, informal   | Hey, Alice!         |
| formal | Professional, polite | Good day, Alice.    |

Default style is casual.
"@
    New-Commit -Message 'feat(api): add formal and casual greeting styles'
}

# ── Branch 3: same minor bump (changelog update only) ────────────────────────

New-Branch `
    -BranchName 'feat/add-language-support' `
    -PrTitle    'feat(i18n): add multi-language greeting support' `
    -PrBody     @'
## Summary

Extends the greeting endpoint to return messages in languages other than English.

## Changes

- Accept optional `language` field: `"en"` | `"es"` | `"fr"` | `"de"`
- Defaults to `"en"` when omitted
- Return 400 for unsupported language codes
- Document supported languages in API reference

## Testing

Tested all supported languages and an unsupported code (`"zh"`).
'@ `
    -FilesBlock {
    New-RepoFile 'src/languages.md' @"
# Supported Languages

Pass an ISO 639-1 language code in the language field to receive a
greeting in a language other than the default English.

| Code | Language |
| :--- | :------- |
| en   | English  |
| es   | Spanish  |
| fr   | French   |
| de   | German   |

Default language is en. Unknown codes return HTTP 400.
"@
    New-Commit -Message 'feat(i18n): add multi-language greeting support'
}

# ── Branch 4: docs-only (no version bump) ────────────────────────────────────

New-Branch `
    -BranchName 'docs/update-api-docs' `
    -PrTitle    'docs(api): add curl examples and error code table' `
    -PrBody     @'
## Summary

Improves the API documentation with runnable curl examples and a
complete table of error codes and their meanings.

No functional changes.
'@ `
    -FilesBlock {
    New-RepoFile 'docs/errors.md' @"
# Error Codes

| HTTP Status | Code              | Meaning                         |
| :---------: | :---------------- | :------------------------------ |
| 400         | BLANK_NAME        | `name` field is empty or absent |
| 400         | UNSUPPORTED_LANG  | `language` code not recognised  |
| 500         | INTERNAL_ERROR    | Unexpected server error         |
"@
    New-Commit -Message 'docs(api): add curl examples and error code table'
}

# ── Branch 5: chore-only (no version bump) ───────────────────────────────────

New-Branch `
    -BranchName 'chore/update-ci' `
    -PrTitle    'chore(ci): update CI pipeline to run on pull_request' `
    -PrBody     @'
## Summary

Switches CI to trigger on `pull_request` events in addition to `push`,
giving earlier feedback on proposed changes. No code or API changes.
'@ `
    -FilesBlock {
    New-RepoFile '.github/workflows/ci.yml' @"
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Lint markdown
        run: echo 'Lint placeholder — replace with real lint step'
"@
    New-Commit -Message 'chore(ci): update CI pipeline to run on pull_request'
}

# ── Branch 6: major breaking bump ────────────────────────────────────────────

New-Branch `
    -BranchName 'feat/breaking-rename-endpoint' `
    -PrTitle    'feat!: rename /greet to /greeting for REST consistency' `
    -PrBody     @'
## Summary

Renames the primary endpoint from `/greet` to `/greeting` to align with
REST resource naming conventions.

## BREAKING CHANGE

The `/greet` endpoint has been removed. All clients must update their
base URL from `/greet` to `/greeting`. No migration period is provided.

## Changes

- Rename endpoint URL from `/greet` to `/greeting`
- Update all internal references and documentation
- Redirect rule is intentionally absent (breaking change)
'@ `
    -FilesBlock {
    New-RepoFile 'src/greeting.md' @"
# Greeting Service API

## Endpoints

### POST /greeting  *(renamed from /greet)*

Returns a greeting in the requested language and style.

> **Breaking change**: the previous `/greet` endpoint has been removed.

**Request body**
``````json
{
  "name": "Alice",
  "style": "formal",
  "language": "fr"
}
``````

**Response**
``````json
{ "message": "Bonjour, Alice." }
``````
"@
    $breakingBody = 'feat!: rename /greet to /greeting for REST consistency

BREAKING CHANGE: The /greet endpoint has been removed. Clients must update
their base URL from /greet to /greeting immediately.'
    New-Commit -Message $breakingBody
}

Write-Success 'All feature branches created and pushed.'

# ─────────────────────────────────────────────────────────────────────────────
# 9. Summary
# ─────────────────────────────────────────────────────────────────────────────

$repoWebUrl = "https://github.com/$FullRepoName"

Write-Host ''
Write-Host '  ┌─────────────────────────────────────────────────────────────┐' -ForegroundColor Green
Write-Host '  │  Test repository ready                                      │' -ForegroundColor Green
Write-Host '  └─────────────────────────────────────────────────────────────┘' -ForegroundColor Green
Write-Host ''
Write-Host "  Repository   : $repoWebUrl"
Write-Host "  Metadata repo: https://github.com/$MetaRepoName"
Write-Host "  Local clone  : $CloneDir"
Write-Host ''
Write-Host '  Config hierarchy' -ForegroundColor Yellow
Write-Host '  ────────────────' -ForegroundColor Yellow
Write-Host '  Level 2 (app)   version_prefix = "vAPP-"   ← samples/config/release-regent.toml'
Write-Host '  Level 3 (global) version_prefix = "vGLOBAL-" ← global.toml in metadata repo'
Write-Host '  Level 4 (group) version_prefix = "v"        ← groups/backend.toml (also locks strategy)'
Write-Host '  Level 5 (repo)  (inherits "v")              ← .release-regent.yml in this repo'
Write-Host '  ────────────────'
Write-Host '  Effective config: version_prefix = "v", strategy = "conventional" (locked)'
Write-Host ''
Write-Host '  Next steps' -ForegroundColor Yellow
Write-Host '  ──────────' -ForegroundColor Yellow
Write-Host "  1. Install your Release Regent GitHub App on the TEST repository:"
Write-Host "       $repoWebUrl/settings/installations"
Write-Host ''
Write-Host "  2. Install the same GitHub App on the METADATA repository:"
Write-Host "       https://github.com/$MetaRepoName/settings/installations"
Write-Host '     (without this the server falls back to the two-level hierarchy)'
Write-Host ''
Write-Host '  3. Start Release Regent locally (from the repository root):'
Write-Host '       .\samples\run-local.ps1 -SmeeUrl https://smee.io/YOUR_CHANNEL'
Write-Host ''
Write-Host '  4. Test PR comment commands (allow_override = true is set in the dotfile):'
Write-Host '       On any open FEATURE PR:  post  !release minor  or  !release major'
Write-Host '       On the open RELEASE PR:  post  !set-version 3.0.0  to override the version'
Write-Host '     Only Write+ users can issue commands; Read-only comments are ignored.'
Write-Host ''
Write-Host '  5. Merge branches in this order and watch the Release Regent logs:'
Write-Host ''

$scenarios = @(
    [pscustomobject]@{ Order = '1'; Branch = 'fix/handle-empty-input'; Type = 'fix:'    ; Expected = 'release/v0.1.1 PR created' }
    [pscustomobject]@{ Order = '2'; Branch = 'feat/add-greeting-styles'; Type = 'feat:'   ; Expected = 'release/v0.2.0 PR created (replaces v0.1.1)' }
    [pscustomobject]@{ Order = '3'; Branch = 'feat/add-language-support'; Type = 'feat:'   ; Expected = 'release/v0.2.0 changelog updated' }
    [pscustomobject]@{ Order = '4'; Branch = 'docs/update-api-docs'; Type = 'docs:'   ; Expected = 'No version bump' }
    [pscustomobject]@{ Order = '5'; Branch = 'chore/update-ci'; Type = 'chore:'  ; Expected = 'No version bump' }
    [pscustomobject]@{ Order = '*'; Branch = 'Merge the release/v0.2.0 PR'; Type = '—'       ; Expected = 'GitHub release v0.2.0 created' }
    [pscustomobject]@{ Order = '6'; Branch = 'feat/breaking-rename-endpoint'; Type = 'feat!:'  ; Expected = 'release/v1.0.0 PR created' }
)

foreach ($s in $scenarios)
{
    Write-Host ("     {0,-2}  {1,-40}  {2,-8}  {3}" -f $s.Order, $s.Branch, $s.Type, $s.Expected)
}

Write-Host ''

if (-not $CreatePRs)
{
    Write-Host '  Tip: Re-run with -CreatePRs to open draft pull requests automatically.' -ForegroundColor DarkGray
}

Write-Host ''
Write-Host '  To delete both repositories when you are done:' -ForegroundColor DarkGray
Write-Host "    gh repo delete $FullRepoName --yes" -ForegroundColor DarkGray
Write-Host "    gh repo delete $MetaRepoName --yes" -ForegroundColor DarkGray
Write-Host "    Remove-Item -Recurse -Force '$CloneDir'" -ForegroundColor DarkGray
Write-Host "    Remove-Item -Recurse -Force '$(Join-Path $WorkDir '.release-regent')'" -ForegroundColor DarkGray
Write-Host ''
