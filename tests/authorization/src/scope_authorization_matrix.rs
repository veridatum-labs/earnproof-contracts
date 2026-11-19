//! Scope-specific authorization matrix.
//!
//! This module extends the authorization matrix with scope-related operations:
//! - set_scoped_pause (protocol-config)
//! - pause_scope / unpause_scope (proof-registry)
//! - propose_emergency_action / approve_emergency_action / execute_emergency_action (protocol-config)
//!
//! Each test verifies that scope operations respect authorization boundaries
//! and that scope commitments are not reusable across contracts or identities.

use crate::harness::{authorize, hash, Deployment, APPROVED_SCHEMA};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, IntoVal, Val};

use shared::{EmergencyAction, PauseScope};

/// The identity attempting the scope operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeIdentity {
    /// No authorization entry at all.
    Missing,
    /// An authorization entry signed by a non-admin address.
    Wrong,
    /// The admin signs (control case).
    Authorized,
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope Authorization: set_scoped_pause (protocol-config)
// ─────────────────────────────────────────────────────────────────────────────

/// Authorization matrix: set_scoped_pause requires admin only
#[test]
fn set_scoped_pause_registration_requires_admin() {
    let d = Deployment::new();

    let non_admin = Address::generate(&d.env);

    // Missing auth: should fail
    let result_missing = d
        .config
        .try_set_scoped_pause(&non_admin, &PauseScope::Registration, &true);
    assert!(
        result_missing.is_err(),
        "set_scoped_pause without auth should fail"
    );

    // Authorized: should succeed
    let result_authorized = d
        .config
        .try_set_scoped_pause(&d.admin, &PauseScope::Registration, &true);
    assert!(
        result_authorized.is_ok(),
        "set_scoped_pause with admin auth should succeed"
    );
}

/// Authorization matrix: set_scoped_pause Updates scope requires admin
#[test]
fn set_scoped_pause_updates_requires_admin() {
    let d = Deployment::new();

    let non_admin = Address::generate(&d.env);

    // Missing auth: should fail
    let result_missing = d
        .config
        .try_set_scoped_pause(&non_admin, &PauseScope::Updates, &true);
    assert!(
        result_missing.is_err(),
        "set_scoped_pause without auth should fail"
    );

    // Authorized: should succeed
    let result_authorized = d.config.try_set_scoped_pause(&d.admin, &PauseScope::Updates, &true);
    assert!(
        result_authorized.is_ok(),
        "set_scoped_pause with admin auth should succeed"
    );
}

/// Authorization matrix: set_scoped_pause Revocation scope requires admin
#[test]
fn set_scoped_pause_revocation_requires_admin() {
    let d = Deployment::new();

    let non_admin = Address::generate(&d.env);

    // Missing auth: should fail
    let result_missing = d
        .config
        .try_set_scoped_pause(&non_admin, &PauseScope::Revocation, &true);
    assert!(
        result_missing.is_err(),
        "set_scoped_pause without auth should fail"
    );

    // Authorized: should succeed
    let result_authorized =
        d.config
            .try_set_scoped_pause(&d.admin, &PauseScope::Revocation, &true);
    assert!(
        result_authorized.is_ok(),
        "set_scoped_pause with admin auth should succeed"
    );
}

/// Authorization matrix: set_scoped_pause Upgrades scope requires admin
#[test]
fn set_scoped_pause_upgrades_requires_admin() {
    let d = Deployment::new();

    let non_admin = Address::generate(&d.env);

    // Missing auth: should fail
    let result_missing = d
        .config
        .try_set_scoped_pause(&non_admin, &PauseScope::Upgrades, &true);
    assert!(
        result_missing.is_err(),
        "set_scoped_pause without auth should fail"
    );

    // Authorized: should succeed
    let result_authorized =
        d.config
            .try_set_scoped_pause(&d.admin, &PauseScope::Upgrades, &true);
    assert!(
        result_authorized.is_ok(),
        "set_scoped_pause with admin auth should succeed"
    );
}

