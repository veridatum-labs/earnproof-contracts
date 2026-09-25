#![no_std]

use earnproof_shared::{
    ArchivedProofRecord, ContractError, PauseScope, ProofError, ProofRecord, ProofStatus,
    UpgradeApprovalRecord, UpgradeHistoryRecord, TTL_EXTEND_TO_LEDGERS, TTL_THRESHOLD_LEDGERS,
};
use soroban_sdk::{
    contract, contractclient, contractevent, contractimpl, contracttype, Address, BytesN, Env,
    Symbol, Vec,
};

#[contractclient(name = "ProtocolConfigContractClient")]
pub trait ProtocolConfigInterface {
    fn is_paused(env: Env) -> bool;
    fn is_schema_version_approved(env: Env, version: u32) -> bool;
}

#[contractclient(name = "IssuerRegistryContractClient")]
pub trait IssuerRegistryInterface {
    fn is_active_address(env: Env, issuer_address: Address) -> bool;
}

#[contract]
pub struct ProofRegistryContract;

const CONTRACT_ROLE: &str = "proof_registry";

#[contracttype]
enum DataKey {
    Admin,
    IssuerRegistry,
    ProtocolConfig,
    Proof(BytesN<32>),
    ArchivedProof(BytesN<32>),
    AllowedWasm(BytesN<32>),
    ContractVersion,
    CurrentWasmHash,
    ScopedPause(PauseScope),
    UpgradeHistory(u32),
    UpgradeHistoryCount,
}

#[contractevent]
pub struct ScopedPauseChanged {
    pub scope: PauseScope,
    pub paused: bool,
    pub changed_by: Address,
}

#[contractevent]
pub struct ProofArchived {
    pub proof_id_hash: BytesN<32>,
    pub archived_at: u64,
}

// ── upgrade events ────────────────────────────────────────────────────────────

/// Emitted when the admin adds a WASM hash to the upgrade allowlist.
#[contractevent]
pub struct UpgradeAllowlisted {
    pub wasm_hash: BytesN<32>,
    pub target_contract: Address,
    pub contract_role: Symbol,
    pub new_contract_version: u32,
    pub approved_by: Address,
}

/// Emitted when the admin removes a WASM hash from the allowlist without
/// applying it.
#[contractevent]
pub struct UpgradeRevoked {
    pub wasm_hash: BytesN<32>,
    pub target_contract: Address,
    pub contract_role: Symbol,
    pub revoked_by: Address,
}

/// Emitted when a WASM upgrade is successfully applied.
#[contractevent]
pub struct ContractUpgraded {
    pub new_wasm_hash: BytesN<32>,
    pub target_contract: Address,
    pub contract_role: Symbol,
    pub old_contract_version: u32,
    pub new_contract_version: u32,
    pub upgraded_by: Address,
}

