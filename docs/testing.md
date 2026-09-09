# Testing

This document covers the test suites in this repository, and in detail the one that needs a procedure rather than just a command: the ledger snapshot regression fixtures.

## Running everything
This document describes how to run and extend the EarnProof test suite, and how
the bounded mutation-testing profile protects the authorization and validation
controls of the on-chain contracts.

## Prerequisites

- Rust toolchain from `rust-toolchain.toml` (`stable` + `rustfmt` + `clippy`).
- No running node, network, or local ledger is required for the Rust suites.

## Unit and integration tests

The workspace test suite spans the in-contract unit tests (`.src/lib.rs` under
`contracts/`) and the scenario-based integration suites under `tests/`:

| Suite | Crate | Covers |
| --- | --- | --- |
| `emergency-tests` | `tests/emergency` | pause matrix, admin rotation, revocation and recovery sequences |
| `cross-contract-tests` | `tests/cross-contract` | cross-contract boundaries, races, and references |
| `event-tests` | `tests/events` | event emission, ordering, and compatibility |
| `resource-budget-tests` | `tests/budgets` | Soroban resource (CPU/memory) budgets |

Run everything:

```bash
cargo test --workspace
```

Run a single suite:

```bash
cargo test -p emergency-tests
```

## Formatting, linting, and building

