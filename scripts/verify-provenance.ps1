param(
  [Parameter(Mandatory = $true)]
  [string]$Provenance,

  [switch]$AllowMismatch
)

$ErrorActionPreference = "Stop"

function Assert-Sha256($Name, $Expected, $Actual) {
  if ($Expected -ne $Actual) {
    if ($AllowMismatch) {
      Write-Warning "$Name hash mismatch: expected $Expected, got $Actual"
      return $false
    }
    throw "$Name hash mismatch: expected $Expected, got $Actual"
  }
  return $true
}

$path = Resolve-Path $Provenance
$manifestJson = Get-Content $path -Raw | ConvertFrom-Json

if (-not $manifestJson.artifacts) {
  throw "Provenance manifest is missing artifacts."
}

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $root

try {
  $allMatch = $true

  if ($manifestJson.sourceCommit -and -not $AllowMismatch) {
    $headCommit = git rev-parse HEAD
    if ($manifestJson.sourceCommit -ne $headCommit) {
      throw "Provenance sourceCommit ($($manifestJson.sourceCommit)) does not match current HEAD ($headCommit). Stale provenance or uncommitted source changes."
    }
  }

  if ($manifestJson.sourceTreeDirty) {
    Write-Warning "Provenance was generated from a working tree with uncommitted changes (sourceTreeDirty = true)."
  }
  foreach ($artifact in $manifestJson.artifacts) {
    $artifactPath = Join-Path $root $artifact.path
    if (-not (Test-Path $artifactPath)) {
      if ($AllowMismatch) {
        Write-Warning "$($artifact.contract) artifact not found at $($artifact.path)"
        $allMatch = $false
        continue
      }
      throw "$($artifact.contract) artifact not found at $($artifact.path)"
    }

    $actualHash = (Get-FileHash -Path $artifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $actualSize = (Get-Item $artifactPath).Length

    $hashMatch = Assert-Sha256 $artifact.contract $artifact.sha256 $actualHash
    if (-not $hashMatch) {
      $allMatch = $false
    }

    if ($artifact.size -ne $actualSize) {
      if ($AllowMismatch) {
        Write-Warning "$($artifact.contract) size mismatch: expected $($artifact.size), got $actualSize"
        $allMatch = $false
        continue
      }
      throw "$($artifact.contract) size mismatch: expected $($artifact.size), got $actualSize"
    }
  }

  if ($allMatch) {
    Write-Host "Provenance verification passed: $path"
    exit 0
  } elseif ($AllowMismatch) {
    Write-Warning "Provenance verification completed with mismatches: $path"
    exit 1
  }
}
finally {
  Pop-Location
}
