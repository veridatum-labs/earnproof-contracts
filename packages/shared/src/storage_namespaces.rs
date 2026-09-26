// Storage key namespace catalog for the Earnproof protocol contracts.
//
// Every `DataKey` enum variant across every contract in this workspace MUST be
// inventoried here. The inventory is used by storage-key property tests to
// verify key encoding determinism, collision freedom, durability tiers, and
// namespace exclusivity across contracts.

/// Durability tier of a storage entry.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StorageClass {
    /// Stored in instance storage (moves with contract instance).
    Instance,
    /// Stored in persistent storage (rent-bearing, independent lifecycle).
    Persistent,
    /// Stored in temporary storage (short TTL, cleared on expiry).
    Temporary,
}

impl StorageClass {
    pub const fn as_str(&self) -> &'static str {
        match self {
            StorageClass::Instance => "instance",
            StorageClass::Persistent => "persistent",
            StorageClass::Temporary => "temporary",
        }
    }
}

/// One `DataKey` variant.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct StorageNamespace {
    /// Contract that owns this namespace.
    pub contract: &'static str,
    /// Name of the `DataKey` enum variant.
    pub namespace: &'static str,
    /// Number of payload arguments in the `DataKey` variant.
    ///
    /// Singletons are 0; per-record entries are 1; composite keys are 2+.
    pub arity: u32,
    /// Durability tier in Soroban storage.
    pub class: StorageClass,
    /// Type of value stored under this key (e.g. `"Address"`, `"IssuerRecord"`).
    pub value: &'static str,
    /// Human-readable party that creates or mutates entries under this key.
    pub owner: &'static str,
}

pub const CONTRACTS: [&str; 3] = ["issuer-registry", "proof-registry", "protocol-config"];