These are the checks CI runs on every pull request:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo build --workspace
python3 scripts/check-doc-links.py
python3 scripts/generate-reference.py --check
```

Every suite runs offline. No test reaches the network, reads a system clock, or depends on a fixture it did not commit.

## Suites

| Crate | What it covers |
|---|---|
| `contracts/*` | Unit tests inside each contract |
| `tests/cross-contract` | Atomicity and authorization across contract boundaries |
| `tests/emergency` | Pause, unpause, and admin rotation under adversarial ordering |
| `tests/events` | Event shape, ordering, and indexer compatibility |
| `tests/event-fixtures` | Golden event fixtures under `tests/fixtures/events` |
| `tests/encoding` | Hashing and encoding vectors under `tests/fixtures/encoding` |
| `tests/time` | Ledger-time boundary behaviour |
| `tests/budgets` | Resource budget regressions (run in release) |
| `tests/ledger-snapshots` | Serialized ledger state and emitted events per lifecycle state |

---

## Ledger snapshot regression fixtures

### What they are for

A test written through a contract client checks what a call **returns**. It does not check what the call **left on the ledger**. A change to how a record serializes, a field that silently changes type, a key that moves, or an event that gains a topic will pass every assertion about return values and still break every indexer reading the chain.

The snapshot suite closes that gap. For five representative lifecycle states it builds a small synthetic deployment, renders every contract-owned ledger entry and every emitted event into normalized text, and compares the result against a committed fixture in [`tests/fixtures/ledger-snapshots/`](../tests/fixtures/ledger-snapshots).

### The five states

| Fixture | State |
|---|---|
| `initialized.snap` | All three contracts provisioned, one schema version approved, no issuer or proof records |
| `active.snap` | One issuer registered, one valid proof registered |
| `paused.snap` | As `active`, with the protocol pause flag engaged |
| `revoked.snap` | Proof revoked and issuer revoked, both terminal states |
| `expired.snap` | As `active`, with ledger time advanced past the proof expiration; the record is untouched |

`expired` is the state a verifier meets most often and the one most likely to be mishandled, which is why it gets a fixture of its own even though its storage bytes are identical to `active`.

### What a fixture contains

Four sections:

- **`[ledger]`** - the sequence and timestamp the snapshot was taken at. Every scenario sets both explicitly, so this is deterministic context, and it is what separates `expired` from `active`.
- **`[storage]`** - every entry each contract holds in instance, persistent, and temporary storage, sorted so host iteration order cannot move a line.
- **`[events]`** - every event emitted over the whole scenario, in emission order, in the XDR form an indexer receives. Order is part of the contract with indexers and is never sorted away.
- **`[verdicts]`** - what the read-only entry points return in this state. Storage records what happened; verdicts record what a verifier concludes from it. A change that left the bytes intact but flipped a verdict is exactly what a storage-only snapshot would miss.

### What normalization does and does not remove

The renderer in [`tests/ledger-snapshots/src/render.rs`](../tests/ledger-snapshots/src/render.rs) is the only path into a fixture. Two rules govern it.

**It excludes host metadata.** Host object handles, live-until ledgers, entry sizes, and budget counters describe the environment a call ran in, not the state the contract produced. They move for reasons unrelated to compatibility, and a fixture that churned on every unrelated change would stop being read. Values are rendered from `ScVal`, the serialized form, which carries none of it.

**It hides no contract state.** Every field of every record is rendered in full. The one substitution is the address alias table, which replaces a generated address with the role it plays (`addr:issuer`, `addr:protocol-config`). The substitution is total: an address with no alias renders as `addr:<UNALIASED>`, and a test rejects any fixture containing it.

Three tests keep the normalization honest:

- `rendering_is_deterministic` builds each scenario twice and requires identical output. Anything that varied would be metadata the renderer failed to exclude.
- `the_normalization_hides_no_stored_entry` walks the real storage of every contract and requires each entry to appear in the rendered body.
- `each_state_is_distinguishable_from_the_others` requires all five fixtures to differ. Five identical fixtures would pass every other test and detect nothing.

### No real addresses or production identifiers

Every address is generated by the test environment and appears in fixtures only as an alias. Identifier hashes are repeated single bytes (`0x11`, `0x33`) chosen so a reader can tell them apart at a glance. A test scans every fixture for anything shaped like a Stellar strkey - 56 uppercase base32 characters starting `G` or `C` - and fails if it finds one, whether or not it was ever real.

### Updating a fixture

A snapshot diff is a compatibility signal. Sometimes it is the intended one, and then the change needs an explanation a reviewer can act on.

```powershell
./scripts/update-ledger-snapshots.ps1 -Reason "revoked_at is now recorded on admin revocation"
```

The script requires the reason, passes it to the guarded regenerator, and re-runs the snapshot tests. Each fixture header then carries:

```text
# scenario: revoked
# revision: 2
# reason: revoked_at is now recorded on admin revocation
# body-digest: <sha256 of the body>
```

The digest is what makes the reason binding. A body cannot change without the digest changing, the digest is written only by the regenerator, and the regenerator refuses to run without a reason of at least twenty characters. An intended update therefore reaches review as a body diff, a bumped revision, and a written explanation, in one commit. A fixture edited by hand fails `every_fixture_header_is_well_formed` instead.

Put the same explanation in the pull request description. The header is for whoever reads the fixture in a year; the description is for whoever reviews it today.

### When a snapshot test fails unexpectedly

Read the diff before regenerating. The suite is designed so that the diff itself tells you what changed:

| Diff | Likely cause |
|---|---|
| A `[storage]` line changed shape | A record's serialization changed. Check `packages/shared` types. |
| A `[storage]` line appeared or vanished | A storage key was added, removed, or moved between durability classes. |
| An `[events]` line changed | An event gained, lost, or reordered a field. This breaks indexers. |
| An `[events]` line moved | Emission order changed. This also breaks indexers. |
| A `[verdicts]` line flipped | A read-only entry point now reaches a different conclusion from the same state. |
| `addr:<UNALIASED>` appeared | A scenario gained an address without registering an alias for it. |

Regenerate only once you can say which of these it is, and why the new content is correct.
```

## Mutation testing

A green suite can still miss a removed `require_auth`, an inverted status check,
or a skipped expiry check. Mutation testing injects those bugs and asks the
suite to catch them.

The **bounded profile** in [`.cargo/mutants.toml`](../.cargo/mutants.toml)
limits mutation to `contracts/**/src/lib.rs` — the authorization and validation
branches listed in [`tests/mutation/README.md`](../tests/mutation/README.md) —
and runs the whole workspace suite against every mutant.

Run the profile and enforce the reviewed score:

```powershell
.\scripts\mutation-test.ps1
```

Prove the gate catches the seeded "removed authorization" and "inverted validity
check" mutations:

```powershell
.\scripts\mutation-test.ps1 -SelfTest
```

### Score policy

The reviewed policy is **zero missed mutants** in the bounded set. `cargo mutants`
exits non-zero when any mutant survives, and `mutation-test.ps1` additionally
computes the score from `mutants.out/outcomes.json` and fails if it drops below
`-MinimumScore` (default `100`).

When a mutant survives:

1. Inspect the exact change in `mutants.out/diff/`.
2. Add a test that asserts the correct behaviour at the right abstraction level
   (preferably through a public entry point), or
3. Explicitly justify the survivor in the PR — e.g. the mutant is behaviourally
   indistinguishable from the correct code.

### CI enforcement

The `mutation` job in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)
runs the bounded profile and the seeded-mutation self-test on a weekly schedule
and on `workflow_dispatch`, and uploads `mutants.out/` as an artifact. It is
deliberately not part of the fast PR loop so the normal contributor test cycle
stays quick.

### Reproducibility

- `cargo-mutants` is pinned to `27.1.0` and installed with `--locked`.
- `mutants.out/` and `mutants.out.old/` are git-ignored; `outcomes.json` records
  the per-mutant verdicts and the summary used to compute the score.
# Testing and coverage

What CI runs, what coverage measures, and what's deliberately excluded (#66).

## What CI runs

Every PR touching the workspace runs (`.github/workflows/ci.yml`, `contracts`
job): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`,
`cargo test --workspace`, `cargo build --workspace`.

