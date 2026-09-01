#![no_std]

//! Deterministic TTL expiration, restoration, and missing-state boundary tests.
//!
//! Every test in this crate drives the ledger sequence explicitly. No test
//! depends on wall-clock time, on the order in which other tests run, or on an
//! undocumented default. The ledger numbers used here are chosen so that each
//! assertion sits exactly on a documented boundary:
//!
//! * `TTL_EXTEND_TO_LEDGERS` ledgers after a write, the entry is on its final
//!   live ledger.
//! * One ledger later, the entry is archived and the host auto-restores it on
//!   the next access.
//! * `TTL_THRESHOLD_LEDGERS` ledgers of remaining life is the exact point at
//!   which an extension call starts having an effect.
//!
//! The operator-facing consequences of these boundaries are written up in
//! [`docs/storage-ttl.md`](../../../docs/storage-ttl.md).

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod fixture;

#[cfg(test)]
mod extension;

#[cfg(test)]
mod expiry;

#[cfg(test)]
mod restoration;

#[cfg(test)]
mod missing_state;
/// TTL Expiration and Restoration Boundary Tests
///
/// Comprehensive deterministic tests for TTL (Time-To-Live) boundaries across
/// protocol-config, issuer-registry, and proof-registry contracts.
///
/// Each test verifies behavior at exact TTL boundaries:
/// - pre_expiry: entry still valid (1 ledger before boundary)
/// - at_expiry: entry at boundary (inclusive validity)
/// - post_expiry: entry expired (1 ledger after boundary)
/// - restoration: entry restored after expiry
///
/// Soroban SDK 27.0.0 TTL Model:
/// - extend_ttl(threshold, extend_to): Extends TTL if current TTL <= threshold
/// - Expiry: entry is expired when ledger.sequence > expiry_ledger
/// - Boundary: inclusive (at expiry ledger = still valid)
mod harness;
mod issuer_registry_ttl;
mod proof_registry_ttl;
mod protocol_config_ttl;
