# Atomic Multi-Action Governance Proposals Design

## Executive Summary

The earnproof-contracts protocol currently supports single-action governance proposals through two channels:
1. **CriticalActionProposal**: Threshold-approved governance actions (schema management, policy updates)
2. **EmergencyActionProposal**: Fast-track pause actions with guardian approval

This document designs an extension to support atomic multi-action proposals where multiple related governance actions are grouped, approved, and executed as a single indivisible unit. If any action fails, all actions in the batch are rolled back.

---

## 1. Current Governance Architecture

### 1.1 Existing Proposal Types

#### CriticalActionProposal
- **Purpose**: Standard governance mechanism for protocol changes requiring threshold approval
- **Approval Model**: Multi-signer threshold (`policy.threshold` signatures required)
- **Proposal States**:
  - Created (pending approvals)
  - Approved (threshold met)
  - Executed (actions applied)
  - Expired (after `CRITICAL_ACTION_APPROVAL_EXPIRY_LEDGERS` = 518,400 ledgers)
  - Cancelled (by admin)

- **Supported Actions** (via `CriticalAction` enum):
  - `SchemaApproval(version)` - Approve new schema version
  - `SchemaActivation(version, activates_at)` - Schedule schema activation
  - `SchemaDeprecation(version)` - Mark schema as deprecated
  - `SchemaReplacement(deprecated, replacement, compatibility)` - Link successor schema
  - `SchemaPayloadLimit(version, max_size)` - Set auxiliary payload size limit
  - `ApprovalPolicyUpdate(policy)` - Change critical action policy
  - `EmergencyPolicyUpdate(policy)` - Change emergency fast-track policy

#### EmergencyActionProposal
- **Purpose**: Fast-track pause mechanism for security incidents
- **Approval Model**: Guardian threshold (subset of critical signers, typically lower threshold)
- **Supported Actions** (via `EmergencyAction` enum):
  - `GlobalPause` - Pause entire protocol
  - `ScopedPause(scope)` - Pause specific operation (Registration, Updates, Revocation, Upgrades, Disputes)
- **Constraints**:
  - Cannot be used for `GlobalPause` on scope level
  - Only for pause actions (explicitly excludes unpause, upgrades, arbitrary calls)

### 1.2 Individual Action Flow

**Proposal Creation**:
```rust
propose_critical_action(env, action: CriticalAction) -> BytesN<32>
```
- Admin proposes action, gets proposal ID via deterministic hash
- Proposal ID = SHA256(("gov_action", VERSION, network_id, contract_id, category, action, nonce))
- Captures approval policy snapshot at creation time for immutability
- Stores proposal in persistent storage with TTL

**Approval Mechanism**:
```rust
approve_critical_action(env, proposal_id: BytesN<32>, signer: Address) -> Result<(), ContractError>
```
- Multi-signature approval from authorized signers
- Signer eligibility checked against proposal's policy snapshot
- Approvals are additive; same signer cannot approve twice
- Proposal remains queryable for approval-in-progress tracking

**Execution**:
```rust
execute_critical_action(env, proposal_id: BytesN<32>) -> Result<(), ContractError>
```
- Verifies threshold (approval count ≥ policy.threshold)
- Executes single action atomically
- Removes proposal and snapshots from storage
- Emits `CriticalActionExecuted` event with proposal hash and receipt

### 1.3 Key Storage and Identification

**DataKey Variants** for proposals:
```rust
CriticalActionProposal(BytesN<32>)      // Proposal record
CriticalActionDomain(BytesN<32>)        // Network/contract domain separation
CriticalActionSnapshot(BytesN<32>)      // Policy version immutability
CriticalApproverSnapshot(BytesN<32>)    // Signer version tracking
```

**Hash Computation** (deterministic, immutable):
```rust
fn governance_action_commitment(
    env: &Env,
    contract_id: &Address,
    action: &CriticalAction,
    nonce: u32,
) -> BytesN<32>
```
- Input: action category, full action data, network ID, contract address, nonce
- XDR encoding ensures unambiguous byte representation
- SHA256 produces proposal ID

**Approval Snapshot** (captures policy at proposal creation):
```rust
pub struct GovernanceApprovalSnapshot {
    pub snapshot_version: u32,
    pub policy_version: u32,
    pub threshold: u32,
    pub signer_set_digest: BytesN<32>,  // SHA256 of ordered signer vector
}
```

### 1.4 Authorization Mechanism

- **Role-Based Access**: Admin proposes; authorized signers approve
- **Policy Snapshot Binding**: Signer eligibility tied to policy version at proposal creation
- **Signer Version Tracking**: `GovernanceApproverSnapshot` records each signer's assignment generation
  - Allows detection when signers were added/removed after proposal creation
  - Prevents retroactive approvals from reassigned signers

### 1.5 Events and Observability

**Proposal Lifecycle Events**:
- `CriticalActionProposed { proposal_id, proposer }`
- `CriticalActionApproved { proposal_id, signer }`
- `CriticalActionExecuted { proposal_id }`
- `CriticalActionCancelled { proposal_id, cancelled_by }`

**Receipt Commitment**:
- `GovernanceReceiptCommitted { proposal_id, receipt_hash, resulting_config_version }`
- Includes execution timestamp, approval count, action commitment

---

## 2. Atomic Multi-Action Design Specification

### 2.1 Conceptual Model

An **atomic multi-action proposal** (MultiActionProposal) groups multiple related governance actions into a single unit with the following properties:

