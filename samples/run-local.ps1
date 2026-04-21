#Requires -Version 5.1
<#
.SYNOPSIS
    Runs Release Regent locally in Docker and forwards GitHub webhooks via Smee.

.DESCRIPTION
    Builds (optionally) and starts the Release Regent Docker container, then
    launches a Smee proxy so that GitHub App webhook events are forwarded from
    GitHub to the local instance for end-to-end testing without a public URL.

    The script:
      1. Validates that Docker and Node.js (npx) are available.
      2. Reads GitHub App credentials from a .env file.
      3. Reads the GitHub App private key from a .pem file and passes its
         content securely to the Docker container via a process-scoped
         environment variable.
      4. Optionally builds the Docker image from source.
      5. Starts the container with the config directory mounted read-only.
      6. Starts a smee-client proxy as a background job.
      7. Streams container logs and smee output to the console.
      8. Cleans up all resources on Ctrl+C.

.PARAMETER SmeeUrl
    Smee.io channel URL to receive GitHub App webhooks. When omitted, a new
    channel is created automatically. Find existing channels or create one
    manually at https://smee.io/new.

.PARAMETER EnvFile
    Path to a .env file containing GitHub App credentials. Defaults to .env
    in the same directory as this script. Copy .env.example to .env and fill
    in your values before running.

.PARAMETER PrivateKeyFile
    Path to the GitHub App private key (.pem file). Overrides the value of
    GITHUB_PRIVATE_KEY_FILE in the .env file when specified directly.

.PARAMETER ConfigDir
    Local directory containing a release-regent.toml file. This directory is
    mounted read-only into the container at /config. Defaults to the config/
    sub-directory next to this script.

.PARAMETER ImageName
    Docker image tag to start. Build locally with -Build, or supply a pre-built
    registry image. Defaults to release-regent:local.

.PARAMETER Port
    Host port mapped to the container's 8080. Defaults to 8080. Change this if
    you have another service already bound to 8080.

.PARAMETER Build
    When set, (re)builds the Docker image from the repository root before
    starting the container. Requires the Dockerfile at the repository root.

.EXAMPLE
    # Use an existing Smee channel (recommended for repeated testing sessions)
    .\run-local.ps1 -SmeeUrl https://smee.io/abc123

.EXAMPLE
    # Auto-create a new Smee channel and build the image first
    .\run-local.ps1 -Build

.EXAMPLE
    # Custom .env file, config directory, and port
    .\run-local.ps1 -EnvFile C:\secrets\release-regent.env `
                    -ConfigDir C:\repos\myrepo `
                    -Port 9090
#>
[CmdletBinding()]
param (
    [string]$SmeeUrl,

    [string]$EnvFile = (Join-Path $PSScriptRoot '.env'),

    [string]$PrivateKeyFile,

    [string]$ConfigDir = (Join-Path $PSScriptRoot 'config'),

    [string]$ImageName = 'release-regent:local',

    [ValidateRange(1, 65535)]
    [int]$Port = 8080,

    [switch]$Build
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Name used for the Docker container so it can be found and removed reliably.
$ContainerName = 'release-regent-local'

# ─────────────────────────────────────────────────────────────────────────────
# Helper functions
# ─────────────────────────────────────────────────────────────────────────────

function Write-Step
{
    param ([string]$Message)
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
    Write-Host "    $Message" -ForegroundColor Green
}

function Write-Warning
{
    param ([string]$Message)
    Write-Host "    WARNING: $Message" -ForegroundColor Yellow
}

function Write-Fatal
{
    param ([string]$Message)
    Write-Host ""
    Write-Host "ERROR: $Message" -ForegroundColor Red
    exit 1
}

# Parse a .env-style file into a hashtable.
# Skips blank lines and lines starting with '#'.
# Strips optional surrounding single or double quotes from values.
function Read-EnvFile
{
    param ([string]$Path)

    $result = [ordered]@{}
    if (-not (Test-Path $Path))
    {
        return $result
    }

    foreach ($rawLine in (Get-Content -Path $Path))
    {
        $line = $rawLine.Trim()
        if (-not $line -or $line.StartsWith('#'))
        {
            continue 
        }

        $eqIndex = $line.IndexOf('=')
        if ($eqIndex -le 0)
        {
            continue 
        }

        $key = $line.Substring(0, $eqIndex).Trim()
        $value = $line.Substring($eqIndex + 1).Trim()

        # Strip optional surrounding quotes
        if (($value.StartsWith('"') -and $value.EndsWith('"')) -or
            ($value.StartsWith("'") -and $value.EndsWith("'")))
        {
            $value = $value.Substring(1, $value.Length - 2)
        }

        $result[$key] = $value
    }

    return $result
}

function Assert-Command
{
    param (
        [string]$Name,
        [string]$InstallHint
    )

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue))
    {
        Write-Fatal "'$Name' was not found on PATH. $InstallHint"
    }
}

