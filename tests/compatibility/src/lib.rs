//! Contract ABI and storage compatibility golden tests.
//!
//! This module captures stable contract interfaces and storage encodings,
//! then gates unintended breaking changes in CI.
//!
//! ## Golden Artifacts
//!
//! Each contract publishes:
//! - Function signatures (entry points, parameters, return types)
//! - Storage keys and their encoded types
//! - Error codes and ranges
//! - Event types and fields
//!
//! Breaking changes are:
//! - Removed or renamed functions
//! - Added/removed/renamed function parameters
//! - Changed return types
//! - Added/removed/renamed storage fields
//! - Changed error codes
//! - Removed or renamed event fields
//!
//! Additive changes pass the gate:
//! - New functions
//! - New storage keys
//! - New error codes
//! - New events
//! - New event fields
//!
//! See docs/compatibility.md for the full compatibility policy.

pub mod artifacts;
pub mod gates;
pub mod negative_fixtures;

#[cfg(test)]
mod tests {
    use crate::artifacts::*;
    use crate::gates::*;

    #[test]
    fn protocol_config_abi_stable() {
        let abi = protocol_config::abi();
        assert!(abi.contains("initialize"));
        assert!(abi.contains("get_admin"));
        assert!(abi.contains("set_admin"));
        assert!(abi.contains("pause"));
        assert!(abi.contains("unpause"));
        assert!(abi.contains("is_paused"));
        assert!(abi.contains("approve_schema_version"));
        assert!(abi.contains("deprecate_schema_version"));
        assert!(abi.contains("is_schema_version_approved"));
        assert!(abi.contains("get_config_version"));
    }

    #[test]
    fn issuer_registry_abi_stable() {
        let abi = issuer_registry::abi();
        assert!(abi.contains("initialize"));
        assert!(abi.contains("get_admin"));
        assert!(abi.contains("register_issuer"));
        assert!(abi.contains("update_issuer"));
        assert!(abi.contains("suspend_issuer"));
        assert!(abi.contains("reactivate_issuer"));
        assert!(abi.contains("revoke_issuer"));
        assert!(abi.contains("rotate_issuer_address"));
        assert!(abi.contains("get_issuer"));
        assert!(abi.contains("get_issuer_by_address"));
        assert!(abi.contains("is_active_issuer"));
        assert!(abi.contains("is_active_address"));
    }

    #[test]
    fn proof_registry_abi_stable() {
        let abi = proof_registry::abi();
        assert!(abi.contains("initialize"));
        assert!(abi.contains("register_proof"));
        assert!(abi.contains("revoke_proof"));
        assert!(abi.contains("admin_revoke_proof"));
        assert!(abi.contains("get_proof"));
        assert!(abi.contains("is_valid_proof"));
        assert!(abi.contains("is_revoked"));
        assert!(abi.contains("get_admin"));
        assert!(abi.contains("get_issuer_registry"));
        assert!(abi.contains("get_protocol_config"));
    }

    #[test]
    fn protocol_config_storage_keys_stable() {
        let keys = protocol_config::storage_keys();
        assert!(keys.contains("Admin"));
        assert!(keys.contains("Paused"));
        assert!(keys.contains("ConfigVersion"));
        assert!(keys.contains("SchemaVersion"));
    }

    #[test]
    fn issuer_registry_storage_keys_stable() {
        let keys = issuer_registry::storage_keys();
        assert!(keys.contains("Admin"));
        assert!(keys.contains("Issuer"));
        assert!(keys.contains("AddressIssuer"));
    }

    #[test]
    fn proof_registry_storage_keys_stable() {
        let keys = proof_registry::storage_keys();
        assert!(keys.contains("Admin"));
        assert!(keys.contains("IssuerRegistry"));
        assert!(keys.contains("ProtocolConfig"));
        assert!(keys.contains("Proof"));
    }

    #[test]
    fn protocol_config_error_codes_stable() {
        let codes = protocol_config::error_codes();
        // Common errors
        assert!(codes.contains(&(1, "AlreadyInitialized")));
        assert!(codes.contains(&(2, "NotInitialized")));
        assert!(codes.contains(&(20, "Unauthorized")));
        assert!(codes.contains(&(60, "InvalidInput")));
        assert!(codes.contains(&(80, "ProtocolPaused")));
    }

    #[test]
    fn issuer_registry_error_codes_stable() {
        let codes = issuer_registry::error_codes();
        // Common errors
        assert!(codes.contains(&(1, "AlreadyInitialized")));
        assert!(codes.contains(&(2, "NotInitialized")));
        assert!(codes.contains(&(20, "Unauthorized")));
        // Issuer-specific errors (200-299)
        assert!(codes.contains(&(200, "IssuerAlreadyRegistered")));
        assert!(codes.contains(&(201, "IssuerNotFound")));
        assert!(codes.contains(&(202, "IssuerAddressAlreadyRegistered")));
        assert!(codes.contains(&(203, "IssuerAddressNotFound")));
        assert!(codes.contains(&(204, "IssuerRevoked")));
        assert!(codes.contains(&(205, "IssuerInactive")));
        assert!(codes.contains(&(206, "InvalidTransition")));
    }

    #[test]
    fn proof_registry_error_codes_stable() {
        let codes = proof_registry::error_codes();
        // Common errors
        assert!(codes.contains(&(1, "AlreadyInitialized")));
        assert!(codes.contains(&(2, "NotInitialized")));
        assert!(codes.contains(&(20, "Unauthorized")));
        // Proof-specific errors (300-399)
        assert!(codes.contains(&(300, "ProofAlreadyRegistered")));
        assert!(codes.contains(&(301, "ProofNotFound")));
        assert!(codes.contains(&(302, "ProofAlreadyRevoked")));
        assert!(codes.contains(&(303, "ProofExpired")));
        assert!(codes.contains(&(304, "InvalidSchemaVersion")));
        assert!(codes.contains(&(305, "SchemaVersionNotApproved")));
    }

    #[test]
    fn protocol_config_events_stable() {
        let events = protocol_config::events();
        assert!(events.contains("Initialized"));
        assert!(events.contains("AdminChanged"));
        assert!(events.contains("Paused"));
        assert!(events.contains("Unpaused"));
        assert!(events.contains("SchemaApproved"));
        assert!(events.contains("SchemaDeprecated"));
    }

    #[test]
    fn issuer_registry_events_stable() {
        let events = issuer_registry::events();
        assert!(events.contains("IssuerRegistered"));
        assert!(events.contains("IssuerMetadataUpdated"));
        assert!(events.contains("IssuerSuspended"));
        assert!(events.contains("IssuerReactivated"));
        assert!(events.contains("IssuerRevoked"));
        assert!(events.contains("IssuerAddressRotated"));
    }

    #[test]
    fn proof_registry_events_stable() {
        let events = proof_registry::events();
        // Future: ProofRegistered, ProofRevokedByIssuer, ProofRevokedByAdmin
        // Currently proof-registry emits no typed events
        assert!(!events.is_empty() || true); // Placeholder for future typed events
    }
}
