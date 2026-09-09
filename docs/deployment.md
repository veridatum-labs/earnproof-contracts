# Deployment and Initialization Guide

This document defines the required deployment order, initialization sequence, configuration prerequisites, and post-deployment verification procedure for the EarnProof contract system.

## Overview

The EarnProof system consists of three Soroban smart contracts that must be deployed and initialized in a specific order due to cross-contract dependencies. Each contract enforces permanent initialization that cannot be repeated or modified after initial deployment.

### Contract Dependencies

```
protocol-config (standalone)
         ↑
         │
    issuer-registry (standalone)
         ↑
         │
    proof-registry (depends on both above)
```

**Key principle:** Initialization is permanent and atomic. Once `initialize()` succeeds on a contract, that same contract cannot be re-initialized. The re-initialization guard checks for the presence of the Admin key and panics unconditionally if it exists, regardless of caller identity.

## Deployment Order

Contracts must be deployed in the following order. Deployment creates the contract on-chain but does NOT initialize it (initialization is a separate transaction).

### Step 1: Deploy protocol-config

Deploy the protocol-config contract code to Soroban. Capture its contract ID for later use.

```bash
stellar contract deploy \
  --source <admin-account> \
  --network testnet \
  --wasm target/wasm32v1-none/release/protocol_config.wasm
# Capture: PROTOCOL_CONFIG_ID = CC3OR...
```

### Step 2: Deploy issuer-registry

Deploy the issuer-registry contract code to Soroban. Capture its contract ID for later use.

```bash
stellar contract deploy \
  --source <admin-account> \
  --network testnet \
  --wasm target/wasm32v1-none/release/issuer_registry.wasm
# Capture: ISSUER_REGISTRY_ID = CB73T...
```

### Step 3: Deploy proof-registry

Deploy the proof-registry contract code to Soroban. Capture its contract ID for later use.

```bash
stellar contract deploy \
  --source <admin-account> \
  --network testnet \
  --wasm target/wasm32v1-none/release/proof_registry.wasm
# Capture: PROOF_REGISTRY_ID = CCMTA...
```

## Initialization Sequence

After all three contracts are deployed, they must be initialized in the following order. Initialization writes permanent state to each contract and enforces authorization through the admin account.

### Step 1: Initialize protocol-config

Initialize the protocol-config contract with the admin address. This establishes:
- `Admin`: the admin account address (requires auth from this account)
- `Paused`: false (protocol is not paused initially)
- `ConfigVersion`: 1 (initial config version counter)
- `ContractVersion`: 1 (initial contract code version)

```bash
stellar contract invoke \
  --source <admin-account> \
  --network testnet \
  --auth-mode root \
  --id PROTOCOL_CONFIG_ID \
  -- initialize --admin <admin-account-address>
```

**State written:**
- Admin address set (immutable, prevents re-initialization)
- ConfigVersion = 1 (increments on each configuration change)
- ContractVersion = 1 (monotonically increases on WASM upgrades)
- Paused = false

### Step 2: Approve initial schema version in protocol-config

Before any proofs can be registered, at least one schema version must be approved in protocol-config. Schema version 1 is the canonical first schema for this deployment.

```bash
stellar contract invoke \
  --source <admin-account> \
  --network testnet \
  --auth-mode root \
  --id PROTOCOL_CONFIG_ID \
  -- approve_schema_version --version 1
```

**Effect:**
- Schema version 1 is now approved and can be used in proof registrations
- ConfigVersion increments to 2

### Step 3: Initialize issuer-registry

Initialize the issuer-registry contract with the admin address. This establishes:
- `Admin`: the admin account address (requires auth from this account)
- `ContractVersion`: 1 (initial contract code version)

```bash
stellar contract invoke \
  --source <admin-account> \
  --network testnet \
  --auth-mode root \
  --id ISSUER_REGISTRY_ID \
  -- initialize --admin <admin-account-address>
```

**State written:**
- Admin address set (immutable, prevents re-initialization)
- ContractVersion = 1 (monotonically increases on WASM upgrades)

### Step 4: Register at least one issuer in issuer-registry

Before any proofs can be registered, at least one issuer must be registered in issuer-registry. Issuers are identified by a hash (`issuer_id_hash = sha256(issuer_id)`) and are associated with a Stellar address.

```bash
stellar contract invoke \
  --source <admin-account> \
  --network testnet \
  --auth-mode root \
  --id ISSUER_REGISTRY_ID \
  -- register_issuer \
      --issuer_id_hash <sha256-of-issuer-id> \
      --issuer_address <issuer-stellar-address> \
      --metadata_hash <sha256-of-issuer-metadata>
```

