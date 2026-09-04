//! `Game` state and `Game::tick` — the single entry point to the core (§15.1).
//!
//! No clock and no I/O (§3.1): time enters only as calls to [`Game::tick`], one
//! fixed 1/60 s tick at a time. Given the same `RulesConfig`, seed and input
//! sequence the state is byte-identical, however the ticks are batched (§19.4).

use crate::config::RulesConfig;
use crate::core::bag::Bag;
use crate::core::geometry::{Point, Rotation};
use crate::core::gravity::{self, Gravity};
use crate::core::lockdown::{LockDown, LockOutcome};
use crate::core::matrix::{ClearedRows, Matrix, VISIBLE_TOP};
use crate::core::piece::PieceKind;
use crate::core::srs;

/// An edge-triggered input, acted on once per press (§10.2).
///
/// The core owns only the first five; the rest belong to the shell's state
/// machine (§7) and are listed here so that one enum describes the whole
/// keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    RotateCw,
    RotateCcw,
    Rotate180,
    Hold,
    HardDrop,
    Pause,
    Restart,
    Quit,
    MenuUp,
    MenuDown,
    MenuSelect,
    MenuBack,
}

/// A horizontal move, already resolved by the shell's DAS/ARR (§10.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    Left,
    Right,
}

/// The actions delivered in one tick.
///
/// A fixed-capacity inline list rather than a `Vec`: the core is called sixty
/// times a second and must not allocate to do nothing (§12.8). Four is more
/// distinct actions than a player can produce in 16 ms; a fifth is dropped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Actions([Option<Action>; 4]);

impl Actions {
    /// Append an action, or report that the tick was already full.
    pub fn push(&mut self, action: Action) -> bool {
        for slot in &mut self.0 {
            if slot.is_none() {
                *slot = Some(action);
                return true;
            }
        }
        false
    }

    /// The actions, in the order the shell delivered them.
    pub fn iter(&self) -> impl Iterator<Item = Action> + '_ {
        self.0.iter().flatten().copied()
    }
}

impl FromIterator<Action> for Actions {
    fn from_iter<I: IntoIterator<Item = Action>>(iter: I) -> Self {
        let mut actions = Actions::default();
        for action in iter {
            actions.push(action);
        }
        actions
    }
}

/// Everything the core needs to know about one tick (§15.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickInput {
    /// Edge-triggered, this tick only.
    pub actions: Actions,
    /// Level-triggered: whether the soft-drop key is down now.
    pub soft_drop: bool,
    /// The direction held, already DAS-resolved by the shell (§10.3).
    pub shift: Option<Shift>,
    /// How many cells to shift this tick; ARR of 0 can ask for more than one.
    pub shift_cells: u8,
}

impl TickInput {
    /// A tick with a single action and nothing held.
    pub fn action(action: Action) -> Self {
        Self {
            actions: [action].into_iter().collect(),
            ..Self::default()
        }
    }

    /// A tick shifting one cell in a direction.
    pub fn shift(shift: Shift) -> Self {
        Self {
            shift: Some(shift),
            shift_cells: 1,
            ..Self::default()
        }
    }
}

/// What the game is doing right now (§12.7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayState {
    /// A piece is in play.
    #[default]
    Falling,
    /// Completed rows are on screen, waiting out the line-clear delay (§9.12).
    Clearing,
    /// The entry delay between one piece and the next (ARE, §9.12).
    Entry,
    /// Block Out or Lock Out has ended the game (§9.16).
    ToppedOut,
}

/// The piece under the player's control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivePiece {
    pub kind: PieceKind,
    pub origin: Point,
    pub rotation: Rotation,
    /// Whether the last successful action was a rotation, and the kick index it
    /// used (§9.13).
    ///
    /// This flag **must survive a hard drop**: a T rotated into its slot and
    /// then hard-dropped is still a T-spin. It is the most commonly botched
    /// rule in the specification.
    pub last_action_was_rotation: bool,
    pub last_kick_index: u8,
}

impl ActivePiece {
    /// The piece's minos, in matrix coordinates.
    pub fn minos(&self) -> [Point; 4] {
        self.kind.minos(self.origin, self.rotation)
    }

    /// The lowest row any of its minos occupies. Lowest means numerically
    /// largest (§5).
    pub fn lowest_row(&self) -> i32 {
        self.minos().iter().map(|p| p.y).max().unwrap_or(0)
    }
}

