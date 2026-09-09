#![no_std]

//! Ledger snapshot regression fixtures.
//!
//! Unit tests written entirely through contract clients check what a call
//! returns. They do not check what the call *left on the ledger*, and they
//! cannot: a serialization change that alters how a record is stored, or an
//! event that quietly gains a field, passes every assertion about return
//! values while breaking every indexer downstream.
//!
//! This crate closes that gap. For five representative lifecycle states it
//! builds a small synthetic deployment, renders every contract-owned ledger
//! entry and every emitted event into a normalized text form, and compares the
//! result against a committed fixture.
//!
//! Module layout:
//!
//! * `render` - turns host values into stable, reviewable text and nothing
//!   else. This is the normalization boundary.
//! * `scenarios` - the five deployments, built deterministically.
//! * `snapshot` - fixture format, header rules, and the comparison.
//! * `regenerate` - the guarded writer used when a fixture change is intended.
//!
//! Everything here runs offline. No network, no external fixture source, and
//! no dependency on wall-clock time.

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod regenerate;

#[cfg(test)]
mod render;

#[cfg(test)]
mod scenarios;

#[cfg(test)]
mod snapshot;
