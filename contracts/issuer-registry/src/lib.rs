#![no_std]

use earnproof_shared::{IssuerRecord, IssuerStatus};
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env};

#[contract]
pub struct IssuerRegistryContract;

#[contracttype]
enum DataKey {
    Admin,
    Issuer(BytesN<32>),
}

#[contractimpl]
impl IssuerRegistryContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn register_issuer(
        env: Env,
        issuer_id_hash: BytesN<32>,
        issuer_address: Address,
        metadata_hash: BytesN<32>,
    ) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();

        let key = DataKey::Issuer(issuer_id_hash.clone());
        if env.storage().persistent().has(&key) {
            panic!("issuer already registered");
        }

        let now = env.ledger().timestamp();
        let record = IssuerRecord {
            issuer_id_hash,
            issuer_address,
            metadata_hash,
            status: IssuerStatus::Active,
            created_at: now,
            updated_at: now,
        };

        env.storage().persistent().set(&key, &record);
    }

    pub fn get_issuer(env: Env, issuer_id_hash: BytesN<32>) -> IssuerRecord {
        env.storage()
            .persistent()
            .get(&DataKey::Issuer(issuer_id_hash))
            .expect("issuer not found")
    }

    pub fn is_active_issuer(env: Env, issuer_id_hash: BytesN<32>) -> bool {
        let record = Self::get_issuer(env, issuer_id_hash);
        record.status == IssuerStatus::Active
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }
}