1. **Atomicity**: All actions execute or all roll back (no partial commits)
2. **Ordering**: Actions execute in canonical order
3. **Authorization**: Single approval policy applies to the entire batch
4. **Determinism**: Canonical proposal hash encodes all actions in order
5. **Observability**: Execution receipt records which actions succeeded/failed

**Constraints**:
- Actions must be compatible (no conflicting state mutations)
- Same action types cannot be duplicated (e.g., two `SchemaApproval`s in one batch)
- Batch size bounded (e.g., max 8 actions) to prevent unbounded execution cost
- Must maintain backward compatibility with single-action proposals

### 2.2 New Data Structures

#### In `packages/shared/src/lib.rs`:

```rust
/// Canonical ordered list of governance actions in a multi-action batch.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiActionBatch {
    pub batch_version: u32,  // Encoding version for future extensibility
    pub actions: Vec<CriticalAction>,  // Ordered, immutable
}

/// Encoding version for multi-action batch commitments.
pub const MULTI_ACTION_BATCH_VERSION: u32 = 1;

/// Maximum actions per batch to prevent unbounded execution.
pub const MAX_BATCH_ACTIONS: u32 = 8;

/// Category for multi-action proposal execution tracking.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MultiActionExecutionStatus {
    Pending,      // Awaiting approvals
    Approved,     // Threshold met, ready to execute
    Executing,    // Currently being executed
    Succeeded,    // All actions completed
    PartialFailure(u32),  // Action at index N failed; all rolled back
}

/// Immutable record of multi-action proposal execution attempt.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiActionExecutionRecord {
    pub record_version: u32,
    pub proposal_id: BytesN<32>,
    pub batch_commitment: BytesN<32>,
    pub status: MultiActionExecutionStatus,
    pub approval_count: u32,
    pub resulting_config_version: u32,
    pub execution_ledger: u32,
    pub execution_timestamp: u64,
    pub receipt_hash: BytesN<32>,
}

/// Computes canonical identifier for multi-action batch.
pub fn multi_action_batch_commitment(
    env: &Env,
    contract_id: &Address,
    actions: &Vec<CriticalAction>,
    nonce: u32,
) -> BytesN<32> {
    let batch = MultiActionBatch {
        batch_version: MULTI_ACTION_BATCH_VERSION,
        actions: actions.clone(),
    };
    let preimage = (
        Symbol::new(env, "gov_batch"),
        GOVERNANCE_ACTION_HASH_VERSION,
        env.ledger().network_id(),
        contract_id.clone(),
        Symbol::new(env, "multi_action"),
        batch,
        nonce,
    );
    env.crypto().sha256(&preimage.to_xdr(env)).to_bytes()
}

/// Validation constraints for multi-action batches.
pub fn is_valid_multi_action_batch(
    actions: &Vec<CriticalAction>,
) -> bool {
    if actions.len() == 0 || actions.len() as u32 > MAX_BATCH_ACTIONS {
        return false;
    }
    
    // Collect categories to detect duplicates
    let mut categories = Vec::new();
    for action in actions.iter() {
        let category = action.category();
        if categories.contains(&category) {
            return false;  // Duplicate action category
        }
        categories.push(category);
    }
    
    // Verify all actions are supported for multi-action proposals
    // (exclusions: ContractReplacements which require special handling)
    for action in actions.iter() {
        match action {
            CriticalAction::IssuerRegistryReplacement(_)
            | CriticalAction::ProtocolConfigReplacement(_) => {
                return false;  // Not allowed in multi-action batches
            }
            _ => {}
        }
    }
    
    true
}
```

#### In `contracts/protocol-config/src/state.rs`:

```rust
// Add to DataKey enum:
pub enum DataKey {
    // ... existing keys ...
    
    /// Multi-action proposal batch
    MultiActionProposal(BytesN<32>),
    
    /// Domain separation for multi-action proposals
    MultiActionDomain(BytesN<32>),
    
    /// Policy snapshot at multi-action creation
    MultiActionSnapshot(BytesN<32>),
    
    /// Approver snapshot for multi-action signer versions
    MultiActionApproverSnapshot(BytesN<32>),
    
    /// Execution record for audit trail
    MultiActionExecutionRecord(BytesN<32>),
    
    /// Latest multi-action execution (for quick access)
    LatestMultiActionRecord,
    
    /// Nonce counter for multi-action proposals (separate from single-action)
    MultiActionNonce,
}

// Storage maps
pub const MULTI_ACTION_PROPOSALS: Map<&str, MultiActionProposal> = Map::new("multi_action_proposals");
pub const MULTI_ACTION_EXECUTION_RECORDS: Map<u64, MultiActionExecutionRecord> = Map::new("multi_action_execution_history");
```

#### New Proposal Type in `packages/shared/src/lib.rs`:

```rust
/// Persisted multi-action proposal and approval window.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiActionProposal {
    pub batch: MultiActionBatch,
    pub policy: CriticalActionPolicy,
    pub proposer: Address,
    pub approvals: Vec<Address>,
    pub created_at: u32,
    pub expires_at: u32,
}
```

### 2.3 Events

