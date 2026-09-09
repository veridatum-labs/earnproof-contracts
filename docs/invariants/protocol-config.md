# Protocol Configuration Specification

## Overview & State Model

The `protocol-config` contract manages system-wide administrative control, emergency pause states, schema version authorizations, and contract upgrade governance.

### Core States
- **Uninitialized**: Contract instance exists but has no configured admin, pause state, or schema approvals.
- **Operational (Unpaused)**: Admin is configured; `paused = false`; proofs can be registered under approved schemas.
- **Contained (Paused)**: Admin is configured; `paused = true`; proof registration across the protocol is blocked; administrative and query functions remain functional.
- **Schema Lifecycle**: Each positive schema version `v > 0` is independently in one of three states: `Approved`, `Deprecated`, or `Absent`. Version `0` is permanently non-existent and invalid.
- **Upgrade Allowlist**: Each 32-byte WASM hash `wasm_hash` is either `Allowlisted(target_version)` or `Absent`.

---

## State Transition Matrix

| Transition / Method | Source State | Target State | Guard / Authorization | State Mutations & Side Effects | Emitted Event | Impossible Transitions & Errors |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `initialize` | Uninitialized | Operational (`paused = false`) | `DataKey::Admin` absent; `admin` authenticates | Stores `Admin`, sets `Paused = false`, sets `ConfigVersion = 1`, sets `ContractVersion = 1` | `Initialized` | Re-initialization: `ContractError::AlreadyInitialized`; unauthenticated caller |
| `set_admin` | Initialized | Initialized (New Admin) | Current admin authenticates (`require_auth(&admin)`) | Updates `Admin`, increments `ConfigVersion` by 1, extends instance TTL | `AdminChanged` | Unauthorized caller; former admin invoking after rotation |
| `pause` | Operational (`paused = false`) | Contained (`paused = true`) | Current admin authenticates (`require_auth(&admin)`) | Sets `Paused = true`, increments `ConfigVersion` by 1, extends instance TTL | `Paused` | Unauthorized caller; non-admin caller cannot pause |
| `unpause` | Contained (`paused = true`) | Operational (`paused = false`) | Current admin authenticates (`require_auth(&admin)`) | Sets `Paused = false`, increments `ConfigVersion` by 1, extends instance TTL | `Unpaused` | Unauthorized caller; non-admin caller cannot unpause |
| `approve_schema_version` | Any schema state | `Approved` | Current admin authenticates; `version > 0` | Sets `SchemaVersion(version) = true`, bumps TTL, increments `ConfigVersion` | `SchemaApproved` | Version 0: `ContractError::InvalidInput`; unauthorized caller |
| `deprecate_schema_version` | `Approved` | `Deprecated` | Current admin authenticates; `version > 0` | Sets `SchemaVersion(version) = false`, bumps TTL, increments `ConfigVersion` | `SchemaDeprecated` | Version 0: `ContractError::InvalidInput`; unauthorized caller |
| `approve_upgrade` | Initialized | Allowlisted | Current admin authenticates; `new_version > ContractVersion` | Sets `AllowedWasm(wasm_hash) = new_version`, extends instance TTL | `UpgradeAllowlisted` | Version downgrade (`new_version <= ContractVersion`); unauthorized caller |
| `revoke_upgrade` | Allowlisted | Absent | Current admin authenticates | Removes `AllowedWasm(wasm_hash)` | `UpgradeRevoked` | Unauthorized caller |
| `upgrade_contract` | Allowlisted | Initialized (New WASM) | Current admin authenticates; `wasm_hash` in allowlist; `target_version > ContractVersion` | Consumes allowlist entry, updates contract WASM, sets `ContractVersion = new_version` | `ContractUpgraded` | Non-allowlisted WASM hash; replay of consumed hash; version downgrade |

---

## Invariants & Safety Guarantees

1. **One-Way Initialization**: `initialize` can execute exactly once per deployment (`contracts/protocol-config/src/lib.rs::initialize`).
2. **Strict Administrative Authority**: Only the active administrator address stored in `DataKey::Admin` can execute state-mutating functions (`contracts/protocol-config/src/lib.rs::require_auth`).
3. **Monotonic Config Versioning**: Every privileged configuration mutation (`set_admin`, `pause`, `unpause`, `approve_schema_version`, `deprecate_schema_version`) strictly increments `ConfigVersion` (`contracts/protocol-config/src/lib.rs::bump_config_version`).
4. **Permanent Invalidity of Version Zero**: Schema version 0 can never be approved, deprecated, or stored (`contracts/protocol-config/src/lib.rs::ensure_nonzero_version`).
5. **Monotonic Contract Upgrades**: Contract version must strictly advance upon upgrade; downgrade versions cannot be approved or installed (`contracts/protocol-config/src/lib.rs::approve_upgrade`, `contracts/protocol-config/src/lib.rs::upgrade_contract`).
6. **Replay-Protected Upgrades**: Upgrade allowlist entries are consumed atomically prior to applying bytecode changes, preventing replay of previously approved hashes.

