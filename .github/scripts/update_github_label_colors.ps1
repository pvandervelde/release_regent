# PowerShell script to update GitHub label colors for pvandervelde/RepoRoller
# Requires: GitHub CLI (gh) installed and authenticated
# Colors chosen from ColorBrewer/Colorblind-safe palettes for accessibility

# Label definitions: Name, Color, Description
$labelUpdates = @(
    @{ Name = "type: feat"; Color = "fcc37b"; Description = "New feature or enhancement" }
    @{ Name = "type: fix"; Color = "fcc37b"; Description = "Bug fix" }
    @{ Name = "type: chore"; Color = "fcc37b"; Description = "Chore or maintenance task" }
    @{ Name = "type: docs"; Color = "fcc37b"; Description = "Documentation changes" }
    @{ Name = "type: refactor"; Color = "fcc37b"; Description = "Code refactoring" }
    @{ Name = "type: test"; Color = "fcc37b"; Description = "Test-related changes" }
    @{ Name = "type: ci"; Color = "fcc37b"; Description = "Continuous integration or build changes" }

    @{ Name = "status: needs-triage"; Color = "64befb"; Description = "Needs triage or review" }
    @{ Name = "status: in-progress"; Color = "64befb"; Description = "Work in progress" }
    @{ Name = "status: needs-review"; Color = "64befb"; Description = "Needs code or design review" }
    @{ Name = "status: blocked"; Color = "64befb"; Description = "Blocked by another issue or dependency" }
    @{ Name = "status: completed"; Color = "64befb"; Description = "Work completed" }

    @{ Name = "prio: high"; Color = "e94648"; Description = "High priority" }
    @{ Name = "prio: medium"; Color = "e94648"; Description = "Medium priority" }
    @{ Name = "prio: low"; Color = "e94648"; Description = "Low priority" }

    @{ Name = "comp: core"; Color = "91c764"; Description = "Core component" }
    @{ Name = "comp: github"; Color = "91c764"; Description = "GitHub integration/component" }
    @{ Name = "comp: azure"; Color = "91c764"; Description = "Azure integration/component" }
    @{ Name = "comp: infra"; Color = "91c764"; Description = "Infrastructure component" }
    @{ Name = "comp: ci"; Color = "91c764"; Description = "CI/CD component" }

    @{ Name = "size: XS"; Color = "fecc3e"; Description = "Extra small change" }
    @{ Name = "size: S"; Color = "fecc3e"; Description = "Small change" }
    @{ Name = "size: M"; Color = "fecc3e"; Description = "Medium change" }
    @{ Name = "size: L"; Color = "fecc3e"; Description = "Large change" }
    @{ Name = "size: XL"; Color = "fecc3e"; Description = "Extra large change" }

    @{ Name = "feedback: discussion"; Color = "c8367a"; Description = "Discussion or open feedback" }
    @{ Name = "feedback: rfc"; Color = "c8367a"; Description = "Request for comments (RFC)" }
    @{ Name = "feedback: question"; Color = "c8367a"; Description = "General question or inquiry" }

    @{ Name = "inactive: duplicate"; Color = "d3d8de"; Description = "Duplicate issue or PR" }
    @{ Name = "inactive: wontfix"; Color = "d3d8de"; Description = "Will not fix" }
    @{ Name = "inactive: by-design"; Color = "d3d8de"; Description = "Closed as by design" }
)

# Get all current labels in the repo
function Get-AllLabels
{
    $perPage = 100
    $labels = gh label list --json name --limit $perPage | ConvertFrom-Json
    if ($labels.Count -eq 0)
    {
        break
    }

    $allLabels = @()
    $allLabels += $labels

    return $allLabels | ForEach-Object { $_.name }
}
$currentLabels = Get-AllLabels

# Build a hashtable of label names for quick lookup
$labelNames = $labelUpdates | ForEach-Object { $_.Name }

# Update or create labels as needed
foreach ($label in $labelUpdates)
{
    $name = $label.Name
    $color = $label.Color
    $desc = $label.Description
    if ($currentLabels -contains $name)
    {
        Write-Host "Updating label '$name' to color #$color and description '$desc'"
        gh label edit "$name" --color $color --description "$desc"
    }
    else
    {
        Write-Host "Creating label '$name' with color #$color and description '$desc'"
        gh label create "$name" --color $color --description "$desc"
    }
}

# Remove labels not in the hashtable
$labelsToRemove = $currentLabels | Where-Object { -not ($labelNames -contains $_) }
foreach ($label in $labelsToRemove)
{
    Write-Host "Deleting label '$label' (not in hashtable)"
    gh label delete "$label" --yes
}
