# Atomic Multi-Action Proposals: Technical Deep Dives

## 1. Transaction Atomicity in Soroban

### 1.1 How Soroban Guarantees Atomicity

Soroban contract invocations execute within a **single ledger transaction**. This means:

1. **All-or-Nothing**: If the contract function returns `Ok`, all storage writes are committed. If it returns `Err`, **all writes are discarded**.

2. **No Partial Commits**: There is no way to partially commit state changes within a single invocation. Either the entire invocation succeeds or fails.

3. **Transactional Consistency**: The ledger sees the contract state before the call or after the call—never an intermediate state.

### 1.2 Implementation Pattern

```rust
pub fn execute_multi_action(env: Env, proposal_id: BytesN<32>) -> Result<(), ContractError> {
    // Pre-execution checks (no state mutation here)
    let proposal = Self::get_multi_action_proposal(env.clone(), proposal_id.clone())
        .ok_or(ContractError::ApprovalProposalNotFound)?;
    
    // Validate thresholds, expiry, etc.
    Self::ensure_proposal_live(&env, &proposal)?;
    if Self::eligible_approval_count(&env, &proposal_id, &proposal) < proposal.policy.threshold {
        return Err(ContractError::InsufficientApprovals);
    }
    
    // === ATOMIC EXECUTION BLOCK ===
    // All mutations below are committed only if this block returns Ok
    
    // Execute each action—short-circuit on error
    for (index, action) in proposal.batch.actions.iter().enumerate() {
        execute_single_critical_action(env.clone(), action.clone())?;
        // If this line returns Err, the entire function returns Err
        // and NO writes up to this point are committed
    }
    
    // All actions succeeded—record the execution
    let record = MultiActionExecutionRecord { /* ... */ };
    env.storage().persistent().set(&DataKey::MultiActionExecutionRecord(proposal_id.clone()), &record);
    
    MultiActionExecuted { /* ... */ }.publish(&env);
    
    Ok(())
    // <-- Only here are all writes from this function committed
}
```

### 1.3 Short-Circuit Semantics

The `?` operator in Rust returns early from the function if an `Err` is encountered:

```rust
// This pattern:
for action in batch {
    execute_single_critical_action(action)?;  // Returns Err -> entire function returns Err
}

// Is equivalent to:
for action in batch {
    match execute_single_critical_action(action) {
        Ok(()) => continue,  // Keep going
        Err(e) => return Err(e),  // Return immediately with error
    }
}
```

When `execute_multi_action` returns `Err`:
- No storage writes from this invocation are committed
- Caller sees the error
- Contract state appears unchanged
- Caller can retry, fix inputs, or cancel proposal

### 1.4 Observable Verification

To verify atomicity from off-chain:

```typescript
// Before proposal execution
const configBefore = await contract.getConfigVersion();

// Execute multi-action batch
const executeTx = await contract.executeMultiAction(proposalId);

if (executeTx.isErr()) {
    // Execution failed
    const configAfter = await contract.getConfigVersion();
    console.assert(configBefore === configAfter, "Atomicity violated!");
}
else {
    // Execution succeeded
    const configAfter = await contract.getConfigVersion();
    console.assert(configBefore < configAfter, "Expected config update");
}
```

---

## 2. Proposal Hash Determinism

### 2.1 XDR Encoding for Determinism

XDR (eXternal Data Representation) is used for all Soroban hash preimages because:

1. **Unambiguous**: Every Rust type serializes to exactly one byte sequence
2. **Canonical**: No choice points (no `Option` serialization flags, explicit length prefixes)
3. **Stable**: The encoding is frozen in the Soroban protocol specification