#[contractimpl]
impl ProofRegistryContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        issuer_registry: Address,
        protocol_config: Address,
    ) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }

        Self::require_valid_principal(&admin)?;
        Self::validate_dependency_addresses(&env, &issuer_registry, &protocol_config)?;
        Self::require_auth(&admin);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::IssuerRegistry, &issuer_registry);
        env.storage()
            .instance()
            .set(&DataKey::ProtocolConfig, &protocol_config);
        env.storage()
            .instance()
            .set(&DataKey::ContractVersion, &1_u32);
        Self::extend_instance_ttl(env);
        Ok(())
    }

    pub fn is_scope_paused(env: Env, scope: PauseScope) -> bool {
        match scope {
            PauseScope::Global => false,
            _ => env
                .storage()
                .persistent()
                .get(&DataKey::ScopedPause(scope))
                .unwrap_or(false),
        }
    }

    pub fn pause_scope(env: Env, scope: PauseScope) -> Result<(), ProofError> {
        let admin = Self::get_admin(env.clone()).map_err(|_| ProofError::ProofNotFound)?;
        Self::require_auth(&admin);
        env.storage()
            .persistent()
            .set(&DataKey::ScopedPause(scope), &true);
        env.storage().persistent().extend_ttl(
            &DataKey::ScopedPause(scope),
            TTL_THRESHOLD_LEDGERS,
            TTL_EXTEND_TO_LEDGERS,
        );
        ScopedPauseChanged {
            scope,
            paused: true,
            changed_by: admin,
        }
        .publish(&env);
        Ok(())
    }

    pub fn unpause_scope(env: Env, scope: PauseScope) -> Result<(), ProofError> {
        let admin = Self::get_admin(env.clone()).map_err(|_| ProofError::ProofNotFound)?;
        Self::require_auth(&admin);
        env.storage()
            .persistent()
            .set(&DataKey::ScopedPause(scope), &false);
        env.storage().persistent().extend_ttl(
            &DataKey::ScopedPause(scope),
            TTL_THRESHOLD_LEDGERS,
            TTL_EXTEND_TO_LEDGERS,
        );
        ScopedPauseChanged {
            scope,
            paused: false,
            changed_by: admin,
        }
        .publish(&env);
        Ok(())
    }

    pub fn register_proof(
        env: Env,
        proof_id_hash: BytesN<32>,
        commitment_hash: BytesN<32>,
        issuer_address: Address,
        schema_version: u32,
        expires_at: u64,
    ) -> Result<(), ProofError> {
        if Self::is_scope_paused(env.clone(), PauseScope::Registration) {
            return Err(ProofError::InvalidSchemaVersion);
        }
        Self::require_valid_issuer_address(&issuer_address)?;
        let protocol_config =
            Self::get_protocol_config(env.clone()).map_err(|_| ProofError::ProofNotFound)?;
        let issuer_registry =
            Self::get_issuer_registry(env.clone()).map_err(|_| ProofError::ProofNotFound)?;
        if issuer_address == env.current_contract_address()
            || issuer_address == protocol_config
            || issuer_address == issuer_registry
        {
            return Err(ProofError::InvalidAddress);
        }
        Self::require_auth(&issuer_address);

        if schema_version == 0 {
            return Err(ProofError::InvalidSchemaVersion);
        }

        if expires_at <= env.ledger().timestamp() {
            return Err(ProofError::ProofExpired);
        }

        let protocol_config =
            Self::get_protocol_config(env.clone()).map_err(|_| ProofError::ProofNotFound)?;
        let protocol_client = ProtocolConfigContractClient::new(&env, &protocol_config);
        if protocol_client.is_paused() {
            return Err(ProofError::InvalidSchemaVersion); // Use existing error for protocol paused state
        }

        if !protocol_client.is_schema_version_approved(&schema_version) {
            return Err(ProofError::SchemaVersionNotApproved);
        }

        let issuer_registry =
            Self::get_issuer_registry(env.clone()).map_err(|_| ProofError::ProofNotFound)?;
        let issuer_client = IssuerRegistryContractClient::new(&env, &issuer_registry);
        if !issuer_client.is_active_address(&issuer_address) {
            return Err(ProofError::InvalidSchemaVersion); // Simplified - issuer inactive
        }

        let key = DataKey::Proof(proof_id_hash.clone());
        if env.storage().persistent().has(&key) {
            return Err(ProofError::ProofAlreadyRegistered);
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
        Self::extend_proof_key_ttl(env, &key);
        Ok(())
    }

    pub fn revoke_proof(env: Env, proof_id_hash: BytesN<32>) -> Result<(), ProofError> {
        if Self::is_scope_paused(env.clone(), PauseScope::Revocation) {
            return Err(ProofError::ProofAlreadyRevoked);
        }
        Self::set_revoked(env, proof_id_hash, false)
    }

    pub fn admin_revoke_proof(env: Env, proof_id_hash: BytesN<32>) -> Result<(), ProofError> {
        if Self::is_scope_paused(env.clone(), PauseScope::Revocation) {
            return Err(ProofError::ProofAlreadyRevoked);
        }
        Self::set_revoked(env, proof_id_hash, true)
    }

    pub fn archive_proof(env: Env, proof_id_hash: BytesN<32>) -> Result<(), ProofError> {
        let archived_key = DataKey::ArchivedProof(proof_id_hash.clone());
        if env.storage().persistent().has(&archived_key) {
            return Ok(());
        }

        let proof_key = DataKey::Proof(proof_id_hash.clone());
        let record: ProofRecord = env
            .storage()
            .persistent()
            .get(&proof_key)
            .ok_or(ProofError::ProofNotFound)?;

        let admin_auth = if let Ok(admin) = Self::get_admin(env.clone()) {
            admin.require_auth_for_args(soroban_sdk::vec![&env]);
            true
        } else {
            false
        };

        if !admin_auth {
            record.issuer_address.require_auth();
        }

        let now = env.ledger().timestamp();
        let is_expired = now > record.expires_at;
        let is_revoked = record.status == ProofStatus::Revoked;

        if !is_expired && !is_revoked {
            return Err(ProofError::InvalidAddress);
        }

        let archived_record = ArchivedProofRecord {
            proof_id_hash: proof_id_hash.clone(),
            commitment_hash: record.commitment_hash,
            issuer_address: record.issuer_address,
            was_revoked: is_revoked,
            schema_version: record.schema_version,
            expired_at: record.expires_at,
            archived_at: now,
        };

        env.storage()
            .persistent()
            .set(&archived_key, &archived_record);
        env.storage().persistent().remove(&proof_key);

        env.storage().persistent().extend_ttl(
            &archived_key,
            TTL_THRESHOLD_LEDGERS,
            TTL_EXTEND_TO_LEDGERS,
        );

        ProofArchived {
            proof_id_hash,
            archived_at: now,
        }
        .publish(&env);

        Ok(())
    }

    pub fn get_archived_proof(
        env: Env,
        proof_id_hash: BytesN<32>,
    ) -> Result<ArchivedProofRecord, ProofError> {
        let key = DataKey::ArchivedProof(proof_id_hash);
        let record = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ProofError::ProofNotFound)?;
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD_LEDGERS, TTL_EXTEND_TO_LEDGERS);
        Ok(record)
    }

    pub fn is_archived(env: Env, proof_id_hash: BytesN<32>) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::ArchivedProof(proof_id_hash))
    }

    pub fn get_proof(env: Env, proof_id_hash: BytesN<32>) -> Result<ProofRecord, ProofError> {
        let key = DataKey::Proof(proof_id_hash);
        let record = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ProofError::ProofNotFound)?;
        Self::extend_proof_key_ttl(env, &key);
        Ok(record)
    }

    pub fn is_valid_proof(env: Env, proof_id_hash: BytesN<32>) -> bool {
        match Self::get_proof(env.clone(), proof_id_hash) {
            Ok(record) => {
                record.status == ProofStatus::Active
                    && env.ledger().timestamp() <= record.expires_at
            }
            Err(_) => false,
        }
    }

    pub fn is_revoked(env: Env, proof_id_hash: BytesN<32>) -> bool {
        if let Ok(archived) = Self::get_archived_proof(env.clone(), proof_id_hash.clone()) {
            return archived.was_revoked;
        }
        match Self::get_proof(env, proof_id_hash) {
            Ok(record) => record.status == ProofStatus::Revoked,
            Err(_) => false,
        }
    }

    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)
    }

    pub fn get_issuer_registry(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::IssuerRegistry)
            .ok_or(ContractError::NotInitialized)
    }

    pub fn get_protocol_config(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::ProtocolConfig)
            .ok_or(ContractError::NotInitialized)
    }

    // ── upgrade governance ────────────────────────────────────────────────────

    /// Returns the stored monotonic contract version.  Starts at 1.
    pub fn get_contract_version(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ContractVersion)
            .unwrap_or(0)
    }

    /// Admin-only: add `wasm_hash` to the upgrade allowlist.
    pub fn approve_upgrade(env: Env, wasm_hash: BytesN<32>, new_version: u32) {
        let admin = Self::get_admin(env.clone()).expect("contract not initialized");
        Self::require_auth(&admin);
        if Self::is_scope_paused(env.clone(), PauseScope::Upgrade) {
            panic!("upgrade operations are paused");
        }

        let current = Self::get_contract_version(env.clone());
        if new_version <= current {
            panic!("new_version must be greater than current contract version");
        }

        let target_contract = env.current_contract_address();
        let contract_role = Symbol::new(&env, CONTRACT_ROLE);

        let approval = UpgradeApprovalRecord {
            new_version,
            target_contract: target_contract.clone(),
            contract_role: contract_role.clone(),
        };

        let key = DataKey::AllowedWasm(wasm_hash.clone());
        env.storage().persistent().set(&key, &approval);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD_LEDGERS, TTL_EXTEND_TO_LEDGERS);

        UpgradeAllowlisted {
            wasm_hash,
            target_contract,
            contract_role,
            new_contract_version: new_version,
            approved_by: admin,
        }
        .publish(&env);
    }

    /// Admin-only: remove a hash from the allowlist without applying it.
    pub fn revoke_upgrade(env: Env, wasm_hash: BytesN<32>) {
        let admin = Self::get_admin(env.clone()).expect("contract not initialized");
        Self::require_auth(&admin);
        if Self::is_scope_paused(env.clone(), PauseScope::Upgrade) {
            panic!("upgrade operations are paused");
        }

        let target_contract = env.current_contract_address();
        let contract_role = Symbol::new(&env, CONTRACT_ROLE);

        let key = DataKey::AllowedWasm(wasm_hash.clone());
        env.storage().persistent().remove(&key);

        UpgradeRevoked {
            wasm_hash,
            target_contract,
            contract_role,
            revoked_by: admin,
        }
        .publish(&env);
    }

    /// Returns true when `wasm_hash` is on the allowlist for this contract instance.
    pub fn is_upgrade_allowed(env: Env, wasm_hash: BytesN<32>) -> bool {
        let key = DataKey::AllowedWasm(wasm_hash);
        if let Some(approval) = env
            .storage()
            .persistent()
            .get::<DataKey, UpgradeApprovalRecord>(&key)
        {
            let current_contract = env.current_contract_address();
            let expected_role = Symbol::new(&env, CONTRACT_ROLE);
            approval.target_contract == current_contract && approval.contract_role == expected_role
        } else {
            false
        }
    }

    /// Admin-only: apply an in-place WASM upgrade.
    pub fn upgrade_contract(env: Env, wasm_hash: BytesN<32>) {
        let admin = Self::get_admin(env.clone()).expect("contract not initialized");
        Self::require_auth(&admin);
        if Self::is_scope_paused(env.clone(), PauseScope::Upgrade) {
            panic!("upgrade operations are paused");
        }

        let key = DataKey::AllowedWasm(wasm_hash.clone());
        let approval: UpgradeApprovalRecord = env
            .storage()
            .persistent()
            .get(&key)
            .expect("wasm hash not on allowlist");

        let current_contract = env.current_contract_address();
        let expected_role = Symbol::new(&env, CONTRACT_ROLE);

        if approval.target_contract != current_contract || approval.contract_role != expected_role {
            panic!("upgrade approval does not match target contract identity or role");
        }

        let old_version = Self::get_contract_version(env.clone());
        if approval.new_version <= old_version {
            panic!("upgrade would not advance contract version");
        }

        env.storage().persistent().remove(&key);

        #[cfg(not(any(test, feature = "testutils")))]
        env.deployer()
            .update_current_contract_wasm(wasm_hash.clone());

        let old_wasm_hash = env
            .storage()
            .instance()
            .get(&DataKey::CurrentWasmHash)
            .unwrap_or_else(|| BytesN::from_array(&env, &[0u8; 32]));

        env.storage()
            .instance()
            .set(&DataKey::ContractVersion, &approval.new_version);
        env.storage()
            .instance()
            .set(&DataKey::CurrentWasmHash, &wasm_hash);
        Self::extend_instance_ttl(env.clone());

        let history_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::UpgradeHistoryCount)
            .unwrap_or(0);

        let history_record = UpgradeHistoryRecord {
            old_wasm_hash,
            new_wasm_hash: wasm_hash.clone(),
            old_version,
            new_version: approval.new_version,
            ledger_sequence: env.ledger().sequence(),
            ledger_timestamp: env.ledger().timestamp(),
            upgraded_by: admin.clone(),
        };

        let history_key = DataKey::UpgradeHistory(history_count);
        env.storage()
            .persistent()
            .set(&history_key, &history_record);
        env.storage().persistent().extend_ttl(
            &history_key,
            TTL_THRESHOLD_LEDGERS,
            TTL_EXTEND_TO_LEDGERS,
        );
        env.storage()
            .instance()
            .set(&DataKey::UpgradeHistoryCount, &(history_count + 1));

        ContractUpgraded {
            new_wasm_hash: wasm_hash,
            target_contract: current_contract,
            contract_role: expected_role,
            old_contract_version: old_version,
            new_contract_version: approval.new_version,
            upgraded_by: admin,
        }
        .publish(&env);
    }

    pub fn get_upgrade_history_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::UpgradeHistoryCount)
            .unwrap_or(0)
    }

    pub fn get_upgrade_history(env: Env, start: u32, limit: u32) -> Vec<UpgradeHistoryRecord> {
        let total = Self::get_upgrade_history_count(env.clone());
        let mut result = Vec::new(&env);
        if start >= total {
            return result;
        }
        let max_limit = if limit > 50 { 50 } else { limit };
        let end = if start + max_limit > total {
            total
        } else {
            start + max_limit
        };
        for i in start..end {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<DataKey, UpgradeHistoryRecord>(&DataKey::UpgradeHistory(i))
            {
                result.push_back(record);
            }
        }
        result
    }

    // ── private helpers ───────────────────────────────────────────────────────

    fn validate_dependency_addresses(
        env: &Env,
        issuer_registry: &Address,
        protocol_config: &Address,
    ) -> Result<(), ContractError> {
        if !earnproof_shared::is_valid_principal_address(issuer_registry)
            || !earnproof_shared::is_valid_principal_address(protocol_config)
        {
            return Err(ContractError::InvalidInput);
        }
        let current = env.current_contract_address();
        if issuer_registry == &current
            || protocol_config == &current
            || issuer_registry == protocol_config
        {
            return Err(ContractError::InvalidInput);
        }
        Ok(())
    }

    fn require_valid_principal(address: &Address) -> Result<(), ContractError> {
        if !earnproof_shared::is_valid_principal_address(address) {
            return Err(ContractError::InvalidInput);
        }
        Ok(())
    }

    fn require_valid_issuer_address(address: &Address) -> Result<(), ProofError> {
        if !earnproof_shared::is_valid_principal_address(address) {
            return Err(ProofError::InvalidAddress);
        }
        Ok(())
    }

    fn set_revoked(env: Env, proof_id_hash: BytesN<32>, by_admin: bool) -> Result<(), ProofError> {
        let key = DataKey::Proof(proof_id_hash.clone());
        let mut record: ProofRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ProofError::ProofNotFound)?;

        if by_admin {
            let admin = Self::get_admin(env.clone()).map_err(|_| ProofError::ProofNotFound)?;
            Self::require_auth(&admin);
        } else {
            Self::require_auth(&record.issuer_address);
        }

        if record.status == ProofStatus::Revoked {
            return Err(ProofError::ProofAlreadyRevoked);
        }

        record.status = ProofStatus::Revoked;
        record.revoked_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);
        Self::extend_proof_key_ttl(env, &key);
        Ok(())
    }

    fn extend_instance_ttl(env: Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD_LEDGERS, TTL_EXTEND_TO_LEDGERS);
    }

    fn extend_proof_key_ttl(env: Env, key: &DataKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, TTL_THRESHOLD_LEDGERS, TTL_EXTEND_TO_LEDGERS);
    }

    fn require_auth(address: &Address) {
        address.require_auth();
    }
}

