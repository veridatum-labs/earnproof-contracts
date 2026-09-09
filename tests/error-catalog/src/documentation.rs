//! `docs/errors.md` is generated from the catalog.
//!
//! The published document carries a block delimited by
//! `<!-- BEGIN GENERATED: do not edit. -->` and `<!-- END GENERATED -->`, in
//! the same style as `docs/reference/api.md`. This module renders that block
//! from the catalog and asserts the document matches it, so the code and the
//! document cannot drift.
//!
//! To regenerate after a catalog change: run this test, copy the rendered block
//! from the failure output, and replace the delimited region in the document.

use earnproof_shared::ERROR_CATALOG;

const DOCUMENT: &str = include_str!("../../../docs/errors.md");
const BEGIN: &str = "<!-- BEGIN GENERATED: do not edit. -->";
const END: &str = "<!-- END GENERATED -->";

/// Renders the generated region: a summary table followed by one section per
/// error carrying the cause, remediation, and client-facing mapping.
fn rendered() -> std::string::String {
    let mut markdown = std::string::String::new();
    markdown.push_str("## Catalog\n\n");
    markdown.push_str("| Code | Name | Enum | Domain | Status | Retry | HTTP |\n");
    markdown.push_str("|---|---|---|---|---|---|---|\n");
    for entry in ERROR_CATALOG {
        markdown.push_str(&std::format!(
            "| {} | `{}` | `{}` | {} | {} | {} | {} |\n",
            entry.code,
            entry.name,
            entry.enum_name,
            entry.domain.as_str(),
            entry.status.as_str(),
            entry.retry.as_str(),
            entry.http_status
        ));
    }

    markdown.push_str("\n## Details\n");
    for entry in ERROR_CATALOG {
        markdown.push_str(&std::format!(
            "\n### {} - `{}`\n\n- Enum: `{}`\n- Domain: {}\n- Status: {}\n- Retry: {}\n- Cause: {}\n- Remediation: {}\n- Suggested HTTP status: {}\n- Client message: \"{}\"\n",
            entry.code,
            entry.name,
            entry.enum_name,
            entry.domain.as_str(),
            entry.status.as_str(),
            entry.retry.as_str(),
            entry.cause,
            entry.remediation,
            entry.http_status,
            entry.client_message
        ));
    }
    markdown
}

fn generated_region() -> std::string::String {
    let document = DOCUMENT.replace("\r\n", "\n");
    let start = document
        .find(BEGIN)
        .expect("docs/errors.md has no generated-region start marker")
        + BEGIN.len();
    let end = document
        .find(END)
        .expect("docs/errors.md has no generated-region end marker");
    document[start..end].trim_matches('\n').into()
}

/// Rewrites the generated region of `docs/errors.md`. Run with
/// `cargo test -p error-catalog-tests -- --ignored regenerate` after a
/// deliberate catalog change, then review the diff.
#[test]
#[ignore = "writes docs/errors.md"]
fn regenerate_the_document() {
    let document = DOCUMENT.replace("\r\n", "\n");
    let start = document.find(BEGIN).expect("missing start marker") + BEGIN.len();
    let end = document.find(END).expect("missing end marker");
    let updated = std::format!(
        "{}\n\n{}\n\n{}",
        &document[..start],
        rendered().trim_matches('\n'),
        &document[end..]
    );
    std::fs::write("../../docs/errors.md", updated).expect("cannot write docs/errors.md");
}

#[test]
fn the_document_matches_the_catalog() {
    let expected = rendered();
    let expected = expected.trim_matches('\n');
    let actual = generated_region();

    if actual != expected {
        std::panic!(
            "docs/errors.md is stale. Replace the generated region with:\n\n{BEGIN}\n\n{expected}\n\n{END}\n"
        );
    }
}

#[test]
fn the_document_lists_every_code_exactly_once() {
    let document = DOCUMENT.replace("\r\n", "\n");
    for entry in ERROR_CATALOG {
        let heading = std::format!("### {} - `{}`\n", entry.code, entry.name);
        assert_eq!(
            document.matches(heading.as_str()).count(),
            1,
            "{} should appear exactly once in docs/errors.md",
            entry.name
        );
    }
}

#[test]
fn the_document_carries_the_full_remediation_text() {
    // The table alone is not a catalog. Every entry's cause and remediation
    // must be published verbatim, so an operator reading the document never
    // has to open the source to learn what to do.
    let document = DOCUMENT.replace("\r\n", "\n");
    for entry in ERROR_CATALOG {
        assert!(
            document.contains(entry.cause),
            "docs/errors.md is missing the cause for {}",
            entry.name
        );
        assert!(
            document.contains(entry.remediation),
            "docs/errors.md is missing the remediation for {}",
            entry.name
        );
    }
}
