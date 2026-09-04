//! `Point`, `Rotation` and direction helpers.
//!
//! **Coordinates are y-down (§5).** A cell is `(col, row)`; `col` increases to
//! the right from 0 to 9; `row` increases *downward* from 0 (top of the 40-row
//! buffer) to 39 (the floor). The visible playfield is rows 20..=39. "Above"
//! means a numerically smaller `row`, and a positive `dy` moves a piece **down**.

/// A matrix coordinate, or a translation between two of them.
///
/// Signed and wider than the matrix because kick offsets (§9.5) and piece box
/// origins may legitimately sit outside the matrix; only the *minos* are
/// constrained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Point {
    /// Column, increasing rightwards.
    pub x: i32,
    /// Row, increasing **downwards** (§5).
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Translate by `(dx, dy)`. A positive `dy` moves **down**.
    pub const fn translate(self, dx: i32, dy: i32) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }
}

/// Piece orientation (§9.3): `0 = North` (spawn), incrementing clockwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Rotation {
    /// Spawn orientation, `0`.
    #[default]
    North,
    /// One clockwise turn from spawn, `1`.
    East,
    /// Two turns from spawn, `2`.
    South,
    /// Three clockwise turns from spawn, `3`.
    West,
}

impl Rotation {
    /// All four orientations in numbering order.
    pub const ALL: [Rotation; 4] = [
        Rotation::North,
        Rotation::East,
        Rotation::South,
        Rotation::West,
    ];

    /// The orientation number, `0..=3` (§9.3).
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The orientation with number `n`, taken modulo 4.
    pub const fn from_index(n: usize) -> Self {
        Self::ALL[n % 4]
    }

    /// One quarter turn clockwise: the orientation number increments mod 4.
    pub const fn cw(self) -> Self {
        Self::from_index(self.index() + 1)
    }

    /// One quarter turn anticlockwise.
    pub const fn ccw(self) -> Self {
        Self::from_index(self.index() + 3)
    }

    /// Two quarter turns; the target of a 180-degree rotation (§9.5).
    pub const fn opposite(self) -> Self {
        Self::from_index(self.index() + 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_dy_moves_down() {
        // The y-down convention of §5, asserted so a later "fix" trips a test.
        assert_eq!(Point::new(4, 19).translate(0, 1), Point::new(4, 20));
    }

    #[test]
    fn rotation_numbering_matches_spec() {
        assert_eq!(Rotation::North.index(), 0);
        assert_eq!(Rotation::East.index(), 1);
        assert_eq!(Rotation::South.index(), 2);
        assert_eq!(Rotation::West.index(), 3);
    }

    #[test]
    fn quarter_turns_compose() {
        for r in Rotation::ALL {
            assert_eq!(r.cw().ccw(), r);
            assert_eq!(r.cw().cw(), r.opposite());
            assert_eq!(r.opposite().opposite(), r);
            assert_eq!(r.cw().cw().cw().cw(), r);
        }
        assert_eq!(Rotation::North.cw(), Rotation::East);
        assert_eq!(Rotation::North.ccw(), Rotation::West);
    }
}