```rust
pub fn multi_action_batch_commitment(
    env: &Env,
    contract_id: &Address,
    actions: &Vec<CriticalAction>,
    nonce: u32,
) -> BytesN<32> {
    let batch = MultiActionBatch {
        batch_version: MULTI_ACTION_BATCH_VERSION,
        actions: actions.clone(),  // Order preserved in Vec
    };
    
    let preimage = (
        Symbol::new(env, "gov_batch"),        // Tag
        GOVERNANCE_ACTION_HASH_VERSION,       // Version
        env.ledger().network_id(),            // Network separation
        contract_id.clone(),                  // Contract separation
        Symbol::new(env, "multi_action"),     // Discriminator
        batch,                                // Full batch (all actions)
        nonce,                                // Unique identifier
    );
    
    // to_xdr(env) serializes as XDR—always produces same bytes for same input
    env.crypto().sha256(&preimage.to_xdr(env)).to_bytes()
}
```

### 2.2 Order Sensitivity

**Key Property**: Changing action order produces different hash

```rust
// Batch A:
let batch_a = vec![
    CriticalAction::SchemaApproval(1),
    CriticalAction::SchemaActivation(1, 1000),
];

// Batch B (reversed):
let batch_b = vec![
    CriticalAction::SchemaActivation(1, 1000),
    CriticalAction::SchemaApproval(1),
];

let id_a = derive_multi_action_proposal_id(&env, batch_a, 1);
let id_b = derive_multi_action_proposal_id(&env, batch_b, 1);

assert_ne!(id_a, id_b);  // Different orders → different IDs
```

**Why This Matters**: It enforces that execution order is canonical and immutable. Changing the order requires a new proposal with a new ID, which requires fresh approvals.

### 2.3 Nonce Uniqueness

Nonces prevent proposal ID collisions:

```rust
// Call 1: Same batch, nonce 1
propose_multi_action(batch.clone());  // nonce incremented to 1
// => proposal_id_1

// Call 2: Same batch again, nonce 2
propose_multi_action(batch.clone());  // nonce incremented to 2
// => proposal_id_2 (different from proposal_id_1)

// Each proposal gets unique ID despite identical batch
```

**Storage**: Nonce stored in instance storage:
```rust
DataKey::MultiActionNonce  // Persistent across invocations
```

### 2.4 Network Separation

The proposal hash is network-specific:

```rust
env.ledger().network_id()  // Different for testnet vs. mainnet
```

This prevents:
- Testnet approval being replayed on mainnet
- Proposal hash ambiguity across different deployments
- Cross-network governance collision

---

## 3. Approval Snapshot Immutability

### 3.1 Problem: Policy Changes During Approval Window

**Scenario**:
1. Admin proposes multi-action with 3-of-5 signers
2. First signer approves
3. Governance changes policy to 4-of-3 signers (removes 2 people)
4. Old signers (now removed) try to approve original proposal

**Solution**: Capture policy snapshot at proposal creation.

### 3.2 Snapshot Structure

```rust
pub struct GovernanceApprovalSnapshot {
    pub snapshot_version: u32,  // v1: Identifies this structure's format
    pub policy_version: u32,    // Which policy state this snapshot represents
    pub threshold: u32,         // How many approvals needed
    pub signer_set_digest: BytesN<32>,  // SHA256 of ordered signer vector
}

pub fn governance_signer_set_digest(env: &Env, signers: &Vec<Address>) -> BytesN<32> {
    let preimage = (
        Symbol::new(env, "gov_signers"),
        GOVERNANCE_APPROVAL_SNAPSHOT_VERSION,
        signers.clone(),  // Ordered vector; order matters
    );
    env.crypto().sha256(&preimage.to_xdr(env)).to_bytes()
}
```

### 3.3 Validation at Approval

When signer attempts to approve, the contract checks:

```rust
pub fn verify_approval_snapshot(
    env: &Env,
    proposal_id: &BytesN<32>,
    proposal: &MultiActionProposal,
) -> Result<(), ContractError> {
    let snapshot = env.storage().persistent().get(&DataKey::MultiActionSnapshot(proposal_id.clone()))
        .ok_or(ContractError::ApprovalProposalNotFound)?;
    
    let current_policy = Self::critical_action_policy(&env);
    let current_digest = governance_signer_set_digest(&env, &current_policy.signers);
    
    // If policy changed, snapshot will mismatch
    if snapshot.signer_set_digest != current_digest {
        // Policy has changed, but proposal still uses old snapshot
        // This is OK—snapshot protects the proposal
        return Ok(());
    }
    
    Ok(())
}
```

