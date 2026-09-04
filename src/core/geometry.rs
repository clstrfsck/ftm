//! `Point`, `Rotation` and direction helpers.
//!
//! **Coordinates are y-down (§5).** A cell is `(col, row)`; `col` increases to
//! the right from 0 to 9; `row` increases *downward* from 0 (top of the 40-row
//! buffer) to 39 (the floor). The visible playfield is rows 20..=39. A positive
//! `dy` moves a piece **down**.

// TODO(stage 1): Point, Rotation (North/East/South/West) with cw/ccw/opposite.
