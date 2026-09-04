//! Speed curve, gravity accumulation and level progression (§9.9, §9.10).
//!
//! Gravity is an integer **fall period** in 16.16 fixed-point ticks per row — a
//! period, not a rate. That distinction is the whole design: at level 1 the
//! period is exactly 3 932 160, so the piece falls on tick 60 and not on tick
//! 61, whereas a per-tick rate of `round(65536/60)` reaches only 65 520 after
//! sixty ticks and arrives late. It also makes speeds above 1 G fall out for
//! free, since the accumulator can clear the period more than once in a tick.
//!
//! The float formula is evaluated **only at a level change** and rounded to an
//! integer immediately (§9.9). No float survives into the rules, so the piece
//! sequence is bit-identical on every platform (§3.1, §15.4).

/// One row, in the 16.16 fixed point the accumulator and period share.
pub const ONE_ROW: u32 = 65_536;

/// The level beyond which the speed curve stops getting faster (§9.9).
///
/// Levels above this keep incrementing for scoring and display. The clamp is not
/// cosmetic: the base term of the formula goes negative past level 115, and long
/// before that the curve is meaningless.
pub const MAX_SPEED_LEVEL: u32 = 15;

/// Fall speed in seconds per row (§9.9):
/// `(0.8 - ((level - 1) * 0.007)) ^ (level - 1)`, with the level clamped to
/// [`MAX_SPEED_LEVEL`].
///
/// This is the one floating-point expression in the core. Its result is rounded
/// by [`fall_period`] before anything else sees it, and it is called only when
/// the level changes.
pub fn seconds_per_row(level: u32) -> f64 {
    let level = level.clamp(1, MAX_SPEED_LEVEL);
    let n = f64::from(level - 1);
    (0.8 - (n * 0.007)).powf(n)
}

/// The fall period for a level, in 16.16 ticks per row (§9.9):
/// `max(1, round(seconds_per_row(level) * TICK_HZ * 65536))`.
///
/// Computed from `seconds_per_row` at full precision, not from the
/// five-decimal values printed in the §9.9 speed table.
pub fn fall_period(level: u32) -> u32 {
    let ticks_per_row = seconds_per_row(level) * crate::config::TICK_HZ as f64;
    let period = (ticks_per_row * f64::from(ONE_ROW)).round();
    // Saturating rather than `as`, which would be undefined for a value past
    // u32::MAX. Level 1 is the slowest and is well inside the range.
    (period as u64).clamp(1, u64::from(u32::MAX)) as u32
}

/// The fall period while soft drop is held (§9.10):
/// `max(1, fall_period / soft_drop_factor)`.
///
/// Dividing a period can only shorten it, so soft drop never makes a piece
/// slower — including at level 15, where normal gravity is already above 1 G.
pub fn soft_drop_period(fall_period: u32, soft_drop_factor: u32) -> u32 {
    (fall_period / soft_drop_factor.max(1)).max(1)
}

/// The integer gravity accumulator of §9.9.
///
/// Each tick it gains one row's worth (`ONE_ROW`); while it holds at least a
/// full period, the piece owes another row and the period is subtracted. The
/// accumulator **carries over across level changes**, so a level-up mid-fall
/// neither loses nor gains the fraction of a row already accrued.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gravity {
    level: u32,
    period: u32,
    accumulator: u32,
}

impl Gravity {
    /// Start at `level`, with nothing accrued.
    pub fn new(level: u32) -> Self {
        Self {
            level,
            period: fall_period(level),
            accumulator: 0,
        }
    }

    pub fn level(&self) -> u32 {
        self.level
    }

    /// The current period, in 16.16 ticks per row.
    pub fn period(&self) -> u32 {
        self.period
    }

    /// Move to a new level, recomputing the period. The accumulator is
    /// deliberately left alone (§9.9).
    pub fn set_level(&mut self, level: u32) {
        if level != self.level {
            self.level = level;
            self.period = fall_period(level);
        }
    }

