//! `GameView`: the serialisable render model (§12.7).
//!
//! The renderer draws only from this. Clipping to the visible rows happens here,
//! not in the renderer.

// TODO(stage 5): GameView, PieceView and `Game::view(&self)`.
// TODO(stage 8): ghost as `Option<PieceView>` and `next` sized to
// `preview_count`.