# ─────────────────────────────────────────────────────────────────────────────
# 1. Prerequisites
# ─────────────────────────────────────────────────────────────────────────────

Write-Step 'Checking prerequisites'

Assert-Command 'docker' `
    'Install Docker Desktop from https://www.docker.com/products/docker-desktop/'

# Verify the Docker daemon is actually responsive before proceeding.
$daemonCheck = docker info 2>&1
if ($LASTEXITCODE -ne 0)
{
    Write-Fatal 'Docker daemon is not running. Start Docker Desktop and try again.'
}

Assert-Command 'npx' `
    'Install Node.js (which includes npx) from https://nodejs.org/'

Write-Success 'Docker and Node.js are available.'

# ─────────────────────────────────────────────────────────────────────────────
# 2. Load environment variables
# ─────────────────────────────────────────────────────────────────────────────

Write-Step "Loading environment from: $EnvFile"

if (-not (Test-Path $EnvFile))
{
    Write-Fatal "Environment file not found: $EnvFile`nCopy samples/.env.example to samples/.env and fill in your values."
}

$envVars = Read-EnvFile -Path $EnvFile

# A direct -PrivateKeyFile parameter takes precedence over the .env setting.
if ($PrivateKeyFile)
{
    $envVars['GITHUB_PRIVATE_KEY_FILE'] = $PrivateKeyFile
}

# Validate that all required variables are present and non-empty.
$requiredVars = @(
    'GITHUB_APP_ID',
    'GITHUB_INSTALLATION_ID',
    'GITHUB_WEBHOOK_SECRET',
    'GITHUB_PRIVATE_KEY_FILE'
)

foreach ($var in $requiredVars)
{
    if (-not $envVars.Contains($var) -or -not $envVars[$var])
    {
        Write-Fatal "$var is required but missing from $EnvFile."
    }
}

Write-Success 'All required environment variables are present.'

# ─────────────────────────────────────────────────────────────────────────────
# 3. Read and validate private key
# ─────────────────────────────────────────────────────────────────────────────

Write-Step 'Loading GitHub App private key'

$pemPath = $envVars['GITHUB_PRIVATE_KEY_FILE']
if (-not (Test-Path $pemPath))
{
    Write-Fatal "Private key file not found: $pemPath`nUpdate GITHUB_PRIVATE_KEY_FILE in $EnvFile."
}

$pemContent = Get-Content -Path $pemPath -Raw
if (-not $pemContent.Contains('BEGIN'))
{
    Write-Fatal "The file at '$pemPath' does not look like a PEM-encoded private key.`nExpected a file containing '-----BEGIN RSA PRIVATE KEY-----' or similar."
}

# Expose the PEM content as a process-level environment variable.
# Docker's '--env GITHUB_PRIVATE_KEY' (without a value) inherits from the
# host process, which avoids shell quoting issues with multi-line strings.
[System.Environment]::SetEnvironmentVariable('GITHUB_PRIVATE_KEY', $pemContent, 'Process')

Write-Success 'Private key loaded.'

# ─────────────────────────────────────────────────────────────────────────────
# 4. Resolve or create Smee channel
# ─────────────────────────────────────────────────────────────────────────────

if (-not $SmeeUrl)
{
    Write-Step 'Creating a new Smee.io channel'

    try
    {
        # Use HttpClient directly so the final redirected URL is reliably
        # available on both PowerShell 5.1 (.NET Framework) and PowerShell 7
        # (.NET Core / .NET 5+).
        $httpClient = [System.Net.Http.HttpClient]::new()
        $smeeResponse = $httpClient.GetAsync([uri]'https://smee.io/new').GetAwaiter().GetResult()
        $SmeeUrl = $smeeResponse.RequestMessage.RequestUri.AbsoluteUri
        $httpClient.Dispose()
    }
    catch
    {
        Write-Fatal "Could not create a Smee channel automatically: $_`nVisit https://smee.io/new in a browser, copy the URL, and pass it via -SmeeUrl."
    }

    if (-not $SmeeUrl -or $SmeeUrl -eq 'https://smee.io/new')
    {
        Write-Fatal "Channel creation returned an unexpected URL: '$SmeeUrl'.`nVisit https://smee.io/new in a browser and pass the URL via -SmeeUrl."
    }

    Write-Success "Channel created: $SmeeUrl"
}
else
{
    Write-Info "Using Smee channel: $SmeeUrl"
}