    /// Accrue one tick and report how many rows the piece now owes.
    ///
    /// `soft_drop_factor` applies only while the key is held (§9.10); pass
    /// `None` for normal gravity. At high levels this legitimately returns more
    /// than one, and under soft drop it can return more rows than the matrix
    /// has — the caller stops at the first blocked row either way.
    pub fn accrue(&mut self, soft_drop_factor: Option<u32>) -> u32 {
        let period = match soft_drop_factor {
            Some(factor) => soft_drop_period(self.period, factor),
            None => self.period,
        };
        self.accumulator = self.accumulator.saturating_add(ONE_ROW);
        let rows = self.accumulator / period;
        self.accumulator -= rows * period;
        rows
    }

    /// Discard the accrued fraction. §9.9: a blocked downward step resets the
    /// accumulator to 0 and hands over to the lock-down state machine (§9.11).
    pub fn reset_accumulator(&mut self) {
        self.accumulator = 0;
    }
}

/// Level progression (§9.9).
///
/// The level advances when cumulative cleared lines reach `lines_per_level *
/// level`, **at most once per line clear event** even when a single clear
/// crosses two thresholds; the surplus carries over. A quadruple at the wrong
/// moment must not skip a level's worth of speed.
pub fn level_after_clear(level: u32, total_lines: u32, lines_per_level: u32) -> u32 {
    let lines_per_level = lines_per_level.max(1);
    if total_lines >= lines_per_level.saturating_mul(level) {
        level + 1
    } else {
        level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The §9.9 speed table, transcribed. The formula is normative and the
    /// table informative, so this asserts they agree to the 5 decimal places
    /// the table is printed to.
    const SECONDS_PER_ROW: [(u32, f64); 15] = [
        (1, 1.00000),
        (2, 0.79300),
        (3, 0.61780),
        (4, 0.47273),
        (5, 0.35520),
        (6, 0.26200),
        (7, 0.18968),
        (8, 0.13473),
        (9, 0.09388),
        (10, 0.06415),
        (11, 0.04298),
        (12, 0.02822),
        (13, 0.01815),
        (14, 0.01144),
        (15, 0.00706),
    ];

    /// The §9.9 fall-period table, transcribed.
    const FALL_PERIODS: [(u32, u32); 5] = [
        (1, 3_932_160),
        (5, 1_396_691),
        (10, 252_254),
        (13, 71_382),
        (15, 27_756),
    ];

    #[test]
    fn the_speed_curve_matches_the_spec_table() {
        // T5.
        for (level, expected) in SECONDS_PER_ROW {
            let actual = seconds_per_row(level);
            assert!(
                (actual - expected).abs() < 5e-6,
                "level {level}: {actual:.7} is not {expected:.5}",
            );
        }
    }

    #[test]
    fn the_curve_is_flat_above_level_fifteen() {
        // T5: levels 16-100 keep the level-15 speed. Above ~115 the base term
        // of the formula goes negative, which is what the clamp is for.
        let fastest = seconds_per_row(MAX_SPEED_LEVEL);
        for level in 16..=100 {
            assert_eq!(seconds_per_row(level), fastest, "level {level}");
            assert_eq!(fall_period(level), fall_period(MAX_SPEED_LEVEL));
        }
        assert_eq!(fall_period(10_000), fall_period(MAX_SPEED_LEVEL));
    }

    #[test]
    fn gravity_never_slows_down_and_never_stops() {
        // T5: monotonically non-decreasing in level, and never zero.
        let mut previous = u32::MAX;
        for level in 1..=100 {
            let period = fall_period(level);
            assert!(period > 0, "level {level} has no gravity at all");
            assert!(
                period <= previous,
                "level {level} is slower than level {}",
                level - 1,
            );
            previous = period;
        }
    }

    #[test]
    fn the_fall_period_table_matches() {
        // T6.
        for (level, expected) in FALL_PERIODS {
            assert_eq!(fall_period(level), expected, "level {level}");
        }
    }

    #[test]
    fn a_piece_at_level_one_falls_on_every_sixtieth_tick() {
        // T6, and the reason §9.9 specifies a period rather than a rate: a rate
        // of round(65536/60) reaches 65 520 after sixty ticks and arrives late.
        let mut gravity = Gravity::new(1);
        for tick in 1..=600 {
            let rows = gravity.accrue(None);
            let expected = u32::from(tick % 60 == 0);
            assert_eq!(rows, expected, "tick {tick}");
        }
    }

    #[test]
    fn level_fifteen_falls_more_than_a_row_a_tick() {
        // T6.
        let mut gravity = Gravity::new(15);
        let rows = gravity.accrue(None);
        assert!(rows > 1, "level 15 dropped {rows} rows in a tick");
        assert_eq!(rows, ONE_ROW / fall_period(15));
    }

    #[test]
    fn soft_drop_at_level_one_falls_every_third_tick() {
        // T6. 3 932 160 / 20 = 196 608, exactly three ticks' worth.
        assert_eq!(soft_drop_period(fall_period(1), 20), ONE_ROW * 3);
        let mut gravity = Gravity::new(1);
        for tick in 1..=60 {
            let rows = gravity.accrue(Some(20));
            assert_eq!(rows, u32::from(tick % 3 == 0), "tick {tick}");
        }
    }

    #[test]
    fn soft_drop_never_makes_a_piece_slower() {
        // §9.10: dividing a period can only shorten it.
        for level in 1..=20 {
            let normal = fall_period(level);
            for factor in 1..=100 {
                assert!(soft_drop_period(normal, factor) <= normal, "level {level}");
            }
            assert_eq!(soft_drop_period(normal, 1), normal, "1 is no speed-up");
        }
        // A factor of 0 cannot reach here through RulesConfig (§6.3 clamps it),
        // but it must not divide by zero if it ever does.
        assert_eq!(soft_drop_period(fall_period(1), 0), fall_period(1));
    }

    #[test]
    fn the_accumulator_carries_across_a_level_change() {
        // §9.9: "The accumulator carries over across level changes." Losing it
        // would let a well-timed level-up cancel a piece's accrued fall.
        let mut gravity = Gravity::new(1);
        for _ in 0..59 {
            assert_eq!(gravity.accrue(None), 0);
        }
        // 59 ticks in: one tick short of a row at level 1.
        gravity.set_level(2);
        assert_eq!(gravity.period(), fall_period(2));
        // At level 2 the period is smaller than what has already accrued, so
        // the row is owed immediately rather than being forgotten.
        assert_eq!(gravity.accrue(None), 1);
    }

    #[test]
    fn a_blocked_step_clears_the_accumulator() {
        // §9.9: a blocked downward step resets the accumulator to 0 before the
        // lock-down machine takes over.
        let mut gravity = Gravity::new(1);
        for _ in 0..30 {
            gravity.accrue(None);
        }
        gravity.reset_accumulator();
        for tick in 1..=59 {
            assert_eq!(gravity.accrue(None), 0, "tick {tick} after the reset");
        }
        assert_eq!(gravity.accrue(None), 1);
    }

    #[test]
    fn a_level_advances_at_most_once_per_clear() {
        // §9.9: even a clear that crosses two thresholds advances one level, and
        // the surplus carries over.
        assert_eq!(level_after_clear(1, 9, 10), 1);
        assert_eq!(level_after_clear(1, 10, 10), 2);
        assert_eq!(level_after_clear(1, 13, 10), 2);
        // lines_per_level = 1: a quadruple crosses four thresholds and still
        // gains exactly one level.
        assert_eq!(level_after_clear(1, 4, 1), 2);
        // The surplus is not lost: the next clear takes the next level.
        assert_eq!(level_after_clear(2, 20, 10), 3);
        assert_eq!(level_after_clear(2, 19, 10), 2);
    }
}