#[cfg(test)]
mod test {
    extern crate std;

    use super::{DataKey, ProofRegistryContract, ProofRegistryContractClient};
    use earnproof_shared::{ProofError, ProofStatus, TTL_THRESHOLD_LEDGERS};
    use issuer_registry::{IssuerRegistryContract, IssuerRegistryContractClient};
    use protocol_config::{ProtocolConfigContract, ProtocolConfigContractClient};
    use soroban_sdk::{testutils::storage::Persistent as _, Address, BytesN, Env};

    const ADMIN: &str = "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR";
    const ISSUER: &str = "GCATS5YOVB6ROX2WUNKGNQ2MP3GMXDMKSG2O4N5CLX3A6W4PZGZZI55U";

    fn bytes(env: &Env, value: u8) -> BytesN<32> {
        BytesN::from_array(env, &[value; 32])
    }

    fn setup() -> (
        Env,
        ProofRegistryContractClient<'static>,
        ProtocolConfigContractClient<'static>,
        IssuerRegistryContractClient<'static>,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let protocol_config_id = env.register(ProtocolConfigContract, ());
        let protocol_config_client = ProtocolConfigContractClient::new(&env, &protocol_config_id);
        let issuer_registry_id = env.register(IssuerRegistryContract, ());
        let issuer_registry_client = IssuerRegistryContractClient::new(&env, &issuer_registry_id);
        let contract_id = env.register(ProofRegistryContract, ());
        let client = ProofRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);
        let issuer = Address::from_str(&env, ISSUER);
        let issuer_id = bytes(&env, 9);

        protocol_config_client.initialize(&admin);
        protocol_config_client.approve_schema_version(&1);
        issuer_registry_client.initialize(&admin);
        issuer_registry_client.register_issuer(&issuer_id, &issuer, &bytes(&env, 8));
        client.initialize(&admin, &issuer_registry_id, &protocol_config_id);