**Effect:**
- Issuer record created with status = Active
- Issuer can now register proofs in proof-registry

### Step 5: Initialize proof-registry

Initialize the proof-registry contract with the admin address and the contract IDs of its two dependencies. This establishes:
- `Admin`: the admin account address (requires auth from this account)
- `IssuerRegistry`: the issuer-registry contract ID (stored but NOT validated at init time)
- `ProtocolConfig`: the protocol-config contract ID (stored but NOT validated at init time)
- `ContractVersion`: 1 (initial contract code version)

```bash
stellar contract invoke \
  --source <admin-account> \
  --network testnet \
  --auth-mode root \
  --id PROOF_REGISTRY_ID \
  -- initialize \
      --admin <admin-account-address> \
      --issuer_registry ISSUER_REGISTRY_ID \
      --protocol_config PROTOCOL_CONFIG_ID
```

**State written:**
- Admin address set (immutable, prevents re-initialization)
- IssuerRegistry address stored (used at runtime for issuer validation)
- ProtocolConfig address stored (used at runtime for protocol pause/schema validation)
- ContractVersion = 1 (monotonically increases on WASM upgrades)

**Important:** Dependency addresses are stored during initialization but are only validated when proofs are actually registered. If the dependency contract IDs are incorrect or uninitialized at this point, proof registration will fail at runtime, not at initialization time.

## System is Now Operational

After completing all five steps above, the EarnProof contract system is fully initialized and operational. Issuers can begin registering proofs.

### Proof Registration

Issuers can now register proofs against the registered issuer address. The proof-registry contract will:
1. Check that the protocol is not paused (reads from protocol-config)
2. Check that the schema version is approved (reads from protocol-config)
3. Check that the issuer address is active (reads from issuer-registry)
4. Store the proof record

```bash
stellar contract invoke \
  --source <issuer-account> \
  --network testnet \
  --auth-mode root \
  --id PROOF_REGISTRY_ID \
  -- register_proof \
      --proof_id_hash <sha256-of-proof-id> \
      --commitment_hash <sha256-of-commitment> \
      --issuer_address <issuer-stellar-address> \
      --schema_version 1 \
      --expires_at <future-timestamp>
```

## Re-initialization Guard

**All three contracts have an absolute re-initialization guard:** once `initialize()` succeeds, the same contract cannot be re-initialized under any circumstances.

The guard is implemented as:

```rust
if env.storage().instance().has(&DataKey::Admin) {
    panic!("already initialized");
}
```

This means:
- Re-initialization by the same admin: **fails with panic**
- Re-initialization by a different admin: **fails with panic**
- Re-initialization with different dependency addresses: **fails with panic**
- The guard is checked **before** any authorization or validation

If initialization fails for any reason (validation error, partial write, etc.) and the Admin key is somehow partially written, the contract becomes permanently uninitialized and cannot be recovered. **Avoid partial initialization failure by ensuring all prerequisites are met before calling `initialize()`.**

## Dependency Validation Timing

Important distinction: **Dependency addresses are validated at runtime, not at initialization time.**

- `initialize()` stores the dependency addresses without validation
- If an address is incorrect, uninitialized, or points to the wrong contract type, proof operations will fail when they attempt to call the dependency
- This allows flexibility in deployment order at the cost of deferring error detection

Example: if you pass the issuer-registry address where the protocol-config address is expected, proof-registry will initialize successfully, but proof registration will fail with a runtime error when it tries to check protocol pause state.

## Configuration Prerequisites

Before proof registration is possible, certain configuration must be in place:

1. **protocol-config**: At least one schema version must be approved (e.g., version 1)
2. **issuer-registry**: At least one issuer must be registered and active
3. **proof-registry**: Must be initialized with both dependency addresses

If any of these prerequisites are missing, proof registration will fail with a runtime error.

## Contract Version Monotonicity

Each contract tracks a `ContractVersion` independently:

- Initialized to 1
- Increments on successful WASM upgrades via `upgrade_contract()`
- Prevents downgrade attacks (new version must be strictly greater than current)
- Does NOT auto-increment on configuration changes

The ConfigVersion in protocol-config (different from ContractVersion) increments on each configuration mutation (pause, unpause, schema approval, admin change) and is separate from the upgrade version counter.

## Summary: Correct Deployment Sequence

1. Deploy protocol-config, issuer-registry, proof-registry (any order)
2. Initialize protocol-config with admin
3. Approve schema version 1 in protocol-config
4. Initialize issuer-registry with admin
5. Register at least one issuer in issuer-registry
6. Initialize proof-registry with admin and dependency contract IDs