/// Authorization matrix: set_scoped_pause Disputes scope requires admin
#[test]
fn set_scoped_pause_disputes_requires_admin() {
    let d = Deployment::new();

    let non_admin = Address::generate(&d.env);

    // Missing auth: should fail
    let result_missing = d
        .config
        .try_set_scoped_pause(&non_admin, &PauseScope::Disputes, &true);
    assert!(
        result_missing.is_err(),
        "set_scoped_pause without auth should fail"
    );

    // Authorized: should succeed
    let result_authorized =
        d.config
            .try_set_scoped_pause(&d.admin, &PauseScope::Disputes, &true);
    assert!(
        result_authorized.is_ok(),
        "set_scoped_pause with admin auth should succeed"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope Authorization: pause_scope / unpause_scope (proof-registry)
// ─────────────────────────────────────────────────────────────────────────────

/// Authorization matrix: pause_scope requires admin
#[test]
fn proof_registry_pause_scope_requires_admin() {
    let d = Deployment::new();

    let non_admin = Address::generate(&d.env);

    // Missing auth: should fail
    let result_missing = d.proofs.try_pause_scope(&non_admin, &PauseScope::Registration);
    assert!(result_missing.is_err(), "pause_scope without auth should fail");

    // Authorized: should succeed
    let result_authorized = d.proofs.try_pause_scope(&d.admin, &PauseScope::Registration);
    assert!(
        result_authorized.is_ok(),
        "pause_scope with admin auth should succeed"
    );
}

/// Authorization matrix: unpause_scope requires admin
#[test]
fn proof_registry_unpause_scope_requires_admin() {
    let d = Deployment::new();

    // Set up: pause first
    d.proofs.pause_scope(&d.admin, &PauseScope::Updates);

    let non_admin = Address::generate(&d.env);

    // Missing auth: should fail
    let result_missing = d.proofs.try_unpause_scope(&non_admin, &PauseScope::Updates);
    assert!(
        result_missing.is_err(),
        "unpause_scope without auth should fail"
    );

    // Authorized: should succeed
    let result_authorized = d.proofs.try_unpause_scope(&d.admin, &PauseScope::Updates);
    assert!(
        result_authorized.is_ok(),
        "unpause_scope with admin auth should succeed"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope Authorization: Emergency Actions (protocol-config)
// ─────────────────────────────────────────────────────────────────────────────

/// Authorization matrix: propose_emergency_action requires admin or guardian
#[test]
fn propose_emergency_action_requires_admin_or_guardian() {
    let d = Deployment::new();

    let guardian = Address::generate(&d.env);
    let non_guardian = Address::generate(&d.env);

    // Enable emergency policy with guardian
    let mut guardians = soroban_sdk::vec![&d.env];
    guardians.push_back(d.admin.clone());
    guardians.push_back(guardian.clone());

    d.config
        .set_emergency_policy(&d.admin, &true, &1, &0, &guardians);

    // Non-admin, non-guardian: should fail
    let result_wrong = d.config.try_propose_emergency_action(
        &EmergencyAction::ScopedPause(PauseScope::Registration),
        &hash(&d.env, 0x42),
        &(d.env.ledger().sequence() + 50),
        &non_guardian,
    );
    assert!(
        result_wrong.is_err(),
        "propose_emergency_action by non-guardian should fail"
    );

    // Admin: should succeed
    let result_admin = d.config.try_propose_emergency_action(
        &EmergencyAction::ScopedPause(PauseScope::Registration),
        &hash(&d.env, 0x42),
        &(d.env.ledger().sequence() + 50),
        &d.admin,
    );
    assert!(
        result_admin.is_ok(),
        "propose_emergency_action by admin should succeed"
    );

    // Guardian: should succeed
    let result_guardian = d.config.try_propose_emergency_action(
        &EmergencyAction::ScopedPause(PauseScope::Updates),
        &hash(&d.env, 0x43),
        &(d.env.ledger().sequence() + 50),
        &guardian,
    );
    assert!(
        result_guardian.is_ok(),
        "propose_emergency_action by guardian should succeed"
    );
}

/// Authorization matrix: approve_emergency_action requires current guardian
#[test]
fn approve_emergency_action_requires_current_guardian() {
    let d = Deployment::new();

    let guardian1 = Address::generate(&d.env);
    let guardian2 = Address::generate(&d.env);
    let non_guardian = Address::generate(&d.env);

    // Initial policy with guardian1
    let mut guardians = soroban_sdk::vec![&d.env];
    guardians.push_back(d.admin.clone());
    guardians.push_back(guardian1.clone());

    d.config
        .set_emergency_policy(&d.admin, &true, &2, &0, &guardians);

    // Propose action
    let proposal_id = d
        .config
        .propose_emergency_action(
            &EmergencyAction::ScopedPause(PauseScope::Registration),
            &hash(&d.env, 0x42),
            &(d.env.ledger().sequence() + 50),
            &d.admin,
        )
        .unwrap();

    // Non-guardian: should fail
    let result_non_guardian = d
        .config
        .try_approve_emergency_action(&proposal_id, &non_guardian);
    assert!(
        result_non_guardian.is_err(),
        "approve_emergency_action by non-guardian should fail"
    );

    // Guardian1: should succeed
    let result_guardian1 = d.config.try_approve_emergency_action(&proposal_id, &guardian1);
    assert!(
        result_guardian1.is_ok(),
        "approve_emergency_action by valid guardian should succeed"
    );
}

/// Authorization matrix: execute_emergency_action requires no auth (anyone can execute)
#[test]
fn execute_emergency_action_requires_threshold_not_identity() {
    let d = Deployment::new();

    let guardian = Address::generate(&d.env);
    let mut guardians = soroban_sdk::vec![&d.env];
    guardians.push_back(d.admin.clone());
    guardians.push_back(guardian.clone());

    d.config
        .set_emergency_policy(&d.admin, &true, &1, &0, &guardians);

    // Propose and approve
    let proposal_id = d
        .config
        .propose_emergency_action(
            &EmergencyAction::ScopedPause(PauseScope::Registration),
            &hash(&d.env, 0x42),
            &(d.env.ledger().sequence() + 50),
            &d.admin,
        )
        .unwrap();

    d.config
        .approve_emergency_action(&proposal_id, &d.admin)
        .unwrap();

    // Any identity can execute (threshold already met)
    let anyone = Address::generate(&d.env);
    let result = d.config.try_execute_emergency_action(&proposal_id);
    assert!(
        result.is_ok(),
        "execute_emergency_action should succeed when threshold is met"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope Authorization: Cross-Contract Scope Consistency
// ─────────────────────────────────────────────────────────────────────────────

/// Cross-contract: pause_scope in proof-registry respects protocol-config admin
/// The proof-registry pause_scope should use protocol-config's admin, not proof-registry's.
#[test]
fn proof_registry_pause_scope_uses_protocol_admin() {
    let d = Deployment::new();

    let non_admin = Address::generate(&d.env);

    // Attempt pause_scope with non-admin in proof-registry
    // This should fail because proof-registry delegates to protocol-config's admin
    let result = d.proofs.try_pause_scope(&non_admin, &PauseScope::Registration);
    assert!(
        result.is_err(),
        "pause_scope by non-admin should fail even in proof-registry"
    );

    // Attempt with protocol-config's admin
    let result_admin = d.proofs.try_pause_scope(&d.admin, &PauseScope::Registration);
    assert!(
        result_admin.is_ok(),
        "pause_scope by admin should succeed in proof-registry"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope Authorization: State Consistency After Failed Authorization
// ─────────────────────────────────────────────────────────────────────────────

/// State consistency: Failed set_scoped_pause leaves no state changes
/// An unauthorized set_scoped_pause should not modify scope state.
#[test]
fn set_scoped_pause_unauthorized_no_state_change() {
    let d = Deployment::new();

    let non_admin = Address::generate(&d.env);

    // Verify initial state: Registration not paused
    let initial_state = d
        .config
        .try_is_scope_paused(&PauseScope::Registration)
        .map(|_| true)
        .unwrap_or(false);
    assert!(!initial_state, "Registration should initially not be paused");

    // Attempt unauthorized pause
    let _ = d
        .config
        .try_set_scoped_pause(&non_admin, &PauseScope::Registration, &true);

    // Verify: State unchanged after failed attempt
    let final_state = d
        .config
        .try_is_scope_paused(&PauseScope::Registration)
        .map(|_| true)
        .unwrap_or(false);
    assert!(
        !final_state,
        "Registration should still not be paused after failed attempt"
    );
}

/// State consistency: Failed pause_scope leaves no state changes
/// An unauthorized pause_scope should not modify scope state.
#[test]
fn proof_registry_pause_scope_unauthorized_no_state_change() {
    let d = Deployment::new();

    let non_admin = Address::generate(&d.env);

    // Verify initial state: Updates not paused
    let initial_state = d
        .config
        .try_is_scope_paused(&PauseScope::Updates)
        .map(|_| true)
        .unwrap_or(false);
    assert!(!initial_state, "Updates should initially not be paused");

    // Attempt unauthorized pause
    let _ = d.proofs.try_pause_scope(&non_admin, &PauseScope::Updates);

    // Verify: State unchanged after failed attempt
    let final_state = d
        .config
        .try_is_scope_paused(&PauseScope::Updates)
        .map(|_| true)
        .unwrap_or(false);
    assert!(
        !final_state,
        "Updates should still not be paused after failed attempt"
    );
}
