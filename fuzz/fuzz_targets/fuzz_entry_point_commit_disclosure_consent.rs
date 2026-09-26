#![no_main]
use libfuzzer_sys::fuzz_target;
use earnproof_shared::ProofError;
use issuer_registry::{IssuerRegistryContract, IssuerRegistryContractClient};
use proof_registry::{ProofRegistryContract, ProofRegistryContractClient};
use protocol_config::{ProtocolConfigContract, ProtocolConfigContractClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, Bytes, BytesN, Env};

fuzz_target!(|data: &[u8]| {
    if data.len() < 132 || data.len() > 4096 {
        return;
    }

    let env = Env::default();
    env.ledger().set_timestamp(1_000_000);
    env.mock_all_auths();

    let proof_id = BytesN::<32>::try_from(Bytes::from_slice(&env, &data[0..32]))
        .expect("fixed-size proof id slice");
    let policy_hash = BytesN::<32>::try_from(Bytes::from_slice(&env, &data[32..64]))
        .expect("fixed-size policy hash slice");
    let receipt_hash = BytesN::<32>::try_from(Bytes::from_slice(&env, &data[64..96]))
        .expect("fixed-size receipt hash slice");
    let receipt_version = u32::from_be_bytes([data[96], data[97], data[98], data[99]]);
    let proof_commitment = BytesN::<32>::try_from(Bytes::from_slice(&env, &data[100..132]))
        .expect("fixed-size proof commitment slice");

    let admin = Address::generate(&env);
    let issuer = Address::from_str(
        &env,
        "GCATS5YOVB6ROX2WUNKGNQ2MP3GMXDMKSG2O4N5CLX3A6W4PZGZZI55U",
    );
    let config_id = env.register(ProtocolConfigContract, ());
    let config = ProtocolConfigContractClient::new(&env, &config_id);
    config.initialize(&admin);
    config.approve_schema_version(&1);

    let issuer_registry_id = env.register(IssuerRegistryContract, ());
    let issuers = IssuerRegistryContractClient::new(&env, &issuer_registry_id);
    issuers.initialize(&admin);
    issuers.register_issuer(
        &BytesN::from_array(&env, &[1; 32]),
        &issuer,
        &BytesN::from_array(&env, &[2; 32]),
        &BytesN::from_array(&env, &[3; 32]),
    );

    let proof_registry_id = env.register(ProofRegistryContract, ());
    let proofs = ProofRegistryContractClient::new(&env, &proof_registry_id);
    proofs.initialize(&admin, &issuer_registry_id, &config_id);
    proofs.register_proof(
        &proof_id,
        &proof_commitment,
        &issuer,
        &1,
        &(env.ledger().timestamp() + 1_000),
    );

    let first = proofs.try_commit_disclosure_consent(
        &proof_id,
        &policy_hash,
        &receipt_version,
        &receipt_hash,
    );
    assert!(first.is_ok(), "active proof accepts a well-sized receipt hash");

    let duplicate = proofs.try_commit_disclosure_consent(
        &proof_id,
        &policy_hash,
        &receipt_version,
        &receipt_hash,
    );
    assert_eq!(
        duplicate,
        Err(Ok(ProofError::ConsentReceiptAlreadyCommitted))
    );
});