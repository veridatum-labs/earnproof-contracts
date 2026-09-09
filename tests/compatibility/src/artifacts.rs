//! Golden artifacts: ABI, storage, error codes, and events for each contract.
//!
//! These values are snapshotted from the stable Rust toolchain at a specific
//! soroban-sdk version. Changes to the contract source or toolchain will be
//! caught if they alter the captured interfaces.

use std::collections::HashSet;

pub mod protocol_config {
    use super::*;

    /// Public entry points in protocol-config.
    pub fn abi() -> HashSet<&'static str> {
        [
            "initialize",
            "get_admin",
            "set_admin",
            "pause",
            "unpause",
            "is_paused",
            "approve_schema_version",
            "deprecate_schema_version",
            "is_schema_version_approved",
            "get_config_version",
        ]
        .iter()
        .cloned()
        .collect()
    }

    /// Instance and persistent storage keys.
    pub fn storage_keys() -> HashSet<&'static str> {
        ["Admin", "Paused", "ConfigVersion", "SchemaVersion"]
            .iter()
            .cloned()
            .collect()
    }

    /// Error codes: (u32, name).
    pub fn error_codes() -> HashSet<(u32, &'static str)> {
        [
            // Common errors (1-99)
            (1, "AlreadyInitialized"),
            (2, "NotInitialized"),
            (20, "Unauthorized"),
            (60, "InvalidInput"),
            (80, "ProtocolPaused"),
        ]
        .iter()
        .cloned()
        .collect()
    }

    /// Event types.
    pub fn events() -> HashSet<&'static str> {
        [
            "Initialized",
            "AdminChanged",
            "Paused",
            "Unpaused",
            "SchemaApproved",
            "SchemaDeprecated",
        ]
        .iter()
        .cloned()
        .collect()
    }
}

pub mod issuer_registry {
    use super::*;

    /// Public entry points in issuer-registry.
    pub fn abi() -> HashSet<&'static str> {
        [
            "initialize",
            "get_admin",
            "register_issuer",
            "update_issuer",
            "suspend_issuer",
            "reactivate_issuer",
            "revoke_issuer",
            "rotate_issuer_address",
            "get_issuer",
            "get_issuer_by_address",
            "is_active_issuer",
            "is_active_address",
        ]
        .iter()
        .cloned()
        .collect()
    }

    /// Persistent storage keys.
    pub fn storage_keys() -> HashSet<&'static str> {
        ["Admin", "Issuer", "AddressIssuer"]
            .iter()
            .cloned()
            .collect()
    }

    /// Error codes: (u32, name).
    pub fn error_codes() -> HashSet<(u32, &'static str)> {
        [
            // Common errors (1-99)
            (1, "AlreadyInitialized"),
            (2, "NotInitialized"),
            (20, "Unauthorized"),
            // Issuer-specific errors (200-299)
            (200, "IssuerAlreadyRegistered"),
            (201, "IssuerNotFound"),
            (202, "IssuerAddressAlreadyRegistered"),
            (203, "IssuerAddressNotFound"),
            (204, "IssuerRevoked"),
            (205, "IssuerInactive"),
            (206, "InvalidTransition"),
        ]
        .iter()
        .cloned()
        .collect()
    }

    /// Event types.
    pub fn events() -> HashSet<&'static str> {
        [
            "IssuerRegistered",
            "IssuerMetadataUpdated",
            "IssuerSuspended",
            "IssuerReactivated",
            "IssuerRevoked",
            "IssuerAddressRotated",
        ]
        .iter()
        .cloned()
        .collect()
    }
}

pub mod proof_registry {
    use super::*;

    /// Public entry points in proof-registry.
    pub fn abi() -> HashSet<&'static str> {
        [
            "initialize",
            "register_proof",
            "revoke_proof",
            "admin_revoke_proof",
            "get_proof",
            "is_valid_proof",
            "is_revoked",
            "get_admin",
            "get_issuer_registry",
            "get_protocol_config",
        ]
        .iter()
        .cloned()
        .collect()
    }

    /// Instance and persistent storage keys.
    pub fn storage_keys() -> HashSet<&'static str> {
        ["Admin", "IssuerRegistry", "ProtocolConfig", "Proof"]
            .iter()
            .cloned()
            .collect()
    }

    /// Error codes: (u32, name).
    pub fn error_codes() -> HashSet<(u32, &'static str)> {
        [
            // Common errors (1-99)
            (1, "AlreadyInitialized"),
            (2, "NotInitialized"),
            (20, "Unauthorized"),
            // Proof-specific errors (300-399)
            (300, "ProofAlreadyRegistered"),
            (301, "ProofNotFound"),
            (302, "ProofAlreadyRevoked"),
            (303, "ProofExpired"),
            (304, "InvalidSchemaVersion"),
            (305, "SchemaVersionNotApproved"),
        ]
        .iter()
        .cloned()
        .collect()
    }

    /// Event types.
    ///
    /// Note: proof-registry currently emits no typed events. This is captured
    /// for completeness and will be populated when typed events are added
    /// (see #35, #36).
    pub fn events() -> HashSet<&'static str> {
        [
            // Future events (not yet implemented):
            // "ProofRegistered",
            // "ProofRevokedByIssuer",
            // "ProofRevokedByAdmin",
        ]
        .iter()
        .cloned()
        .collect()
    }
}