# ─────────────────────────────────────────────────────────────────────────────
# 5. Build image (optional)
# ─────────────────────────────────────────────────────────────────────────────

if ($Build)
{
    Write-Step "Building Docker image: $ImageName"

    # The Dockerfile lives in the repository root, one level above samples/.
    $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

    & docker build --tag $ImageName $repoRoot
    if ($LASTEXITCODE -ne 0)
    {
        Write-Fatal "docker build failed (exit code $LASTEXITCODE)."
    }

    Write-Success "Image built successfully."
}

# Confirm the image exists before attempting to start a container.
$null = docker image inspect $ImageName 2>&1
if ($LASTEXITCODE -ne 0)
{
    Write-Fatal "Docker image '$ImageName' not found.`nRun with -Build to build it from source, or pull a registry image."
}

# ─────────────────────────────────────────────────────────────────────────────
# 6. Validate config directory
# ─────────────────────────────────────────────────────────────────────────────

if (-not (Test-Path $ConfigDir))
{
    Write-Fatal "Config directory not found: $ConfigDir`nCreate the directory and add a release-regent.toml file."
}

$configDirAbs = (Resolve-Path $ConfigDir).Path

# ─────────────────────────────────────────────────────────────────────────────
# 7. Start Docker container
# ─────────────────────────────────────────────────────────────────────────────

Write-Step "Starting container ($ImageName)"

# Remove any stale container from a previous run that was not cleaned up.
$existingId = (docker ps -aq --filter "name=$ContainerName") 2>&1
if ($existingId)
{
    Write-Info 'Removing stale container from a previous run...'
    docker rm -f $ContainerName 2>&1 | Out-Null
}

$allowedRepos = if ($envVars.Contains('ALLOWED_REPOS') -and $envVars['ALLOWED_REPOS'])
{
    $envVars['ALLOWED_REPOS']
}
else
{
    '*'
}

$rustLog = if ($envVars.Contains('RUST_LOG') -and $envVars['RUST_LOG'])
{
    $envVars['RUST_LOG']
}
else
{
    'info'
}

$channelCap = if ($envVars.Contains('EVENT_CHANNEL_CAPACITY') -and $envVars['EVENT_CHANNEL_CAPACITY'])
{
    $envVars['EVENT_CHANNEL_CAPACITY']
}
else
{
    '1024'
}

$dockerArgs = @(
    'run', '--detach',
    '--name', $ContainerName,
    '--publish', "${Port}:8080",
    '--volume', "${configDirAbs}:/config:ro",
    '--env', "GITHUB_APP_ID=$($envVars['GITHUB_APP_ID'])",
    '--env', 'GITHUB_PRIVATE_KEY',        # inherited from host process env
    '--env', "GITHUB_INSTALLATION_ID=$($envVars['GITHUB_INSTALLATION_ID'])",
    '--env', "GITHUB_WEBHOOK_SECRET=$($envVars['GITHUB_WEBHOOK_SECRET'])",
    '--env', "ALLOWED_REPOS=$allowedRepos",
    '--env', "RUST_LOG=$rustLog",
    '--env', "EVENT_CHANNEL_CAPACITY=$channelCap",
    '--env', 'CONFIG_DIR=/config',
    $ImageName
)

& docker @dockerArgs | Out-Null
if ($LASTEXITCODE -ne 0)
{
    Write-Fatal "docker run failed (exit code $LASTEXITCODE)."
}

Write-Success 'Container started.'

# ─────────────────────────────────────────────────────────────────────────────
# 8. Wait for health check
# ─────────────────────────────────────────────────────────────────────────────

Write-Step 'Waiting for server to become healthy'

$healthUrl = "http://localhost:$Port/health"
$maxWaitSec = 30
$elapsed = 0

