# Atomic Multi-Action Proposals: Implementation Checklist

## Phase 1: Shared Types and Infrastructure

### Shared Library (`packages/shared/src/lib.rs`)

- [ ] Add `MultiActionBatch` struct
  - Fields: `batch_version`, `actions: Vec<CriticalAction>`
  - Derive: Clone, Debug, Eq, PartialEq
  - Add `#[contracttype]` attribute

- [ ] Add `MultiActionExecutionStatus` enum
  - Variants: `Pending`, `Approved`, `Executing`, `Succeeded`, `PartialFailure(u32)`
  - Add `#[contracttype]` attribute

- [ ] Add `MultiActionExecutionRecord` struct
  - Fields: record_version, proposal_id, batch_commitment, status, approval_count, config_version, ledger, timestamp, receipt_hash
  - Add `#[contracttype]` attribute

- [ ] Add `MultiActionProposal` struct
  - Fields: batch, policy, proposer, approvals, created_at, expires_at
  - Add `#[contracttype]` attribute

- [ ] Add constants
  - `MULTI_ACTION_BATCH_VERSION = 1`
  - `MAX_BATCH_ACTIONS = 8`
  - `MULTI_ACTION_EXECUTION_RECORD_VERSION = 1`

- [ ] Add `CriticalActionCategory::MultiAction` enum variant

- [ ] Implement `is_valid_multi_action_batch(actions: &Vec<CriticalAction>) -> bool`
  - Check: non-empty, ≤ MAX_BATCH_ACTIONS
  - Check: no duplicate action categories
  - Check: no ContractReplacement variants
  - Return true only if all checks pass

- [ ] Implement `multi_action_batch_commitment()` function
  - Input: env, contract_id, actions, nonce
  - XDR encode batch with full action list
  - Include domain separation: "gov_batch", version, network_id, "multi_action"
  - Return SHA256 hash

- [ ] Add error codes (in `ContractError` enum)
  - `PartialExecutionFailure = 100` (or next available)
  - `InvalidMultiActionBatch = 101`

---

## Phase 2: Protocol Config Contract State

### Storage Keys (`contracts/protocol-config/src/state.rs`)

- [ ] Add to `DataKey` enum (in `lib.rs`):
  ```rust
  MultiActionProposal(BytesN<32>),
  MultiActionDomain(BytesN<32>),
  MultiActionSnapshot(BytesN<32>),
  MultiActionApproverSnapshot(BytesN<32>),
  MultiActionExecutionRecord(BytesN<32>),
  LatestMultiActionRecord,
  MultiActionNonce,
  ```

- [ ] Add constant storage maps (if using separate storage namespace):
  ```rust
  pub const MULTI_ACTION_PROPOSALS: Map<&str, MultiActionProposal> = 
    Map::new("multi_action_proposals");
  pub const MULTI_ACTION_EXECUTION_RECORDS: Map<u64, MultiActionExecutionRecord> = 
    Map::new("multi_action_execution_history");
  ```

---

## Phase 3: Events

### Event Definitions (`contracts/protocol-config/src/lib.rs`)

- [ ] Add `MultiActionProposed` event
  - Fields: proposal_id, proposer, action_count, expires_at

- [ ] Add `MultiActionApproved` event
  - Fields: proposal_id, signer, approval_count

- [ ] Add `MultiActionExecutionStarted` event
  - Fields: proposal_id, batch_size

- [ ] Add `MultiActionExecuted` event
  - Fields: proposal_id, batch_commitment, status

- [ ] Add `MultiActionRolledBack` event
  - Fields: proposal_id, failed_action_index, error_code

- [ ] Add `#[contractevent]` attribute to all events

---

## Phase 4: Core Logic Implementation

### Proposal Creation (`contracts/protocol-config/src/lib.rs`)

- [ ] Implement `propose_multi_action(env, actions) -> Result<BytesN<32>, ContractError>`
  - [ ] Verify not decommissioned
  - [ ] Verify admin role
  - [ ] Verify critical action policy enabled
  - [ ] Validate batch: `is_valid_multi_action_batch(&actions)`
  - [ ] Generate nonce
  - [ ] Compute proposal ID: `multi_action_batch_commitment()`
  - [ ] Capture approval policy snapshot
  - [ ] Create `MultiActionProposal` struct
  - [ ] Store proposal in persistent storage with TTL
  - [ ] Store domain metadata
  - [ ] Store approval snapshot
  - [ ] Store approver snapshot (signer versions)
  - [ ] Extend TTL for all related keys
  - [ ] Emit `MultiActionProposed` event
  - [ ] Return proposal ID

