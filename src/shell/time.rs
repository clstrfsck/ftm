//! The monotonic timestamp the shell is handed instead of reading a clock
//! (§3.1, `FRONTEND.md` F1).
//!
//! `std::time::Instant` is a platform facility: on `wasm32-unknown-unknown`
//! `Instant::now()` panics, because there is no such clock in the platform.
//! §3.1's "no clock" rule therefore extends one layer out — the shell takes
//! time as a *value*, and the front-end, which is the only layer that knows
//! whether it is a terminal, a window or a browser tab, is the only thing that
//! knows where the value came from.
//!
//! `Duration` stays: it is `core::time::Duration`, it has no platform
//! dependency, and it is what all the arithmetic above the core is already in.
//!
//! A pleasant consequence, and the reason this is worth a type rather than a
//! bare `u64`: stamps can be *constructed*, so the whole shell becomes testable
//! without a clock — the property §17.1 gives the core, one layer out.

use std::ops::{Add, AddAssign, Sub};
use std::time::Duration;

/// Monotonic time since the front-end started, in microseconds.
///
/// Microseconds rather than milliseconds because §12.5's line-clear flash
/// alternates at 12 Hz and §10.3's ARR can be a single tick, and millisecond
/// resolution is visibly coarse at both. A `u64` of microseconds is about
/// 584,000 years of range, so nothing here has to think about the end of it.
///
/// Two properties are the front-end's to keep and are normative in
/// `FRONTEND.md` F1: a stamp is **monotonic** — never earlier than one already
/// handed over — and it is **wall-clock-paced**. Every subtraction here is
/// saturating, so a violation degrades rather than panics; it does not become
/// harmless.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stamp(u64);

impl Stamp {
    /// The moment the front-end started.
    pub const ZERO: Stamp = Stamp(0);

    /// A stamp from microseconds since the front-end started.
    pub const fn from_micros(micros: u64) -> Stamp {
        Stamp(micros)
    }

    /// A stamp from a `Duration` since the front-end started, which is how a
    /// host with an `Instant` origin makes one.
    pub fn from_elapsed(elapsed: Duration) -> Stamp {
        Stamp(u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX))
    }

    /// A stamp from seconds, for a front-end whose clock is an `f64` — which is
    /// what macroquad's `get_time()` hands over.
    ///
    /// The conversion happens here, at the boundary, and never in the rules:
    /// §9.9 and §6.6 keep floating point out of everything below this line.
    /// A negative or absurd input clamps rather than wrapping, because the
    /// alternative is a stamp that runs backwards.
    pub fn from_secs_f64(seconds: f64) -> Stamp {
        let micros = seconds * 1_000_000.0;
        if micros.is_nan() || micros <= 0.0 {
            return Stamp::ZERO;
        }
        Stamp(if micros >= u64::MAX as f64 {
            u64::MAX
        } else {
            micros as u64
        })
    }

    /// Microseconds since the front-end started.
    pub const fn as_micros(self) -> u64 {
        self.0
    }

    /// How long has passed since `earlier`, or zero if it is not earlier.
    ///
    /// Saturating rather than checked because a front-end that breaks F1's
    /// monotonicity must not be able to panic the game (§16); the animation it
    /// stutters is the visible cost.
    pub fn saturating_since(self, earlier: Stamp) -> Duration {
        Duration::from_micros(self.0.saturating_sub(earlier.0))
    }

    /// `self` plus `duration`, clamped at the end of the range.
    pub fn saturating_add(self, duration: Duration) -> Stamp {
        let micros = u64::try_from(duration.as_micros()).unwrap_or(u64::MAX);
        Stamp(self.0.saturating_add(micros))
    }
}

impl Add<Duration> for Stamp {
    type Output = Stamp;

    fn add(self, duration: Duration) -> Stamp {
        self.saturating_add(duration)
    }
}

impl AddAssign<Duration> for Stamp {
    fn add_assign(&mut self, duration: Duration) {
        *self = self.saturating_add(duration);
    }
}

impl Sub<Duration> for Stamp {
    type Output = Stamp;

    /// Saturating at [`Stamp::ZERO`], because there is no time before the
    /// front-end started and a stamp that wrapped there would be a clock
    /// running backwards.
    fn sub(self, duration: Duration) -> Stamp {
        let micros = u64::try_from(duration.as_micros()).unwrap_or(u64::MAX);
        Stamp(self.0.saturating_sub(micros))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_time_is_the_difference_between_two_stamps() {
        let start = Stamp::from_micros(1_000_000);
        let later = start + Duration::from_millis(250);
        assert_eq!(later.saturating_since(start), Duration::from_millis(250));
        assert_eq!(later.as_micros(), 1_250_000);
    }

    #[test]
    fn a_stamp_that_ran_backwards_yields_zero_rather_than_panicking() {
        // `FRONTEND.md` F1 makes monotonicity the front-end's obligation, and
        // §16 makes a broken one a degradation rather than an abort. This is
        // the whole of that promise.
        let start = Stamp::from_micros(1_000);
        let earlier = Stamp::from_micros(999);
        assert_eq!(earlier.saturating_since(start), Duration::ZERO);
        assert_eq!(start.saturating_since(start), Duration::ZERO);
    }

    #[test]
    fn addition_saturates_rather_than_wrapping() {
        let late = Stamp::from_micros(u64::MAX - 1);
        assert_eq!(late + Duration::from_secs(1), Stamp::from_micros(u64::MAX));
        let mut stamp = late;
        stamp += Duration::from_secs(1);
        assert_eq!(stamp, Stamp::from_micros(u64::MAX));
    }

    #[test]
    fn seconds_convert_at_the_boundary() {
        // A frame-based front-end hands over `f64` seconds; that is the one
        // place a float is allowed, and it stops here (§9.9).
        assert_eq!(Stamp::from_secs_f64(1.5), Stamp::from_micros(1_500_000));
        assert_eq!(Stamp::from_secs_f64(0.0), Stamp::ZERO);
        assert_eq!(
            Stamp::from_secs_f64(-1.0),
            Stamp::ZERO,
            "clamped, not wrapped"
        );
        assert_eq!(Stamp::from_secs_f64(f64::NAN), Stamp::ZERO);
        assert_eq!(Stamp::from_secs_f64(f64::MAX), Stamp::from_micros(u64::MAX));
    }

    #[test]
    fn subtraction_stops_at_the_start_of_the_run() {
        let stamp = Stamp::from_micros(500);
        assert_eq!(stamp - Duration::from_micros(200), Stamp::from_micros(300));
        assert_eq!(stamp - Duration::from_secs(1), Stamp::ZERO);
    }

    #[test]
    fn a_duration_beyond_the_range_clamps() {
        assert_eq!(
            Stamp::ZERO + Duration::MAX,
            Stamp::from_micros(u64::MAX),
            "584,000 years is enough, and running past it must not wrap",
        );
        assert_eq!(
            Stamp::from_elapsed(Duration::MAX),
            Stamp::from_micros(u64::MAX)
        );
    }
}
