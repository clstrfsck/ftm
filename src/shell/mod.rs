//! The front-end-agnostic shell.
//!
//! Everything above the core that no front-end owns: §6's config, §10's input
//! model, §14's high-score table, the menus of §12.6 and §13, the attract
//! screen's state machine and §12.5's animation timers.
//!
//! Nothing here may name a front-end's toolkit, and the compiler holds that
//! boundary the way it holds the core's (§17.3 A10): `cargo check
//! --no-default-features` builds `core` and `shell` with neither front-end
//! feature on, so a `ratatui` or an `egui` that crept in here goes red in the
//! same commit. `EGUI.md` stage G3 tightens the same check to
//! `wasm32-unknown-unknown`, which is what takes the *platform* out too.
//!
//! A key reaches the shell as [`keys::KeyEvent`], which each front-end's
//! adapter produces from whatever its own toolkit delivers (`FRONTEND.md` F5).

pub mod attract;
pub mod config;
pub mod cosmetics;
pub mod highscore;
pub mod input;
pub mod keys;
pub mod menus;
pub mod palette;
