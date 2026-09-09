//! Golden fixture for the published codes.
//!
//! `tests/fixtures/errors/catalog.tsv` is the committed record of every code
//! that has been published. The catalog is rendered into the same shape and
//! compared line by line, so a renumbering, a reuse, or an undocumented
//! addition cannot reach `develop` without appearing as a fixture diff in
//! review.
//!
//! Updating the fixture is allowed. Updating it silently is what this test
//! prevents.

use earnproof_shared::ERROR_CATALOG;

const FIXTURE: &str = include_str!("../../fixtures/errors/catalog.tsv");

/// One fixture row per catalog entry, in code order.
fn rendered() -> std::vec::Vec<std::string::String> {
    ERROR_CATALOG
        .iter()
        .map(|entry| {
            std::format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                entry.code,
                entry.name,
                entry.enum_name,
                entry.domain.as_str(),
                entry.status.as_str(),
                entry.retry.as_str(),
                entry.http_status
            )
        })
        .collect()
}

/// Fixture rows with comments, blank lines, and line-ending noise removed.
fn fixture_rows() -> std::vec::Vec<std::string::String> {
    FIXTURE
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(std::string::String::from)
        .collect()
}

/// Rewrites the fixture from the catalog. Run with
/// `cargo test -p error-catalog-tests -- --ignored regenerate` after a
/// deliberate catalog change, then review the diff.
#[test]
#[ignore = "writes tests/fixtures/errors/catalog.tsv"]
fn regenerate_the_golden_fixture() {
    let mut contents = std::string::String::new();
    contents.push_str("# Golden record of every published contract error code.\n");
    contents
        .push_str("# Regenerate with: cargo test -p error-catalog-tests -- --ignored regenerate\n");
    contents.push_str("# code\tname\tenum\tdomain\tstatus\tretry\thttp_status\n");
    for row in rendered() {
        contents.push_str(&row);
        contents.push('\n');
    }
    std::fs::write("../fixtures/errors/catalog.tsv", contents).expect("cannot write fixture");
}

#[test]
fn the_catalog_matches_the_golden_fixture() {
    assert_eq!(rendered(), fixture_rows());
}

#[test]
fn the_fixture_has_no_duplicate_or_reused_codes() {
    // Read independently of the catalog, so that a fixture edited to match a
    // renumbered catalog still fails if it reuses a code.
    let mut codes: std::vec::Vec<u32> = fixture_rows()
        .iter()
        .map(|row| {
            row.split('\t')
                .next()
                .expect("fixture row is empty")
                .parse()
                .expect("fixture row does not start with a code")
        })
        .collect();
    let total = codes.len();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), total, "the fixture reuses a code");
}

#[test]
fn the_fixture_row_count_matches_the_catalog() {
    assert_eq!(fixture_rows().len(), ERROR_CATALOG.len());
}

// ---------------------------------------------------------------------------
// Disclosure
// ---------------------------------------------------------------------------

#[test]
fn no_client_message_can_carry_record_content() {
    // A Soroban contract error is a type and a code. There is no payload, so
    // there is nothing for a message to interpolate. These assertions keep it
    // that way at the catalog layer: a message with a placeholder would invite
    // a client to splice in an identifier that the error never carried.
    for entry in ERROR_CATALOG {
        for marker in ['{', '}', '%', '$'] {
            assert!(
                !entry.client_message.contains(marker),
                "{} has an interpolation marker in its client message",
                entry.name
            );
        }
    }
}

#[test]
fn authorization_failures_share_one_undifferentiated_code() {
    // Every authorization failure is code 20. A caller learns that it was not
    // authorized and nothing else: not which address would have been accepted,
    // not whether one exists.
    let authorization: std::vec::Vec<&str> = ERROR_CATALOG
        .iter()
        .filter(|entry| entry.http_status == 403 && entry.enum_name == "ContractError")
        .map(|entry| entry.name)
        .collect();
    assert_eq!(authorization, std::vec!["Unauthorized"]);
}

#[test]
fn client_messages_describe_the_condition_not_the_record() {
    // No message may name a field of a stored record. The on-chain record set
    // is public by design, so this is not about hiding existence; it is about
    // keeping the client-facing surface free of anything a future record could
    // widen into.
    // The list is private data, not field names: "Issuer address already
    // registered" names a category and carries no value, while a message that
    // reached for an amount or an email would have to have come from somewhere
    // the contracts never store.
    const FORBIDDEN: [&str; 7] = [
        "commitment",
        "metadata",
        "amount",
        "income",
        "salary",
        "email",
        "payload",
    ];

    for entry in ERROR_CATALOG {
        let message = entry.client_message.to_ascii_lowercase();
        for term in FORBIDDEN {
            assert!(
                !message.contains(term),
                "{} names {term} in its client message",
                entry.name
            );
        }
    }
}
