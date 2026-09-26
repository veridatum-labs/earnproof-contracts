//! Stale and former-admin identities.
//!
//! The matrix in [`crate::matrix`] proves that a *never-authorized* identity is
//! rejected. This module proves the stronger claim the acceptance criteria
//! name: identities that were once authorized lose that authority the moment
//! it rotates away. A rotated-out protocol admin, and a rotated-out issuer
//! address, must not be able to write anything afterwards — and the few
//! write capabilities that intentionally survive a rotation are pinned here
//! so the boundary is explicit rather than accidental.

use crate::harness::{authorize, hash, Deployment, APPROVED_SCHEMA};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, IntoVal};

/// Attempts every privileged `protocol-config` mutation signed by `signer`,
/// returning `(entry point, accepted?)` pairs.
fn config_attempts(d: &Deployment, signer: &Address) -> std::vec::Vec<(&'static str, bool)> {
    let mut out = std::vec::Vec::new();

    let p1 = hash(&d.env, 0x10);
    authorize(
        &d.env,
        signer,
        &d.config_address,
        "pause",
        (&p1,).into_val(&d.env),
    );
    out.push(("protocol-config::pause", d.config.try_pause(&p1).is_ok()));

    let p2 = hash(&d.env, 0x11);
    authorize(
        &d.env,
        signer,
        &d.config_address,
        "unpause",
        (&p2,).into_val(&d.env),
    );
    out.push((
        "protocol-config::unpause",
        d.config.try_unpause(&p2).is_ok(),
    ));

    let next = Address::generate(&d.env);
    let p3 = hash(&d.env, 0x12);
    authorize(
        &d.env,
        signer,
        &d.config_address,
        "set_admin",
        (&p3, &next).into_val(&d.env),
    );
    out.push((
        "protocol-config::set_admin",
        d.config.try_set_admin(&p3, &next).is_ok(),
    ));

    let version = 7_u32;
    let p4 = hash(&d.env, 0x13);
    authorize(
        &d.env,
        signer,
        &d.config_address,
        "approve_schema_version",
        (&p4, &version).into_val(&d.env),
    );
    out.push((
        "protocol-config::approve_schema_version",
        d.config.try_approve_schema_version(&p4, &version).is_ok(),
    ));

    let p5 = hash(&d.env, 0x14);
    authorize(
        &d.env,
        signer,
        &d.config_address,
        "deprecate_schema_version",
        (&p5, &APPROVED_SCHEMA).into_val(&d.env),
    );
    out.push((
        "protocol-config::deprecate_schema_version",
        d.config
            .try_deprecate_schema_version(&p5, &APPROVED_SCHEMA)
            .is_ok(),
    ));

    out
}

#[test]
fn a_former_admin_retains_no_authority_over_any_privileged_mutation() {
    let deployment = Deployment::new();
    let former = deployment.admin.clone();
    let successor = Address::generate(&deployment.env);

    deployment.set_admin(&successor);
    assert_eq!(deployment.config.get_admin(), successor);

    let before = deployment.snapshot();
    for (name, accepted) in config_attempts(&deployment, &former) {
        assert!(
            !accepted,
            "{name}: the former admin was accepted after rotation"
        );
        deployment.assert_no_side_effects(&before, name);
    }

    // Control: the successor still holds full authority, so the rejections
    // above are attributable to the rotation and nothing else.
    let p = hash(&deployment.env, 0x15);
    authorize(
        &deployment.env,
        &successor,
        &deployment.config_address,
        "pause",
        (&p,).into_val(&deployment.env),
    );
    assert!(deployment.config.try_pause(&p).is_ok());
    assert!(deployment.config.is_paused());
}

#[test]
fn a_former_admin_cannot_reclaim_authority_by_rotating_to_themselves() {
    // The most direct privilege-recovery attempt: the removed admin tries to
    // put themselves back in charge. Only the current admin may rotate, so
    // this must fail and leave the successor in place.
    let deployment = Deployment::new();
    let former = deployment.admin.clone();
    let successor = Address::generate(&deployment.env);

    deployment.set_admin(&successor);

    let p = hash(&deployment.env, 0x16);
    authorize(
        &deployment.env,
        &former,
        &deployment.config_address,
        "set_admin",
        (&p, &former).into_val(&deployment.env),
    );
    assert!(deployment.config.try_set_admin(&p, &former).is_err());

    assert_eq!(deployment.config.get_admin(), successor);
    assert!(
        !deployment.config.is_paused(),
        "the failed reclaim must not change any other state"
    );
}

#[test]
fn rotation_to_the_incumbent_keeps_authority_intact() {
    // A retried rotation that names the current admin is a no-op for
    // authority and must not strand the contract.
    let deployment = Deployment::new();
    let admin = deployment.admin.clone();

    deployment.set_admin(&admin);
    assert_eq!(deployment.config.get_admin(), admin);

    let p = hash(&deployment.env, 0x17);
    authorize(
        &deployment.env,
        &admin,
        &deployment.config_address,
        "pause",
        (&p,).into_val(&deployment.env),
    );
    assert!(deployment.config.try_pause(&p).is_ok());
    assert!(deployment.config.is_paused());
}

