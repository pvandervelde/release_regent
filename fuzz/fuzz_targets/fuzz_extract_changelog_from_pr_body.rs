// Fuzz target for `extract_changelog_from_pr_body`.
//
// Splits the input into two UTF-8 segments (body, changelog_header) and
// asserts the function never panics.  Any string output is acceptable.
//
// Run on Linux/macOS CI via:
//   cargo +nightly fuzz run fuzz_extract_changelog_from_pr_body -- -max_total_time=60
#![no_main]

use libfuzzer_sys::fuzz_target;
use release_regent_core::release_orchestrator::extract_changelog_from_pr_body;

fuzz_target!(|data: &[u8]| {
    let parts: Vec<&[u8]> = data.splitn(2, |&b| b == 0x00).collect();
    let get = |i: usize| std::str::from_utf8(parts.get(i).copied().unwrap_or(&[])).unwrap_or("");

    let body = get(0);
    let header = get(1);

    // Must not panic.  Any string result is acceptable.
    let _ = extract_changelog_from_pr_body(body, header);
});
