//! The automated player (`PILOT.md`).
//!
//! A sibling of [`crate::core`] and [`crate::shell`], not a part of either. It
//! may name the core's façade and §P2.3's search seam, and
//! `shell::config::RulesConfig`, and nothing else in either layer: the planner
//! must not learn what a screen, a key or a stamp is.
//!
//! **It takes no clock** (§P3.3). No `Instant`, no frame count, no wall-time
//! budget, and no search amortised across frames — so a PILOT round plays the
//! same game at 60 Hz, at 144 Hz and at a jittery cadence, which is §19.4's
//! property one layer up. That is the house rule for the third time: the core
//! takes no clock, the shell takes no clock, and neither does the player.
//! `make shell` and `make portable` are the compiler holding it, exactly as
//! they hold §3.1 for the shell — a `cfg(target_arch)` in here is a bug, not a
//! fix.
//!
//! **It uses no floating point** (§P5). The features are integers, the weights
//! are integers and the evaluation is integer arithmetic, so a plan is
//! identical on every target and in every build profile — §9.9's reason, one
//! layer up again.
//!
//! What is here (`PILOT-PLAN.md` P2 and P3):
//!
//! * [`Pilot`] and [`Settings`] — §P3.4's controller, which is the whole of the
//!   player seen from outside: fed a tick's events, asked for a tick's input.
//! * [`knowledge`] — what the planner is allowed to know (§P2.1), folded out of
//!   the event stream and inferred from §9.6's bag arithmetic.
//! * [`fork`] — the object a search runs on: the fair fork of §P2.3.
//! * [`eval`] — §P5's board features and the integer evaluator over them, split
//!   at the line between a leaf and an interior ply.
//! * [`bench`] — §P8.2's report: a batch of headless games, folded into
//!   integers. The half of the benchmark that has no clock in it, which is why
//!   it is in here and `src/bench.rs` is not.
//!
//! Those four are public because §P8's runner and §P9's acceptance checks are
//! written against them from outside the crate. The three modules under the
//! controller are not: `placement` is §P4.1's generator, `search` is §P6's
//! lookahead over it and `controller` is the plan they feed, and all three are
//! reached through [`Pilot`] alone.
//!
//! Since P6 it looks two plies ahead (§P6.1), over a beam of deduplicated states
//! with a transposition cache, chance nodes past the preview blended 80/20, and
//! an integer node budget that always returns the best *fully* evaluated move.
//! What is left is §P4.2's exact placements, in P7: everything a hard drop
//! cannot reach.

pub mod bench;
mod controller;
pub mod eval;
pub mod fork;
pub mod knowledge;
mod placement;
mod search;

pub use controller::{Counted, Pilot, Settings};
