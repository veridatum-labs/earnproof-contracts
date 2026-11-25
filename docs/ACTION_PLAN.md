# Action Plan: Atomic Multi-Action Governance Proposals

## Overview

You have been provided with **comprehensive design documentation** for implementing atomic multi-action governance proposals in earnproof-contracts. This document guides you through the next steps.

---

## What Has Been Delivered

**5 Analysis & Design Documents** (~107 KB total):

1. **README_ANALYSIS.md** - Start here. Executive summary and document hierarchy
2. **ATOMIC_MULTI_ACTION_PROPOSALS_DESIGN.md** - Complete design specification with architecture
3. **IMPLEMENTATION_CHECKLIST.md** - Task-by-task breakdown (120+ tasks, 10 phases)
4. **TECHNICAL_DEEP_DIVES.md** - Edge cases, security, and implementation guidance
5. **INDEX.md** - Quick reference and navigation

All files are in: `c:\Users\User\Desktop\musty1\`

---

## Before You Start Implementation

### Phase 0: Review & Planning (1-2 days)

1. **Read the key documents** (in order):
   - Start: `README_ANALYSIS.md` (15 min)
   - Primary design: `ATOMIC_MULTI_ACTION_PROPOSALS_DESIGN.md` (60-90 min)
   - Reference: `TECHNICAL_DEEP_DIVES.md` as needed
   - Use: `IMPLEMENTATION_CHECKLIST.md` to understand scope

2. **Understand the design goals**:
   - Multiple related governance actions group into atomic batches
   - All actions succeed together or all roll back (Soroban transaction atomicity)
   - Deterministic proposal IDs via XDR hashing
   - Reuse existing approval policy infrastructure
   - Bounded execution (max 8 actions per batch)

3. **Identify your team**:
   - Lead architect / governance SME
   - 2-3 backend engineers
   - 1 QA engineer
   - 1 documentation owner

4. **Establish prerequisites**:
   - [ ] Repository access to veridatum-labs/earnproof-contracts
   - [ ] Rust toolchain installed (see rust-toolchain.toml)
   - [ ] Soroban SDK knowledge (recommend: soroban-sdk docs)
   - [ ] Understanding of existing CriticalActionProposal flow

---

## Implementation Timeline

**Total Estimated Time**: 5-6 weeks (full-time team of 2-3)

| Phase | Week | Tasks | Owner |
|-------|------|-------|-------|
| Phase 1: Shared Types | Week 1 | 10 | Backend #1 |
| Phase 2: Storage Keys | Week 1 | 5 | Backend #1 |
| Phase 3: Events | Week 1 | 5 | Backend #2 |
| Phase 4: Core Logic | Week 2-3 | 25 | Backend #1, #2 |
| Phase 5: Snapshots | Week 3 | 3 | Backend #2 |
| Phase 6: Testing | Week 3-4 | 30+ | QA + Backend |
| Phase 7: Documentation | Week 4 | 5 | Docs Owner |
| Phase 8: Bindings | Week 4 | 5 | Backend #1 |
| Phase 9: Security Review | Week 5 | 10 | Governance SME |
| Phase 10: Deployment | Week 5-6 | 5 | DevOps + Backend |

---

## Step 1: Create Your Feature Branch

```bash
cd c:\Users\User\Desktop\musty1\earnproof-contracts
git checkout -b feat/add-atomic-multi-action-governance-proposals
```

---

## Step 2: Start with Phase 1 (Shared Types)

Reference: **IMPLEMENTATION_CHECKLIST.md** → Phase 1 section

### Key Tasks:
1. Add `MultiActionBatch` struct to `packages/shared/src/lib.rs`
2. Add `MultiActionProposal` struct
3. Add `MultiActionExecutionRecord` and `MultiActionExecutionStatus`
4. Implement helper functions:
   - `is_valid_multi_action_batch()`
   - `multi_action_batch_commitment()` (XDR hashing)
5. Add new error codes
6. Add constants (MAX_BATCH_ACTIONS = 8, etc.)

### Expected File Changes:
- `packages/shared/src/lib.rs` (~200-250 lines added)

### Testing:
- Unit tests for batch validation
- Property tests for deterministic hashing (same input → same hash)

---

## Step 3: Continue Through Phases 2-5

**Phase 2**: Storage management (DataKey variants)  
**Phase 3**: Event definitions (MultiActionProposed, Approved, Executed, etc.)  
**Phase 4**: Core contract logic (proposal creation, approval, execution)  
**Phase 5**: Snapshot immutability (policy snapshot handling)

See **IMPLEMENTATION_CHECKLIST.md** for detailed task lists per phase.

---

## Step 4: Comprehensive Testing (Phase 6)

Testing categories (from issue requirements):

1. **Positive Tests**
   - Multi-action proposal creation with valid batches
   - Approval flow (single & multiple signers)
   - Execution with all actions succeeding
   - Event emission in correct order

2. **Negative Tests**
   - Invalid batches (empty, duplicates, too large)
   - Unauthorized proposer/signer
   - Expired proposals
   - Attempted re-approval

3. **Boundary Tests**
   - MAX_BATCH_ACTIONS (8) edge cases
   - Very large action payloads
   - Approval threshold edge cases (1 of N, N of N)

4. **Atomicity & Rollback Tests**
   - Forced failure in action N: verify rollback of actions 1..N-1
   - Configuration unchanged on partial failure
   - Event state consistent on rollback

5. **Authorization & Privacy Tests**
   - Policy snapshot immutability
   - Signer version tracking
   - Reauthorization on policy change
   - Privacy of approvals before execution

6. **Regression Tests**
   - Existing single-action proposals still work
   - Existing approval policies unchanged
   - All existing test suites pass

### Test Frameworks:
- Unit: `#[test]` macros in contract modules
- Integration: `tests/property/` using proptest
- Fuzz: `fuzz/` workspace for harness