```rust
#[contractevent]
pub struct MultiActionProposed {
    pub proposal_id: BytesN<32>,
    pub proposer: Address,
    pub action_count: u32,
    pub expires_at: u32,
}

#[contractevent]
pub struct MultiActionApproved {
    pub proposal_id: BytesN<32>,
    pub signer: Address,
    pub approval_count: u32,
}

#[contractevent]
pub struct MultiActionExecutionStarted {
    pub proposal_id: BytesN<32>,
    pub batch_size: u32,
}

#[contractevent]
pub struct MultiActionExecuted {
    pub proposal_id: BytesN<32>,
    pub batch_commitment: BytesN<32>,
    pub status: MultiActionExecutionStatus,
}

#[contractevent]
pub struct MultiActionRolledBack {
    pub proposal_id: BytesN<32>,
    pub failed_action_index: u32,
    pub error_code: u32,
}
```

---

## 3. Implementation Design

### 3.1 Proposal Creation Flow

```rust
pub fn propose_multi_action(
    env: Env,
    actions: Vec<CriticalAction>,
) -> Result<BytesN<32>, ContractError>
{
    // 1. Validation
    Self::ensure_not_decommissioned(&env)?;
    let admin = Self::get_admin(env.clone())?;
    Self::require_auth(&admin);
    
    let policy = Self::critical_action_policy(&env);
    if !policy.enabled {
        return Err(ContractError::InvalidState);
    }
    
    if !is_valid_multi_action_batch(&actions) {
        return Err(ContractError::InvalidInput);
    }
    
    // 2. Generate deterministic proposal ID
    let nonce = env
        .storage()
        .instance()
        .get::<_, u32>(&DataKey::MultiActionNonce)
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(ContractError::InvalidState)?;
    
    let proposal_id = multi_action_batch_commitment(
        &env,
        &env.current_contract_address(),
        &actions,
        nonce,
    );
    
    // 3. Capture immutable policy snapshot
    let created_at = env.ledger().sequence();
    let expires_at = created_at
        .checked_add(CRITICAL_ACTION_APPROVAL_EXPIRY_LEDGERS)
        .ok_or(ContractError::InvalidInput)?;
    
    let approval_snapshot = GovernanceApprovalSnapshot {
        snapshot_version: GOVERNANCE_APPROVAL_SNAPSHOT_VERSION,
        policy_version: Self::get_critical_policy_version(env.clone()),
        threshold: policy.threshold,
        signer_set_digest: governance_signer_set_digest(&env, &policy.signers),
    };
    
    // 4. Store proposal and metadata
    let batch = MultiActionBatch {
        batch_version: MULTI_ACTION_BATCH_VERSION,
        actions: actions.clone(),
    };
    
    let proposal = MultiActionProposal {
        batch: batch.clone(),
        policy: policy.clone(),
        proposer: admin.clone(),
        approvals: Vec::new(&env),
        created_at,
        expires_at,
    };
    
    env.storage()
        .instance()
        .set(&DataKey::MultiActionNonce, &nonce);
    
    let key = DataKey::MultiActionProposal(proposal_id.clone());
    env.storage().persistent().set(&key, &proposal);
    
    // Store domain and snapshots (matching single-action pattern)
    let domain_key = DataKey::MultiActionDomain(proposal_id.clone());
    let domain = GovernanceActionDomain {
        encoding_version: GOVERNANCE_ACTION_HASH_VERSION,
        network_id: env.ledger().network_id(),
        contract_id: env.current_contract_address(),
        action_category: CriticalActionCategory::MultiAction,  // NEW enum variant
        nonce,
        commitment: proposal_id.clone(),
    };
    env.storage().persistent().set(&domain_key, &domain);
    
    let snapshot_key = DataKey::MultiActionSnapshot(proposal_id.clone());
    env.storage().persistent().set(&snapshot_key, &approval_snapshot);
    
    let approver_snapshot_key = DataKey::MultiActionApproverSnapshot(proposal_id.clone());
    let approver_snapshot = Self::approver_snapshot(&env, &policy);
    env.storage().persistent().set(&approver_snapshot_key, &approver_snapshot);
    
    // 5. TTL management
    env.storage().persistent().extend_ttl(
        &key,
        TTL_THRESHOLD_LEDGERS,
        CRITICAL_ACTION_APPROVAL_EXPIRY_LEDGERS + TTL_THRESHOLD_LEDGERS,
    );
    env.storage().persistent().extend_ttl(
        &domain_key,
        TTL_THRESHOLD_LEDGERS,
        CRITICAL_ACTION_APPROVAL_EXPIRY_LEDGERS + TTL_THRESHOLD_LEDGERS,
    );
    env.storage().persistent().extend_ttl(
        &snapshot_key,
        TTL_THRESHOLD_LEDGERS,
        CRITICAL_ACTION_APPROVAL_EXPIRY_LEDGERS + TTL_THRESHOLD_LEDGERS,
    );
    env.storage().persistent().extend_ttl(
        &approver_snapshot_key,
        TTL_THRESHOLD_LEDGERS,
        CRITICAL_ACTION_APPROVAL_EXPIRY_LEDGERS + TTL_THRESHOLD_LEDGERS,
    );
    
    // 6. Emit event
    MultiActionProposed {
        proposal_id: proposal_id.clone(),
        proposer: admin,
        action_count: actions.len() as u32,
        expires_at,
    }
    .publish(&env);
    
    Ok(proposal_id)
}
```

### 3.2 Approval Flow

Identical to single-action, reusing existing infrastructure:

