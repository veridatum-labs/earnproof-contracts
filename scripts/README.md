# Deployment Scripts

These scripts provide a reproducible Stellar testnet deployment path for the EarnProof Soroban contracts.

## Prerequisites

- Rust toolchain `1.92.0` from `rust-toolchain.toml`
- `wasm32v1-none` target support
- Stellar CLI available as `stellar` (`cargo install --locked stellar-cli`, version matching the pinned `soroban-sdk`)
- A funded Stellar testnet identity configured in Stellar CLI
- No secret keys committed to the repository

## Build Provenance

```powershell
.\scripts\build-release.ps1 -ReproducibilityChecks 2 -Output artifacts\provenance.json
```

This script builds release WASM artifacts, records toolchain versions, source commit, artifact sizes, and SHA-256 hashes in a provenance manifest, and performs reproducibility checks by building multiple times and comparing hashes.

```powershell
.\scripts\verify-provenance.ps1 -Provenance artifacts\provenance.json
```

This script verifies that current WASM artifacts match the recorded provenance manifest. It throws on hash or size mismatches.

```powershell
.\scripts\test-provenance.ps1
```

This script runs the full provenance test suite: reproducibility checks, hash-tampering detection, and stale-artifact detection. Pass `-SkipBuild` to run only the verification tests against an existing provenance manifest (used by CI after `build-release.ps1`):

```powershell
.\scripts\test-provenance.ps1 -SkipBuild
```

The provenance manifest maps each WASM artifact to its source commit, Rust/Cargo/Stellar CLI versions, artifact size, and SHA-256. It also records whether the working tree was dirty at build time and a list of identified non-determinism risks. It contains no deployer keys or secrets.

## Build and Deploy

```powershell
.\scripts\deploy-testnet.ps1 -Source deployer -Admin G... -IssuerAddress G... -Output scripts\deployment-manifest.testnet.json
```

By default, Stellar CLI deploy and invoke calls are retried up to five times for transient RPC transport failures such as connection resets, send failures, timeouts, temporary unavailability, and sequence races. Override this with `-MaxRetries` when needed:

```powershell
.\scripts\deploy-testnet.ps1 -Source deployer -Admin G... -IssuerAddress G... -MaxRetries 8
```

The script:

- installs the `wasm32v1-none` target if needed;
- builds optimized release WASM artifacts with `stellar contract build`;
- verifies artifacts against provenance manifest if present;
- deploys `protocol-config`, `issuer-registry`, and `proof-registry`;
- initializes each contract;
- approves schema version `1`;
- registers the backend issuer address before proof anchoring is enabled;
- writes a manifest with contract IDs, WASM hashes, admin address, schema versions, and CLI command evidence.

## Verify Manifest

```powershell
.\scripts\verify-manifest.ps1 -Manifest scripts\deployment-manifest.testnet.json
```

For the checked-in example manifest:

```powershell
.\scripts\verify-manifest.ps1 -Manifest scripts\deployment-manifest.example.json -AllowPlaceholders
```

The verifier checks the manifest shape and rejects placeholder contract IDs unless `-AllowPlaceholders` is explicitly supplied.
