//! Lock down: extended placement and its variants (§9.11).
//!
//! The state machine has one job and several ways to get it subtly wrong, so
//! each rule is named here:
//!
//! - A piece becomes *landed* when a downward move is blocked, and the timer
//!   starts then.
//! - A successful move or rotation while landed resets the timer to full and
//!   spends one of 15 resets.
//! - Reaching a row **lower than any the piece has occupied** cancels the timer
//!   and restores the whole reset budget. Being kicked back up does not.
//! - If the timer expires while the piece is no longer resting on anything, it
//!   does **not** lock; the timer is cancelled and gravity resumes.
//!
//! The variants change only the reset rule: `infinite` never caps the resets,
//! `classic` never grants one.

use crate::config::LockDownRule;

/// The reset budget of extended placement (§9.11).
pub const MAX_RESETS: u32 = 15;

/// What the caller should do after advancing the timer by a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockOutcome {
    /// Nothing to do this tick.
    Waiting,
    /// The timer expired while the piece was resting: lock it now.
    Lock,
}

/// The lock-down timer for the piece currently in play (§9.11).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockDown {
    rule: LockDownRule,
    delay: u32,
    /// Ticks left before the piece locks, or `None` when it is airborne.
    timer: Option<u32>,
    resets_used: u32,
    /// The lowest row any mino of this piece has reached, which is what the
    /// reset budget is measured against. `None` until the piece is placed.
    lowest_row_reached: Option<i32>,
}

impl LockDown {
    /// A timer for a newly spawned piece, airborne and with a full budget.
    pub fn new(rule: LockDownRule, delay: u32) -> Self {
        Self {
            rule,
            delay: delay.max(1),
            timer: None,
            resets_used: 0,
            lowest_row_reached: None,
        }
    }

    /// Whether the piece is currently counting down to a lock.
    pub fn is_landed(&self) -> bool {
        self.timer.is_some()
    }

    /// Ticks remaining, for the debug overlay (§6.3 `show_debug`).
    pub fn remaining(&self) -> Option<u32> {
        self.timer
    }

    pub fn resets_used(&self) -> u32 {
        self.resets_used
    }

    /// Record where the piece now sits, before anything else this tick.
    ///
    /// Reaching a new lowest row cancels the timer and restores the reset
    /// budget (§9.11). A kick that lifts the piece back up does not: the row is
    /// a high-water mark for the piece's whole lifetime, not its current
    /// position.
    pub fn observe_row(&mut self, lowest_row: i32) {
        if self.lowest_row_reached.is_none_or(|seen| lowest_row > seen) {
            self.lowest_row_reached = Some(lowest_row);
            self.timer = None;
            self.resets_used = 0;
        }
    }

    /// The piece's downward move was blocked: start the timer if it is not
    /// already running (§9.11).
    pub fn land(&mut self) {
        if self.timer.is_none() {
            self.timer = Some(self.delay);
        }
    }

    /// The piece is airborne again — a move or rotation left it over a hole.
    ///
    /// The timer is cancelled, not paused. The reset budget is deliberately
    /// kept: only a new lowest row restores that.
    pub fn lift(&mut self) {
        self.timer = None;
    }

    /// A move or rotation succeeded while landed (§9.11).
    ///
    /// Returns whether it actually bought any time, which is what the shell
    /// needs to know for the "step" sound in §12.5. Under `classic` the answer
    /// is always no; under `infinite` always yes; under `extended` it is yes for
    /// the first fifteen and no afterwards, though the move itself is still
    /// permitted.
    pub fn on_move(&mut self) -> bool {
        if self.timer.is_none() {
            return false;
        }
        match self.rule {
            LockDownRule::Classic => false,
            LockDownRule::Infinite => {
                self.resets_used += 1;
                self.timer = Some(self.delay);
                true
            }
            LockDownRule::Extended => {
                if self.resets_used >= MAX_RESETS {
                    false
                } else {
                    self.resets_used += 1;
                    self.timer = Some(self.delay);
                    true
                }
            }
        }
    }

