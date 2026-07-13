# EarnProof Contracts

Soroban contract starter for EarnProof.

## Phase 0 Included

- Rust workspace
- Shared on-chain types
- `protocol-config` contract shell
- `issuer-registry` contract shell
- `proof-registry` contract shell
- Contract crates that compile against `soroban-sdk`

## Local Commands

```bash
cargo test
cargo build
```

The current starter intentionally avoids `soroban-sdk/testutils` because the
latest SDK test utility dependency is not compiling cleanly against this local
Rust toolchain. Contract unit tests should be added once the SDK/toolchain pair
is pinned for the project.

## Privacy Boundary

Contracts must not store exact income, raw wallet history, personal names,
emails, employment documents, or complete transaction lists. Store hashes,
status, schema version, issuer address, expiration, and timestamps only.
