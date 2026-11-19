//! Invariant checkpoint receipt integration tests.
//!
//! This crate contains comprehensive tests for the post-upgrade invariant checkpoint
//! receipt recording system. Tests are organized into categories:
//!
//! - **Positive tests**: Happy path, successful operations
//! - **Negative tests**: Error cases, failures, blocking conditions
//! - **Boundary tests**: Edge values, ring buffer wrapping, min/max fields
//! - **Authorization tests**: Auth patterns, role enforcement
//! - **Regression tests**: Backward compatibility, side effects

#![no_std]
