# Storage Namespaces and Key Safety

This document covers one question the [Storage Model](./storage-model.md) does not: whether the storage keys themselves are safe. It records who owns each namespace, why two keys can never collide, why each key sits in the durability class it does, and what a contributor has to do when adding a key.

Two companion documents cover the neighbouring concerns:

- [Storage Model](./storage-model.md) - what each key holds, which call writes it, and how long it lives.
- [Compatibility](./compatibility.md) - what constitutes a breaking change.

The inventory below is machine-readable in [`packages/shared/src/storage_namespaces.rs`](../packages/shared/src/storage_namespaces.rs) and enforced by [`tests/storage-keys/`](../tests/storage-keys/src/lib.rs).

---

## What a storage key is

Each contract addresses its ledger entries through a private `DataKey` enum. A `#[contracttype]` enum encodes as a vector whose first element is the variant name as a `Symbol` and whose remaining elements are the payload fields in declaration order:

| `DataKey` value | Encoded key |
|---|---|
| `Admin` | `[Symbol("Admin")]` |
| `Proof(h)` | `[Symbol("Proof"), BytesN<32>(h)]` |
| `SchemaVersion(3)` | `[Symbol("SchemaVersion"), u32(3)]` |

The full ledger key additionally carries the contract address and the durability class, so the same variant name in two contracts addresses two unrelated entries.

---

## Namespace inventory

Ownership is the role accountable for the entry existing and staying live. It is not an authorization statement: authorization is documented per call in the [invariant specifications](./invariants/README.md).

### `protocol-config`

| Namespace | Arity | Class | Value | Owner |
|---|---|---|---|---|
| `Admin` | 0 | instance | `Address` | protocol operator |
| `ConfigVersion` | 0 | instance | `u32` | protocol operator |
| `Paused` | 0 | instance | `bool` | protocol operator |
| `SchemaVersion` | 1 | persistent | `bool` | protocol operator |

### `issuer-registry`

| Namespace | Arity | Class | Value | Owner |
|---|---|---|---|---|
| `AddressIssuer` | 1 | persistent | `BytesN<32>` | registry operator |
| `Admin` | 0 | instance | `Address` | deployment operator |
| `Issuer` | 1 | persistent | `IssuerRecord` | registry operator |

### `proof-registry`

| Namespace | Arity | Class | Value | Owner |
|---|---|---|---|---|
| `Admin` | 0 | instance | `Address` | deployment operator |
| `IssuerRegistry` | 0 | instance | `Address` | deployment operator |
| `Proof` | 1 | persistent | `ProofRecord` | issuing party |
| `ProtocolConfig` | 0 | instance | `Address` | deployment operator |

`Admin` appears in all three contracts. That is safe, and deliberate: ledger keys are scoped by contract address, so the three entries are independent. A test asserts this explicitly so the uniqueness rule below is not misread as forbidding it.

---

## Why keys cannot collide

Three independent properties, each asserted in [`tests/storage-keys/src/encoding.rs`](../tests/storage-keys/src/encoding.rs) on serialized XDR bytes rather than on host value identity.

**Different discriminants cannot meet.** Two variants with different names differ in the first element of the encoded vector, whatever their payloads carry. `Issuer(h)` and `Proof(h)` hold the same 32 bytes and are still two keys.

**Arity is part of the key.** A vector of one element and a vector of two elements are different values even when the discriminant matches. A variant that gains a field is a new key, not a colliding one.

**There is no concatenation step.** This is the property that a hand-rolled scheme usually gets wrong. If a key were built by joining the namespace text to the payload text, then `"Ab" + "cd"` and `"Abcd" + nothing` would produce the same key. Under the vector encoding they do not, and the test asserts exactly that pair.

Beyond the encoding, two further properties are covered:

- **Payload sensitivity.** Identifiers differing in a single leading or trailing bit, and consecutive `u32` versions, produce distinct keys. Nothing truncates or folds the payload.
- **Contract scoping.** A key written by the proof registry is not visible from the issuer registry, asserted by reading the same key from both contract contexts.

---

## Why each key sits where it does

The three durability classes are not interchangeable, and the choice is enforced rather than merely described.

| Class | Lifetime | Recoverable after expiry | Used for |
|---|---|---|---|
| Instance | Shared with the contract instance | Yes, restored on access | Fixed singletons: admin, pause flag, config version, contract addresses |
| Persistent | Independent per key | Yes, restored on access | Per-record state: issuers, the reverse index, proofs, schema flags |
| Temporary | Independent per key | **No** | Nothing |

Two rules follow, and [`tests/storage-keys/src/inventory.rs`](../tests/storage-keys/src/inventory.rs) enforces both:

1. **Instance keys carry no payload.** A key with a payload is per-record by construction. Putting one in instance storage would grow a single ledger entry without bound and load all of it on every call that touches the contract.
2. **Persistent keys carry a payload.** A singleton in persistent storage gains an independent lifetime for no benefit, and it can then archive separately from the contract that reads it.

**Temporary storage is not used, anywhere.** It cannot be restored once expired, which makes it unusable for anything a verifier might need after an idle period. A test asserts that all three temporary stores are empty after a lifecycle that exercises every state-mutating entry point. A future key that genuinely needs temporary durability has to change that assertion, which forces the trade-off into review.

---

## The compatibility gate

[`tests/storage-keys/src/lifetimes.rs`](../tests/storage-keys/src/lifetimes.rs) runs a deployment through every state-mutating entry point on all three contracts, reads the namespaces that actually appear in each durability class, and compares them against the inventory. The comparison is exact in both directions, so each of the following fails the build:

| Change | How it fails |
|---|---|
| A new `DataKey` variant is written | The namespace is not in the inventory; the test panics naming the unknown discriminant |
| A variant is renamed | The old name is missing and the new name is unknown |
| A variant moves durability class | The namespace appears under the wrong class |
| A key is added to the inventory but never written | The expected namespace is absent from storage |
| Two variants in one contract share a name | The inventory self-consistency test fails |
| A namespace exceeds the 32-character symbol limit | The inventory self-consistency test fails |

The gate is deliberately noisy about additions. A new storage key is a compatibility event for indexers and for anyone restoring archived state, and it should not be possible to add one without a reviewer seeing it.

---

## Adding a storage key

1. Add the variant to the contract's `DataKey` enum. Pick a name that is unique within that contract and no longer than 32 characters. Reusing a name that a previous version used for something else is a breaking change; pick a new one.
2. Choose the durability class using the table above. If the answer is temporary, write down why the entry may be lost, and expect that to be the main subject of review.
3. Add the row to `STORAGE_NAMESPACES` in [`packages/shared/src/storage_namespaces.rs`](../packages/shared/src/storage_namespaces.rs), keeping the array sorted by contract and then by namespace. Record the value type and the owning role.
4. Extend the deployment in [`tests/storage-keys/src/support.rs`](../tests/storage-keys/src/support.rs) so the new key is actually written; an unwritten key fails the gate.
5. Add the key and its TTL policy to the [Storage Model](./storage-model.md).
6. Run `cargo test -p storage-key-tests`.

Steps 1 and 3 are two halves of one change. Doing either alone fails the gate, which is the point.
