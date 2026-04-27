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

    main (tagged v0.1.0)
    ├── fix/handle-empty-input       PATCH bump → v0.1.1
    ├── feat/add-greeting-styles     MINOR bump → v0.2.0
    ├── feat/add-language-support    MINOR bump → v0.2.0 (changelog only update)
    ├── docs/update-api-docs         no version bump
    ├── chore/update-ci              no version bump
    └── feat/breaking-rename-endpoint  MAJOR bump → v1.0.0

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

    [switch]$SkipTagV0
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
# 4. Clone the (empty) repository locally
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

# Cloning an empty repo leaves HEAD in an "unborn" state. The branch name used
# for the first commit comes from the local init.defaultBranch git setting,
# which varies from system to system (commonly 'master' on older git installs).
# Pin it explicitly to 'main' before making any commits so that the branch
# name is predictable and consistent with what Release Regent expects.
Invoke-Git @('symbolic-ref', 'HEAD', 'refs/heads/main')

Write-Success 'Repository cloned.'

# ─────────────────────────────────────────────────────────────────────────────
# 5. Initial commit on main
# ─────────────────────────────────────────────────────────────────────────────

Write-Step 'Creating initial commit on main'

New-RepoFile 'README.md' @"
# $RepoName

A disposable test repository for [Release Regent](https://github.com/pvandervelde/release_regent).

## Purpose

This repository was generated by `create-test-repo.ps1` so that the Release Regent
webhook integration can be tested end-to-end without affecting real projects.

## Suggested merge order

Merge the branches in the following order to exercise each Release Regent code path:

| Order | Branch | Conventional commit type | Expected outcome |
| :---: | :----- | :----------------------- | :--------------- |
| 1 | `fix/handle-empty-input` | `fix:` | `release/v0.1.1` PR created |
| 2 | `feat/add-greeting-styles` | `feat:` | `release/v0.2.0` PR created, replaces v0.1.1 |
| 3 | `feat/add-language-support` | `feat:` | `release/v0.2.0` changelog updated |
| 4 | `docs/update-api-docs` | `docs:` | No version bump |
| 5 | `chore/update-ci` | `chore:` | No version bump |
|   | _Merge `release/v0.2.0` PR_ | — | GitHub release v0.2.0 created |
| 6 | `feat/breaking-rename-endpoint` | `feat!:` | `release/v1.0.0` PR created |
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
auto_merge = false

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

New-Commit -Message 'chore: initial repository setup'

Write-Success 'Initial commit created.'

# ─────────────────────────────────────────────────────────────────────────────
# 6. Tag v0.1.0 as the baseline release
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
# 7. Feature branches
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
    New-RepoFile 'src/greeting.md' @"
# Greeting Service API

## Endpoints

### POST /greet

Returns a greeting for the given name. Returns `400 Bad Request` when
`name` is missing or blank.

**Request body**
``````json
{ "name": "Alice" }
``````

**Response — success**
``````json
{ "message": "Hello, Alice!" }
``````

**Response — validation error**
``````json
{ "error": "name must not be blank" }
``````
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
    New-RepoFile 'src/greeting.md' @"
# Greeting Service API

## Endpoints

### POST /greet

Returns a greeting for the given name.

**Request body**
``````json
{
  "name": "Alice",
  "style": "formal"
}
``````

`style` values: `"casual"` (default), `"formal"`.

**Response — casual**
``````json
{ "message": "Hey, Alice!" }
``````

**Response — formal**
``````json
{ "message": "Good day, Alice." }
``````
"@
    New-RepoFile 'src/styles.md' @"
# Greeting Styles

| Style    | Example output         |
| :------- | :--------------------- |
| casual   | Hey, Alice!            |
| formal   | Good day, Alice.       |
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

| Code | Language |
| :--- | :------- |
| en   | English  |
| es   | Spanish  |
| fr   | French   |
| de   | German   |
"@
    New-RepoFile 'src/greeting.md' @"
# Greeting Service API

## Endpoints

### POST /greet

Returns a greeting in the requested language and style.

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

Supported languages: `en`, `es`, `fr`, `de`. Defaults to `en`.
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
# 8. Summary
# ─────────────────────────────────────────────────────────────────────────────

$repoWebUrl = "https://github.com/$FullRepoName"

Write-Host ''
Write-Host '  ┌─────────────────────────────────────────────────────────────┐' -ForegroundColor Green
Write-Host '  │  Test repository ready                                      │' -ForegroundColor Green
Write-Host '  └─────────────────────────────────────────────────────────────┘' -ForegroundColor Green
Write-Host ''
Write-Host "  Repository : $repoWebUrl"
Write-Host "  Local clone: $CloneDir"
Write-Host ''
Write-Host '  Next steps' -ForegroundColor Yellow
Write-Host '  ──────────' -ForegroundColor Yellow
Write-Host "  1. Install your Release Regent GitHub App on this repository:"
Write-Host "       $repoWebUrl/settings/installations"
Write-Host ''
Write-Host '  2. Start Release Regent locally (from the repository root):'
Write-Host '       .\samples\run-local.ps1 -SmeeUrl https://smee.io/YOUR_CHANNEL'
Write-Host ''
Write-Host '  3. Merge branches in this order and watch the Release Regent logs:'
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
Write-Host '  To delete the repository when you are done:' -ForegroundColor DarkGray
Write-Host "    gh repo delete $FullRepoName --yes" -ForegroundColor DarkGray
Write-Host "    Remove-Item -Recurse -Force '$CloneDir'" -ForegroundColor DarkGray
Write-Host ''
