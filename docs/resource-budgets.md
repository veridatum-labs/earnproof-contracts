# Resource budgets

Soroban enforces a per-transaction CPU instruction ceiling and memory ceiling.
This document is the capacity-planning reference for how close each EarnProof
contract operation runs to those ceilings, and it is the specification that
`tests/budgets/` (package `resource-budget-tests`) executes: every threshold
below has a matching assertion, and the two must be changed together.

**A note before anything else — read this first.** At the time of writing,
`develop` does not compile (`cargo build --workspace` fails: a missing
`#[contractevent]` macro import in `issuer-registry`, an unclosed delimiter
syntax error, and a half-finished `Result`-based refactor of
`proof-registry::set_revoked` that hasn't been threaded through its callers —
all from a very recent merge). This document could not be verified against a
live measurement as a result. The numbers below are the threshold
**constants already committed** in `tests/budgets/src/lib.rs`, not a fresh
run's output. Once `develop` builds again, run
`cargo test -p resource-budget-tests -- --nocapture` and reconcile this
table against the actual printed `CPU=... Memory=...` lines before relying on
it for capacity planning.

There is also a second, unrelated document, `docs/resources.md`, that claims
"Status: COMPLETE" for a different, more ambitious resource-limits scheme
(`MAX_ISSUER_ID_HASH_BYTES` and similar constants in `packages/shared/src/lib.rs`,
a `tests/resource-boundaries/` suite run via `cargo test -p resource-boundaries`).
Neither the constants nor a `resource-boundaries` package actually exist in
this tree — `packages/shared/src/lib.rs` has no such constants, and
`tests/resource-boundaries/` is four loose `.rs` files with no `Cargo.toml`,
so `cargo test -p resource-boundaries` cannot resolve to anything. Treat
`docs/resources.md` as aspirational, not authoritative, until someone
reconciles it with what's actually in the tree. This document describes only
what's real: the `tests/budgets/` suite.

## What's measured

