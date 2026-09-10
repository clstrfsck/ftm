//! The front-end-agnostic shell.
//!
//! Everything above the core that no front-end owns: the config, §10's input
//! model, the state machine and the menus. Today it holds only the neutral key
//! vocabulary of `FRONTEND.md` F5; `EGUI.md` stage G2 moves the rest of the
//! shell in beside it, out of `app.rs`, `config.rs`, `input.rs`,
//! `highscore.rs` and `ui/`.
//!
//! Nothing here may name a front-end's toolkit. A key reaches the shell as
//! [`keys::KeyEvent`], which each front-end's adapter produces from whatever
//! its own toolkit delivers.

pub mod keys;