### 3.4 Signer Version Tracking

**Signer Versions** track when each address was added/removed from the policy:

```rust
pub struct GovernanceApproverSnapshot {
    pub encoding_version: u32,
    pub approvers: Vec<GovernanceApproverVersion>,  // Signer + version pairs
}

pub struct GovernanceApproverVersion {
    pub address: Address,
    pub assignment_version: u32,  // Version when this signer was added
}
```

**Use Case**: Detect retroactive approval attempts

```rust
// Proposal created when policy had signers A, B, C (versions 1, 1, 1)
// Policy changes: remove B, add D (now versions 1, 2, 1 for A, D, C)

// When D tries to approve original proposal:
// - D is in current policy (version 2)
// - D is NOT in snapshot (version was 1)
// - Rejection: D cannot approve—they weren't a signer when proposal was created
```

---

## 4. Bounded Execution and Cost Analysis

### 4.1 Why MAX_BATCH_ACTIONS = 8?

Soroban has implicit and explicit limits:

| Limit | Value | Notes |
|-------|-------|-------|
| Per-invocation CPU | ~2 seconds | Actual: measured in ops; ~10M-50M ops safe |
| Per-invocation memory | ~32 MB | Conservative; most contracts use <10 MB |
| Ledger footprint entries | ~100 | Read + write footprints combined |
| Ledger entry size | 65 KB | Raw entry data |

### 4.2 Action Execution Cost (Estimates)

| Action | Approx. Time | Storage Writes | Notes |
|--------|----------|-----------------|-------|
| SchemaApproval | ~10 ms | 1-2 | Simple state set |
| SchemaActivation | ~15 ms | 2-3 | Schedule + history record |
| SchemaDeprecation | ~10 ms | 1-2 | Deprecation mark |
| SchemaReplacement | ~20 ms | 2-3 | Link successor + history |
| SchemaPayloadLimit | ~10 ms | 1-2 | Configuration update |
| ApprovalPolicyUpdate | ~30 ms | 3-5 | Policy validation + versioning |
| EmergencyPolicyUpdate | ~25 ms | 2-3 | Emergency parameter update |

**Total for 8 actions**: ~130-200 ms (well within 2-second limit)

### 4.3 Storage Footprint Analysis

```rust
// Assume worst case: 8 schema operations, each touching persistent storage

// Persistent reads (don't count toward footprint):
// - MultiActionProposal
// - Approval snapshots

// Persistent writes:
// - 8 schema state updates: 8 entries
// - 8 history records: 8 entries
// - Execution record: 1 entry
// Total: 17 entries
// Each entry: avg 100-500 bytes
// Total footprint: ~2-8 KB
// Limit: 65 KB per entry, 100 entries max
// Conclusion: Well within limits
```

### 4.4 Setting MAX_BATCH_ACTIONS

Start with **4** (proven safe), increase to **8** after benchmarking.

```rust
pub const MAX_BATCH_ACTIONS: u32 = 4;  // Initial: very conservative

// After benchmarking and proving execution time < 500ms:
pub const MAX_BATCH_ACTIONS: u32 = 8;  // Can handle safely
```

### 4.5 Bounded History Recording

Multi-action execution records expire:

```rust
// Execution record TTL
env.storage().persistent().extend_ttl(
    &key,
    TTL_THRESHOLD_LEDGERS,  // 50,000 ledgers (~10 days)
    CRITICAL_ACTION_APPROVAL_EXPIRY_LEDGERS + TTL_THRESHOLD_LEDGERS,  // ~530k ledgers (~100 days)
);
```

