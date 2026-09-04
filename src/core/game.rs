//! `Game` state and `Game::tick` — the single entry point to the core (§15.1).
//!
//! No clock and no I/O (§3.1): time enters only as calls to [`Game::tick`], one
//! fixed 1/60 s tick at a time. Given the same `RulesConfig`, seed and input
//! sequence the state is byte-identical, however the ticks are batched (§19.4).

use serde::{Deserialize, Serialize};

use crate::config::RulesConfig;
use crate::core::bag::Bag;
use crate::core::events::{ClearKind, GameEvent, TopOutCause};
use crate::core::geometry::{Point, Rotation};
use crate::core::gravity::{self, Gravity};
use crate::core::lockdown::{LockDown, LockOutcome};
use crate::core::matrix::{ClearedRows, Matrix, VISIBLE_TOP};
use crate::core::piece::PieceKind;
use crate::core::scoring::Scoring;
use crate::core::srs;
use crate::core::tspin;
use crate::core::view::to_visible;

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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Whether the clear waiting out that delay was paid at the back-to-back
    /// rate, kept for the perfect-clear bonus that may follow it (§9.15).
    clearing_b2b: bool,
    /// The piece in the hold slot, if any (§9.7).
    hold: Option<PieceKind>,
    /// Whether hold has already been used for the piece in play. Cleared when
    /// the next piece **locks**, not when it spawns (§9.7).
    hold_locked: bool,
    ticks: u64,
    pieces: u32,
    lines: u32,
    scoring: Scoring,
}

