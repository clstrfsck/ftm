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

// The core's façade. The shell is expected to reach for these and nothing else:
// `Game` to advance the rules, `GameView` to draw them, `GameEvent` to animate
// them (§3.1, §19.2). Acceptance A10 audits that in Stage 12.
pub use events::{ClearKind, GameEvent, ScoreReason, TopOutCause};
pub use game::{Action, Actions, Game, PlayState, Shift, TickInput};
pub use view::{DebugView, GameView, PieceView};
