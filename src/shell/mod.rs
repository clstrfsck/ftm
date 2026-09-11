//! The front-end-agnostic shell.
//!
//! Everything above the core that no front-end owns: §6's config, §10's input
//! model, §14's high-score table, the menus of §12.6 and §13, the attract
//! screen's state machine, §12.5's animation timers, and — since `EGUI.md`
//! stage G4 — §7's two pumpable screens themselves, [`round::Round`] and
//! [`attract::Attract`], over the [`session::Session`] they share.
//!
//! Nothing here may name a front-end's toolkit, and the compiler holds that
//! boundary the way it holds the core's (§17.3 A10): `cargo check
//! --no-default-features` builds `core` and `shell` with neither front-end
//! feature on, so a `ratatui` or an `egui` that crept in here goes red in the
//! same commit. Since `EGUI.md` stage G3 the same check runs again for
//! `wasm32-unknown-unknown`, which is what takes the *platform* out too: no
//! `Instant::now`, no `std::fs`, no `rand::random`, no calendar, and no `cfg`
//! anywhere in here pretending otherwise.
//!
//! The front-end owns the loop and calls in (`FRONTEND.md` F7); §15.2's seven
//! numbered steps are methods here rather than the body of a `while`, so a
//! terminal's poll loop, `eframe`'s `update` and a test harness with no screen
//! at all drive the same game.
//!
//! A key reaches the shell as [`keys::KeyEvent`], which each front-end's
//! adapter produces from whatever its own toolkit delivers (`FRONTEND.md` F5);
//! the clock, the filesystem, the entropy and the calendar reach it as
//! [`time::Stamp`], [`storage::Storage`] and the two function pointers of
//! [`host::Host`] (F1-F4).

pub mod attract;
pub mod config;
pub mod cosmetics;
pub mod figures;
pub mod highscore;
pub mod host;
pub mod input;
pub mod keys;
pub mod menus;
pub mod palette;
pub mod round;
pub mod session;
pub mod storage;
pub mod time;
