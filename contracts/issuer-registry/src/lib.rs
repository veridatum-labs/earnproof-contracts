#![no_std]

use earnproof_shared::{IssuerRecord, IssuerStatus};
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env};

#[contract]
pub struct IssuerRegistryContract;

#[contracttype]
enum DataKey {
    Admin,
    Issuer(BytesN<32>),
    AddressIssuer(Address),
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

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
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

        let address_key = DataKey::AddressIssuer(issuer_address.clone());
        if env.storage().persistent().has(&address_key) {
            panic!("issuer address already registered");
        }

        let now = env.ledger().timestamp();
        let record = IssuerRecord {
            issuer_id_hash: issuer_id_hash.clone(),
            issuer_address: issuer_address.clone(),
            metadata_hash,
            status: IssuerStatus::Active,
            created_at: now,
            updated_at: now,
        };

        env.storage().persistent().set(&key, &record);
        env.storage()
            .persistent()
            .set(&address_key, &issuer_id_hash);
    }

    pub fn update_issuer(env: Env, issuer_id_hash: BytesN<32>, metadata_hash: BytesN<32>) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();

        let key = DataKey::Issuer(issuer_id_hash);
        let mut record: IssuerRecord = env
            .storage()
            .persistent()
            .get(&key)
            .expect("issuer not found");

        if record.status == IssuerStatus::Revoked {
            panic!("revoked issuer cannot be updated");
        }

        record.metadata_hash = metadata_hash;
        record.updated_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);
    }

    pub fn suspend_issuer(env: Env, issuer_id_hash: BytesN<32>) {
        Self::set_status(env, issuer_id_hash, IssuerStatus::Suspended);
    }

    pub fn reactivate_issuer(env: Env, issuer_id_hash: BytesN<32>) {
        Self::set_status(env, issuer_id_hash, IssuerStatus::Active);
    }

    pub fn revoke_issuer(env: Env, issuer_id_hash: BytesN<32>) {
        Self::set_status(env, issuer_id_hash, IssuerStatus::Revoked);
    }

    pub fn rotate_issuer_address(env: Env, issuer_id_hash: BytesN<32>, new_address: Address) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();

        let key = DataKey::Issuer(issuer_id_hash.clone());
        let mut record: IssuerRecord = env
            .storage()
            .persistent()
            .get(&key)
            .expect("issuer not found");

        if record.status == IssuerStatus::Revoked {
            panic!("revoked issuer cannot rotate address");
        }

        let new_address_key = DataKey::AddressIssuer(new_address.clone());
        if env.storage().persistent().has(&new_address_key) {
            panic!("issuer address already registered");
        }

        env.storage()
            .persistent()
            .remove(&DataKey::AddressIssuer(record.issuer_address.clone()));
        record.issuer_address = new_address.clone();
        record.updated_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);
        env.storage()
            .persistent()
            .set(&new_address_key, &issuer_id_hash);
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

    pub fn is_active_address(env: Env, issuer_address: Address) -> bool {
        let issuer_id_hash: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::AddressIssuer(issuer_address))
            .expect("issuer address not found");

        Self::is_active_issuer(env, issuer_id_hash)
    }

    fn set_status(env: Env, issuer_id_hash: BytesN<32>, status: IssuerStatus) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();

        let key = DataKey::Issuer(issuer_id_hash);
        let mut record: IssuerRecord = env
            .storage()
            .persistent()
            .get(&key)
            .expect("issuer not found");

        if record.status == IssuerStatus::Revoked && status != IssuerStatus::Revoked {
            panic!("revoked issuer cannot be reactivated");
        }

        record.status = status;
        record.updated_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);
    }

    pub fn get_issuer_by_address(env: Env, issuer_address: Address) -> IssuerRecord {
        let issuer_id_hash: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::AddressIssuer(issuer_address))
            .expect("issuer address not found");

        env.storage()
            .persistent()
            .get(&DataKey::Issuer(issuer_id_hash))
            .expect("issuer not found")
    }
}
