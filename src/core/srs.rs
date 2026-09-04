//! Super Rotation System: wall-kick tables and rotation resolution (§9.5).
//!
//! **The kick tables in §9.5 are already converted to the y-down convention of
//! §5. They are transcribed here as written; do not negate them again.** A
//! positive `dy` moves the piece **down**.
//!
//! A rotation tests each offset of its table row in order and takes the first
//! that fits. The **index** of the accepted offset is returned along with the
//! new origin: §9.13 needs it, because kick test 5 always means a proper T-spin.

use crate::core::geometry::{Point, Rotation};
use crate::core::matrix::Matrix;
use crate::core::piece::PieceKind;

/// Offsets tried per rotation, for every piece but `O` (§9.5).
pub const KICK_TESTS: usize = 5;
/// The eight quarter-turn transitions the kick tables cover.
pub const TRANSITIONS: usize = 8;

/// The order of the rows in both §9.5 tables: `0→R`, `R→0`, `R→2`, `2→R`,
/// `2→L`, `L→2`, `L→0`, `0→L`, where `0 = North`, `R = East`, `2 = South` and
/// `L = West`.
const TRANSITION_ORDER: [(Rotation, Rotation); TRANSITIONS] = [
    (Rotation::North, Rotation::East),
    (Rotation::East, Rotation::North),
    (Rotation::East, Rotation::South),
    (Rotation::South, Rotation::East),
    (Rotation::South, Rotation::West),
    (Rotation::West, Rotation::South),
    (Rotation::West, Rotation::North),
    (Rotation::North, Rotation::West),
];

/// Kick table for `J`, `L`, `S`, `T`, `Z` (§9.5).
#[rustfmt::skip]
const JLSTZ_KICKS: [[Point; KICK_TESTS]; TRANSITIONS] = [
    // From → To      Test 1              Test 2               Test 3                Test 4               Test 5
    /* 0 → R */ [Point::new(0, 0), Point::new(-1,  0), Point::new(-1, -1), Point::new(0,  2), Point::new(-1,  2)],
    /* R → 0 */ [Point::new(0, 0), Point::new( 1,  0), Point::new( 1,  1), Point::new(0, -2), Point::new( 1, -2)],
    /* R → 2 */ [Point::new(0, 0), Point::new( 1,  0), Point::new( 1,  1), Point::new(0, -2), Point::new( 1, -2)],
    /* 2 → R */ [Point::new(0, 0), Point::new(-1,  0), Point::new(-1, -1), Point::new(0,  2), Point::new(-1,  2)],
    /* 2 → L */ [Point::new(0, 0), Point::new( 1,  0), Point::new( 1, -1), Point::new(0,  2), Point::new( 1,  2)],
    /* L → 2 */ [Point::new(0, 0), Point::new(-1,  0), Point::new(-1,  1), Point::new(0, -2), Point::new(-1, -2)],
    /* L → 0 */ [Point::new(0, 0), Point::new(-1,  0), Point::new(-1,  1), Point::new(0, -2), Point::new(-1, -2)],
    /* 0 → L */ [Point::new(0, 0), Point::new( 1,  0), Point::new( 1, -1), Point::new(0,  2), Point::new( 1,  2)],
];

/// Kick table for `I` (§9.5).
#[rustfmt::skip]
const I_KICKS: [[Point; KICK_TESTS]; TRANSITIONS] = [
    // From → To      Test 1              Test 2               Test 3                Test 4               Test 5
    /* 0 → R */ [Point::new(0, 0), Point::new(-2,  0), Point::new( 1,  0), Point::new(-2,  1), Point::new( 1, -2)],
    /* R → 0 */ [Point::new(0, 0), Point::new( 2,  0), Point::new(-1,  0), Point::new( 2, -1), Point::new(-1,  2)],
    /* R → 2 */ [Point::new(0, 0), Point::new(-1,  0), Point::new( 2,  0), Point::new(-1, -2), Point::new( 2,  1)],
    /* 2 → R */ [Point::new(0, 0), Point::new( 1,  0), Point::new(-2,  0), Point::new( 1,  2), Point::new(-2, -1)],
    /* 2 → L */ [Point::new(0, 0), Point::new( 2,  0), Point::new(-1,  0), Point::new( 2, -1), Point::new(-1,  2)],
    /* L → 2 */ [Point::new(0, 0), Point::new(-2,  0), Point::new( 1,  0), Point::new(-2,  1), Point::new( 1, -2)],
    /* L → 0 */ [Point::new(0, 0), Point::new(-1,  0), Point::new( 2,  0), Point::new(-1, -2), Point::new( 2,  1)],
    /* 0 → L */ [Point::new(0, 0), Point::new( 1,  0), Point::new(-2,  0), Point::new( 1,  2), Point::new(-2, -1)],
];

