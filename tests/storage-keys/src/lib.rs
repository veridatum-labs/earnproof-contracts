#![no_std]

//! Storage namespace and key-collision safety.
//!
//! Three separate questions are answered here, one per module:
//!
//! * `inventory` - is the namespace inventory in `earnproof-shared` internally
//!   consistent, and does it describe keys a Soroban host can actually address?
//! * `encoding` - can two distinct logical keys ever encode to the same ledger
//!   key, through arity, through payload confusion, or through a composite
//!   identifier that concatenates ambiguously?
//! * `lifetimes` - do the keys the contracts actually write land in the
//!   durability class the inventory claims, and nowhere else?
//!
//! Together they are a compatibility gate: adding a `DataKey` variant, renaming
//! one, changing its arity, or moving it between storage classes fails at least
//! one test in this crate.

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod support;

#[cfg(test)]
mod encoding;

#[cfg(test)]
mod inventory;

#[cfg(test)]
mod lifetimes;
