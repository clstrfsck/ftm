//! `Game` state and `Game::tick` — the single entry point to the core (§15.1).

// TODO(stage 4): spawn with the immediate one-row drop, movement, hard drop,
// line clear with the clear/entry delay states, Block Out and Lock Out.
// TODO(stage 5): `Game::tick(&TickInput, &mut Vec<GameEvent>)` as the only way
// to advance the core.
// TODO(stage 8): hold, with its lock-out cleared on the next *lock* (§9.7).