/// `O` never kicks: its rotation always succeeds at `(0, 0)` (§9.5).
const O_KICKS: [Point; 1] = [Point::new(0, 0)];

/// The row index of a quarter-turn transition, or `None` if `from` and `to` are
/// not one quarter turn apart.
fn transition_index(from: Rotation, to: Rotation) -> Option<usize> {
    TRANSITION_ORDER.iter().position(|&t| t == (from, to))
}

/// The offsets tried for this piece and transition, in order.
///
/// `None` when the transition is not a quarter turn — 180 degrees has its own
/// path in [`try_rotate_180`], with no kick tests at all.
pub fn kicks(kind: PieceKind, from: Rotation, to: Rotation) -> Option<&'static [Point]> {
    let row = transition_index(from, to)?;
    Some(match kind {
        PieceKind::O => &O_KICKS,
        PieceKind::I => &I_KICKS[row],
        _ => &JLSTZ_KICKS[row],
    })
}

/// Attempt a quarter-turn rotation from `from` to `to` (§9.5).
///
/// Returns the accepted origin and the **index of the offset used**, or `None`
/// if every offset collided — in which case the caller must change nothing: a
/// failed rotation resets no timer and scores nothing.
///
/// The kick index is not a diagnostic. §9.13 makes index 4 (test 5) a proper
/// T-spin regardless of the corner rule, so it must be threaded through to the
/// lock, not dropped here.
pub fn try_rotate(
    matrix: &Matrix,
    kind: PieceKind,
    origin: Point,
    from: Rotation,
    to: Rotation,
) -> Option<(Point, u8)> {
    let tests = kicks(kind, from, to)?;
    tests.iter().enumerate().find_map(|(index, offset)| {
        let kicked = origin.translate(offset.x, offset.y);
        (!matrix.collides(kind, kicked, to)).then_some((kicked, index as u8))
    })
}

