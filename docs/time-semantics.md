# Ledger-Time Semantics

All time-sensitive behavior reads `Env::ledger().timestamp()`. Tests set this
value explicitly through the reusable `LedgerClock` fixture in
`tests/time/src/lib.rs`.

## Boundary Semantics

| Check | Expression | Inclusive? | Boundary |
|-------|-----------|------------|----------|
| Registration | `expires_at <= now` → reject | Exclusive lower | `expires_at` must be **strictly greater** than `now` |
| Validity | `now <= expires_at` → valid | Inclusive upper | Proof is valid **at** `expires_at`, invalid one tick after |
| Revocation | `revoked_at = now` | Exact match | Terminal; dominates both registration and validity checks |
| Schema approval | `approved == true` | State guard | Instantaneous — no time component |
| Protocol pause | `paused == true` | State guard | Instantaneous — no time component |

### Interval diagram (proof validity)

```
         registered                expires_at              expires_at + 1
              │                          │                          │
  ────────────┼──────────────────────────┼──────────────────────────┼────
    NOT VALID │        VALID (Active)    │   VALID (at boundary)    │ NOT VALID
  ────────────┼──────────────────────────┼──────────────────────────┼────
              │                          │                          │
         expires_at > now           now == expires_at           now > expires_at
         (rejected)                (accepted)                  (expired)
```

### Registration interval

Registration rejects `expires_at` in the closed interval `[0, now]`. The
accepted interval is `(now, u64::MAX]`. This is an **exclusive lower bound**:
the proof must expire strictly in the future.

### Validity interval

An Active proof is valid for `now` in the interval `[created_at, expires_at]`.
This is an **inclusive upper bound**: the proof remains valid at the exact
expiry timestamp and becomes invalid at `expires_at + 1`.

### Revocation

Revocation is terminal and time-independent. Once `status == Revoked`, the
proof is invalid regardless of the current timestamp or `expires_at` value.
`revoked_at` records the timestamp at the moment of revocation and is `0`
until that point.

### Schema deprecation

Schema deprecation is a **state guard**, not a time interval. It takes effect
immediately at the timestamp it is called. Existing proofs registered against
a now-deprecated schema remain valid; only new registrations are rejected.

## Edge Cases

- **`u64::MAX` as `expires_at`**: Valid. Registration performs no `now + interval`
  arithmetic, so there is no overflow. Clients must reject values that overflow
  their own interval calculations before submitting.
- **`0` as `expires_at`**: Rejected with `ProofExpired`. Zero is not in the
  future.
- **`expires_at == now`**: Rejected with `ProofExpired`. The lower bound is
  exclusive.
- **`expires_at == 1` with `now == 0`**: Accepted. `1 > 0`.

## Test Coverage

All tests are in `tests/time/src/lib.rs` and use the deterministic `LedgerClock`
fixture (no wall-clock time).

| Test | What it verifies |
|------|-----------------|
| `validity_is_inclusive_at_expiration_and_false_after` | Inclusive upper bound at `expires_at` |
| `registration_requires_strictly_future_expiration` | Exclusive lower bound: rejects `now-1`, `now`, `0` |
| `revocation_dominates_expiration` | Revoked proof is invalid even before natural expiry |
| `zero_schema_and_pause_are_deterministic_guards` | State guards, not time intervals |
| `maximum_timestamp_is_representable_without_interval_overflow` | `u64::MAX` does not overflow |
| `proof_valid_before_expiry` | Valid one tick before `expires_at` |
| `proof_valid_at_exact_expiry` | Valid exactly at `expires_at` |
| `proof_invalid_after_expiry` | Invalid one tick after `expires_at` |
| `deprecated_schema_rejects_new_registrations` | Deprecated schema blocks new proofs |
| `schema_deprecation_takes_effect_immediately` | Same-timestamp approve+deprecate |
| `proof_registered_before_deprecation_remains_valid` | Existing proofs survive deprecation |
| `zero_expires_at_rejected` | `0` is rejected |
| `one_is_valid_expires_at_when_now_is_zero` | `1` accepted when `now = 0` |
| `revocation_before_expiry_makes_invalid` | Pre-expiry revocation |
| `revoked_at_timestamp_is_correct` | `revoked_at` matches revocation time |
| `created_at_matches_registration_timestamp` | `created_at` matches registration time |
| `revoked_at_zero_until_revoked` | `revoked_at = 0` on fresh proof |
| `multiple_proofs_different_expiry_times` | Each proof expires at its own boundary |
| `revoked_proof_ignores_expiry_advance` | Revocation is terminal across time advancement |
