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
//! * [`tui`] is the terminal front-end, behind `feature = "tui"`, and [`gui`]
//!   is the window one, behind `feature = "gui"`. `TUI.md` and `GUI.md` are
//!   normative for them.
//!
//! [`pilot`] is the automated player of `PILOT.md`, and is a **sibling** of the
//! first two rather than a fourth layer: it sees the core through the same
//! façade the shell does, plus §P2.3's crate-private search seam, and it names
//! nothing in the shell but `RulesConfig`. It takes no clock and no I/O for the
//! same reason the other two do not, and the same two checks hold it there.
//!
//! A third front-end is a fourth directory, a feature and a `[[bin]]` —
//! deliberately not a `trait Frontend`. `FRONTEND.md` is the contract they
//! share, written down instead of typed.
//!
//! [`native`] and [`argv`] are the odd ones out and are not a layer: they are
//! the *desktop* both native front-ends stand on. [`native`] is `FRONTEND.md`
//! F1-F4 — a clock, a filesystem, an entropy source and a calendar — kept in
//! one place because §6.2 and §14 give `ftm` and `ftm-gui` one config file and
//! one high-score table between them; [`argv`] is the one capability beside
//! those four, and holds the two §6.4 value spellings both binaries share. The
//! web build of `gui` takes nothing from either, and is not compiled with
//! them: a browser tab has no clock of that kind, no files, and no argv.

#![forbid(unsafe_code)]

pub mod core;
pub mod pilot;
pub mod shell;

#[cfg(all(any(feature = "tui", feature = "gui"), not(target_arch = "wasm32")))]
pub mod argv;
#[cfg(all(any(feature = "tui", feature = "gui"), not(target_arch = "wasm32")))]
pub mod native;

#[cfg(feature = "gui")]
pub mod gui;

#[cfg(feature = "tui")]
pub mod tui;