/// Attempt a 180-degree rotation (§9.5).
///
/// An extension beyond the guideline: the piece is tested only at `(0, 0)` for
/// the opposite orientation, and there are no kick tests. The `kick_index` is
/// therefore always 0, which is what lets §9.13 treat a 180 as a T-spin only by
/// the corner rule.
///
/// Availability is `allow_180_rotation`, enforced at the input boundary (§10.1)
/// so that a disabled key cannot reset a lock-delay timer as a side effect.
pub fn try_rotate_180(
    matrix: &Matrix,
    kind: PieceKind,
    origin: Point,
    from: Rotation,
) -> Option<(Point, u8)> {
    let to = from.opposite();
    (!matrix.collides(kind, origin, to)).then_some((origin, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::matrix::tests::from_bottom_rows;

    /// Every origin at which `kind` fits in `rotation` on an empty board.
    fn free_origins(kind: PieceKind, rotation: Rotation) -> Vec<Point> {
        let matrix = Matrix::new();
        let size = kind.box_size();
        (-size..crate::core::matrix::WIDTH)
            .flat_map(|x| (-size..crate::core::matrix::HEIGHT).map(move |y| Point::new(x, y)))
            .filter(|&origin| !matrix.collides(kind, origin, rotation))
            .collect()
    }

    #[test]
    fn both_tables_are_eight_rows_of_five() {
        assert_eq!(JLSTZ_KICKS.len(), 8);
        assert_eq!(I_KICKS.len(), 8);
        for row in JLSTZ_KICKS.iter().chain(I_KICKS.iter()) {
            assert_eq!(row.len(), 5);
            assert_eq!(row[0], Point::new(0, 0), "test 1 is always no offset");
        }
        // Every quarter turn is covered exactly once, and no transition is a
        // 180 or a no-op.
        for from in Rotation::ALL {
            for to in [from.cw(), from.ccw()] {
                assert!(transition_index(from, to).is_some(), "{from:?} -> {to:?}");
            }
            assert_eq!(transition_index(from, from), None);
            assert_eq!(transition_index(from, from.opposite()), None);
        }
    }

    #[test]
    fn o_never_kicks() {
        // §9.5: `O` always succeeds at (0, 0), in every direction.
        for to in [Rotation::East, Rotation::West] {
            assert_eq!(kicks(PieceKind::O, Rotation::North, to).unwrap().len(), 1);
        }
        let matrix = Matrix::new();
        let origin = PieceKind::O.spawn_origin();
        assert_eq!(
            try_rotate(
                &matrix,
                PieceKind::O,
                origin,
                Rotation::North,
                Rotation::East
            ),
            Some((origin, 0)),
        );
    }

    #[test]
    fn a_rotation_and_its_inverse_return_the_origin() {
        // T2. Where no kick is needed in either direction -- which is every
        // position clear of the walls -- a quarter turn and its inverse are
        // exact opposites. (Against a wall they are not, and must not be: see
        // `an_i_against_the_wall_does_not_round_trip`.)
        let matrix = Matrix::new();
        for kind in PieceKind::ALL {
            for from in Rotation::ALL {
                for to in [from.cw(), from.ccw()] {
                    for origin in free_origins(kind, from) {
                        if matrix.collides(kind, origin, to) {
                            continue;
                        }
                        let (turned, index) =
                            try_rotate(&matrix, kind, origin, from, to).expect("must fit");
                        assert_eq!((turned, index), (origin, 0), "{kind:?} {from:?} -> {to:?}");
                        assert_eq!(
                            try_rotate(&matrix, kind, turned, to, from),
                            Some((origin, 0)),
                            "{kind:?} {to:?} -> {from:?} did not return to {origin:?}",
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn spawn_rotations_round_trip_on_an_empty_board() {
        // T2, in the spec's own words: 0 -> R followed by R -> 0.
        let matrix = Matrix::new();
        for kind in PieceKind::ALL {
            let origin = kind.spawn_origin();
            let (east, _) = try_rotate(&matrix, kind, origin, Rotation::North, Rotation::East)
                .expect("spawn rotation fits on an empty board");
            let (back, _) = try_rotate(&matrix, kind, east, Rotation::East, Rotation::North)
                .expect("and back again");
            assert_eq!(back, origin, "{kind:?}");
        }
    }

    #[test]
    fn an_i_against_the_left_wall_kicks_clear_of_it() {
        // T3. A vertical I in the leftmost column, resting on the floor: its box
        // origin is (-2, 36), two columns outside the matrix, because the East
        // pattern occupies box column 2.
        let matrix = Matrix::new();
        let origin = Point::new(-2, 36);
        assert!(!matrix.collides(PieceKind::I, origin, Rotation::East));
        // R -> 0 for I: (0,0) is inside the wall, so test 2, (+2, 0), takes it.
        assert_eq!(
            try_rotate(
                &matrix,
                PieceKind::I,
                origin,
                Rotation::East,
                Rotation::North
            ),
            Some((Point::new(0, 36), 1)),
        );
    }

    #[test]
    fn an_i_against_the_wall_does_not_round_trip() {
        // The counterpart to the property test above, and not a defect: once the
        // wall has kicked the I clear, rotating back needs no kick, so it does
        // not return to where it started. Pinned here so nobody "fixes" it.
        let matrix = Matrix::new();
        let (north, _) = try_rotate(
            &matrix,
            PieceKind::I,
            Point::new(-2, 36),
            Rotation::East,
            Rotation::North,
        )
        .unwrap();
        assert_eq!(
            try_rotate(
                &matrix,
                PieceKind::I,
                north,
                Rotation::North,
                Rotation::East
            ),
            Some((Point::new(0, 36), 0)),
        );
    }

    /// The T-spin triple set-up (T3): a three-deep well at column 7 with an
    /// overhang at (7, 35), and a nook at (6, 38).
    ///
    /// ```text
    ///        0123456789
    /// row 35 #####..###
    /// row 36 #####...##
    /// row 37 #######.##
    /// row 38 ######..##
    /// row 39 #######.##
    /// ```
    ///
    /// The T slides right under the overhang in North, then rotates
    /// anticlockwise, diving two rows down and one column right into the well.
    const TST_ROWS: [&str; 5] = [
        "#####..###",
        "#####...##",
        "#######.##",
        "######..##",
        "#######.##",
    ];

    #[test]
    fn a_t_in_a_one_wide_well_performs_the_t_spin_triple_kick() {
        let matrix = from_bottom_rows(&TST_ROWS);
        let start = Point::new(5, 35);
        assert!(
            !matrix.collides(PieceKind::T, start, Rotation::North),
            "the T must fit where it starts",
        );

        let (origin, index) = try_rotate(
            &matrix,
            PieceKind::T,
            start,
            Rotation::North,
            Rotation::West,
        )
        .expect("the T-spin triple kick must succeed");

        // Test 5 of `0 -> L` is (+1, +2). §9.13 makes kick index 4 a proper
        // T-spin on its own, whatever the corner rule says.
        assert_eq!((origin, index), (Point::new(6, 37), 4));

        // Tests 1 to 4 must each have been blocked, or this fixture is not
        // testing what it claims to.
        for (i, offset) in kicks(PieceKind::T, Rotation::North, Rotation::West).unwrap()[..4]
            .iter()
            .enumerate()
        {
            let candidate = start.translate(offset.x, offset.y);
            assert!(
                matrix.collides(PieceKind::T, candidate, Rotation::West),
                "kick test {} should have been blocked",
                i + 1,
            );
        }
    }

    #[test]
    fn the_t_spin_triple_fills_three_rows() {
        // What makes it a *triple*: the four minos complete rows 37, 38 and 39.
        let mut matrix = from_bottom_rows(&TST_ROWS);
        let (origin, _) = try_rotate(
            &matrix,
            PieceKind::T,
            Point::new(5, 35),
            Rotation::North,
            Rotation::West,
        )
        .unwrap();
        for mino in PieceKind::T.minos(origin, Rotation::West) {
            matrix.set(mino.x, mino.y, Some(PieceKind::T));
        }
        for row in 37..=39 {
            assert!(matrix.row_is_full(row), "row {row} should be complete");
        }
        assert!(!matrix.row_is_full(36));
    }

    #[test]
    fn a_failed_rotation_changes_nothing() {
        // §9.5 step 4: if no offset fits, the rotation fails outright. A T lying
        // flat in a sealed pocket on the floor has nowhere to turn: down and
        // sideways are solid, and so are the four rows above it. Note that a
        // well with open sky above is *not* such a case -- kick test 4 lifts the
        // piece straight out of it.
        let matrix = from_bottom_rows(&[
            "##########",
            "##########",
            "##########",
            "##########",
            "####.#####",
            "###...####",
        ]);
        let origin = Point::new(3, 38);
        assert!(!matrix.collides(PieceKind::T, origin, Rotation::North));
        for to in [Rotation::East, Rotation::West] {
            assert_eq!(
                try_rotate(&matrix, PieceKind::T, origin, Rotation::North, to),
                None,
                "North -> {to:?} should have no room",
            );
        }
        assert_eq!(
            try_rotate_180(&matrix, PieceKind::T, origin, Rotation::North),
            None,
        );
    }

    #[test]
    fn a_180_rotation_takes_no_kick_tests() {
        // §9.5: tested only at (0, 0), and the kick index is always 0 so that
        // §9.13 can only ever call it a T-spin by the corner rule.
        let matrix = Matrix::new();
        let origin = PieceKind::T.spawn_origin();
        assert_eq!(
            try_rotate_180(&matrix, PieceKind::T, origin, Rotation::North),
            Some((origin, 0)),
        );
        // A T in a flat-side-down pocket cannot turn over: South needs the cell
        // below the middle, and it is filled.
        let blocked = from_bottom_rows(&["..........", "###...####", "##########"]);
        assert_eq!(
            try_rotate_180(&blocked, PieceKind::T, Point::new(3, 37), Rotation::North),
            None,
        );
        // And 180 is not reachable through the quarter-turn path.
        assert_eq!(
            try_rotate(
                &matrix,
                PieceKind::T,
                origin,
                Rotation::North,
                Rotation::South
            ),
            None,
        );
    }
}
