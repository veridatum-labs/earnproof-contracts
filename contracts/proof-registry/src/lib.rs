#![no_std]

use earnproof_shared::{ProofRecord, ProofStatus};
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env};

#[contract]
pub struct ProofRegistryContract;

#[contracttype]
enum DataKey {
    Admin,
    IssuerRegistry,
    ProtocolConfig,
    Proof(BytesN<32>),
}

#[contractimpl]
impl ProofRegistryContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        issuer_registry: Address,
        protocol_config: Address,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::IssuerRegistry, &issuer_registry);
        env.storage()
            .instance()
            .set(&DataKey::ProtocolConfig, &protocol_config);
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

        if schema_version == 0 {
            panic!("schema version must be greater than zero");
        }

        if expires_at <= env.ledger().timestamp() {
            panic!("proof expiration must be in the future");
        }

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
        Self::set_revoked(env, proof_id_hash, false);
    }

    pub fn admin_revoke_proof(env: Env, proof_id_hash: BytesN<32>) {
        Self::set_revoked(env, proof_id_hash, true);
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

    pub fn is_revoked(env: Env, proof_id_hash: BytesN<32>) -> bool {
        let record = Self::get_proof(env, proof_id_hash);
        record.status == ProofStatus::Revoked
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }

    pub fn get_issuer_registry(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::IssuerRegistry)
            .expect("issuer registry not configured")
    }

    pub fn get_protocol_config(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::ProtocolConfig)
            .expect("protocol config not configured")
    }

    fn set_revoked(env: Env, proof_id_hash: BytesN<32>, by_admin: bool) {
        let key = DataKey::Proof(proof_id_hash.clone());
        let mut record: ProofRecord = env
            .storage()
            .persistent()
            .get(&key)
            .expect("proof not found");

        if by_admin {
            Self::get_admin(env.clone()).require_auth();
        } else {
            record.issuer_address.require_auth();
        }

        if record.status == ProofStatus::Revoked {
            panic!("proof already revoked");
        }

        record.status = ProofStatus::Revoked;
        record.revoked_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);
    }
}
