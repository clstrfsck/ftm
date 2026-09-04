//! T-spin and T-spin mini detection (§9.13).
//!
//! The "last action was a rotation" flag must survive a hard drop, and kick test
//! 5 always means a proper T-spin. This is the most commonly botched rule in the
//! specification.

use crate::core::game::ActivePiece;
use crate::core::geometry::{Point, Rotation};
use crate::core::matrix::Matrix;
use crate::core::piece::PieceKind;

/// The kick index that always means a proper T-spin (§9.13): test 5, the last
/// row of the kick table, counted from zero as [`srs::try_rotate`] reports it.
///
/// [`srs::try_rotate`]: crate::core::srs::try_rotate
pub const KICK_TEST_5: u8 = 4;

/// What a lock was, as far as §9.13 is concerned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TSpin {
    /// Not a T-spin: the wrong piece, the wrong last action, or too few corners.
    #[default]
    None,
    /// Exactly one front corner and both back corners.
    Mini,
    /// Both front corners and at least one back corner — or kick test 5,
    /// whatever the corners say.
    Proper,
}

/// The four corners of the T's 3 × 3 bounding box, as offsets from its origin.
///
/// Indexed by the four constants below, which is what lets [`FRONT_CORNERS`]
/// transcribe the §9.13 table by name rather than by arithmetic.
const CORNERS: [Point; 4] = [
    Point::new(0, 0),
    Point::new(2, 0),
    Point::new(0, 2),
    Point::new(2, 2),
];

const TOP_LEFT: usize = 0;
const TOP_RIGHT: usize = 1;
const BOTTOM_LEFT: usize = 2;
const BOTTOM_RIGHT: usize = 3;

/// The §9.13 table: the two corners adjacent to the side the T points towards.
/// The other two are the back corners.
const fn front_corners(rotation: Rotation) -> [usize; 2] {
    match rotation {
        Rotation::North => [TOP_LEFT, TOP_RIGHT],
        Rotation::East => [TOP_RIGHT, BOTTOM_RIGHT],
        Rotation::South => [BOTTOM_LEFT, BOTTOM_RIGHT],
        Rotation::West => [TOP_LEFT, BOTTOM_LEFT],
    }
}

