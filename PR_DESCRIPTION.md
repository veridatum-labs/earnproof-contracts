# Numeric Boundary and Overflow Tests for Contract Inputs

Closes #60

## Summary

This PR adds comprehensive table-driven boundary tests for every numeric input across all EarnProof contracts. It also consolidates numeric constants to a shared location and fixes an unchecked arithmetic operation that could overflow.

### Key Changes

1. **Consolidated Numeric Constants** (`packages/shared/src/lib.rs`)
   - Moved all numeric limits to a single source of truth
   - Added documented constants: `MIN_SCHEMA_VERSION`, `MIN_EXPIRATION_OFFSET_FROM_NOW`, `MIN_CONTRACT_VERSION`, `MAX_CONFIG_VERSION`
   - All contracts now import and use these shared definitions

2. **Fixed Unchecked Arithmetic** (`contracts/protocol-config/src/lib.rs`)
   - Replaced raw `current + 1` addition in `bump_config_version()` with `checked_add()`
   - Now panics with clear message if config version would overflow
   - Prevents silent wraparound to 0 at u32::MAX

3. **Comprehensive Boundary Tests**
   - Added 50+ new test cases across all three contracts
   - Table-driven tests cover: zero, minimum, maximum, and type limits for each numeric input
   - Verified state invariants: failed validations leave storage and events unchanged

### Numeric Input Inventory

#### proof-registry
| Input | Type | Bounds | Tests |
|-------|------|--------|-------|
| `schema_version` | u32 | >= 1 | min, max, zero rejection, valid range |
| `expires_at` | u64 | > current_timestamp | future validation, past rejection, boundary offset |

#### protocol-config
| Input | Type | Bounds | Tests |
|-------|------|--------|-------|
| `version` (schema) | u32 | >= 1 | min, max, zero rejection, approval flow |
| `new_version` (contract) | u32 | > current_version | downgrade rejection, skip versions, large values |
| ConfigVersion | u32 | increment with overflow check | monotonic increase, near-max safety |

#### issuer-registry
| Input | Type | Bounds | Tests |
|-------|------|--------|-------|
| Contract version | u32 | governance shared with others | upgrade boundaries, version monotonicity |

### Checked Arithmetic Operations

**Before:** 
```rust
fn bump_config_version(env: Env) {
    let current = Self::get_config_version(env.clone());
    env.storage()
        .instance()
        .set(&DataKey::ConfigVersion, &(current + 1));
}
```

**After:**
```rust
fn bump_config_version(env: Env) {
    let current = Self::get_config_version(env.clone());
    let new_version = current
        .checked_add(1)
        .unwrap_or_else(|| panic!("config version overflow: reached maximum"));
    env.storage()
        .instance()
        .set(&DataKey::ConfigVersion, &new_version);
}
```

### State & Event Invariants

All boundary failure tests verify that failed validations do not:
- Write state to storage (ProofRecord, IssuerRecord, upgrade allowlist entries verified absent)
- Modify existing state (config version not incremented on failed validation)
- Emit partial or incorrect events

Examples:
- `failed_register_proof_schema_zero_leaves_state_unchanged()` — verifies proof not written when schema_version == 0
- `failed_register_proof_expired_leaves_state_unchanged()` — verifies proof not written when expires_at <= current_time
- `failed_upgrade_version_downgrade_leaves_state_unchanged()` — verifies no state change on version guard rejection

### Behavior Changes

**Breaking Change:** ConfigVersion now enforces safe arithmetic boundaries. If ConfigVersion reaches u32::MAX, further configuration mutations will panic instead of silently wrapping to 0. This is a safety improvement but represents a state machine state change.

**Impact Assessment:**
- Operationally unreachable: u32::MAX ≈ 4 billion mutations required
- No current deployment close to this limit
- Explicit error message aids debugging if ever approached

### Validation

All changes pass:
```
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo build --workspace
```

**Test Output Summary:**
- 50+ new boundary test cases added
- All existing tests pass
- No new compiler warnings
- All unsafe arithmetic eliminated

### Files Changed

- `packages/shared/src/lib.rs` — Consolidated numeric constants with documentation
- `contracts/protocol-config/src/lib.rs` — Fixed checked_add, updated imports, added 15+ tests
- `contracts/proof-registry/src/lib.rs` — Updated imports, added 20+ tests
- `contracts/issuer-registry/src/lib.rs` — Updated imports, added 15+ tests

### Documentation

All numeric constants are now documented with:
- Purpose and use case
- Valid range and enforced bounds
- References to where they're checked
- Notes on overflow behavior and operational limits

Example:
```rust
/// Minimum valid schema version. Schema version 0 is reserved and invalid.
/// All schema versions must be > 0 to be approved or queried.
pub const MIN_SCHEMA_VERSION: u32 = 1;
```

### References

- Issue: #60 - Add numeric boundary and overflow tests for every contract input
- Related: Numeric safety, bounded arithmetic, state invariants