`tests/budgets/src/lib.rs` measures CPU instructions and memory bytes for one
representative invocation of each mutating and read entry point, using
`env.cost_estimate().budget()` after a `reset_unlimited()` call scoped to
just the operation under test (setup calls run before the reset, so their
cost isn't included). A threshold assertion fails the test — and CI — if
either metric exceeds its constant.

**What this does *not* measure.** Every scenario below uses fixed-size
inputs: 32-byte hashes (`BytesN<32>`) and simple primitives. None of the
three contracts currently accepts a genuinely variable-length input (a
`Bytes` or `String` whose size the caller controls) — every hash argument is
a compile-time-fixed `BytesN<32>`, so there is no "maximum metadata size"
scenario to construct today; the worst case *is* the only case. If a future
change adds a variable-length parameter, a new budget test using the largest
size the contract will accept (matching the pattern documented in
[Adding a new budget test](#adding-a-new-budget-test)) is required before
merging that change — the existing thresholds say nothing about it.

## Protocol config

| Operation | CPU max | Memory max |
|---|---|---|
| `initialize` | 300,000 | 100,000 bytes |
| `pause` | 200,000 | 80,000 bytes |
| `approve_schema_version` | 250,000 | 90,000 bytes |

## Issuer registry

| Operation | CPU max | Memory max |
|---|---|---|
| `initialize` | 300,000 | 100,000 bytes |
| `register_issuer` | 600,000 | 200,000 bytes |
| `get_issuer` | 150,000 | 80,000 bytes |
| `update_issuer` | 400,000 | 150,000 bytes |
| `suspend_issuer` | 400,000 | 150,000 bytes |
| `revoke_issuer` | 400,000 | 150,000 bytes |
| `rotate_issuer_address` | 500,000 | 180,000 bytes |

## Proof registry

| Operation | CPU max | Memory max |
|---|---|---|
| `initialize` | 400,000 | 120,000 bytes |
| `register_proof` | 800,000 | 250,000 bytes |
| `get_proof` | 150,000 | 80,000 bytes |
| `revoke_proof` | 400,000 | 150,000 bytes |
| `is_valid_proof` | 200,000 | 100,000 bytes |

`register_proof` is the only entry point that reaches across contracts —
`proof-registry` calls `protocol-config.is_paused()`,
`protocol-config.is_schema_version_approved()`, and
`issuer-registry.is_active_address()` before writing (see
`docs/security-review/README.md#cross-contract-calls`). Its 800,000/250,000
budget is the highest of any operation in the repository for exactly that
reason — it's the worst-case *call graph*, even though every argument is a
fixed 32-byte hash rather than a worst-case *payload*.

## Reading these against the network ceiling

Soroban publishes its current per-transaction resource limits at
[lab.stellar.org/network-limits](https://lab.stellar.org/network-limits) (a
live tool, not a static value — the network can change these limits, so
that page is the source of truth, not this document). The commonly cited
default CPU instruction ceiling per transaction is **100,000,000**; this
document does not independently pin down the current memory ceiling, so
check the live page for that figure rather than trusting a number here.

| Resource | Network default (verify against the live page) | Highest single-operation budget above | Headroom |
|---|---|---|---|
| CPU instructions | 100,000,000 | 800,000 (`register_proof`) | >99% |
| Memory | *(see lab.stellar.org/network-limits)* | 250,000 bytes (`register_proof`) | Not computed — see below |

Even without a pinned memory ceiling, the CPU comparison alone establishes
the headroom claim: every operation in this repository, even at its
threshold ceiling (which already includes the ~20% headroom the test file's
own comment describes — see `tests/budgets/src/lib.rs:6`), consumes under 1%
of a transaction's CPU budget. Memory usage (at most 250,000 bytes,
`register_proof`'s ceiling) is small enough in absolute terms — a quarter of
a megabyte — that it's very unlikely to be the binding constraint regardless
of the exact network memory ceiling, but that's a judgment call, not a
computed percentage; confirm against the live page if precision matters for
your use case. There is no realistic path to a single-transaction resource
exhaustion from calling one of these entry points once. The one caveat: a
caller batching many entry-point invocations
into a single transaction (e.g. many `register_proof` calls via a custom
multi-invoke transaction) could approach the ceiling well before any
individual threshold above would signal it — this repository's tests only
ever exercise one call per transaction, so multi-call batching is untested
and is an open gap, not a covered case.

## Capacity planning

At the current thresholds, a single ledger (whose own resource ceiling is
separate from, and larger than, a single transaction's) could in principle
admit on the order of 100+ `register_proof` calls before CPU becomes the
binding constraint, assuming even distribution and no other network activity
competing for the same ledger's capacity. This is a back-of-envelope bound
for planning discussions, not a throughput guarantee — real ledger capacity
is shared across all Soroban activity on the network, not reserved for this
protocol.

## Adding a new budget test

1. Add a threshold constant next to the existing ones in
   `tests/budgets/src/lib.rs`, following the `<CONTRACT>_<OPERATION>_CPU_MAX`
   / `_MEM_MAX` naming pattern already in use.
2. Write a `#[test]` function that sets up any required preconditions,
   calls `env.cost_estimate().budget().reset_unlimited()` immediately before
   the operation under test (not before setup calls — their cost must not be
   attributed to the measured operation), invokes the operation, then calls
   the existing `assert_budget` helper.
3. If the new operation accepts a variable-length input, use the *largest*
   size the contract will accept for that parameter — not a representative
   or average size. A budget test on a typical-size input says nothing
   about the worst case.
4. Add a row to the appropriate table above once you have a real measurement
   from a passing `cargo test -p resource-budget-tests -- --nocapture` run.
5. Set the threshold constant with the ~20% headroom convention already
   established in this file, not the bare measured value — a threshold set
   exactly at today's measurement fails the very next harmless refactor.

## Running the suite

```bash
cargo test -p resource-budget-tests -- --nocapture
```

The `--nocapture` flag is required to see the `CPU=... (max=...), Memory=...
(max=...)` lines each test prints; without it, a passing run reports nothing
beyond pass/fail.

`cargo test --workspace` runs this suite alongside everything else, but
without `--nocapture` you'll only see failures, not the underlying numbers —
useful for CI, not for reading off current measurements.
