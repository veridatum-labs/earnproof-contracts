# Emergency operations

This document defines what the EarnProof contracts do while the protocol is
paused, who may change that state, and what an operator can still rely on during
an incident. It is the specification that `tests/emergency/` executes: every rule
below has a corresponding assertion, and the two must be changed together.

Scope is the on-chain surface only. Backend containment (API rate limits, key
custody, webhook suspension) is out of scope here and belongs to
`earnproof-backend`.

## The pause switch

`protocol-config` owns a single boolean. `proof-registry` reads it over a
cross-contract call before admitting new proofs. `issuer-registry` does not read
it at all.

That asymmetry is deliberate. Pausing is meant to stop the protocol taking on
**new** obligations while leaving every tool an operator needs to contain the
existing ones. A pause that also froze revocation would remove the responder's
only lever at the moment they need it.

| Contract | Reads the pause flag |
|---|---|
| `contracts/protocol-config/src/lib.rs` | owns it |
| `contracts/proof-registry/src/lib.rs` | yes — in `register_proof` only |
| `contracts/issuer-registry/src/lib.rs` | no |

## Pause triggers

Pausing is a decision, not an automatic response — nothing in the contracts
triggers it. An operator should pause `protocol-config` when one of the
following is observed. Each maps to a threat in
[`threat-model.md`](threat-model.md); read the linked section for the full
attack description before acting.

