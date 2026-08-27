param(
  [string]$Output = "artifacts/provenance.json",
  [int]$ReproducibilityChecks = 2,
  [switch]$Clean
)

$ErrorActionPreference = "Stop"

function Assert-Command($Name) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Required command '$Name' was not found."
  }
}

function Get-Sha256($Path) {
  return (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-FileSize($Path) {
  return (Get-Item $Path).Length
}

function Get-ToolchainVersion($Command, $Arguments) {
  $previousEap = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try {
    $output = & $Command @Arguments 2>&1
    $exitCode = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $previousEap
  }
  if ($exitCode -ne 0) {
    return "unknown"
  }
  return (($output | ForEach-Object { Format-CapturedLine $_ }) -join " ").Trim()
}

function Format-CapturedLine($Item) {
  if ($Item -is [System.Management.Automation.ErrorRecord]) {
    return $Item.Exception.Message
  }
  return "$Item"
}

function ConvertTo-RelativePath($Root, $Path) {
  $rootPrefix = $Root.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
  if (-not $Path.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Path '$Path' is outside of repository root '$Root'."
  }
  return $Path.Substring($rootPrefix.Length).Replace("\", "/")
}

function Get-LockedPackageVersion($LockFile, $Package) {
  $lock = [IO.File]::ReadAllText($LockFile)
  $m = [regex]::Match($lock, 'name = "' + [regex]::Escape($Package) + '"' + "\r?\nversion = `"([^`"]+)`"")
  if ($m.Success) {
    return $m.Groups[1].Value
  }
  return "unknown"
}

function Invoke-Step($Description, $Command) {
  Write-Host "==> $Description"
  $previousEap = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try {
    $output = & $Command[0] @($Command | Select-Object -Skip 1) 2>&1
    $exitCode = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $previousEap
  }
  if ($exitCode -ne 0) {
    if ($output) {
      $output | ForEach-Object { Write-Warning (Format-CapturedLine $_) }
    }
    throw "Command failed: $Description"
  }
  if ($output) {
    $output | ForEach-Object { Write-Host (Format-CapturedLine $_) }
  }
}

Assert-Command "cargo"
Assert-Command "rustup"
Assert-Command "stellar"
Assert-Command "git"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $root

try {
  Invoke-Step "Install Stellar WASM target" @("rustup", "target", "add", "wasm32v1-none")

  $rustVersion = Get-ToolchainVersion "rustc" @("--version")
  $cargoVersion = Get-ToolchainVersion "cargo" @("--version")
  $stellarVersion = Get-ToolchainVersion "stellar" @("--version")
  $commit = git rev-parse HEAD
  $dirty = @(git status --porcelain)
  if ($dirty.Count -gt 0) {
    Write-Warning "Working tree is not clean. Provenance records the source commit but the build may include uncommitted changes."
  }
  $sorobanSdkVersion = Get-LockedPackageVersion (Join-Path $root "Cargo.lock") "soroban-sdk"

  $wasmRoot = Join-Path $root "target/wasm32v1-none/release"
  $artifacts = @(
    @{ contract = "protocol-config"; package = "protocol-config"; path = Join-Path $wasmRoot "protocol_config.wasm" },
    @{ contract = "issuer-registry"; package = "issuer-registry"; path = Join-Path $wasmRoot "issuer_registry.wasm" },
    @{ contract = "proof-registry"; package = "proof-registry"; path = Join-Path $wasmRoot "proof_registry.wasm" }
  )

  $buildResults = @{}
  $reproducibilityResults = @()

  for ($i = 1; $i -le $ReproducibilityChecks; $i++) {
    if ($Clean -or $ReproducibilityChecks -gt 1) {
      Invoke-Step "Clean build artifacts (check $i)" @("cargo", "clean")
      if (Test-Path $wasmRoot) {
        Remove-Item $wasmRoot -Recurse -Force
      }
    }

    Invoke-Step "Build release WASM artifacts (check $i)" @("stellar", "contract", "build")

    $checkResult = [ordered]@{
      build = $i
      timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
      artifacts = @{}
    }

    foreach ($artifact in $artifacts) {
      if (-not (Test-Path $artifact.path)) {
        throw "Expected WASM artifact was not found: $($artifact.path)"
      }
      $sha256 = Get-Sha256 $artifact.path
      $size = Get-FileSize $artifact.path
      $checkResult.artifacts[$artifact.contract] = $sha256

      if ($buildResults.ContainsKey($artifact.contract)) {
        if ($buildResults[$artifact.contract].sha256 -ne $sha256) {
          throw "Reproducibility check $i failed: $($artifact.contract) hash mismatch. Expected $($buildResults[$artifact.contract].sha256), got $sha256"
        }
      } else {
        $buildResults[$artifact.contract] = @{ path = $artifact.path; sha256 = $sha256; size = $size }
      }
    }

    $reproducibilityResults += $checkResult
    Write-Host "Reproducibility check $i passed."
  }

  $manifestArtifacts = @()
  foreach ($artifact in $artifacts) {
    $manifestArtifacts += [ordered]@{
      contract = $artifact.contract
      package = $artifact.package
      path = ConvertTo-RelativePath $root $artifact.path
      size = $buildResults[$artifact.contract].size
      sha256 = $buildResults[$artifact.contract].sha256
    }
  }

  $manifest = [ordered]@{
    schemaVersion = "1.0"
    generatedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    sourceCommit = $commit
    sourceTreeDirty = ($dirty.Count -gt 0)
    toolchain = [ordered]@{
      rust = $rustVersion
      cargo = $cargoVersion
      sorobanCli = $stellarVersion
      target = "wasm32v1-none"
    }
    sdk = [ordered]@{
      sorobanSdk = $sorobanSdkVersion
    }
    artifacts = $manifestArtifacts
    reproducibility = [ordered]@{
      checks = $ReproducibilityChecks
      results = $reproducibilityResults
    }
    nonDeterminismRisks = @(
      "Incremental compilation can produce different binaries between builds. Mitigation: clean target directory before each build.",
      "System time embedded in debug symbols. Mitigation: use release builds.",
      "Environment-specific path metadata in debug info. Mitigation: build from consistent paths.",
      "Non-deterministic linker behavior. Mitigation: pinned Rust toolchain version."
    )
    notes = "Generated by build-release.ps1. Do not edit manually."
  }

  $outputPath = Join-Path $root $Output
  $manifest | ConvertTo-Json -Depth 10 | Set-Content -Path $outputPath -Encoding UTF8
  Write-Host "Wrote provenance manifest: $outputPath"
}
finally {
  Pop-Location
}