/// Inventory of every storage namespace across every contract, sorted by
/// `(contract, namespace)`.
///
/// Adding a row here is the second half of adding a storage key; the first is
/// adding the `DataKey` variant. Doing one without the other fails the tests in
/// `tests/storage-keys/`.
pub const STORAGE_NAMESPACES: [StorageNamespace; 56] = [
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "AddressIssuer",
        arity: 1,
        class: StorageClass::Persistent,
        value: "BytesN<32>",
        owner: "registry operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "AddressTtl",
        arity: 1,
        class: StorageClass::Persistent,
        value: "u32",
        owner: "keepalive operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "Admin",
        arity: 0,
        class: StorageClass::Instance,
        value: "Address",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "AllowedWasm",
        arity: 1,
        class: StorageClass::Persistent,
        value: "UpgradeApprovalRecord",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "ContractVersion",
        arity: 0,
        class: StorageClass::Instance,
        value: "u32",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "CurrentWasmHash",
        arity: 0,
        class: StorageClass::Instance,
        value: "BytesN<32>",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "Decommissioned",
        arity: 0,
        class: StorageClass::Instance,
        value: "bool",
        owner: "registry operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "InstanceLiveUntil",
        arity: 0,
        class: StorageClass::Instance,
        value: "u32",
        owner: "keepalive operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "Issuer",
        arity: 1,
        class: StorageClass::Persistent,
        value: "IssuerRecord",
        owner: "registry operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "IssuerTtl",
        arity: 1,
        class: StorageClass::Persistent,
        value: "u32",
        owner: "keepalive operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "LatestUpgradeReceipt",
        arity: 0,
        class: StorageClass::Instance,
        value: "UpgradeReceipt",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "MigrationStatus",
        arity: 0,
        class: StorageClass::Instance,
        value: "MigrationStatus",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "ScopedPause",
        arity: 1,
        class: StorageClass::Persistent,
        value: "bool",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "Successor",
        arity: 0,
        class: StorageClass::Instance,
        value: "Address",
        owner: "registry operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "UpgradeApproval",
        arity: 0,
        class: StorageClass::Instance,
        value: "UpgradeApproval",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "UpgradeApprovalMetadata",
        arity: 1,
        class: StorageClass::Persistent,
        value: "UpgradeApprovalMetadata",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "UpgradeHistory",
        arity: 1,
        class: StorageClass::Persistent,
        value: "UpgradeHistoryRecord",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "issuer-registry",
        namespace: "UpgradeHistoryCount",
        arity: 0,
        class: StorageClass::Instance,
        value: "u32",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "Admin",
        arity: 0,
        class: StorageClass::Instance,
        value: "Address",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "AllowedWasm",
        arity: 1,
        class: StorageClass::Persistent,
        value: "UpgradeApprovalRecord",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "ArchivedProof",
        arity: 1,
        class: StorageClass::Persistent,
        value: "ArchivedProofRecord",
        owner: "issuing party",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "ContractVersion",
        arity: 0,
        class: StorageClass::Instance,
        value: "u32",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "CurrentWasmHash",
        arity: 0,
        class: StorageClass::Instance,
        value: "BytesN<32>",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "InstanceLiveUntil",
        arity: 0,
        class: StorageClass::Instance,
        value: "u32",
        owner: "keepalive operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "IssuerRegistry",
        arity: 0,
        class: StorageClass::Instance,
        value: "Address",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "IssuerRegistryVersion",
        arity: 0,
        class: StorageClass::Instance,
        value: "u32",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "LatestUpgradeReceipt",
        arity: 0,
        class: StorageClass::Instance,
        value: "UpgradeReceipt",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "MigrationStatus",
        arity: 0,
        class: StorageClass::Instance,
        value: "MigrationStatus",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "Proof",
        arity: 1,
        class: StorageClass::Persistent,
        value: "ProofRecord",
        owner: "issuing party",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "ProofTtl",
        arity: 1,
        class: StorageClass::Persistent,
        value: "u32",
        owner: "keepalive operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "ProtocolConfig",
        arity: 0,
        class: StorageClass::Instance,
        value: "Address",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "ProtocolConfigVersion",
        arity: 0,
        class: StorageClass::Instance,
        value: "u32",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "ScopedPause",
        arity: 1,
        class: StorageClass::Persistent,
        value: "bool",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "UpgradeApproval",
        arity: 0,
        class: StorageClass::Instance,
        value: "UpgradeApproval",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "UpgradeApprovalMetadata",
        arity: 1,
        class: StorageClass::Persistent,
        value: "UpgradeApprovalMetadata",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "UpgradeHistory",
        arity: 1,
        class: StorageClass::Persistent,
        value: "UpgradeHistoryRecord",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "proof-registry",
        namespace: "UpgradeHistoryCount",
        arity: 0,
        class: StorageClass::Instance,
        value: "u32",
        owner: "deployment operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "Admin",
        arity: 0,
        class: StorageClass::Instance,
        value: "Address",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "AllowedWasm",
        arity: 1,
        class: StorageClass::Persistent,
        value: "UpgradeApprovalRecord",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "ConfigVersion",
        arity: 0,
        class: StorageClass::Instance,
        value: "u32",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "ContractVersion",
        arity: 0,
        class: StorageClass::Instance,
        value: "u32",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "CurrentPause",
        arity: 0,
        class: StorageClass::Instance,
        value: "PauseMetadata",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "CurrentWasmHash",
        arity: 0,
        class: StorageClass::Instance,
        value: "BytesN<32>",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "InstanceLiveUntil",
        arity: 0,
        class: StorageClass::Instance,
        value: "u32",
        owner: "keepalive operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "LatestPause",
        arity: 0,
        class: StorageClass::Instance,
        value: "PauseMetadata",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "LatestUpgradeReceipt",
        arity: 0,
        class: StorageClass::Instance,
        value: "UpgradeReceipt",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "MigrationStatus",
        arity: 0,
        class: StorageClass::Instance,
        value: "MigrationStatus",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "Paused",
        arity: 0,
        class: StorageClass::Instance,
        value: "bool",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "SchemaRecord",
        arity: 1,
        class: StorageClass::Persistent,
        value: "SchemaRecord",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "SchemaTtl",
        arity: 1,
        class: StorageClass::Persistent,
        value: "u32",
        owner: "keepalive operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "SchemaVersion",
        arity: 1,
        class: StorageClass::Persistent,
        value: "bool",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "ScopedPause",
        arity: 1,
        class: StorageClass::Persistent,
        value: "bool",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "UpgradeApproval",
        arity: 0,
        class: StorageClass::Instance,
        value: "UpgradeApproval",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "UpgradeApprovalMetadata",
        arity: 1,
        class: StorageClass::Persistent,
        value: "UpgradeApprovalMetadata",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "UpgradeHistory",
        arity: 1,
        class: StorageClass::Persistent,
        value: "UpgradeHistoryRecord",
        owner: "protocol operator",
    },
    StorageNamespace {
        contract: "protocol-config",
        namespace: "UpgradeHistoryCount",
        arity: 0,
        class: StorageClass::Instance,
        value: "u32",
        owner: "protocol operator",
    },
];

/// Looks up a namespace entry by contract and namespace name.
pub fn namespace(contract: &str, namespace: &str) -> Option<StorageNamespace> {
    STORAGE_NAMESPACES
        .into_iter()
        .find(|entry| entry.contract == contract && entry.namespace == namespace)
}

/// Iterates over every namespace declared for a contract.
pub fn namespaces_for<'a>(
    contract: &'a str,
    class: StorageClass,
) -> impl Iterator<Item = &'static str> + 'a {
    STORAGE_NAMESPACES
        .into_iter()
        .filter(move |entry| entry.contract == contract && entry.class == class)
        .map(|entry| entry.namespace)
}