| Trigger | Example | Threat model reference |
|---|---|---|
| **Exploit in production** | A caller is admitted through `register_proof` without a valid authorization, or a way is found to forge a proof/issuer relationship. | [T1: Authorization Bypass](threat-model.md#t1-authorization-bypass) |
| **Bug discovered post-deployment or post-upgrade** | Validation logic in `proof-registry` is found to accept malformed or expired input after a release. | Section 7, [`upgrades.md`](upgrades.md#7-emergency-pause-procedure) |
| **Coordinated or abnormal registration activity** | A sustained burst of `register_proof` calls inconsistent with normal traffic, suggesting griefing or an attempt to exhaust resources ahead of a fix. | [T13: Resource Exhaustion / Griefing](threat-model.md#t13-resource-exhaustion--griefing) |
| **Malicious or compromised issuer** | An issuer is registering proofs inconsistent with its attested behaviour. Prefer `suspend_issuer` or `revoke_issuer` (see [Recovery procedure](#recovery-procedure)) — pausing the whole protocol is not required for a single-issuer incident and should be reserved for cases where the issuer registry response is insufficient (e.g. registrations from an address not yet identified). | [T3: Malicious Issuer Behavior](threat-model.md#t3-malicious-issuer-behavior) |
| **Compromised admin key** | The current `protocol-config`, `issuer-registry`, or `proof-registry` admin key is suspected leaked or stolen. Pause first, then rotate (see [Recovery procedure](#recovery-procedure)) — a compromised key can also unpause, so pausing alone does not fully contain this trigger. | [T4: Compromised Admin Key](threat-model.md#t4-compromised-admin-key) |
| **Invalid proof or expiry logic accepted** | `is_valid_proof` is found to return `true` for a proof that should be expired or otherwise invalid. | [T9: Expired Proof Acceptance](threat-model.md#t9-expired-proof-acceptance) |

Pausing is not free: it is itself the subject of
[T5: Protocol Pause Abuse / Denial of Service](threat-model.md#t5-protocol-pause-abuse--denial-of-service).
An operator who pauses without one of the triggers above is creating the
condition that threat describes. When the trigger is ambiguous, prefer the
narrower containment tool (`suspend_issuer`, `revoke_issuer`,
`admin_revoke_proof`) over a full pause, and escalate per the
[communication plan](#communication-plan) below before pausing on suspicion
alone.

## Behaviour under pause

Every public entry point across the three contracts appears below. The
`tests/emergency/src/pause_matrix.rs` table mirrors this list exactly; a
mismatch in either direction fails `matrix_covers_every_public_entry_point`.

"Available" means the call behaves identically to an unpaused deployment.
"Contained" means it is rejected for as long as the pause is in force.

### `protocol-config`

| Entry point | Under pause | Why |
|---|---|---|
| `get_admin` | Available | Read. |
| `is_paused` | Available | Read. Operators and integrators must be able to observe containment. |
| `get_config_version` | Available | Read. Monotonic counter used to detect unaccounted changes. |
| `is_schema_version_approved` | Available | Read. |
| `approve_schema_version` | Available | Schema administration is a response tool, not a new obligation. |
| `deprecate_schema_version` | Available | Withdrawing a bad schema must not require unpausing first. |
| `set_admin` | Available | Authority handover must work during an incident. |
| `pause` | Available | Idempotent; a repeat must not toggle. |
| `unpause` | Available | The recovery path. |

### `issuer-registry`

| Entry point | Under pause | Why |
|---|---|---|
| `get_admin` | Available | Read. |
| `get_issuer` | Available | Read. |
| `get_issuer_by_address` | Available | Read. |
| `is_active_issuer` | Available | Read. Relying parties keep verifying. |
| `is_active_address` | Available | Read. |
| `register_issuer` | Available | The registry is not pause-gated; see the asymmetry note above. |
| `update_issuer` | Available | Metadata correction is a response tool. |
| `suspend_issuer` | Available | **Containment operation.** |
| `reactivate_issuer` | Available | Reversal of a suspension. |
| `revoke_issuer` | Available | **Containment operation.** Terminal. |
| `rotate_issuer_address` | Available | **Containment operation** — cuts a compromised key away from an identity. |

### `proof-registry`

| Entry point | Under pause | Why |
|---|---|---|
| `get_admin` | Available | Read. |
| `get_issuer_registry` | Available | Read. |
| `get_protocol_config` | Available | Read. |
| `get_proof` | Available | Read. Verification must not go dark during an incident. |
| `is_valid_proof` | Available | Read. |
| `is_revoked` | Available | Read. A relying party must still learn that a credential was revoked. |
| `register_proof` | **Contained** | The only operation that admits new obligations. |
| `revoke_proof` | Available | **Containment operation.** |
| `admin_revoke_proof` | Available | **Containment operation.** |

`initialize` is excluded from the table on all three contracts: it is
single-shot and unreachable on a live deployment. Its rejection is asserted in
`tests/emergency/src/sequences.rs`.

## Authority rules

1. Only the **current** administrator may change pause state. Authority is read
   from storage at call time, never cached, so a rotation takes effect
   immediately.
2. Rotation moves authority and nothing else. It does not clear the pause flag.
   An operator handing over control mid-incident does not silently re-open
   registration.
3. A rotated-out administrator retains nothing. They cannot pause, unpause, or
   rotate authority back to themselves.
4. Each contract holds its own administrator record. Rotating the
   `protocol-config` admin does not move authority over either registry — a
   single rotation contains less than it might appear to.
5. Re-initialisation is rejected on an initialised deployment. Were it not, an
   attacker could reset the administrator without emitting a rotation event,
   which would be the quietest available privilege escalation.

### Stranding

`set_admin` accepts any address. The contract cannot verify that a successor is
able to authorise, so an operator can strand a paused contract by rotating to an
address they do not control.

This is an **accepted risk**, not a mitigated one. What the contracts guarantee
is that the change is never silent: every rotation advances `config_version` and
emits `AdminChanged`, so a monitor watching either signal sees it. The
operational control is the handover procedure below, not a contract check.

## Recovery procedure

1. **Contain.** `pause()` on `protocol-config`. Confirm with `is_paused()`.
2. **Communicate (initial).** Post the incident notice from the
   [communication plan](#communication-plan) — do this before or immediately
   after containment, not after investigation.
3. **Assess.** Reads remain available. Use `get_proof`, `is_revoked`, and
   `get_issuer` to establish scope. No mutation is required to investigate.
4. **Revoke.** For each affected credential, `admin_revoke_proof`. For a
   compromised issuer, `suspend_issuer` (reversible) or `revoke_issuer`
   (terminal). For a compromised issuer *key* where the identity is sound,
   `rotate_issuer_address` — the old address stops resolving immediately.
5. **Withdraw schemas** if the incident is schema-borne:
   `deprecate_schema_version`. Callers holding transactions built before the
   incident will be rejected on retry, including after the pause lifts.
6. **Hand over**, if required: `set_admin`. Verify the successor with
   `get_admin()` and confirm `config_version` advanced *before* proceeding.
7. **Recover.** `unpause()`. Exactly one operation returns: `register_proof`.
   Everything revoked during the incident stays revoked.
8. **Communicate (resolution)** and schedule the
   [post-incident review](#post-incident-review).

### Commands

`pause`/`unpause` take no arguments; only the admin identity and the
`protocol-config` contract ID vary by network. This mirrors the invocation
style already used in
[`upgrades.md` §7](upgrades.md#7-emergency-pause-procedure) and the
deployment scripts.

**Testnet** (the only network this project currently deploys to — see
[`SECURITY.md`](../SECURITY.md#supported-scope)):

```powershell
stellar contract invoke `
  --source earnproof-deployer --network testnet `
  --auth-mode root --auto-sign `
  --id <protocol-config-contract-id> -- pause

# Verify:
stellar contract invoke `
  --source earnproof-deployer --network testnet `
  --id <protocol-config-contract-id> -- is_paused
# Expected output: true
```

Unpause is identical with `-- unpause` in place of `-- pause`.
`<protocol-config-contract-id>` is the `contracts.protocolConfig` value from
the deployment manifest for the target environment (see
[`scripts/deployment-manifest.example.json`](../scripts/deployment-manifest.example.json)).

**Mainnet:** not applicable today — mainnet deployment is explicitly out of
scope until the [Mainnet Release Gates](threat-model.md#mainnet-release-gates)
are satisfied (`SECURITY.md`). Once a mainnet deployment exists, the same
commands apply with `--network mainnet` and a source identity backed by the
multi-sig/hardware-wallet custody required by
[T4](threat-model.md#t4-compromised-admin-key); do not treat the
testnet procedure above as a template for a single-key mainnet pause.

### What does not come back

`unpause` restores registration and nothing else. A revoked issuer cannot
register proofs afterwards; a revoked proof stays revoked; a deprecated schema
stays deprecated. Recovery is not a rollback.

## Communication plan

Two audiences need different information at different points in the
incident. Both notices go out over the channel in
[`SECURITY.md`](../SECURITY.md#reporting-a-vulnerability)
(`security@veridatum.dev`) plus whatever status channel the maintainers
operate for the deployment in question; this document does not invent a
channel that doesn't exist elsewhere in the repo.

| Audience | Initial notice (at containment) | Resolution notice (at recovery) |
|---|---|---|
| **Users** (proof holders / relying parties) | That `register_proof` is temporarily paused, reads and revocation are unaffected, and no user action is required. | That registration has resumed, and — if applicable — that specific proofs or issuers were revoked and will not return with `unpause` (see [What does not come back](#what-does-not-come-back)). |
| **Integrators** (`earnproof-backend` and downstream callers, per [`backend-integration.md`](backend-integration.md)) | That `register_proof` will reject with `Error(WasmVm, InvalidAction)` and should not be retried in a loop; what remains available (all reads, `revoke_proof`, `admin_revoke_proof`). | That registration is available again, plus any schema deprecations (`deprecate_schema_version`) that change what a caller must submit going forward. |

### Timelines and escalation

These are operational targets, not contract-enforced guarantees — nothing in
the contracts times out an incident. Times are relative to trigger detection
(T+0):

1. **T+0 to T+15 min — Contain.** The person who observes the trigger pages
   the current `protocol-config` admin (or pauses themselves if they hold the
   key) per [Pause triggers](#pause-triggers). If the admin is unreachable,
   escalate to a second maintainer listed in
   [`MAINTAINERS.md`](../MAINTAINERS.md).
2. **T+15 min — Initial communication.** Notice goes out per the table
   above, even if the assessment is incomplete. Do not wait for a root cause
   to say "we are aware and have contained it."
3. **T+1 hour — Assessment checkpoint.** Scope of the incident (which
   issuers/proofs are affected) should be established using the read-only
   calls in step 3 of the [recovery procedure](#recovery-procedure). If not,
   escalate to all maintainers in `MAINTAINERS.md`.
4. **Recovery** happens when containment and revocation are verified
   complete — no fixed deadline, since `unpause`-ing before an exploit is
   understood re-opens the same trigger.
5. **Within 5 business days of recovery — Post-incident review**, per the
   template below, matching the two-business-day triage expectation in
   [`MAINTAINERS.md`](../MAINTAINERS.md).

## Post-incident review

Every incident that reaches step 1 of the recovery procedure (`pause()` is
actually called) gets a written review, filed as a repository issue and
linked from this section's history. Template:

```markdown
## Incident: <short title>

- **Trigger**: which row of [Pause triggers](#pause-triggers) applied
- **Detected**: <timestamp, UTC> / **Paused**: <timestamp, UTC>
- **Network**: testnet | mainnet
- **Scope**: affected proof IDs / issuer IDs (hashes, not identities —
  see [Evidence and privacy](#evidence-and-privacy))
- **Root cause**: what allowed the trigger condition
- **Actions taken**: pause / revoke / suspend / rotate / deprecate-schema /
  admin handover — list each call made, in order, with `config_version`
  before and after
- **Recovery**: `unpause` timestamp; confirmation that
  `register_proof` resumed correctly
- **What did not come back**: per [What does not come back](#what-does-not-come-back)
- **Communication sent**: links to the initial and resolution notices
- **Follow-up**: contract change, doc change, or new test required
  (link the tracking issue); note if `pause_matrix.rs` or `sequences.rs`
  need updating per the [refresh checklist](#refresh-checklist)
```

## Evidence and privacy

This is also the monitoring surface: while `protocol-config` is paused, watch
these same two sources for anything the operator did not initiate — that is
the signal that containment was incomplete.

Operators reconstruct an incident from two sources:

- **Events.** `Paused`, `Unpaused`, `AdminChanged`, `SchemaApproved`,
  `SchemaDeprecated` on `protocol-config`; the `Issuer*` events on
  `issuer-registry`. Versioned fixtures live in `tests/fixtures/events/`.
- **`config_version`.** Monotonic across every privileged `protocol-config`
  mutation. A gap is evidence of a change the operator has not accounted for.

Neither source carries private data. The contracts store only hashes —
`proof_id_hash`, `commitment_hash`, `metadata_hash` — never a wallet-linked
identifier, an amount, a memo, or an off-chain payload. Panic messages are fixed
strings (`"protocol is paused"`, `"proof already revoked"`) and never interpolate
call arguments.

One consequence is worth stating plainly: a cross-contract rejection surfaces to
the caller as `Error(WasmVm, InvalidAction)`, not as the underlying message. The
specific reason is visible in the diagnostic event log but is not part of the
returned error. Callers must not attempt to distinguish failure causes from the
error value alone.

`proof-registry` emits no events. Registration and revocation are observable
only through storage reads, which is a **known gap** for off-chain indexers
building an incident timeline.

## Test coverage

`tests/emergency/` (30 tests):

| Module | Covers |
|---|---|
| `harness.rs` | Three-contract deployment; synthetic fixtures only. |
| `pause_matrix.rs` | The table above, asserted in both directions — every entry point must also be reachable while unpaused, so a "contained" verdict is attributable to the pause and not to a pre-existing break. |
| `admin_rotation.rs` | Authority rules 1–4, containment-tool availability, issuer key rotation, terminal revocation. Uses recorded auth trees rather than rejection, since `mock_all_auths` admits every call. |
| `sequences.rs` | Every ordering of 2 and 3 operations from an 8-symbol alphabet (576 sequences) replayed against an independent model written from *this document*, plus repetition, conflicting pairs, stale callers, cross-contract disagreement, and no-partial-state on rejection. |

Run with `cargo test -p emergency-tests`, or `cargo test --workspace` for
everything.

## Refresh checklist

This document is stale when any of the following happens:

- [ ] A public entry point is added or removed → update the tables and
      `DOCUMENTED_ENTRY_POINTS` in `pause_matrix.rs`.
- [ ] A contract starts or stops reading the pause flag → update the asymmetry
      table and the affected rows.
- [ ] An authority rule changes → update "Authority rules" and `admin_rotation.rs`.
- [ ] An operation's acceptance conditions change → update `Model::apply` in
      `sequences.rs`, which is written from this document by design.
- [ ] `proof-registry` gains events → remove the known gap above.
- [ ] A new attack class is added to `threat-model.md` that involves the pause
      flag → add a row to [Pause triggers](#pause-triggers).
- [ ] A mainnet deployment goes live → replace the "not applicable today" note
      in [Commands](#commands) with the real mainnet contract ID source and
      custody procedure.
- [ ] The security contact, status channel, or maintainer escalation path
      changes → update [Communication plan](#communication-plan).
