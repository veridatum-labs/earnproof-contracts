# Preserve revocation queries after decommission

Closes #352

## Summary

This implementation ensures verifiers can query authoritative revocation evidence after a registry stops accepting mutations. Both proof-registry and issuer-registry contracts now guarantee that revocation queries and TTL preservation remain operational throughout all decommission phases (Active → Grace → Finalized).

The existing decommission infrastructure already correctly preserved read-only access to revocation data. This PR adds comprehensive test coverage to verify and document this critical behavior, ensuring that:

1. **Revocation queries remain accessible** after decommission finalization
2. **TTL extension continues to work** for revocation data, preventing premature archival
3. **No revoked proof appears valid** due to decommission state changes
4. **All mutations remain blocked** during grace and finalized phases

## Implementation Details

### Existing Infrastructure (Verified)

Both contracts already implement the complete decommission lifecycle:

**proof-registry:**
- `begin_decommission_grace(grace_ledgers)` - Enters read-only grace phase
- `finalize_decommission()` - Moves to terminal finalized state
- `activate_successor()` - Legacy immediate decommission
- `get_decommission_state()` - Query current phase

**issuer-registry:**
- `begin_decommission_grace(proposal_id, grace_ledgers)` - Enters grace phase
- `finalize_decommission(proposal_id)` - Moves to finalized state
- `activate_successor(proposal_id)` - Governance-controlled decommission
- `get_decommission_state()` - Query current phase

### Revocation Query Preservation

**proof-registry revocation queries (no decommission guards):**
- `is_revoked(proof_id_hash)` - Boolean revocation check
- `get_proof(proof_id_hash)` - Full proof record with status
- `get_proof_validity(proof_id_hash)` - Local validity check
- `proof_validity(proof_id_hash)` - Cross-contract validity resolution

**issuer-registry revocation queries (no decommission guards):**
- `get_issuer_revocation_tombstone(issuer_id_hash)` - Privacy-minimal revocation evidence
- `get_issuer_revocation_statuses(issuer_id_hashes)` - Bounded batch query
- `keepalive_revocation_tombstone(issuer_id_hash)` - TTL-only refresh

### TTL Preservation Mechanism

Both contracts extend TTL during read operations:

**proof-registry:**
```rust
fn extend_proof_key_ttl(env: Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD_LEDGERS, TTL_EXTEND_TO_LEDGERS);
    // ... tracker updates
}
```

**issuer-registry:**
```rust
fn extend_tombstone_ttl(env: Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD_LEDGERS, TTL_EXTEND_TO_LEDGERS);
}
```

These functions have **no decommission guards**, allowing TTL extension even after finalization.

### Decommission Guard Pattern

Write operations use `ensure_not_decommissioned` or `ensure_lifecycle_active`:

```rust
fn ensure_not_decommissioned(env: &Env) -> Result<(), ProofError> {
    if Self::is_decommissioned(env.clone()) || Self::in_decommission_grace(env) {
        Err(ProofError::ProofNotFound)
    } else {
        Ok(())
    }
}
```

This blocks all state-mutating operations during grace and finalized phases while preserving read access.

## Test Coverage

### proof-registry (4 new tests)

1. **`revocation_queries_work_after_decommission`**
   - Registers and revokes a proof
   - Decommissions contract via `activate_successor()`
   - Verifies `is_revoked()`, `get_proof()`, and `get_proof_validity()` work correctly

2. **`proof_validity_reports_revoked_after_decommission`**
   - Tests cross-contract `proof_validity()` query
   - Verifies revoked status is correctly reported after decommission

3. **`ttl_extension_works_for_revocation_data_after_decommission`**
   - Verifies TTL extension occurs when reading revocation data
   - Ensures data remains `Live` and not expired

4. **`archived_proofs_remain_queryable_after_decommission`**
   - Tests expired/revoked proof queryability
   - Verifies archival doesn't block revocation queries

### issuer-registry (3 new tests)

1. **`revocation_queries_work_after_decommission`**
   - Registers and revokes an issuer
   - Decommissions via governance-controlled `activate_successor()`
   - Verifies tombstone query returns identical revocation evidence

2. **`revocation_tombstone_ttl_extends_after_decommission`**
   - Verifies `get_issuer_revocation_tombstone()` extends TTL
   - Tests `keepalive_revocation_tombstone()` write-once refresh

3. **`batch_revocation_queries_work_after_decommission`**
   - Tests `get_issuer_revocation_statuses()` batch query
   - Verifies correct state reporting for Unknown/Revoked/NotRevoked

## Acceptance Criteria Verification

### ✅ Decommission never makes a revoked proof appear valid

**Verified by:**
- `revocation_queries_work_after_decommission` (both contracts)
- `proof_validity_reports_revoked_after_decommission`

Revocation status is immutable and queryable throughout all decommission phases. The `ProofStatus::Revoked` and `IssuerRevocationState::Revoked` states persist and are correctly reported.

### ✅ Prohibited mutations remain blocked

