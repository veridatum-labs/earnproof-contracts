# EarnProof Contracts

EarnProof is an open-source, privacy-focused income and payment verification protocol built on Stellar.

This repository contains the Soroban contracts that support issuer trust, proof commitments, revocation status, and protocol configuration for EarnProof.

## Product Role

The contracts provide public status and trust primitives without storing private income data on-chain.

Contracts should answer questions such as:

- Is this issuer active?
- Was this proof commitment registered?
- Has this proof been revoked?
- Is this schema version approved?
- Are sensitive protocol operations paused?

Contracts must not calculate income, store salaries, store raw payment history, or custody user funds.

## Current Scope

Implemented:

- Rust workspace
- Shared on-chain record types
- `protocol-config` contract
- `issuer-registry` contract shell
- `proof-registry` contract shell
- Buildable contract crates against `soroban-sdk`

The `protocol-config` contract currently supports:

- `initialize`
- `get_admin`
- `set_admin`
- `pause`
- `unpause`
- `is_paused`
- `approve_schema_version`
- `deprecate_schema_version`
- `is_schema_version_approved`
- `get_config_version`

Planned next:

- Complete issuer registry status transitions
- Complete proof registry registration and revocation rules
- Add deployment scripts
- Add testnet deployment manifest
- Add backend integration notes
- Add event types using the current Soroban event macro style
- Add contract tests once the SDK/toolchain pair is pinned

## Tech Stack

- Rust
- Soroban SDK
- Stellar testnet

## Repository Structure

```text
contracts/
  issuer-registry/
  proof-registry/
  protocol-config/
packages/
  shared/
scripts/
tests/
docs/
```

## Local Setup

```bash
cargo build
cargo test
```

Formatting:

```bash
cargo fmt --all --check
```

## Toolchain Note

This starter currently avoids `soroban-sdk/testutils` because the latest SDK test utility dependency is not compiling cleanly against this local Rust toolchain. Contract unit tests should be added after the SDK and Rust toolchain versions are pinned together.

The current contract event calls compile, but the SDK warns that raw `env.events().publish` is deprecated. The next contract cleanup should move events to the current `#[contractevent]` macro style.

## On-Chain Privacy Boundary

Contracts must not store:

- Exact salary
- Exact payment amount
- Full wallet history
- Personal name
- Email address
- Employment documents
- Raw transaction lists
- Unencrypted personal information

Contracts may store:

- Proof ID hash
- Commitment hash
- Issuer address
- Status
- Expiration
- Schema version
- Timestamp
- Public metadata hash

## Security Requirements

- Authorization checks on every state mutation.
- Duplicate registration prevention.
- Status transitions must be explicit.
- Proof validity must respect expiration and revocation.
- Issuer-backed proof operations must reject inactive issuers.
- Sensitive operations should respect protocol pause state.
- Mainnet deployment should wait for independent review.

## Related Repositories

- `earnproof-frontend`: Public app, worker dashboard, issuer UI, verifier UI, and admin UI.
- `earnproof-backend`: API, payment indexing, proof generation, credential signing, and verification.
- `earnproof-sdk`: Future TypeScript SDK for integrations.
- `earnproof-specification`: Future credential and verification standard.