After step 6, the system is fully operational. Re-initialization is impossible on any contract.

## Post-Deployment Verification

Issue [#96](https://github.com/veridatum-labs/earnproof-contracts/issues/96) asks for a deployment verification script that validates contract deployment, initialization, and basic functionality. That script already exists — as `scripts/verify-manifest.ps1 -Live`, delivered by
[#7](https://github.com/veridatum-labs/earnproof-contracts/issues/7)
("Add live on-chain state checks to deployment verification").

Checked against #96's acceptance criteria directly:

| #96 requirement | Covered by `verify-manifest.ps1 -Live` |
|---|---|
| Verifies contract IDs in deployment manifest | Yes — offline shape checks (`Assert-ContractId`) run before any network call, in both modes |
| Validates admin addresses configured correctly | Yes — confirms `get_admin` on all three contracts matches the manifest's `admin` field |
| Tests basic operations (issuer lookup, schema check, pause state) | Yes — `is_paused`, `is_schema_version_approved` for every listed schema version, and `get_issuer_status` for the manifest's `initialIssuer` |
| Verifies cross-contract wiring | Yes — confirms `proof-registry`'s `get_issuer_registry`/`get_protocol_config` references match the manifest's contract IDs |
| Outputs clear success/failure messages | Yes — pass/fail per check, with expected-vs-actual values on mismatch |
| Never modifies state (read-only verification) | Yes — every call in `-Live` mode goes through `Invoke-StellarRead`, which only ever runs `stellar contract invoke -- <read-only-function>`; no signing key or seed phrase is required |
| Works with both testnet and sandbox | Yes — `-Network` is an explicit parameter, defaulting to the manifest's declared network; `scripts/deployment-manifest.testnet.json` is the checked-in example |

There is no gap here to duplicate. A second script (`scripts/verify-deployment.ps1`, matching #96's literal suggested filename) implementing the same checks against the same manifest shape would be **redundant with `verify-manifest.ps1 -Live`, not additive** — it would either drift out of sync with it over time, or the two would need to be kept manually consistent forever. Neither is better than having one verification path.

### Verification procedure

1. **Deploy and initialize**, following the sequence above, recording the manifest (contract IDs, WASM hashes, build
   metadata) per [`docs/compatibility.md`](compatibility.md)'s release
   requirements. Copy
   [`scripts/deployment-manifest.example.json`](../scripts/deployment-manifest.example.json)
   as a starting point.

2. **Offline validation** — always run this first, requires no network
   access:

   ```powershell
   pwsh -File scripts/verify-manifest.ps1 -Manifest <path-to-manifest>
   ```

   Confirms the manifest itself is well-formed: contract IDs and WASM
   hashes look like real Stellar values (not leftover placeholders),
   required fields are present, and — when `-Release` is also supplied — the
   release note's declared metadata matches the manifest.

3. **Live on-chain verification** — confirms the deployment actually came up
   correctly, not just that the manifest describing it is well-formed:

   ```powershell
   pwsh -File scripts/verify-manifest.ps1 -Manifest <path-to-manifest> -Live
   ```

   This is the read-only check #96 asks for. Example against the checked-in
   testnet manifest:

   ```powershell
   pwsh -File scripts/verify-manifest.ps1 `
     -Manifest scripts/deployment-manifest.testnet.json `
     -Live
   ```

   Optional parameters (all have sane defaults):
   - `-Network <name>` — overrides the manifest's declared network.
   - `-Source <identity>` — a read-only source identity for the CLI call; no
     signing key is required since every call is a read-only entry point.
   - `-CliPath <path>` — path to the `stellar` CLI binary, if not on `PATH`.
   - `-TimeoutSeconds <n>` / `-MaxRetries <n>` — transient-RPC-failure
     handling (connection resets, `502`/`503`/`504`) is retried automatically
     up to this many times before the check is reported as failed.

4. **On a mismatch**, the script reports the expected value (from the
   manifest) and the actual on-chain value side by side, and exits non-zero
   — safe to wire into a CI/CD gate that blocks promoting a deployment until
   verification passes.

### Sandbox vs. testnet

The same script and manifest shape work for both — `-Network` accepts
whatever network alias the `stellar` CLI has configured locally (e.g. a
local sandbox network started via
[`scripts/local-sandbox/`](../scripts/local-sandbox/), or `testnet`). There
is no sandbox-specific verification path, since the checks themselves (an
admin address, a pause flag, a schema approval, a cross-contract reference)
mean the same thing regardless of which network they're read from.