- [ ] Implement `get_multi_action_proposal(env, proposal_id) -> Option<MultiActionProposal>`
  - [ ] Retrieve from persistent storage
  - [ ] Extend TTL if found
  - [ ] Return proposal or None

### Approval Mechanism

- [ ] Implement `approve_multi_action(env, proposal_id, signer) -> Result<(), ContractError>`
  - [ ] Retrieve proposal or error
  - [ ] Verify domain separation
  - [ ] Verify approval snapshot
  - [ ] Verify proposal not expired
  - [ ] Require auth from signer
  - [ ] Check signer eligibility (using snapshot)
  - [ ] Verify signer hasn't already approved
  - [ ] Add signer to approvals vec
  - [ ] Store updated proposal
  - [ ] Emit `MultiActionApproved` event with new approval count
  - [ ] Return Ok

- [ ] Implement `cancel_multi_action(env, proposal_id) -> Result<(), ContractError>`
  - [ ] Verify admin auth
  - [ ] Retrieve proposal or error
  - [ ] Remove proposal and all related keys
  - [ ] Emit cancel event
  - [ ] Return Ok

### Execution Flow (Atomic)

- [ ] Implement `execute_multi_action(env, proposal_id) -> Result<(), ContractError>`
  - [ ] Verify not decommissioned
  - [ ] Retrieve proposal or error
  - [ ] Verify domain separation
  - [ ] Verify approval snapshot
  - [ ] Verify proposal not expired
  - [ ] Count eligible approvals
  - [ ] Verify approvals ≥ threshold
  - [ ] Emit `MultiActionExecutionStarted` event
  - [ ] **Atomicity block** (all within one transaction):
    - [ ] Save config version before execution
    - [ ] Loop over actions in order:
      - [ ] Call `execute_single_critical_action(action)`
      - [ ] If error: emit `MultiActionRolledBack`, set status to PartialFailure, break
    - [ ] If all succeeded: set status to Succeeded
  - [ ] Compute batch commitment
  - [ ] Compute receipt hash
  - [ ] Create `MultiActionExecutionRecord`
  - [ ] Store execution record
  - [ ] Store latest record in instance storage
  - [ ] Remove proposal and related keys
  - [ ] Emit `MultiActionExecuted` event with final status
  - [ ] Return Err if partial failure, Ok if succeeded

- [ ] Extract and refactor `execute_single_critical_action(env, action) -> Result<(), ContractError>`
  - [ ] Match on action type (schema approval, activation, replacement, etc.)
  - [ ] Apply state mutations (identical to existing single-action logic)
  - [ ] Record config change
  - [ ] Emit action-specific events
  - [ ] Return Ok or Err

### Query Functions

- [ ] Implement `get_multi_action_execution_record(env, proposal_id) -> Option<MultiActionExecutionRecord>`

- [ ] Implement `get_latest_multi_action_record(env) -> Option<MultiActionExecutionRecord>`

- [ ] Implement `derive_multi_action_proposal_id(env, actions, nonce) -> BytesN<32>`
  - Wrapper around `multi_action_batch_commitment()`

---

## Phase 5: Snapshot Validation (Reuse Existing)

- [ ] Implement `verify_multi_action_domain(env, proposal_id, proposal) -> Result<(), ContractError>`
  - [ ] Call existing domain verification logic (shared with single-action)
  - [ ] Check network ID matches
  - [ ] Check contract ID matches
  - [ ] Check category is MultiAction

- [ ] Implement `verify_approval_snapshot(env, proposal_id, proposal) -> Result<(), ContractError>`
  - [ ] Retrieve approval snapshot
  - [ ] Compare policy version with current
  - [ ] Compare signer digest with current
  - [ ] Return error if mismatch (indicates policy changed)

- [ ] Reuse `approval_eligibility_for()` from single-action logic
  - [ ] Already checks signer against proposal snapshot
  - [ ] Already checks signer version (assignment generation)

---

## Phase 6: Testing

### Unit Tests (`tests/` directory)

Create new test file: `tests/multi-action/` or add to existing test structure

- [ ] Test: Deterministic Hash
  - [ ] Same batch → same ID
  - [ ] Different order → different ID
  - [ ] Different action → different ID
  - [ ] Different nonce → different ID

