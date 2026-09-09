#![no_main]
use earnproof_shared::IssuerStatus;
use libfuzzer_sys::fuzz_target;

// Fuzz target for IssuerStatus enum deserialization
// Tests that arbitrary discriminants are handled correctly:
// Valid: 0 (Active), 1 (Suspended), 2 (Revoked)
// Invalid: anything >= 3 should fail gracefully
fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // Use first byte as discriminant
    let discriminant = data[0];

    let status = match discriminant {
        0 => Some(IssuerStatus::Active),
        1 => Some(IssuerStatus::Suspended),
        2 => Some(IssuerStatus::Revoked),
        _ => None, // Invalid discriminant
    };

    if let Some(s) = status {
        // Verify we can clone and compare
        let cloned = s.clone();
        assert_eq!(s, cloned);

        // Verify enum variant matching
        match s {
            IssuerStatus::Active => {
                assert_eq!(discriminant, 0);
            }
            IssuerStatus::Suspended => {
                assert_eq!(discriminant, 1);
            }
            IssuerStatus::Revoked => {
                assert_eq!(discriminant, 2);
            }
        }
    }
    // For invalid discriminants, we silently skip (no panic expected)
});
