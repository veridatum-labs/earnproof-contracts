#![no_std]

use earnproof_shared::{ProofRecord, ProofStatus};
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env};

#[contract]
pub struct ProofRegistryContract;

#[contracttype]
enum DataKey {
    Admin,
    Proof(BytesN<32>),
}

#[contractimpl]
impl ProofRegistryContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn register_proof(
        env: Env,
        proof_id_hash: BytesN<32>,
        commitment_hash: BytesN<32>,
        issuer_address: Address,
        schema_version: u32,
        expires_at: u64,
    ) {
        issuer_address.require_auth();

        let key = DataKey::Proof(proof_id_hash.clone());
        if env.storage().persistent().has(&key) {
            panic!("proof already registered");
        }

        let now = env.ledger().timestamp();
        let record = ProofRecord {
            proof_id_hash,
            commitment_hash,
            issuer_address,
            status: ProofStatus::Active,
            schema_version,
            expires_at,
            created_at: now,
            revoked_at: 0,
        };

        env.storage().persistent().set(&key, &record);
    }

    pub fn revoke_proof(env: Env, proof_id_hash: BytesN<32>) {
        let key = DataKey::Proof(proof_id_hash.clone());
        let mut record: ProofRecord = env
            .storage()
            .persistent()
            .get(&key)
            .expect("proof not found");

        record.issuer_address.require_auth();
        record.status = ProofStatus::Revoked;
        record.revoked_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);
    }

    pub fn get_proof(env: Env, proof_id_hash: BytesN<32>) -> ProofRecord {
        env.storage()
            .persistent()
            .get(&DataKey::Proof(proof_id_hash))
            .expect("proof not found")
    }

    pub fn is_valid_proof(env: Env, proof_id_hash: BytesN<32>) -> bool {
        let record = Self::get_proof(env.clone(), proof_id_hash);
        record.status == ProofStatus::Active && env.ledger().timestamp() <= record.expires_at
    }
}
