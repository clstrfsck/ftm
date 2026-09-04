//! `GameEvent`: what happened during a tick (§12.8).
//!
//! Events drive cosmetics only. Dropping every event must change nothing about
//! the game state (T16).

// TODO(stage 5): the event enum, emitted in rules order and allocation-free in
// the common case.