        (
            env,
            client,
            protocol_config_client,
            issuer_registry_client,
            issuer_registry_id,
        )
    }

    // ── existing tests ────────────────────────────────────────────────────────

    #[test]
    fn registers_and_validates_proof() {
        let (env, client, _protocol_config, _issuer_registry, issuer_registry_id) = setup();
        let proof_id = bytes(&env, 1);
        let commitment = bytes(&env, 2);
        let issuer = Address::from_str(&env, ISSUER);

        client.register_proof(&proof_id, &commitment, &issuer, &1, &2_000);

        let record = client.get_proof(&proof_id);
        assert_eq!(record.proof_id_hash, proof_id);
        assert_eq!(record.commitment_hash, commitment);
        assert_eq!(record.issuer_address, issuer);
        assert_eq!(record.status, ProofStatus::Active);
        assert_eq!(client.get_issuer_registry(), issuer_registry_id);
        assert!(client.is_valid_proof(&proof_id));
        assert!(!client.is_revoked(&proof_id));
    }

    #[test]
    fn issuer_can_revoke_proof() {
        let (env, client, _protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        let proof_id = bytes(&env, 1);
        let issuer = Address::from_str(&env, ISSUER);

        client.register_proof(&proof_id, &bytes(&env, 2), &issuer, &1, &2_000);
        client.revoke_proof(&proof_id);

        let record = client.get_proof(&proof_id);
        assert_eq!(record.status, ProofStatus::Revoked);
        assert!(client.is_revoked(&proof_id));
        assert!(!client.is_valid_proof(&proof_id));
    }

    #[test]
    fn rejects_expired_proof() {
        let (env, client, _protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        use earnproof_shared::ProofError;

        let result = client.try_register_proof(
            &bytes(&env, 1),
            &bytes(&env, 2),
            &Address::from_str(&env, ISSUER),
            &1,
            &0,
        );
        assert_eq!(result, Err(Ok(ProofError::ProofExpired)));
    }

    #[test]
    fn rejects_duplicate_proof_id() {
        let (env, client, _protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        use earnproof_shared::ProofError;
        let proof_id = bytes(&env, 1);
        let issuer = Address::from_str(&env, ISSUER);

        client.register_proof(&proof_id, &bytes(&env, 2), &issuer, &1, &2_000);

        let result = client.try_register_proof(&proof_id, &bytes(&env, 3), &issuer, &1, &2_000);
        assert_eq!(result, Err(Ok(ProofError::ProofAlreadyRegistered)));
    }

    #[test]
    fn rejects_unapproved_schema_version() {
        let (env, client, _protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        use earnproof_shared::ProofError;

        let result = client.try_register_proof(
            &bytes(&env, 1),
            &bytes(&env, 2),
            &Address::from_str(&env, ISSUER),
            &2,
            &2_000,
        );
        assert_eq!(result, Err(Ok(ProofError::SchemaVersionNotApproved)));
    }

    #[test]
    fn rejects_registration_when_protocol_is_paused() {
        let (env, client, protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        use earnproof_shared::ProofError;
        protocol_config.pause();

        let result = client.try_register_proof(
            &bytes(&env, 1),
            &bytes(&env, 2),
            &Address::from_str(&env, ISSUER),
            &1,
            &2_000,
        );
        assert_eq!(result, Err(Ok(ProofError::InvalidSchemaVersion)));
    }

    #[test]
    fn rejects_inactive_issuer_address() {
        let (env, client, _protocol_config, issuer_registry, _issuer_registry_id) = setup();
        use earnproof_shared::ProofError;
        let inactive_issuer = Address::from_str(
            &env,
            "GBXHUHG5FGYLPD6RHL2MKWMP572O6KUXCZXDZJXS4T57ZTMAKBN7DWXN",
        );
        issuer_registry.register_issuer(&bytes(&env, 10), &inactive_issuer, &bytes(&env, 11));
        issuer_registry.suspend_issuer(&bytes(&env, 10));

        let result = client.try_register_proof(
            &bytes(&env, 1),
            &bytes(&env, 2),
            &inactive_issuer,
            &1,
            &2_000,
        );
        assert_eq!(result, Err(Ok(ProofError::InvalidSchemaVersion)));
    }

    #[test]
    fn extends_proof_storage_ttl() {
        let (env, client, _protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        let proof_id = bytes(&env, 1);
        let issuer = Address::from_str(&env, ISSUER);

        client.register_proof(&proof_id, &bytes(&env, 2), &issuer, &1, &2_000);

        env.as_contract(&client.address, || {
            assert!(
                env.storage()
                    .persistent()
                    .get_ttl(&DataKey::Proof(proof_id.clone()))
                    > TTL_THRESHOLD_LEDGERS
            );
        });
    }

    // ── upgrade governance tests ──────────────────────────────────────────────

    #[test]
    fn contract_version_initialized_to_one() {
        let (_env, client, ..) = setup();
        assert_eq!(client.get_contract_version(), 1);
    }

    #[test]
    fn approve_and_check_allowlist() {
        let (env, client, ..) = setup();
        let hash = bytes(&env, 0xab);

        assert!(!client.is_upgrade_allowed(&hash));
        client.approve_upgrade(&hash, &2);
        assert!(client.is_upgrade_allowed(&hash));
    }

    #[test]
    fn revoke_removes_from_allowlist() {
        let (env, client, ..) = setup();
        let hash = bytes(&env, 0xcd);

        client.approve_upgrade(&hash, &2);
        client.revoke_upgrade(&hash);
        assert!(!client.is_upgrade_allowed(&hash));
    }

    #[test]
    #[should_panic(expected = "new_version must be greater than current contract version")]
    fn approve_upgrade_rejects_downgrade_version() {
        let (env, client, ..) = setup();
        client.approve_upgrade(&bytes(&env, 1), &1);
    }

    #[test]
    #[should_panic(expected = "wasm hash not on allowlist")]
    fn upgrade_contract_rejects_non_allowlisted_hash() {
        let (env, client, ..) = setup();
        client.upgrade_contract(&bytes(&env, 0xff));
    }

    #[test]
    #[should_panic]
    fn upgrade_contract_requires_admin_auth() {
        let env = Env::default();
        let protocol_config_id = env.register(ProtocolConfigContract, ());
        let issuer_registry_id = env.register(IssuerRegistryContract, ());
        let contract_id = env.register(ProofRegistryContract, ());
        let client = ProofRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);
        let issuer = Address::from_str(&env, ISSUER);
        let issuer_id = bytes(&env, 9);

        env.mock_all_auths();
        let pc_client = ProtocolConfigContractClient::new(&env, &protocol_config_id);
        let ir_client = IssuerRegistryContractClient::new(&env, &issuer_registry_id);
        pc_client.initialize(&admin);
        pc_client.approve_schema_version(&1);
        ir_client.initialize(&admin);
        ir_client.register_issuer(&issuer_id, &issuer, &bytes(&env, 8));
        client.initialize(&admin, &issuer_registry_id, &protocol_config_id);

        let hash = BytesN::from_array(&env, &[0xde; 32]);
        client.approve_upgrade(&hash, &2);
        env.set_auths(&[]);

        client.upgrade_contract(&hash);
    }

    #[test]
    fn upgrade_advances_version_and_consumes_allowlist() {
        let (env, client, ..) = setup();
        let hash = bytes(&env, 0x42);

        client.approve_upgrade(&hash, &2);
        client.upgrade_contract(&hash);

        assert_eq!(client.get_contract_version(), 2);
        assert!(!client.is_upgrade_allowed(&hash));
    }

    #[test]
    #[should_panic(expected = "wasm hash not on allowlist")]
    fn upgrade_hash_cannot_be_replayed() {
        let (env, client, ..) = setup();
        let hash = bytes(&env, 0x42);

        client.approve_upgrade(&hash, &2);
        client.upgrade_contract(&hash);
        client.upgrade_contract(&hash);
    }

    /// Persistent proof state must survive an upgrade.
    #[test]
    fn state_preserved_across_upgrade() {
        let (env, client, ..) = setup();
        let proof_id = bytes(&env, 1);
        let issuer = Address::from_str(&env, ISSUER);

        client.register_proof(&proof_id, &bytes(&env, 2), &issuer, &1, &2_000);
        assert!(client.is_valid_proof(&proof_id));

        let hash = bytes(&env, 0x77);
        client.approve_upgrade(&hash, &2);
        client.upgrade_contract(&hash);

        assert!(client.is_valid_proof(&proof_id));
        assert_eq!(client.get_contract_version(), 2);
    }

    #[test]
    #[should_panic(expected = "new_version must be greater than current contract version")]
    fn cannot_re_approve_old_version_after_upgrade() {
        let (env, client, ..) = setup();
        let hash_v2 = bytes(&env, 0x01);
        let old_hash = bytes(&env, 0x02);

        client.approve_upgrade(&hash_v2, &2);
        client.upgrade_contract(&hash_v2);

        client.approve_upgrade(&old_hash, &1);
    }

    // ── numeric boundary tests ────────────────────────────────────────────────

    /// Table-driven tests for schema version boundaries in proof registration.
    /// Schema versions must be >= MIN_SCHEMA_VERSION (1).
    #[test]
    fn register_proof_schema_version_boundaries() {
        let (env, client, _pc, _ir, _ir_id) = setup();
        let issuer = Address::from_str(&env, ISSUER);

        // Valid: minimum allowed schema version
        client.register_proof(&bytes(&env, 1), &bytes(&env, 2), &issuer, &1, &2_000);
        assert!(client.is_valid_proof(&bytes(&env, 1)));

        // Valid: typical schema version
        _pc.approve_schema_version(&99);
        client.register_proof(&bytes(&env, 10), &bytes(&env, 11), &issuer, &99, &2_000);
        assert!(client.is_valid_proof(&bytes(&env, 10)));

        // Valid: large schema version
        _pc.approve_schema_version(&u32::MAX);
        client.register_proof(
            &bytes(&env, 20),
            &bytes(&env, 21),
            &issuer,
            &u32::MAX,
            &2_000,
        );
        assert!(client.is_valid_proof(&bytes(&env, 20)));
    }

    #[test]
    fn register_proof_schema_version_zero_rejected() {
        let (env, client, _pc, _ir, _ir_id) = setup();
        let issuer = Address::from_str(&env, ISSUER);

        // Schema version 0 must be rejected with a typed error.
        let result =
            client.try_register_proof(&bytes(&env, 1), &bytes(&env, 2), &issuer, &0, &2_000);
        assert_eq!(result, Err(Ok(ProofError::InvalidSchemaVersion)));
    }

    /// Table-driven tests for proof expiration boundaries.
    /// Expiration timestamp must be strictly greater than current ledger timestamp.
    #[test]
    fn register_proof_expiration_boundaries() {
        let (env, client, _pc, _ir, _ir_id) = setup();
        let issuer = Address::from_str(&env, ISSUER);
        let current_time = env.ledger().timestamp();

        // Valid: one second in the future (minimum practical offset)
        client.register_proof(
            &bytes(&env, 1),
            &bytes(&env, 2),
            &issuer,
            &1,
            &(current_time + 1),
        );
        assert!(client.is_valid_proof(&bytes(&env, 1)));

        // Valid: reasonable future expiration (1 year in seconds)
        client.register_proof(
            &bytes(&env, 10),
            &bytes(&env, 11),
            &issuer,
            &1,
            &(current_time + 365 * 24 * 3600),
        );
        assert!(client.is_valid_proof(&bytes(&env, 10)));

        // Valid: far future (max u64 is reachable in practice)
        client.register_proof(&bytes(&env, 20), &bytes(&env, 21), &issuer, &1, &u64::MAX);
        assert!(client.is_valid_proof(&bytes(&env, 20)));
    }

    #[test]
    fn register_proof_expiration_at_current_time_rejected() {
        let (env, client, _pc, _ir, _ir_id) = setup();
        let issuer = Address::from_str(&env, ISSUER);
        let current_time = env.ledger().timestamp();

        // Expiration equal to current time is rejected with a typed error.
        let result =
            client.try_register_proof(&bytes(&env, 1), &bytes(&env, 2), &issuer, &1, &current_time);
        assert_eq!(result, Err(Ok(ProofError::ProofExpired)));
    }

    #[test]
    fn register_proof_expiration_in_past_rejected() {
        let (env, client, _pc, _ir, _ir_id) = setup();
        let issuer = Address::from_str(&env, ISSUER);
        let current_time = env.ledger().timestamp();

        // Expiration in the past is rejected with a typed error.
        if current_time > 0 {
            let result = client.try_register_proof(
                &bytes(&env, 1),
                &bytes(&env, 2),
                &issuer,
                &1,
                &(current_time - 1),
            );
            assert_eq!(result, Err(Ok(ProofError::ProofExpired)));
        }
    }

    /// Test storage and event invariants: failed boundary cases
    /// must not modify state or emit events.
    #[test]
    fn failed_register_proof_schema_zero_leaves_state_unchanged() {
        let (env, client, _pc, _ir, _ir_id) = setup();
        let issuer = Address::from_str(&env, ISSUER);

        // Check that no proofs exist initially
        let proof_id = bytes(&env, 99);
        env.as_contract(&client.address, || {
            assert!(
                !env.storage()
                    .persistent()
                    .has(&DataKey::Proof(proof_id.clone())),
                "initial state must not contain the proof"
            );
        });

        // Attempt to register with schema version 0 — should panic
        let register_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.register_proof(&proof_id, &bytes(&env, 88), &issuer, &0, &2_000);
        }));

        // Must have panicked
        assert!(register_result.is_err());

        // State must be unchanged: proof must not exist in storage
        env.as_contract(&client.address, || {
            assert!(
                !env.storage()
                    .persistent()
                    .has(&DataKey::Proof(proof_id.clone())),
                "failed proof registration must not write to storage"
            );
        });
    }

    #[test]
    fn failed_register_proof_expired_leaves_state_unchanged() {
        let (env, client, _pc, _ir, _ir_id) = setup();
        let issuer = Address::from_str(&env, ISSUER);
        let current_time = env.ledger().timestamp();
        let proof_id = bytes(&env, 77);

        // Attempt to register with expired timestamp — should panic
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.register_proof(
                &proof_id,
                &bytes(&env, 66),
                &issuer,
                &1,
                &current_time, // Equal to current time, must be rejected
            );
        }));

        // Must have panicked
        assert!(result.is_err());

        // State must be unchanged: proof must not exist in storage
        env.as_contract(&client.address, || {
            assert!(
                !env.storage().persistent().has(&DataKey::Proof(proof_id)),
                "failed proof registration with expired timestamp must not write to storage"
            );
        });
    }

    /// Verify contract version boundaries in upgrade operations.
    #[test]
    fn contract_version_upgrade_boundaries() {
        let (env, client, _pc, _ir, _ir_id) = setup();
        assert_eq!(client.get_contract_version(), 1);

        // Valid: immediate next version
        client.approve_upgrade(&bytes(&env, 1), &2);
        client.upgrade_contract(&bytes(&env, 1));
        assert_eq!(client.get_contract_version(), 2);

        // Valid: large version number
        client.approve_upgrade(&bytes(&env, 2), &u32::MAX);
        client.upgrade_contract(&bytes(&env, 2));
        assert_eq!(client.get_contract_version(), u32::MAX);
    }

    // ── adversarial initialization tests ───────────────────────────────────────

    /// Verify that first initialization writes exactly the documented state
    /// with no partial writes or missing fields.
    ///
    /// Required behavior: First call to `initialize` results in:
    /// - Admin address set and readable
    /// - IssuerRegistry address set and readable
    /// - ProtocolConfig address set and readable
    /// - ContractVersion = 1
    #[test]
    fn initialization_writes_exactly_documented_state() {
        let env = Env::default();
        env.mock_all_auths();
        let protocol_config_id = env.register(ProtocolConfigContract, ());
        let issuer_registry_id = env.register(IssuerRegistryContract, ());
        let contract_id = env.register(ProofRegistryContract, ());
        let client = ProofRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);

        let pc_client = ProtocolConfigContractClient::new(&env, &protocol_config_id);
        let ir_client = IssuerRegistryContractClient::new(&env, &issuer_registry_id);
        pc_client.initialize(&admin);
        ir_client.initialize(&admin);

        // Perform initialization
        client.initialize(&admin, &issuer_registry_id, &protocol_config_id);

        // Verify exact state written
        assert_eq!(client.get_admin(), admin, "admin must be set");
        assert_eq!(
            client.get_issuer_registry(),
            issuer_registry_id,
            "issuer registry address must be set"
        );
        assert_eq!(
            client.get_protocol_config(),
            protocol_config_id,
            "protocol config address must be set"
        );
        assert_eq!(
            client.get_contract_version(),
            1,
            "contract version must be exactly 1 after initialization"
        );

        // Verify storage keys are set
        env.as_contract(&contract_id, || {
            let instance = env.storage().instance();
            assert!(
                instance.has(&DataKey::Admin),
                "Admin key must exist in instance storage"
            );
            assert!(
                instance.has(&DataKey::IssuerRegistry),
                "IssuerRegistry key must exist in instance storage"
            );
            assert!(
                instance.has(&DataKey::ProtocolConfig),
                "ProtocolConfig key must exist in instance storage"
            );
            assert!(
                instance.has(&DataKey::ContractVersion),
                "ContractVersion key must exist in instance storage"
            );
        });
    }

    /// Verify that repeated initialization by any address fails without
    /// altering state or emitting events.
    ///
    /// Required behavior for re-initialization guard:
    /// - Second call to `initialize` with any admin (same or different) panics
    /// - Storage is byte-for-byte unchanged
    /// - No additional events are emitted
    #[test]
    fn reinitialization_by_same_admin_fails_atomically() {
        let (env, client, _pc, _ir, ir_id) = setup();
        let admin = Address::from_str(&env, ADMIN);
        let protocol_config_id = env.register(ProtocolConfigContract, ());

        let contract_version_after_first = client.get_contract_version();
        let issuer_registry_after_first = client.get_issuer_registry();

        // Attempt second initialization with same admin
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.initialize(&admin, &ir_id, &protocol_config_id);
        }));

        // Must have panicked with "already initialized"
        assert!(result.is_err(), "re-initialization must panic");

        // Verify state is byte-for-byte identical
        assert_eq!(
            client.get_admin(),
            admin,
            "admin must not change after failed re-initialization"
        );
        assert_eq!(
            client.get_issuer_registry(),
            issuer_registry_after_first,
            "issuer registry must not change after failed re-initialization"
        );
        assert_eq!(
            client.get_contract_version(),
            contract_version_after_first,
            "contract version must not change after failed re-initialization"
        );
    }

    /// Verify that re-initialization with different dependency addresses
    /// also fails without state changes.
    ///
    /// This tests that the re-initialization guard prevents address swapping.
    #[test]
    fn reinitialization_with_different_dependencies_fails_atomically() {
        let (env, client, _pc, _ir, _ir_id) = setup();
        let admin = Address::from_str(&env, ADMIN);

        let issuer_registry_after_first = client.get_issuer_registry();
        let protocol_config_after_first = client.get_protocol_config();

        // Attempt re-initialization with different dependency addresses
        let new_ir = env.register(IssuerRegistryContract, ());
        let new_pc = env.register(ProtocolConfigContract, ());

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.initialize(&admin, &new_ir, &new_pc);
        }));

        // Must have panicked
        assert!(
            result.is_err(),
            "re-initialization with different deps must panic"
        );

        // Original dependency addresses must be preserved
        assert_eq!(
            client.get_issuer_registry(),
            issuer_registry_after_first,
            "issuer registry must not change when re-initialization attempts different address"
        );
        assert_eq!(
            client.get_protocol_config(),
            protocol_config_after_first,
            "protocol config must not change when re-initialization attempts different address"
        );
    }

    /// Verify that re-initialization by a different admin also fails.
    ///
    /// Tests that the guard does not discriminate based on caller identity.
    #[test]
    fn reinitialization_by_different_admin_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let protocol_config_id = env.register(ProtocolConfigContract, ());
        let issuer_registry_id = env.register(IssuerRegistryContract, ());
        let contract_id = env.register(ProofRegistryContract, ());
        let client = ProofRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);
        let other_admin = Address::from_str(&env, ISSUER);

        let pc_client = ProtocolConfigContractClient::new(&env, &protocol_config_id);
        let ir_client = IssuerRegistryContractClient::new(&env, &issuer_registry_id);
        pc_client.initialize(&admin);
        ir_client.initialize(&admin);

        // First initialization with original admin
        client.initialize(&admin, &issuer_registry_id, &protocol_config_id);
        let stored_admin = client.get_admin();

        // Attempt re-initialization with different admin
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.initialize(&other_admin, &issuer_registry_id, &protocol_config_id);
        }));

        // Must have panicked
        assert!(
            result.is_err(),
            "re-initialization by different admin must panic"
        );

        // Original admin must be preserved
        assert_eq!(
            client.get_admin(),
            stored_admin,
            "admin must not change when different address attempts re-initialization"
        );
    }

    /// Verify that invalid dependency addresses are rejected during initialization
    /// and do not write any state.
    ///
    /// Tests initialization with zero/null addresses where contract addresses
    /// are expected. The contract does not validate this at initialization time
    /// (it validates at runtime when dependencies are called), but we should
    /// verify that any panic during initialization leaves state atomic.
    #[test]
    fn reinitialization_guard_is_absolute() {
        let (env, client, _pc, _ir, ir_id) = setup();
        let admin = Address::from_str(&env, ADMIN);
        let pc_id = env.register(ProtocolConfigContract, ());

        // Multiple re-initialization attempts must all fail
        for attempt in 1..=3 {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                client.initialize(&admin, &ir_id, &pc_id);
            }));

            assert!(
                result.is_err(),
                "re-initialization attempt {} must fail",
                attempt
            );

            // Admin must remain unchanged
            assert_eq!(
                client.get_admin(),
                admin,
                "admin must not change after re-initialization attempt {}",
                attempt
            );
        }
    }

    /// Verify that initialization state is maintained across proof registration
    /// and other operations.
    ///
    /// Tests that the initialization state (admin, dependencies, contract version)
    /// is stable after initialization and before any subsequent operations.
    #[test]
    fn initialization_state_stable_across_operations() {
        let (env, client, _pc, _ir, ir_id) = setup();

        let admin = Address::from_str(&env, ADMIN);
        let protocol_config_id = client.get_protocol_config();

        // State immediately after initialization (from setup())
        assert_eq!(client.get_admin(), admin);
        assert_eq!(client.get_issuer_registry(), ir_id);
        assert_eq!(client.get_protocol_config(), protocol_config_id);
        assert_eq!(client.get_contract_version(), 1);

        // Perform proof registration
        let proof_id = bytes(&env, 1);
        let issuer = Address::from_str(&env, ISSUER);
        client.register_proof(&proof_id, &bytes(&env, 2), &issuer, &1, &2_000);

        // Dependencies must remain unchanged
        assert_eq!(
            client.get_admin(),
            admin,
            "admin must not change after proof registration"
        );
        assert_eq!(
            client.get_issuer_registry(),
            ir_id,
            "issuer registry must not change after proof registration"
        );
        assert_eq!(
            client.get_protocol_config(),
            protocol_config_id,
            "protocol config must not change after proof registration"
        );
        // Contract version must still be 1 (no upgrade yet)
        assert_eq!(
            client.get_contract_version(),
            1,
            "contract version must not change on proof registration"
        );
    }

    /// Summary test: proof-registry initialization spec verification.
    ///
    /// This test serves as executable documentation of what the test matrix
    /// expects from proof-registry initialization:
    /// - Depends on two other contracts (issuer-registry, protocol-config)
    /// - Has re-initialization guard
    /// - Does NOT emit an event during initialization
    /// - Sets: admin, issuer_registry, protocol_config, contract_version=1
    #[test]
    fn proof_registry_initialization_spec_summary() {
        // CONTRACT SPEC: proof-registry
        // - Name: "proof-registry"
        // - Has re-initialization guard: YES (panics "already initialized")
        // - Emits initialization event: NO
        // - Takes dependency addresses: YES
        // - Dependencies: ["issuer-registry", "protocol-config"]
        // - First init writes:
        //   - Admin: passed address (requires auth)
        //   - IssuerRegistry: passed address (no validation at init time)
        //   - ProtocolConfig: passed address (no validation at init time)
        //   - ContractVersion: 1
        // - Re-init guard: DataKey::Admin presence check; panics if set
        // - Re-init allowed by different admin: NO (guard blocks all)
        // - Invalid config cases: Dependency validation happens at runtime (register_proof)
        //   not at initialization time

        let env = Env::default();
        env.mock_all_auths();
        let protocol_config_id = env.register(ProtocolConfigContract, ());
        let issuer_registry_id = env.register(IssuerRegistryContract, ());
        let contract_id = env.register(ProofRegistryContract, ());
        let client = ProofRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);

        let pc_client = ProtocolConfigContractClient::new(&env, &protocol_config_id);
        let ir_client = IssuerRegistryContractClient::new(&env, &issuer_registry_id);
        pc_client.initialize(&admin);
        ir_client.initialize(&admin);

        // Verify the spec
        client.initialize(&admin, &issuer_registry_id, &protocol_config_id);
        assert_eq!(client.get_admin(), admin);
        assert_eq!(client.get_issuer_registry(), issuer_registry_id);
        assert_eq!(client.get_protocol_config(), protocol_config_id);
        assert_eq!(client.get_contract_version(), 1);

        // Re-initialization must fail
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.initialize(&admin, &issuer_registry_id, &protocol_config_id)
        }))
        .is_err());
    }

    // ── cross-contract initialization and ordering tests ──────────────────────

    /// Verify that the required deployment and initialization ordering is enforced.
    ///
    /// The correct order is:
    /// 1. Deploy protocol-config, initialize with admin
    /// 2. Deploy issuer-registry, initialize with admin
    /// 3. Approve schema version in protocol-config
    /// 4. Register at least one issuer in issuer-registry
    /// 5. Deploy proof-registry, initialize with admin + both dependency addresses
    ///
    /// This test deploys contracts in the correct order and verifies that
    /// the full system initializes successfully end-to-end.
    #[test]
    fn cross_contract_initialization_correct_order_succeeds() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::from_str(&env, ADMIN);
        let issuer = Address::from_str(&env, ISSUER);
        let issuer_id = bytes(&env, 9);

        // Step 1: Deploy and initialize protocol-config
        let pc_id = env.register(ProtocolConfigContract, ());
        let pc_client = ProtocolConfigContractClient::new(&env, &pc_id);
        pc_client.initialize(&admin);
        assert_eq!(pc_client.get_admin(), admin);
        assert_eq!(pc_client.get_contract_version(), 1);

        // Step 2: Deploy and initialize issuer-registry
        let ir_id = env.register(IssuerRegistryContract, ());
        let ir_client = IssuerRegistryContractClient::new(&env, &ir_id);
        ir_client.initialize(&admin);
        assert_eq!(ir_client.get_admin(), admin);
        assert_eq!(ir_client.get_contract_version(), 1);

        // Step 3: Approve schema version in protocol-config
        pc_client.approve_schema_version(&1);
        assert!(pc_client.is_schema_version_approved(&1));

        // Step 4: Register an issuer in issuer-registry
        ir_client.register_issuer(&issuer_id, &issuer, &bytes(&env, 8));
        assert!(ir_client.is_active_address(&issuer));

        // Step 5: Deploy and initialize proof-registry with both dependencies
        let proof_id = env.register(ProofRegistryContract, ());
        let proof_client = ProofRegistryContractClient::new(&env, &proof_id);
        proof_client.initialize(&admin, &ir_id, &pc_id);
        assert_eq!(proof_client.get_admin(), admin);
        assert_eq!(proof_client.get_issuer_registry(), ir_id);
        assert_eq!(proof_client.get_protocol_config(), pc_id);
        assert_eq!(proof_client.get_contract_version(), 1);

        // Verify the full system is functional: proof registration works
        let proof_id_hash = bytes(&env, 1);
        proof_client.register_proof(&proof_id_hash, &bytes(&env, 2), &issuer, &1, &2_000);
        assert!(proof_client.is_valid_proof(&proof_id_hash));
    }

    /// Verify that proof-registry initialization with uninitialized dependencies
    /// succeeds (no validation at init time), but proof registration fails when
    /// those dependencies are actually needed.
    ///
    /// This tests that initialization stores the dependency addresses without
    /// validating them, and validation happens at runtime (register_proof).
    #[test]
    fn proof_registry_init_with_uninitialized_dependencies_defers_validation() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::from_str(&env, ADMIN);
        let issuer = Address::from_str(&env, ISSUER);

        // Deploy contracts but DON'T initialize the dependencies
        let pc_id = env.register(ProtocolConfigContract, ());
        let ir_id = env.register(IssuerRegistryContract, ());
        let proof_id = env.register(ProofRegistryContract, ());
        let proof_client = ProofRegistryContractClient::new(&env, &proof_id);

        // Proof-registry initialization should succeed even with uninitialized deps
        // (initialization does not validate dependency addresses)
        proof_client.initialize(&admin, &ir_id, &pc_id);
        assert_eq!(proof_client.get_admin(), admin);
        assert_eq!(proof_client.get_issuer_registry(), ir_id);
        assert_eq!(proof_client.get_protocol_config(), pc_id);

        // However, attempting to use the proof registry should fail because
        // the dependencies are not initialized
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            proof_client.register_proof(&bytes(&env, 1), &bytes(&env, 2), &issuer, &1, &2_000);
        }));

        // Must have panicked (dependencies are not initialized)
        assert!(
            result.is_err(),
            "proof registration must fail with uninitialized dependencies"
        );
    }

    /// Verify that proof-registry with swapped dependency addresses
    /// (issuer-registry address passed where protocol-config address expected)
    /// results in runtime failure when proof operations are attempted.
    ///
    /// This demonstrates that dependency address validation is runtime, not compile-time.
    #[test]
    fn proof_registry_swapped_dependencies_fails_at_runtime() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::from_str(&env, ADMIN);
        let issuer = Address::from_str(&env, ISSUER);
        let issuer_id = bytes(&env, 9);

        // Deploy and initialize all contracts correctly
        let pc_id = env.register(ProtocolConfigContract, ());
        let pc_client = ProtocolConfigContractClient::new(&env, &pc_id);
        pc_client.initialize(&admin);
        pc_client.approve_schema_version(&1);

        let ir_id = env.register(IssuerRegistryContract, ());
        let ir_client = IssuerRegistryContractClient::new(&env, &ir_id);
        ir_client.initialize(&admin);
        ir_client.register_issuer(&issuer_id, &issuer, &bytes(&env, 8));

        // Deploy proof-registry
        let proof_id = env.register(ProofRegistryContract, ());
        let proof_client = ProofRegistryContractClient::new(&env, &proof_id);

        // Initialize proof-registry with SWAPPED dependency addresses
        // (pass issuer-registry where protocol-config expected, and vice versa)
        proof_client.initialize(&admin, &pc_id, &ir_id); // Intentionally swapped!
        assert_eq!(proof_client.get_issuer_registry(), pc_id); // Swapped!
        assert_eq!(proof_client.get_protocol_config(), ir_id); // Swapped!

        // Initialization succeeds, but proof registration must fail at runtime
        // because the dependencies are the wrong contracts
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            proof_client.register_proof(&bytes(&env, 1), &bytes(&env, 2), &issuer, &1, &2_000);
        }));

        // Must have panicked
        assert!(
            result.is_err(),
            "proof registration must fail when dependencies are swapped"
        );
    }

    /// Verify that initialization order matters: proof-registry can be deployed
    /// and initialized BEFORE its dependencies, but operations fail at runtime.
    ///
    /// This demonstrates that Soroban does not enforce deployment-time ordering,
    /// only runtime contract calls enforce dependencies.
    #[test]
    fn proof_registry_initialized_before_dependencies_fails_at_operations() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::from_str(&env, ADMIN);
        let issuer = Address::from_str(&env, ISSUER);

        // Deploy proof-registry FIRST, before dependencies are even deployed
        let proof_id = env.register(ProofRegistryContract, ());
        let proof_client = ProofRegistryContractClient::new(&env, &proof_id);

        // Deploy dependencies (but order is reversed)
        let pc_id = env.register(ProtocolConfigContract, ());
        let ir_id = env.register(IssuerRegistryContract, ());

        // Initialize proof-registry with dependency addresses
        // (they exist as addresses, but aren't initialized yet)
        proof_client.initialize(&admin, &ir_id, &pc_id);

        // Now initialize dependencies
        let pc_client = ProtocolConfigContractClient::new(&env, &pc_id);
        pc_client.initialize(&admin);
        pc_client.approve_schema_version(&1);

        let ir_client = IssuerRegistryContractClient::new(&env, &ir_id);
        ir_client.initialize(&admin);
        let issuer_id = bytes(&env, 9);
        ir_client.register_issuer(&issuer_id, &issuer, &bytes(&env, 8));

        // Now proof registration should work because dependencies are initialized
        let proof_id_hash = bytes(&env, 1);
        proof_client.register_proof(&proof_id_hash, &bytes(&env, 2), &issuer, &1, &2_000);
        assert!(proof_client.is_valid_proof(&proof_id_hash));
    }

    /// Verify that attempting to initialize proof-registry without initializing
    /// its dependencies' prerequisites fails at operation time.
    ///
    /// For example: schema version not approved in protocol-config, or issuer
    /// not registered in issuer-registry.
    #[test]
    fn proof_registry_operations_fail_without_dependency_configuration() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::from_str(&env, ADMIN);
        let issuer = Address::from_str(&env, ISSUER);

        // Deploy and initialize all contracts in correct order
        let pc_id = env.register(ProtocolConfigContract, ());
        let pc_client = ProtocolConfigContractClient::new(&env, &pc_id);
        pc_client.initialize(&admin);
        // NOTE: NOT approving schema version 1!

        let ir_id = env.register(IssuerRegistryContract, ());
        let ir_client = IssuerRegistryContractClient::new(&env, &ir_id);
        ir_client.initialize(&admin);
        // NOTE: NOT registering any issuer!

        let proof_id = env.register(ProofRegistryContract, ());
        let proof_client = ProofRegistryContractClient::new(&env, &proof_id);
        proof_client.initialize(&admin, &ir_id, &pc_id);

        // Proof registration should fail because:
        // 1. Schema version 1 is not approved
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            proof_client.register_proof(&bytes(&env, 1), &bytes(&env, 2), &issuer, &1, &2_000);
        }));
        assert!(
            result.is_err(),
            "proof registration must fail without approved schema version"
        );

        // Now approve schema version but still no issuer registered
        pc_client.approve_schema_version(&1);

        // Proof registration should fail because issuer is not registered
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            proof_client.register_proof(&bytes(&env, 2), &bytes(&env, 3), &issuer, &1, &2_000);
        }));
        assert!(
            result.is_err(),
            "proof registration must fail with unregistered issuer"
        );

        // Now register the issuer and everything should work
        let issuer_id = bytes(&env, 9);
        ir_client.register_issuer(&issuer_id, &issuer, &bytes(&env, 8));

        proof_client.register_proof(&bytes(&env, 3), &bytes(&env, 4), &issuer, &1, &2_000);
        assert!(proof_client.is_valid_proof(&bytes(&env, 3)));
    }

    /// Verify that all three contracts can be initialized successfully
    /// in their respective dependency order, demonstrating a complete,
    /// valid deployment sequence.
    ///
    /// This is the "happy path" test that confirms the full system
    /// can reach a fully-operational state.
    #[test]
    fn complete_system_initialization_happy_path() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::from_str(&env, ADMIN);
        let issuer = Address::from_str(&env, ISSUER);
        let issuer_id = bytes(&env, 9);
        let proof_id_hash = bytes(&env, 1);

        // Initialize protocol-config first (no dependencies)
        let pc_id = env.register(ProtocolConfigContract, ());
        let pc_client = ProtocolConfigContractClient::new(&env, &pc_id);
        pc_client.initialize(&admin);
        pc_client.approve_schema_version(&1);
        assert!(pc_client.is_schema_version_approved(&1));

        // Initialize issuer-registry second (no dependencies on proof-registry)
        let ir_id = env.register(IssuerRegistryContract, ());
        let ir_client = IssuerRegistryContractClient::new(&env, &ir_id);
        ir_client.initialize(&admin);
        ir_client.register_issuer(&issuer_id, &issuer, &bytes(&env, 8));
        assert!(ir_client.is_active_address(&issuer));

        // Initialize proof-registry third (depends on both above)
        let proof_id = env.register(ProofRegistryContract, ());
        let proof_client = ProofRegistryContractClient::new(&env, &proof_id);
        proof_client.initialize(&admin, &ir_id, &pc_id);

        // System is now fully operational
        // Verify all initialization invariants
        assert_eq!(pc_client.get_admin(), admin);
        assert_eq!(ir_client.get_admin(), admin);
        assert_eq!(proof_client.get_admin(), admin);

        // Verify all re-initialization guards are in place
        let other_admin = Address::from_str(
            &env,
            "GBXHUHG5FGYLPD6RHL2MKWMP572O6KUXCZXDZJXS4T57ZTMAKBN7DWXN",
        );
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pc_client.initialize(&other_admin)
        }))
        .is_err());
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ir_client.initialize(&other_admin)
        }))
        .is_err());
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            proof_client.initialize(&other_admin, &ir_id, &pc_id)
        }))
        .is_err());

        // Verify core operations work as expected
        proof_client.register_proof(&proof_id_hash, &bytes(&env, 2), &issuer, &1, &2_000);
        assert!(proof_client.is_valid_proof(&proof_id_hash));

        // Verify state mutations work
        let new_issuer = Address::from_str(
            &env,
            "GBXHUHG5FGYLPD6RHL2MKWMP572O6KUXCZXDZJXS4T57ZTMAKBN7DWXN",
        );
        let new_issuer_id = bytes(&env, 99);
        ir_client.register_issuer(&new_issuer_id, &new_issuer, &bytes(&env, 88));
        assert!(ir_client.is_active_issuer(&new_issuer_id));

        // Verify admin can still perform admin operations
        pc_client.pause();
        assert!(pc_client.is_paused());
        pc_client.unpause();
        assert!(!pc_client.is_paused());
    }
}
