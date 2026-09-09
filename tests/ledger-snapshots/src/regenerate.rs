//! The guarded fixture writer.
//!
//! Regenerating a snapshot is allowed. Regenerating one without saying why is
//! what this module prevents. The writer is `#[ignore]`d so it never runs in
//! CI, and it refuses to run at all unless `SNAPSHOT_REASON` explains the
//! change. Each rewritten fixture gets its revision bumped, the reason
//! recorded, and a digest over the new body, so the explanation and the bytes
//! it explains land in the same commit.
//!
//! ```text
//! scripts/update-ledger-snapshots.ps1 -Reason "revoked_at is now recorded on admin revocation"
//! ```

use super::scenarios::{build, SCENARIOS};
use super::snapshot::{digest, path, render};

const REASON_VARIABLE: &str = "SNAPSHOT_REASON";
const MINIMUM_REASON: usize = 20;

#[test]
#[ignore = "rewrites tests/fixtures/ledger-snapshots; requires SNAPSHOT_REASON"]
fn regenerate_fixtures() {
    let reason = std::env::var(REASON_VARIABLE).unwrap_or_default();
    let reason = std::string::String::from(reason.trim());
    assert!(
        reason.len() >= MINIMUM_REASON,
        "set {REASON_VARIABLE} to a compatibility explanation of at least \
         {MINIMUM_REASON} characters before regenerating. A fixture diff \
         without one cannot be reviewed."
    );
    assert!(
        !reason.contains('\n'),
        "{REASON_VARIABLE} must be a single line"
    );

    for scenario in SCENARIOS {
        let body = render(&build(scenario));
        let body = body.trim_end_matches('\n');
        let revision = previous_revision(scenario) + 1;

        let contents = std::format!(
            "# scenario: {scenario}\n# revision: {revision}\n# reason: {reason}\n# body-digest: {}\n\n{body}\n",
            digest(body)
        );
        std::fs::write(path(scenario), contents)
            .unwrap_or_else(|error| std::panic!("cannot write fixture for {scenario}: {error}"));
    }
}

/// Revision currently on disk, or zero for a fixture that does not exist yet.
fn previous_revision(scenario: &str) -> u32 {
    match std::fs::read_to_string(path(scenario)) {
        Ok(contents) => super::snapshot::parse(&contents).revision,
        Err(_) => 0,
    }
}
