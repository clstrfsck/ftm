//! `GameView`: the serialisable render model (§12.7).
//!
//! The renderer draws only from this and never reads `Game`. Clipping to the
//! visible rows happens here, not in the renderer: the view carries rows
//! `20..=39` of the matrix as rows `0..=19` of itself, so nothing downstream of
//! the core has to know that a buffer zone exists.
//!
//! The view is **derived, never authoritative**. [`Game::view`] takes `&self`,
//! which is the compile-time half of that promise; T15 is the other half.

use serde::{Deserialize, Serialize};

use crate::core::events::OFF_SCREEN;
use crate::core::game::{ActivePiece, Game, PlayState};
use crate::core::gravity::ONE_ROW;
use crate::core::matrix::{VISIBLE_ROWS, VISIBLE_TOP, WIDTH};
use crate::core::piece::{MINOS, PieceKind};

/// The visible width, as a `usize` for indexing.
pub const VIEW_WIDTH: usize = WIDTH as usize;
/// The visible height, as a `usize` for indexing.
pub const VIEW_HEIGHT: usize = VISIBLE_ROWS as usize;

/// One tetromino, ready to draw (§12.7).
///
/// `cells` are absolute visible-field coordinates `(col, row)`, already clipped:
/// a mino above the visible field is omitted, encoded as [`OFF_SCREEN`], so the
/// array may hold fewer than four drawable cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceView {
    pub kind: PieceKind,
    pub cells: [(u8, u8); MINOS],
}

/// Everything any screen needs to draw, and nothing else (§12.7).
///
/// Owned and flat, so it can be serialised, cached, queued or diffed against the
/// previous frame — which is what buys §19 its wire format for free.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameView {
    /// The locked cells of the visible rows only, `rows[row][col]`.
    pub rows: [[Option<PieceKind>; VIEW_WIDTH]; VIEW_HEIGHT],
    /// The falling piece; absent during the clear and entry delays.
    pub current: Option<PieceView>,
    /// The landing position; absent when the ghost is disabled.
    pub ghost: Option<PieceView>,
    pub hold: Option<PieceKind>,
    pub hold_locked: bool,
    /// The preview queue: exactly `preview_count` entries.
    pub next: Vec<PieceKind>,
    pub score: u64,
    pub level: u32,
    pub lines: u32,
    /// Elapsed game time, in ticks.
    pub ticks: u64,
    pub pieces: u32,
    pub combo: i32,
    pub back_to_back: bool,
    pub state: PlayState,
}

/// The four figures §12.4's debug strip needs that [`GameView`] does not carry.
///
/// A **separate** view type, built by [`Game::debug`], rather than a field on
/// `GameView`. Two reasons, and the second is the load-bearing one:
///
/// - `show_debug` is a presentation setting (§6.5), so the core cannot be told
///   whether to produce it. A field on `GameView` would be filled on every
///   frame of every game, whether or not anyone was looking.
/// - The bag beyond `preview_count` is **hidden information**. Under §19
///   `GameView` is what a server sends a player; a field on it that reveals the
///   rest of the bag is a field that has to be remembered and stripped before
///   transmission. A separate type is not sent by accident.
///
/// §12.7's layering rule is untouched: the renderer is handed one of these,
/// never a `Game`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugView {
    /// Rows per tick x 1000 — G, in integers, because the rules carry no
    /// floating point (§9.9). At level 1 this is 16.
    pub milli_g: u32,
    /// The fall period in 16.16 ticks per row (§9.9). A *period*, not a rate.
    pub fall_period: u32,
    /// Lock-delay ticks remaining (§9.11), absent while the piece is airborne.
    pub lock_delay: Option<u32>,
    /// What is left of the current bag (§9.6) — see the note above.
    pub bag: Vec<PieceKind>,
}

/// A matrix cell in visible-field coordinates, or [`OFF_SCREEN`] if it is above
/// the visible field.
///
/// Rows below the field cannot occur: a mino at row 40 or beyond would have
/// collided with the floor.
pub fn to_visible(x: i32, y: i32) -> (u8, u8) {
    let row = y - VISIBLE_TOP;
    if row < 0 {
        return OFF_SCREEN;
    }
    debug_assert!((0..VISIBLE_ROWS).contains(&row) && (0..WIDTH).contains(&x));
    (x as u8, row as u8)
}