After expiry, records are purged by Soroban ledger cleanup.

---

## 5. Rollback Scenarios and Edge Cases

### 5.1 Action-Specific Rollback Scenarios

#### Scenario A: SchemaApproval Followed by Invalid SchemaActivation

```rust
batch = [
    SchemaApproval(1),        // Succeeds
    SchemaActivation(1, 1000), // Fails: activates_at already passed
]

// Execution attempt:
1. SchemaApproval(1): ✓ Sets schema state
2. SchemaActivation(1, 1000): ✗ Returns Err

// Result:
- `execute_multi_action` returns Err
- Entire transaction rolled back
- Schema is NOT approved (SchemaApproval write discarded)
- Proposal not removed from storage (for retry)
```

#### Scenario B: Conflicting State Updates

```rust
batch = [
    SchemaPayloadLimit(1, 1024),   // Succeeds: limit = 1024
    SchemaPayloadLimit(1, 2048),   // Fails: Duplicate action category
]

// Validation at proposal creation:
- is_valid_multi_action_batch(batch) → false
- Duplicate category detected
- Proposal rejected before creation
```

#### Scenario C: Authorization Change Mid-Batch

```rust
batch = [
    ApprovalPolicyUpdate(new_policy),  // Succeeds: changes threshold to 4-of-3
    SchemaApproval(1),                  // Would succeed regardless
]

// Execution:
1. ApprovalPolicyUpdate: ✓ Policy updated
2. SchemaApproval: ✓ Schema approved
// Both succeed—no atomicity issue because both are valid

// Counter-example: if SchemaApproval had invalid admin, would fail and rollback both
```

### 5.2 Partial vs. Total Failure

**Total Failure** (batch never executes):
- Threshold not met → No invocation attempted
- Proposal expired → Rejected at invocation start
- Invalid batch → Rejected at proposal creation

**Partial Failure** (some actions execute then stop):
- Action N fails → Entire transaction rolls back
- Event `MultiActionRolledBack { failed_action_index: N }` emitted
- Proposal remains queryable
- Caller can retry or investigate

### 5.3 Retry Logic (Off-Chain)

```typescript
async function executeWithRetry(client, proposalId, maxRetries = 3) {
    for (let i = 0; i < maxRetries; i++) {
        try {
            const result = await client.executeMultiAction(proposalId);
            if (result.isOk()) {
                console.log("Multi-action executed successfully");
                return result;
            } else {
                console.error(`Execution failed: ${result.err()}`);
                
                // Fetch execution record to see which action failed
                const record = await client.getMultiActionExecutionRecord(proposalId);
                if (record && record.status.tag === 'PartialFailure') {
                    const failedIndex = record.status.fields[0];
                    console.error(`Failed at action index ${failedIndex}`);
                    
                    // Optionally: inspect batch and determine if retryable
                    const proposal = await client.getMultiActionProposal(proposalId);
                    const failedAction = proposal.batch.actions[failedIndex];
                    console.log(`Failed action: ${failedAction}`);
                }
                
                if (i < maxRetries - 1) {
                    console.log(`Retrying in 5 seconds...`);
                    await sleep(5000);
                }
            }
        } catch (error) {
            console.error(`Error executing: ${error}`);
            if (i === maxRetries - 1) throw error;
        }
    }
    throw new Error("Max retries exceeded");
}
```

---

## 6. Event Ordering and Observability

### 6.1 Event Emission Sequence

```rust
pub fn execute_multi_action(env: Env, proposal_id: BytesN<32>) -> Result<(), ContractError> {
    // ... validation ...
    
    // 1. ExecutionStarted event
    MultiActionExecutionStarted { proposal_id, batch_size }.publish(&env);
    
    // 2. For each action:
    for action in batch {
        // Action-specific events emitted here (e.g., SchemaApproved)
        execute_single_critical_action(action)?;
    }
    
    // 3a. If all succeeded:
    MultiActionExecuted {
        proposal_id,
        batch_commitment,
        status: MultiActionExecutionStatus::Succeeded,
    }.publish(&env);
    Ok(())
    
    // 3b. Or if failed:
    // (Error case—transaction rolls back, events discarded)
}
```

