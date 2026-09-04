//! Score table, back-to-back, combo and perfect clear (§9.14, §9.15).
//!
//! Every value in §9.14 is a *base*, multiplied by the level in force when the
//! piece locked — except the drop points, which are not multiplied at all. The
//! multiplication happens here, once, so that the table below can be read
//! against the specification line by line.

use crate::core::events::{ClearKind, GameEvent, ScoreReason};

/// Points per row descended under soft drop (§9.10), not multiplied by level.
pub const SOFT_DROP_PER_ROW: u64 = 1;

/// Points per row descended in a hard drop (§9.10), not multiplied by level.
pub const HARD_DROP_PER_ROW: u64 = 2;

/// The combo unit (§9.15): `50 × combo count × level`.
pub const COMBO_UNIT: u64 = 50;

/// The value a combo counter is reset to (§9.15). Two clears in a row are
/// needed before it pays anything.
pub const NO_COMBO: i32 = -1;

/// The §9.14 line-clear table, before the level multiplier.
///
/// `b2b` selects the back-to-back column, which **replaces** the base value
/// rather than adding to it (§9.15). The rows with no back-to-back column are
/// the ones that cannot be part of a chain: their two columns are equal here so
/// that the caller never has to ask which is which.
pub const fn line_clear_base(clear: ClearKind, b2b: bool) -> u64 {
    match clear {
        ClearKind::Single => 100,
        ClearKind::Double => 300,
        ClearKind::Triple => 500,
        ClearKind::Quad if b2b => 1200,
        ClearKind::Quad => 800,
        ClearKind::TSpin => 400,
        ClearKind::TSpinSingle if b2b => 1200,
        ClearKind::TSpinSingle => 800,
        ClearKind::TSpinDouble if b2b => 1800,
        ClearKind::TSpinDouble => 1200,
        ClearKind::TSpinTriple if b2b => 2400,
        ClearKind::TSpinTriple => 1600,
        ClearKind::TSpinMini => 100,
        ClearKind::TSpinMiniSingle if b2b => 300,
        ClearKind::TSpinMiniSingle => 200,
        ClearKind::TSpinMiniDouble if b2b => 600,
        ClearKind::TSpinMiniDouble => 400,
    }
}

/// The §9.14 perfect-clear bonus for a clear of `lines` rows, before the level
/// multiplier. Added on top of the line-clear score, not instead of it.
///
/// The table is indexed by the row count alone: a perfect clear is a perfect
/// clear however the rows were completed.
pub const fn perfect_clear_base(lines: usize, b2b: bool) -> u64 {
    match lines {
        1 if b2b => 1200,
        1 => 800,
        2 if b2b => 1800,
        2 => 1200,
        3 if b2b => 2400,
        3 => 1800,
        4 if b2b => 3200,
        4 => 2000,
        _ => 0,
    }
}

/// The running score and the state the §9.15 bonuses are computed from.
///
/// Owns the whole of §9.14 and §9.15: `Game` hands it locks and rows descended
/// and takes back a number. Nothing here reads the matrix or the clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scoring {
    score: u64,
    combo: i32,
    back_to_back: bool,
}

impl Default for Scoring {
    fn default() -> Self {
        Self::new()
    }
}

impl Scoring {
    pub const fn new() -> Self {
        Self {
            score: 0,
            // §9.15: "A combo counter starts at -1". The first clear of a run
            // takes it to 0, which still pays nothing.
            combo: NO_COMBO,
            back_to_back: false,
        }
    }

    /// The running total. A `u64`, and not capped (§9.14).
    pub const fn score(&self) -> u64 {
        self.score
    }

    /// The combo counter (§9.15). Below 1 there is no combo to show.
    pub const fn combo(&self) -> i32 {
        self.combo
    }

    /// Whether the back-to-back chain is active (§9.15) — that is, whether the
    /// last line clear was a difficult one. This is what the status bar's `B2B`
    /// indicator reads; whether a *particular* clear was paid at the chained
    /// rate is what [`Scoring::lock`] returns.
    pub const fn back_to_back(&self) -> bool {
        self.back_to_back
    }

