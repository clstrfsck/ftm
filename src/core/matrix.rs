//! Matrix storage, collision and line clearing (§9.1).
//!
//! 10 columns x 40 rows. Out of bounds is solid to the left, right and below,
//! and empty above row 0.

// TODO(stage 1): storage, `is_filled`, `collides(piece, origin, rotation)`.
// TODO(stage 4): line detection and clearing (§9.12).
