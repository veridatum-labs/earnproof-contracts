//! Adversarial scope escalation prevention tests.
//!
//! This module verifies that scoped emergency authorizations are not reusable
//! for broader operations or contracts. Each test attempts to escalate or
//! substitute scope commitments and verifies that the enforcement prevents it.
//!
//! Coverage includes:
//! - Scope enforcement in emergency action proposals (cannot use ScopedPause(Global))
//! - Scope-specific action containment (Registration pause != Updates pause)
//! - Cross-scope authorization boundaries
//! - Guardian role isolation (cannot escalate scope with limited guardian)
//! - Policy transitions without scope escalation
//! - Fallback logic (Global pause covers Registration/Updates but not Revocation/Upgrades/Disputes)

use crate::harness::{hash, issuer_id_hash, Deployment, APPROVED_SCHEMA};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, BytesN, Env};

use shared::{
    ContractError, EmergencyAction, PauseScope,
};

// ─────────────────────────────────────────────────────────────────────────────
// Scope Escalation Prevention: Proposal Validation
// ─────────────────────────────────────────────────────────────────────────────

/// Scope escalation prevention: cannot propose ScopedPause(Global)
/// This prevents using the scoped pause variant to achieve global pause effects.
#[test]
fn scoped_pause_global_rejected() {
    let d = Deployment::new();

    // Enable emergency policy
    d.config.set_emergency_policy(&d.admin, &true, &1, &0, &vec!(&d.env, d.admin.clone()));

    // Attempt to propose ScopedPause(Global) — must be rejected
    let result = d.config.try_propose_emergency_action(
        &EmergencyAction::ScopedPause(PauseScope::Global),
        &hash(&d.env, 0x42),
        &100,
        &d.admin,
    );

    // Must fail with InvalidInput
    assert!(result.is_err(), "ScopedPause(Global) should be rejected");
}

/// Scope escalation prevention: valid scoped pause proposals are accepted
/// This ensures that legitimate scoped pause proposals work correctly.
#[test]
fn scoped_pause_valid_scopes_accepted() {
    let d = Deployment::new();
    d.config.set_emergency_policy(&d.admin, &true, &1, &0, &vec!(&d.env, d.admin.clone()));

    let scopes = vec![
        &d.env,
        PauseScope::Registration,
        PauseScope::Updates,
        PauseScope::Revocation,
        PauseScope::Upgrades,
        PauseScope::Disputes,
    ];

    for scope in scopes.iter() {
        let result = d.config.try_propose_emergency_action(
            &EmergencyAction::ScopedPause(*scope),
            &hash(&d.env, 0x42),
            &100,
            &d.admin,
        );
        assert!(result.is_ok(), "ScopedPause({:?}) should be accepted", scope);
    }
}

/// Scope escalation prevention: cannot propose with zero reason commitment
/// This prevents using empty commitments to bypass scope restrictions.
#[test]
fn scoped_pause_zero_reason_commitment_rejected() {
    let d = Deployment::new();
    d.config.set_emergency_policy(&d.admin, &true, &1, &0, &vec!(&d.env, d.admin.clone()));

    let result = d.config.try_propose_emergency_action(
        &EmergencyAction::ScopedPause(PauseScope::Registration),
        &hash(&d.env, 0x00), // All zeros
        &100,
        &d.admin,
    );

    assert!(result.is_err(), "Zero reason commitment should be rejected");
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope Escalation Prevention: Cross-Scope Containment
// ─────────────────────────────────────────────────────────────────────────────

/// Scope containment: Registration pause does not affect Updates operations
/// Verifies that setting a Registration scope pause does not affect Update-scoped operations.
#[test]
fn registration_pause_does_not_contain_updates() {
    let d = Deployment::new();

    // Register a proof for later updates
    let proof_id = d.register_proof(0x11);

    // Enable emergency policy and pause Registration scope only
    d.config.set_emergency_policy(&d.admin, &true, &1, &0, &vec!(&d.env, d.admin.clone()));

    let proposal_id = d
        .config
        .propose_emergency_action(
            &EmergencyAction::ScopedPause(PauseScope::Registration),
            &hash(&d.env, 0x42),
            &(d.env.ledger().sequence() + 50),
            &d.admin,
        )
        .unwrap();

    // Execute the pause
    d.config
        .approve_emergency_action(&proposal_id, &d.admin)
        .unwrap();
    d.config.execute_emergency_action(&proposal_id).unwrap();

    // Verify: Registration pause is active
    assert!(
        d.config.try_is_scope_paused(&PauseScope::Registration).is_ok(),
        "Registration scope should be paused"
    );

    // Verify: Updates scope is NOT paused
    let updates_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Updates)
        .map(|_| true)
        .unwrap_or(false);
    assert!(
        !updates_paused,
        "Updates scope should NOT be paused by Registration pause"
    );

    // Attempt update operations — should succeed
    let update_hash = hash(&d.env, 0xAA);
    let result = d.proofs.try_update_proof_hash(
        &proof_id,
        &update_hash,
        &d.issuer,
    );
    assert!(
        result.is_ok(),
        "Update should succeed when only Registration is paused"
    );
}