    /// Award the rows a soft drop descended (§9.10): 1 point each, unmultiplied.
    pub fn soft_drop(&mut self, rows: u32, out: &mut Vec<GameEvent>) {
        self.award(
            u64::from(rows) * SOFT_DROP_PER_ROW,
            ScoreReason::SoftDrop,
            out,
        );
    }

    /// Award the rows a hard drop descended (§9.10): 2 points each,
    /// unmultiplied. A drop of zero rows awards nothing and says nothing.
    pub fn hard_drop(&mut self, rows: u32, out: &mut Vec<GameEvent>) {
        self.award(
            u64::from(rows) * HARD_DROP_PER_ROW,
            ScoreReason::HardDrop,
            out,
        );
    }

    /// Score a lock (§9.12 step 4) and move the chain and the combo on.
    ///
    /// `clear` is the classification from §9.13 and the completed row count;
    /// `None` is the ordinary lock that neither spun nor cleared. `level` is the
    /// level in force at the lock, before the line count moves it (§9.12 steps
    /// 4 and 7).
    ///
    /// Returns whether this clear was paid at the back-to-back rate, which is
    /// what `LinesCleared` reports and what the perfect-clear bonus that may
    /// follow it is judged by.
    pub fn lock(&mut self, clear: Option<ClearKind>, level: u32, out: &mut Vec<GameEvent>) -> bool {
        let Some(clear) = clear else {
            // §9.15: the combo ends on any lock that clears nothing. The chain
            // does not: only a *line clear* can break it.
            self.combo = NO_COMBO;
            return false;
        };

        // §9.15: back-to-back applies when a difficult clear follows another
        // one. The first Quad of a chain is not itself a back-to-back Quad --
        // it is what makes the next one possible.
        let b2b = self.back_to_back && clear.is_difficult();
        self.award(
            line_clear_base(clear, b2b) * u64::from(level),
            ScoreReason::LineClear(clear),
            out,
        );

        if clear.lines() == 0 {
            // A spin that completed nothing. It scores, and it ends the combo,
            // but it is not a line clear and so leaves the chain untouched.
            self.combo = NO_COMBO;
            return b2b;
        }

        self.combo += 1;
        if self.combo >= 1 {
            let combo = u64::try_from(self.combo).unwrap_or(0);
            self.award(
                COMBO_UNIT * combo * u64::from(level),
                ScoreReason::Combo,
                out,
            );
        }
        self.back_to_back = clear.is_difficult();
        b2b
    }

    /// Award the §9.15 perfect-clear bonus, once the rows have actually gone.
    ///
    /// `b2b` is the flag [`Scoring::lock`] returned for the clear that emptied
    /// the board, not the state of the chain afterwards: a first Quad leaves the
    /// chain active but was not itself paid at the chained rate.
    pub fn perfect_clear(&mut self, lines: usize, b2b: bool, level: u32, out: &mut Vec<GameEvent>) {
        self.award(
            perfect_clear_base(lines, b2b) * u64::from(level),
            ScoreReason::PerfectClear,
            out,
        );
    }

