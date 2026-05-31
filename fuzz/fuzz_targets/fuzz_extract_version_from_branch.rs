// Fuzz target for `extract_version_from_branch`.
//
// Splits the input into three segments (branch, branch_prefix, version_prefix)
// and asserts the function never panics.  Returning `Err` is acceptable.
//
// Run on Linux/macOS CI via:
//   cargo +nightly fuzz run fuzz_extract_version_from_branch -- -max_total_time=60
#![no_main]

use libfuzzer_sys::fuzz_target;
use release_regent_core::release_automator::extract_version_from_branch;

fuzz_target!(|data: &[u8]| {
    let parts: Vec<&[u8]> = data.splitn(3, |&b| b == 0x00).collect();
    let get = |i: usize| std::str::from_utf8(parts.get(i).copied().unwrap_or(&[])).unwrap_or("");

    let branch = get(0);
    let branch_prefix = get(1);
    let version_prefix = get(2);

    // Must not panic.  Err is acceptable.
    let _ = extract_version_from_branch(branch, branch_prefix, version_prefix);
});
