//! The two slots of text the shell keeps between runs (§6.2, §14,
//! `FRONTEND.md` F2).
//!
//! §6.2's config is TOML and §14's table is JSON; neither is large and neither
//! is binary, so the whole of the shell's persistence is two strings. What is
//! deliberately *not* here is a path: a browser tab has no paths, and the
//! filesystem is a front-end capability like the clock (§3.1).
//!
//! Every rule about the bytes stays in [`config`](crate::shell::config) and
//! [`highscore`](crate::shell::highscore) — the value-by-value parse of §6.3,
//! the clamping, the warnings, §14's capacity and tie-breaking. What moves out
//! is only where they are kept and how they are written: §14's atomic write is
//! a *filesystem technique*, and the trait promises durability rather than a
//! technique.

/// Which of the two things the shell keeps between runs (§6.2, §14).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    /// §6.2's TOML document.
    Config,
    /// §14's JSON table.
    HighScores,
}

impl Slot {
    /// What §16's warnings call the slot when there is no better name for it.
    ///
    /// A [`StorageError::Failed`] carries the store's own words — a native host
    /// names the file — but a slot that was read successfully and turned out to
    /// hold nonsense is the parser's complaint, and the parser has no idea
    /// where the bytes came from.
    pub const fn name(self) -> &'static str {
        match self {
            Slot::Config => "config file",
            Slot::HighScores => "high-score table",
        }
    }
}

/// Why a slot could not be read or written (§16).
///
/// Both variants are recoverable by construction: §16 gives the shell no way to
/// abort over storage, and every caller here degrades to a default and adds a
/// line to the warnings printed at exit.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StorageError {
    /// There is nowhere to keep this slot on this platform — no config
    /// directory, no `localStorage` in a sandboxed frame.
    ///
    /// It is reported by the *read*, which is what warns about it, and a later
    /// write to the same slot says nothing: the player has been told once
    /// (§6.2, §14).
    #[error("nowhere to keep it on this platform")]
    Unavailable,
    /// The store is there and the operation failed. The message is the store's
    /// own and names the location if it has one, because it is what a player
    /// reads at exit and then goes and fixes.
    #[error("{0}")]
    Failed(String),
}

impl StorageError {
    /// The line §16 prints for this, or `None` when there is nothing new to
    /// say.
    ///
    /// A slot with nowhere to live warned once, when it was *read*; a write to
    /// the same slot a moment later is the same news and the player does not
    /// need it twice (§6.2, §14).
    pub fn warning(&self) -> Option<&str> {
        match self {
            StorageError::Unavailable => None,
            StorageError::Failed(message) => Some(message),
        }
    }
}

/// Where the shell's two slots live, whatever that means on this front-end
/// (`FRONTEND.md` F2).
///
/// `Ok(None)` is "nothing stored yet", which is what every first run looks like
/// and is never worth a warning. The obligations that come with implementing
/// this are §6.2's location, §14's durability, §16's "a failure is a warning
/// and never an abort" and §6.2's rule that saving must not drop what another
/// front-end wrote.
pub trait Storage {
    fn read(&self, slot: Slot) -> Result<Option<String>, StorageError>;
    fn write(&mut self, slot: Slot, contents: &str) -> Result<(), StorageError>;
}

/// A store that remembers only for as long as the process runs.
///
/// It is the honest answer for a front-end with nowhere to write — and it is
/// what the shell's own tests use, because §6.2 and §14 are rules about *text*
/// and testing them through a temp directory tests the filesystem instead.
#[derive(Clone, Debug, Default)]
pub struct Memory {
    config: Option<String>,
    scores: Option<String>,
}

impl Memory {
    pub fn new() -> Self {
        Self::default()
    }

    /// A store already holding `contents`, for a test that wants to load
    /// something.
    pub fn holding(slot: Slot, contents: &str) -> Self {
        let mut memory = Self::new();
        let _ = memory.write(slot, contents);
        memory
    }

    fn slot(&mut self, slot: Slot) -> &mut Option<String> {
        match slot {
            Slot::Config => &mut self.config,
            Slot::HighScores => &mut self.scores,
        }
    }
}

impl Storage for Memory {
    fn read(&self, slot: Slot) -> Result<Option<String>, StorageError> {
        Ok(match slot {
            Slot::Config => self.config.clone(),
            Slot::HighScores => self.scores.clone(),
        })
    }

    fn write(&mut self, slot: Slot, contents: &str) -> Result<(), StorageError> {
        *self.slot(slot) = Some(contents.to_string());
        Ok(())
    }
}

/// A store that reads nothing and refuses every write, for §16's failure paths.
///
/// Not a test fixture with a test's licence to be unrealistic: it is exactly
/// what a read-only config directory looks like from up here, and §17.3's
/// sign-off exercised that case on a real one.
#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub struct Unwritable;

#[cfg(test)]
impl Storage for Unwritable {
    fn read(&self, _slot: Slot) -> Result<Option<String>, StorageError> {
        Ok(None)
    }

    fn write(&mut self, slot: Slot, _contents: &str) -> Result<(), StorageError> {
        Err(StorageError::Failed(format!(
            "{}: permission denied",
            slot.name(),
        )))
    }
}

/// A store with nowhere to keep anything: the platform admits to no config or
/// data directory at all (§6.2, §14).
#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub struct Homeless;

#[cfg(test)]
impl Storage for Homeless {
    fn read(&self, _slot: Slot) -> Result<Option<String>, StorageError> {
        Err(StorageError::Unavailable)
    }

    fn write(&mut self, _slot: Slot, _contents: &str) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_store_reads_as_nothing_stored() {
        // The ordinary first run: absent is not an error, in either slot.
        let memory = Memory::new();
        assert_eq!(memory.read(Slot::Config), Ok(None));
        assert_eq!(memory.read(Slot::HighScores), Ok(None));
    }

    #[test]
    fn the_two_slots_are_separate() {
        let mut memory = Memory::new();
        memory.write(Slot::Config, "a").expect("wrote it");
        assert_eq!(memory.read(Slot::Config), Ok(Some("a".to_string())));
        assert_eq!(memory.read(Slot::HighScores), Ok(None));
        memory.write(Slot::HighScores, "b").expect("wrote it");
        assert_eq!(memory.read(Slot::Config), Ok(Some("a".to_string())));
        assert_eq!(memory.read(Slot::HighScores), Ok(Some("b".to_string())));
    }
}
