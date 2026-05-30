// Fuzz target for `EventType::from`.
//
// Converts arbitrary byte slices to UTF-8 strings (falling back to empty on
// invalid UTF-8) and asserts `EventType::from` never panics.
//
// Run on Linux/macOS CI via:
//   cargo +nightly fuzz run fuzz_event_type_from_str -- -max_total_time=60
#![no_main]

use libfuzzer_sys::fuzz_target;
use release_regent_core::traits::event_source::EventType;

fuzz_target!(|data: &[u8]| {
    let s = std::str::from_utf8(data).unwrap_or("");

    // Must not panic.  Any EventType variant (including Unknown) is acceptable.
    let _ = EventType::from(s);
});
