//! Negative fixture tests: prove that breaking changes fail the gates.
//!
//! These tests capture intentional breaking changes and verify that the
//! compatibility gates catch them. They serve as proof that the gates work
//! and as a reference for what a failing report looks like.
//!
//! When a real breaking change is introduced, the gates will fail with a
//! report similar to these fixtures.

#[cfg(test)]
mod tests {
    use compatibility_tests::artifacts::*;
    use compatibility_tests::gates::*;
    use std::collections::HashSet;

    /// Fixture: removed function fails the ABI gate.
    #[test]
    fn breaking_change_removed_function_fails_abi_gate() {
        // Golden snapshot includes "initialize"
        let golden = ["initialize", "get_admin"].iter().cloned().collect();
        // Current code is missing "initialize"
        let current = ["get_admin"].iter().cloned().collect();

        let report = check_abi("protocol-config", &golden, &current);

        assert!(report.is_breaking(), "removed function should be breaking");
        assert!(
            report.removed.contains(&"initialize".to_string()),
            "report should list removed function"
        );
    }

    /// Fixture: added function passes the ABI gate as additive.
    #[test]
    fn additive_change_new_function_passes_abi_gate() {
        let golden = ["initialize", "get_admin"].iter().cloned().collect();
        let current = ["initialize", "get_admin", "new_function"]
            .iter()
            .cloned()
            .collect();

        let report = check_abi("protocol-config", &golden, &current);

        assert!(
            report.is_additive(),
            "added function should be additive"
        );
        assert!(
            report.added.contains(&"new_function".to_string()),
            "report should list added function"
        );
    }

    /// Fixture: removed storage key fails the storage gate.
    #[test]
    fn breaking_change_removed_storage_key_fails_gate() {
        let golden = ["Admin", "Paused", "ConfigVersion"]
            .iter()
            .cloned()
            .collect();
        let current = ["Admin", "Paused"].iter().cloned().collect();

        let report = check_storage("protocol-config", &golden, &current);

        assert!(report.is_breaking(), "removed key should be breaking");
        assert!(
            report.removed.contains(&"ConfigVersion".to_string()),
            "report should list removed key"
        );
    }

    /// Fixture: added storage key passes the storage gate as additive.
    #[test]
    fn additive_change_new_storage_key_passes_gate() {
        let golden = ["Admin", "Paused"].iter().cloned().collect();
        let current = ["Admin", "Paused", "NewKey"]
            .iter()
            .cloned()
            .collect();

        let report = check_storage("protocol-config", &golden, &current);

        assert!(report.is_additive(), "new key should be additive");
        assert!(
            report.added.contains(&"NewKey".to_string()),
            "report should list added key"
        );
    }

    /// Fixture: removed error code fails the error gate.
    #[test]
    fn breaking_change_removed_error_code_fails_gate() {
        let golden = [(1u32, "AlreadyInitialized"), (2u32, "NotInitialized")]
            .iter()
            .cloned()
            .collect();
        let current = [(1u32, "AlreadyInitialized")]
            .iter()
            .cloned()
            .collect();

        let report = check_errors("protocol-config", &golden, &current);

        assert!(report.is_breaking(), "removed error should be breaking");
        assert!(
            report
                .removed
                .iter()
                .any(|e| e.contains("NotInitialized")),
            "report should list removed error"
        );
    }

    /// Fixture: error code reassignment fails the error gate.
    #[test]
    fn breaking_change_error_code_changed_fails_gate() {
        let golden = [(1u32, "AlreadyInitialized"), (2u32, "NotInitialized")]
            .iter()
            .cloned()
            .collect();
        let current = [(1u32, "AlreadyInitialized"), (99u32, "NotInitialized")]
            .iter()
            .cloned()
            .collect();

        let report = check_errors("protocol-config", &golden, &current);

        assert!(
            report.is_breaking(),
            "reassigned error code should be breaking"
        );
        assert!(
            !report.changed.is_empty(),
            "report should list changed error codes"
        );
    }

    /// Fixture: added error code passes as semantic (behavior change, not interface).
    #[test]
    fn semantic_change_new_error_code_passes_gate() {
        let golden = [(1u32, "AlreadyInitialized"), (2u32, "NotInitialized")]
            .iter()
            .cloned()
            .collect();
        let current = [
            (1u32, "AlreadyInitialized"),
            (2u32, "NotInitialized"),
            (99u32, "NewError"),
        ]
        .iter()
        .cloned()
        .collect();

        let report = check_errors("protocol-config", &golden, &current);

        // Adding an error is semantic (changes behavior) but not breaking
        assert!(!report.is_breaking(), "added error should not be breaking");
    }

    /// Fixture: removed event fails the event gate.
    #[test]
    fn breaking_change_removed_event_fails_gate() {
        let golden = ["Initialized", "AdminChanged"]
            .iter()
            .cloned()
            .collect();
        let current = ["Initialized"].iter().cloned().collect();

        let report = check_events("protocol-config", &golden, &current);

        assert!(report.is_breaking(), "removed event should be breaking");
        assert!(
            report.removed.contains(&"AdminChanged".to_string()),
            "report should list removed event"
        );
    }

    /// Fixture: added event passes as additive.
    #[test]
    fn additive_change_new_event_passes_gate() {
        let golden = ["Initialized", "AdminChanged"]
            .iter()
            .cloned()
            .collect();
        let current = ["Initialized", "AdminChanged", "NewEvent"]
            .iter()
            .cloned()
            .collect();

        let report = check_events("protocol-config", &golden, &current);

        assert!(report.is_additive(), "new event should be additive");
        assert!(
            report.added.contains(&"NewEvent".to_string()),
            "report should list added event"
        );
    }
}