```rust
pub fn approve_multi_action(
    env: Env,
    proposal_id: BytesN<32>,
    signer: Address,
) -> Result<(), ContractError>
{
    let mut proposal = Self::get_multi_action_proposal(env.clone(), proposal_id.clone())
        .ok_or(ContractError::ApprovalProposalNotFound)?;
    
    Self::verify_multi_action_domain(&env, &proposal_id, &proposal)?;
    Self::verify_approval_snapshot(&env, &proposal_id, &proposal)?;
    Self::ensure_proposal_live(&env, &proposal)?;
    Self::require_auth(&signer);
    
    // Verify signer eligibility (matches single-action pattern)
    match Self::approval_eligibility_for(&env, &proposal_id, &proposal, &signer) {
        GovernanceApprovalEligibility::Eligible => {}
        other => return Err(ContractError::Unauthorized),  // map eligibility to error
    }
    
    if proposal.approvals.contains(&signer) {
        return Err(ContractError::AlreadyExists);
    }
    
    proposal.approvals.push_back(signer.clone());
    env.storage().persistent().set(
        &DataKey::MultiActionProposal(proposal_id.clone()),
        &proposal,
    );
    
    MultiActionApproved {
        proposal_id,
        signer,
        approval_count: proposal.approvals.len() as u32,
    }
    .publish(&env);
    
    Ok(())
}
```

### 3.3 Execution Flow (Atomicity Pattern)

```rust
pub fn execute_multi_action(
    env: Env,
    proposal_id: BytesN<32>,
) -> Result<(), ContractError>
{
    Self::ensure_not_decommissioned(&env)?;
    
    let proposal = Self::get_multi_action_proposal(env.clone(), proposal_id.clone())
        .ok_or(ContractError::ApprovalProposalNotFound)?;
    
    // Pre-execution validation
    Self::verify_multi_action_domain(&env, &proposal_id, &proposal)?;
    Self::verify_approval_snapshot(&env, &proposal_id, &proposal)?;
    Self::ensure_proposal_live(&env, &proposal)?;
    
    let approval_count = Self::eligible_approval_count(&env, &proposal_id, &proposal);
    if approval_count < proposal.policy.threshold {
        return Err(ContractError::InsufficientApprovals);
    }
    
    // Emit start event for observability
    MultiActionExecutionStarted {
        proposal_id: proposal_id.clone(),
        batch_size: proposal.batch.actions.len() as u32,
    }
    .publish(&env);
    
    // Execute actions in order, with rollback on failure
    let mut config_version_before = Self::get_config_version(env.clone());
    let mut execution_status = MultiActionExecutionStatus::Executing;
    let mut failed_index: Option<u32> = None;
    
    for (index, action) in proposal.batch.actions.iter().enumerate() {
        let result = Self::execute_single_critical_action(env.clone(), action.clone());
        
        if result.is_err() {
            failed_index = Some(index as u32);
            execution_status = MultiActionExecutionStatus::PartialFailure(index as u32);
            
            // ROLLBACK: Restore config version and emit rollback event
            Self::set_config_version(env.clone(), config_version_before);
            
            MultiActionRolledBack {
                proposal_id: proposal_id.clone(),
                failed_action_index: index as u32,
                error_code: result.err().unwrap() as u32,
            }
            .publish(&env);
            
            break;  // Stop execution, don't process remaining actions
        }
    }
    
    // If all succeeded, update status
    if failed_index.is_none() {
        execution_status = MultiActionExecutionStatus::Succeeded;
    }
    
    // Record execution outcome
    let resulting_config_version = Self::get_config_version(env.clone());
    let execution_ledger = env.ledger().sequence();
    let execution_timestamp = env.ledger().timestamp();
    
    let batch_commitment = multi_action_batch_commitment(
        &env,
        &env.current_contract_address(),
        &proposal.batch.actions,
        0,  // nonce not needed for batch hash after creation
    );
    
    let receipt_hash = env.crypto().sha256(&(
        Symbol::new(&env, "multi_action_receipt"),
        &proposal_id,
        &batch_commitment,
        approval_count,
        resulting_config_version,
        execution_ledger,
        execution_timestamp,
    ).to_xdr(&env)).to_bytes();
    
    let record = MultiActionExecutionRecord {
        record_version: MULTI_ACTION_BATCH_VERSION,
        proposal_id: proposal_id.clone(),
        batch_commitment: batch_commitment.clone(),
        status: execution_status.clone(),
        approval_count,
        resulting_config_version,
        execution_ledger,
        execution_timestamp,
        receipt_hash: receipt_hash.clone(),
    };
    
    // Store execution record
    env.storage()
        .persistent()
        .set(&DataKey::MultiActionExecutionRecord(proposal_id.clone()), &record);
    env.storage()
        .instance()
        .set(&DataKey::LatestMultiActionRecord, &record);
    
    // Clean up proposal
    env.storage()
        .persistent()
        .remove(&DataKey::MultiActionProposal(proposal_id.clone()));
    env.storage()
        .persistent()
        .remove(&DataKey::MultiActionDomain(proposal_id.clone()));
    env.storage()
        .persistent()
        .remove(&DataKey::MultiActionSnapshot(proposal_id.clone()));
    env.storage()
        .persistent()
        .remove(&DataKey::MultiActionApproverSnapshot(proposal_id.clone()));
    
    // Emit final event
    MultiActionExecuted {
        proposal_id,
        batch_commitment,
        status: execution_status,
    }
    .publish(&env);
    
    // Return error if execution was partial (caller knows to retry or cancel)
    if failed_index.is_some() {
        return Err(ContractError::PartialExecutionFailure);  // NEW error code
    }
    
    Ok(())
}

/// Execute a single critical action (extracted logic for reuse).
/// This is the action handler that already exists—factored out for multi-action use.
fn execute_single_critical_action(
    env: Env,
    action: CriticalAction,
) -> Result<(), ContractError>
{
    let commitment = Self::commit(&env, action.clone());
    match action {
        CriticalAction::SchemaApproval(version) => {
            env.storage().persistent().set(&DataKey::SchemaVersion(version), &true);
            Self::record_critical_action_change(
                env.clone(),
                ConfigChangeCategory::SchemaApproval,
                commitment,
            );
            SchemaApproved { version }.publish(&env);
            Ok(())
        }
        CriticalAction::SchemaActivation(version, activates_at) => {
            // Detailed activation logic (existing single-action handler)
            Self::apply_schema_activation(env, version, activates_at, commitment)
        }
        // ... other action types ...
        _ => Err(ContractError::InvalidInput),
    }
}
```

