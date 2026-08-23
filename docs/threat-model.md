# EarnProof Contracts — Threat Model and Security Review Checklist

This document covers the trust model, threat surface, implemented controls, open gaps, and mainnet release gates for the three Soroban contracts in this repository: `protocol-config`, `issuer-registry`, and `proof-registry`.

It is tied to the code at `contracts/*/src/lib.rs` and `packages/shared/src/lib.rs`. Claims reference specific functions or tests rather than general assertions.

---

## 1. Assets

| Asset | Where stored | Sensitivity |
|---|---|---|
| Admin address (`DataKey::Admin`) | Instance storage, all three contracts | Critical — controls all privileged mutations |
| Pause flag (`DataKey::Paused`) | Instance storage, `protocol-config` | High — halts proof registration protocol-wide |
| Schema version approval (`DataKey::SchemaVersion(u32)`) | Persistent storage, `protocol-config` | High — gates which proof schemas are accepted |
| Config version counter (`DataKey::ConfigVersion`) | Instance storage, `protocol-config` | Low — monotonic audit counter |
| Issuer record (`DataKey::Issuer(BytesN<32>)`) | Persistent storage, `issuer-registry` | High — determines who may register proofs |
| Address→issuer mapping (`DataKey::AddressIssuer(Address)`) | Persistent storage, `issuer-registry` | High — cross-contract lookup used by proof-registry |
| Proof record (`DataKey::Proof(BytesN<32>)`) | Persistent storage, `proof-registry` | High — proof existence and revocation status |
| Cross-contract addresses (`DataKey::IssuerRegistry`, `DataKey::ProtocolConfig`) | Instance storage, `proof-registry` | High — if stale, proof-registry calls wrong contracts |