- [ ] Test: Batch Validation
  - [ ] Empty batch rejected
  - [ ] Batch with 1 action accepted
  - [ ] Batch with MAX_BATCH_ACTIONS accepted
  - [ ] Batch with MAX_BATCH_ACTIONS+1 rejected
  - [ ] Batch with duplicate categories rejected
  - [ ] Batch with ContractReplacement rejected

- [ ] Test: Proposal Lifecycle
  - [ ] Proposal created successfully
  - [ ] Proposal ID deterministic
  - [ ] Proposal queryable after creation
  - [ ] Proposal expires after timeout
  - [ ] Proposal can be cancelled by admin

- [ ] Test: Approval Counting
  - [ ] First approval counted
  - [ ] Multiple approvals counted
  - [ ] Duplicate approval rejected
  - [ ] Non-eligible signer rejected
  - [ ] Threshold checked at execution

- [ ] Test: Atomicity
  - [ ] All actions succeed → batch succeeds
  - [ ] First action fails → batch fails, nothing committed
  - [ ] Middle action fails → earlier actions not committed, later not attempted
  - [ ] Last action fails → batch fails
  - [ ] Failed batch → config version unchanged

- [ ] Test: Execution Record
  - [ ] Record created on successful execution
  - [ ] Record contains correct status
  - [ ] Record contains action count
  - [ ] Record contains approval count
  - [ ] Record contains receipt hash
  - [ ] Record queryable by proposal ID

- [ ] Test: Policy Snapshot Immutability
  - [ ] Signer in snapshot can approve even if removed from current policy
  - [ ] Signer not in snapshot cannot approve
  - [ ] Signer added after proposal creation can approve new proposals but not old

- [ ] Test: Events Emitted (in order)
  - [ ] MultiActionProposed on creation
  - [ ] MultiActionApproved on each approval
  - [ ] MultiActionExecutionStarted on execution begin
  - [ ] Action-specific events during execution
  - [ ] MultiActionExecuted with final status
  - [ ] If failed: MultiActionRolledBack before MultiActionExecuted

### Integration Tests

- [ ] Test: Coexistence with Single-Action Proposals
  - [ ] Create single-action proposal
  - [ ] Create multi-action proposal
  - [ ] Approve both independently
  - [ ] Execute in any order
  - [ ] Both succeed

- [ ] Test: Multiple Multi-Action Proposals
  - [ ] Create 2+ multi-action proposals
  - [ ] Each has unique proposal ID
  - [ ] Each tracks approvals independently
  - [ ] Execute one doesn't affect other

- [ ] Test: Approval across Policy Changes
  - [ ] Create multi-action proposal with policy A
  - [ ] Change policy to B
  - [ ] Signer from old policy A can still approve old proposal
  - [ ] Signer only in new policy B cannot approve old proposal

### Property-Based Tests (using proptest)

- [ ] Batch hash is order-sensitive
  - [ ] Hypothesis: Reversing action order → different hash

- [ ] Execution failure prevents later actions
  - [ ] Hypothesis: If action N fails, action N+1 never executes

- [ ] Rollback is idempotent
  - [ ] Hypothesis: Retrying failed batch returns same error

- [ ] Proposal ID determinism over all parameters
  - [ ] Hypothesis: Hash only depends on actions, contract, network, nonce

---

## Phase 7: Documentation

- [ ] Add rustdoc comments to all public functions
  - [ ] `propose_multi_action`: Purpose, constraints, atomicity guarantee
  - [ ] `approve_multi_action`: Approval model, eligibility rules
  - [ ] `execute_multi_action`: Execution order, rollback behavior
  - [ ] `is_valid_multi_action_batch`: Validation rules

- [ ] Create `docs/governance-multi-action.md`
  - [ ] Design rationale
  - [ ] Use cases and examples
  - [ ] API reference
  - [ ] Error codes and meanings

- [ ] Create `docs/atomicity-guarantees.md`
  - [ ] Explain Soroban transaction semantics
  - [ ] Describe rollback mechanism
  - [ ] Define observable properties

- [ ] Update `BINDINGS_IMPLEMENTATION.md`
  - [ ] Add TypeScript types for new structs
  - [ ] Add client methods for multi-action endpoints

- [ ] Update main `README.md`
  - [ ] Link to multi-action documentation