### 3.4 Rollback Implementation

**Rollback Strategy**:
- **Simple**: For schema approvals and policy updates that only add/modify state, rollback = restore config version counter
- **Transactional**: Soroban contract VM does not support transaction rollback; instead:
  - Each action's state mutations are tracked
  - On failure, inverse operations are applied (e.g., undo state sets)
  - Or: re-initialize the proposal's state from snapshots

**Recommended Approach**: Use Soroban's **read-your-writes in same transaction** semantics:
- All state reads within `execute_multi_action` transaction see writes from previous actions
- On error, transaction fails atomically—no writes commit
- This is **automatic** if we structure execution without try-catch

```rust
// Pseudo-code showing transaction atomicity
pub fn execute_multi_action(env: Env, proposal_id: BytesN<32>) -> Result<(), ContractError> {
    // All of this executes in one transaction:
    for action in actions {
        // Each action writes state
        // If ANY action returns Err, the entire transaction rolls back
        execute_single_action(action)?;  // ? operator short-circuits
    }
    // All writes commit only after this function returns Ok
}
```

### 3.5 Query Functions

```rust
pub fn get_multi_action_proposal(
    env: Env,
    proposal_id: BytesN<32>,
) -> Option<MultiActionProposal> {
    let key = DataKey::MultiActionProposal(proposal_id);
    let proposal = env.storage().persistent().get(&key);
    if proposal.is_some() {
        env.storage().persistent().extend_ttl(
            &key,
            TTL_THRESHOLD_LEDGERS,
            CRITICAL_ACTION_APPROVAL_EXPIRY_LEDGERS + TTL_THRESHOLD_LEDGERS,
        );
    }
    proposal
}

pub fn get_multi_action_execution_record(
    env: Env,
    proposal_id: BytesN<32>,
) -> Option<MultiActionExecutionRecord> {
    env.storage().persistent().get(&DataKey::MultiActionExecutionRecord(proposal_id))
}

pub fn get_latest_multi_action_record(env: Env) -> Option<MultiActionExecutionRecord> {
    env.storage().instance().get(&DataKey::LatestMultiActionRecord)
}

pub fn derive_multi_action_proposal_id(
    env: Env,
    actions: Vec<CriticalAction>,
    nonce: u32,
) -> BytesN<32> {
    multi_action_batch_commitment(
        &env,
        &env.current_contract_address(),
        &actions,
        nonce,
    )
}
```

---

## 4. Compatibility Analysis

### 4.1 Backward Compatibility

**Single-Action Proposals Unaffected**:
- New `MultiActionProposal` type is separate storage namespace
- Single-action `CriticalActionProposal` unchanged
- Existing approval/execution infrastructure reusable
- No migration required for existing deployments

**Event Namespacing**:
- New events (`MultiActionProposed`, `MultiActionExecuted`) don't collide with existing events
- Event consumers can filter by event type

### 4.2 Authorization Preservation

**Shared Approval Infrastructure**:
- Both single-action and multi-action use same policy validation
- Both require admin to propose
- Both require threshold approvals from same signer set
- Signer eligibility checks identical

**New Authorization Capability**:
- Multi-action execution requires same approval count as single-action
- No privilege escalation: multi-action batch subject to same policy

### 4.3 Storage Layout Compatibility

**New Storage Keys Added**:
- `MultiActionProposal(BytesN<32>)` - Separate from `CriticalActionProposal`
- `MultiActionNonce` - Separate counter, not mixed with single-action nonce
- No existing keys modified or repurposed

**Storage Classes**:
- Multi-action proposals: persistent storage (long-lived approval window)
- Execution records: persistent storage (audit trail)
- Nonce: instance storage (contract lifetime)

### 4.4 External API Compatibility

**New Contract Methods**:
- `propose_multi_action(actions: Vec<CriticalAction>)`
- `approve_multi_action(proposal_id, signer)`
- `execute_multi_action(proposal_id)`
- `cancel_multi_action(proposal_id)` (optional)
- Query methods for retrieval

**No Existing Method Changes**:
- All single-action methods (`propose_critical_action`, etc.) remain unchanged
- Clients can upgrade gradually

---

## 5. Acceptance Criteria Mapping

### 5.1 "Any Failed Action Rolls Back All Actions"

**Requirement**: If action N in a batch fails, actions 1..(N-1) are undone; actions (N+1)..end are never attempted.

