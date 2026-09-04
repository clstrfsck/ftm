//! Screen dispatch, the terminal-too-small screen (§12.1) and the animation
//! timers (§12.5).
//!
//! Everything here draws from `GameView` alone and never reads `Game` (§12.7).

pub mod attract;
pub mod cells;
pub mod overlays;
pub mod playfield;
pub mod theme;

// TODO(stage 9): screen dispatch and the six §12.5 animations, each started by a
// GameEvent and timed on the wall clock.
// TODO(stage 12): the terminal-too-small screen and resize handling (§8.4).
