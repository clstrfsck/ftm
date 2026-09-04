//! Speed curve, gravity accumulation and level progression (§9.9).
//!
//! Gravity is an integer **fall period** in 16.16 ticks-per-row — a period, not
//! a rate — so level 1 falls on tick 60 exactly. No floating point (§9.9).

// TODO(stage 4): the fall-period table, the integer accumulator, the soft-drop
// divisor (§9.10) and level progression.
