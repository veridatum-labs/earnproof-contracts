#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env};

#[contract]
pub struct ProtocolConfigContract;

#[contracttype]
enum DataKey {
    Admin,
    Paused,
    ConfigVersion,
    SchemaVersion(u32),
}

#[contractimpl]
impl ProtocolConfigContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage()
            .instance()
            .set(&DataKey::ConfigVersion, &1_u32);
        env.events().publish((symbol_short!("init"),), admin);
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }

    pub fn set_admin(env: Env, new_admin: Address) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Self::bump_config_version(env.clone());
        env.events().publish((symbol_short!("admin"),), new_admin);
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    pub fn pause(env: Env) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
        Self::bump_config_version(env.clone());
        env.events().publish((symbol_short!("pause"),), true);
    }

    pub fn unpause(env: Env) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
        Self::bump_config_version(env.clone());
        env.events().publish((symbol_short!("unpause"),), false);
    }

    pub fn approve_schema_version(env: Env, version: u32) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();
        Self::ensure_nonzero_version(version);
        env.storage()
            .persistent()
            .set(&DataKey::SchemaVersion(version), &true);
        Self::bump_config_version(env.clone());
        env.events()
            .publish((symbol_short!("schema"), symbol_short!("approve")), version);
    }

    pub fn deprecate_schema_version(env: Env, version: u32) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();
        Self::ensure_nonzero_version(version);
        env.storage()
            .persistent()
            .set(&DataKey::SchemaVersion(version), &false);
        Self::bump_config_version(env.clone());
        env.events()
            .publish((symbol_short!("schema"), symbol_short!("deprec")), version);
    }

    pub fn is_schema_version_approved(env: Env, version: u32) -> bool {
        if version == 0 {
            return false;
        }

        env.storage()
            .persistent()
            .get(&DataKey::SchemaVersion(version))
            .unwrap_or(false)
    }

    pub fn get_config_version(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ConfigVersion)
            .unwrap_or(0)
    }

    fn ensure_nonzero_version(version: u32) {
        if version == 0 {
            panic!("schema version must be greater than zero");
        }
    }

    fn bump_config_version(env: Env) {
        let current = Self::get_config_version(env.clone());
        env.storage()
            .instance()
            .set(&DataKey::ConfigVersion, &(current + 1));
    }
}
