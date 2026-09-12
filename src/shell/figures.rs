//! How the figures a screen shows are written (§12.4, §12.6).
//!
//! A score, a clock: two numbers every front-end prints, and prints the same
//! way, because the way they are written is the specification's and not a
//! rendering technique. The same reasoning put the menus' labels in
//! [`menus`](crate::shell::menus) (`EGUI-PLAN.md` G2) — a second front-end
//! that wrote its own would be a second §12.4.
//!
//! *Where* a figure is drawn, and at what size, is each front-end's own.

/// A score with `,` every three digits (§12.6, §13.3, §13.5; `GUI.md` §G4).
///
/// Wherever there is room for it. §12.4's stats box is the one exception, and
/// it is the terminal's: eight characters of interior hold every score a game
/// can plausibly reach as bare digits, but not once a comma costs a column
/// every thousand.
pub fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + (digits.len() - 1) / 3);
    for (i, digit) in digits.char_indices() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// `MM:SS`, capped at `99:59` (§12.4).
///
/// From the view's tick count and not from a wall clock (§11), so the time a
/// screen shows cannot drift from the game it is showing.
pub fn clock(ticks: u64) -> String {
    let seconds = (ticks / 60).min(99 * 60 + 59);
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// Pieces per second over the whole run, to one decimal place (§11, §12.6).
///
/// Integer arithmetic: the tenths are computed, not rounded off a float, so the
/// figure is the same on every platform.
pub fn pps(pieces: u32, ticks: u64) -> String {
    if ticks == 0 {
        return "0.0".to_string();
    }
    let tenths = u64::from(pieces) * 600 / ticks;
    format!("{}.{}", tenths / 10, tenths % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_per_second_is_over_the_whole_run() {
        // §11, and the §12.6 mock-up's own numbers: 128 pieces in 2:14.
        assert_eq!(pps(0, 0), "0.0");
        assert_eq!(pps(128, (2 * 60 + 14) * 60), "0.9");
        assert_eq!(pps(60, 60 * 60), "1.0");
        assert_eq!(pps(150, 60 * 60), "2.5");
    }

    #[test]
    fn scores_are_grouped_in_threes() {
        // The boundaries, because an off-by-one here puts a comma in front of
        // the number or leaves the last group short.
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(7), "7");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(12_480), "12,480");
        assert_eq!(thousands(999_999), "999,999");
        assert_eq!(thousands(1_234_567), "1,234,567");
        // The widest thing the §13.5 column is sized for, and the widest there
        // is at all.
        assert_eq!(thousands(u64::from(u32::MAX)), "4,294,967,295");
        assert_eq!(thousands(u64::MAX), "18,446,744,073,709,551,615");
    }

    #[test]
    fn the_clock_counts_ticks_and_stops_at_ninety_nine_fifty_nine() {
        // §11: elapsed time is counted in ticks and converted for display, so
        // it cannot drift from the game. §12.4 caps it.
        assert_eq!(clock(0), "00:00");
        assert_eq!(clock(59), "00:00");
        assert_eq!(clock(60), "00:01");
        assert_eq!(clock((2 * 60 + 14) * 60), "02:14");
        assert_eq!(clock(u64::MAX), "99:59");
    }
}
