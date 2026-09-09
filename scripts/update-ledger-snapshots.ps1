#Requires -Version 7.0
<#
.SYNOPSIS
    Regenerates the ledger snapshot fixtures under tests/fixtures/ledger-snapshots.

.DESCRIPTION
    Snapshot fixtures are a compatibility record. Rewriting one is allowed;
    rewriting one without saying why is not, because a reviewer cannot tell an
    intended serialization change from an accidental one by reading the diff.

    This script requires a written explanation, passes it to the guarded
    regenerator in tests/ledger-snapshots, and then re-runs the snapshot tests
    so a bad regeneration fails here rather than in CI. Each rewritten fixture
    gets its revision bumped, the reason recorded in its header, and a digest
    over the new body, so the explanation and the bytes it explains land in the
    same commit.

    The reason belongs in the pull request description as well. The header is
    for whoever reads the fixture in a year.

.PARAMETER Reason
    One line explaining what changed and why the new fixture content is correct.
    At least 20 characters.

.EXAMPLE
    ./scripts/update-ledger-snapshots.ps1 -Reason "revoked_at is now recorded on admin revocation"

.EXAMPLE
    ./scripts/update-ledger-snapshots.ps1 -Reason "new SchemaVersion key added by protocol-config" -WhatIf
#>
[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$Reason
)

$ErrorActionPreference = "Stop"

$trimmed = $Reason.Trim()

if ($trimmed.Length -lt 20) {
    Write-Error "The reason must be at least 20 characters. A fixture diff without an explanation cannot be reviewed."
}

if ($trimmed -match "`n") {
    Write-Error "The reason must be a single line."
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repositoryRoot

try {
    if (-not $PSCmdlet.ShouldProcess("tests/fixtures/ledger-snapshots", "regenerate")) {
        Write-Host "Would regenerate with reason: $trimmed"
        return
    }

    $env:SNAPSHOT_REASON = $trimmed
    try {
        Write-Host "Regenerating ledger snapshot fixtures..."
        cargo test -p ledger-snapshot-tests -- --ignored regenerate
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Regeneration failed."
        }
    }
    finally {
        Remove-Item Env:\SNAPSHOT_REASON -ErrorAction SilentlyContinue
    }

    Write-Host "Verifying the regenerated fixtures..."
    cargo test -p ledger-snapshot-tests
    if ($LASTEXITCODE -ne 0) {
        Write-Error "The regenerated fixtures do not pass their own tests."
    }

    Write-Host ""
    Write-Host "Fixtures regenerated. Review the diff before committing:"
    Write-Host "  git diff tests/fixtures/ledger-snapshots"
    Write-Host ""
    Write-Host "Include this explanation in the pull request description:"
    Write-Host "  $trimmed"
}
finally {
    Pop-Location
}
