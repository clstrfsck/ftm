//! The pure game core (§3.1): no I/O, no clock, deterministic, advanced only in
//! fixed 1/60 s ticks (§15.1).
//!
//! The shell sees this module only through `view::GameView` (§12.7) and
//! `events::GameEvent` (§12.8).
//!
//! Referred to as `crate::core` throughout, to avoid ambiguity with the `core`
//! crate of the standard library.

pub mod bag;
pub mod events;
pub mod game;
pub mod geometry;
pub mod gravity;
pub mod lockdown;
pub mod matrix;
pub mod piece;
pub mod scoring;
pub mod srs;
pub mod tspin;
pub mod view;

// TODO(stage 5): re-export the `Game` façade and the view/event types — and
// nothing else (§19.2, acceptance A10).
