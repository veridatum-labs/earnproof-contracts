//! Fixture format, header rules, and the comparison itself.
//!
//! A fixture is a header of `# key: value` lines followed by a body. The
//! header carries three fields:
//!
//! ```text
//! # scenario: active
//! # revision: 1
//! # reason: initial snapshot of the active lifecycle state
//! # body-digest: <sha256 of the body>
//! ```
//!
//! The digest is what makes the `reason` mean something. A body cannot be
//! changed without the digest changing, the digest is only written by the
//! guarded regenerator, and the regenerator refuses to run without a reason.
//! An intentional fixture update therefore arrives in review as a body diff, a
//! bumped revision, and a written explanation, together in one commit.

use sha2::{Digest, Sha256};
use soroban_sdk::testutils::storage::{Instance as _, Persistent as _, Temporary as _};
use soroban_sdk::{Address, Env, IntoVal, Map, Val};

use super::render::{event, value};
use super::scenarios::{build, Scenario, SCENARIOS};

pub const FIXTURE_DIR: &str = "../fixtures/ledger-snapshots";

// ---------------------------------------------------------------------------
// Rendering a scenario
// ---------------------------------------------------------------------------

/// Renders every contract-owned ledger entry and every emitted event.
///
/// Storage lines come first, sorted, so that host iteration order cannot move
/// them. Event lines follow in emission order, which is part of the contract
/// with indexers and must not be sorted away.
pub fn render(scenario: &Scenario) -> std::string::String {
    let mut out = std::string::String::new();

    // Ledger position is set explicitly by every scenario, so it is
    // deterministic context rather than host metadata. It is also what
    // separates a live proof from an expired one, which is otherwise the same
    // bytes on the ledger.
    out.push_str(&std::format!(
        "[ledger]\nsequence = {}\ntimestamp = {}\n\n",
        scenario.ledger.0,
        scenario.ledger.1
    ));

    out.push_str("[storage]\n");
    let mut lines = std::vec::Vec::new();
    for (alias, address) in &scenario.contracts {
        for (class, keys) in [
            ("instance", entries(&scenario.env, address, Class::Instance)),
            (
                "persistent",
                entries(&scenario.env, address, Class::Persistent),
            ),
            (
                "temporary",
                entries(&scenario.env, address, Class::Temporary),
            ),
        ] {
            for (key, val) in keys {
                lines.push(std::format!(
                    "{alias} {class} {} = {}",
                    value(&scenario.env, &scenario.aliases, &key),
                    value(&scenario.env, &scenario.aliases, &val)
                ));
            }
        }
    }
    lines.sort();
    for line in lines {
        out.push_str(&line);
        out.push('\n');
    }

    out.push_str("\n[events]\n");
    for emitted in scenario.events() {
        out.push_str(&event(&scenario.aliases, emitted));
        out.push('\n');
    }

    // Storage records what happened; these record what a verifier concludes
    // from it. A change that left the bytes intact but flipped a verdict is
    // exactly the kind of break a storage-only snapshot would miss.
    out.push_str("\n[verdicts]\n");
    for (call, verdict) in &scenario.verdicts {
        out.push_str(&std::format!("{call} = {verdict}\n"));
    }

    out
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum Class {
    Instance,
    Persistent,
    Temporary,
}

/// Entries owned by one contract in one durability class.
///
/// `persistent().all()` and `temporary().all()` return every entry of that
/// durability in the test ledger rather than only the current contract's, so
/// the unscoped set is partitioned with `has()`, which is contract-scoped.
fn entries(env: &Env, contract: &Address, class: Class) -> std::vec::Vec<(Val, Val)> {
    let all: Map<Val, Val> = env.as_contract(contract, || match class {
        Class::Instance => env.storage().instance().all(),
        Class::Persistent => env.storage().persistent().all(),
        Class::Temporary => env.storage().temporary().all(),
    });

    all.iter()
        .filter(|(key, _)| match class {
            Class::Instance => true,
            Class::Persistent => env.as_contract(contract, || env.storage().persistent().has(key)),
            Class::Temporary => env.as_contract(contract, || env.storage().temporary().has(key)),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Fixture format
// ---------------------------------------------------------------------------

pub struct Fixture {
    pub scenario: std::string::String,
    pub revision: u32,
    pub reason: std::string::String,
    pub body_digest: std::string::String,
    pub body: std::string::String,
}

pub fn digest(body: &str) -> std::string::String {
    let mut hasher = Sha256::new();
    hasher.update(body.replace("\r\n", "\n").as_bytes());
    let out = hasher.finalize();
    let mut hex = std::string::String::with_capacity(out.len() * 2);
    for byte in out {
        hex.push_str(&std::format!("{byte:02x}"));
    }
    hex
}

pub fn parse(contents: &str) -> Fixture {
    let contents = contents.replace("\r\n", "\n");
    let mut scenario = None;
    let mut revision = None;
    let mut reason = None;
    let mut body_digest = None;
    let mut body = std::string::String::new();
    let mut in_body = false;

    for line in contents.lines() {
        if !in_body && line.starts_with("# ") {
            let (key, val) = line[2..].split_once(':').expect("malformed header line");
            let val = val.trim();
            match key.trim() {
                "scenario" => scenario = Some(std::string::String::from(val)),
                "revision" => revision = Some(val.parse().expect("revision is not a number")),
                "reason" => reason = Some(std::string::String::from(val)),
                "body-digest" => body_digest = Some(std::string::String::from(val)),
                other => std::panic!("unknown header field {other}"),
            }
            continue;
        }
        if !in_body && line.trim().is_empty() {
            in_body = true;
            continue;
        }
        in_body = true;
        body.push_str(line);
        body.push('\n');
    }

    Fixture {
        scenario: scenario.expect("fixture has no scenario header"),
        revision: revision.expect("fixture has no revision header"),
        reason: reason.expect("fixture has no reason header"),
        body_digest: body_digest.expect("fixture has no body-digest header"),
        body: std::string::String::from(body.trim_end_matches('\n')),
    }
}

pub fn path(scenario: &str) -> std::string::String {
    std::format!("{FIXTURE_DIR}/{scenario}.snap")
}

pub fn read(scenario: &str) -> Fixture {
    let contents = std::fs::read_to_string(path(scenario))
        .unwrap_or_else(|error| std::panic!("cannot read fixture for {scenario}: {error}"));
    parse(&contents)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn every_scenario_matches_its_fixture() {
    for scenario in SCENARIOS {
        let fixture = read(scenario);
        let rendered = render(&build(scenario));
        assert_eq!(
            rendered.trim_end_matches('\n'),
            fixture.body,
            "{scenario} no longer matches its fixture. If the change is intended, \
             regenerate with scripts/update-ledger-snapshots.ps1 and explain why."
        );
    }
}

#[test]
fn every_fixture_header_is_well_formed() {
    for scenario in SCENARIOS {
        let fixture = read(scenario);
        assert_eq!(fixture.scenario, scenario);
        assert!(fixture.revision >= 1, "{scenario} has revision 0");
        assert!(
            fixture.reason.len() >= 20,
            "{scenario} has no meaningful reason for its current revision"
        );
        assert_eq!(
            fixture.body_digest,
            digest(&fixture.body),
            "{scenario} body was edited without regenerating; \
             a fixture change needs a bumped revision and a written reason"
        );
    }
}

#[test]
fn rendering_is_deterministic() {
    // Two independent builds of the same scenario must render identically.
    // Anything that varied between them would be host metadata that the
    // renderer failed to exclude.
    for scenario in SCENARIOS {
        assert_eq!(render(&build(scenario)), render(&build(scenario)));
    }
}

#[test]
fn fixtures_carry_no_addresses_or_production_identifiers() {
    for scenario in SCENARIOS {
        let fixture = read(scenario);

        assert!(
            !fixture.body.contains("<UNALIASED>"),
            "{scenario} contains an address with no alias"
        );

        // Stellar strkeys are uppercase base32 of length 56. Nothing that
        // shape may appear in a fixture, whether or not it was ever real.
        for token in fixture.body.split(|c: char| !c.is_ascii_alphanumeric()) {
            assert!(
                !(token.len() == 56
                    && token.starts_with(['G', 'C'])
                    && token
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())),
                "{scenario} contains what looks like a Stellar strkey: {token}"
            );
        }
    }
}

#[test]
fn every_scenario_has_a_fixture_and_every_fixture_has_a_scenario() {
    let mut on_disk: std::vec::Vec<std::string::String> = std::fs::read_dir(FIXTURE_DIR)
        .expect("fixture directory is missing")
        .map(|entry| {
            entry
                .expect("unreadable fixture directory entry")
                .file_name()
        })
        .filter_map(|name| {
            let name = name.to_string_lossy().into_owned();
            name.strip_suffix(".snap").map(std::string::String::from)
        })
        .collect();
    on_disk.sort();

    let mut expected: std::vec::Vec<std::string::String> = SCENARIOS
        .iter()
        .map(|scenario| std::string::String::from(*scenario))
        .collect();
    expected.sort();

    assert_eq!(on_disk, expected);
}

#[test]
fn the_normalization_hides_no_stored_entry() {
    // Every entry the contracts hold must appear in the rendered body. A
    // renderer that dropped an entry would produce a snapshot that passes while
    // the ledger diverges, which is the one failure mode this crate exists to
    // prevent.
    for scenario in SCENARIOS {
        let built = build(scenario);
        let rendered = render(&built);
        let mut count = 0;
        for (alias, address) in &built.contracts {
            for class in [Class::Instance, Class::Persistent, Class::Temporary] {
                for (key, _) in entries(&built.env, address, class) {
                    let key_text = value(&built.env, &built.aliases, &key);
                    assert!(
                        rendered.contains(&std::format!("{alias} "))
                            && rendered.contains(&key_text),
                        "{scenario}: {alias} entry {key_text} is missing from the snapshot"
                    );
                    count += 1;
                }
            }
        }
        assert!(count > 0, "{scenario} rendered no storage at all");
    }
}

#[test]
fn each_state_is_distinguishable_from_the_others() {
    // Five fixtures that happened to be identical would pass every test above
    // and detect nothing. Each lifecycle state must leave a different trace.
    let bodies: std::vec::Vec<std::string::String> =
        SCENARIOS.iter().map(|name| read(name).body).collect();

    for (index, body) in bodies.iter().enumerate() {
        for (other_index, other) in bodies.iter().enumerate().skip(index + 1) {
            assert_ne!(
                body, other,
                "{} and {} render identically",
                SCENARIOS[index], SCENARIOS[other_index]
            );
        }
    }
}

/// Keeps the `IntoVal` import honest for the `has()` calls above.
const _: fn() = || {
    fn assert_into_val<T: IntoVal<Env, Val>>() {}
    assert_into_val::<Val>();
};
