//! Termino — a guideline-conformant falling-block game for the terminal.
//!
//! Entry point: terminal setup/teardown and the panic hook (TERMINO.md §8).
//! Everything else lives in the library crate (`lib.rs`), so the integration
//! tests of §17.2 can drive the core headlessly.

#![forbid(unsafe_code)]

// TODO(stage 6): terminal setup/teardown in §8.1/§8.3 order, panic hook that
// restores the terminal first, and the §15.2 loop.
// TODO(stage 10): clap CLI (§6.4) and the §16 error-handling contract.
fn main() {
    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
}