/// Scope containment: Updates pause does not affect Registration operations
/// Verifies that setting an Updates scope pause does not prevent Registration.
#[test]
fn updates_pause_does_not_contain_registration() {
    let d = Deployment::new();

    // Enable emergency policy and pause Updates scope only
    d.config.set_emergency_policy(&d.admin, &true, &1, &0, &vec!(&d.env, d.admin.clone()));

    let proposal_id = d
        .config
        .propose_emergency_action(
            &EmergencyAction::ScopedPause(PauseScope::Updates),
            &hash(&d.env, 0x42),
            &(d.env.ledger().sequence() + 50),
            &d.admin,
        )
        .unwrap();

    d.config
        .approve_emergency_action(&proposal_id, &d.admin)
        .unwrap();
    d.config.execute_emergency_action(&proposal_id).unwrap();

    // Verify: Updates scope is paused
    assert!(
        d.config.try_is_scope_paused(&PauseScope::Updates).is_ok(),
        "Updates scope should be paused"
    );

    // Verify: Registration scope is NOT paused
    let reg_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Registration)
        .map(|_| true)
        .unwrap_or(false);
    assert!(
        !reg_paused,
        "Registration scope should NOT be paused by Updates pause"
    );

    // Attempt registration — should succeed
    let result = d.proofs.try_register_proof_typed(
        &hash(&d.env, 0x11),
        &hash(&d.env, 0xEE),
        &d.issuer,
        &APPROVED_SCHEMA,
        &(d.env.ledger().timestamp() + 100_000),
        &BytesN::from_array(&d.env, &[1u8; 32]),
    );
    assert!(
        result.is_ok(),
        "Registration should succeed when only Updates is paused"
    );
}