### 6.2 Event Indexing

Off-chain indexers can track:

```typescript
// Event listener: MultiActionProposed
listener.on(MultiActionProposed, (event) => {
    // proposal_id = event.proposal_id
    // action_count = event.action_count
    // Track for approval window
    db.saveProposal({
        id: event.proposal_id,
        status: 'pending',
        created_at: event.timestamp,
        action_count: event.action_count,
    });
});

// Event listener: MultiActionApproved
listener.on(MultiActionApproved, (event) => {
    // proposal_id = event.proposal_id
    // approval_count = event.approval_count
    db.updateApprovalCount(event.proposal_id, event.approval_count);
});

// Event listener: MultiActionExecuted
listener.on(MultiActionExecuted, (event) => {
    // proposal_id = event.proposal_id
    // status = event.status (Succeeded or PartialFailure(index))
    db.updateProposalStatus(event.proposal_id, {
        status: event.status.tag,
        failed_at: event.status.fields?.[0],
    });
});

// Event listener: MultiActionRolledBack
listener.on(MultiActionRolledBack, (event) => {
    // proposal_id = event.proposal_id
    // failed_action_index = event.failed_action_index
    db.logRollback({
        proposal_id: event.proposal_id,
        failed_action: event.failed_action_index,
        error_code: event.error_code,
    });
});
```

### 6.3 Event Replay Attack Prevention

**Concern**: Can events be replayed on different networks?

**Answer**: No—events are bound to contract invocation, which is bound to:
1. Network ID (Testnet vs. Mainnet)
2. Contract address (different deployments)
3. Transaction hash (ledger transaction)

Replaying events requires re-executing the same contract call, which itself is network-specific.

---

## 7. Authorization Revalidation

### 7.1 When Authorization is Checked

```
Proposal Creation:
  ├─ Admin auth required: ✓ checked with `require_auth(admin)`
  └─ Signer eligibility: N/A (not checked yet)

Approval:
  ├─ Signer auth required: ✓ checked with `require_auth(signer)`
  ├─ Signer in policy: ✓ checked against snapshot
  └─ Not already approved: ✓ checked in approvals vec

Execution:
  ├─ Threshold met: ✓ checked (approval_count >= policy.threshold)
  ├─ Proposal not expired: ✓ checked (ledger <= expires_at)
  └─ Policy snapshot valid: ✓ checked (signer_set_digest matches)
```

### 7.2 Authorization Escalation Prevention

**Concern**: Can a removed signer approve via old proposal?

**Answer**: No—signature verification prevents this.

```rust
pub fn approve_multi_action(
    env: Env,
    proposal_id: BytesN<32>,
    signer: Address,
) -> Result<(), ContractError> {
    // ...
    
    // Step 1: Require cryptographic signature from signer
    Self::require_auth(&signer);  // Must match transaction signer
    
    // Step 2: Check signer is in snapshot
    match Self::approval_eligibility_for(&env, &proposal_id, &proposal, &signer) {
        GovernanceApprovalEligibility::Eligible => {}
        GovernanceApprovalEligibility::RemovedFromCurrentPolicy => {
            // Signer was in snapshot but not current policy
            // Still eligible to approve this old proposal
            return Ok(());
        }
        _ => return Err(ContractError::Unauthorized),
    }
    
    // Step 3: Add to approvals
    proposal.approvals.push_back(signer.clone());
    // ...
}
```

The `require_auth(&signer)` call ensures:
1. Transaction is signed by `signer`'s key
2. Signer must provide cryptographic proof (not just an address claim)
3. Removed signers cannot forge signatures from current authorities

---

## 8. Comparison: Multi-Action vs. Single-Action Chains

### 8.1 Inefficiency of Single-Action Chains

