# Compatibility Testing Guide

This directory contains the golden test framework for EarnProof contract compatibility.

## What is tested

- **ABI compatibility**: Function names, signatures (no added/removed parameters)
- **Storage compatibility**: Storage keys and their types (no removed keys)
- **Error compatibility**: Error codes and names (no removed or reassigned codes)
- **Event compatibility**: Event types and fields (no removed events)

## Running the tests

```bash
# Run all compatibility tests
cargo test -p compatibility-tests

# Run a specific test
cargo test -p compatibility-tests protocol_config_abi_stable

# Run negative fixtures (which deliberately fail, demonstrating gate functionality)
cargo test -p compatibility-tests breaking_change
```

## Test structure

- `src/lib.rs`: Main test suite with golden artifact assertions
- `src/artifacts.rs`: Golden snapshots of ABI, storage, errors, and events
- `src/gates.rs`: Compatibility gate logic (breaking vs additive detection)
- `src/negative_fixtures.rs`: Synthetic breaking changes to demonstrate gate behavior

## Updating golden artifacts

When an intentional breaking change is approved (with governance sign-off), update the artifacts:

### Adding a new public function

In `src/artifacts.rs`, add the function name to the contract's `abi()` set:

```rust
pub mod protocol_config {
    pub fn abi() -> HashSet<&'static str> {
        [
            // ... existing functions ...
            "new_function",  // ADD HERE
        ]
        .iter()
        .cloned()
        .collect()
    }
}
```

### Adding a new storage key

Add the key name to the contract's `storage_keys()` set:

```rust
pub fn storage_keys() -> HashSet<&'static str> {
    ["Admin", "Paused", "ConfigVersion", "NewKey"]  // ADD HERE
        .iter()
        .cloned()
        .collect()
}
```

### Adding a new error code

Add the tuple `(code, name)` to the contract's `error_codes()` set:

```rust
pub fn error_codes() -> HashSet<(u32, &'static str)> {
    [
        // ... existing errors ...
        (99, "NewError"),  // ADD HERE
    ]
    .iter()
    .cloned()
    .collect()
}
```

### Adding a new event

Add the event name to the contract's `events()` set:

```rust
pub fn events() -> HashSet<&'static str> {
    [
        // ... existing events ...
        "NewEvent",  // ADD HERE
    ]
    .iter()
    .cloned()
    .collect()
}
```

Then re-run the tests to confirm they pass:

```bash
cargo test -p compatibility-tests
```

## CI integration

The compatibility tests run on every CI build as part of the standard test suite:

```bash
cargo test --workspace
```

A breaking change causes the build to fail with a report showing:
- Which contract changed
- What was added/removed/changed
- The compatibility classification (Unchanged/Additive/Semantic/Breaking)

Example failure output:

```
test protocol_config_abi_stable ... FAILED

assertion failed: abi.contains("removed_function")
```

## Negative fixtures

The `negative_fixtures` module contains tests that deliberately fail to prove the gates work:

- `breaking_change_removed_function_fails_abi_gate` — proves removed functions fail
- `additive_change_new_function_passes_abi_gate` — proves new functions pass
- `breaking_change_error_code_changed_fails_gate` — proves error code changes fail
- And more...

These tests document the expected gate behavior and serve as regression tests. They should
**always pass** (meaning the gates correctly identify breaking changes as breaking).

## Related documentation

- [Compatibility Policy](../../docs/compatibility.md) — full policy and change classification rules
- [Backend Integration](../../docs/backend-integration.md) — consumer expectations and error handling
- [Storage Model](../../docs/storage-model.md) — every DataKey, TTL, and privacy boundary
