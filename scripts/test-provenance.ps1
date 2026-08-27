param(
  [string]$Provenance = "artifacts/provenance.json",
  [int]$ReproducibilityChecks = 2,
  [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$backupPath = $null
$restorePath = $null
Push-Location $root

try {
  if (-not $SkipBuild) {
    Write-Host "=== Step 1: Build provenance manifest with reproducibility checks ==="
    & (Join-Path $PSScriptRoot "build-release.ps1") -Output $Provenance -ReproducibilityChecks $ReproducibilityChecks -Clean
  } else {
    Write-Host "=== Step 1: Skipping build (-SkipBuild), reusing existing provenance ==="
  }

  if (-not (Test-Path $Provenance)) {
    throw "Provenance manifest was not created at $Provenance"
  }

  Write-Host "=== Step 2: Verify provenance matches built artifacts ==="
  & (Join-Path $PSScriptRoot "verify-provenance.ps1") -Provenance $Provenance

  Write-Host "=== Step 3: Test hash-tampering detection ==="
  $tamperedPath = "$Provenance.tampered.json"
  Copy-Item $Provenance $tamperedPath
  $tamperedJson = Get-Content $tamperedPath -Raw | ConvertFrom-Json
  $originalHash = $tamperedJson.artifacts[0].sha256
  $replacement = if ($originalHash.Substring(0, 1) -eq "0") { "1" } else { "0" }
  $tamperedHash = $replacement + $originalHash.Substring(1)
  $tamperedJson.artifacts[0].sha256 = $tamperedHash
  $tamperedJson | ConvertTo-Json -Depth 10 | Set-Content -Path $tamperedPath -Encoding UTF8

  $tamperedResult = $false
  try {
    & (Join-Path $PSScriptRoot "verify-provenance.ps1") -Provenance $tamperedPath -AllowMismatch
    $tamperedResult = $LASTEXITCODE -ne 0
  } catch {
    $tamperedResult = $true
  }

  if (-not $tamperedResult) {
    throw "Hash-tampering test failed: verification did not detect the tampered hash."
  }
  Write-Host "Hash-tampering test passed."

  Write-Host "=== Step 4: Test stale-artifact detection ==="
  $stalePath = "$Provenance.stale.json"
  Copy-Item $Provenance $stalePath
  $staleJson = Get-Content $stalePath -Raw | ConvertFrom-Json
  $restorePath = Join-Path $root $staleJson.artifacts[0].path
  if (Test-Path $restorePath) {
    $backupPath = "$restorePath.provenance-tests.bak"
    Remove-Item $backupPath -Force -ErrorAction SilentlyContinue
    Copy-Item $restorePath $backupPath
    Remove-Item $restorePath -Force
  }

  $staleResult = $false
  try {
    & (Join-Path $PSScriptRoot "verify-provenance.ps1") -Provenance $stalePath -AllowMismatch
    $staleResult = $LASTEXITCODE -ne 0
  } catch {
    $staleResult = $true
  }

  if (-not $staleResult) {
    throw "Stale-artifact test failed: verification did not detect the missing artifact."
  }
  Write-Host "Stale-artifact test passed."

  Write-Host "=== All provenance tests passed ==="
}
finally {
  if ($backupPath -and (Test-Path $backupPath)) {
    Move-Item $backupPath $restorePath -Force
  }
  Remove-Item "$Provenance.tampered.json" -Force -ErrorAction SilentlyContinue
  Remove-Item "$Provenance.stale.json" -Force -ErrorAction SilentlyContinue
  Pop-Location
}