---

## Phase 8: Client Bindings Generation

### TypeScript Artifacts (`artifacts/bindings/`)

- [ ] Run code generation to create:
  - [ ] Types for `MultiActionBatch`, `MultiActionProposal`, `MultiActionExecutionRecord`
  - [ ] Types for all new events
  - [ ] Client methods: `proposeMultiAction`, `approveMultiAction`, `executeMultiAction`, `cancelMultiAction`
  - [ ] Query methods: `getMultiActionProposal`, `getMultiActionExecutionRecord`, `getLatestMultiActionRecord`

- [ ] Update test fixtures
  - [ ] Add encoding vectors for multi-action batch hashes
  - [ ] Add test data for integration tests

- [ ] Update `__tests__/bindings.test.ts`
  - [ ] Test multi-action hash computation
  - [ ] Test multi-action proposal round-trip serialization

- [ ] Update `__tests__/client.integration.test.ts`
  - [ ] Test multi-action proposal creation
  - [ ] Test multi-action approval and execution
  - [ ] Test atomicity (success and failure scenarios)

---

## Phase 9: Security and Audit

- [ ] Security Review Checklist
  - [ ] No privilege escalation (approval policy unchanged)
  - [ ] No storage exhaustion (MAX_BATCH_ACTIONS enforced)
  - [ ] No reordering vulnerability (XDR order canonical)
  - [ ] No duplication vulnerability (unique proposal ID check)
  - [ ] Rollback is complete (no orphaned state)

- [ ] Fuzzing
  - [ ] Run `cargo +nightly fuzz` on multi-action inputs
  - [ ] Target: hash computation, batch validation, execution
  - [ ] Goal: No panics or unexpected errors

- [ ] Code Review
  - [ ] Peer review of atomicity implementation
  - [ ] Verify transaction semantics correctness
  - [ ] Check error handling coverage

---

## Phase 10: Deployment and Monitoring

- [ ] Build and Compile
  - [ ] `cargo build --release` succeeds
  - [ ] No warnings

- [ ] Run Full Test Suite
  - [ ] All unit tests pass
  - [ ] All integration tests pass
  - [ ] All property tests pass (with sufficient iterations)

- [ ] Documentation Deployment
  - [ ] Docs built and published
  - [ ] Bindings documentation updated
  - [ ] Examples visible in docs

- [ ] Deployment Preparation
  - [ ] Contract version bumped
  - [ ] Migration checkpoint created (if needed)
  - [ ] Changelog entry written
  - [ ] Release notes prepared

- [ ] Post-Deployment Monitoring
  - [ ] Event logs monitored for multi-action events
  - [ ] Execution records audited for failures
  - [ ] Performance metrics collected

---

## Acceptance Criteria Verification

### Criterion 1: Atomic Execution

- [ ] Test: Any failed action rolls back all prior actions
- [ ] Test: Execution record reflects success or partial failure
- [ ] Test: Config version unchanged on failure
- [ ] Test: Proposal remains queryable if execution failed

### Criterion 2: Deterministic Hashing

- [ ] Test: Same batch always produces same proposal ID
- [ ] Test: Different action order produces different ID
- [ ] Test: Hash includes all actions in encoded form
- [ ] Test: Hash is network-specific (different networks produce different IDs)

### Criterion 3: Bounded Execution

- [ ] Test: Batch size limited to MAX_BATCH_ACTIONS
- [ ] Test: Rejection of oversized batches
- [ ] Test: No unbounded loops or allocation

### Criterion 4: Backward Compatibility

- [ ] Test: Single-action proposals still work
- [ ] Test: Existing approval policy applies to multi-action
- [ ] Test: Existing query endpoints unchanged
- [ ] No breaking changes to public API

### Criterion 5: Observable Behavior

- [ ] Test: Events emitted in correct order
- [ ] Test: Execution record contains all relevant metadata
- [ ] Test: Failed batch action index recorded
- [ ] Test: Receipt hash verifiable off-chain

---

## Sign-Off

- [ ] Implementation complete and tested
- [ ] All acceptance criteria verified
- [ ] Security review approved
- [ ] Documentation complete
- [ ] Bindings generated and tested
- [ ] Ready for mainnet deployment

**Target Completion**: 5 weeks from start  
**Estimated Effort**: 120-150 person-hours  
**Risk Level**: Medium (new governance flow, but reuses existing infrastructure)