while ($elapsed -lt $maxWaitSec)
{
    Start-Sleep -Seconds 1
    $elapsed++

    try
    {
        $healthResponse = Invoke-WebRequest -Uri $healthUrl -UseBasicParsing -TimeoutSec 2 -ErrorAction Stop
        if ($healthResponse.StatusCode -eq 200)
        {
            Write-Success "Server is healthy (${elapsed}s)."
            break
        }
    }
    catch
    {
        # Server not ready yet; keep polling.
    }
}

if ($elapsed -ge $maxWaitSec)
{
    Write-Warning "Server did not report healthy within ${maxWaitSec}s. Check the logs below for errors."
}

# ─────────────────────────────────────────────────────────────────────────────
# 9. Start Smee proxy
# ─────────────────────────────────────────────────────────────────────────────

$webhookTarget = "http://localhost:$Port/webhook"

Write-Step "Starting Smee proxy -> $webhookTarget"

$smeeJob = Start-Job -Name 'SmeeProxy' -ScriptBlock {
    param ($url, $target)
    npx --yes smee-client --url $url --target $target
} -ArgumentList $SmeeUrl, $webhookTarget

# Give smee a moment to connect before showing the banner.
Start-Sleep -Seconds 2

# ─────────────────────────────────────────────────────────────────────────────
# 10. Stream logs until Ctrl+C
# ─────────────────────────────────────────────────────────────────────────────

Write-Host ''
Write-Host '  ┌─────────────────────────────────────────────────────────────┐' -ForegroundColor Green
Write-Host '  │  Release Regent is running locally                          │' -ForegroundColor Green
Write-Host '  └─────────────────────────────────────────────────────────────┘' -ForegroundColor Green
Write-Host ''
Write-Host "  Health endpoint  : http://localhost:$Port/health"
Write-Host "  Webhook endpoint : http://localhost:$Port/webhook"
Write-Host ''
Write-Host '  Configure your GitHub App webhook URL to:' -ForegroundColor Yellow
Write-Host "    $SmeeUrl" -ForegroundColor Yellow
Write-Host ''
Write-Host '  Press Ctrl+C to stop all services.' -ForegroundColor DarkGray
Write-Host ''

# Stream Docker container logs as a background job so that smee output can be
# interleaved with server output in the main polling loop.
$dockerLogsJob = Start-Job -Name 'DockerLogs' -ScriptBlock {
    param ($name)
    & docker logs --follow $name 2>&1
} -ArgumentList $ContainerName

try
{
    while ($true)
    {
        # Relay server log lines (dark gray, prefixed with [server]).
        $serverOutput = Receive-Job $dockerLogsJob -ErrorAction SilentlyContinue
        if ($serverOutput)
        {
            foreach ($line in ($serverOutput -split "`n"))
            {
                $trimmed = $line.TrimEnd()
                if ($trimmed)
                {
                    Write-Host "[server] $trimmed" -ForegroundColor DarkGray
                }
            }
        }

        # Relay smee log lines (cyan, prefixed with [smee]).
        $smeeOutput = Receive-Job $smeeJob -ErrorAction SilentlyContinue
        if ($smeeOutput)
        {
            foreach ($line in ($smeeOutput -split "`n"))
            {
                $trimmed = $line.TrimEnd()
                if ($trimmed)
                {
                    Write-Host "[smee]   $trimmed" -ForegroundColor Cyan
                }
            }
        }

        # Detect unexpected container exit.
        $containerState = (& docker inspect --format '{{.State.Status}}' $ContainerName 2>&1)
        if ($containerState -ne 'running')
        {
            Write-Host ''
            Write-Host "  Container stopped unexpectedly (state: $containerState)." -ForegroundColor Red
            Write-Host '  Review the [server] log lines above for the cause.' -ForegroundColor Red
            break
        }

        Start-Sleep -Milliseconds 500
    }
}
finally
{
    Write-Host ''
    Write-Step 'Stopping services'

    Stop-Job  $dockerLogsJob, $smeeJob -ErrorAction SilentlyContinue
    Remove-Job $dockerLogsJob, $smeeJob -ErrorAction SilentlyContinue

    & docker stop $ContainerName 2>&1 | Out-Null
    & docker rm   $ContainerName 2>&1 | Out-Null

    # Clear the PEM from the process environment: it should not persist after
    # this script exits.
    [System.Environment]::SetEnvironmentVariable('GITHUB_PRIVATE_KEY', $null, 'Process')

    Write-Success 'All services stopped.'
}
