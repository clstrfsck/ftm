//! Falling Tetromino Manager — a guideline-conformant falling-block game.
//!
//! The crate is a library plus thin binaries in `src/bin/`. The split exists so
//! the integration tests of §17.2 — the scripted game and its batch-invariance
//! canary (§19.4) — can drive the core from `tests/` without a front-end.
//!
//! Three layers, each held apart by the compiler rather than by review (§3.1):
//!
//! * [`core`] is the rules: no I/O, no clock, every module `pub(crate)` behind
//!   the façade `core/mod.rs` re-exports (§17.3 A10).
//! * [`shell`] is everything above the core that no front-end owns — the
//!   config, §10's input model, the high-score table, the menus and §12.5's
//!   animation timers. It sees the core only through `core::GameView` (§12.7)
//!   and `core::GameEvent` (§12.8), and it names no front-end's toolkit:
//!   `cargo check --no-default-features` builds `core` and `shell` alone.
//! * [`tui`] is the terminal front-end, behind `feature = "tui"`. `TUI.md` is
//!   normative for it, as `GUI.md` will be for the window `EGUI.md` adds next.
//!
//! A fourth front-end is a fifth directory, a feature and a `[[bin]]` —
//! deliberately not a `trait Frontend`. `FRONTEND.md` is the contract they
//! share, written down instead of typed.

#![forbid(unsafe_code)]

pub mod core;
pub mod shell;

#[cfg(feature = "tui")]
pub mod tui;