impl PieceView {
    /// The drawable form of a piece in play.
    pub fn of(piece: &ActivePiece) -> Self {
        let minos = piece.minos();
        let mut cells = [OFF_SCREEN; MINOS];
        for (cell, mino) in cells.iter_mut().zip(minos) {
            *cell = to_visible(mino.x, mino.y);
        }
        Self {
            kind: piece.kind,
            cells,
        }
    }
}

impl Game {
    /// A snapshot of everything the screen needs (§12.7).
    ///
    /// `&self`: building the view must not mutate the game.
    pub fn view(&self) -> GameView {
        let mut rows = [[None; VIEW_WIDTH]; VIEW_HEIGHT];
        for (row, cells) in rows.iter_mut().enumerate() {
            let y = VISIBLE_TOP + row as i32;
            for (col, cell) in cells.iter_mut().enumerate() {
                *cell = self.matrix().get(col as i32, y);
            }
        }
        GameView {
            rows,
            current: self.current().as_ref().map(PieceView::of),
            ghost: self.ghost().as_ref().map(PieceView::of),
            hold: self.held(),
            hold_locked: self.hold_locked(),
            next: self.preview().collect(),
            score: self.score(),
            level: self.level(),
            lines: self.lines(),
            ticks: self.ticks(),
            pieces: self.pieces(),
            combo: self.combo(),
            back_to_back: self.back_to_back(),
            state: self.state(),
        }
    }