No contract stores exact income, payment amounts, personal names, email addresses, wallet history, or raw transaction lists. See [Section 6](#6-privacy-analysis) for a full analysis.

---

## 2. Actors and Trust Assumptions

### 2.1 Admin

A single `Address` stored in each contract's instance storage. Set at `initialize` time and transferable via `set_admin` (protocol-config) or implicit replacement on re-initialization (prevented by the already-initialized guard).

Trust level: fully trusted for all privileged state mutations.

Assumed to be:
- A multisig or hardware-secured account before mainnet.
- Replaced with a multisig or governance address as part of the mainnet release gate (see [Section 9](#9-mainnet-release-gates)).

Risk: a single compromised EOA acting as admin can pause the protocol, revoke all issuers, deprecate all schemas, or transfer admin to an attacker-controlled address. This is the highest-impact single point of failure.

### 2.2 Issuer

An `Address` registered by admin in `issuer-registry` with an associated `issuer_id_hash`. Issuers are the only accounts authorized to call `register_proof` and `revoke_proof` (non-admin path).

Trust level: trusted for proof submission under their own address; not trusted for admin operations.

Assumed to be a backend service account (`earnproof-backend`) whose signing key is managed separately from the contract admin key.

### 2.3 Backend service

The EarnProof API (`earnproof-backend`). Performs off-chain hashing of credentials, submits `register_proof` and `revoke_proof` transactions signed by the issuer key.

Trust level: trusted to produce valid hashes per the rules in `docs/backend-integration.md`. Not trusted to bypass on-chain authorization.

Hashing contract (`sha256`) is performed off-chain. The backend is responsible for:
- Computing `proof_id_hash = sha256(proof_id)`
- Computing `commitment_hash = sha256(canonical_credential_payload_without_signature)`
- Computing `issuer_id_hash = sha256(issuer_id)`
- Not sending personal data on-chain.

### 2.4 Verifier / read-only caller

Any party reading public contract state (verifiers, frontends, explorers). No write access. All read functions are permissionless.

Trust level: untrusted; they receive only public hashed data.

### 2.5 Stellar network and validators

The underlying consensus layer. Assumed to be honest in aggregate (Stellar SCP). Transaction ordering within a ledger is not under application control.

### 2.6 Deployment operator

The account identified by `$Source` in `scripts/deploy-testnet.ps1`. Deploys WASM and calls `initialize` functions. After initialization the deployer has no special on-chain power beyond what the admin key controls.

---

## 3. Trust Boundaries and Cross-Contract Calls

```
[Backend/Issuer]
      |
      | register_proof / revoke_proof
      v
[proof-registry] --is_paused()--> [protocol-config]
      |          --is_schema_version_approved()-->
      |          --is_active_address()--> [issuer-registry]
      |
      | get_proof / is_valid_proof / is_revoked (read-only)
      v
[Verifier / Frontend]

[Admin]
  |---> [protocol-config]: pause, unpause, set_admin, approve_schema_version, deprecate_schema_version
  |---> [issuer-registry]: register_issuer, update_issuer, suspend_issuer, reactivate_issuer, revoke_issuer, rotate_issuer_address
  |---> [proof-registry]: admin_revoke_proof, get_admin, get_issuer_registry, get_protocol_config
```

The addresses of `issuer-registry` and `protocol-config` are stored in `proof-registry` instance storage at `initialize` time (`DataKey::IssuerRegistry`, `DataKey::ProtocolConfig`). There is no on-chain upgrade path to change these references after deployment. A redeployment of dependent contracts requires redeploying `proof-registry` and repointing references.

Cross-contract call surface in `proof-registry::register_proof` (`contracts/proof-registry/src/lib.rs`):
- `ProtocolConfigContractClient::is_paused()`
- `ProtocolConfigContractClient::is_schema_version_approved(&schema_version)`
- `IssuerRegistryContractClient::is_active_address(&issuer_address)`

These are synchronous calls within the same transaction. There is no callback or re-entrancy path — Soroban does not support re-entrant cross-contract calls in the same way EVM does, but each cross-contract call executes in its own host function frame.

---

## 4. Entry Points

### 4.1 `protocol-config`

| Function | Auth required | State written |
|---|---|---|
| `initialize(admin)` | `admin.require_auth()` | Admin, Paused=false, ConfigVersion=1 |
| `set_admin(new_admin)` | current `admin.require_auth()` | Admin, ConfigVersion++ |
| `pause()` | `admin.require_auth()` | Paused=true, ConfigVersion++ |
| `unpause()` | `admin.require_auth()` | Paused=false, ConfigVersion++ |
| `approve_schema_version(version)` | `admin.require_auth()` | SchemaVersion(v)=true, ConfigVersion++ |
| `deprecate_schema_version(version)` | `admin.require_auth()` | SchemaVersion(v)=false, ConfigVersion++ |
| `get_admin` | none | — |
| `is_paused` | none | — |
| `is_schema_version_approved` | none | — (extends TTL) |
| `get_config_version` | none | — |

### 4.2 `issuer-registry`

| Function | Auth required | State written |
|---|---|---|
| `initialize(admin)` | `admin.require_auth()` | Admin |
| `register_issuer(id_hash, address, metadata_hash)` | `admin.require_auth()` | IssuerRecord, AddressIssuer mapping |
| `update_issuer(id_hash, metadata_hash)` | `admin.require_auth()` | IssuerRecord.metadata_hash, updated_at |
| `suspend_issuer(id_hash)` | `admin.require_auth()` | IssuerRecord.status=Suspended |
| `reactivate_issuer(id_hash)` | `admin.require_auth()` | IssuerRecord.status=Active |
| `revoke_issuer(id_hash)` | `admin.require_auth()` | IssuerRecord.status=Revoked |
| `rotate_issuer_address(id_hash, new_address)` | `admin.require_auth()` | Old mapping removed, new mapping written |
| `get_issuer` | none | — (extends TTL) |
| `is_active_issuer` | none | — (extends TTL) |
| `is_active_address` | none | — (extends TTL) |
| `get_issuer_by_address` | none | — (extends TTL) |

### 4.3 `proof-registry`

| Function | Auth required | State written |
|---|---|---|
| `initialize(admin, issuer_registry, protocol_config)` | `admin.require_auth()` | Admin, IssuerRegistry, ProtocolConfig refs |
| `register_proof(proof_id_hash, commitment_hash, issuer_address, schema_version, expires_at)` | `issuer_address.require_auth()` | ProofRecord |
| `revoke_proof(proof_id_hash)` | `record.issuer_address.require_auth()` | ProofRecord.status=Revoked |
| `admin_revoke_proof(proof_id_hash)` | `admin.require_auth()` | ProofRecord.status=Revoked |
| `get_proof` | none | — (extends TTL) |
| `is_valid_proof` | none | — (extends TTL via get_proof) |
| `is_revoked` | none | — (extends TTL via get_proof) |
| `get_admin` | none | — |
| `get_issuer_registry` | none | — |
| `get_protocol_config` | none | — |

---

## 5. Threat Enumeration

Each threat entry includes: description, affected entry points, implemented control, status, and required action or test.

---

### T-01 — Unauthorized admin operation

**Description:** An attacker calls a privileged function (`pause`, `register_issuer`, `admin_revoke_proof`, etc.) without holding the admin signing key.

**Affected entry points:** All admin-gated functions across all three contracts.

**Implemented control:** Every state-mutating function calls `Self::require_auth(&admin)` before reading or writing state. `require_auth` calls `address.require_auth()` which invokes the Soroban host authorization check — the transaction must carry a valid signature from that address.

**Code reference:** `issuer-registry/src/lib.rs` `register_issuer`, `set_status`; `protocol-config/src/lib.rs` `pause`, `set_admin`; `proof-registry/src/lib.rs` `set_revoked` (admin path).

**Test coverage:** All contract tests run with `env.mock_all_auths()`. Authorization is exercised via mocked auth rather than bypassed — the `require_auth` call is present in production code and mocked in tests per Soroban SDK convention.

**Status:** Implemented. Test coverage via mocked auth.

**Open gap:** Tests do not assert that calls fail when the required auth is absent (negative auth tests). Add tests that omit `mock_all_auths` and confirm unauthorized calls panic.

---

### T-02 — Admin key compromise

**Description:** The admin signing key is stolen. Attacker can pause the protocol, revoke all issuers, deprecate all schemas, transfer admin to themselves, and perform administrative proof revocations.

**Affected entry points:** All admin-gated functions.

**Implemented control:** None beyond Stellar account security. The contracts have no time-lock, no multisig enforcement at the contract layer, and no secondary confirmation step.

**Status:** Open risk. Accepted for testnet. Mainnet blocker — see [Section 9, Gate M-02](#m-02-admin-custody).

**Required action:** Before mainnet, the admin address must be a Stellar multisig account (threshold ≥ 2) or a governance contract. Document the key custody procedure.

---

### T-03 — Re-initialization attack

**Description:** An attacker calls `initialize` a second time to overwrite the admin address.

**Affected entry points:** `initialize` on all three contracts.

**Implemented control:** All three `initialize` functions check `env.storage().instance().has(&DataKey::Admin)` and panic with `"already initialized"` if true.

**Code reference:** `issuer-registry/src/lib.rs:20`, `proof-registry/src/lib.rs:37`, `protocol-config/src/lib.rs:54`.

**Status:** Implemented.

---

### T-04 — Duplicate proof registration (replay)

**Description:** The backend or an attacker submits the same `proof_id_hash` twice, potentially overwriting or duplicating an existing proof commitment.

**Affected entry points:** `register_proof`.

**Implemented control:** `proof-registry::register_proof` checks `env.storage().persistent().has(&key)` and panics with `"proof already registered"` before writing.

**Code reference:** `proof-registry/src/lib.rs` `register_proof`.

**Test coverage:** `rejects_duplicate_proof_id` in `proof-registry/src/lib.rs`.

**Status:** Implemented.

---

### T-05 — Duplicate issuer registration

**Description:** Admin accidentally or maliciously registers the same issuer ID hash or issuer address twice.

**Affected entry points:** `register_issuer`.

**Implemented control:** Checks both `DataKey::Issuer(issuer_id_hash)` and `DataKey::AddressIssuer(issuer_address)` for existence before writing. Panics with `"issuer already registered"` or `"issuer address already registered"`.

**Code reference:** `issuer-registry/src/lib.rs` `register_issuer`.

**Test coverage:** `rejects_duplicate_issuer_id` in `issuer-registry/src/lib.rs`.

**Status:** Implemented.

**Open gap:** There is no test for duplicate address registration (second `register_issuer` call with same address but different ID hash). Add a test for this path.

---

### T-06 — Malicious issuer submits proof

**Description:** An unregistered, suspended, or revoked issuer address calls `register_proof`.

**Affected entry points:** `register_proof`.

**Implemented control:** `register_proof` calls `IssuerRegistryContractClient::is_active_address(&issuer_address)` and panics with `"issuer is not active"` if the result is false. An unregistered address will panic in `is_active_address` because `get` on a missing key panics with `"issuer address not found"`.

**Code reference:** `proof-registry/src/lib.rs` `register_proof`; `issuer-registry/src/lib.rs` `is_active_address`.

**Test coverage:** `rejects_inactive_issuer_address` in `proof-registry/src/lib.rs`.

**Status:** Implemented.

**Note:** The panic message for an unregistered address is `"issuer address not found"` rather than `"issuer is not active"`. Both prevent proof registration. The distinction is observable by callers; document this in the backend integration guide.

---

### T-07 — Proof submitted while protocol is paused

**Description:** A proof is registered during an emergency pause window.

**Affected entry points:** `register_proof`.

**Implemented control:** `register_proof` calls `ProtocolConfigContractClient::is_paused()` and panics with `"protocol is paused"` if true.

**Code reference:** `proof-registry/src/lib.rs` `register_proof`.

**Test coverage:** `rejects_registration_when_protocol_is_paused` in `proof-registry/src/lib.rs`.

**Status:** Implemented.

---

### T-08 — Proof submitted with unapproved schema version

**Description:** The backend submits a proof using a schema version that has been deprecated or never approved.

**Affected entry points:** `register_proof`.

**Implemented control:** `register_proof` calls `ProtocolConfigContractClient::is_schema_version_approved(&schema_version)` and panics with `"schema version is not approved"` if false. The zero-version guard (`schema_version == 0` panics) is also in place.

**Code reference:** `proof-registry/src/lib.rs` `register_proof`.

**Test coverage:** `rejects_unapproved_schema_version` in `proof-registry/src/lib.rs`.

**Status:** Implemented.

---

### T-09 — Expired proof accepted as valid

**Description:** A verifier queries `is_valid_proof` for a proof whose `expires_at` timestamp has passed and receives `true`.

**Affected entry points:** `is_valid_proof`, `register_proof`.

**Implemented control (registration side):** `register_proof` checks `expires_at <= env.ledger().timestamp()` and panics with `"proof expiration must be in the future"`.

**Implemented control (read side):** `is_valid_proof` returns `record.status == ProofStatus::Active && env.ledger().timestamp() <= record.expires_at`. An expired but non-revoked proof correctly returns `false`.

**Code reference:** `proof-registry/src/lib.rs` `register_proof`, `is_valid_proof`.

**Test coverage:** `rejects_expired_proof` covers the registration guard. The read-side expiry path is not explicitly tested by a test that advances the ledger clock past `expires_at`. Add a test that advances `env.ledger().set_timestamp()` past the expiration and asserts `is_valid_proof` returns `false`.

**Status:** Partially implemented. Registration guard tested; read-path expiry test is missing.

---

### T-10 — TTL expiry causes data loss

**Description:** Ledger storage entries expire (TTL reaches zero) and are evicted. Proof or issuer records become inaccessible, which could cause a live issuer to appear unregistered or a valid proof to appear non-existent.

**Affected assets:** All persistent storage entries; instance storage entries.

**Implemented control:** Every write and read operation extends TTL to `TTL_EXTEND_TO_LEDGERS = 500_000` ledgers when the remaining TTL falls below `TTL_THRESHOLD_LEDGERS = 50_000`. Both `IssuerRecord` and `ProofRecord` keys have TTL extended on registration and on every read.

**Code reference:** `packages/shared/src/lib.rs` constants; TTL extension calls in all three contracts; `extends_issuer_storage_ttl` and `extends_proof_storage_ttl` tests.

**Test coverage:** `extends_issuer_storage_ttl` and `extends_proof_storage_ttl` assert that TTL exceeds the threshold after registration.

**Status:** Implemented.

**Open gap:** Instance storage TTL is extended at initialize time and on every config write, but there is no test asserting instance TTL is set after initialization. If the instance entry expires, `get_admin` panics. Add an instance TTL test for `protocol-config` and `issuer-registry`. Additionally, consider a background process (archival/restore) or backend monitoring to catch entries whose TTL is approaching the threshold.

---

### T-11 — Revoked issuer reactivated

**Description:** A revoked issuer is mistakenly reactivated, allowing it to register proofs again.

**Affected entry points:** `reactivate_issuer`.

**Implemented control:** `set_status` checks `record.status == IssuerStatus::Revoked && status != IssuerStatus::Revoked` and panics with `"revoked issuer cannot be reactivated"`. Revocation is permanent.

**Code reference:** `issuer-registry/src/lib.rs` `set_status`.

**Test coverage:** `revoked_issuer_cannot_be_reactivated` and `status_transitions_reject_reactivated_revoked_issuer`.

**Status:** Implemented.

---

### T-12 — Revoked issuer's existing proofs remain valid

**Description:** Revoking an issuer does not automatically revoke proofs already registered under that issuer. Verifiers querying `is_valid_proof` for those proofs will still receive `true`.

**Affected assets:** Existing `ProofRecord` entries.

**Implemented control:** None. This is an accepted design decision — proof records are independent of issuer status after registration. If an issuer is revoked, the admin must separately call `admin_revoke_proof` for each affected proof.

**Status:** Accepted risk. Document this explicitly in backend integration guide.

**Required action:** The backend must maintain an index of proofs per issuer and perform batch `admin_revoke_proof` calls when an issuer is revoked. Add this to `docs/backend-integration.md`.

---

### T-13 — Stale cross-contract registry references

**Description:** `proof-registry` was initialized pointing to a specific `issuer-registry` or `protocol-config` address. If those contracts are redeployed at new addresses, `proof-registry` continues calling the old (potentially empty or malicious) addresses.

**Affected entry points:** `register_proof` cross-contract calls.

**Implemented control:** The addresses are immutable after `initialize`. `get_issuer_registry` and `get_protocol_config` are public read functions that allow off-chain verification that the references are correct.

**Status:** Accepted risk for the current contract design (no upgrade path). Any redeployment of `issuer-registry` or `protocol-config` requires a full redeployment and re-initialization of `proof-registry`.

**Required action:** Document this redeployment dependency. Add the cross-contract address check to the post-deployment verification checklist (see [Section 9, Gate M-10](#m-10-post-deployment-verification)).

---

### T-14 — Compromised admin transfers admin to attacker

**Description:** A compromised admin calls `set_admin` to transfer the `protocol-config` admin role to an attacker address. There is no time-lock or confirmation step.

**Affected entry points:** `protocol-config::set_admin`.

**Implemented control:** None beyond the existing `require_auth` check.

**Status:** Open risk. Mainnet blocker — see [Section 9, Gate M-02](#m-02-admin-custody).

**Note:** `issuer-registry` and `proof-registry` do not expose a `set_admin` function. Admin transfer is only possible on `protocol-config`. For the other two contracts, admin can only be changed via WASM upgrade + redeployment.

---

### T-15 — Event consumers receive misleading events

**Description:** Off-chain event consumers (indexers, monitoring) misinterpret or miss events and take incorrect actions (e.g., treating a paused protocol as active).

**Affected assets:** `protocol-config` events (`Initialized`, `AdminChanged`, `Paused`, `Unpaused`, `SchemaApproved`, `SchemaDeprecated`).

**Implemented control:** `protocol-config` uses typed `#[contractevent]` structs for all state-mutating operations. `issuer-registry` and `proof-registry` do not currently emit events.

**Status:** Partial. `protocol-config` events are typed and published. `issuer-registry` and `proof-registry` emit no events.

**Required action (open gap):** Add typed events to `issuer-registry` (issuer registered, status changed, address rotated) and `proof-registry` (proof registered, proof revoked). This is necessary for backend indexing and incident response. Track as an open issue.

---

### T-16 — Upgrade / deployment supply chain compromise

**Description:** A malicious WASM artifact is deployed instead of the audited build. This could happen via a compromised CI pipeline, a tampered build artifact, or a deployment operator using an unverified WASM file.

**Affected assets:** All three contract WASM artifacts.

**Implemented control:** `scripts/deployment-manifest.testnet.json` records SHA-256 hashes of each deployed WASM file. `scripts/verify-manifest.ps1` can be used to verify the manifest. The deployment script (`deploy-testnet.ps1`) captures and records WASM hashes using `Get-FileHash` before deployment.

**Status:** Implemented for testnet. Requires a reproducible build process and independent hash verification for mainnet.

**Required action:** See [Section 9, Gate M-07](#m-07-reproducible-build-artifacts).

---

### T-17 — Accidental mainnet deployment

**Description:** An operator runs the deployment script against mainnet instead of testnet, deploying unaudited contracts to production.

**Implemented control:** `deploy-testnet.ps1` defaults `$Network = "testnet"`. The script name itself signals testnet intent.

**Status:** Partial. The default is testnet, but there is no explicit guard that fails closed if `$Network` is set to `mainnet` or `pubnet`.

**Required action:** Add an explicit check to `deploy-testnet.ps1` that panics/errors if `$Network` is not `"testnet"`. A separate, explicitly-named `deploy-mainnet.ps1` should be created only after the mainnet release gates are satisfied, and should require an explicit `--confirm-mainnet` flag. See [Section 9, Gate M-01](#m-01-independent-audit-resolution).

---

### T-18 — Proof record not found causes panic rather than false return

**Description:** `get_proof`, `is_valid_proof`, and `is_revoked` call `expect("proof not found")` which panics if the key does not exist. A verifier calling `is_valid_proof` for a non-existent proof ID receives a transaction failure rather than `false`.

**Affected entry points:** `get_proof`, `is_valid_proof`, `is_revoked`.

**Status:** Accepted design. Panicking on missing proof distinguishes "proof never registered" from "proof registered and invalid." Callers must handle this distinction.

**Required action:** Document the panic behavior in `docs/backend-integration.md` so verifiers know to handle the error case.

---

### T-19 — Schema version deprecation does not invalidate existing proofs

**Description:** When a schema version is deprecated via `deprecate_schema_version`, existing proofs registered under that version remain in `ProofStatus::Active` and pass `is_valid_proof`.

**Implemented control:** None. This is by design — deprecation prevents new registrations under the deprecated schema but does not retroactively revoke existing proofs.

**Status:** Accepted risk. Document in backend integration guide.

---

### T-20 — Proof `expires_at` uses ledger timestamp, not ledger sequence

**Description:** Expiration is checked against `env.ledger().timestamp()` (Unix seconds). This is the ledger close time as reported by the Stellar network. It is not under application control but is subject to minor clock skew between validators (bounded by Stellar's protocol).

**Status:** Accepted. Stellar ledger timestamps are consensus-agreed values with bounded skew. No action required.

---

### T-21 — Backend hashing inconsistency

**Description:** If the backend uses a different hash function or canonical serialization than documented in `docs/backend-integration.md`, the same logical proof or issuer will produce different on-chain IDs, breaking lookups.

**Implemented control:** `docs/backend-integration.md` specifies SHA-256 hashing rules for all ID fields.

**Status:** Accepted off-chain risk. The contract cannot verify the hashing algorithm used by the caller.

**Required action:** The backend must have tests asserting the exact hash values produced for known inputs. These are backend tests, not contract tests.

---

## 6. Privacy Analysis

The on-chain privacy boundary is enforced by what the contracts accept as input and what they store. This section confirms no path stores private personal data.

### 6.1 What contracts store

| Field | Type | Privacy assessment |
|---|---|---|
| `proof_id_hash` | `BytesN<32>` (SHA-256) | Hash only. Not reversible to the original proof ID without the preimage. |
| `commitment_hash` | `BytesN<32>` (SHA-256) | Hash of the credential payload without signature. Does not reveal income, identity, or payment history. |
| `issuer_address` | `Address` | Public Stellar address. Identifies the issuing organization, not the subject. |
| `issuer_id_hash` | `BytesN<32>` (SHA-256) | Hash of the issuer identifier. Does not reveal the issuer name or credentials. |
| `metadata_hash` | `BytesN<32>` (SHA-256) | Hash of public issuer metadata. Does not reveal personal information. |
| `status` | Enum (`Active`, `Suspended`, `Revoked` / `Active`, `Revoked`) | Operational state only. |
| `schema_version` | `u32` | Schema identifier. Does not reveal personal data. |
| `expires_at`, `created_at`, `updated_at`, `revoked_at` | `u64` timestamps | Timestamps only. No personal data. |

### 6.2 What contracts do not store

The following are explicitly absent from all contract storage types defined in `packages/shared/src/lib.rs`:

- Exact salary or income amount
- Exact payment amount or frequency
- Full wallet transaction history
- Personal name, email, phone number
- Employment documents or raw credential content
- Unencrypted personal identifiers
- Raw credential signatures

### 6.3 Off-chain risk

The privacy guarantee depends on the backend correctly computing only hashes before submitting to contracts. If the backend were to pass a cleartext credential payload as the `commitment_hash` argument, it would be stored on-chain. This risk is off-chain and cannot be enforced by the contract.

---

## 7. Invariants

The following invariants must hold at all times. Tests covering each invariant are noted.

| # | Invariant | Test reference |
|---|---|---|
| I-01 | Each contract can be initialized only once | `initialize` guards checked in each contract |
| I-02 | A proof ID hash can be registered at most once | `rejects_duplicate_proof_id` |
| I-03 | An issuer ID hash can be registered at most once | `rejects_duplicate_issuer_id` |
| I-04 | An issuer address can be associated with at most one issuer ID hash at a time | `issuer address already registered` guard |
| I-05 | A revoked issuer cannot be reactivated | `revoked_issuer_cannot_be_reactivated` |
| I-06 | A revoked issuer cannot have metadata updated or address rotated | `revoked issuer cannot be updated` / `revoked issuer cannot rotate address` guards |
| I-07 | A revoked proof cannot be revoked again | `proof already revoked` guard |
| I-08 | `register_proof` rejects `expires_at` in the past | `rejects_expired_proof` |
| I-09 | `register_proof` rejects when protocol is paused | `rejects_registration_when_protocol_is_paused` |
| I-10 | `register_proof` rejects unapproved schema versions | `rejects_unapproved_schema_version` |
| I-11 | `register_proof` rejects inactive issuer addresses | `rejects_inactive_issuer_address` |
| I-12 | `is_valid_proof` returns false for expired proofs | Expiry read-path test missing — see T-09 |
| I-13 | Schema version zero is always rejected | `rejects_zero_schema_version` |

---

## 8. Security Review Checklist

Maintainers must complete this checklist against the commit being reviewed. Check each item or link to an open issue explaining why it is not yet satisfied.

### Authorization

- [ ] **AUTH-01** Every state-mutating entry point calls `require_auth` before reading or writing state. Verify by reading each `#[contractimpl]` function that writes to storage.
- [ ] **AUTH-02** Negative authorization tests exist for at least `register_issuer`, `pause`, `register_proof`, and `admin_revoke_proof` that assert the call panics without the required auth.
- [ ] **AUTH-03** `set_admin` emits the `AdminChanged` event. Confirm the new admin address in the event matches the stored admin.
- [ ] **AUTH-04** The `revoke_proof` path requires the original `issuer_address` from the stored record, not an address supplied by the caller.

### Replay and Duplicate Writes

- [ ] **REPLAY-01** `register_proof` panics on duplicate `proof_id_hash`. Covered by `rejects_duplicate_proof_id`.
- [ ] **REPLAY-02** `register_issuer` panics on duplicate `issuer_id_hash`. Covered by `rejects_duplicate_issuer_id`.
- [ ] **REPLAY-03** `register_issuer` panics on duplicate `issuer_address`. Add a dedicated test for this path.
- [ ] **REPLAY-04** `initialize` on all three contracts panics on second call. Verify by reading each `initialize` function.

### Issuer Lifecycle

- [ ] **ISSUER-01** Suspended issuers cannot register proofs. Covered by `rejects_inactive_issuer_address`.
- [ ] **ISSUER-02** Revoked issuers cannot register proofs. Add a dedicated test (current test uses a suspended issuer).
- [ ] **ISSUER-03** Revoked issuers cannot be reactivated. Covered by `revoked_issuer_cannot_be_reactivated`.
- [ ] **ISSUER-04** Address rotation removes the old `AddressIssuer` mapping atomically. Verify that `is_active_address` returns false for the old address after rotation.

### Proof Lifecycle

- [ ] **PROOF-01** Expired proofs return `false` from `is_valid_proof`. Add a test advancing `env.ledger().set_timestamp()` past `expires_at`.
- [ ] **PROOF-02** Revoked proofs return `false` from `is_valid_proof`. Covered by `issuer_can_revoke_proof`.
- [ ] **PROOF-03** `admin_revoke_proof` requires admin auth. Verify `set_revoked(by_admin=true)` path.
- [ ] **PROOF-04** Double revocation panics. Verify `proof already revoked` guard exists and add a test.

### Protocol Pause

- [ ] **PAUSE-01** `register_proof` panics when paused. Covered by `rejects_registration_when_protocol_is_paused`.
- [ ] **PAUSE-02** Existing proofs are still readable while paused. `get_proof` and `is_valid_proof` do not check pause state. Confirm this is intended behavior.
- [ ] **PAUSE-03** `unpause` restores `register_proof` functionality. Add a test that pauses, unpauses, and then successfully registers a proof.

### Storage and TTL

- [ ] **TTL-01** Persistent issuer records have TTL > threshold after registration. Covered by `extends_issuer_storage_ttl`.
- [ ] **TTL-02** Persistent proof records have TTL > threshold after registration. Covered by `extends_proof_storage_ttl`.
- [ ] **TTL-03** Instance storage TTL is extended at initialization. Add tests for instance TTL on `protocol-config` and `issuer-registry`.
- [ ] **TTL-04** Schema version persistent entries have TTL > threshold after `approve_schema_version`. Covered by `extends_schema_storage_ttl`.
- [ ] **TTL-05** TTL constants `TTL_THRESHOLD_LEDGERS = 50_000` and `TTL_EXTEND_TO_LEDGERS = 500_000` are documented with their approximate real-world duration (at 5s/ledger: threshold ≈ 2.9 days, extend-to ≈ 28.9 days).

### Cross-Contract Calls

- [ ] **CROSS-01** `proof-registry` calls `is_paused` on the address stored at `DataKey::ProtocolConfig`, not a hardcoded address. Verify by reading `register_proof`.
- [ ] **CROSS-02** `proof-registry` calls `is_active_address` on the address stored at `DataKey::IssuerRegistry`. Verify by reading `register_proof`.
- [ ] **CROSS-03** Post-deployment: `get_issuer_registry()` and `get_protocol_config()` return the expected contract addresses. Verify against the deployment manifest.

### Privacy

- [ ] **PRIV-01** No contract function accepts a parameter typed for or documented as containing raw income, payment amounts, or personal identity data.
- [ ] **PRIV-02** `IssuerRecord` and `ProofRecord` in `packages/shared/src/lib.rs` contain only hashes, addresses, statuses, schema version, and timestamps. Confirm no new fields have been added.
- [ ] **PRIV-03** The backend integration doc (`docs/backend-integration.md`) states the hashing rules and the on-chain data boundary.

### Events

- [ ] **EVENT-01** All `protocol-config` state mutations emit a typed event. Verify each function emits the correct event struct.
- [ ] **EVENT-02** `issuer-registry` and `proof-registry` emit no events today. Open issue exists to add them before mainnet.

### Upgrade and Deployment

- [ ] **DEPLOY-01** `deploy-testnet.ps1` defaults to `--network testnet`. Confirm the `$Network` parameter default.
- [ ] **DEPLOY-02** Add a guard to `deploy-testnet.ps1` that errors if `$Network` is set to `mainnet` or `pubnet`.
- [ ] **DEPLOY-03** WASM SHA-256 hashes in the deployment manifest are verified against the built artifacts before deployment. Confirm `verify-manifest.ps1` covers this.
- [ ] **DEPLOY-04** Contract IDs in the deployment manifest match the live testnet explorer links.

---

## 9. Mainnet Release Gates

The following gates must all be satisfied before any mainnet deployment. Each gate has an owner field to be filled in by maintainers.

### M-01 — Independent Audit Resolution

**Requirement:** An independent security audit of all three contracts and deployment tooling has been completed. All critical and high findings are resolved or have documented accepted-risk rationale. Medium findings are triaged.

**Current status:** Not started.

**Blocking:** Yes.

**Owner:** _Assign before mainnet._

---

### M-02 — Admin Custody

**Requirement:** The admin address used in each contract's `initialize` call is a Stellar multisig account with a signing threshold of at least 2, or a governance contract. Key custody procedures are documented. The deployer key and the admin key are different accounts.

**Current status:** On testnet, admin and source are the same account (`GCDPMNCCMADKEL4YJAJNJXTCGZFAGWQCEFXJYBZVLJCFYOI76FTX6HMV`). Single-key admin is acceptable for testnet only.

**Blocking:** Yes.

**Owner:** _Assign before mainnet._

---

### M-03 — Upgrade Policy

**Requirement:** A written upgrade policy exists covering: (a) under what conditions WASM can be upgraded, (b) who holds upgrade authority, (c) how upgrade transactions are reviewed before broadcast, and (d) whether current Soroban contracts support `upgrade` or require full redeployment.

**Current status:** No upgrade path is implemented. Full redeployment is the current mechanism.

**Blocking:** Yes.

**Owner:** _Assign before mainnet._

---

### M-04 — Monitoring and Alerting

**Requirement:** A monitoring setup is in place that alerts on: admin key usage, pause/unpause events, issuer revocations, admin transfer, and any transaction failure rate spike. The `get_config_version` monotonic counter can be used to detect unexpected state changes.

**Current status:** Not configured.

**Blocking:** Yes.

**Owner:** _Assign before mainnet._

---

### M-05 — Incident Response Plan

**Requirement:** A documented incident response plan covers: how to pause the protocol, how to revoke a compromised issuer, how to perform batch proof revocation, who has pause authority, and how to communicate a security event to users.

**Current status:** Not documented.

**Blocking:** Yes.

**Owner:** _Assign before mainnet._

---

### M-06 — TTL and Resource Analysis

**Requirement:** A ledger resource budget analysis confirms that the TTL constants (`TTL_THRESHOLD_LEDGERS = 50_000`, `TTL_EXTEND_TO_LEDGERS = 500_000`) are sufficient given expected proof and issuer volumes, and that the cost of TTL extension operations is within acceptable bounds. The approximate real-world duration of the extend-to window (≈ 28.9 days at 5 s/ledger) is documented and accepted.

**Current status:** Constants are set but no formal analysis exists.

**Blocking:** Yes.

**Owner:** _Assign before mainnet._

---

### M-07 — Reproducible Build Artifacts

**Requirement:** The WASM artifacts deployed to mainnet can be reproduced from the repository at the tagged commit using only `rustup target add wasm32v1-none && stellar contract build`. The SHA-256 hash of the built artifacts matches the deployment manifest. An independent party must verify this before deployment.

**Current status:** Testnet WASM hashes are recorded in `scripts/deployment-manifest.testnet.json`. Reproducibility has not been independently verified.

**Blocking:** Yes.

**Owner:** _Assign before mainnet._

---

### M-08 — Backend Compatibility

**Requirement:** The live backend (`earnproof-backend`) has been tested against the mainnet contract IDs using the hashing rules in `docs/backend-integration.md`. Proof registration, issuer lookup, and revocation flows work end-to-end. The remaining blocker noted in the README ("live backend anchoring against deployed proof registry") is resolved.

**Current status:** Backend anchoring against testnet is the remaining open blocker per README.

**Blocking:** Yes.

**Owner:** _Assign before mainnet._

---

### M-09 — Deployment Acknowledgement

**Requirement:** Before executing the mainnet deployment, the deploying team completes and signs off on:

- [ ] The audit has been completed and all blockers are resolved (M-01).
- [ ] Admin custody is confirmed (M-02).
- [ ] The upgrade policy is documented (M-03).
- [ ] Monitoring is in place (M-04).
- [ ] The incident response plan is documented (M-05).
- [ ] TTL analysis is complete (M-06).
- [ ] Reproducible build is verified (M-07).
- [ ] Backend compatibility is confirmed (M-08).
- [ ] All security review checklist items in Section 8 are checked.
- [ ] The deployment script uses a separate mainnet script with an explicit `--confirm-mainnet` flag (T-17).

**Current status:** Not ready.

**Blocking:** Yes.

---

### M-10 — Post-Deployment Verification

**Requirement:** After mainnet deployment, the following read-only checks are performed and recorded:

- `protocol-config::get_admin()` returns the expected multisig address.
- `protocol-config::is_paused()` returns `false`.
- `protocol-config::is_schema_version_approved(1)` returns `true`.
- `issuer-registry::get_admin()` returns the expected multisig address.
- `proof-registry::get_admin()` returns the expected multisig address.
- `proof-registry::get_issuer_registry()` returns the mainnet `issuer-registry` contract ID.
- `proof-registry::get_protocol_config()` returns the mainnet `protocol-config` contract ID.
- WASM SHA-256 of deployed artifacts matches the manifest.
- Explorer links for all deployment transactions are recorded.

**Current status:** This procedure is defined here for the first time. The testnet equivalent is recorded in `scripts/deployment-manifest.testnet.json`.

**Blocking:** Yes — must be completed after deployment before declaring mainnet live.

---

### M-11 — Containment and Rollback Limits

**Requirement:** The team acknowledges and documents the following containment limits:

- There is no on-chain rollback mechanism. Data written to persistent storage cannot be erased from within a contract.
- The only emergency lever available after deployment is `pause()`, which halts new proof registrations.
- Proof records and issuer records written before a pause remain readable.
- Rolling back a deployment requires re-deploying all three contracts at new addresses and migrating all off-chain references (backend config, frontend config, indexers).
- Batch proof revocation must be done by calling `admin_revoke_proof` individually per proof. There is no bulk revocation function.

**Current status:** Documented here. Team must acknowledge before mainnet.

**Blocking:** Acknowledgement required before M-09 sign-off.

---

## 10. Open Gaps Summary

The following items are unresolved as of this document. Each should be tracked as an issue in the repository.

| # | Area | Description | Severity |
|---|---|---|---|
| G-01 | Authorization | Negative auth tests missing — calls without required auth should be tested to confirm they panic. | Medium |
| G-02 | Duplicate registration | No test for duplicate `issuer_address` registration (different ID hash, same address). | Low |
| G-03 | Proof expiry | No test advancing the ledger clock to confirm `is_valid_proof` returns false after `expires_at`. | Medium |
| G-04 | Instance TTL | No test asserting instance storage TTL is set after initialization on `protocol-config` and `issuer-registry`. | Low |
| G-05 | Events | `issuer-registry` and `proof-registry` emit no events. Required for backend indexing and incident response. | High |
| G-06 | Revoked issuer proofs | No on-chain mechanism to batch-revoke proofs when an issuer is revoked. Backend must handle this off-chain. | Medium |
| G-07 | Deployment guard | `deploy-testnet.ps1` does not fail closed if `$Network` is set to `mainnet` or `pubnet`. | High |
| G-08 | Pause unpause test | No test that pauses, unpauses, and confirms `register_proof` succeeds after unpause. | Low |
| G-09 | Revoked issuer proof test | No test confirming a revoked (not just suspended) issuer cannot register a proof. | Low |
| G-10 | Admin key | Single EOA admin is used on testnet. Mainnet requires multisig. | Critical (mainnet) |