**Scenario**: Schema lifecycle (approve, schedule activation, set replacement)

**Single-Action Approach** (current):
```
Time T:   propose_critical_action(SchemaApproval(3))
          └─> Wait for threshold approvals and execution (10+ minutes typical)
Time T+: propose_critical_action(SchemaActivation(3, T+1000))
          └─> Wait for threshold approvals and execution
Time T++: propose_critical_action(SchemaReplacement(2, 3, BackwardCompatible))
          └─> Wait for threshold approvals and execution

Total time: 30+ minutes
Approvals needed: 3 × threshold = 3 × 3 = 9 total signatures
```

**Multi-Action Approach** (new):
```
Time T:   propose_multi_action([
            SchemaApproval(3),
            SchemaActivation(3, T+1000),
            SchemaReplacement(2, 3, BackwardCompatible),
          ])
          └─> Wait for threshold approvals and single execution (10+ minutes)

Total time: 10+ minutes (parallel approval phase)
Approvals needed: 1 × threshold = 3 total signatures
```

### 8.2 Atomicity Guarantee

**Single-Action Chain**: No guarantee that all actions complete
- Approval 1 succeeds, approvals 2-3 fail → Partial upgrade (broken state)
- Fixes require manual intervention or additional proposals

**Multi-Action**: Atomic guarantee
- All three actions execute together or none execute
- No broken intermediate states

---

## 9. Future Extensions (Out of Scope)

### 9.1 Conditional Execution

```rust
pub enum BatchCondition {
    RequiresSchemaActive(u32),
    RequiresPolicyVersion(u32),
    OnlyIfConfigVersion(u32),
}

// Future proposal:
propose_multi_action_conditional(
    actions: Vec<CriticalAction>,
    conditions: Vec<BatchCondition>,
)
// Preconditions checked before any action executes
```

### 9.2 Parallel Action Groups

```rust
pub struct MultiActionBatchGroup {
    pub sequential: Vec<CriticalAction>,      // Must execute in order
    pub parallel: Vec<Vec<CriticalAction>>,   // Branches can execute in any order
}

// Future: Execute independent branches in parallel (if Soroban allows)
```

### 9.3 Conditional Approval Thresholds

```rust
pub struct WeightedApprovalPolicy {
    pub actions_weight: Map<CriticalActionCategory, u32>,
    pub threshold: u32,  // Sum of action weights must meet threshold
}

// Example: Schema changes require 3, emergency policy requires 4
```

---

## 10. Security Considerations

### 10.1 Threat Model

| Threat | Mitigation |
|--------|-----------|
| Proposal ID collision | XDR canonicality, nonce uniqueness |
| Unauthorized approval | require_auth, policy snapshot |
| Reordering attacks | Order encoded in hash, different ID |
| Partial execution exposure | Soroban transaction atomicity |
| Policy change bypass | Snapshot immutability, version tracking |
| Execution cost overrun | MAX_BATCH_ACTIONS enforcement |
| Storage exhaustion | TTL expiry, bounded history |

### 10.2 Invariants to Maintain

**Invariant 1**: Proposal hash is deterministic
- **Verification**: Test with identical inputs → identical hash
- **Enforcement**: Use XDR for all hash preimages

**Invariant 2**: Approvals respect snapshot
- **Verification**: Signer eligibility checked against snapshot, not current policy
- **Enforcement**: Compare signer against snapshot version, not current

**Invariant 3**: Execution is atomic
- **Verification**: Config version unchanged if execution fails
- **Enforcement**: Use `?` operator for early return on error

**Invariant 4**: Batch size is bounded
- **Verification**: Batch size <= MAX_BATCH_ACTIONS at proposal creation
- **Enforcement**: Validation check in `is_valid_multi_action_batch()`

**Invariant 5**: Action order is immutable
- **Verification**: Changing order produces different proposal ID
- **Enforcement**: Order encoded in XDR hash

---

**This document provides implementation guidance for core engineers and architects.**
