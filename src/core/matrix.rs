//! Matrix storage, collision and line clearing (§9.1).
//!
//! 10 columns x 40 rows, indexed `[row][col]` with `row` increasing downwards
//! (§5). Rows 20..=39 are visible; rows 0..=19 are the buffer zone and are never
//! drawn. `None` is an empty cell; `Some(kind)` records which tetromino filled
//! it, so locked minos keep their colour.

use crate::core::geometry::{Point, Rotation};
use crate::core::piece::PieceKind;

/// Matrix width in columns (§9.1).
pub const WIDTH: i32 = 10;
/// Matrix height in rows, buffer zone included (§9.1).
pub const HEIGHT: i32 = 40;
/// The first visible row; rows above it are the buffer zone (§5).
pub const VISIBLE_TOP: i32 = 20;
/// The number of visible rows (§5).
pub const VISIBLE_ROWS: i32 = HEIGHT - VISIBLE_TOP;

/// The playfield's locked cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Matrix {
    rows: [[Option<PieceKind>; WIDTH as usize]; HEIGHT as usize],
}

impl Default for Matrix {
    fn default() -> Self {
        Self::new()
    }
}

impl Matrix {
    /// An empty matrix.
    pub const fn new() -> Self {
        Self {
            rows: [[None; WIDTH as usize]; HEIGHT as usize],
        }
    }

    /// The cell at `(x, y)`, or `None` if it is empty or out of bounds.
    pub fn get(&self, x: i32, y: i32) -> Option<PieceKind> {
        if !Self::in_bounds(x, y) {
            return None;
        }
        self.rows[y as usize][x as usize]
    }

    /// Whether `(x, y)` is inside the matrix.
    pub const fn in_bounds(x: i32, y: i32) -> bool {
        x >= 0 && x < WIDTH && y >= 0 && y < HEIGHT
    }

    /// Whether `(x, y)` blocks a mino.
    ///
    /// Out of bounds is **solid** to the left, right and below, and **empty**
    /// above row 0 (§9.1). A piece may never be positioned with a mino above row
    /// 0 — spawn is defined so it cannot occur — but collision still has to
    /// answer the question, and the answer is "not blocked".
    pub fn is_filled(&self, x: i32, y: i32) -> bool {
        if !(0..WIDTH).contains(&x) || y >= HEIGHT {
            return true;
        }
        if y < 0 {
            return false;
        }
        self.rows[y as usize][x as usize].is_some()
    }

    /// Fill a single cell. Out-of-bounds writes are ignored.
    // TODO(stage 4): locking a piece and clearing lines (§9.12) build on this.
    pub fn set(&mut self, x: i32, y: i32, kind: Option<PieceKind>) {
        if Self::in_bounds(x, y) {
            self.rows[y as usize][x as usize] = kind;
        }
    }

    /// Whether `kind` at `origin` in `rotation` overlaps a filled cell, a wall or
    /// the floor.
    pub fn collides(&self, kind: PieceKind, origin: Point, rotation: Rotation) -> bool {
        kind.minos(origin, rotation)
            .iter()
            .any(|p| self.is_filled(p.x, p.y))
    }

    /// Whether every cell of row `y` is filled.
    pub fn row_is_full(&self, y: i32) -> bool {
        (0..WIDTH).all(|x| self.is_filled(x, y))
    }

    /// Whether the matrix holds no locked cells at all — the perfect-clear
    /// condition of §9.15.
    pub fn is_empty(&self) -> bool {
        self.rows.iter().all(|row| row.iter().all(Option::is_none))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a matrix from a picture of its bottom rows, `#` filled and `.`
    /// empty, the last string being row 39. Every fixture in the later stages is
    /// written this way, so it lives here rather than in each test.
    #[cfg(test)]
    pub fn from_bottom_rows(rows: &[&str]) -> Matrix {
        let mut matrix = Matrix::new();
        for (i, row) in rows.iter().rev().enumerate() {
            let y = HEIGHT - 1 - i as i32;
            assert_eq!(
                row.chars().count(),
                WIDTH as usize,
                "row {row:?} is not 10 wide"
            );
            for (x, c) in row.chars().enumerate() {
                if c == '#' {
                    matrix.set(x as i32, y, Some(PieceKind::I));
                }
            }
        }
        matrix
    }

    #[test]
    fn dimensions_match_the_spec() {
        assert_eq!((WIDTH, HEIGHT), (10, 40));
        assert_eq!((VISIBLE_TOP, VISIBLE_ROWS), (20, 20));
    }

    #[test]
    fn an_empty_matrix_is_empty() {
        let mut matrix = Matrix::new();
        assert!(matrix.is_empty());
        matrix.set(0, 39, Some(PieceKind::T));
        assert!(!matrix.is_empty());
        assert_eq!(matrix.get(0, 39), Some(PieceKind::T));
    }

    #[test]
    fn walls_and_floor_are_solid_and_the_buffer_ceiling_is_not() {
        let matrix = Matrix::new();
        assert!(matrix.is_filled(-1, 20), "left of the wall is solid");
        assert!(matrix.is_filled(WIDTH, 20), "right of the wall is solid");
        assert!(matrix.is_filled(4, HEIGHT), "below the floor is solid");
        assert!(!matrix.is_filled(4, -1), "above row 0 is empty (§9.1)");
        assert!(!matrix.is_filled(4, 0), "row 0 is an ordinary empty row");
        // A corner: out of bounds on both axes, and the solid rule wins.
        assert!(matrix.is_filled(-1, -1));
    }

    #[test]
    fn a_piece_on_an_empty_board_does_not_collide_at_spawn() {
        let matrix = Matrix::new();
        for kind in PieceKind::ALL {
            assert!(!matrix.collides(kind, kind.spawn_origin(), Rotation::North));
        }
    }

    #[test]
    fn collision_detects_the_floor_and_both_walls() {
        let matrix = Matrix::new();
        // An O at the floor: origin row 38 puts its minos in rows 38 and 39.
        assert!(!matrix.collides(PieceKind::O, Point::new(4, 38), Rotation::North));
        assert!(matrix.collides(PieceKind::O, Point::new(4, 39), Rotation::North));
        // An I in North occupies box row 1, columns 0..=3 of its box.
        assert!(!matrix.collides(PieceKind::I, Point::new(0, 20), Rotation::North));
        assert!(matrix.collides(PieceKind::I, Point::new(-1, 20), Rotation::North));
        assert!(!matrix.collides(PieceKind::I, Point::new(6, 20), Rotation::North));
        assert!(matrix.collides(PieceKind::I, Point::new(7, 20), Rotation::North));
    }

    #[test]
    fn collision_detects_locked_cells() {
        let matrix = from_bottom_rows(&[
            "..........",
            "####.#####", // a one-wide well at column 4
        ]);
        assert!(
            !matrix.row_is_full(39),
            "the well leaves row 39 short a cell"
        );
        assert!(!matrix.row_is_full(38));
        // A T resting on the surface, its lowest minos in row 38.
        assert!(!matrix.collides(PieceKind::T, Point::new(3, 37), Rotation::North));
        assert!(matrix.collides(PieceKind::T, Point::new(3, 38), Rotation::North));
        // Vertical I down the well at column 4 reaches the floor.
        assert!(!matrix.collides(PieceKind::I, Point::new(2, 36), Rotation::East));
    }

    #[test]
    fn a_full_row_is_detected() {
        let matrix = from_bottom_rows(&["##########", ".########."]);
        assert!(matrix.row_is_full(38));
        assert!(!matrix.row_is_full(39));
        assert!(!matrix.row_is_full(0));
    }
}
