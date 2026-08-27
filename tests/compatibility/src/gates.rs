//! Compatibility gates: distinguish breaking vs. additive changes.
//!
//! A breaking change gate detects when a contract's ABI, storage, errors, or
//! events change in a way that would break downstream consumers. Additive
//! changes (new functions, new keys, new errors, new events) pass; breaking
//! changes (removed functions, changed types, renamed fields) fail.
//!
//! Each gate compares the current artifacts against the golden snapshot,
//! classifies the change, and fails if a breaking change is detected.

use std::collections::HashSet;

/// Result of a compatibility check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangeClass {
    /// No change.
    Unchanged,
    /// A purely additive change (new function, new key, new error, new event).
    Additive,
    /// A change with potential side effects but no interface break.
    Semantic,
    /// A change that breaks existing callers.
    Breaking,
}

/// Detailed report on a compatibility check.
#[derive(Clone, Debug)]
pub struct CompatibilityReport {
    pub contract_name: &'static str,
    pub class: ChangeClass,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
}

impl CompatibilityReport {
    pub fn is_breaking(&self) -> bool {
        self.class == ChangeClass::Breaking
    }

    pub fn is_additive(&self) -> bool {
        self.class == ChangeClass::Additive
    }

    pub fn summary(&self) -> String {
        let mut lines = vec![format!("{}: {:?}", self.contract_name, self.class)];

        if !self.added.is_empty() {
            lines.push(format!("  + Added: {}", self.added.join(", ")));
        }
        if !self.removed.is_empty() {
            lines.push(format!("  - Removed: {}", self.removed.join(", ")));
        }
        if !self.changed.is_empty() {
            lines.push(format!("  ~ Changed: {}", self.changed.join(", ")));
        }

        lines.join("\n")
    }
}

/// Gate for function ABI compatibility.
pub fn check_abi(
    contract_name: &'static str,
    golden: &HashSet<&'static str>,
    current: &HashSet<&'static str>,
) -> CompatibilityReport {
    let added: Vec<_> = current
        .difference(golden)
        .map(|s| s.to_string())
        .collect();
    let removed: Vec<_> = golden
        .difference(current)
        .map(|s| s.to_string())
        .collect();

    let class = if !removed.is_empty() {
        ChangeClass::Breaking
    } else if !added.is_empty() {
        ChangeClass::Additive
    } else {
        ChangeClass::Unchanged
    };

    CompatibilityReport {
        contract_name,
        class,
        added,
        removed,
        changed: vec![],
    }
}

/// Gate for storage key compatibility.
pub fn check_storage(
    contract_name: &'static str,
    golden: &HashSet<&'static str>,
    current: &HashSet<&'static str>,
) -> CompatibilityReport {
    let added: Vec<_> = current
        .difference(golden)
        .map(|s| s.to_string())
        .collect();
    let removed: Vec<_> = golden
        .difference(current)
        .map(|s| s.to_string())
        .collect();

    let class = if !removed.is_empty() {
        ChangeClass::Breaking
    } else if !added.is_empty() {
        ChangeClass::Additive
    } else {
        ChangeClass::Unchanged
    };

    CompatibilityReport {
        contract_name,
        class,
        added,
        removed,
        changed: vec![],
    }
}

/// Gate for error code compatibility.
///
/// Removing an error code is breaking; adding one is semantic (changes behavior).
pub fn check_errors(
    contract_name: &'static str,
    golden: &HashSet<(u32, &'static str)>,
    current: &HashSet<(u32, &'static str)>,
) -> CompatibilityReport {
    let added: Vec<_> = current
        .difference(golden)
        .map(|(code, name)| format!("{} ({})", name, code))
        .collect();
    let removed: Vec<_> = golden
        .difference(current)
        .map(|(code, name)| format!("{} ({})", name, code))
        .collect();

    // Check for changed error code assignments (same name, different code)
    let mut changed = vec![];
    for (golden_code, golden_name) in golden.iter() {
        if let Some((current_code, current_name)) = current.iter().find(|(_, n)| n == golden_name) {
            if golden_code != current_code {
                changed.push(format!(
                    "{}: {} -> {}",
                    golden_name, golden_code, current_code
                ));
            }
        }
    }

    let class = if !removed.is_empty() || !changed.is_empty() {
        ChangeClass::Breaking
    } else if !added.is_empty() {
        ChangeClass::Semantic // Adding errors changes behavior, not interface
    } else {
        ChangeClass::Unchanged
    };

    CompatibilityReport {
        contract_name,
        class,
        added,
        removed,
        changed,
    }
}

/// Gate for event compatibility.
///
/// Removing an event is breaking; adding one is additive.
pub fn check_events(
    contract_name: &'static str,
    golden: &HashSet<&'static str>,
    current: &HashSet<&'static str>,
) -> CompatibilityReport {
    let added: Vec<_> = current
        .difference(golden)
        .map(|s| s.to_string())
        .collect();
    let removed: Vec<_> = golden
        .difference(current)
        .map(|s| s.to_string())
        .collect();

    let class = if !removed.is_empty() {
        ChangeClass::Breaking
    } else if !added.is_empty() {
        ChangeClass::Additive
    } else {
        ChangeClass::Unchanged
    };

    CompatibilityReport {
        contract_name,
        class,
        added,
        removed,
        changed: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_removed_function() {
        let golden = ["foo", "bar"].iter().cloned().collect();
        let current = ["foo"].iter().cloned().collect();
        let report = check_abi("test", &golden, &current);
        assert_eq!(report.class, ChangeClass::Breaking);
        assert!(report.removed.contains(&"bar".to_string()));
    }

    #[test]
    fn detects_added_function() {
        let golden = ["foo"].iter().cloned().collect();
        let current = ["foo", "bar"].iter().cloned().collect();
        let report = check_abi("test", &golden, &current);
        assert_eq!(report.class, ChangeClass::Additive);
        assert!(report.added.contains(&"bar".to_string()));
    }

    #[test]
    fn detects_removed_error() {
        let golden = [(1u32, "Error1"), (2u32, "Error2")]
            .iter()
            .cloned()
            .collect();
        let current = [(1u32, "Error1")].iter().cloned().collect();
        let report = check_errors("test", &golden, &current);
        assert_eq!(report.class, ChangeClass::Breaking);
    }

    #[test]
    fn detects_changed_error_code() {
        let golden = [(1u32, "Error1"), (2u32, "Error2")]
            .iter()
            .cloned()
            .collect();
        let current = [(1u32, "Error1"), (3u32, "Error2")]
            .iter()
            .cloned()
            .collect();
        let report = check_errors("test", &golden, &current);
        assert_eq!(report.class, ChangeClass::Breaking);
        assert!(!report.changed.is_empty());
    }
}