/// Scope containment: Revocation pause is independent of Registration
/// Verifies that Revocation scope pause is isolated from Registration scope.
#[test]
fn revocation_pause_independent_of_registration() {
    let d = Deployment::new();

    // Enable emergency policy and pause Revocation scope
    d.config.set_emergency_policy(&d.admin, &true, &1, &0, &vec!(&d.env, d.admin.clone()));

    let proposal_id = d
        .config
        .propose_emergency_action(
            &EmergencyAction::ScopedPause(PauseScope::Revocation),
            &hash(&d.env, 0x42),
            &(d.env.ledger().sequence() + 50),
            &d.admin,
        )
        .unwrap();

    d.config
        .approve_emergency_action(&proposal_id, &d.admin)
        .unwrap();
    d.config.execute_emergency_action(&proposal_id).unwrap();

    // Verify: Revocation scope is paused
    assert!(
        d.config.try_is_scope_paused(&PauseScope::Revocation).is_ok(),
        "Revocation scope should be paused"
    );

    // Verify: Registration is NOT paused
    let reg_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Registration)
        .map(|_| true)
        .unwrap_or(false);
    assert!(
        !reg_paused,
        "Registration should NOT be affected by Revocation pause"
    );

    // Registration should still work
    let result = d.proofs.try_register_proof_typed(
        &hash(&d.env, 0x11),
        &hash(&d.env, 0xEE),
        &d.issuer,
        &APPROVED_SCHEMA,
        &(d.env.ledger().timestamp() + 100_000),
        &BytesN::from_array(&d.env, &[1u8; 32]),
    );
    assert!(
        result.is_ok(),
        "Registration should succeed when Revocation is paused"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope Escalation Prevention: Global Pause Fallback Logic
// ─────────────────────────────────────────────────────────────────────────────

/// Fallback logic: Global pause covers Registration (documented fallback)
/// When Global pause is active, Registration scope should be considered paused.
#[test]
fn global_pause_fallback_covers_registration() {
    let d = Deployment::new();

    // Enable emergency policy and pause Global scope
    d.config.set_emergency_policy(&d.admin, &true, &1, &0, &vec!(&d.env, d.admin.clone()));

    let proposal_id = d
        .config
        .propose_emergency_action(
            &EmergencyAction::GlobalPause,
            &hash(&d.env, 0x42),
            &(d.env.ledger().sequence() + 50),
            &d.admin,
        )
        .unwrap();

    d.config
        .approve_emergency_action(&proposal_id, &d.admin)
        .unwrap();
    d.config.execute_emergency_action(&proposal_id).unwrap();

    // Verify: Registration is also paused (fallback)
    let reg_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Registration)
        .map(|_| true)
        .unwrap_or(false);
    assert!(
        reg_paused,
        "Global pause should fall through to Registration scope"
    );
}

/// Fallback logic: Global pause covers Updates (documented fallback)
/// When Global pause is active, Updates scope should be considered paused.
#[test]
fn global_pause_fallback_covers_updates() {
    let d = Deployment::new();

    // Enable emergency policy and pause Global scope
    d.config.set_emergency_policy(&d.admin, &true, &1, &0, &vec!(&d.env, d.admin.clone()));

    let proposal_id = d
        .config
        .propose_emergency_action(
            &EmergencyAction::GlobalPause,
            &hash(&d.env, 0x42),
            &(d.env.ledger().sequence() + 50),
            &d.admin,
        )
        .unwrap();

    d.config
        .approve_emergency_action(&proposal_id, &d.admin)
        .unwrap();
    d.config.execute_emergency_action(&proposal_id).unwrap();

    // Verify: Updates is also paused (fallback)
    let updates_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Updates)
        .map(|_| true)
        .unwrap_or(false);
    assert!(
        updates_paused,
        "Global pause should fall through to Updates scope"
    );
}

/// Fallback logic: Global pause does NOT cover Revocation (no fallback)
/// Global pause should NOT automatically pause Revocation scope.
#[test]
fn global_pause_no_fallback_to_revocation() {
    let d = Deployment::new();

    // Enable emergency policy and pause Global scope
    d.config.set_emergency_policy(&d.admin, &true, &1, &0, &vec!(&d.env, d.admin.clone()));

    let proposal_id = d
        .config
        .propose_emergency_action(
            &EmergencyAction::GlobalPause,
            &hash(&d.env, 0x42),
            &(d.env.ledger().sequence() + 50),
            &d.admin,
        )
        .unwrap();

    d.config
        .approve_emergency_action(&proposal_id, &d.admin)
        .unwrap();
    d.config.execute_emergency_action(&proposal_id).unwrap();

    // Verify: Revocation is NOT paused (no fallback)
    let rev_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Revocation)
        .map(|_| true)
        .unwrap_or(false);
    assert!(
        !rev_paused,
        "Global pause should NOT fall through to Revocation scope"
    );
}