A separate `coverage` job runs `cargo-llvm-cov` (pinned to `0.8.7` — see the
workflow file for why an unpinned install isn't used), generates both a
human-readable summary and a machine-readable JSON report, checks the
critical-path gates below, and uploads both files as a build artifact
(`coverage-report`, 30-day retention). No source code, secrets, or coverage
data leave GitHub Actions — nothing is uploaded to an external coverage
service.

## Running coverage locally

```bash
cargo install cargo-llvm-cov --version 0.8.7 --locked
cargo llvm-cov --workspace --summary-only
```

Add `--html` for a browsable per-line report, or `--json --output-path
coverage.json` to reproduce exactly what CI checks.

## Critical-path gates

`scripts/check-coverage-gates.py` reads the JSON report and fails if any
contract's **region** coverage — the metric that catches an untested branch a
line- or function-level number can miss — drops below its gate:

| File | Minimum | Measured when introduced |
|---|---|---|
| `contracts/issuer-registry/src/lib.rs` | 90.0% | 98.77% |
| `contracts/proof-registry/src/lib.rs` | 90.0% | 98.61% |
| `contracts/protocol-config/src/lib.rs` | 90.0% | 97.51% |

Minimums are set below the measured figure at introduction, not at it — an
unrelated one-line change to an already-well-tested branch shouldn't fail CI
over noise, while an actual regression (new logic added with no test
reaching it) still gets caught. Each contract's real coverage is already
close to complete: the specific 1-11 missed regions per file are almost
entirely defensive `unreachable!()`/internal-invariant branches that
`mock_all_auths()`-based tests can't reach without deliberately corrupting
storage first — see the `#[cfg(test)]` module in each contract for what
*is* covered (initialization, every state transition, every documented
error path, TTL renewal, event emission).

Authorization, validation, state-mutation, and error branches are exercised
directly: every `require_auth` call site has a test asserting the exact
address it demands (see `contracts/issuer-registry/src/lib.rs`'s
`revoke_issuer_rejects_a_valid_signature_from_the_issuer_itself` for the
pattern, and `tests/emergency/src/admin_rotation.rs`'s
`assert_authorized_by` helper for scoped-auth assertions against the real
invocation tree rather than a blanket `mock_all_auths()`), every documented
error variant in `earnproof_shared::{ContractError, IssuerError, ProofError}`
has a test that triggers it, and every state-changing entry point has a
paired "emits exactly one event" / "emits no event on rejection" assertion
(see [`docs/events.md`](./events.md)).

## What's excluded, and why

**`packages/shared/src/lib.rs`** is excluded from the gates above (not from
the report — it still appears in the summary, at whatever llvm-cov measures
for it). It contains only `#[contracterror]`/`#[contracttype]` declarations
and constants — zero `pub fn` or `impl` blocks of its own — so there is no
executable logic for llvm-cov to attribute coverage to directly. The
derive-macro-generated (de)serialization code these types produce *is*
exercised, but only observably through the three contracts that use them,
which is exactly what the three gates above already measure.

**Generated client bindings** (the `*Client` structs `#[contractimpl]`
generates) and **host glue code** are not separately gated for the same
reason: they have no logic of their own to regress independently of the
contract method they wrap.

## Test layout

- `contracts/*/src/lib.rs` — unit tests per contract, in an inline
  `#[cfg(test)] mod test`, using `env.mock_all_auths()` for the majority of
  behavioral coverage.
- `tests/cross-contract/` — cross-contract wiring, using scoped `MockAuth`/
  `MockAuthInvoke` (see [`docs/troubleshooting.md`](./troubleshooting.md#authorization-failures)
  for the authorization-scoping concepts these tests exercise).
- `tests/emergency/`, `tests/budgets/`, `tests/events/`, `tests/time/`,
  `tests/encoding/`, `tests/event-fixtures/` — one workspace member per
  concern, each independently coverage-measured but not separately gated
  (they exercise the same three contracts the gates above already cover).

## Fuzz testing

Fuzz testing exercises the robustness of type deserialization and input validation
by feeding arbitrary malformed input to the codec and entry points. The goal is
to ensure that malformed input is always rejected gracefully with a proper error
code, never causing a panic, trap, or undefined behavior.

### Fuzz targets

The `fuzz/` crate defines six fuzz targets that test shared types and entry point
parameter validation:

| Target | Covers |
| --- | --- |
| `fuzz_proof_record_decode` | ProofRecord XDR deserialization, field boundary validation (BytesN<32>, u32, u64 fields) |
| `fuzz_issuer_record_decode` | IssuerRecord XDR deserialization, field boundaries |
| `fuzz_issuer_status_decode` | IssuerStatus enum discriminant (0=Active, 1=Suspended, 2=Revoked); invalid discriminants should be rejected |
| `fuzz_proof_status_decode` | ProofStatus enum discriminant (0=Active, 1=Revoked); invalid discriminants should be rejected |
| `fuzz_address_validation` | `is_valid_principal_address()` and `is_zero_or_sentinel_address()` from `packages/shared/`; tests 56-char length, character set [A-Z2-7], sentinel rejection |
| `fuzz_entry_point_register_proof` | `register_proof()` entry point parameter validation: schema_version > 0, expires_at > now, Address format |

Each target:
- **Never panics or traps** on arbitrary input
- **Accepts valid input** according to documented invariants
- **Rejects invalid input** with deterministic error codes, not undefined behavior
- **Verifies no partial state mutations** occur on rejection (where observable)

### Quick smoke test (CI)

The CI `fuzz` job runs each target for 30 seconds with libFuzzer's default settings:

```bash
cargo fuzz run <target> -- -max_total_time=30
```

This catches obvious crashes or hangs with a recent corpus.

### Local fuzzing: quick run

Test a single target locally with the same 30-second smoke profile:

```bash
# Requires nightly toolchain and cargo-fuzz installed
rustup toolchain install nightly
cargo +nightly install cargo-fuzz

cd fuzz
cargo +nightly fuzz run fuzz_proof_record_decode -- -max_total_time=30
```

### Local fuzzing: deep run

To run a deeper/longer fuzz campaign that explores more of the input space:

```bash
# Run for 1 hour (3600 seconds), generating new inputs
cargo +nightly fuzz run fuzz_proof_record_decode -- -max_total_time=3600

# Or run with a specific number of iterations
cargo +nightly fuzz run fuzz_proof_record_decode -- -runs=100000
```

### Corpus and seeds

Each target has a seed corpus under `fuzz/corpus/<target_name>/`. Seed files
document:
- **Valid minimal cases** — smallest inputs that should be accepted
- **Boundary cases** — edge values (e.g., schema_version=0, expires_at=now)
- **Malformed cases** — inputs too short, invalid discriminants, wrong character sets

Seeds are checked into git to ensure reproducibility across CI runs and machines.

**To add a new seed** after fixing a discovered bug:

1. Locate the failing input in `fuzz/corpus/<target>/crash-*` or `fuzz/corpus/<target>/leak-*`
2. Minimize it with `cargo +nightly fuzz cmin <target>` (creates a smaller reproducer)
3. Rename the minimized input to a descriptive name (e.g., `boundary_schema_zero`) and commit it

### Reproduction and minimization

If the fuzz job finds a crash:

1. **Reproduce locally:**
   ```bash
   cargo +nightly fuzz run fuzz_proof_record_decode -- path/to/crash-file
   ```

2. **Minimize to the smallest failing input:**
   ```bash
   cargo +nightly fuzz cmin fuzz_proof_record_decode
   ```
   This creates `fuzz/artifacts/fuzz_proof_record_decode/` with minimized inputs.

3. **Inspect the crash:**
   Run under a debugger or add instrumentation to `fuzz/fuzz_targets/fuzz_proof_record_decode.rs`
   to understand what input triggered the failure.

4. **Add a regression test:**
   - If the crash reveals a bug in production code (`packages/shared/` or `contracts/*/src/lib.rs`),
     file an issue and fix the bug (ensure the fix is a proper error, not a silent ignore).
   - Create a seed corpus entry under `fuzz/corpus/<target>/` to prevent regression.

### Sanitizers and instrumentation

cargo-fuzz runs with Address Sanitizer (ASan) by default on nightly, which catches
memory unsafety. To disable sanitizers (if they cause false positives):

```bash
LLVM_PROFILE_FILE=/tmp/ignored cargo +nightly fuzz run fuzz_proof_record_decode \
  -- -max_total_time=30
```

### Corpus generation on CI

The CI fuzz job (`fuzz` in `.github/workflows/ci.yml`) runs as part of every PR.
If it discovers new interesting inputs (crashers or coverage improvements), they
are stored in `fuzz/corpus/` but **not automatically committed**. After investigating
and adding to the seed corpus manually, commit the regression cases so they stay
in CI.

### Constraints and design notes

- **No contract state corruption:** Fuzz targets for shared types (ProofRecord, IssuerStatus)
  test deserialization in isolation; they do not test state mutations. Entry-point
  targets (fuzz_entry_point_register_proof) construct a fresh Env and verify no
  storage changes occur on invalid input.
- **No production code changes:** If fuzzing uncovers a panic or undefined behavior
  in production code, it is a bug to be fixed. The fuzz test itself should not be
  weakened to make a bad implementation pass.
- **Bounded time/memory:** Fuzz targets skip inputs > 8KB to prevent memory exhaustion.
  This is a practical limit for type deserialization and entry-point testing; deeper
  fuzzing with larger inputs can be run manually if needed.
- **Deterministic corpus:** Seed files are versioned and deterministic; randomness
  comes only from libFuzzer's input generation, making results reproducible.

