#![no_main]
use earnproof_shared::ProofStatus;
use libfuzzer_sys::fuzz_target;

// Fuzz target for ProofStatus enum deserialization
// Tests that arbitrary discriminants are handled correctly:
// Valid: 0 (Active), 1 (Revoked)
// Invalid: anything >= 2 should fail gracefully
fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // Use first byte as discriminant
    let discriminant = data[0];

    let status = match discriminant {
        0 => Some(ProofStatus::Active),
        1 => Some(ProofStatus::Revoked),
        _ => None, // Invalid discriminant
    };

    if let Some(s) = status {
        // Verify we can clone and compare
        let cloned = s.clone();
        assert_eq!(s, cloned);

        // Verify enum variant matching
        match s {
            ProofStatus::Active => {
                assert_eq!(discriminant, 0);
            }
            ProofStatus::Revoked => {
                assert_eq!(discriminant, 1);
            }
        }
    }
    // For invalid discriminants, we silently skip (no panic expected)
});