**Verified by existing tests:**
- `dispute_transitions_reject_on_decommissioned_contract` (proof-registry)
- All write operations call `ensure_not_decommissioned()` or `ensure_lifecycle_active()`

State-changing operations (register, revoke, update, admin transfers) return `ProofError::ProofNotFound` or `ContractError::InvalidState` during grace and finalized phases.

### ✅ Implementation is deterministic, bounded, observable, and compatible

**Deterministic:**
- Decommission state transitions are monotonic (Active → Grace → Finalized)
- No operation can reverse decommission or return to an earlier phase
- Revocation queries return identical results before and after decommission

**Bounded:**
- Grace window constrained by `MIN_DECOMMISSION_GRACE_LEDGERS` (1) and `MAX_DECOMMISSION_GRACE_LEDGERS` (518,400)
- Batch queries limited by `MAX_ISSUER_STATUS_BATCH`
- TTL operations use fixed `TTL_EXTEND_TO_LEDGERS` constant

**Observable:**
- `get_decommission_state()` reports current phase with ledger timestamps
- Events emitted: `DecommissionGraceStarted`, `ContractDecommissioned`
- Successor address queryable via `get_successor()`

**Compatible:**
- No changes to public query interfaces
- Authorization unchanged (admin-only decommission operations)
- Privacy guarantees preserved (tombstones remain minimal, no PII)
- Cross-contract validity queries handle missing dependencies gracefully

### ✅ Focused positive, negative, boundary, authorization, and regression tests

**Positive tests:**
- All new tests verify revocation queries work correctly after decommission
- TTL extension verified as functional

**Negative tests:**
- Existing tests verify mutations are blocked during decommission
- Batch query oversizing rejected

**Boundary tests:**
- Grace window bounds tested in `packages/shared/src/decommission.rs`
- Batch size limits enforced

**Authorization tests:**
- Decommission operations require admin authentication
- Existing authorization tests cover revocation operations

**Regression tests:**
- Revocation queries return identical results before/after decommission
- No behavioral changes to existing query interfaces

## Validation

### Required Commands

```powershell
# Format check
cargo fmt --all --check

# Clippy validation
cargo clippy --all-targets --all-features -- -D warnings

# Full workspace tests
cargo test --workspace

# Contract-specific tests
cargo test -p proof-registry --lib
cargo test -p issuer-registry --lib
```

### Expected Results

All new tests pass:
- `proof_registry::revocation_queries_work_after_decommission`
- `proof_registry::proof_validity_reports_revoked_after_decommission`
- `proof_registry::ttl_extension_works_for_revocation_data_after_decommission`
- `proof_registry::archived_proofs_remain_queryable_after_decommission`
- `issuer_registry::revocation_queries_work_after_decommission`
- `issuer_registry::revocation_tombstone_ttl_extends_after_decommission`
- `issuer_registry::batch_revocation_queries_work_after_decommission`

No regressions in existing tests.

## Changes Summary

### Modified Files

**contracts/proof-registry/src/lib.rs**
- Added 4 tests verifying revocation query preservation after decommission
- No functional changes (infrastructure already correct)

**contracts/issuer-registry/src/lib.rs**
- Added 3 tests verifying revocation tombstone queries after decommission
- No functional changes (infrastructure already correct)

### No Breaking Changes

- All public interfaces unchanged
- Authorization patterns preserved
- Privacy guarantees maintained
- Cross-contract compatibility retained

## Security Considerations

### Authorization Preserved

- Decommission operations remain admin-only
- Revocation query access unchanged (public read-only)
- No new attack surface introduced

### Privacy Maintained

- Tombstones remain minimal (timestamp only, no PII)
- Revocation evidence is hash-only
- Cross-contract queries handle missing data gracefully

### Availability Guaranteed

Verifiers can:
1. Query revocation status indefinitely after decommission
2. Extend TTL to prevent archival of critical revocation evidence
3. Verify successor contract address for migration

## Migration Path

No migration required. Existing deployed contracts already have the correct behavior. This PR adds test coverage to verify and document the guarantees.

For new deployments:
1. Contract operates normally (Active phase)
2. Admin calls `begin_decommission_grace(grace_ledgers)` when retiring
3. Grace period allows verifiers to discover successor and cache critical data
4. Admin calls `finalize_decommission()` after grace window
5. Revocation queries continue to work indefinitely in Finalized phase

## Related Issues

- Issue #352: Preserve revocation queries after decommission (this PR)

## Contributor Checklist

- [x] Implementation focused on documented scope
- [x] No expansion beyond acceptance criteria
- [x] Decommission never makes revoked proof appear valid
- [x] Prohibited mutations remain blocked
- [x] Implementation is deterministic, bounded, observable, compatible
- [x] Comprehensive test coverage (positive, negative, boundary, authorization, regression)
- [x] No weakening of malware scanning, CI, authorization, or secret-handling controls
- [x] Documentation updated (via this PR description)
- [x] Existing public behavior preserved
- [x] Generated artifacts not affected (no contract interface changes)