    /// The figures §12.4's debug strip needs, on the same `&self` terms as
    /// [`Game::view`] (§12.7).
    ///
    /// Built only when it is asked for, which is what keeps it off the wire.
    pub fn debug(&self) -> DebugView {
        let fall_period = self.fall_period();
        DebugView {
            // Rows per tick is `ONE_ROW / fall_period`; a thousand times that,
            // in integers, is what the strip prints. `fall_period` is never 0
            // (§9.9), so the division is safe.
            milli_g: (u64::from(ONE_ROW) * 1000 / u64::from(fall_period)) as u32,
            fall_period,
            lock_delay: self.lock_delay_remaining(),
            bag: self.bag_remaining().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::game::tests::{new_game, place, set_matrix};
    use crate::core::game::{Action, TickInput};
    use crate::core::geometry::{Point, Rotation};
    use crate::core::matrix::Matrix;

    /// Advance both games through the same script, so any difference in what
    /// they did — including a difference in the randomiser's state, which no
    /// single view shows — turns into a difference in the views.
    fn run(game: &mut Game, ticks: u32) {
        for i in 0..ticks {
            let input = match i % 17 {
                0 => TickInput::action(Action::HardDrop),
                5 => TickInput::action(Action::RotateCw),
                _ => TickInput::default(),
            };
            game.tick(&input, &mut Vec::new());
        }
    }

    #[test]
    fn building_the_view_does_not_touch_the_game() {
        // T15. `view(&self)` settles this at compile time for the fields; what
        // this adds is the randomiser, whose state no view reveals. If `view`
        // drew a piece from the bag, the two games would diverge.
        let mut looked_at = new_game(7);
        let mut untouched = looked_at.clone();
        for _ in 0..50 {
            let _ = looked_at.view();
        }
        run(&mut looked_at, 400);
        run(&mut untouched, 400);
        assert_eq!(looked_at.view(), untouched.view());
    }

    #[test]
    fn the_view_carries_the_visible_rows_only() {
        // T15: rows 20..=39 of the matrix arrive as rows 0..=19 of the view,
        // and the buffer zone never leaves the core.
        let mut game = new_game(11);
        let mut matrix = Matrix::new();
        matrix.set(0, 19, Some(PieceKind::S)); // the last buffer row
        matrix.set(1, 20, Some(PieceKind::Z)); // the first visible row
        matrix.set(2, 39, Some(PieceKind::L)); // the floor
        set_matrix(&mut game, matrix);
        let view = game.view();

        assert_eq!(view.rows.len(), VIEW_HEIGHT, "twenty rows, not forty");
        assert_eq!(view.rows[0][0], None, "row 19 is not in the view");
        assert_eq!(view.rows[0][1], Some(PieceKind::Z));
        assert_eq!(view.rows[VIEW_HEIGHT - 1][2], Some(PieceKind::L));
    }

    #[test]
    fn a_piece_straddling_row_twenty_is_clipped_by_the_core() {
        // T15, and the reason `cells` may hold fewer than four drawable cells:
        // the renderer must never have to know a buffer zone exists.
        let mut game = new_game(13);
        // A vertical I in column 3 spans four rows; put two of them above the
        // visible field, at rows 18, 19, 20 and 21.
        place(&mut game, PieceKind::I, Point::new(1, 18), Rotation::East);
        let piece = game.view().current.expect("a piece is in play");

        assert_eq!(piece.kind, PieceKind::I);
        assert_eq!(
            piece.cells,
            [OFF_SCREEN, OFF_SCREEN, (3, 0), (3, 1)],
            "the two buffer-zone minos are omitted, not renumbered",
        );
    }

    #[test]
    fn the_view_reports_what_the_screen_needs() {
        let game = new_game(17);
        let view = game.view();
        assert_eq!(view.state, PlayState::Falling);
        assert_eq!(view.ticks, 0);
        assert_eq!(view.pieces, 0);
        assert_eq!(view.score, 0);
        assert_eq!(view.level, 1);
        assert_eq!(view.combo, -1);
        assert!(!view.back_to_back);
        assert_eq!(view.next.len(), 5, "the default preview_count (§6.3)");
        assert!(view.current.is_some());
        assert!(view.ghost.is_some(), "the ghost is on by default (§6.3)");
        assert_eq!(view.hold, None, "the hold slot starts empty (§9.7)");
        assert!(!view.hold_locked);
    }

    #[test]
    fn no_piece_is_shown_during_the_clear_delay() {
        // §12.7: `current` is absent during the clear and entry delays, and the
        // completed row is still drawn while it flashes (§9.12 step 5).
        let mut game = new_game(19);
        set_matrix(
            &mut game,
            crate::core::matrix::tests::from_bottom_rows(&["####.#####"]),
        );
        place(&mut game, PieceKind::I, Point::new(2, 36), Rotation::East);
        game.tick(&TickInput::action(Action::HardDrop), &mut Vec::new());

        let view = game.view();
        assert_eq!(view.state, PlayState::Clearing);
        assert_eq!(view.current, None);
        assert_eq!(
            view.rows[VIEW_HEIGHT - 1][4],
            Some(PieceKind::I),
            "the completed row is still on screen",
        );
    }

    #[test]
    fn the_view_survives_a_serde_round_trip() {
        // T15, and the §19 wire format: the view is owned and flat, so it
        // serialises without reference to anything in the core.
        let mut game = new_game(23);
        run(&mut game, 300);
        let view = game.view();
        let json = serde_json::to_string(&view).expect("the view serialises");
        let back: GameView = serde_json::from_str(&json).expect("and comes back");
        assert_eq!(back, view);
    }
    #[test]
    fn the_debug_view_reports_the_state_the_strip_needs() {
        // §12.4's strip, and §12.7's reason for a second view type: `bag` is
        // hidden information and must not ride along on `GameView`.
        let game = new_game(29);
        let debug = game.debug();
        // Level 1 falls one row every 60 ticks exactly (§9.9), so a tick is a
        // sixtieth of a row: 16 thousandths of a G.
        assert_eq!(debug.fall_period, 60 * ONE_ROW);
        assert_eq!(debug.milli_g, 16);
        assert_eq!(debug.lock_delay, None, "the first piece is airborne");
        assert!(debug.bag.len() <= 7);

        let json = serde_json::to_string(&debug).expect("serialises");
        let back: DebugView = serde_json::from_str(&json).expect("and comes back");
        assert_eq!(back, debug);
    }

    #[test]
    fn asking_for_the_debug_view_does_not_touch_the_game() {
        // The same promise `view` makes (§12.7): `&self`, and the randomiser
        // is not drawn from.
        let mut looked_at = new_game(31);
        let mut untouched = looked_at.clone();
        for _ in 0..50 {
            let _ = looked_at.debug();
        }
        run(&mut looked_at, 400);
        run(&mut untouched, 400);
        assert_eq!(looked_at.view(), untouched.view());
        assert_eq!(looked_at.debug(), untouched.debug());
    }

    #[test]
    fn a_grounded_piece_reports_its_lock_delay() {
        // §9.11: the strip's most useful figure, and the one that is absent
        // rather than zero while the piece is still falling.
        let mut game = new_game(37);
        for _ in 0..60 * 30 {
            game.tick(&TickInput::default(), &mut Vec::new());
            if game.debug().lock_delay.is_some() {
                break;
            }
        }
        let left = game.debug().lock_delay.expect("the piece landed");
        assert!(left <= 30, "at most the default lock delay: {left}");
    }
}