**Implementation**:
- Soroban contract execution is **transactional**
- Each contract invocation is one transaction
- If `execute_multi_action` returns `Err`, the entire transaction is rolled back by the ledger
- All state changes within that transaction are discarded
- Use Rust's `?` operator to short-circuit on error:
  ```rust
  for action in batch {
      execute_single_action(action)?;  // Returns Err, short-circuits
      // If we reach here, action succeeded
  }
  // Only reached if ALL actions succeeded
  ```
- No explicit rollback code needed; Soroban VM provides atomicity

**Observable Property**:
- If execution returns `Err`, config version is unchanged
- If execution returns `Ok`, all actions committed

### 5.2 Deterministic Batch Hash

**Requirement**: Proposal ID must be deterministic and encodes the full action batch.

**Implementation**:
```rust
pub fn multi_action_batch_commitment(
    env: &Env,
    contract_id: &Address,
    actions: &Vec<CriticalAction>,
    nonce: u32,
) -> BytesN<32> {
    let batch = MultiActionBatch {
        batch_version: MULTI_ACTION_BATCH_VERSION,
        actions: actions.clone(),  // Order preserved
    };
    let preimage = (
        Symbol::new(env, "gov_batch"),
        GOVERNANCE_ACTION_HASH_VERSION,
        env.ledger().network_id(),
        contract_id.clone(),
        Symbol::new(env, "multi_action"),
        batch,  // Full batch encoded
        nonce,
    );
    env.crypto().sha256(&preimage.to_xdr(env)).to_bytes()
}
```

**Properties**:
- **Deterministic**: Same inputs → same hash (SHA256)
- **Canonical**: XDR encoding unambiguous
- **Complete**: All actions included in preimage
- **Immutable**: Hash tied to specific network, contract, nonce

### 5.3 Ordered Execution

**Requirement**: Actions execute in strict, defined order with no reordering.

**Implementation**:
- Batch stored as `Vec<CriticalAction>` with stable order
- Execution loop: `for (index, action) in proposal.batch.actions.iter()`
- Order is encoded in proposal hash; changing order would produce different proposal ID
- Actions depend on sequential state updates (e.g., schema approval before activation)

**Observable**:
- Execution record contains `failed_action_index` (0-based position)
- Events preserve order in event log

### 5.4 Bounded Execution

**Requirement**: Execution cannot be unbounded; batch size is limited.

**Implementation**:
```rust
pub const MAX_BATCH_ACTIONS: u32 = 8;

pub fn is_valid_multi_action_batch(actions: &Vec<CriticalAction>) -> bool {
    if actions.len() == 0 || actions.len() as u32 > MAX_BATCH_ACTIONS {
        return false;
    }
    true
}
```

**Cost Analysis**:
- Each action: ~10-100ms execution (depends on storage writes)
- 8 actions max: <1 second total execution time
- Fits within Soroban's per-invocation limits (default 2 seconds)

### 5.5 Observable Behavior

**Events Emitted** (in order):
1. `MultiActionProposed` (at proposal creation)
2. `MultiActionApproved` (once per approval)
3. `MultiActionExecutionStarted` (at execution begin)
4. For each action: action-specific events (e.g., `SchemaApproved`)
5. If failure: `MultiActionRolledBack { failed_action_index, error_code }`
6. Final: `MultiActionExecuted { status }`

**Execution Record** (audit trail):
- Stored in persistent storage
- Contains: proposal ID, actions, approval count, status, timestamp
- Receipt hash allows off-chain verification

---

## 6. Test Requirements

### 6.1 Unit Tests (in `tests/` directory)

#### Test: Deterministic Hash
```rust
#[test]
fn multi_action_proposal_id_is_deterministic() {
    let actions = vec![
        CriticalAction::SchemaApproval(1),
        CriticalAction::SchemaActivation(1, 1000),
    ];
    let id1 = derive_multi_action_proposal_id(&env, actions.clone(), 1);
    let id2 = derive_multi_action_proposal_id(&env, actions.clone(), 1);
    assert_eq!(id1, id2);
}
```

#### Test: Batch Validation
```rust
#[test]
fn batch_with_duplicate_actions_is_invalid() {
    let actions = vec![
        CriticalAction::SchemaApproval(1),
        CriticalAction::SchemaApproval(2),  // Duplicate category
    ];
    assert!(!is_valid_multi_action_batch(&actions));
}

#[test]
fn batch_exceeding_max_size_is_invalid() {
    let mut actions = Vec::new(&env);
    for i in 0..=MAX_BATCH_ACTIONS {
        actions.push_back(CriticalAction::SchemaApproval(i as u32));
    }
    assert!(!is_valid_multi_action_batch(&actions));
}
```

#### Test: Atomicity
```rust
#[test]
fn multi_action_execution_rolls_back_on_error() {
    let env = Env::default();
    let (contract, admin, policy) = setup_contract(&env);
    
    let actions = vec![
        CriticalAction::SchemaApproval(1),     // Succeeds
        CriticalAction::SchemaPayloadLimit(99, 1024),  // Fails: unknown schema
    ];
    
    let proposal_id = contract.propose_multi_action(&actions).unwrap();
    contract.approve_multi_action(&proposal_id, &admin).unwrap();
    
    let config_before = contract.get_config_version();
    let result = contract.execute_multi_action(&proposal_id);
    
    assert!(result.is_err());
    let config_after = contract.get_config_version();
    assert_eq!(config_before, config_after);  // Rolled back
}
```