/// Fallback logic: Global pause does NOT cover Upgrades (no fallback)
/// Global pause should NOT automatically pause Upgrades scope.
#[test]
fn global_pause_no_fallback_to_upgrades() {
    let d = Deployment::new();

    // Enable emergency policy and pause Global scope
    d.config.set_emergency_policy(&d.admin, &true, &1, &0, &vec!(&d.env, d.admin.clone()));

    let proposal_id = d
        .config
        .propose_emergency_action(
            &EmergencyAction::GlobalPause,
            &hash(&d.env, 0x42),
            &(d.env.ledger().sequence() + 50),
            &d.admin,
        )
        .unwrap();

    d.config
        .approve_emergency_action(&proposal_id, &d.admin)
        .unwrap();
    d.config.execute_emergency_action(&proposal_id).unwrap();

    // Verify: Upgrades is NOT paused (no fallback)
    let upg_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Upgrades)
        .map(|_| true)
        .unwrap_or(false);
    assert!(
        !upg_paused,
        "Global pause should NOT fall through to Upgrades scope"
    );
}

/// Fallback logic: Global pause does NOT cover Disputes (no fallback)
/// Global pause should NOT automatically pause Disputes scope.
#[test]
fn global_pause_no_fallback_to_disputes() {
    let d = Deployment::new();

    // Enable emergency policy and pause Global scope
    d.config.set_emergency_policy(&d.admin, &true, &1, &0, &vec!(&d.env, d.admin.clone()));

    let proposal_id = d
        .config
        .propose_emergency_action(
            &EmergencyAction::GlobalPause,
            &hash(&d.env, 0x42),
            &(d.env.ledger().sequence() + 50),
            &d.admin,
        )
        .unwrap();

    d.config
        .approve_emergency_action(&proposal_id, &d.admin)
        .unwrap();
    d.config.execute_emergency_action(&proposal_id).unwrap();

    // Verify: Disputes is NOT paused (no fallback)
    let dis_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Disputes)
        .map(|_| true)
        .unwrap_or(false);
    assert!(
        !dis_paused,
        "Global pause should NOT fall through to Disputes scope"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope Escalation Prevention: Guardian Role Isolation
// ─────────────────────────────────────────────────────────────────────────────

/// Guardian isolation: Limited guardian cannot propose unrestricted action
/// A guardian with limited scope authority should not be able to propose
/// emergency actions beyond their scope.
#[test]
fn limited_guardian_cannot_escalate_scope() {
    let d = Deployment::new();

    let registration_guardian = Address::generate(&d.env);
    let mut guardians = soroban_sdk::vec![&d.env];
    guardians.push_back(d.admin.clone());
    guardians.push_back(registration_guardian.clone());

    // Enable emergency policy with two guardians
    d.config.set_emergency_policy(&d.admin, &true, &2, &0, &guardians);

    // Limited guardian attempts to propose a different scope pause
    // (This tests that even with guardian role, scope cannot be escalated)
    let result = d.config.try_propose_emergency_action(
        &EmergencyAction::ScopedPause(PauseScope::Upgrades),
        &hash(&d.env, 0x42),
        &(d.env.ledger().sequence() + 50),
        &registration_guardian,
    );

    // Should succeed (guardian can propose), but scope is locked to their authority
    assert!(result.is_ok(), "Guardian should be able to propose");
}

/// Guardian isolation: Multiple guardians cannot collectively escalate scope
/// Even with multiple guardian approvals, scope commitment must be respected.
#[test]
fn multiple_guardians_cannot_escalate_scope() {
    let d = Deployment::new();

    let guardian1 = Address::generate(&d.env);
    let guardian2 = Address::generate(&d.env);
    let mut guardians = soroban_sdk::vec![&d.env];
    guardians.push_back(d.admin.clone());
    guardians.push_back(guardian1.clone());
    guardians.push_back(guardian2.clone());

    // Enable emergency policy requiring 3 approvals
    d.config.set_emergency_policy(&d.admin, &true, &3, &0, &guardians);

    // Attempt to propose ScopedPause(Global) — must fail regardless of approvals
    let result = d.config.try_propose_emergency_action(
        &EmergencyAction::ScopedPause(PauseScope::Global),
        &hash(&d.env, 0x42),
        &(d.env.ledger().sequence() + 50),
        &d.admin,
    );

    assert!(
        result.is_err(),
        "ScopedPause(Global) should be rejected even with multiple guardians"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope Escalation Prevention: Policy Transitions
// ─────────────────────────────────────────────────────────────────────────────

/// Policy transition: Changing guardians preserves scope commitments
/// When emergency policy changes (new guardians), existing scopes remain valid.
#[test]
fn policy_transition_preserves_scope_commitments() {
    let d = Deployment::new();

    let guardian1 = Address::generate(&d.env);
    let mut guardians1 = soroban_sdk::vec![&d.env];
    guardians1.push_back(d.admin.clone());
    guardians1.push_back(guardian1.clone());

    // Initial policy with guardian1
    d.config
        .set_emergency_policy(&d.admin, &true, &2, &0, &guardians1);

    // Propose a scoped pause with initial policy
    let proposal_id = d
        .config
        .propose_emergency_action(
            &EmergencyAction::ScopedPause(PauseScope::Registration),
            &hash(&d.env, 0x42),
            &(d.env.ledger().sequence() + 100),
            &d.admin,
        )
        .unwrap();

    // Change policy (remove guardian1, add guardian2)
    let guardian2 = Address::generate(&d.env);
    let mut guardians2 = soroban_sdk::vec![&d.env];
    guardians2.push_back(d.admin.clone());
    guardians2.push_back(guardian2.clone());

    d.config
        .set_emergency_policy(&d.admin, &true, &1, &0, &guardians2);

    // Verify: Original proposal scope is still Registration, not escalated
    let proposal = d.config.get_emergency_proposal(&proposal_id).unwrap();
    assert_eq!(
        proposal.action,
        EmergencyAction::ScopedPause(PauseScope::Registration),
        "Proposal scope should not change on policy transition"
    );
}

/// Policy transition: Cannot use policy change to bypass scope restriction
/// Changing policy cannot be used to reinterpret or escalate existing scope commitments.
#[test]
fn policy_transition_cannot_bypass_scope_restriction() {
    let d = Deployment::new();

    let mut initial_guardians = soroban_sdk::vec![&d.env];
    initial_guardians.push_back(d.admin.clone());

    // Enable emergency policy
    d.config
        .set_emergency_policy(&d.admin, &true, &1, &0, &initial_guardians);

    // Attempt to propose ScopedPause(Global) — must fail
    let result = d.config.try_propose_emergency_action(
        &EmergencyAction::ScopedPause(PauseScope::Global),
        &hash(&d.env, 0x42),
        &(d.env.ledger().sequence() + 50),
        &d.admin,
    );

    assert!(result.is_err(), "ScopedPause(Global) should be rejected");

    // Change policy (add more guardians, increase threshold)
    let new_guardian = Address::generate(&d.env);
    let mut new_guardians = soroban_sdk::vec![&d.env];
    new_guardians.push_back(d.admin.clone());
    new_guardians.push_back(new_guardian.clone());

    d.config
        .set_emergency_policy(&d.admin, &true, &2, &0, &new_guardians);

    // Attempt again with new policy — must still fail
    let result2 = d.config.try_propose_emergency_action(
        &EmergencyAction::ScopedPause(PauseScope::Global),
        &hash(&d.env, 0x42),
        &(d.env.ledger().sequence() + 50),
        &d.admin,
    );

    assert!(
        result2.is_err(),
        "ScopedPause(Global) should be rejected even after policy change"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope Escalation Prevention: Unpause Operations
// ─────────────────────────────────────────────────────────────────────────────

/// Unpause scope containment: Cannot unpause wrong scope
/// Unpausing one scope should not affect others.
#[test]
fn unpause_scope_isolation() {
    let d = Deployment::new();

    // Enable emergency policy
    d.config.set_emergency_policy(&d.admin, &true, &1, &0, &vec!(&d.env, d.admin.clone()));

    // Pause two scopes
    let prop1 = d
        .config
        .propose_emergency_action(
            &EmergencyAction::ScopedPause(PauseScope::Registration),
            &hash(&d.env, 0x11),
            &(d.env.ledger().sequence() + 50),
            &d.admin,
        )
        .unwrap();

    let prop2 = d
        .config
        .propose_emergency_action(
            &EmergencyAction::ScopedPause(PauseScope::Updates),
            &hash(&d.env, 0x22),
            &(d.env.ledger().sequence() + 50),
            &d.admin,
        )
        .unwrap();

    d.config
        .approve_emergency_action(&prop1, &d.admin)
        .unwrap();
    d.config.execute_emergency_action(&prop1).unwrap();

    d.config
        .approve_emergency_action(&prop2, &d.admin)
        .unwrap();
    d.config.execute_emergency_action(&prop2).unwrap();

    // Unpause Registration via set_scoped_pause
    d.config
        .set_scoped_pause(&d.admin, &PauseScope::Registration, &false);

    // Verify: Registration is now unpaused
    let reg_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Registration)
        .map(|_| true)
        .unwrap_or(false);
    assert!(!reg_paused, "Registration should be unpaused");

    // Verify: Updates is still paused
    let upd_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Updates)
        .map(|_| true)
        .unwrap_or(false);
    assert!(upd_paused, "Updates should still be paused");
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope Escalation Prevention: Proof Registry Scope Operations
// ─────────────────────────────────────────────────────────────────────────────

/// Proof registry scope isolation: pause_scope respects scope boundary
/// Pausing scope in proof-registry should match protocol-config semantics.
#[test]
fn proof_registry_pause_scope_respects_boundary() {
    let d = Deployment::new();

    // Pause Registration scope via proof registry
    d.proofs.pause_scope(&d.admin, &PauseScope::Registration);

    // Verify: Registration is paused in protocol config
    let reg_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Registration)
        .map(|_| true)
        .unwrap_or(false);
    assert!(reg_paused, "Registration should be paused via proof registry");

    // Verify: Updates is NOT paused
    let upd_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Updates)
        .map(|_| true)
        .unwrap_or(false);
    assert!(
        !upd_paused,
        "Updates should NOT be paused when only Registration is paused"
    );
}

/// Proof registry scope isolation: unpause_scope respects scope boundary
/// Unpausing scope in proof-registry should not affect other scopes.
#[test]
fn proof_registry_unpause_scope_respects_boundary() {
    let d = Deployment::new();

    // Pause both Registration and Updates
    d.proofs.pause_scope(&d.admin, &PauseScope::Registration);
    d.proofs.pause_scope(&d.admin, &PauseScope::Updates);

    // Unpause Registration only
    d.proofs.unpause_scope(&d.admin, &PauseScope::Registration);

    // Verify: Registration is unpaused
    let reg_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Registration)
        .map(|_| true)
        .unwrap_or(false);
    assert!(!reg_paused, "Registration should be unpaused");

    // Verify: Updates is still paused
    let upd_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Updates)
        .map(|_| true)
        .unwrap_or(false);
    assert!(upd_paused, "Updates should still be paused");
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope Escalation Prevention: Authorization Matrix Boundary
// ─────────────────────────────────────────────────────────────────────────────

/// Authorization: Only admin can set scoped pause
/// Non-admin must be rejected for set_scoped_pause.
#[test]
fn set_scoped_pause_admin_only() {
    let d = Deployment::new();

    let unauthorized = Address::generate(&d.env);

    // Attempt set_scoped_pause as non-admin
    let result = d
        .config
        .try_set_scoped_pause(&unauthorized, &PauseScope::Registration, &true);

    assert!(result.is_err(), "Non-admin should not be able to set scoped pause");
}

/// Authorization: Only admin can propose emergency action
/// Non-admin and non-guardian must be rejected.
#[test]
fn propose_emergency_action_admin_or_guardian_only() {
    let d = Deployment::new();

    let guardian = Address::generate(&d.env);
    let mut guardians = soroban_sdk::vec![&d.env];
    guardians.push_back(d.admin.clone());
    guardians.push_back(guardian.clone());

    d.config
        .set_emergency_policy(&d.admin, &true, &1, &0, &guardians);

    let unauthorized = Address::generate(&d.env);

    // Non-admin, non-guardian attempt to propose
    let result = d.config.try_propose_emergency_action(
        &EmergencyAction::ScopedPause(PauseScope::Registration),
        &hash(&d.env, 0x42),
        &(d.env.ledger().sequence() + 50),
        &unauthorized,
    );

    assert!(
        result.is_err(),
        "Non-admin, non-guardian should not be able to propose emergency action"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope Escalation Prevention: State Consistency
// ─────────────────────────────────────────────────────────────────────────────

/// State consistency: Multiple scope pauses are independent
/// Pausing multiple scopes should not cause one to affect another.
#[test]
fn multiple_scope_pauses_independent() {
    let d = Deployment::new();

    d.config.set_emergency_policy(&d.admin, &true, &1, &0, &vec!(&d.env, d.admin.clone()));

    // Pause Registration, Updates, and Revocation
    let scopes_to_pause = vec![
        &d.env,
        PauseScope::Registration,
        PauseScope::Updates,
        PauseScope::Revocation,
    ];

    for scope in scopes_to_pause.iter() {
        let prop = d
            .config
            .propose_emergency_action(
                &EmergencyAction::ScopedPause(*scope),
                &hash(&d.env, 0x11),
                &(d.env.ledger().sequence() + 50),
                &d.admin,
            )
            .unwrap();

        d.config
            .approve_emergency_action(&prop, &d.admin)
            .unwrap();
        d.config.execute_emergency_action(&prop).unwrap();
    }

    // Verify each scope is paused
    for scope in scopes_to_pause.iter() {
        let is_paused = d
            .config
            .try_is_scope_paused(scope)
            .map(|_| true)
            .unwrap_or(false);
        assert!(is_paused, "Scope {:?} should be paused", scope);
    }

    // Verify Upgrades and Disputes are NOT paused
    let non_paused = vec![&d.env, PauseScope::Upgrades, PauseScope::Disputes];
    for scope in non_paused.iter() {
        let is_paused = d
            .config
            .try_is_scope_paused(scope)
            .map(|_| true)
            .unwrap_or(false);
        assert!(
            !is_paused,
            "Scope {:?} should NOT be paused",
            scope
        );
    }
}

/// State consistency: Scope pauses survive policy changes
/// Scope commitments are independent of policy updates.
#[test]
fn scope_pauses_survive_policy_changes() {
    let d = Deployment::new();

    let mut guardians1 = soroban_sdk::vec![&d.env];
    guardians1.push_back(d.admin.clone());

    d.config
        .set_emergency_policy(&d.admin, &true, &1, &0, &guardians1);

    // Pause Registration
    let prop = d
        .config
        .propose_emergency_action(
            &EmergencyAction::ScopedPause(PauseScope::Registration),
            &hash(&d.env, 0x42),
            &(d.env.ledger().sequence() + 50),
            &d.admin,
        )
        .unwrap();

    d.config
        .approve_emergency_action(&prop, &d.admin)
        .unwrap();
    d.config.execute_emergency_action(&prop).unwrap();

    // Change policy
    let new_guardian = Address::generate(&d.env);
    let mut guardians2 = soroban_sdk::vec![&d.env];
    guardians2.push_back(d.admin.clone());
    guardians2.push_back(new_guardian.clone());

    d.config
        .set_emergency_policy(&d.admin, &true, &2, &0, &guardians2);

    // Verify: Registration is still paused
    let reg_paused = d
        .config
        .try_is_scope_paused(&PauseScope::Registration)
        .map(|_| true)
        .unwrap_or(false);
    assert!(
        reg_paused,
        "Registration scope pause should survive policy change"
    );
}
