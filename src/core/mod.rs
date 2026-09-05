//! The pure game core (§3.1): no I/O, no clock, deterministic, advanced only in
//! fixed 1/60 s ticks (§15.1).
//!
//! The shell sees this module only through `view::GameView` (§12.7) and
//! `events::GameEvent` (§12.8).
//!
//! Referred to as `crate::core` throughout, to avoid ambiguity with the `core`
//! crate of the standard library.

pub(crate) mod bag;
pub(crate) mod events;
pub(crate) mod game;
pub(crate) mod geometry;
pub(crate) mod gravity;
pub(crate) mod lockdown;
pub(crate) mod matrix;
pub(crate) mod piece;
pub(crate) mod scoring;
pub(crate) mod srs;
pub(crate) mod tspin;
pub(crate) mod view;

// The core's façade, and — since the modules above are `pub(crate)` — the
// whole of its public surface. This is acceptance A10 made mechanical rather
// than audited: `Game::tick` to advance the rules, `Game::view` to draw them,
// `GameEvent` to animate them, and the vocabulary those three are written in
// (§3.1, §19.2). Nothing else in the core can be named from outside the crate,
// so the renderer cannot come to depend on the rules by accident.
//
// The vocabulary is the part that is easy to get wrong. A §19 client is handed
// a `GameView` and has to draw it: that means `PieceKind` (which the view's
// cells and the hold and next slots are made of), the cell patterns it is
// drawn from and the `Rotation` they are indexed by, §9.2's `Colour`, the
// view's dimensions, and `OFF_SCREEN`, which is how an event says "above the
// field". None of those is a rule; all of them are needed to put a `GameView`
// on a screen.
pub use events::{ClearKind, GameEvent, OFF_SCREEN, ScoreReason, TopOutCause};
pub use game::{Action, Actions, Game, PlayState, Shift, TickInput};
pub use geometry::Rotation;
pub use piece::{Colour, PieceKind};
pub use view::{DebugView, GameView, PieceView, VIEW_HEIGHT, VIEW_WIDTH};
