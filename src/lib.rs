//! Termino — a guideline-conformant falling-block game for the terminal.
//!
//! The crate is a library plus a thin binary (`main.rs`). The split exists so
//! the integration tests of §17.2 — the scripted game and its batch-invariance
//! canary (§19.4) — can drive the core from `tests/` without a terminal.
//!
//! The crate is split into a pure `core` (rules, no I/O, no clock — §3.1) and a
//! shell (`app`, `config`, `input`, `highscore`, `ui`). The shell sees the core
//! only through `core::GameView` (§12.7) and `core::GameEvent` (§12.8).

#![forbid(unsafe_code)]
// The core is built ahead of the shell that consumes it (PLAN.md sequencing), so
// items land a stage or two before their first caller and `dead_code` fires on
// every one of them.
// TODO(stage 12): remove this; acceptance A10 audits the core's surface by hand.
#![allow(dead_code)]

pub mod app;
pub mod config;
pub mod core;
pub mod highscore;
pub mod input;
pub mod ui;
