//! The capabilities the front-end lends the shell (§3.1, `FRONTEND.md` F1-F4).
//!
//! §3.1's "no I/O and no clock" is the core's rule, and it extends one layer
//! out: the shell has no clock, no filesystem, no entropy and no calendar of
//! its own. Three of the four are values it must be *given* and are gathered
//! here; the fourth, time, travels as an argument instead, because every entry
//! point above the core already takes the moment it is being called at
//! ([`Stamp`](crate::shell::time::Stamp), F1).
//!
//! This is not a `trait Frontend` and must not become one. It is a bundle of
//! *values*: the front-ends share the shell by calling it, not by satisfying an
//! interface designed before the third one existed (`FRONTEND.md`, "Adding a
//! front-end").

use crate::shell::storage::Storage;

/// What a run borrows from the front-end for its whole length.
///
/// Borrowed rather than owned so that a front-end keeps its own store — `ftm`
/// still has to write §6.2's commented default file after the loop has handed
/// back (§8.3).
pub struct Host<'a> {
    /// §6.2 and §14's two slots of text (F2).
    pub storage: &'a mut dyn Storage,
    /// One `u64` per game, and the whole entropy budget of the program (F3):
    /// §9.6's expansion of it into a piece sequence is `core/bag.rs`'s and is
    /// written out there precisely so that it never changes.
    pub seed: fn() -> u64,
    /// §14's `YYYY-MM-DD` date stamp (F4). A calendar is a platform facility
    /// like the other three, so `chrono` is a front-end dependency.
    pub today: fn() -> String,
}

impl<'a> Host<'a> {
    pub fn new(storage: &'a mut dyn Storage, seed: fn() -> u64, today: fn() -> String) -> Self {
        Self {
            storage,
            seed,
            today,
        }
    }
}