    /// Add `points` and say so. Zero points are not an award and raise nothing:
    /// a hard drop of no rows is silent (§9.10).
    fn award(&mut self, points: u64, reason: ScoreReason, out: &mut Vec<GameEvent>) {
        if points == 0 {
            return;
        }
        self.score = self.score.saturating_add(points);
        out.push(GameEvent::ScoreAwarded { points, reason });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Score one lock in isolation and report the points it awarded.
    fn lock(scoring: &mut Scoring, clear: Option<ClearKind>, level: u32) -> u64 {
        let before = scoring.score();
        scoring.lock(clear, level, &mut Vec::new());
        scoring.score() - before
    }

    #[test]
    fn every_row_of_the_score_table() {
        // T10, transcribed from §9.14 rather than computed, at level 1 so that
        // the numbers on the page are the numbers asserted. The back-to-back
        // column is the second one; a dash in the specification is a row that
        // cannot chain, and reads the same in both columns.
        for (clear, base, chained) in [
            (ClearKind::Single, 100, 100),
            (ClearKind::Double, 300, 300),
            (ClearKind::Triple, 500, 500),
            (ClearKind::Quad, 800, 1200),
            (ClearKind::TSpin, 400, 400),
            (ClearKind::TSpinSingle, 800, 1200),
            (ClearKind::TSpinDouble, 1200, 1800),
            (ClearKind::TSpinTriple, 1600, 2400),
            (ClearKind::TSpinMini, 100, 100),
            (ClearKind::TSpinMiniSingle, 200, 300),
            (ClearKind::TSpinMiniDouble, 400, 600),
        ] {
            assert_eq!(line_clear_base(clear, false), base, "{clear:?}");
            assert_eq!(line_clear_base(clear, true), chained, "{clear:?} b2b");
        }
    }

    #[test]
    fn the_perfect_clear_table() {
        // §9.14's second table, by the number of rows the clear removed.
        for (lines, base, chained) in [
            (1, 800, 1200),
            (2, 1200, 1800),
            (3, 1800, 2400),
            (4, 2000, 3200),
        ] {
            assert_eq!(perfect_clear_base(lines, false), base, "{lines} rows");
            assert_eq!(perfect_clear_base(lines, true), chained, "{lines} rows b2b");
        }
        assert_eq!(perfect_clear_base(0, false), 0, "nothing cleared, no bonus");
    }

    #[test]
    fn the_level_multiplies_the_line_clear_but_not_the_drop() {
        // §9.14: "All values below are multiplied by the current level except
        // where stated", and the drop rows are where it is stated.
        let mut scoring = Scoring::new();
        assert_eq!(lock(&mut scoring, Some(ClearKind::Triple), 7), 500 * 7);

        let mut scoring = Scoring::new();
        scoring.soft_drop(11, &mut Vec::new());
        scoring.hard_drop(5, &mut Vec::new());
        assert_eq!(scoring.score(), 11 + 5 * 2);
    }

    #[test]
    fn a_chain_forms_on_the_second_difficult_clear_and_not_the_first() {
        // §9.15: back-to-back applies when a difficult clear *follows* another.
        let mut scoring = Scoring::new();
        assert!(!scoring.back_to_back(), "no chain to begin with");
        assert_eq!(lock(&mut scoring, Some(ClearKind::Quad), 1), 800);
        assert!(scoring.back_to_back(), "the chain is now live");
        // The second Quad is paid at the chained rate, and the combo it is now
        // part of pays 50 on top.
        assert_eq!(lock(&mut scoring, Some(ClearKind::Quad), 1), 1200 + 50);
    }

    #[test]
    fn a_plain_clear_breaks_the_chain_and_a_barren_lock_does_not() {
        // §9.15: "Any line clear of 1-3 lines that is not a T-spin breaks the
        // chain. Locks that clear no lines do not break the chain."
        let mut scoring = Scoring::new();
        lock(&mut scoring, Some(ClearKind::Quad), 1);
        assert!(scoring.back_to_back());

        lock(&mut scoring, None, 1);
        assert!(scoring.back_to_back(), "an ordinary lock is not a clear");
        lock(&mut scoring, Some(ClearKind::TSpin), 1);
        assert!(scoring.back_to_back(), "a spin that cleared nothing either");

        lock(&mut scoring, Some(ClearKind::Triple), 1);
        assert!(!scoring.back_to_back(), "a plain Triple breaks it");
    }

    #[test]
    fn a_t_spin_and_a_quad_chain_with_each_other() {
        // The chain is over *difficult* clears, not over one kind of clear.
        let mut scoring = Scoring::new();
        assert_eq!(lock(&mut scoring, Some(ClearKind::TSpinDouble), 1), 1200);
        // Chained, plus the 50 for the second clear in a row.
        assert_eq!(lock(&mut scoring, Some(ClearKind::Quad), 1), 1200 + 50);
        // And back the other way, plus 100 for the third.
        assert_eq!(
            lock(&mut scoring, Some(ClearKind::TSpinMiniSingle), 1),
            300 + 100
        );
    }

    #[test]
    fn the_combo_counts_clears_and_not_locks() {
        // §9.15: the counter starts at -1, increments on every lock that clears
        // at least one line, and pays 50 x counter x level from 1 upwards.
        let mut scoring = Scoring::new();
        assert_eq!(scoring.combo(), NO_COMBO);

        assert_eq!(lock(&mut scoring, Some(ClearKind::Single), 2), 100 * 2);
        assert_eq!(scoring.combo(), 0, "the first clear pays no combo");

        assert_eq!(
            lock(&mut scoring, Some(ClearKind::Single), 2),
            100 * 2 + 50 * 2
        );
        assert_eq!(scoring.combo(), 1);

        assert_eq!(
            lock(&mut scoring, Some(ClearKind::Single), 2),
            100 * 2 + 50 * 2 * 2
        );
        assert_eq!(scoring.combo(), 2);

        lock(&mut scoring, None, 2);
        assert_eq!(scoring.combo(), NO_COMBO, "a lock that cleared nothing");
    }

    #[test]
    fn a_spin_that_clears_nothing_ends_the_combo() {
        // §9.15: "resets to -1 on any lock that clears none" -- a T-spin with
        // no lines scores 400 and still ends the run.
        let mut scoring = Scoring::new();
        lock(&mut scoring, Some(ClearKind::Single), 1);
        assert_eq!(scoring.combo(), 0);
        assert_eq!(lock(&mut scoring, Some(ClearKind::TSpin), 1), 400);
        assert_eq!(scoring.combo(), NO_COMBO);
    }

    #[test]
    fn the_perfect_clear_bonus_is_added_to_the_clear_that_earned_it() {
        // §9.15: "added on top of the line-clear score and also multiplied by
        // level". A first Quad is 800, and the all-clear bonus 2000 more.
        let mut scoring = Scoring::new();
        let b2b = scoring.lock(Some(ClearKind::Quad), 1, &mut Vec::new());
        assert!(!b2b, "the first Quad of a chain is not itself chained");
        scoring.perfect_clear(4, b2b, 1, &mut Vec::new());
        assert_eq!(scoring.score(), 800 + 2000);

        // The next one is chained on both halves.
        let b2b = scoring.lock(Some(ClearKind::Quad), 1, &mut Vec::new());
        assert!(b2b);
        scoring.perfect_clear(4, b2b, 1, &mut Vec::new());
        assert_eq!(scoring.score(), 800 + 2000 + 1200 + 50 + 3200);
    }

    #[test]
    fn every_award_says_why_and_a_zero_award_says_nothing() {
        // §12.8: `ScoreAwarded` is how the status line learns what happened, so
        // the reasons are distinct and each award is its own event. Nothing
        // happening raises nothing -- a hard drop of zero rows is silent
        // (§9.10).
        let mut events = Vec::new();
        let mut scoring = Scoring::new();
        scoring.hard_drop(0, &mut events);
        scoring.soft_drop(0, &mut events);
        assert!(events.is_empty(), "{events:?}");

        scoring.hard_drop(3, &mut events);
        lock(&mut scoring, Some(ClearKind::Single), 1);
        scoring.lock(Some(ClearKind::TSpinDouble), 1, &mut events);
        assert_eq!(
            events,
            vec![
                GameEvent::ScoreAwarded {
                    points: 6,
                    reason: ScoreReason::HardDrop,
                },
                GameEvent::ScoreAwarded {
                    points: 1200,
                    reason: ScoreReason::LineClear(ClearKind::TSpinDouble),
                },
                GameEvent::ScoreAwarded {
                    points: 50,
                    reason: ScoreReason::Combo,
                },
            ]
        );
    }
}
