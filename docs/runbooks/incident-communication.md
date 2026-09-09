# Incident communication, monitoring, and post-incident review

This runbook covers what `docs/emergency-operations.md` doesn't: what to
watch for while a pause is in force, who gets told what and when, and how an
incident gets closed out afterward. Read `docs/emergency-operations.md`
first — it's the on-chain specification (the pause switch, per-entry-point
behavior, the recovery procedure) that this document assumes and builds on
top of. This document adds nothing to that specification; it's operational
process around it.

Scope is the same as `docs/emergency-operations.md`: the on-chain surface
only. Backend alerting, on-call paging, and status-page tooling belong to
`earnproof-backend` and its own operational docs — this runbook describes
*what* to communicate and *when*, not the tooling used to send it.

## Triggers

`docs/threat-model.md` is the source of truth for what can go wrong; this is
the operational checklist for recognizing it in the moment.

| Trigger | What you'd observe | Where |
|---|---|---|
| Exploit in progress | Unexpected `ProofRegistered`/`IssuerRegistered` volume, or registrations from an address not matching known integrator patterns | Event stream (`docs/events.md`) |
| Bug causing incorrect state | `is_valid_proof`/`is_active_issuer` returning results that contradict the expected state for a known test credential | Manual verification via read calls |
| Coordinated attack (admin key targeting) | An `AdminChanged` event you did not initiate | Event stream |
| Reported by an integrator | A relying party reports proofs verifying (or failing to verify) unexpectedly | Support/integrator channel |
| Reported by a security researcher | Direct report, typically off-channel | See [Reporting a vulnerability](#reporting-a-vulnerability-external) |

None of these triggers is itself proof of an incident — `docs/emergency-operations.md`'s
"Assess" step exists precisely because reads stay available during a pause,
so assessment does not require committing to "this is real" before you've
looked.

## Monitoring during pause

Once paused, `register_proof` is contained but every read and every
containment operation (`suspend_issuer`, `revoke_issuer`,
`rotate_issuer_address`, `revoke_proof`, `admin_revoke_proof`) stays
available — see the per-entry-point table in `docs/emergency-operations.md`.
What to watch during that window:

- **`config_version` on `protocol-config`.** Monotonic across every
  privileged mutation on that contract. Poll it (`get_config_version`) at a
  fixed interval during the incident; a gap between two consecutive reads
  that's larger than the mutations *you* performed means something changed
  that isn't accounted for in your own action log.
- **`AdminChanged` events on any of the three contracts.** Each contract has
  an independent admin — a rotation on one contract says nothing about the
  others (`docs/security-review/README.md`, invariant I5). Watch all three,
  not just `protocol-config`.
- **`IssuerRevoked`/`IssuerSuspended` volume.** If your own containment
  actions are the only source of these events during the window, the count
  should match your action log exactly. A mismatch means another admin-key
  holder (or a compromised key) is acting concurrently.
- **`is_paused()` itself, periodically.** Confirms the pause is still in
  force — relevant if the admin key rotated mid-incident and you need to
  confirm the new holder hasn't (deliberately or by mistake) unpaused early.

None of the above requires new tooling beyond what `docs/emergency-operations.md`'s
recovery procedure already uses (`get_config_version`, `get_proof`,
`is_revoked`, `get_issuer`) — this section says *when* and *how often* to
call them during an active incident, not new commands.

## Investigation procedure

1. **Scope the blast radius first, fix nothing yet.** Using only read calls
   (`get_proof`, `get_issuer`, `is_valid_proof`, `is_active_issuer`),
   enumerate every affected proof id and issuer id. Do this before any
   `revoke_proof`/`suspend_issuer` call — a revocation while you're still
   discovering scope risks revoking the wrong set, or missing part of it
   because you stopped looking once you started acting.
2. **Correlate against events, not just current state.** Current state
   (`get_proof`) tells you where things stand now; the event stream
   (`tests/fixtures/events/`, `docs/events.md`) tells you the sequence that
   got there. For a suspected admin-key compromise in particular, the order
   of `AdminChanged` relative to other privileged events establishes when
   the attacker actually gained control, which bounds how far back you need
   to look.
3. **Note: `proof-registry` emits no events** (a documented gap, see
   `docs/events.md`). Its revocation timeline can only be reconstructed from
   storage reads at different points in time, not from the event stream —
   if you don't have periodic snapshots from before the incident, you
   cannot reconstruct exactly when a given proof's state changed, only that
   it changed.
4. **Cross-contract calls resolve toward containment, not away from it**
   (`docs/security-review/README.md#cross-contract-calls`) — `register_proof`
   requires *both* the pause check and the issuer-active check to pass, so
   a paused protocol with a still-active malicious issuer is still
   contained on the registration path specifically. This doesn't mean the
   issuer itself is contained — that still requires an explicit
   `suspend_issuer`/`revoke_issuer` call.
5. **Record every action taken, with its `config_version` before and
   after**, as you go — not from memory afterward. This log is both the
   input to [Communication](#communication) and the primary source for the
   [post-incident review](#post-incident-review-template).

## Escalation

| Time since trigger | Action |
|---|---|
| T+0 | Trigger recognized. Begin [Investigation](#investigation-procedure). Do not pause reflexively — confirm the trigger is real first, since `docs/emergency-operations.md`'s reads stay available regardless, so there is no cost to a few minutes of assessment before pausing. |
| T+15 min (or sooner if scope is already clear) | Decision: pause or not. If yes, execute `docs/emergency-operations.md`'s recovery procedure step 1 (`pause()`, confirm with `is_paused()`). |
| T+30 min from pause | Internal notification sent (see [Communication](#communication)) even if investigation is incomplete — do not wait for full scope before telling the team an incident is active. |
| T+2 hr from pause, and every 2 hr after while still paused | Status update to integrators, whether or not there's new information — "still investigating, still paused" is itself information a relying party needs. |
| Resolution | `unpause()` per `docs/emergency-operations.md`, followed by the closure notification (see [Communication](#communication)) and the [post-incident review](#post-incident-review-template), scheduled within 5 business days. |

These are defaults, not hard rules — a trivial false alarm resolved in
minutes doesn't need a 2-hour status-update cadence; a severe, actively
exploited issue may warrant notifying integrators before T+30 min. Judgment
overrides the table; the table exists so silence is never the default.

## Communication

### Internal (team)

Sent at T+30 min from pause (see [Escalation](#escalation)) and on every
material update after that (scope change, new containment action, decision
to unpause). Minimum content: trigger, current scope as currently
understood (explicitly marked "partial" if investigation is ongoing),
actions taken so far with their `config_version` values, and current pause
state.

### External (integrators and relying parties)

Integrators depend on `is_valid_proof`/`is_active_issuer` returning
consistent answers; a silent pause looks identical to an outage from their
side; they should not have to guess which one they're seeing. Minimum
content:

- That a pause is in effect and, at the level of detail appropriate for a
  public audience, why (e.g. "investigating irregular issuer activity" — no
  private income data, no un-redacted proof or issuer identifiers, since
  none of that reaches the chain in the first place and should not
  appear in a report either, per `docs/threat-model.md`'s data-minimization
  posture).
- What remains available during the pause: every read (`get_proof`,
  `is_valid_proof`, `is_active_issuer`, etc.) and every containment
  operation. Registration (`register_proof`) is the only contained
  operation — say so explicitly, since "paused" alone doesn't tell an
  integrator whether their existing verification flow still works.
- An estimated next update time (not a resolution ETA unless you actually
  have one) — matches the escalation table's update cadence.
- On resolution: what was revoked (proof ids and issuer ids affected, using
  the same hash identifiers the chain uses — never a raw income figure or
  off-chain identifier), and confirmation that `unpause()` has been called.

### Users (end holders of an affected credential)

Only relevant when a specific holder's proof was revoked as part of the
incident. Notify that holder that their credential was revoked, when, and
through what channel to request re-issuance if the revocation was
precautionary rather than for cause. This notification is a backend
responsibility (the contracts hold no contact information — see
`docs/security-review/README.md`'s asset list, "no asset in this list is
private data") but is listed here because the trigger for sending it is an
on-chain event this runbook's audience will observe first.

## Reporting a vulnerability (external)

If a report arrives from outside the team (a security researcher, an
integrator, a bug bounty submission), treat it as a [trigger](#triggers)
immediately — do not wait for internal reproduction before beginning
[Investigation](#investigation-procedure), since reads are free and
non-disruptive. See `SECURITY.md` at the repository root for the disclosure
channel and any bounty terms; this runbook governs the on-chain response
once a report is confirmed credible, not the disclosure process itself.

## Post-incident review template

Complete within 5 business days of `unpause()` (see [Escalation](#escalation)).
File as an entry under `docs/runbooks/post-incident/` (create the file as
`YYYY-MM-DD-short-description.md`); this template is the required section
list, not a fixed prose format.

```markdown
# Incident: <short description>

**Date:** <trigger date>
**Duration paused:** <pause timestamp> to <unpause timestamp>
**Severity:** <critical | high | medium | low, per docs/threat-model.md's framing>

## Trigger
What was observed, and how (which monitoring signal from
[Monitoring during pause](#monitoring-during-pause), or an external report).

## Timeline
Every action taken, with `config_version` before/after each privileged
mutation, and wall-clock timestamps. Pull directly from the investigation
log kept per step 5 of [Investigation](#investigation-procedure) — do not
reconstruct from memory.

## Scope
Which proof ids and issuer ids were affected, and how that scope was
determined (which read calls, which event correlation).

## Root cause
What actually happened. Distinguish "what we observed" (the trigger) from
"why it happened" (the root cause) — they are not always the same thing.

## What worked
Which parts of docs/emergency-operations.md's procedure and this runbook's
communication plan functioned as designed.

## What didn't
Gaps found during the actual incident that this runbook or
docs/emergency-operations.md didn't anticipate. This is the primary input
for updating either document — an incident that reveals no gap in either
document either means the documents are complete or means the review
wasn't thorough; assume the latter and look harder before concluding the
former.

## Follow-up actions
Concrete changes to contracts, docs, or process, each with an owner. Link
to a tracking issue for anything that isn't fixed same-day.
```

## Refresh checklist

This document is stale when any of the following happens:

- [ ] `docs/emergency-operations.md`'s pause-behavior tables change → the
      "what remains available" language in
      [Communication](#communication) and the assumptions in
      [Investigation](#investigation-procedure) step 4 need review.
- [ ] `proof-registry` gains events → step 3 of
      [Investigation](#investigation-procedure) (the "no events" gap) no
      longer applies, and the timeline-reconstruction limitation it
      describes should be removed.
- [ ] The admin-rotation authority rules in `docs/emergency-operations.md`
      change → [Monitoring during pause](#monitoring-during-pause)'s
      `AdminChanged`/`config_version` guidance needs review.
- [ ] `SECURITY.md`'s disclosure channel changes →
      [Reporting a vulnerability](#reporting-a-vulnerability-external)
      needs updating.
- [ ] A real incident's post-incident review identifies a gap in this
      document — see the "What didn't" section of the template above.
