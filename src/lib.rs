//! Termino — a guideline-conformant falling-block game for the terminal.
//!
//! The crate is a library plus a thin binary (`main.rs`). The split exists so
//! the integration tests of §17.2 — the scripted game and its batch-invariance
//! canary (§19.4) — can drive the core from `tests/` without a terminal.
//!
//! The crate is split into a pure `core` (rules, no I/O, no clock — §3.1) and a
//! shell (`app`, `config`, `input`, `highscore`, `ui`). The shell sees the core
//! only through `core::GameView` (§12.7) and `core::GameEvent` (§12.8), which
//! since Stage 12 the compiler enforces: every module inside `core` is
//! `pub(crate)` and its façade is the whole of its public surface (A10).

#![forbid(unsafe_code)]

pub mod app;
pub mod config;
pub mod core;
pub mod highscore;
pub mod input;
pub mod ui;
