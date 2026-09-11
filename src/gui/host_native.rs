//! The four capabilities of `FRONTEND.md` F1-F4, for the window on a desktop
//! (`GUI.md` §G1).
//!
//! There is almost nothing here, and that is the point. A desktop is a desktop
//! whichever front-end is standing on it, and §6.2 and §14 say `ftm` and
//! `ftm-gui` share one config file and one high-score table — so the clock,
//! the files, the seed and the date all come from [`crate::native`], which the
//! terminal front-end uses unchanged. What this module is, is the *native* arm
//! of this front-end's own split: `host_web.rs` beside it answers the same
//! names over `web_sys`, and `gui/app.rs` never learns which of the two it is
//! running on.

pub use crate::native::{Clock, Files, seed, today};

/// §16's warnings as they arise: natively, they wait.
///
/// The window's run has an end — `eframe::run_native` returns when it closes —
/// and §G1.2 prints them on stderr then, from `main`, where the terminal
/// front-end prints them after teardown. So there is nothing to do here; it
/// exists so that `gui/app.rs` says the same thing on both hosts, and a tab,
/// which has no end, can answer differently (§G8.6).
pub fn report(_warnings: &[String]) {}