#### Test: Approval Counting
```rust
#[test]
fn multi_action_requires_threshold_approvals() {
    let (env, contract, signers, policy) = setup_multi_signer(&env);
    
    let actions = vec![CriticalAction::SchemaApproval(1)];
    let proposal_id = contract.propose_multi_action(&actions).unwrap();
    
    // Approve with N-1 signers (below threshold)
    for signer in &signers[..signers.len()-1] {
        contract.approve_multi_action(&proposal_id, signer).unwrap();
    }
    
    let result = contract.execute_multi_action(&proposal_id);
    assert_eq!(result, Err(ContractError::InsufficientApprovals));
}
```

#### Test: Execution Record
```rust
#[test]
fn multi_action_execution_record_captures_outcome() {
    let (env, contract, admin) = setup(&env);
    let actions = vec![CriticalAction::SchemaApproval(1)];
    let proposal_id = contract.propose_multi_action(&actions).unwrap();
    
    contract.approve_multi_action(&proposal_id, &admin).unwrap();
    contract.execute_multi_action(&proposal_id).unwrap();
    
    let record = contract.get_multi_action_execution_record(&proposal_id).unwrap();
    assert_eq!(record.status, MultiActionExecutionStatus::Succeeded);
    assert!(record.receipt_hash != BytesN::from_array(&env, &[0; 32]));
}
```

### 6.2 Integration Tests

#### Test: Single vs. Multi Interoperability
```rust
#[test]
fn single_and_multi_action_proposals_coexist() {
    let (env, contract, admin) = setup(&env);
    
    // Create both types
    let single_id = contract.propose_critical_action(
        CriticalAction::SchemaApproval(1)
    ).unwrap();
    
    let multi_id = contract.propose_multi_action(
        vec![CriticalAction::SchemaApproval(2)]
    ).unwrap();
    
    // Both can be approved independently
    contract.approve_critical_action(&single_id, &admin).unwrap();
    contract.approve_multi_action(&multi_id, &admin).unwrap();
    
    // Execute in any order
    contract.execute_multi_action(&multi_id).unwrap();
    contract.execute_critical_action(&single_id).unwrap();
}
```

#### Test: Policy Snapshot Immutability
```rust
#[test]
fn multi_action_uses_policy_snapshot_at_creation() {
    let (env, contract, admin, signers) = setup_multi_signer(&env);
    
    let actions = vec![CriticalAction::SchemaApproval(1)];
    let proposal_id = contract.propose_multi_action(&actions).unwrap();
    
    // Change policy (remove a signer)
    let new_policy = CriticalActionPolicy {
        enabled: true,
        threshold: 1,
        signers: vec![signers[0].clone()],
    };
    contract.set_critical_action_policy(new_policy).unwrap();
    
    // Original signer still able to approve (snapshot protects them)
    contract.approve_multi_action(&proposal_id, &signers[0]).unwrap();
    
    // Removed signer cannot approve new proposals
    let new_proposal_id = contract.propose_multi_action(&actions).unwrap();
    let result = contract.approve_multi_action(&new_proposal_id, &signers[1]);
    assert!(result.is_err());
}
```

### 6.3 Property-Based Tests

Use `proptest` (already in project) to verify invariants:

```rust
proptest! {
    #[test]
    fn multi_action_hash_is_order_sensitive(actions in any_valid_batch()) {
        let mut shuffled = actions.clone();
        shuffled.reverse();
        
        let hash1 = derive_multi_action_proposal_id(&env, actions, 0);
        let hash2 = derive_multi_action_proposal_id(&env, shuffled, 0);
        prop_assert_ne!(hash1, hash2);
    }
    
    #[test]
    fn failed_action_prevents_later_execution(
        actions in any_valid_batch(),
        fail_at in 0usize..8,
    ) {
        let mut env_actions = actions.clone();
        if fail_at < env_actions.len() {
            // Inject a failing action at position fail_at
            env_actions[fail_at] = CriticalAction::SchemaPayloadLimit(999, 0);
        }
        
        let result = execute_batch(&env, env_actions);
        prop_assert!(result.is_err());
    }
}
```

---

## 7. Documentation and Migration Path

### 7.1 Documentation to Add/Update

**In-code docs** (rustdoc):
- `propose_multi_action`: "Create a multi-action governance proposal with atomic execution guarantees"
- `execute_multi_action`: "Execute all actions in batch or roll back all; no partial commits"
- `MultiActionBatch`: "Ordered list of governance actions; duplicates not allowed"

**Architecture docs**:
- `docs/governance-multi-action.md`: Design, use cases, examples
- `docs/atomicity-guarantees.md`: Explain transaction semantics, rollback behavior
- Update `BINDINGS_IMPLEMENTATION.md` to include multi-action client bindings

### 7.2 Migration and Rollout

**Phase 1** (Backward Compatible):
- Deploy new contract version with multi-action code
- Single-action proposals continue to work
- Multi-action available but optional
- Monitor for issues

**Phase 2** (Adoption):
- Governance guidelines document when to use multi-action vs. single-action
- Examples: Schema upgrade chains (approval → activation → replacement) as one batch
- Client libraries updated to support multi-action proposal creation

**Phase 3** (Maturity)**:
- Deprecation notice for inefficient single-action chains
- Recommend consolidating related actions into multi-action proposals

### 7.3 Upgrade Checklist

