//! `wxqr` CLI binary — thin shim that calls the library entry point.

use std::process::ExitCode;

#[cfg(not(test))]
fn main() -> ExitCode {
    wxqr::run()
}

// When cargo test runs, the wxqr *binary* is also invoked as a test
// harness (target/release/deps/wxqr-<hash>). Without this stub
// `cargo test` would call main() → wxqr::run() and panic with
// "Unrecognized option: 'v'" (the cargo test args don't look like
// wxqr CLI args). The stub gives cargo a no-op main when invoked
// from the test harness.
#[cfg(test)]
fn main() -> ExitCode {
    ExitCode::SUCCESS
}
