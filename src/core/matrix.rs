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

/// The most rows one lock can complete: a piece has four minos, and every
/// newly completed row must contain at least one of them.
pub const MAX_CLEARED_ROWS: usize = 4;

/// The rows completed by one lock, top to bottom (§9.12).
///
/// Fixed capacity and `Copy`, so finding the rows costs no allocation on the
/// one tick in ten that clears anything (§12.8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClearedRows {
    rows: [i32; MAX_CLEARED_ROWS],
    len: u8,
}

impl ClearedRows {
    /// Append a row, or report that there was no space.
    fn push(&mut self, row: i32) -> bool {
        if usize::from(self.len) == MAX_CLEARED_ROWS {
            return false;
        }
        self.rows[usize::from(self.len)] = row;
        self.len += 1;
        true
    }

    /// The rows, top to bottom.
    pub fn as_slice(&self) -> &[i32] {
        &self.rows[..usize::from(self.len)]
    }

    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn contains(&self, row: i32) -> bool {
        self.as_slice().contains(&row)
    }
}

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

    /// The completed rows, top to bottom (§9.12 step 3).
    ///
    /// At most four, since every newly completed row must contain one of the
    /// four minos just locked. A board handed in with completed rows already on
    /// it is a fixture error, not a game state, and trips a debug assertion.
    pub fn full_rows(&self) -> ClearedRows {
        let mut rows = ClearedRows::default();
        for y in 0..HEIGHT {
            if self.row_is_full(y) && !rows.push(y) {
                debug_assert!(false, "more than {MAX_CLEARED_ROWS} rows were complete");
                break;
            }
        }
        rows
    }

    /// Remove the given rows; everything above each one shifts down by a row,
    /// and empty rows appear at the top of the buffer zone (§9.12 step 6).
    ///
    /// Naive gravity: no cascading, no sticky cells. A cell's column never
    /// changes, and the order of the surviving rows is preserved.
    pub fn clear_rows(&mut self, cleared: &ClearedRows) {
        if cleared.is_empty() {
            return;
        }
        let mut write = HEIGHT - 1;
        for read in (0..HEIGHT).rev() {
            if cleared.contains(read) {
                continue;
            }
            self.rows[write as usize] = self.rows[read as usize];
            write -= 1;
        }
        for y in 0..=write {
            self.rows[y as usize] = [None; WIDTH as usize];
        }
    }

    /// Whether the matrix holds no locked cells at all — the perfect-clear
    /// condition of §9.15.
    pub fn is_empty(&self) -> bool {
        self.rows.iter().all(|row| row.iter().all(Option::is_none))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Build a matrix from a picture of its bottom rows, the last string being
    /// row 39. `.` is empty; `#` is an anonymous filled cell; a piece letter
    /// fills the cell with that kind, for the fixtures where colour matters.
    /// Every fixture in the later stages is written this way, so it lives here
    /// rather than in each test.
    pub(crate) fn from_bottom_rows(rows: &[&str]) -> Matrix {
        let mut matrix = Matrix::new();
        for (i, row) in rows.iter().rev().enumerate() {
            let y = HEIGHT - 1 - i as i32;
            assert_eq!(
                row.chars().count(),
                WIDTH as usize,
                "row {row:?} is not 10 wide"
            );
            for (x, c) in row.chars().enumerate() {
                let kind = match c {
                    '.' => None,
                    '#' => Some(PieceKind::I),
                    _ => Some(
                        *PieceKind::ALL
                            .iter()
                            .find(|k| k.glyph() == c)
                            .unwrap_or_else(|| panic!("{c:?} is not a piece letter")),
                    ),
                };
                matrix.set(x as i32, y, kind);
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

    /// The matrix's bottom `n` rows as a picture, for comparing against a
    /// fixture.
    fn bottom_rows(matrix: &Matrix, n: usize) -> Vec<String> {
        ((HEIGHT - n as i32)..HEIGHT)
            .map(|y| {
                (0..WIDTH)
                    .map(|x| match matrix.get(x, y) {
                        None => '.',
                        Some(kind) => kind.glyph(),
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn rows_collapse_with_naive_gravity() {
        // T8. The row below the cleared one stays put; the rows above it shift
        // down by exactly one, keeping their contents and their order.
        let mut matrix =
            from_bottom_rows(&["T........T", "SS......SS", "##########", "L........L"]);
        let cleared = matrix.full_rows();
        assert_eq!(cleared.as_slice(), [38]);
        matrix.clear_rows(&cleared);
        assert_eq!(
            bottom_rows(&matrix, 4),
            ["..........", "T........T", "SS......SS", "L........L"],
        );
    }

    #[test]
    fn a_clear_preserves_rows_above_and_below_in_order() {
        // T8's named case: a full row with filled rows on both sides.
        let mut matrix = from_bottom_rows(&[
            "I.........",
            "JJ........",
            "##########",
            "ZZZ.......",
            "TTTT......",
        ]);
        let cleared = matrix.full_rows();
        assert_eq!(cleared.len(), 1);
        matrix.clear_rows(&cleared);
        assert_eq!(
            bottom_rows(&matrix, 5),
            [
                "..........",
                "I.........",
                "JJ........",
                "ZZZ.......",
                "TTTT......",
            ],
        );
    }

    #[test]
    fn four_rows_clear_at_once() {
        // T8. The most a single lock can complete, and the rows are reported
        // top to bottom.
        let mut matrix = from_bottom_rows(&[
            "L.........",
            "##########",
            "##########",
            "##########",
            "##########",
        ]);
        let cleared = matrix.full_rows();
        assert_eq!(cleared.as_slice(), [36, 37, 38, 39]);
        matrix.clear_rows(&cleared);
        assert_eq!(bottom_rows(&matrix, 2), ["..........", "L........."]);
        assert!(!matrix.is_empty());
    }

    #[test]
    fn non_adjacent_rows_clear_together() {
        // Rows 37 and 39 clear; row 38 survives and lands on the floor.
        let mut matrix = from_bottom_rows(&["##########", "TT........", "##########"]);
        let cleared = matrix.full_rows();
        assert_eq!(cleared.as_slice(), [37, 39]);
        matrix.clear_rows(&cleared);
        assert_eq!(bottom_rows(&matrix, 2), ["..........", "TT........"]);
    }

    #[test]
    fn clearing_the_only_rows_empties_the_matrix() {
        // The perfect clear of §9.15, seen from the matrix's side.
        let mut matrix = from_bottom_rows(&["##########", "##########"]);
        let cleared = matrix.full_rows();
        matrix.clear_rows(&cleared);
        assert!(matrix.is_empty());
    }

    #[test]
    fn clearing_nothing_changes_nothing() {
        let matrix = from_bottom_rows(&["#########.", "TT........"]);
        let mut after = matrix.clone();
        let cleared = after.full_rows();
        assert!(cleared.is_empty());
        after.clear_rows(&cleared);
        assert_eq!(after, matrix);
    }

    #[test]
    fn a_full_row_is_detected() {
        let matrix = from_bottom_rows(&["##########", ".########."]);
        assert!(matrix.row_is_full(38));
        assert!(!matrix.row_is_full(39));
        assert!(!matrix.row_is_full(0));
    }
}
