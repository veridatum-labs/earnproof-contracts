# Resource budgets

This is the capacity-planning reference for CPU, memory, and WASM-size costs
of every public entry point across `protocol-config`, `issuer-registry`, and
`proof-registry`. It exists so an operator or integrator can answer "how much
of a transaction's budget does this call use" without running the test suite
themselves, and so a resource *regression* — an operation getting measurably
more expensive — is visible as a documentation diff, not just a CI failure.

The authoritative, enforced source is
[`tests/budgets/`](../tests/budgets) (`resource-budget-tests`), a workspace
member that measures every operation against the thresholds below and fails
`cargo test --workspace` if any is exceeded. This document restates those
thresholds for readers who want the numbers without reading Rust; the two
must be kept in sync, and `tests/budgets/` is the one to trust if they ever
disagree. See [`tests/budgets/README.md`](../tests/budgets/README.md) for the
full methodology (why 20% headroom, how to update a baseline, how the
regression-gate tests prove the gate itself works).

## Thresholds

CPU instructions and memory bytes below are the `_MAX` threshold constants in
`tests/budgets/src/lib.rs`, which already include ~20% headroom over the
baseline measurement each was set from. "Baseline (derived)" divides the
threshold back out (`threshold / 1.2`, rounded to the nearest 1,000) to give
an approximate typical cost — it is arithmetic on the committed threshold,
not a fresh measurement; see [Verification status](#verification-status).

### `protocol-config`

| Operation | CPU max | Baseline (derived) | Memory max | Baseline (derived) |
|---|---:|---:|---:|---:|
| `initialize` | 300,000 | ~250,000 | 100,000 | ~83,000 |
| `pause` / `unpause` | 200,000 | ~167,000 | 80,000 | ~67,000 |
| `approve_schema_version` | 250,000 | ~208,000 | 90,000 | ~75,000 |

`deprecate_schema_version` and `set_admin` are not yet covered by a budget
test — see [Coverage gaps](#coverage-gaps).

### `issuer-registry`

| Operation | CPU max | Baseline (derived) | Memory max | Baseline (derived) |
|---|---:|---:|---:|---:|
| `initialize` | 300,000 | ~250,000 | 100,000 | ~83,000 |
| `register_issuer` | 600,000 | ~500,000 | 200,000 | ~167,000 |
| `get_issuer` (lookup) | 150,000 | ~125,000 | 80,000 | ~67,000 |
| `update_issuer` | 400,000 | ~333,000 | 150,000 | ~125,000 |
| `suspend_issuer` | 400,000 | ~333,000 | 150,000 | ~125,000 |
| `revoke_issuer` | 400,000 | ~333,000 | 150,000 | ~125,000 |
| `rotate_issuer_address` | 500,000 | ~417,000 | 180,000 | ~150,000 |

`get_issuer_by_address`, `is_active_issuer`, and `is_active_address` are not
yet covered — see [Coverage gaps](#coverage-gaps).

### `proof-registry`

| Operation | CPU max | Baseline (derived) | Memory max | Baseline (derived) |
|---|---:|---:|---:|---:|
| `initialize` | 400,000 | ~333,000 | 120,000 | ~100,000 |
| `register_proof` (worst case: fresh cross-contract calls to both `issuer-registry` and `protocol-config`) | 800,000 | ~667,000 | 250,000 | ~208,000 |
| `get_proof` (lookup) | 150,000 | ~125,000 | 80,000 | ~67,000 |
| `revoke_proof` | 400,000 | ~333,000 | 150,000 | ~125,000 |
| `is_valid_proof` | 200,000 | ~167,000 | 100,000 | ~83,000 |

`admin_revoke_proof` and `is_revoked` are not yet covered — see
[Coverage gaps](#coverage-gaps).

`register_proof` is the only entry point in the protocol whose cost includes
two cross-contract calls in its critical path (`issuer-registry.is_active_address`
and `protocol-config.is_schema_version_approved`); its budget test exercises
that path rather than a synthetic single-contract call, which is why it
carries the highest threshold of any operation measured.

### WASM binary size

From [`scripts/measure-resources.ps1`](../scripts/measure-resources.ps1),
release-optimized `wasm32-unknown-unknown` builds, ~10% headroom over
baseline:

| Contract | Max size |
|---|---:|
| `protocol-config` | 5,000 bytes |
| `issuer-registry` | 8,000 bytes |
| `proof-registry` | 9,000 bytes |

This script is not currently run in CI (see `tests/budgets/README.md`); run
it locally with `./scripts/measure-resources.ps1` before a release if binary
size matters for the deployment.

## Soroban network limits, for context

Per-transaction resource limits on the Stellar network (not specific to this
protocol — see the
[Soroban resource documentation](https://soroban.stellar.org/docs/fundamentals-and-concepts/resource-limits-fees)
for the authoritative, current values):

| Resource | Approximate limit |
|---|---|
| CPU instructions | ~100,000,000 per transaction |
| Memory | ~40,960,000 bytes per transaction |
| Ledger entry size | ~64KB per entry |

Every threshold above consumes a small fraction of these limits even before
accounting for the 20% headroom, which leaves room for a caller to batch
multiple operations, or for the network's actual limits to tighten, without
an existing operation becoming unviable on its own.

## Coverage gaps

Not every public entry point has a budget test yet. The ones without one are
called out inline in each table above. `deprecate_schema_version` and
`get_issuer_by_address` in particular are cheap, read/write-analogous
operations to ones already covered (`approve_schema_version`, `get_issuer`),
so their cost is expected to be similar — but "expected to be similar" is not
the same guarantee a budget test provides, and none of them are gated in CI
today. Adding them is a natural follow-up, tracked by this issue's own
acceptance criteria rather than a separate issue.

## Verification status

The numbers in this document were **not** re-measured by running the test
suite for this change. `contracts/protocol-config`, `contracts/issuer-registry`,
and `contracts/proof-registry` all currently fail `cargo build` on `develop`
at the commit this document was written against (confirmed with
`cargo build -p protocol-config`, `-p issuer-registry`, and `-p proof-registry`
individually — each fails with unrelated compile errors, and the repository's
own CI is red on `develop` for the same reason). This is a pre-existing,
workspace-wide build break unrelated to documentation or the budget test
crate itself, and out of scope to fix here.

Because of that, the thresholds above are transcribed directly from the
committed constants in `tests/budgets/src/lib.rs` — real, reviewed values
already enforced by CI when the workspace builds — rather than freshly
measured. Once the build is restored, refresh this document by running:

```bash
cargo test -p resource-budget-tests -- --nocapture
```

and updating any threshold that has drifted from what is printed.

### A second, unrelated finding

While researching this document, a second, pre-existing test suite was found
at [`tests/resource-boundaries/`](../tests/resource-boundaries) with its own
worst-case and cross-contract-call coverage. It is not wired into this
workspace — there is no `Cargo.toml` for it, it is not listed in the root
`Cargo.toml`'s `[workspace] members`, and it imports
`earnproof_shared::{MAX_PROOF_ID_HASH_BYTES, MAX_COMMITMENT_HASH_BYTES}`,
constants that do not exist in `packages/shared/src/lib.rs`. It has never
compiled or run as part of `cargo test --workspace`. A separate file,
[`docs/resources.md`](resources.md), describes this same suite (as
`resource-boundaries` / 42 tests) and those same constants as complete,
tested, and verified, which does not match the current codebase. Both are
flagged for maintainer attention in this PR rather than fixed here — wiring
`tests/resource-boundaries/` in correctly is a larger, separate change (it
would need the missing constants defined with an actual rationale, or the
tests rewritten for the fixed-size `BytesN<32>` hash fields the contracts
actually use instead of the variable-length inputs the file assumes), on top
of a workspace that does not currently build.

## Related documentation

- [`tests/budgets/README.md`](../tests/budgets/README.md) — full methodology, how to update a baseline, CI integration
- [`docs/resources.md`](resources.md) — input size limits and validation guarantees (see the caveat above)
- [`docs/storage-model.md`](storage-model.md) — storage keys and TTL policy, another axis of per-operation cost
- [`docs/threat-model.md`](threat-model.md#t13-resource-exhaustion--griefing) — T13: Resource Exhaustion / Griefing, the threat these budgets bound