#[test]
fn a_rotated_out_issuer_address_loses_issuer_status() {
    let deployment = Deployment::new();
    let old = deployment.issuer.clone();
    let replacement = Address::generate(&deployment.env);

    deployment.rotate_issuer_address(&deployment.issuer_id, &replacement);

    assert!(!deployment.issuers.is_active_address(&old));
    assert!(deployment.issuers.is_active_address(&replacement));

    // The old address can still authorize for its own name, so the rejection
    // must come from the issuer-status precondition — and it must leave no
    // state behind.
    let proof_id = hash(&deployment.env, 0xE1);
    let commitment = hash(&deployment.env, 0xE2);
    let expires_at = deployment.env.ledger().timestamp() + 100_000;
    let before = deployment.snapshot();

    authorize(
        &deployment.env,
        &old,
        &deployment.proofs_address,
        "register_proof",
        (&proof_id, &commitment, &old, &APPROVED_SCHEMA, &expires_at).into_val(&deployment.env),
    );
    assert!(
        deployment
            .proofs
            .try_register_proof(&proof_id, &commitment, &old, &APPROVED_SCHEMA, &expires_at)
            .is_err(),
        "a rotated-out issuer address must not register proofs"
    );
    deployment.assert_no_side_effects(&before, "rotated-out issuer on register_proof");

    // Control: the replacement address can register.
    authorize(
        &deployment.env,
        &replacement,
        &deployment.proofs_address,
        "register_proof",
        (
            &proof_id,
            &commitment,
            &replacement,
            &APPROVED_SCHEMA,
            &expires_at,
        )
            .into_val(&deployment.env),
    );
    assert!(
        deployment
            .proofs
            .try_register_proof(
                &proof_id,
                &commitment,
                &replacement,
                &APPROVED_SCHEMA,
                &expires_at
            )
            .is_ok(),
        "the replacement address must be able to register proofs"
    );
}

#[test]
fn a_rotated_out_issuer_address_keeps_revocation_authority_over_its_historical_proofs() {
    // Revocation authority is bound to the address stored on the proof
    // record, which rotation does not rewrite. The old key can still retire
    // the proofs it issued. Asserting this keeps the boundary explicit rather
    // than accidental.
    let deployment = Deployment::new();
    let old = deployment.issuer.clone();
    let replacement = Address::generate(&deployment.env);
    let proof_id = deployment.register_proof(0xE5); // registered by `old`

    deployment.rotate_issuer_address(&deployment.issuer_id, &replacement);

    authorize(
        &deployment.env,
        &old,
        &deployment.proofs_address,
        "revoke_proof",
        (&proof_id,).into_val(&deployment.env),
    );
    assert!(
        deployment.proofs.try_revoke_proof(&proof_id).is_ok(),
        "the rotated-out key must keep revocation authority over proofs it issued"
    );
    assert!(deployment.proofs.is_revoked(&proof_id));
}

#[test]
fn the_replacement_address_cannot_revoke_historical_proofs_of_the_rotated_out_address() {
    // The mirror of the test above: the *new* address did not register the
    // historical proof, so it cannot revoke it — the demand stays with the
    // address stored on the proof record.
    let deployment = Deployment::new();
    let replacement = Address::generate(&deployment.env);
    let proof_id = deployment.register_proof(0xE6); // registered by `deployment.issuer`

    deployment.rotate_issuer_address(&deployment.issuer_id, &replacement);

    let before = deployment.snapshot();
    authorize(
        &deployment.env,
        &replacement,
        &deployment.proofs_address,
        "revoke_proof",
        (&proof_id,).into_val(&deployment.env),
    );
    assert!(
        deployment.proofs.try_revoke_proof(&proof_id).is_err(),
        "the replacement address must not revoke proofs it did not register"
    );
    deployment.assert_no_side_effects(&before, "replacement address on revoke_proof");
}

#[test]
fn rotating_the_config_admin_does_not_move_registry_authority() {
    // The three contracts hold separate administrator records. Rotating the
    // protocol-config admin must not silently move authority over the
    // registries.
    let deployment = Deployment::new();
    let registry_admin = deployment.admin.clone();
    let proof_id = deployment.register_proof(0xD1);
    let config_admin = Address::generate(&deployment.env);

    deployment.set_admin(&config_admin);

    assert_eq!(deployment.config.get_admin(), config_admin);
    assert_eq!(deployment.issuers.get_admin(), registry_admin);
    assert_eq!(deployment.proofs.get_admin(), registry_admin);

    let before = deployment.snapshot();

    // The new config admin must not inherit issuer-registry authority...
    let p = hash(&deployment.env, 0x18);
    authorize(
        &deployment.env,
        &config_admin,
        &deployment.issuers_address,
        "suspend_issuer",
        (&p, &deployment.issuer_id).into_val(&deployment.env),
    );
    assert!(deployment
        .issuers
        .try_suspend_issuer(&p, &deployment.issuer_id)
        .is_err());
    deployment.assert_no_side_effects(&before, "config admin on suspend_issuer");

    // ...nor proof-registry admin authority.
    authorize(
        &deployment.env,
        &config_admin,
        &deployment.proofs_address,
        "admin_revoke_proof",
        (&proof_id,).into_val(&deployment.env),
    );
    assert!(deployment.proofs.try_admin_revoke_proof(&proof_id).is_err());
    deployment.assert_no_side_effects(&before, "config admin on admin_revoke_proof");

    // Control: the unchanged registry admin still can.
    authorize(
        &deployment.env,
        &registry_admin,
        &deployment.proofs_address,
        "admin_revoke_proof",
        (&proof_id,).into_val(&deployment.env),
    );
    assert!(deployment.proofs.try_admin_revoke_proof(&proof_id).is_ok());
}