/// The rules core.
#[derive(Clone, Debug)]
pub struct Game {
    rules: RulesConfig,
    matrix: Matrix,
    bag: Bag,
    gravity: Gravity,
    lock: LockDown,
    current: Option<ActivePiece>,
    state: PlayState,
    /// Ticks left in `Clearing` or `Entry`.
    state_timer: u32,
    /// The rows waiting out the line-clear delay, still drawn (§9.12 step 5).
    clearing_rows: ClearedRows,
    ticks: u64,
    pieces: u32,
    lines: u32,
    score: u64,
    combo: i32,
    back_to_back: bool,
}

impl Game {
    /// A new game at `start_level`, with the first piece already spawned.
    pub fn new(rules: RulesConfig, seed: u64) -> Self {
        let level = rules.start_level;
        let mut game = Self {
            bag: Bag::new(seed, rules.preview_count),
            lock: LockDown::new(rules.lock_down, rules.lock_delay_ticks),
            gravity: Gravity::new(level),
            matrix: Matrix::new(),
            current: None,
            state: PlayState::Falling,
            state_timer: 0,
            clearing_rows: ClearedRows::default(),
            ticks: 0,
            pieces: 0,
            lines: 0,
            score: 0,
            combo: -1,
            back_to_back: false,
            rules,
        };
        game.spawn();
        game
    }

    // -- read-only accessors, until the view model lands in Stage 5 ---------

    pub fn state(&self) -> PlayState {
        self.state
    }

    pub fn matrix(&self) -> &Matrix {
        &self.matrix
    }

    pub fn current(&self) -> Option<ActivePiece> {
        self.current
    }

    pub fn level(&self) -> u32 {
        self.gravity.level()
    }

    pub fn lines(&self) -> u32 {
        self.lines
    }