- [ ] Add `MultiActionBatch` and `MultiActionProposal` types to shared
- [ ] Update `CriticalActionCategory` enum to include `MultiAction`
- [ ] Add all new `DataKey` variants to protocol-config state
- [ ] Implement all proposal lifecycle methods (`propose_`, `approve_`, `execute_`, `cancel_`)
- [ ] Add all new events
- [ ] Update error catalog (`ContractError::PartialExecutionFailure`, etc.)
- [ ] Write unit tests (batch validation, atomicity, approval counting)
- [ ] Write integration tests (interop with single-action, policy snapshot)
- [ ] Update event fixtures for test bindings generation
- [ ] Generate new TypeScript bindings (if using `artifacts/bindings`)
- [ ] Update security evidence documentation
- [ ] Run fuzz tests to find edge cases

---

## 8. Risk Analysis and Mitigations

### Risk 1: Execution Cost Overruns

**Risk**: 8 actions executed together might exceed per-invocation CPU/memory limits.

**Mitigation**:
- Benchmark each action's execution cost
- Set `MAX_BATCH_ACTIONS = 8` conservatively (start with 4, increase as proven)
- Include cost budget tests in CI

### Risk 2: Proposal Hash Collision

**Risk**: Two different batches produce same proposal ID.

**Mitigation**:
- Use SHA256 (collision-resistant) with domain separation
- Include nonce (unique per proposal) in hash preimage
- Include network ID (prevents cross-network collisions)

### Risk 3: Partial Failure Observability

**Risk**: Caller doesn't know which action failed without parsing error message.

**Mitigation**:
- Emit `MultiActionRolledBack` event with `failed_action_index`
- Execution record stored in contract state queryable after failure
- Client can retry with corrected batch or investigate logs

### Risk 4: Storage Bloat

**Risk**: Storing batch proposals + execution records indefinitely consumes storage.

**Mitigation**:
- Execution records have TTL (expire after `CRITICAL_ACTION_APPROVAL_EXPIRY_LEDGERS`)
- Queries return only recent records
- Bounded history size (future enhancement via history retention)

---

## 9. Example Use Cases

### Use Case 1: Schema Upgrade Chain

```rust
// Current (3 single-action proposals):
1. propose_critical_action(SchemaApproval(3))
2. wait for approval & execution
3. propose_critical_action(SchemaActivation(3, ledger+1000))
4. wait for approval & execution
5. propose_critical_action(SchemaReplacement(2, 3, BackwardCompatible))
6. wait for approval & execution

// With multi-action (1 proposal):
propose_multi_action(vec![
    SchemaApproval(3),
    SchemaActivation(3, ledger+1000),
    SchemaReplacement(2, 3, BackwardCompatible),
])
// Single approval round, atomic execution
```

### Use Case 2: Policy Update with Emergency Fallback

```rust
propose_multi_action(vec![
    ApprovalPolicyUpdate(new_policy),
    EmergencyPolicyUpdate(emergency_policy),
])
// Both policies updated atomically; if emergency policy is invalid, entire batch fails
// and original policy remains in effect
```

---

## 10. Summary and Next Steps

### Key Design Decisions

1. **Atomicity via Soroban Transaction Semantics**: No explicit rollback code needed; short-circuit on error
2. **Shared Approval Infrastructure**: Reuse existing policy snapshot and signer eligibility logic
3. **Deterministic Batch Hash**: XDR-encoded order + nonce ensures reproducibility
4. **Separate Storage Namespace**: New `MultiActionProposal` keys don't conflict with single-action
5. **Optional Adoption**: Backward compatible; clients can use multi-action when beneficial

### Immediate Implementation Steps

1. **Week 1**: Define types in shared, add storage keys
2. **Week 2**: Implement proposal creation, approval, execution flows
3. **Week 3**: Write comprehensive tests (unit, integration, property-based)
4. **Week 4**: Documentation, bindings generation, security review
5. **Week 5**: Audit and deployment preparation

### Dependencies

- Soroban SDK 27.0.0 (already in Cargo.lock)
- No new external dependencies required
- Builds on existing infrastructure (governance_action_commitment, approver snapshots, TTL management)

---

## Appendix: Encoding Examples

### Example 1: Proposal Hash Computation

```
Input Batch:
  actions = [SchemaApproval(1), SchemaActivation(1, 5000)]
  nonce = 42
  network_id = <Testnet hash>
  contract_id = CBVTJKNJURKX65RN37J5M2EH2JRUETJVDNPZGCJM5CFUQJ63KWHKGZ3

Preimage (XDR-encoded):
  ("gov_batch", 1, network_id, contract_id, "multi_action", 
   MultiActionBatch { 
     batch_version: 1, 
     actions: [SchemaApproval(1), SchemaActivation(1, 5000)]
   }, 
   42)

Hash:
  SHA256(preimage) = 0x7f3e2d1c4b5a6f8e...
```

### Example 2: Execution Record

```rust
MultiActionExecutionRecord {
    record_version: 1,
    proposal_id: 0x7f3e2d1c4b5a6f8e...,
    batch_commitment: 0x7f3e2d1c4b5a6f8e...,
    status: Succeeded,
    approval_count: 3,
    resulting_config_version: 42,
    execution_ledger: 1_000_000,
    execution_timestamp: 1_700_000_000,
    receipt_hash: 0xabcdef0123456789...,
}
```

---

**Document Version**: 1.0  
**Last Updated**: 2024-11-19  
**Status**: Design Ready for Implementation
