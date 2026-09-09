#![no_std]

//! Golden tests for the published contract error catalog.
//!
//! The catalog in `earnproof-shared` is only useful if it is true. Four
//! independent checks keep it that way:
//!
//! * `variants` - every variant of every error enum has a catalog entry with a
//!   matching code, and every catalog entry corresponds to a real variant. An
//!   undocumented error cannot be added.
//! * `golden` - the catalog matches a committed fixture line for line, so
//!   renumbering, reuse, or an addition shows up as a fixture diff in review.
//! * `documentation` - the tables in `docs/errors.md` match the catalog, so the
//!   published document cannot drift from the code.
//! * `observed` - the codes the contracts actually return on each failure path
//!   match the catalog, so the catalog describes deployed behaviour rather than
//!   intended behaviour.
//!
//! A fifth module, `client`, exercises the mapping a backend is expected to
//! perform, including for codes that did not exist when the client was written.

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod client;

#[cfg(test)]
mod documentation;

#[cfg(test)]
mod golden;

#[cfg(test)]
mod observed;

#[cfg(test)]
mod variants;