impl Game {
    /// A new game at `start_level`, with the first piece already spawned.
    ///
    /// The `PieceSpawned` of that first piece is discarded: construction is not
    /// a tick, and the piece is in the very first `view()` regardless. Events
    /// are a notification, never a mechanism (§12.8), so losing one costs
    /// nothing but an animation that had nothing to animate over.
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
            clearing_b2b: false,
            hold: None,
            hold_locked: false,
            ticks: 0,
            pieces: 0,
            lines: 0,
            scoring: Scoring::new(),
            rules,
        };
        game.spawn(&mut Vec::new());
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
        self.scoring.score()
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    pub fn pieces(&self) -> u32 {
        self.pieces
    }

    pub fn combo(&self) -> i32 {
        self.scoring.combo()
    }

    pub fn back_to_back(&self) -> bool {
        self.scoring.back_to_back()
    }

    /// The upcoming pieces the player can see: exactly `preview_count` of them
    /// (§12.7).
    pub fn preview(&self) -> impl Iterator<Item = PieceKind> + '_ {
        self.bag.preview()
    }

    /// The piece in the hold slot (§9.7).
    pub fn held(&self) -> Option<PieceKind> {
        self.hold
    }

    /// Whether hold has been spent on the piece in play (§9.7). The box is
    /// drawn dimmed while this is true.
    pub fn hold_locked(&self) -> bool {
        self.hold_locked
    }

    pub fn is_over(&self) -> bool {
        self.state == PlayState::ToppedOut
    }

    // -- the tick ----------------------------------------------------------

    /// Advance exactly one tick (§15.1), appending what happened to `out`.
    ///
    /// The buffer is supplied by the caller and appended to, never cleared: the
    /// common tick raises nothing, and the core must not allocate in order to do
    /// nothing (§12.8). The core writes to `out` and never reads it back, so
    /// whatever the caller does with the events cannot change the game (T16).
    pub fn tick(&mut self, input: &TickInput, out: &mut Vec<GameEvent>) {
        if self.state == PlayState::ToppedOut {
            return;
        }
        self.ticks += 1;
        match self.state {
            PlayState::Clearing => self.tick_clearing(out),
            PlayState::Entry => self.tick_entry(out),
            PlayState::Falling => self.tick_falling(input, out),
            PlayState::ToppedOut => {}
        }
    }

    /// Wait out the line-clear delay, then collapse the rows (§9.12 steps 5-7).
    fn tick_clearing(&mut self, out: &mut Vec<GameEvent>) {
        self.state_timer = self.state_timer.saturating_sub(1);
        if self.state_timer > 0 {
            return;
        }
        self.matrix.clear_rows(&self.clearing_rows);
        // §9.15: a perfect clear is judged once the rows have gone, and its
        // bonus is multiplied by the level in force for the clear -- so it is
        // awarded here, before §9.12 step 7 moves the level on.
        if self.matrix.is_empty() {
            out.push(GameEvent::PerfectClear);
            self.scoring.perfect_clear(
                self.clearing_rows.len(),
                self.clearing_b2b,
                self.gravity.level(),
                out,
            );
        }
        self.lines += self.clearing_rows.len() as u32;
        let level = gravity::level_after_clear(
            self.gravity.level(),
            self.lines,
            self.rules.lines_per_level,
        );
        if level != self.gravity.level() {
            out.push(GameEvent::LevelUp(level));
        }
        self.gravity.set_level(level);
        self.clearing_rows = ClearedRows::default();
        self.begin_entry_delay(out);
    }

    /// Wait out the entry delay, then spawn (§9.12 steps 8-9).
    fn tick_entry(&mut self, out: &mut Vec<GameEvent>) {
        self.state_timer = self.state_timer.saturating_sub(1);
        if self.state_timer == 0 {
            self.spawn(out);
        }
    }

    /// A tick with a piece in play.
    ///
    /// Order matters and is fixed: the player's actions, then their movement,
    /// then gravity, then the lock-down timer. Input first is what lets a piece
    /// be steered on the tick it would otherwise lock.
    fn tick_falling(&mut self, input: &TickInput, out: &mut Vec<GameEvent>) {
        for action in input.actions.iter() {
            match action {
                Action::RotateCw => self.rotate(Rotation::cw, out),
                Action::RotateCcw => self.rotate(Rotation::ccw, out),
                Action::Rotate180 => self.rotate_180(out),
                Action::HardDrop => {
                    self.hard_drop(out);
                    return;
                }
                Action::Hold => self.hold(out),
                _ => {}
            }
        }
        // A hold spawns, and a spawn can end the game by Block Out (§9.16).
        // Once it has, this is no longer a falling tick and there is nothing
        // left to move.
        if self.state != PlayState::Falling {
            return;
        }

        if let Some(shift) = input.shift {
            let dx = match shift {
                Shift::Left => -1,
                Shift::Right => 1,
            };
            let mut moved = false;
            for _ in 0..input.shift_cells {
                if !self.try_move(dx, 0) {
                    break;
                }
                moved = true;
            }
            if moved {
                out.push(GameEvent::PieceMoved);
            }
        }

        self.apply_gravity(input.soft_drop, out);
        self.settle(out);
    }

    /// Accrue gravity and fall as far as it allows (§9.9, §9.10).
    fn apply_gravity(&mut self, soft_drop: bool, out: &mut Vec<GameEvent>) {
        let factor = soft_drop.then_some(self.rules.soft_drop_factor);
        let rows = self.gravity.accrue(factor);
        let mut fell = 0;
        for _ in 0..rows {
            if !self.try_move(0, 1) {
                // §9.9: a blocked step clears the accumulator and hands over to
                // the lock-down machine.
                self.gravity.reset_accumulator();
                break;
            }
            fell += 1;
        }
        if fell > 0 {
            out.push(GameEvent::PieceMoved);
        }
        if soft_drop {
            // §9.10: 1 point per row *actually* descended while the key is
            // held, unmultiplied by level. Rows the piece could not take are
            // not descended, and are not paid for.
            self.scoring.soft_drop(fell, out);
        }
    }

    /// Update the lock-down timer and lock if it expires (§9.11).
    fn settle(&mut self, out: &mut Vec<GameEvent>) {
        let resting = !self.can_move(0, 1);
        if resting {
            self.lock.land();
        } else {
            self.lock.lift();
        }
        if self.lock.tick(resting) == LockOutcome::Lock {
            self.lock_piece(out);
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
    fn rotate(&mut self, turn: fn(Rotation) -> Rotation, out: &mut Vec<GameEvent>) {
        let Some(piece) = self.current else { return };
        let to = turn(piece.rotation);
        let Some((origin, kick_index)) =
            srs::try_rotate(&self.matrix, piece.kind, piece.origin, piece.rotation, to)
        else {
            // §9.5 step 4: a failed rotation changes nothing and resets no
            // timer.
            out.push(GameEvent::RotationFailed);
            return;
        };
        self.accept_rotation(origin, to, kick_index, out);
    }

    /// Rotate 180 degrees, if the rules allow it (§9.5).
    ///
    /// The gate is also enforced at the input boundary (§10.1) so a disabled key
    /// never reaches the core; this is the backstop for a caller that ignores
    /// that, and it must return before touching any timer.
    fn rotate_180(&mut self, out: &mut Vec<GameEvent>) {
        if !self.rules.allow_180_rotation {
            // An inert key is not a failed rotation: it raises nothing, exactly
            // as it resets nothing.
            return;
        }
        let Some(piece) = self.current else { return };
        let Some((origin, kick_index)) =
            srs::try_rotate_180(&self.matrix, piece.kind, piece.origin, piece.rotation)
        else {
            out.push(GameEvent::RotationFailed);
            return;
        };
        self.accept_rotation(origin, piece.rotation.opposite(), kick_index, out);
    }

    /// Commit a successful rotation and its lock-down consequences.
    fn accept_rotation(
        &mut self,
        origin: Point,
        rotation: Rotation,
        kick_index: u8,
        out: &mut Vec<GameEvent>,
    ) {
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
        out.push(GameEvent::PieceRotated { kick_index });
    }

    /// Swap the falling piece with the hold slot (§9.7).
    ///
    /// Like the 180 gate, `hold_enabled` is enforced at the input boundary
    /// (§10.1) so a disabled key never reaches the core; this is the backstop,
    /// and like that one it must return before touching any timer.
    fn hold(&mut self, out: &mut Vec<GameEvent>) {
        if !self.rules.hold_enabled {
            // An inert key raises nothing, exactly as it resets nothing.
            return;
        }
        if self.hold_locked {
            // Once per piece (§9.7). Refused, but worth announcing: §12.5 has
            // the hold box to shake.
            out.push(GameEvent::HoldRejected);
            return;
        }
        let Some(piece) = self.current.take() else {
            return;
        };
        let incoming = self.hold.replace(piece.kind);
        self.hold_locked = true;
        out.push(GameEvent::HoldUsed);
        // Either way the replacement is spawned *normally* (§9.4): orientation
        // `North`, at its spawn origin, and not where the outgoing piece was.
        // `spawn` also resets the gravity accumulator and rebuilds the lock-down
        // machine, which is how the incoming piece begins fresh.
        match incoming {
            Some(kind) => self.spawn_kind(kind, out),
            None => self.spawn(out),
        }
    }

    /// Drop to the floor and lock immediately, skipping lock delay (§9.10).
    ///
    /// §9.13 requires the "last action was a rotation" flag to **survive** the
    /// drop: a T rotated into its slot and then hard-dropped is still a T-spin.
    /// Every `try_move` clears that flag, so it is carried across by hand here.
    /// This is the rule everybody gets wrong.
    fn hard_drop(&mut self, out: &mut Vec<GameEvent>) {
        let Some(before) = self.current else { return };
        let mut rows = 0u8;
        while self.try_move(0, 1) {
            rows = rows.saturating_add(1);
        }
        out.push(GameEvent::HardDropped { rows });
        // §9.10: 2 points per row descended, unmultiplied by level. A drop of
        // zero rows still locks the piece and awards nothing.
        self.scoring.hard_drop(u32::from(rows), out);
        if let Some(piece) = self.current.as_mut() {
            piece.last_action_was_rotation = before.last_action_was_rotation;
            piece.last_kick_index = before.last_kick_index;
        }
        self.lock_piece(out);
    }

    // -- locking and spawning ----------------------------------------------

    /// Write the piece into the matrix and deal with what follows (§9.12).
    fn lock_piece(&mut self, out: &mut Vec<GameEvent>) {
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
        out.push(GameEvent::PieceLocked {
            cells: std::array::from_fn(|i| to_visible(minos[i].x, minos[i].y)),
            kind: piece.kind,
        });
        // §9.12 step 2: the T-spin status is decided before any row is removed.
        let spin = tspin::classify(&self.matrix, &piece);
        // §9.7: hold is spent once per piece, and the piece is over now. On
        // the *lock*, not on the spawn: clearing it a moment earlier would let
        // a piece taken out of hold be held straight back, twice per turn.
        self.hold_locked = false;

        if locked_out {
            self.state = PlayState::ToppedOut;
            out.push(GameEvent::ToppedOut(TopOutCause::LockOut));
            return;
        }

        self.clearing_rows = self.matrix.full_rows();
        // §9.12 step 4: scored with the level in force now, which step 7 has
        // yet to move.
        let clear = ClearKind::of(spin, self.clearing_rows.len());
        self.clearing_b2b = self.scoring.lock(clear, self.gravity.level(), out);

        if self.clearing_rows.is_empty() {
            self.begin_entry_delay(out);
        } else {
            // §12.8: raised as the rows are found, so the §12.5 flash covers
            // the clear pause exactly. The rows are still in the matrix, and
            // still drawn, until the pause runs out.
            if let Some(clear) = clear {
                out.push(GameEvent::LinesCleared {
                    rows: self.visible_clearing_rows(),
                    clear,
                    // Whether *this* clear was paid at the chained rate, which
                    // is what the banner announces. The status bar's standing
                    // `B2B` indicator is `GameView::back_to_back` (§9.15).
                    b2b: self.clearing_b2b,
                    combo: self.scoring.combo(),
                });
            }
            self.state = PlayState::Clearing;
            self.state_timer = self.rules.line_clear_delay_ticks;
        }
    }

    /// The clearing rows in visible-field coordinates (§12.8).
    ///
    /// A completed row inside the buffer zone cannot be drawn, so it is left
    /// out: the list may be shorter than the number of rows being removed.
    fn visible_clearing_rows(&self) -> Vec<u8> {
        self.clearing_rows
            .as_slice()
            .iter()
            .filter_map(|&y| u8::try_from(y - VISIBLE_TOP).ok())
            .collect()
    }

    /// Start the entry delay, or spawn at once when it is zero (§9.12 step 8).
    ///
    /// `entry_delay_ticks` may legitimately be 0 (§6.6), which means the next
    /// piece enters on this very tick rather than one tick later.
    fn begin_entry_delay(&mut self, out: &mut Vec<GameEvent>) {
        if self.rules.entry_delay_ticks == 0 {
            self.spawn(out);
        } else {
            self.state = PlayState::Entry;
            self.state_timer = self.rules.entry_delay_ticks;
        }
    }

    /// Take the next piece from the queue and place it (§9.4).
    fn spawn(&mut self, out: &mut Vec<GameEvent>) {
        let kind = self.bag.next_piece();
        self.spawn_kind(kind, out);
    }

    /// Place a particular piece (§9.4), for the one coming back out of hold.
    ///
    /// The queue is not touched: a hold swap does not consume a preview.
    fn spawn_kind(&mut self, kind: PieceKind, out: &mut Vec<GameEvent>) {
        let origin = kind.spawn_origin();
        out.push(GameEvent::PieceSpawned(kind));
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
            out.push(GameEvent::ToppedOut(TopOutCause::BlockOut));
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
pub mod tests {
    use super::*;
    use crate::config::{GameplaySettings, TimingSettings};
    use crate::core::events::ScoreReason;
    use crate::core::matrix::{HEIGHT, WIDTH, tests::from_bottom_rows};

    /// A game with the default rules under a fixed seed.
    pub fn new_game(seed: u64) -> Game {
        Game::new(RulesConfig::default(), seed)
    }

    /// A game whose rules differ from the defaults.
    fn new_game_with(gameplay: GameplaySettings, timing: TimingSettings, seed: u64) -> Game {
        Game::new(RulesConfig::from_settings(&gameplay, &timing), seed)
    }

    /// Advance one tick, discarding the events. The event stream has its own
    /// tests; every other test in this module is about the state it leaves
    /// behind, and by §12.8 that state is the same either way.
    fn tick(game: &mut Game, input: &TickInput) {
        game.tick(input, &mut Vec::new());
    }

    /// Advance one tick, keeping the events it raised.
    fn tick_events(game: &mut Game, input: &TickInput) -> Vec<GameEvent> {
        let mut events = Vec::new();
        game.tick(input, &mut events);
        events
    }

    /// Run `n` ticks with nothing held.
    fn idle(game: &mut Game, n: u32) {
        let input = TickInput::default();
        for _ in 0..n {
            tick(game, &input);
        }
    }

    /// Force a particular piece into play at a particular place, bypassing the
    /// bag. Board fixtures need a known piece, and the bag is not steerable.
    pub fn place(game: &mut Game, kind: PieceKind, origin: Point, rotation: Rotation) {
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

    /// Replace the board wholesale. `Game`'s fields are private to this module,
    /// so the fixtures other modules' tests need are handed out from here.
    pub fn set_matrix(game: &mut Game, matrix: Matrix) {
        game.matrix = matrix;
    }

    pub fn bottom_rows(game: &Game, n: usize) -> Vec<String> {
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
    fn soft_drop_pressed_late_in_a_fall_moves_one_row() {
        // Reported from play: soft drop "occasionally executed a hard drop".
        // It was gravity, not the keyboard -- the accumulator had banked most
        // of a level-1 row, and dividing the period by `soft_drop_factor` cashed
        // it in as seventeen rows on the tick the key went down (§9.9, §9.10).
        let mut game = new_game(5);
        place(&mut game, PieceKind::O, Point::new(4, 19), Rotation::North);
        idle(&mut game, 50);
        let before = game.current().expect("still falling").lowest_row();

        tick(
            &mut game,
            &TickInput {
                soft_drop: true,
                ..TickInput::default()
            },
        );

        let after = game.current().expect("still falling").lowest_row();
        assert_eq!(after - before, 1, "one row on the press, not a plummet");
    }

    #[test]
    fn a_hard_drop_locks_at_once() {
        // §9.10: lock delay is skipped entirely.
        let mut game = new_game(9);
        let kind = game.current().unwrap().kind;
        tick(&mut game, &TickInput::action(Action::HardDrop));
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
        tick(&mut game, &TickInput::action(Action::HardDrop));
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
        tick(&mut game, &TickInput::action(Action::RotateCw));
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

        tick(&mut game, &TickInput::action(Action::HardDrop));
        assert_eq!(game.pieces(), 1);
    }

    #[test]
    fn a_moved_piece_is_no_longer_marked_as_rotated() {
        // The other half of §9.13's precondition: a shift clears the flag, so a
        // T slid into a slot cannot claim a T-spin.
        let mut game = new_game(17);
        place(&mut game, PieceKind::T, Point::new(3, 20), Rotation::North);
        tick(&mut game, &TickInput::action(Action::RotateCw));
        assert!(game.current().unwrap().last_action_was_rotation);
        tick(&mut game, &TickInput::shift(Shift::Left));
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
        tick(&mut game, &TickInput::default());
        let landed_at = game.lock.remaining();
        assert_eq!(landed_at, Some(29), "landed and counting");
        tick(&mut game, &TickInput::action(Action::RotateCw));
        assert_eq!(game.lock.remaining(), Some(28), "no reset was granted");
        assert_eq!(game.lock.resets_used(), 0);
    }

    #[test]
    fn a_shift_while_landed_resets_the_timer() {
        let mut game = new_game(23);
        place(&mut game, PieceKind::O, Point::new(4, 38), Rotation::North);
        tick(&mut game, &TickInput::default());
        assert_eq!(game.lock.remaining(), Some(29));
        tick(&mut game, &TickInput::shift(Shift::Left));
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
        tick(&mut game, &TickInput::action(Action::HardDrop));

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
        tick(&mut game, &TickInput::action(Action::HardDrop));
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
        tick(&mut game, &TickInput::action(Action::HardDrop));
        assert_eq!(game.state(), PlayState::Clearing);
        idle(&mut game, 15);
        assert_eq!(game.lines(), 4);
        assert_eq!(game.level(), 2, "four lines at four per level");
        assert!(game.matrix().is_empty(), "a perfect clear, as it happens");
    }

    /// A board that block-outs the *next* piece, whatever it turns out to be.
    ///
    /// A tower filling columns 3, 4 and 5 from row 19 to the floor covers every
    /// cell the §9.4 spawn table can put a mino in, so the spawn collides no
    /// matter which piece the bag deals. Columns 6-9 are left open, so nothing
    /// here is a completed row, and column 0 is left clear for the piece that
    /// has to lock first -- that piece must reach row 20 or below, or the game
    /// would end in a Lock Out before the spawn was ever attempted.
    fn tower_over_the_spawn_columns() -> Matrix {
        let mut matrix = Matrix::new();
        for y in 19..HEIGHT {
            for x in 3..=5 {
                matrix.set(x, y, Some(PieceKind::I));
            }
        }
        matrix
    }

    #[test]
    fn block_out_ends_the_game() {
        // T11: a newly spawned piece overlaps a locked mino at its spawn
        // position (§9.16). The piece that triggers it locks in the visible
        // field, so this is a Block Out and not a Lock Out wearing its name.
        let mut game = new_game(41);
        game.matrix = tower_over_the_spawn_columns();
        place(&mut game, PieceKind::O, Point::new(0, 36), Rotation::North);
        let events = tick_events(&mut game, &TickInput::action(Action::HardDrop));
        assert!(game.is_over(), "the next piece cannot spawn");
        assert_eq!(game.state(), PlayState::ToppedOut);
        assert_eq!(
            events.last(),
            Some(&GameEvent::ToppedOut(TopOutCause::BlockOut)),
            "the O itself locked on the floor, in plain view",
        );
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
        tick(&mut game, &TickInput::action(Action::HardDrop));
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
        tick(&mut game, &TickInput::action(Action::HardDrop));
        assert!(!game.is_over(), "one mino reached row 20");
        assert_eq!(game.matrix().get(0, 20), Some(PieceKind::I));
        assert_eq!(game.matrix().get(0, 17), Some(PieceKind::I));
    }

    /// Drop a piece after shifting it, then wait out any clear or entry delay,
    /// so the caller is handed back a game with the next piece already in play.
    fn drop_piece(game: &mut Game, shift: Option<Shift>, cells: u8) {
        if let Some(shift) = shift {
            tick(
                game,
                &TickInput {
                    shift: Some(shift),
                    shift_cells: cells,
                    ..TickInput::default()
                },
            );
        }
        tick(game, &TickInput::action(Action::HardDrop));
        while game.current().is_none() && !game.is_over() {
            tick(game, &TickInput::default());
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

    /// A script long enough to spawn, steer, lock, clear and level up several
    /// times over. Deliberately dense: a desync has to have somewhere to show
    /// itself.
    fn script(ticks: u32) -> Vec<TickInput> {
        (0..ticks)
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
            .collect()
    }

    #[test]
    fn two_games_from_the_same_seed_agree_on_every_tick() {
        // T14, the first half: same `RulesConfig`, same seed, same inputs, and
        // the views must match at every tick -- not merely at the end, where a
        // divergence could have cancelled itself out.
        let mut a = new_game(42);
        let mut b = new_game(42);
        assert_eq!(a.view(), b.view(), "before either has ticked");
        for (i, input) in script(900).iter().enumerate() {
            tick(&mut a, input);
            tick(&mut b, input);
            assert_eq!(a.view(), b.view(), "at tick {i}");
        }
    }

    #[test]
    fn a_recorded_log_replays_to_the_same_state() {
        // T14, the second half. A different seed must not reproduce it, or the
        // test would pass for a core that ignored the seed entirely.
        let log = script(900);
        let replay = |seed| {
            let mut game = new_game(seed);
            for input in &log {
                tick(&mut game, input);
            }
            game.view()
        };
        assert_eq!(replay(42), replay(42));
        assert_ne!(replay(42), replay(43));
    }

    #[test]
    fn a_scripted_game_is_unaffected_by_how_its_ticks_are_batched() {
        // T14 and §19.4, the desync canary, at the level of the view. `tests/`
        // repeats it end to end against a checked-in snapshot; having it here
        // as well means a regression names the stage that introduced it.
        //
        // One game is looked at every tick, the other only every sixth: if any
        // rules decision depended on how the shell batched its calls, or on
        // `view` being called, the two would part company.
        let mut single = new_game(42);
        let mut batched = new_game(42);
        let script = script(900);
        for (chunk_index, chunk) in script.chunks(6).enumerate() {
            for input in chunk {
                tick(&mut single, input);
                let _ = single.view();
            }
            for input in chunk {
                tick(&mut batched, input);
            }
            assert_eq!(single.view(), batched.view(), "after batch {chunk_index}",);
        }
    }

    #[test]
    fn a_game_played_by_dropping_everything_eventually_tops_out() {
        // Hard-dropping every piece in the same column tops out, and the game
        // then stops rather than running on.
        let mut game = new_game(101);
        let mut guard = 0;
        while !game.is_over() {
            tick(&mut game, &TickInput::action(Action::HardDrop));
            while game.current().is_none() && !game.is_over() {
                tick(&mut game, &TickInput::default());
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
        tick(&mut game, &TickInput::default());
        let before = game.current().unwrap();
        assert_eq!(game.lock.remaining(), Some(29));
        tick(&mut game, &TickInput::action(Action::Rotate180));
        assert_eq!(game.current().unwrap().rotation, before.rotation);
        assert_eq!(game.lock.resets_used(), 0, "an inert key resets no timer");
    }

    #[test]
    fn a_180_rotation_turns_the_piece_over_when_allowed() {
        let mut game = new_game(61);
        place(&mut game, PieceKind::T, Point::new(3, 25), Rotation::North);
        tick(&mut game, &TickInput::action(Action::Rotate180));
        let piece = game.current().unwrap();
        assert_eq!(piece.rotation, Rotation::South);
        assert!(piece.last_action_was_rotation);
        assert_eq!(piece.last_kick_index, 0, "180 takes no kick tests");
    }

    // -- T9, T10: T-spins and scoring (§9.13, §9.14, §9.15) ----------------

    /// The canonical three-corner board, `rows 37..=39`:
    ///
    /// ```text
    /// row 37   . . . . . # # # # #     the overhang, above the box's right
    /// row 38   # # # . . . # # # #     the slot the T turns into
    /// row 39   # # # # . # # # # #
    /// ```
    ///
    /// The T's 3 × 3 box sits at `(3, 37)`. Three of its four corners are
    /// filled — top-right, bottom-left, bottom-right — which is a proper spin
    /// for a T that ends up pointing down and a mini for one pointing left, on
    /// the very same board. That is the §9.13 front/back rule, and the reason
    /// one fixture serves both.
    fn three_corner_slot() -> Matrix {
        from_bottom_rows(&[".....#####", "###...####", "####.#####"])
    }

    /// Rotate and immediately hard drop, which is one tick's worth of actions
    /// (§10.2) and the shape of every T-spin a player actually performs.
    fn spin_and_drop(game: &mut Game, rotate: Action) -> Vec<GameEvent> {
        tick_events(
            game,
            &TickInput {
                actions: [rotate, Action::HardDrop].into_iter().collect(),
                ..TickInput::default()
            },
        )
    }

    /// The `LinesCleared` of a tick, if it raised one.
    fn clear_of(events: &[GameEvent]) -> Option<(ClearKind, bool, i32)> {
        events.iter().find_map(|event| match event {
            GameEvent::LinesCleared {
                clear, b2b, combo, ..
            } => Some((*clear, *b2b, *combo)),
            _ => None,
        })
    }

    #[test]
    fn the_canonical_t_spin_double() {
        // T9. A T stood on its side in the slot, turned to point down: both
        // front corners (the bottom pair) and one back corner. Rows 38 and 39
        // both complete, and §9.14 pays 1200 at level 1.
        let mut game = new_game(101);
        game.matrix = three_corner_slot();
        place(&mut game, PieceKind::T, Point::new(3, 37), Rotation::West);
        let events = spin_and_drop(&mut game, Action::RotateCcw);

        assert_eq!(clear_of(&events), Some((ClearKind::TSpinDouble, false, 0)));
        assert_eq!(game.score(), 1200);
        assert!(game.back_to_back(), "a scoring T-spin starts a chain");
    }

    #[test]
    fn the_canonical_t_spin_single() {
        // T9. The same slot with row 39 one cell short of complete, so only row
        // 38 goes: 800 at level 1.
        let mut game = new_game(103);
        game.matrix = from_bottom_rows(&[".....#####", "###...####", ".###.#####"]);
        place(&mut game, PieceKind::T, Point::new(3, 37), Rotation::West);
        let events = spin_and_drop(&mut game, Action::RotateCcw);

        assert_eq!(clear_of(&events), Some((ClearKind::TSpinSingle, false, 0)));
        assert_eq!(game.score(), 800);
    }

    #[test]
    fn the_canonical_t_spin_triple() {
        // T9, and the reason §9.13 has a kick-index override at all. The well is
        // roofed by the cell at `(4, 35)`, so the first four kick tests all
        // collide and only test 5 -- one left and two down -- fits. Its corners
        // would say proper anyway; boards where they do not are what the
        // override is for, and `tspin` tests that half in isolation.
        //
        // ```text
        // row 35   . . . . # . . . . .     the overhang the T rests against
        // row 36   . . . . . . . . . .
        // row 37   # # # # . # # # # #
        // row 38   # # # # . . # # # #
        // row 39   # # # # . # # # # #
        // ```
        let mut game = new_game(107);
        game.matrix = from_bottom_rows(&[
            "....#.....",
            "..........",
            "####.#####",
            "####..####",
            "####.#####",
        ]);
        place(&mut game, PieceKind::T, Point::new(4, 35), Rotation::North);
        let events = spin_and_drop(&mut game, Action::RotateCw);

        assert!(
            events.contains(&GameEvent::PieceRotated { kick_index: 4 }),
            "the triple is reached by kick test 5: {events:?}",
        );
        assert_eq!(clear_of(&events), Some((ClearKind::TSpinTriple, false, 0)));
        assert_eq!(game.score(), 1600);
    }

    #[test]
    fn the_canonical_t_spin_mini() {
        // T9. The same board as the double, entered flat and turned to point
        // left: one front corner (the top-left is empty) and both back corners.
        // Only row 39 completes, so §9.14 pays 200.
        let mut game = new_game(109);
        game.matrix = three_corner_slot();
        place(&mut game, PieceKind::T, Point::new(3, 37), Rotation::North);
        let events = spin_and_drop(&mut game, Action::RotateCcw);

        assert_eq!(
            clear_of(&events),
            Some((ClearKind::TSpinMiniSingle, false, 0))
        );
        assert_eq!(game.score(), 200);
    }

    #[test]
    fn a_rotation_before_a_hard_drop_is_still_the_last_action() {
        // §9.13 end to end, and the rule everybody gets wrong: the T is turned
        // two rows above the slot and dropped into it, and the spin counts.
        // Compare with the test below, which is the same landing without the
        // rotation.
        let mut game = new_game(113);
        game.matrix = three_corner_slot();
        place(&mut game, PieceKind::T, Point::new(3, 35), Rotation::North);
        let events = spin_and_drop(&mut game, Action::RotateCcw);

        assert!(events.contains(&GameEvent::HardDropped { rows: 2 }));
        assert_eq!(
            clear_of(&events),
            Some((ClearKind::TSpinMiniSingle, false, 0))
        );
        // 200 for the mini single, and 2 points a row for the drop (§9.10).
        assert_eq!(game.score(), 200 + 4);
    }

    #[test]
    fn a_t_that_was_moved_into_the_slot_is_not_a_spin() {
        // T9's negative case: the same piece, the same landing, the same three
        // corners -- but it arrived by falling, so §9.13 does not look at it and
        // the row is worth 100, not 200.
        let mut game = new_game(127);
        game.matrix = three_corner_slot();
        place(&mut game, PieceKind::T, Point::new(3, 35), Rotation::West);
        let events = tick_events(&mut game, &TickInput::action(Action::HardDrop));

        assert!(events.contains(&GameEvent::HardDropped { rows: 2 }));
        assert_eq!(clear_of(&events), Some((ClearKind::Single, false, 0)));
        assert_eq!(game.score(), 100 + 4);
        assert!(!game.back_to_back(), "a plain single is not difficult");
    }

    #[test]
    fn a_rotated_piece_that_is_not_a_t_clears_plainly() {
        // The other negative case, end to end: an I turned over the gap it
        // fills scores a Single, not a spin. `tspin` checks every non-T kind
        // against a board whose corners are all filled.
        let mut game = new_game(131);
        game.matrix = from_bottom_rows(&["###....###"]);
        place(&mut game, PieceKind::I, Point::new(3, 37), Rotation::North);
        let events = spin_and_drop(&mut game, Action::Rotate180);

        assert_eq!(clear_of(&events), Some((ClearKind::Single, false, 0)));
        assert_eq!(game.score(), 100);
    }

    #[test]
    fn the_back_to_back_chain_survives_the_piece_between_two_spins() {
        // §9.15 through the game rather than the counter: two T-spin doubles
        // with an ordinary lock in between, which clears nothing and so breaks
        // nothing.
        let mut game = new_game(137);
        game.matrix = three_corner_slot();
        place(&mut game, PieceKind::T, Point::new(3, 37), Rotation::West);
        spin_and_drop(&mut game, Action::RotateCcw);
        assert_eq!(game.score(), 1200);
        idle(&mut game, 15);

        // A piece parked out of the way: no rows, no spin, no chain broken.
        place(&mut game, PieceKind::O, Point::new(0, 30), Rotation::North);
        tick(&mut game, &TickInput::action(Action::HardDrop));
        assert!(game.back_to_back(), "an empty lock is not a clear");
        assert_eq!(game.combo(), -1, "but it does end the combo");

        // Rebuild the slot and do it again: 1800 this time, and no combo, the
        // O having reset the counter.
        game.matrix = three_corner_slot();
        place(&mut game, PieceKind::T, Point::new(3, 37), Rotation::West);
        let events = spin_and_drop(&mut game, Action::RotateCcw);
        assert_eq!(clear_of(&events), Some((ClearKind::TSpinDouble, true, 0)));
        // 1200, plus 16 for the O's eight-row drop, plus the chained 1800.
        assert_eq!(game.score(), 1200 + 16 + 1800);
    }

    #[test]
    fn two_clears_in_a_row_are_a_combo() {
        // §9.15 through the game: an O into each corner of a board that is one
        // row short at both ends.
        //
        // ```text
        // row 38   . . # # # # # # . .
        // row 39   . . # # # # # # # #
        // ```
        let mut game = new_game(139);
        game.matrix = from_bottom_rows(&["..######..", "..########"]);

        place(&mut game, PieceKind::O, Point::new(0, 30), Rotation::North);
        let events = tick_events(&mut game, &TickInput::action(Action::HardDrop));
        assert_eq!(clear_of(&events), Some((ClearKind::Single, false, 0)));
        // 100 for the row, 16 for the eight rows dropped. No combo yet: the
        // first clear of a run takes the counter to 0.
        assert_eq!(game.score(), 100 + 16);
        idle(&mut game, 15);

        // Row 39 has gone and what was row 38 has taken its place, still two
        // cells short at the right-hand end.
        place(&mut game, PieceKind::O, Point::new(8, 30), Rotation::North);
        let events = tick_events(&mut game, &TickInput::action(Action::HardDrop));
        assert_eq!(clear_of(&events), Some((ClearKind::Single, false, 1)));
        // 100 and 16 again, and 50 x 1 x 1 for the combo (§9.14).
        assert_eq!(game.score(), 100 + 16 + 100 + 16 + 50);
    }

    #[test]
    fn soft_drop_pays_a_point_a_row_and_hard_drop_two() {
        // §9.10, and neither is multiplied by level. At level 1 the soft-drop
        // period is one row every three ticks, so thirty ticks is ten rows.
        let mut game = new_game(149);
        let start = game.current().unwrap().lowest_row();
        let input = TickInput {
            soft_drop: true,
            ..TickInput::default()
        };
        for _ in 0..30 {
            tick(&mut game, &input);
        }
        assert_eq!(game.current().unwrap().lowest_row(), start + 10);
        assert_eq!(game.score(), 10);

        let mut game = new_game(151);
        place(&mut game, PieceKind::O, Point::new(4, 30), Rotation::North);
        tick(&mut game, &TickInput::action(Action::HardDrop));
        assert_eq!(game.score(), 8 * 2, "eight rows to the floor");
    }

    #[test]
    fn the_perfect_clear_bonus_is_worth_more_than_the_clear() {
        // §9.15. The same fixture as the event test above, scored: a Single is
        // 100 and emptying the board with it is 800 more.
        let mut game = new_game(157);
        game.matrix = from_bottom_rows(&["######...."]);
        place(&mut game, PieceKind::I, Point::new(6, 30), Rotation::North);
        tick(&mut game, &TickInput::action(Action::HardDrop));
        assert_eq!(game.score(), 100 + 8 * 2, "the clear, before the pause");
        idle(&mut game, 15);
        assert_eq!(game.score(), 100 + 16 + 800);
    }

    // -- T16: the event stream (§12.8) -------------------------------------

    #[test]
    fn a_clearing_tick_reports_the_rows_it_is_about_to_clear() {
        // T16. The rows are named in visible-field coordinates, the clear is
        // classified, and the event arrives as the flash starts -- one tick
        // before the rows actually go. The score for both the drop and the
        // clear is awarded first, in §9.12's order: step 4 pays, step 5 starts
        // the pause.
        let mut game = new_game(29);
        game.matrix = from_bottom_rows(&["####.#####", "####.#####"]);
        place(&mut game, PieceKind::I, Point::new(2, 34), Rotation::East);
        let events = tick_events(&mut game, &TickInput::action(Action::HardDrop));

        assert_eq!(
            events,
            vec![
                GameEvent::HardDropped { rows: 2 },
                GameEvent::ScoreAwarded {
                    points: 4,
                    reason: ScoreReason::HardDrop,
                },
                GameEvent::PieceLocked {
                    cells: [(4, 16), (4, 17), (4, 18), (4, 19)],
                    kind: PieceKind::I,
                },
                GameEvent::ScoreAwarded {
                    points: 300,
                    reason: ScoreReason::LineClear(ClearKind::Double),
                },
                GameEvent::LinesCleared {
                    rows: vec![18, 19],
                    clear: ClearKind::Double,
                    b2b: false,
                    // The first clear of a run takes the counter to 0, which
                    // is still no combo (§9.15).
                    combo: 0,
                },
            ],
        );
        assert_eq!(game.lines(), 0, "the rows are still on screen");
    }

    #[test]
    fn a_clear_that_empties_the_board_reports_a_perfect_clear() {
        // §9.15, judged once the rows have gone, and followed by the level and
        // the next piece in that order (§9.12 steps 6-9).
        let mut game = new_game_with(
            GameplaySettings {
                lines_per_level: 1,
                ..GameplaySettings::default()
            },
            TimingSettings::default(),
            31,
        );
        // Six filled cells and a four-wide gap: a flat I completes the row
        // and leaves nothing behind, which is what makes it a perfect clear.
        game.matrix = from_bottom_rows(&["######...."]);
        place(&mut game, PieceKind::I, Point::new(6, 30), Rotation::North);
        tick(&mut game, &TickInput::action(Action::HardDrop));
        idle(&mut game, 14);
        let events = tick_events(&mut game, &TickInput::default());

        assert_eq!(events[0], GameEvent::PerfectClear, "the board is empty");
        assert_eq!(
            events[1],
            GameEvent::ScoreAwarded {
                points: 800,
                reason: ScoreReason::PerfectClear,
            },
            "a one-row perfect clear at level 1 (§9.14)",
        );
        assert_eq!(events[2], GameEvent::LevelUp(2));
        assert!(
            matches!(events[3], GameEvent::PieceSpawned(_)),
            "and the next piece follows, entry delay being 0",
        );
        assert_eq!(events.len(), 4);
    }

    #[test]
    fn the_common_tick_says_nothing_and_allocates_nothing() {
        // T16: "the common tick emits no events and allocates nothing". The
        // buffer is the caller's and is never cleared by the core, so a run of
        // quiet ticks must leave both its length and its capacity alone.
        let mut game = new_game(37);
        let mut events = Vec::with_capacity(4);
        let capacity = events.capacity();
        for _ in 0..59 {
            game.tick(&TickInput::default(), &mut events);
        }
        assert!(events.is_empty(), "nothing happened, so nothing was said");
        assert_eq!(events.capacity(), capacity, "and nothing was allocated");

        // The sixtieth tick is the one gravity moves the piece on (§9.9).
        game.tick(&TickInput::default(), &mut events);
        assert_eq!(events, vec![GameEvent::PieceMoved]);
    }

    #[test]
    fn a_failed_rotation_is_reported_but_an_inert_key_is_not() {
        // §9.5 step 4 raises `RotationFailed` so the shell can buzz; a key
        // switched off in the config never reaches the rules at all (§10.1).
        let mut game = new_game(41);
        // A flat I in a slot one row high. Every kick test lands it across a
        // filled row, so the rotation has nowhere to go. Column 9 is left open
        // so that no row here is complete.
        game.matrix = from_bottom_rows(&["#########.", "###....##.", "#########."]);
        place(&mut game, PieceKind::I, Point::new(3, 37), Rotation::North);
        assert_eq!(
            tick_events(&mut game, &TickInput::action(Action::RotateCw)),
            vec![GameEvent::RotationFailed],
        );

        let mut off = new_game_with(
            GameplaySettings {
                allow_180_rotation: false,
                ..GameplaySettings::default()
            },
            TimingSettings::default(),
            41,
        );
        assert_eq!(
            tick_events(&mut off, &TickInput::action(Action::Rotate180)),
            vec![],
            "an inert key raises nothing, exactly as it resets nothing",
        );
    }

    #[test]
    fn topping_out_names_its_cause() {
        // §9.16, both ways round.
        let mut lock_out = new_game(47);
        let mut matrix = Matrix::new();
        for x in 0..WIDTH {
            matrix.set(x, 20, Some(PieceKind::I));
        }
        lock_out.matrix = matrix;
        place(
            &mut lock_out,
            PieceKind::O,
            Point::new(4, 18),
            Rotation::North,
        );
        let events = tick_events(&mut lock_out, &TickInput::action(Action::HardDrop));
        assert_eq!(
            events.last(),
            Some(&GameEvent::ToppedOut(TopOutCause::LockOut)),
        );

        let mut block_out = new_game(53);
        block_out.matrix = tower_over_the_spawn_columns();
        place(
            &mut block_out,
            PieceKind::O,
            Point::new(0, 36),
            Rotation::North,
        );
        let events = tick_events(&mut block_out, &TickInput::action(Action::HardDrop));
        assert_eq!(
            events.last(),
            Some(&GameEvent::ToppedOut(TopOutCause::BlockOut)),
        );
    }

    // -- T12: hold (§9.7) --------------------------------------------------

    /// Default gameplay with hold switched off (§9.7).
    fn without_hold() -> GameplaySettings {
        GameplaySettings {
            hold_enabled: false,
            ..GameplaySettings::default()
        }
    }

    /// Timing with a visible entry delay, so the gap between a lock and the
    /// next spawn can be looked at. The default is zero ticks (§6.6), which
    /// closes that gap entirely.
    fn with_entry_delay() -> TimingSettings {
        TimingSettings {
            entry_delay_ms: 100,
            ..TimingSettings::default()
        }
    }

    #[test]
    fn holding_an_empty_slot_banks_the_piece_and_takes_the_next() {
        // §9.7: the current piece goes to the slot and the queue supplies its
        // replacement, which is therefore one shorter than it was.
        let mut game = new_game(101);
        let banked = game.current().unwrap().kind;
        let queue: Vec<PieceKind> = game.preview().collect();
        let events = tick_events(&mut game, &TickInput::action(Action::Hold));

        assert_eq!(game.held(), Some(banked));
        assert_eq!(game.current().unwrap().kind, queue[0]);
        assert!(game.hold_locked());
        assert_eq!(
            game.preview().take(queue.len() - 1).collect::<Vec<_>>(),
            queue[1..],
            "the queue advanced by exactly one piece",
        );
        assert!(events.contains(&GameEvent::HoldUsed));
        assert!(events.contains(&GameEvent::PieceSpawned(queue[0])));
        assert_eq!(game.pieces(), 0, "a hold places nothing");
        assert_eq!(game.score(), 0, "and scores nothing");
    }

    #[test]
    fn holding_an_occupied_slot_exchanges_and_spawns_normally() {
        // §9.7: the piece coming out of hold arrives in `North` at its spawn
        // origin -- not at the outgoing piece's position or orientation.
        let mut game = new_game(103);
        let banked = game.current().unwrap().kind;
        tick(&mut game, &TickInput::action(Action::Hold));
        // A lock frees hold again; the replacement is then steered somewhere
        // thoroughly unlike a spawn before it is swapped out.
        drop_piece(&mut game, None, 0);
        let outgoing = game.current().unwrap().kind;
        tick(&mut game, &TickInput::action(Action::RotateCw));
        tick(&mut game, &TickInput::shift(Shift::Left));
        idle(&mut game, 120);
        assert_ne!(game.current().unwrap().origin, outgoing.spawn_origin());

        tick(&mut game, &TickInput::action(Action::Hold));
        let piece = game.current().unwrap();
        assert_eq!(piece.kind, banked, "the two pieces changed places");
        assert_eq!(game.held(), Some(outgoing));
        assert_eq!(piece.rotation, Rotation::North);
        assert_eq!(
            piece.origin,
            banked.spawn_origin().translate(0, 1),
            "spawned normally (§9.4), including the drop of one row",
        );
    }

    #[test]
    fn hold_is_spent_once_per_piece_and_returns_on_the_lock() {
        // §9.7, and the rule that is easiest to get a tick wrong: the lock-out
        // is cleared when the next piece *locks*, not when it spawns. With an
        // entry delay in force there is a moment between the two to look at.
        let mut game = new_game_with(GameplaySettings::default(), with_entry_delay(), 107);
        tick(&mut game, &TickInput::action(Action::Hold));
        let held = game.held();
        let current = game.current().unwrap();

        let events = tick_events(&mut game, &TickInput::action(Action::Hold));
        assert_eq!(
            events,
            vec![GameEvent::HoldRejected],
            "refused, and said so"
        );
        assert_eq!(game.held(), held, "and nothing moved");
        assert_eq!(game.current(), Some(current));

        tick(&mut game, &TickInput::action(Action::HardDrop));
        assert_eq!(game.state(), PlayState::Entry);
        assert_eq!(game.current(), None, "the next piece has not spawned yet");
        assert!(!game.hold_locked(), "the lock alone cleared it");
    }

    #[test]
    fn the_piece_out_of_hold_begins_fresh() {
        // §9.7: a hold clears the outgoing piece's lock-delay state entirely,
        // so a piece swapped away while grounded does not hand its spent
        // budget -- or its running timer -- to the one arriving at the top.
        let mut game = new_game(109);
        place(&mut game, PieceKind::T, Point::new(3, 38), Rotation::North);
        tick(&mut game, &TickInput::default());
        tick(&mut game, &TickInput::shift(Shift::Left));
        assert!(game.lock.remaining().is_some(), "grounded and counting");
        assert_eq!(game.lock.resets_used(), 1, "with one reset spent");

        tick(&mut game, &TickInput::action(Action::Hold));
        assert_eq!(game.lock.remaining(), None, "the new piece is airborne");
        assert_eq!(game.lock.resets_used(), 0, "with its budget untouched");
    }

    #[test]
    fn an_obstructed_hold_swap_is_a_block_out() {
        // T11, §9.16: "a newly spawned piece (from the queue **or from hold**)".
        let mut game = new_game(113);
        tick(&mut game, &TickInput::action(Action::Hold));
        game.matrix = tower_over_the_spawn_columns();
        // As the next lock would, so the second hold is allowed to happen.
        game.hold_locked = false;

        let events = tick_events(&mut game, &TickInput::action(Action::Hold));
        assert!(game.is_over(), "the held piece had nowhere to spawn");
        assert_eq!(
            events.last(),
            Some(&GameEvent::ToppedOut(TopOutCause::BlockOut)),
        );
    }

    #[test]
    fn a_disabled_hold_is_a_no_op_and_leaves_the_sequence_alone() {
        // T12: "with `hold_enabled = false` the hold action is a no-op and the
        // piece sequence is unaffected." The key is dropped at the input
        // boundary (§10.1); this is the core's backstop for a caller that
        // hands one over anyway, and the sequence is what would give it away.
        let mut pressed = new_game_with(without_hold(), TimingSettings::default(), 127);
        let mut untouched = new_game_with(without_hold(), TimingSettings::default(), 127);
        for i in 0..600u32 {
            let held_down = i % 7 == 0;
            let input = if held_down {
                TickInput::action(Action::Hold)
            } else {
                TickInput::default()
            };
            let events = tick_events(&mut pressed, &input);
            tick(&mut untouched, &TickInput::default());
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, GameEvent::HoldUsed | GameEvent::HoldRejected)),
                "an inert key raises nothing on tick {i}",
            );
            assert_eq!(pressed.view(), untouched.view(), "diverged on tick {i}");
        }
        assert_eq!(pressed.held(), None, "nothing was ever banked");
        assert!(!pressed.hold_locked());
    }

    #[test]
    fn what_the_caller_does_with_the_events_cannot_change_the_game() {
        // T16: "discarding every event leaves the game state bit-identical."
        // One game's events are thrown away every tick, the other's pile up in
        // a buffer that is never cleared. If the core ever read `out` back --
        // branching on its length, say -- the two would diverge.
        let mut discarded = new_game(59);
        let mut hoarded = new_game(59);
        let mut pile = Vec::new();
        for input in &script(900) {
            discarded.tick(input, &mut Vec::new());
            hoarded.tick(input, &mut pile);
            assert_eq!(discarded.view(), hoarded.view());
        }
        assert!(!pile.is_empty(), "the script did something worth reporting");
    }
}
