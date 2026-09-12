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
//! What is here so far (`PILOT-PLAN.md` P2):
//!
//! * [`knowledge`] — what the planner is allowed to know (§P2.1), folded out of
//!   the event stream and inferred from §9.6's bag arithmetic.
//! * [`fork`] — the object a search runs on: the fair fork of §P2.3.
//! * [`eval`] — §P5's board features and the integer evaluator over them.
//!
//! §P3.4's `Pilot` and `Settings` — the controller that turns these into a
//! `TickInput` — arrive with the stages that can fill them in: placements and a
//! one-ply choice in P3, the menu item and the two front-ends in P4.

pub mod eval;
pub mod fork;
pub mod knowledge;
