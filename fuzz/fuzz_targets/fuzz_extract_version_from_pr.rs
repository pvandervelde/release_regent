// Fuzz target for `extract_version_from_pr`.
//
// The function accepts four arbitrary UTF-8 strings (branch, title, body,
// branch_prefix, version_prefix) and must never panic regardless of input.
// Returning `Err` is the correct response to unparseable inputs.
//
// This target is designed to run on Linux/macOS CI via:
//   cargo +nightly fuzz run fuzz_extract_version_from_pr -- -max_total_time=60
#![no_main]

use libfuzzer_sys::fuzz_target;
use release_regent_core::release_automator::extract_version_from_pr;

fuzz_target!(|data: &[u8]| {
    // Split the byte slice into five segments using the first four 0x00 bytes
    // as separators.  If there are fewer than four separators, the remaining
    // segments default to empty strings — all valid inputs for the function.
    let parts: Vec<&[u8]> = data.splitn(5, |&b| b == 0x00).collect();
    let get = |i: usize| std::str::from_utf8(parts.get(i).copied().unwrap_or(&[])).unwrap_or("");

    let branch = get(0);
    let title = get(1);
    let body = get(2);
    let branch_prefix = get(3);
    let version_prefix = get(4);

    // Must not panic.  Err is acceptable.
    let _ = extract_version_from_pr(branch, title, body, branch_prefix, version_prefix);
});
