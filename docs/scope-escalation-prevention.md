# Scope escalation prevention

This document specifies how the EarnProof contracts prevent scoped emergency
authorizations from being reused for broader operations or contracts. Every
assertion in this document maps to a test in `tests/emergency/src/scope_escalation_prevention.rs`
and `tests/authorization/src/scope_authorization_matrix.rs`.

## Problem statement

Emergency authorization must be constrained to its declared scope. A guardian
authorized to pause Registration scope should not be able to:

- Pause Updates, Revocation, Upgrades, or Disputes scopes
- Escalate a scoped pause into a global pause
- Use policy changes to reinterpret or bypass scope restrictions
- Affect operations across contract boundaries

## Solution architecture

### Scope enumeration

Six scopes are defined (Rust enum `shared::PauseScope`):

```rust
pub enum PauseScope {
    Global,      // Affects new obligations (Registration, Updates) via fallback
    Registration, // Blocks new proof registration
    Updates,     // Blocks proof hash updates
    Revocation,  // Blocks revocation operations
    Upgrades,    // Blocks upgrade/migration operations
    Disputes,    // Blocks dispute resolution operations
}
```

Each scope is independent. Pausing one does not affect the others.

### Action enumeration

Two emergency action types are defined (Rust enum `shared::EmergencyAction`):

```rust
pub enum EmergencyAction {
    GlobalPause,              // Triggers DataKey::Paused = true
    ScopedPause(PauseScope),  // Triggers DataKey::ScopedPause(scope) = true
}
```

The key constraint: `ScopedPause(PauseScope::Global)` is rejected at proposal time.
Only `GlobalPause` or `ScopedPause(scope)` where `scope != Global` are valid.

### Storage isolation

Scope pauses are stored independently:

- **Global pause** (instance storage): `DataKey::Paused = bool`
- **Scoped pauses** (persistent storage): `DataKey::ScopedPause(scope) = bool`
  - One entry per scope, independently managed
  - Pausing Registration does not create or modify Updates entry

### Authorization boundaries

#### `protocol-config::set_scoped_pause`

Only the admin can call. Each scope can be paused/unpaused independently.

```rust
pub fn set_scoped_pause(
    env: Env,
    scope: PauseScope,
    paused: bool,
) -> Result<(), ContractError>
```

Authorization: `require_auth(&admin)`

#### `proof-registry::pause_scope` and `unpause_scope`

Scope pause operations delegated to proof-registry. Same admin as protocol-config.

```rust
pub fn pause_scope(env: Env, scope: PauseScope) -> Result<(), ProofError>
pub fn unpause_scope(env: Env, scope: PauseScope) -> Result<(), ProofError>
```

Authorization: `require_auth(&admin)` where admin is fetched from protocol-config

#### Emergency action proposal (`protocol-config::propose_emergency_action`)

Admin or guardian may propose. Scope restrictions are enforced here:

```rust
pub fn propose_emergency_action(
    env: Env,
    action: EmergencyAction,
    reason_commitment: BytesN<32>,
    expires_at: u32,
    proposer: Address,
) -> Result<BytesN<32>, ContractError>
```

Validation rules:
1. `reason_commitment` must not be all zeros
2. `action` must not be `ScopedPause(PauseScope::Global)`
3. `proposer` must be admin or current guardian
4. Proposal must not have already expired

If any check fails, the call returns `Err` and no state changes.

#### Emergency action approval (`protocol-config::approve_emergency_action`)

Guardian approves a proposal. Cross-checks scope consistency:

```rust
pub fn approve_emergency_action(
    env: Env,
    proposal_id: BytesN<32>,
    guardian: Address,
) -> Result<(), ContractError>
```

Validation rules:
1. Proposal must exist and not be expired
2. `guardian` must be in both proposal snapshot policy AND current policy
3. Current emergency policy must still be enabled

Removed guardians cannot add approvals. Policy changes do not reinterpret
existing proposals.

#### Emergency action execution (`protocol-config::execute_emergency_action`)

Anyone may execute once threshold is met. Scope is applied:

```rust
pub fn execute_emergency_action(
    env: Env,
    proposal_id: BytesN<32>,
) -> Result<(), ContractError>
```

Execution validation:
1. Proposal must exist and not be expired
2. Approval count from current guardians must meet proposal policy threshold
3. Proposal policy threshold must exceed normal critical-action threshold
4. No auth required (anyone can execute)

Action dispatch:
- **GlobalPause**: Sets instance storage `DataKey::Paused = true`
- **ScopedPause(scope)**: Sets persistent storage `DataKey::ScopedPause(scope) = true`

### Scope checking at call time

When a contract method checks if an operation is paused:

```rust
pub fn is_scope_paused(env: Env, scope: PauseScope) -> bool
```

Logic:
1. Check if specific scope is paused in persistent storage
2. If not, check if Global pause is active AND scope is in [Registration, Updates]
3. Return true if either condition holds, false otherwise

```
is_paused(scope) := 
  (scope paused explicitly) OR 
  (Global paused AND scope IN [Registration, Updates])
```