---

## Code and Test Linkage

### Implementation References
- Initialization: `contracts/protocol-config/src/lib.rs::initialize`
- Administration: `contracts/protocol-config/src/lib.rs::get_admin`, `contracts/protocol-config/src/lib.rs::set_admin`
- Pause Controls: `contracts/protocol-config/src/lib.rs::is_paused`, `contracts/protocol-config/src/lib.rs::pause`, `contracts/protocol-config/src/lib.rs::unpause`
- Schema Lifecycle: `contracts/protocol-config/src/lib.rs::approve_schema_version`, `contracts/protocol-config/src/lib.rs::deprecate_schema_version`, `contracts/protocol-config/src/lib.rs::is_schema_version_approved`
- Upgrade Governance: `contracts/protocol-config/src/lib.rs::approve_upgrade`, `contracts/protocol-config/src/lib.rs::revoke_upgrade`, `contracts/protocol-config/src/lib.rs::is_upgrade_allowed`, `contracts/protocol-config/src/lib.rs::upgrade_contract`

### Positive Test Coverage
- `contracts/protocol-config/src/lib.rs::initializes_config_defaults`: Verifies default parameters on initialization.
- `contracts/protocol-config/src/lib.rs::pause_and_unpause_bump_config_version`: Verifies toggle of pause state and version increments.
- `contracts/protocol-config/src/lib.rs::schema_versions_can_be_approved_and_deprecated`: Verifies valid lifecycle transitions for schema versions.
- `contracts/protocol-config/src/lib.rs::extends_schema_storage_ttl`: Verifies storage TTL extension upon schema operations.
- `contracts/protocol-config/src/lib.rs::approve_and_check_allowlist`: Verifies upgrade pre-authorization.
- `contracts/protocol-config/src/lib.rs::upgrade_contract_advances_version_and_consumes_allowlist`: Verifies execution of upgrade and consumption of allowlist entry.
- `tests/emergency/src/admin_rotation.rs::every_rotation_is_observable_through_the_config_version`: Verifies config version increases on admin transfer.
- `tests/emergency/src/pause_matrix.rs::every_entry_point_matches_its_documented_pause_behaviour`: Verifies system behavior across paused matrix.

### Negative & Rejection Test Coverage
- `contracts/protocol-config/src/lib.rs::rejects_zero_schema_version`: Asserts version 0 returns `ContractError::InvalidInput`.
- `contracts/protocol-config/src/lib.rs::approve_upgrade_rejects_downgrade_version`: Asserts rejection when proposed version is equal to current.
- `contracts/protocol-config/src/lib.rs::approve_upgrade_rejects_lower_version`: Asserts rejection when proposed version is lower than current.
- `contracts/protocol-config/src/lib.rs::upgrade_contract_rejects_non_allowlisted_hash`: Asserts rejection for unapproved WASM hashes.
- `contracts/protocol-config/src/lib.rs::upgrade_contract_requires_admin_auth`: Asserts rejection when non-admin calls upgrade.
- `contracts/protocol-config/src/lib.rs::upgrade_contract_hash_cannot_be_replayed`: Asserts consumed WASM hashes cannot be re-executed.
- `contracts/protocol-config/src/lib.rs::cannot_re_approve_old_version_after_upgrade`: Asserts old versions cannot be re-approved post-upgrade.
- `tests/emergency/src/admin_rotation.rs::pause_authority_follows_rotation_and_does_not_stay_with_the_former_admin`: Verifies old admin cannot pause post-rotation.
- `tests/emergency/src/admin_rotation.rs::unpause_requires_the_current_admin`: Verifies old admin cannot unpause post-rotation.
- `tests/emergency/src/sequences.rs::initialize_is_rejected_on_an_already_initialized_deployment`: Verifies duplicate initialization fails.
- `tests/events/src/ghost.rs::reinitializing_protocol_config_emits_no_event`: Asserts zero events on failed re-initialization.

---

## Security Property Mapping

- **Authorization Guards**: [Threat Model - T1: Authorization Bypass](../threat-model.md#t1-authorization-bypass)
- **Admin Compromise Controls**: [Threat Model - T4: Compromised Admin Key](../threat-model.md#t4-compromised-admin-key)
- **Emergency Pause Containment**: [Threat Model - T5: Protocol Pause Abuse](../threat-model.md#t5-protocol-pause-abuse--denial-of-service)
- **Schema Validation & Gating**: [Threat Model - T11: Unapproved Schema Version Bypass](../threat-model.md#t11-unapproved-schema-version-bypass)