### Required Commands (from issue):
```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all --test '*'
# Property tests
cargo test --manifest-path=tests/property/Cargo.toml --all-features
# Full workspace
cargo test --workspace
```

---

## Step 5: Documentation & Bindings

**Phase 7**: Documentation
- Update README.md with multi-action proposal examples
- Add rustdoc comments to new types and functions
- Create docs/governance-multi-action.md with user guide

**Phase 8**: TypeScript Bindings
- Run code generation for new types
- Create client helper for multi-action proposals
- Update TypeScript test fixtures

---

## Step 6: Security Review & Deployment

**Phase 9**: Security Review
- Internal code review by governance SME
- Audit checklist (see TECHNICAL_DEEP_DIVES.md Security section)
- Fuzz testing against harnesses
- Property-based test validation

**Phase 10**: Deployment
- Final test run on testnet
- Release PR to main branch
- Tag version release

---

## PR Description Template

When you're ready to create a PR, use this template:

```markdown
## Atomic Multi-Action Governance Proposals

Closes #348

### Summary

Implements atomic multi-action governance proposals, allowing multiple related 
governance actions to be grouped into a single batch, approved once, and executed 
atomically. Leverages Soroban transaction semantics to ensure all-or-nothing 
execution.

### Changes

- **Shared Types**: MultiActionBatch, MultiActionProposal, MultiActionExecutionRecord
- **Storage**: New DataKey variants for multi-action state
- **Events**: MultiActionProposed, MultiActionApproved, MultiActionExecuted, etc.
- **Contract Logic**: Full proposal lifecycle (creation → approval → execution)
- **Tests**: 30+ unit, integration, and property-based tests

### Implementation Details

- Deterministic batch hashing via XDR encoding with domain separation
- Bounded execution (max 8 actions per batch)
- Reuses existing approval policy infrastructure
- All-or-nothing execution via Soroban transaction atomicity
- Backward compatible with single-action proposals

### Test Results

```
cargo fmt --all --check                                       ✓ PASS
cargo clippy --all-targets --all-features -- -D warnings      ✓ PASS
cargo test --workspace                                         ✓ PASS (XXX tests)
cargo test --manifest-path=tests/property/Cargo.toml          ✓ PASS (XX properties)
```

### Acceptance Criteria Met

- [x] Failed action rolls back all actions (via transaction atomicity)
- [x] Implementation is deterministic (XDR hashing)
- [x] Implementation is bounded (MAX_BATCH_ACTIONS = 8)
- [x] Implementation is observable (comprehensive events)
- [x] Existing authorization and privacy guarantees preserved
- [x] Focused positive, negative, boundary, authorization, regression tests
- [x] All required validation commands pass
```

---

## Key Resources

### Documentation Files
- `ATOMIC_MULTI_ACTION_PROPOSALS_DESIGN.md` - Architecture & design decisions
- `TECHNICAL_DEEP_DIVES.md` - Edge cases, security, implementation details
- `IMPLEMENTATION_CHECKLIST.md` - Concrete task list

### Repository Files
- `contracts/protocol-config/src/contract.rs` - Contract implementation
- `packages/shared/src/lib.rs` - Shared types and validation
- `tests/property/` - Property-based testing examples
- `rust-toolchain.toml` - Rust version specification

### External References
- Soroban SDK: https://docs.rs/soroban-sdk/
- XDR encoding: https://github.com/stellar/rs-soroban-env
- Property testing: https://proptest.rs/

---

## Checkpoints

Use these checkpoints to track progress:

- [ ] **End of Week 1**: Phases 1-3 complete (shared types, storage, events)
- [ ] **End of Week 2**: Phase 4 core logic 50% complete
- [ ] **End of Week 3**: Phase 4 complete, Phase 5-6 started
- [ ] **End of Week 4**: Phases 6-8 complete (testing, docs, bindings)
- [ ] **End of Week 5**: Phase 9 security review complete
- [ ] **End of Week 6**: Phase 10 deployment, PR merged

---

## Common Questions

**Q: What if an action fails during execution?**  
A: All writes are rolled back automatically by Soroban's transaction atomicity. The config version remains unchanged, and a `MultiActionRolledBack` event is emitted.

**Q: How is the proposal ID computed?**  
A: XDR encoding of the entire batch with domain separation. Changing action order produces a different hash, so order matters.

**Q: Can I mix single-action and multi-action proposals?**  
A: Yes. They use separate storage keys and nonces. Existing single-action proposals are unaffected.

**Q: What's the maximum batch size?**  
A: 8 actions (MAX_BATCH_ACTIONS). This is bounded based on execution cost analysis (~200ms per batch).

**Q: Do approval policies change during the proposal lifecycle?**  
A: Policy snapshots are immutable. A signer in the original snapshot can always approve, even if policies change later.

---

## Next Steps

1. **Today**: Read README_ANALYSIS.md and ATOMIC_MULTI_ACTION_PROPOSALS_DESIGN.md
2. **Tomorrow**: Review IMPLEMENTATION_CHECKLIST.md with your team
3. **This Week**: Create feature branch and start Phase 1
4. **Weekly**: Track progress against timeline checkpoints

---

## Questions or Issues?

Refer to:
- **Design questions** → ATOMIC_MULTI_ACTION_PROPOSALS_DESIGN.md
- **Implementation details** → TECHNICAL_DEEP_DIVES.md
- **Task questions** → IMPLEMENTATION_CHECKLIST.md
- **Acceptance criteria** → README_ANALYSIS.md (Acceptance Criteria Verification section)

Good luck! 🚀