/// Classify a lock (§9.13).
///
/// Called at the moment the piece locks, before any row is removed (§9.12 step
/// 2). The T's own minos never fall on the corners of its bounding box, so it
/// makes no difference whether they have been written into `matrix` yet.
pub fn classify(matrix: &Matrix, piece: &ActivePiece) -> TSpin {
    // §9.13: a lock is examined only if the piece is a T *and* the last
    // successful action applied to it was a rotation. Gravity, a shift and a
    // hold all disqualify it; a hard drop does not, because `Game::hard_drop`
    // carries the flag across.
    if piece.kind != PieceKind::T || !piece.last_action_was_rotation {
        return TSpin::None;
    }

    // §9.13: "regardless of the above". The classic T-spin triple fills only
    // three corners *of the wrong two*, and scores as a proper spin because of
    // the kick it needed, not because of what surrounds it.
    if piece.last_kick_index == KICK_TEST_5 {
        return TSpin::Proper;
    }

    let front = front_corners(piece.rotation);
    let occupied = |corner: usize| {
        let at = piece.origin.translate(CORNERS[corner].x, CORNERS[corner].y);
        // §9.13's "occupied" is exactly the collision rule of §9.1: solid to the
        // left, right and below the matrix, empty above row 0.
        matrix.is_filled(at.x, at.y)
    };
    let front_count = front.iter().filter(|&&c| occupied(c)).count();
    let back_count = (0..CORNERS.len())
        .filter(|c| !front.contains(c) && occupied(*c))
        .count();

    match (front_count, back_count) {
        (2, 1..) => TSpin::Proper,
        (1, 2) => TSpin::Mini,
        _ => TSpin::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::matrix::tests::from_bottom_rows;

    /// A T that has just rotated into `rotation` at `origin` with `kick`.
    fn spun_t(origin: Point, rotation: Rotation, kick: u8) -> ActivePiece {
        ActivePiece {
            kind: PieceKind::T,
            origin,
            rotation,
            last_action_was_rotation: true,
            last_kick_index: kick,
        }
    }

    /// A board with every corner of a 3 × 3 box at `(3, 37)` filled, and the
    /// cells a T would occupy there left free.
    ///
    /// ```text
    /// row 37   # . #
    /// row 38   . . .
    /// row 39   # . #
    /// ```
    fn all_four_corners() -> Matrix {
        from_bottom_rows(&["...#.#....", "..........", "...#.#...."])
    }

    #[test]
    fn a_piece_that_is_not_a_t_is_never_a_spin() {
        // §9.13's first precondition. Every other kind is checked, because
        // "the piece is a T" is one comparison away from "the piece is not an
        // O", and a board that surrounds the box does not care which.
        let matrix = all_four_corners();
        for kind in PieceKind::ALL.into_iter().filter(|&k| k != PieceKind::T) {
            let piece = ActivePiece {
                kind,
                ..spun_t(Point::new(3, 37), Rotation::North, 0)
            };
            assert_eq!(classify(&matrix, &piece), TSpin::None, "{kind:?}");
        }
    }

    #[test]
    fn a_t_that_was_not_rotated_last_is_never_a_spin() {
        // §9.13's second precondition, in isolation: the corners say proper and
        // the answer is still None. T9's end-to-end half lives in `game`.
        let matrix = all_four_corners();
        let mut piece = spun_t(Point::new(3, 37), Rotation::North, 0);
        assert_eq!(classify(&matrix, &piece), TSpin::Proper);
        piece.last_action_was_rotation = false;
        assert_eq!(classify(&matrix, &piece), TSpin::None);
    }

    #[test]
    fn kick_test_five_is_a_proper_spin_whatever_the_corners_say() {
        // The override, isolated from any board that would earn it honestly:
        // an empty matrix, nothing within reach of a corner, kick index 4.
        let matrix = Matrix::new();
        let piece = spun_t(Point::new(3, 20), Rotation::North, KICK_TEST_5);
        assert_eq!(classify(&matrix, &piece), TSpin::Proper);
        // ... and index 3 -- test 4 -- earns nothing.
        let piece = spun_t(Point::new(3, 20), Rotation::North, KICK_TEST_5 - 1);
        assert_eq!(classify(&matrix, &piece), TSpin::None);
    }

    #[test]
    fn three_corners_are_needed_before_any_of_this_matters() {
        // Two corners is never a spin, however they are arranged.
        let matrix = from_bottom_rows(&["...#.#....", "..........", ".........."]);
        for rotation in Rotation::ALL {
            let piece = spun_t(Point::new(3, 37), rotation, 0);
            assert_eq!(classify(&matrix, &piece), TSpin::None, "{rotation:?}");
        }
    }

    #[test]
    fn the_front_corners_are_the_ones_the_t_points_towards() {
        // The §9.13 table, read off a board with exactly three corners filled.
        // Which three decides proper against mini, and the orientation decides
        // which of them are the front pair -- so the same board is a proper
        // spin for two orientations and a mini for the other two.
        //
        // ```text
        // row 37   . . #     top-left is the empty one
        // row 38   . . .
        // row 39   # . #
        // ```
        let matrix = from_bottom_rows(&[".....#....", "..........", "...#.#...."]);
        let origin = Point::new(3, 37);
        for (rotation, expected) in [
            // Front is the top pair; the top-left is missing.
            (Rotation::North, TSpin::Mini),
            // Front is the right pair; both are filled.
            (Rotation::East, TSpin::Proper),
            // Front is the bottom pair; both are filled.
            (Rotation::South, TSpin::Proper),
            // Front is the left pair; the top-left is missing.
            (Rotation::West, TSpin::Mini),
        ] {
            let piece = spun_t(origin, rotation, 0);
            assert_eq!(classify(&matrix, &piece), expected, "{rotation:?}");
        }
    }

    #[test]
    fn a_wall_and_the_floor_count_as_occupied_corners() {
        // §9.13: a corner outside the matrix's left, right or bottom bounds is
        // occupied. A T flat against the left wall in East orientation has its
        // whole back pair off the edge, so two filled cells in column 1 are a
        // proper spin.
        let matrix = from_bottom_rows(&[".#........", "..........", ".#........"]);
        let piece = spun_t(Point::new(-1, 37), Rotation::East, 0);
        assert_eq!(classify(&matrix, &piece), TSpin::Proper);

        // Fill only the lower of the two and the front pair is down to one:
        // the canonical mini against a wall.
        let matrix = from_bottom_rows(&["..........", "..........", ".#........"]);
        assert_eq!(classify(&matrix, &piece), TSpin::Mini);

        // The floor is solid in the same way: a T sitting on it in North
        // orientation has both bottom corners below row 39.
        let matrix = from_bottom_rows(&["...#.#....", ".........."]);
        let piece = spun_t(Point::new(3, 38), Rotation::North, 0);
        assert_eq!(classify(&matrix, &piece), TSpin::Proper);
    }

    #[test]
    fn corners_above_row_zero_are_empty() {
        // §9.13: "Cells above row 0 count as empty." A T in South orientation
        // has its back pair on top, and up there the back pair can never be
        // filled -- so both front corners are not enough.
        let mut matrix = Matrix::new();
        matrix.set(3, 1, Some(PieceKind::I));
        matrix.set(5, 1, Some(PieceKind::I));
        let piece = spun_t(Point::new(3, -1), Rotation::South, 0);
        assert_eq!(classify(&matrix, &piece), TSpin::None);

        // The same shape two rows lower, where the back pair is inside the
        // matrix and filled, is the proper spin it looks like.
        matrix.set(3, 3, Some(PieceKind::I));
        matrix.set(5, 3, Some(PieceKind::I));
        let piece = spun_t(Point::new(3, 1), Rotation::South, 0);
        assert_eq!(classify(&matrix, &piece), TSpin::Proper);
    }
}