    pub fn score(&self) -> u64 {
        self.score
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    pub fn pieces(&self) -> u32 {
        self.pieces
    }

    pub fn is_over(&self) -> bool {
        self.state == PlayState::ToppedOut
    }

    // -- the tick ----------------------------------------------------------

    /// Advance exactly one tick (§15.1).
    // TODO(stage 5): takes `out: &mut Vec<GameEvent>` and emits §12.8 events.
    pub fn tick(&mut self, input: &TickInput) {
        if self.state == PlayState::ToppedOut {
            return;
        }
        self.ticks += 1;
        match self.state {
            PlayState::Clearing => self.tick_clearing(),
            PlayState::Entry => self.tick_entry(),
            PlayState::Falling => self.tick_falling(input),
            PlayState::ToppedOut => {}
        }
    }

    /// Wait out the line-clear delay, then collapse the rows (§9.12 steps 5-7).
    fn tick_clearing(&mut self) {
        self.state_timer = self.state_timer.saturating_sub(1);
        if self.state_timer > 0 {
            return;
        }
        self.matrix.clear_rows(&self.clearing_rows);
        self.lines += self.clearing_rows.len() as u32;
        self.gravity.set_level(gravity::level_after_clear(
            self.gravity.level(),
            self.lines,
            self.rules.lines_per_level,
        ));
        self.clearing_rows = ClearedRows::default();
        self.begin_entry_delay();
    }

    /// Wait out the entry delay, then spawn (§9.12 steps 8-9).
    fn tick_entry(&mut self) {
        self.state_timer = self.state_timer.saturating_sub(1);
        if self.state_timer == 0 {
            self.spawn();
        }
    }

    /// A tick with a piece in play.
    ///
    /// Order matters and is fixed: the player's actions, then their movement,
    /// then gravity, then the lock-down timer. Input first is what lets a piece
    /// be steered on the tick it would otherwise lock.
    fn tick_falling(&mut self, input: &TickInput) {
        for action in input.actions.iter() {
            match action {
                Action::RotateCw => self.rotate(Rotation::cw),
                Action::RotateCcw => self.rotate(Rotation::ccw),
                Action::Rotate180 => self.rotate_180(),
                Action::HardDrop => {
                    self.hard_drop();
                    return;
                }
                // TODO(stage 8): Action::Hold, gated on `hold_enabled` (§9.7).
                _ => {}
            }
        }

        if let Some(shift) = input.shift {
            let dx = match shift {
                Shift::Left => -1,
                Shift::Right => 1,
            };
            for _ in 0..input.shift_cells {
                if !self.try_move(dx, 0) {
                    break;
                }
            }
        }

        self.apply_gravity(input.soft_drop);
        self.settle();
    }

    /// Accrue gravity and fall as far as it allows (§9.9, §9.10).
    fn apply_gravity(&mut self, soft_drop: bool) {
        let factor = soft_drop.then_some(self.rules.soft_drop_factor);
        let rows = self.gravity.accrue(factor);
        for _ in 0..rows {
            if !self.try_move(0, 1) {
                // §9.9: a blocked step clears the accumulator and hands over to
                // the lock-down machine.
                self.gravity.reset_accumulator();
                break;
            }
        }
        // TODO(stage 7): 1 point per row descended under soft drop (§9.10).
    }

    /// Update the lock-down timer and lock if it expires (§9.11).
    fn settle(&mut self) {
        let resting = !self.can_move(0, 1);
        if resting {
            self.lock.land();
        } else {
            self.lock.lift();
        }
        if self.lock.tick(resting) == LockOutcome::Lock {
            self.lock_piece();
        }
    }

    // -- piece movement ----------------------------------------------------

    /// Whether the current piece could move by `(dx, dy)`.
    fn can_move(&self, dx: i32, dy: i32) -> bool {
        self.current.is_some_and(|piece| {
            !self
                .matrix
                .collides(piece.kind, piece.origin.translate(dx, dy), piece.rotation)
        })
    }

    /// Move the current piece if the destination is clear, reporting success.
    fn try_move(&mut self, dx: i32, dy: i32) -> bool {
        if !self.can_move(dx, dy) {
            return false;
        }
        let Some(piece) = self.current.as_mut() else {
            return false;
        };
        piece.origin = piece.origin.translate(dx, dy);
        piece.last_action_was_rotation = false;
        let lowest = piece.lowest_row();
        self.lock.observe_row(lowest);
        if dy == 0 {
            // §9.11: a horizontal move while landed spends a reset. A downward
            // move does not -- reaching new ground restores the budget instead.
            self.lock.on_move();
        }
        true
    }

    /// Rotate a quarter turn, honouring the kick table (§9.5).
    fn rotate(&mut self, turn: fn(Rotation) -> Rotation) {
        let Some(piece) = self.current else { return };
        let to = turn(piece.rotation);
        let Some((origin, kick_index)) =
            srs::try_rotate(&self.matrix, piece.kind, piece.origin, piece.rotation, to)
        else {
            // §9.5 step 4: a failed rotation changes nothing and resets no
            // timer.
            return;
        };
        self.accept_rotation(origin, to, kick_index);
    }

    /// Rotate 180 degrees, if the rules allow it (§9.5).
    ///
    /// The gate is also enforced at the input boundary (§10.1) so a disabled key
    /// never reaches the core; this is the backstop for a caller that ignores
    /// that, and it must return before touching any timer.
    fn rotate_180(&mut self) {
        if !self.rules.allow_180_rotation {
            return;
        }
        let Some(piece) = self.current else { return };
        let Some((origin, kick_index)) =
            srs::try_rotate_180(&self.matrix, piece.kind, piece.origin, piece.rotation)
        else {
            return;
        };
        self.accept_rotation(origin, piece.rotation.opposite(), kick_index);
    }

    /// Commit a successful rotation and its lock-down consequences.
    fn accept_rotation(&mut self, origin: Point, rotation: Rotation, kick_index: u8) {
        let Some(piece) = self.current.as_mut() else {
            return;
        };
        piece.origin = origin;
        piece.rotation = rotation;
        piece.last_action_was_rotation = true;
        piece.last_kick_index = kick_index;
        let lowest = piece.lowest_row();
        self.lock.observe_row(lowest);
        self.lock.on_move();
    }

    /// Drop to the floor and lock immediately, skipping lock delay (§9.10).
    ///
    /// §9.13 requires the "last action was a rotation" flag to **survive** the
    /// drop: a T rotated into its slot and then hard-dropped is still a T-spin.
    /// Every `try_move` clears that flag, so it is carried across by hand here.
    /// This is the rule everybody gets wrong.
    fn hard_drop(&mut self) {
        let Some(before) = self.current else { return };
        let mut rows = 0;
        while self.try_move(0, 1) {
            rows += 1;
        }
        let _ = rows;
        // TODO(stage 7): 2 points per row descended (§9.10).
        if let Some(piece) = self.current.as_mut() {
            piece.last_action_was_rotation = before.last_action_was_rotation;
            piece.last_kick_index = before.last_kick_index;
        }
        self.lock_piece();
    }

    // -- locking and spawning ----------------------------------------------

    /// Write the piece into the matrix and deal with what follows (§9.12).
    fn lock_piece(&mut self) {
        let Some(piece) = self.current.take() else {
            return;
        };
        let minos = piece.minos();
        // §9.16: Lock Out is decided before the minos are written, on where the
        // piece came to rest.
        let locked_out = minos.iter().all(|p| p.y < VISIBLE_TOP);
        for mino in minos {
            self.matrix.set(mino.x, mino.y, Some(piece.kind));
        }
        self.pieces += 1;
        // TODO(stage 7): classify the T-spin here, *before* clearing rows
        // (§9.12 step 2), and award the score (§9.14).
        // TODO(stage 8): clear the hold lock-out here -- on lock, not on spawn
        // (§9.7).

        if locked_out {
            self.state = PlayState::ToppedOut;
            return;
        }

        self.clearing_rows = self.matrix.full_rows();
        if self.clearing_rows.is_empty() {
            self.begin_entry_delay();
        } else {
            self.state = PlayState::Clearing;
            self.state_timer = self.rules.line_clear_delay_ticks;
        }
    }

    /// Start the entry delay, or spawn at once when it is zero (§9.12 step 8).
    ///
    /// `entry_delay_ticks` may legitimately be 0 (§6.6), which means the next
    /// piece enters on this very tick rather than one tick later.
    fn begin_entry_delay(&mut self) {
        if self.rules.entry_delay_ticks == 0 {
            self.spawn();
        } else {
            self.state = PlayState::Entry;
            self.state_timer = self.rules.entry_delay_ticks;
        }
    }

    /// Take the next piece from the queue and place it (§9.4).
    fn spawn(&mut self) {
        let kind = self.bag.next_piece();
        let origin = kind.spawn_origin();
        if self.matrix.collides(kind, origin, Rotation::North) {
            // §9.16 Block Out. The drop-one attempt is skipped, and the piece is
            // still shown where it could not fit.
            self.current = Some(ActivePiece {
                kind,
                origin,
                rotation: Rotation::North,
                last_action_was_rotation: false,
                last_kick_index: 0,
            });
            self.state = PlayState::ToppedOut;
            return;
        }

        let mut piece = ActivePiece {
            kind,
            origin,
            rotation: Rotation::North,
            last_action_was_rotation: false,
            last_kick_index: 0,
        };
        // §9.4: immediately after spawning the piece drops one row if it can.
        // One unconditional attempt, not a gravity step: it scores nothing and
        // resets no timer.
        if !self
            .matrix
            .collides(kind, origin.translate(0, 1), Rotation::North)
        {
            piece.origin = origin.translate(0, 1);
        }

        self.current = Some(piece);
        self.state = PlayState::Falling;
        self.state_timer = 0;
        self.gravity.reset_accumulator();
        self.lock = LockDown::new(self.rules.lock_down, self.rules.lock_delay_ticks);
        self.lock.observe_row(piece.lowest_row());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GameplaySettings, TimingSettings};
    use crate::core::matrix::{HEIGHT, WIDTH, tests::from_bottom_rows};

    /// A game with the default rules under a fixed seed.
    fn new_game(seed: u64) -> Game {
        Game::new(RulesConfig::default(), seed)
    }

    /// A game whose rules differ from the defaults.
    fn new_game_with(gameplay: GameplaySettings, timing: TimingSettings, seed: u64) -> Game {
        Game::new(RulesConfig::from_settings(&gameplay, &timing), seed)
    }

    /// Run `n` ticks with nothing held.
    fn idle(game: &mut Game, n: u32) {
        let input = TickInput::default();
        for _ in 0..n {
            game.tick(&input);
        }
    }

    /// Force a particular piece into play at a particular place, bypassing the
    /// bag. Board fixtures need a known piece, and the bag is not steerable.
    fn place(game: &mut Game, kind: PieceKind, origin: Point, rotation: Rotation) {
        game.current = Some(ActivePiece {
            kind,
            origin,
            rotation,
            last_action_was_rotation: false,
            last_kick_index: 0,
        });
        game.state = PlayState::Falling;
        game.lock = LockDown::new(game.rules.lock_down, game.rules.lock_delay_ticks);
        game.lock.observe_row(game.current.unwrap().lowest_row());
    }

    fn bottom_rows(game: &Game, n: usize) -> Vec<String> {
        ((HEIGHT - n as i32)..HEIGHT)
            .map(|y| {
                (0..WIDTH)
                    .map(|x| match game.matrix().get(x, y) {
                        None => '.',
                        Some(kind) => kind.glyph(),
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_new_game_has_a_piece_one_row_below_spawn() {
        // §9.4: the piece drops one row immediately after spawning, so on an
        // empty board its lowest minos sit in row 20, the topmost visible row.
        let game = new_game(1);
        let piece = game.current().expect("a piece is in play");
        assert_eq!(piece.rotation, Rotation::North);
        assert_eq!(piece.origin, piece.kind.spawn_origin().translate(0, 1));
        assert_eq!(piece.lowest_row(), 20);
        assert_eq!(game.state(), PlayState::Falling);
        assert_eq!(game.ticks(), 0);
    }

    #[test]
    fn the_spawn_drop_is_not_a_gravity_step() {
        // §9.4: "a single unconditional attempt, not a gravity step ... it
        // neither scores nor resets any timer." If it were counted as gravity,
        // the piece would reach row 21 on tick 60 rather than row 21 on tick 60
        // from row 20.
        let mut game = new_game(1);
        assert_eq!(game.current().unwrap().lowest_row(), 20);
        idle(&mut game, 59);
        assert_eq!(game.current().unwrap().lowest_row(), 20, "not yet");
        idle(&mut game, 1);
        assert_eq!(game.current().unwrap().lowest_row(), 21, "on tick 60");
    }

    #[test]
    fn a_piece_falls_one_row_every_sixty_ticks_at_level_one() {
        // T6, end to end through the game rather than the accumulator alone.
        let mut game = new_game(3);
        for row in 1..=10 {
            idle(&mut game, 60);
            assert_eq!(game.current().unwrap().lowest_row(), 20 + row);
        }
    }

    #[test]
    fn a_piece_lands_and_locks_after_the_lock_delay() {
        // An O resting on the floor lands on the first tick and locks thirty
        // ticks later, the default lock_delay_ms of 500 (§6.6).
        let mut game = new_game(5);
        place(&mut game, PieceKind::O, Point::new(4, 38), Rotation::North);
        idle(&mut game, 1);
        assert!(game.lock.is_landed(), "a blocked step lands the piece");
        assert_eq!(game.lock.remaining(), Some(29));
        idle(&mut game, 28);
        assert_eq!(game.pieces(), 0, "not locked yet");
        idle(&mut game, 1);
        assert_eq!(game.pieces(), 1, "locked on the 30th tick after landing");
        assert_eq!(game.matrix().get(4, 39), Some(PieceKind::O));
    }

    #[test]
    fn a_piece_falls_all_the_way_to_the_floor_under_gravity_alone() {
        // Nineteen rows from the spawn row to the floor, one per sixty ticks.
        let mut game = new_game(5);
        place(&mut game, PieceKind::O, Point::new(4, 19), Rotation::North);
        for row in 1..=19 {
            idle(&mut game, 60);
            assert_eq!(
                game.current().expect("still falling").lowest_row(),
                20 + row,
                "after {row} rows",
            );
        }
    }

    #[test]
    fn a_hard_drop_locks_at_once() {
        // §9.10: lock delay is skipped entirely.
        let mut game = new_game(9);
        let kind = game.current().unwrap().kind;
        game.tick(&TickInput::action(Action::HardDrop));
        assert_eq!(game.pieces(), 1);
        assert!(
            game.current().is_some(),
            "the next piece is already in play"
        );
        assert_ne!(game.matrix().get(4, 39), None, "it landed on the floor");
        let _ = kind;
    }

    #[test]
    fn a_hard_drop_of_zero_rows_still_locks() {
        // §9.10: "A hard drop of zero rows (piece already resting) still locks
        // the piece."
        let mut game = new_game(11);
        // Fill row 20 either side so the spawned piece is already resting.
        let mut matrix = Matrix::new();
        for x in 0..WIDTH {
            matrix.set(x, 21, Some(PieceKind::I));
        }
        game.matrix = matrix;
        place(&mut game, PieceKind::O, Point::new(4, 19), Rotation::North);
        assert!(!game.can_move(0, 1), "already resting");
        game.tick(&TickInput::action(Action::HardDrop));
        assert_eq!(game.pieces(), 1);
        assert_eq!(game.matrix().get(4, 20), Some(PieceKind::O));
    }

    #[test]
    fn the_rotation_flag_survives_a_hard_drop() {
        // §9.13, and the rule the plan singles out as the one everybody gets
        // wrong. The flag is read at lock time, so a rotate-then-drop must
        // arrive at the lock still marked as a rotation.
        let mut game = new_game(13);
        place(&mut game, PieceKind::T, Point::new(3, 20), Rotation::North);
        game.tick(&TickInput::action(Action::RotateCw));
        let rotated = game.current().unwrap();
        assert!(rotated.last_action_was_rotation);

        // Drop it, and inspect the piece the moment before it locks by doing
        // the same descent by hand.
        let mut checked = game.clone();
        while checked.try_move(0, 1) {}
        assert!(
            !checked.current().unwrap().last_action_was_rotation,
            "a plain descent clears the flag, which is why hard_drop restores it",
        );

        game.tick(&TickInput::action(Action::HardDrop));
        assert_eq!(game.pieces(), 1);
    }

    #[test]
    fn a_moved_piece_is_no_longer_marked_as_rotated() {
        // The other half of §9.13's precondition: a shift clears the flag, so a
        // T slid into a slot cannot claim a T-spin.
        let mut game = new_game(17);
        place(&mut game, PieceKind::T, Point::new(3, 20), Rotation::North);
        game.tick(&TickInput::action(Action::RotateCw));
        assert!(game.current().unwrap().last_action_was_rotation);
        game.tick(&TickInput::shift(Shift::Left));
        assert!(!game.current().unwrap().last_action_was_rotation);
    }

    #[test]
    fn a_failed_rotation_resets_no_timer() {
        // §9.5 step 4. A rotation that cannot happen must not buy lock-delay
        // time, or a piece in a tight spot could be held up indefinitely.
        let mut game = new_game(19);
        game.matrix = from_bottom_rows(&[
            "##########",
            "##########",
            "##########",
            "##########",
            "####.#####",
            "###...####",
        ]);
        place(&mut game, PieceKind::T, Point::new(3, 38), Rotation::North);
        game.tick(&TickInput::default());
        let landed_at = game.lock.remaining();
        assert_eq!(landed_at, Some(29), "landed and counting");
        game.tick(&TickInput::action(Action::RotateCw));
        assert_eq!(game.lock.remaining(), Some(28), "no reset was granted");
        assert_eq!(game.lock.resets_used(), 0);
    }

    #[test]
    fn a_shift_while_landed_resets_the_timer() {
        let mut game = new_game(23);
        place(&mut game, PieceKind::O, Point::new(4, 38), Rotation::North);
        game.tick(&TickInput::default());
        assert_eq!(game.lock.remaining(), Some(29));
        game.tick(&TickInput::shift(Shift::Left));
        assert_eq!(
            game.lock.remaining(),
            Some(29),
            "reset to full, then ticked"
        );
        assert_eq!(game.lock.resets_used(), 1);
    }

    #[test]
    fn a_completed_row_clears_after_the_line_clear_delay() {
        // §9.12 steps 5-7, and T8 through the game.
        let mut game = new_game(29);
        game.matrix = from_bottom_rows(&["####.#####"]);
        place(&mut game, PieceKind::I, Point::new(2, 36), Rotation::East);
        game.tick(&TickInput::action(Action::HardDrop));

        assert_eq!(game.state(), PlayState::Clearing);
        assert_eq!(game.lines(), 0, "the count waits for the collapse");
        assert_eq!(
            game.matrix().get(4, 39),
            Some(PieceKind::I),
            "the completed row is still drawn during the flash",
        );

        idle(&mut game, 14);
        assert_eq!(game.state(), PlayState::Clearing, "15 ticks at 250 ms");
        idle(&mut game, 1);
        assert_eq!(game.lines(), 1);
        assert_eq!(
            game.state(),
            PlayState::Falling,
            "entry delay is 0 by default"
        );
        // Naive gravity (§9.12 step 6): only the completed row goes. The I's
        // other three minos shift down a row and keep their column.
        assert_eq!(
            bottom_rows(&game, 4),
            ["..........", "....I.....", "....I.....", "....I....."],
        );
    }

    #[test]
    fn the_entry_delay_holds_the_next_piece_back() {
        // §9.12 step 8. With ARE at zero the next piece enters on the same tick;
        // with ARE set it waits, and no piece is in play meanwhile.
        let mut game = new_game_with(
            GameplaySettings::default(),
            TimingSettings {
                entry_delay_ms: 100,
                ..TimingSettings::default()
            },
            31,
        );
        game.tick(&TickInput::action(Action::HardDrop));
        assert_eq!(game.state(), PlayState::Entry);
        assert!(game.current().is_none(), "nothing is in play during ARE");
        idle(&mut game, 5);
        assert_eq!(game.state(), PlayState::Entry);
        idle(&mut game, 1);
        assert_eq!(game.state(), PlayState::Falling);
        assert!(game.current().is_some());
    }

    #[test]
    fn four_rows_clear_and_the_level_advances() {
        // §9.9 level progression driven by a real clear.
        let mut game = new_game_with(
            GameplaySettings {
                lines_per_level: 4,
                ..GameplaySettings::default()
            },
            TimingSettings::default(),
            37,
        );
        game.matrix = from_bottom_rows(&["####.#####", "####.#####", "####.#####", "####.#####"]);
        place(&mut game, PieceKind::I, Point::new(2, 32), Rotation::East);
        game.tick(&TickInput::action(Action::HardDrop));
        assert_eq!(game.state(), PlayState::Clearing);
        idle(&mut game, 15);
        assert_eq!(game.lines(), 4);
        assert_eq!(game.level(), 2, "four lines at four per level");
        assert!(game.matrix().is_empty(), "a perfect clear, as it happens");
    }

    #[test]
    fn block_out_ends_the_game() {
        // T11: a newly spawned piece overlaps a locked mino at its spawn
        // position (§9.16).
        let mut game = new_game(41);
        let mut matrix = Matrix::new();
        for x in 0..WIDTH {
            for y in 18..=19 {
                matrix.set(x, y, Some(PieceKind::I));
            }
        }
        game.matrix = matrix;
        place(&mut game, PieceKind::O, Point::new(4, 15), Rotation::North);
        game.tick(&TickInput::action(Action::HardDrop));
        assert!(game.is_over(), "the next piece cannot spawn");
        assert_eq!(game.state(), PlayState::ToppedOut);
    }

    #[test]
    fn a_topped_out_game_ignores_further_ticks() {
        let mut game = new_game(43);
        game.state = PlayState::ToppedOut;
        let before = game.ticks();
        idle(&mut game, 100);
        assert_eq!(game.ticks(), before, "time itself stops");
    }

    #[test]
    fn lock_out_ends_the_game() {
        // T11: a piece locks with all four minos above row 20 (§9.16).
        let mut game = new_game(47);
        let mut matrix = Matrix::new();
        for x in 0..WIDTH {
            matrix.set(x, 20, Some(PieceKind::I));
        }
        game.matrix = matrix;
        // An O resting on row 20's stack sits entirely in rows 18 and 19.
        place(&mut game, PieceKind::O, Point::new(4, 18), Rotation::North);
        assert!(!game.can_move(0, 1));
        game.tick(&TickInput::action(Action::HardDrop));
        assert!(
            game.is_over(),
            "all four minos locked inside the buffer zone"
        );
    }

    #[test]
    fn a_piece_straddling_row_twenty_is_not_a_lock_out() {
        // §9.16 says *all four* minos. Three above and one visible is a legal,
        // if desperate, placement.
        let mut game = new_game(53);
        let mut matrix = Matrix::new();
        // A floor, so the notch at column 0 is one cell deep and the I cannot
        // fall through it. Column 9 is left open throughout, so nothing here is
        // a completed row: this test is about topping out, not clearing. The
        // notch is at the edge so that the piece which spawns afterwards has
        // somewhere to go -- otherwise this would end in a Block Out and prove
        // nothing about Lock Out.
        for y in 21..HEIGHT {
            for x in 0..WIDTH - 1 {
                matrix.set(x, y, Some(PieceKind::I));
            }
        }
        for x in 1..WIDTH - 1 {
            matrix.set(x, 20, Some(PieceKind::I));
        }
        game.matrix = matrix;
        // A vertical I in column 0 comes to rest with minos in rows 17, 18, 19
        // and 20: three inside the buffer zone and one just visible.
        place(&mut game, PieceKind::I, Point::new(-2, 14), Rotation::East);
        game.tick(&TickInput::action(Action::HardDrop));
        assert!(!game.is_over(), "one mino reached row 20");
        assert_eq!(game.matrix().get(0, 20), Some(PieceKind::I));
        assert_eq!(game.matrix().get(0, 17), Some(PieceKind::I));
    }

    /// Drop a piece after shifting it, then wait out any clear or entry delay,
    /// so the caller is handed back a game with the next piece already in play.
    fn drop_piece(game: &mut Game, shift: Option<Shift>, cells: u8) {
        if let Some(shift) = shift {
            game.tick(&TickInput {
                shift: Some(shift),
                shift_cells: cells,
                ..TickInput::default()
            });
        }
        game.tick(&TickInput::action(Action::HardDrop));
        while game.current().is_none() && !game.is_over() {
            game.tick(&TickInput::default());
        }
    }

    /// Twenty pieces, each shifted to a different part of the board and hard
    /// dropped. Spreading them is what keeps the stack low enough to finish:
    /// twenty pieces dropped where they spawn tops out at eleven.
    fn play_twenty(seed: u64) -> Game {
        let mut game = new_game(seed);
        for i in 0..20u8 {
            let shift = if i % 2 == 0 {
                Shift::Left
            } else {
                Shift::Right
            };
            drop_piece(&mut game, Some(shift), 4 - (i / 2) % 5);
        }
        game
    }

    #[test]
    fn twenty_pieces_hard_dropped_reach_a_known_board() {
        // The stage's hand-driven game. It is a canary for the whole tick loop:
        // spawn, gravity, lock, clear and level progression all have to agree
        // for the board to come out the same way twice.
        let game = play_twenty(42);
        assert_eq!(game.pieces(), 20);
        assert!(!game.is_over());

        // The same seed and the same inputs reproduce it exactly (§15.4).
        let replay = play_twenty(42);
        assert_eq!(bottom_rows(&replay, 20), bottom_rows(&game, 20));
        assert_eq!(replay.lines(), game.lines());
        assert_eq!(replay.level(), game.level());
        assert_eq!(replay.ticks(), game.ticks());

        // A different seed does not.
        let other = play_twenty(43);
        assert_ne!(bottom_rows(&other, 20), bottom_rows(&game, 20));
    }

    #[test]
    fn a_scripted_game_is_unaffected_by_how_its_ticks_are_batched() {
        // A first look at §19.4, the desync canary. Stage 5 makes this an
        // integration test over GameView; asserting it here means a regression
        // is caught in the stage that introduced it.
        let mut single = new_game(42);
        let mut batched = new_game(42);
        let script: Vec<TickInput> = (0..600)
            .map(|i: u32| match i % 30 {
                0 => TickInput::action(Action::HardDrop),
                7 => TickInput::action(Action::RotateCw),
                11 => TickInput::shift(Shift::Left),
                19 => TickInput::action(Action::RotateCcw),
                23 => TickInput::shift(Shift::Right),
                _ => TickInput {
                    soft_drop: i % 3 == 0,
                    ..TickInput::default()
                },
            })
            .collect();
        for input in &script {
            single.tick(input);
        }
        for chunk in script.chunks(6) {
            for input in chunk {
                batched.tick(input);
            }
        }
        assert_eq!(bottom_rows(&batched, 20), bottom_rows(&single, 20));
        assert_eq!(batched.ticks(), single.ticks());
        assert_eq!(batched.pieces(), single.pieces());
        assert_eq!(batched.lines(), single.lines());
        assert_eq!(batched.current(), single.current());
    }

    #[test]
    fn a_game_played_by_dropping_everything_eventually_tops_out() {
        // Hard-dropping every piece in the same column tops out, and the game
        // then stops rather than running on.
        let mut game = new_game(101);
        let mut guard = 0;
        while !game.is_over() {
            game.tick(&TickInput::action(Action::HardDrop));
            while game.current().is_none() && !game.is_over() {
                game.tick(&TickInput::default());
            }
            guard += 1;
            assert!(guard < 1000, "a stack of dropped pieces should top out");
        }
        assert!(game.pieces() > 0);
    }

    #[test]
    fn the_180_gate_is_honoured_by_the_core_too() {
        // T12 belongs to Stage 8, but the backstop of §10.1 is here: with the
        // rule off, the action must change nothing at all -- including the
        // lock-delay timer.
        let mut game = new_game_with(
            GameplaySettings {
                allow_180_rotation: false,
                ..GameplaySettings::default()
            },
            TimingSettings::default(),
            59,
        );
        place(&mut game, PieceKind::T, Point::new(3, 38), Rotation::North);
        game.tick(&TickInput::default());
        let before = game.current().unwrap();
        assert_eq!(game.lock.remaining(), Some(29));
        game.tick(&TickInput::action(Action::Rotate180));
        assert_eq!(game.current().unwrap().rotation, before.rotation);
        assert_eq!(game.lock.resets_used(), 0, "an inert key resets no timer");
    }

    #[test]
    fn a_180_rotation_turns_the_piece_over_when_allowed() {
        let mut game = new_game(61);
        place(&mut game, PieceKind::T, Point::new(3, 25), Rotation::North);
        game.tick(&TickInput::action(Action::Rotate180));
        let piece = game.current().unwrap();
        assert_eq!(piece.rotation, Rotation::South);
        assert!(piece.last_action_was_rotation);
        assert_eq!(piece.last_kick_index, 0, "180 takes no kick tests");
    }
}