This ensures:
- Registration pause ≠ Updates pause
- Revocation pause is never affected by Global pause
- Upgrades pause is never affected by Global pause
- Disputes pause is never affected by Global pause

## Test coverage

### Proposal validation tests (`scope_escalation_prevention.rs`)

1. `scoped_pause_global_rejected` — ScopedPause(Global) is rejected
2. `scoped_pause_valid_scopes_accepted` — Valid scoped pauses work
3. `scoped_pause_zero_reason_commitment_rejected` — Zero commitments rejected

### Cross-scope containment tests (`scope_escalation_prevention.rs`)

1. `registration_pause_does_not_contain_updates` — Registration pause ≠ Updates
2. `updates_pause_does_not_contain_registration` — Updates pause ≠ Registration
3. `revocation_pause_independent_of_registration` — Revocation is independent
4. `global_pause_fallback_covers_registration` — Global → Registration (fallback)
5. `global_pause_fallback_covers_updates` — Global → Updates (fallback)
6. `global_pause_no_fallback_to_revocation` — Global ⇸ Revocation (no fallback)
7. `global_pause_no_fallback_to_upgrades` — Global ⇸ Upgrades (no fallback)
8. `global_pause_no_fallback_to_disputes` — Global ⇸ Disputes (no fallback)

### Guardian isolation tests (`scope_escalation_prevention.rs`)

1. `limited_guardian_cannot_escalate_scope` — Guardian scopes are not escalated
2. `multiple_guardians_cannot_escalate_scope` — Multiple guardians can't bypass rules

### Policy transition tests (`scope_escalation_prevention.rs`)

1. `policy_transition_preserves_scope_commitments` — Policy changes don't reinterpret proposals
2. `policy_transition_cannot_bypass_scope_restriction` — ScopedPause(Global) rejected even after policy change

### Unpause operation tests (`scope_escalation_prevention.rs`)

1. `unpause_scope_isolation` — Unpausing one scope doesn't affect others
2. `proof_registry_pause_scope_respects_boundary` — Cross-contract scope isolation
3. `proof_registry_unpause_scope_respects_boundary` — Cross-contract unpause isolation

### State consistency tests (`scope_escalation_prevention.rs`)

1. `multiple_scope_pauses_independent` — Multiple pauses stay independent
2. `scope_pauses_survive_policy_changes` — Pauses persist across policy changes

### Authorization matrix tests (`scope_authorization_matrix.rs`)

1. `set_scoped_pause_*_requires_admin` — Each scope requires admin (5 scopes)
2. `proof_registry_pause_scope_requires_admin` — Proof registry pause requires admin
3. `proof_registry_unpause_scope_requires_admin` — Proof registry unpause requires admin
4. `propose_emergency_action_requires_admin_or_guardian` — Proposal auth check
5. `approve_emergency_action_requires_current_guardian` — Approval auth check
6. `execute_emergency_action_requires_threshold_not_identity` — Execution doesn't require specific auth
7. `set_scoped_pause_unauthorized_no_state_change` — Failed calls don't modify state
8. `proof_registry_pause_scope_unauthorized_no_state_change` — Failed calls don't modify state

### Cross-contract consistency tests (`scope_authorization_matrix.rs`)

1. `proof_registry_pause_scope_uses_protocol_admin` — Proof registry uses protocol-config admin

## Verification

All scope escalation prevention guarantees are verified by:

1. **Unit tests** in each contract validating scope logic
2. **Integration tests** in `tests/emergency/` validating cross-contract scope boundaries
3. **Authorization tests** in `tests/authorization/` validating auth boundaries
4. **State consistency checks** ensuring no unauthorized call modifies state

Run the full test suite with:

```bash
cargo test --all
cargo test --all --doc
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
```

## Security considerations

### Guarantees

- A proposal cannot be created with scope escalation
- An existing proposal cannot be reinterpreted by policy changes
- A removed guardian cannot add approvals or affect scope
- A paused scope does not affect unrelated scopes
- Unauthorized calls do not modify any state

### Assumptions

- Admin address is correctly managed and not compromised
- Guardian set is trustworthy (see incident response procedures)
- Ledger time moves forward (no rollbacks)
- No contract upgrade that changes scope semantics mid-incident

## Operational guidance

### Pausing one scope

```bash
# Pause Registration scope only
soroban contract invoke ... --fn set_scoped_pause \
  --arg '{"tag":"Registration"}' --arg 'true'
```

Updates, Revocation, Upgrades, and Disputes remain operational.

### Global pause fallback behavior

```bash
# Pause everything (Global)
soroban contract invoke ... --fn propose_emergency_action \
  --arg '{"tag":"GlobalPause"}' ...
```

This pauses:
- Registration scope (fallback)
- Updates scope (fallback)

This does NOT pause:
- Revocation scope (no fallback)
- Upgrades scope (no fallback)
- Disputes scope (no fallback)

### Selective recovery

```bash
# Unpause Updates only while keeping Registration paused
soroban contract invoke ... --fn set_scoped_pause \
  --arg '{"tag":"Updates"}' --arg 'false'
```

Registration remains paused; other scopes unaffected.
