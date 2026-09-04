//! Key decoding, action mapping and DAS/ARR (§10).
//!
//! DAS/ARR are resolved here in the shell, never in the core (§10.3). Disabled
//! bindings are dropped at this boundary so they cannot reset a lock-delay timer
//! as a side effect (§10.1).

// TODO(stage 6): key -> Action/Held mapping and the §10.3 DAS/ARR state machine.
// TODO(stage 8): the `hold_enabled` / `allow_180_rotation` gates.
