//! T-spin and T-spin mini detection (§9.13).
//!
//! The "last action was a rotation" flag must survive a hard drop, and kick test
//! 5 always means a proper T-spin. This is the most commonly botched rule in the
//! specification.

// TODO(stage 7): the three-corner rule, front/back corners per orientation, the
// kick-index-5 override, and the last-action precondition.
