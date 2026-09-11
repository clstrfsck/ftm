//! The four capabilities of `FRONTEND.md` F1-F4, for the window on a desktop
//! (`GUI.md` §G1).
//!
//! There is almost nothing here, and that is the point. A desktop is a desktop
//! whichever front-end is standing on it, and §6.2 and §14 say `ftm` and
//! `ftm-gui` share one config file and one high-score table — so the clock,
//! the files, the seed and the date all come from [`crate::native`], which the
//! terminal front-end uses unchanged. What this module is, is the *native* arm
//! of this front-end's own split: `EGUI.md` G6 adds `host_web.rs` beside it
//! with the same four names over `web_sys`, and `gui/app.rs` never learns which
//! of the two it is running on.

pub use crate::native::{Clock, Files, seed, today};
