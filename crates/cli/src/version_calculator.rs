//! Default version calculator — re-exported from `release-regent-core`.
//!
//! The implementation lives in [`release_regent_core::DefaultVersionCalculator`]
//! so that both the CLI and the server binary can share it without duplication.

pub use release_regent_core::DefaultVersionCalculator;

#[cfg(test)]
#[path = "version_calculator_tests.rs"]
mod tests;