    /// Advance one tick.
    ///
    /// `resting` is whether the piece is still supported *now*, which the caller
    /// re-tests each tick: a piece whose timer runs out in mid-air does not
    /// lock, and its timer is cancelled so gravity can take over again (§9.11).
    pub fn tick(&mut self, resting: bool) -> LockOutcome {
        let Some(timer) = self.timer else {
            return LockOutcome::Waiting;
        };
        let remaining = timer.saturating_sub(1);
        if remaining > 0 {
            self.timer = Some(remaining);
            return LockOutcome::Waiting;
        }
        self.timer = None;
        if resting {
            LockOutcome::Lock
        } else {
            LockOutcome::Waiting
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A landed piece under the default rule, with the default 30-tick delay.
    fn landed(rule: LockDownRule) -> LockDown {
        let mut lock = LockDown::new(rule, 30);
        lock.observe_row(39);
        lock.land();
        lock
    }

    /// Run `ticks` ticks with the piece resting, returning the tick the lock
    /// happened on, if it did.
    fn run(lock: &mut LockDown, ticks: u32) -> Option<u32> {
        (1..=ticks).find(|_| lock.tick(true) == LockOutcome::Lock)
    }

    #[test]
    fn extended_placement_locks_after_exactly_thirty_ticks() {
        // T7. 30 ticks is the default lock_delay_ms of 500 (§6.6).
        let mut lock = landed(LockDownRule::Extended);
        assert!(lock.is_landed());
        assert_eq!(run(&mut lock, 100), Some(30));
        assert!(!lock.is_landed(), "the timer is spent, not restarted");
    }

    #[test]
    fn the_timer_only_starts_when_the_piece_lands() {
        let mut lock = LockDown::new(LockDownRule::Extended, 30);
        lock.observe_row(30);
        assert!(!lock.is_landed());
        assert_eq!(run(&mut lock, 1000), None, "an airborne piece never locks");
        lock.land();
        assert_eq!(run(&mut lock, 100), Some(30));
    }

    #[test]
    fn landing_twice_does_not_restart_the_timer() {
        // `land` is called every tick the piece is grounded, so it must be
        // idempotent -- otherwise a resting piece would never lock at all.
        let mut lock = landed(LockDownRule::Extended);
        for _ in 0..20 {
            lock.land();
            assert_eq!(lock.tick(true), LockOutcome::Waiting);
        }
        assert_eq!(lock.remaining(), Some(10));
    }

    #[test]
    fn fifteen_resets_extend_the_timer_and_the_sixteenth_does_not() {
        // T7, the heart of extended placement.
        let mut lock = landed(LockDownRule::Extended);
        for reset in 1..=MAX_RESETS {
            for _ in 0..29 {
                assert_eq!(lock.tick(true), LockOutcome::Waiting);
            }
            assert!(lock.on_move(), "reset {reset} should have been granted");
            assert_eq!(lock.remaining(), Some(30));
        }
        assert_eq!(lock.resets_used(), MAX_RESETS);
        // The sixteenth move is still allowed, but buys nothing.
        for _ in 0..29 {
            assert_eq!(lock.tick(true), LockOutcome::Waiting);
        }
        assert!(!lock.on_move(), "the 16th reset must not be granted");
        assert_eq!(lock.remaining(), Some(1));
        assert_eq!(lock.tick(true), LockOutcome::Lock);
    }

    #[test]
    fn a_new_lowest_row_restores_the_reset_budget() {
        // T7. The budget is spent per resting place, not per piece.
        let mut lock = landed(LockDownRule::Extended);
        for _ in 0..MAX_RESETS {
            lock.on_move();
        }
        assert_eq!(lock.resets_used(), MAX_RESETS);
        assert!(!lock.on_move(), "budget spent");

        lock.observe_row(40);
        assert_eq!(lock.resets_used(), 0, "a lower row restores the budget");
        assert!(!lock.is_landed(), "and cancels the timer");
        lock.land();
        assert!(lock.on_move());
    }

    #[test]
    fn being_kicked_upwards_does_not_restore_the_budget() {
        // §9.11: "Moving back up via a kick does not reset it." The tracked row
        // is a high-water mark for the piece's whole lifetime.
        let mut lock = landed(LockDownRule::Extended);
        for _ in 0..MAX_RESETS {
            lock.on_move();
        }
        lock.observe_row(35);
        assert_eq!(
            lock.resets_used(),
            MAX_RESETS,
            "a kick upwards buys nothing"
        );
        assert!(!lock.on_move());
        // Falling back to where it had already been buys nothing either.
        lock.observe_row(39);
        assert_eq!(lock.resets_used(), MAX_RESETS);
        // Only genuinely new ground does.
        lock.observe_row(39 + 1);
        assert_eq!(lock.resets_used(), 0);
    }

    #[test]
    fn a_piece_moved_into_mid_air_at_expiry_does_not_lock() {
        // T7, and the case the plan calls out explicitly. A move that leaves the
        // piece over a hole must not lock it in mid-air when the timer runs out.
        let mut lock = landed(LockDownRule::Extended);
        for _ in 0..29 {
            assert_eq!(lock.tick(true), LockOutcome::Waiting);
        }
        // The last tick, but the piece is no longer resting on anything.
        assert_eq!(lock.tick(false), LockOutcome::Waiting);
        assert!(!lock.is_landed(), "the timer is cancelled, not paused");
        // Gravity resumes; when the piece next lands it gets a full delay.
        lock.land();
        assert_eq!(run(&mut lock, 100), Some(30));
    }

    #[test]
    fn lift_cancels_the_timer_but_keeps_the_budget() {
        // Only a new lowest row restores the budget (§9.11), so a piece cannot
        // buy fresh resets by shuffling on and off a ledge at the same height.
        let mut lock = landed(LockDownRule::Extended);
        for _ in 0..MAX_RESETS {
            lock.on_move();
        }
        lock.lift();
        assert!(!lock.is_landed());
        lock.land();
        assert_eq!(lock.resets_used(), MAX_RESETS);
        assert!(!lock.on_move());
    }

    #[test]
    fn a_move_while_airborne_resets_nothing() {
        let mut lock = LockDown::new(LockDownRule::Extended, 30);
        lock.observe_row(20);
        assert!(!lock.on_move());
        assert_eq!(lock.resets_used(), 0, "resets are only spent while landed");
    }

    #[test]
    fn infinite_never_runs_out_of_resets() {
        // T7, variant two.
        let mut lock = landed(LockDownRule::Infinite);
        for reset in 1..=200 {
            for _ in 0..29 {
                assert_eq!(lock.tick(true), LockOutcome::Waiting);
            }
            assert!(lock.on_move(), "reset {reset} should have been granted");
        }
        assert_eq!(
            run(&mut lock, 100),
            Some(30),
            "it still locks if left alone"
        );
    }

    #[test]
    fn classic_is_never_reset_at_all() {
        // T7, variant three: the timer is set on landing and that is that.
        let mut lock = landed(LockDownRule::Classic);
        for _ in 0..29 {
            assert!(!lock.on_move(), "classic grants no resets");
            assert_eq!(lock.tick(true), LockOutcome::Waiting);
        }
        assert_eq!(lock.tick(true), LockOutcome::Lock, "on tick 30 regardless");
        assert_eq!(lock.resets_used(), 0);
    }

    #[test]
    fn classic_still_honours_a_new_lowest_row() {
        // §9.11 defines `classic` as "never reset by moves or rotations". The
        // lowest-row rule is neither, and a piece that falls further must get a
        // fresh delay under every variant.
        let mut lock = landed(LockDownRule::Classic);
        for _ in 0..29 {
            lock.tick(true);
        }
        lock.observe_row(40);
        assert!(!lock.is_landed());
        lock.land();
        assert_eq!(run(&mut lock, 100), Some(30));
    }

    #[test]
    fn a_one_tick_delay_still_takes_a_tick() {
        // §6.6 guarantees lock_delay_ticks >= 1; this is the shape of the
        // shortest legal one.
        let mut lock = LockDown::new(LockDownRule::Extended, 1);
        lock.observe_row(39);
        lock.land();
        assert_eq!(lock.tick(true), LockOutcome::Lock);
    }
}
