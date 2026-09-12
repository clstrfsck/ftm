//! High-score table load and save (§14).
//!
//! The table is the one piece of state that outlives a run of the program, so
//! it is also the one place where a failed write must not cost anything: every
//! path here degrades to an empty table plus a line in the warnings printed at
//! exit (§16), and none of them can stop a game starting.
//!
//! Nothing in here reads a clock or a calendar: the date an entry is stamped
//! with arrives as a `String` the front-end produced (§3.1, `FRONTEND.md` F4),
//! so the qualification and ordering rules of §14 are testable without one.
//! Nor does anything here know where the table is kept — that is a
//! [`Storage`] slot, and §14's atomic write is the native store's technique
//! rather than this module's rule (F2).

use serde::{Deserialize, Serialize};

use crate::core::GameView;
use crate::shell::storage::{Slot, Storage, StorageError};

/// §14: the table holds the top ten.
pub const CAPACITY: usize = 10;
/// §12.6: name entry accepts up to twelve printable ASCII characters.
pub const NAME_MAX: usize = 12;
/// §12.6: an empty name becomes this.
pub const ANONYMOUS: &str = "ANON";
/// The only format version this build writes, and the only one it reads (§14).
const VERSION: u32 = 1;

/// One finished run (§14).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub score: u64,
    pub level: u32,
    pub lines: u32,
    /// Elapsed game time in seconds — ticks / 60, so it is the game's clock and
    /// not the wall's (§11).
    pub duration_secs: u64,
    /// `YYYY-MM-DD`, local time.
    pub date: String,
}

impl Entry {
    /// The entry a finished run earns (§11, §14).
    ///
    /// The date is a parameter rather than read here: it is the only value in
    /// the entry that does not come from the run, and passing it in is what
    /// keeps the ordering rules of §14 testable without a clock.
    pub fn of(name: &str, view: &GameView, date: String) -> Self {
        Self {
            name: tidy_name(name),
            score: view.score,
            level: view.level,
            lines: view.lines,
            duration_secs: view.ticks / crate::shell::config::TICK_HZ,
            date,
        }
    }
}

/// The whole file (§14).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Table {
    pub version: u32,
    pub entries: Vec<Entry>,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            version: VERSION,
            entries: Vec::new(),
        }
    }
}

impl Table {
    /// The rank a score would take, or `None` if it does not qualify (§14).
    ///
    /// Zero-based: rank 0 is the top of the table. §14's two rules are that a
    /// score of 0 never qualifies, and that a full table only admits a score
    /// strictly greater than its tenth.
    pub fn rank_for(&self, score: u64) -> Option<usize> {
        if score == 0 {
            return None;
        }
        if self.entries.len() >= CAPACITY && score <= self.entries[CAPACITY - 1].score {
            return None;
        }
        Some(
            self.entries
                .iter()
                .position(|entry| score > entry.score)
                .unwrap_or(self.entries.len()),
        )
    }

    /// File `entry` and report the rank it took, or `None` if it did not make
    /// the table (§14).
    ///
    /// Ties go to the entry that was already there: the new one is inserted
    /// *after* every equal score, which is the same rule as "earlier date
    /// first" for an entry stamped today.
    pub fn insert(&mut self, entry: Entry) -> Option<usize> {
        let at = self.rank_for(entry.score)?;
        self.entries.insert(at, entry);
        self.entries.truncate(CAPACITY);
        Some(at)
    }

    /// The first `n` entries, for the attract screen's panel (§13.3).
    pub fn top(&self, n: usize) -> &[Entry] {
        &self.entries[..n.min(self.entries.len())]
    }

    /// Put a table read from disk into the order §14 specifies, and hold it to
    /// the top ten.
    ///
    /// A hand-edited file is not an error — it is the ordinary way a player
    /// tampers with one — so it is sorted rather than rejected.
    fn tidy(&mut self) {
        self.entries.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.date.cmp(&b.date))
                .then_with(|| a.name.cmp(&b.name))
        });
        self.entries.truncate(CAPACITY);
        for entry in &mut self.entries {
            entry.name = tidy_name(&entry.name);
        }
    }

    /// Write the table (§14).
    ///
    /// §14 requires the write to be **durable against a crash mid-write** — a
    /// run that dies while saving leaves the old table or the new one, never a
    /// truncated file. That is a promise the store keeps, and how it keeps it
    /// is its own business: a temp file renamed over the target on a
    /// filesystem, nothing at all in `localStorage`, where `setItem` is
    /// already atomic (`FRONTEND.md` F2).
    ///
    /// §16 makes every failure here a warning rather than an abort, so the
    /// error is handed back for the caller to file.
    pub fn save(&self, storage: &mut dyn Storage) -> Result<(), StorageError> {
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| StorageError::Failed(error.to_string()))?;
        storage.write(Slot::HighScores, &text)
    }
}

/// Read the table, degrading to an empty one for anything unusable (§14).
///
/// Never fails. "Missing, malformed, unreadable" all yield an empty table; only
/// the last two are worth a warning, because a missing file is what every first
/// run looks like.
pub fn load(storage: &dyn Storage, warnings: &mut Vec<String>) -> Table {
    let text = match storage.read(Slot::HighScores) {
        Ok(Some(text)) => text,
        Ok(None) => return Table::default(),
        Err(StorageError::Unavailable) => {
            // Neutral about *where*, as §6.2's twin is: natively there is no
            // data directory, in a browser tab `localStorage` is blocked
            // (§3.1, `GUI.md` §G8.3).
            warnings.push(
                "nowhere to keep the scores on this platform; they are not recorded".to_string(),
            );
            return Table::default();
        }
        Err(StorageError::Failed(message)) => {
            warnings.push(format!("{message}; starting an empty table"));
            return Table::default();
        }
    };
    // A parse failure is the parser's complaint and it has no idea where the
    // bytes came from, so it is the slot that gets named rather than a path.
    let name = Slot::HighScores.name();
    let mut table: Table = match serde_json::from_str(&text) {
        Ok(table) => table,
        Err(error) => {
            warnings.push(format!("{name}: {error}; starting an empty table"));
            return Table::default();
        }
    };
    if table.version != VERSION {
        warnings.push(format!(
            "{name}: version {} is not {VERSION}; starting an empty table",
            table.version,
        ));
        return Table::default();
    }
    table.tidy();
    table
}

/// A name as the table stores it (§12.6): printable ASCII, at most twelve
/// characters, never empty.
///
/// Applied on the way in *and* on the way out of the file, so a hand-edited
/// entry cannot make the high-score table ragged or smuggle a control
/// character into the terminal.
pub fn tidy_name(raw: &str) -> String {
    let name: String = raw
        .chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ')
        .take(NAME_MAX)
        .collect();
    let name = name.trim();
    if name.is_empty() {
        ANONYMOUS.to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::storage::{Homeless, Memory, Unwritable};

    fn entry(name: &str, score: u64, date: &str) -> Entry {
        Entry {
            name: name.to_string(),
            score,
            level: 1,
            lines: 0,
            duration_secs: 0,
            date: date.to_string(),
        }
    }

    fn filled(scores: &[u64]) -> Table {
        let mut table = Table::default();
        for (n, score) in scores.iter().enumerate() {
            table
                .entries
                .push(entry(&format!("P{n}"), *score, "2026-01-01"));
        }
        table
    }

    #[test]
    fn a_short_table_admits_anything_but_zero() {
        // §14: "a score qualifies if it is greater than the tenth entry's
        // score, or the table has fewer than 10 entries. A score of 0 never
        // qualifies."
        let table = filled(&[500, 400, 300]);
        assert_eq!(table.rank_for(600), Some(0));
        assert_eq!(table.rank_for(350), Some(2));
        assert_eq!(table.rank_for(1), Some(3), "below every entry, but it fits");
        assert_eq!(table.rank_for(0), None, "and zero never does");
    }

    #[test]
    fn a_full_table_admits_only_a_better_score_than_its_tenth() {
        let table = filled(&[100, 90, 80, 70, 60, 50, 40, 30, 20, 10]);
        assert_eq!(table.entries.len(), CAPACITY);
        assert_eq!(table.rank_for(11), Some(9));
        assert_eq!(table.rank_for(10), None, "equal to the tenth is not better");
        assert_eq!(table.rank_for(9), None);
    }

    #[test]
    fn a_tie_leaves_the_older_entry_above() {
        // §14: "ties are broken by earlier date first (so an existing entry
        // keeps the better rank)".
        let mut table = filled(&[500, 300]);
        assert_eq!(table.insert(entry("NEW", 300, "2026-09-05")), Some(2));
        assert_eq!(
            table
                .entries
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            ["P0", "P1", "NEW"],
        );
    }

    #[test]
    fn inserting_holds_the_table_to_ten() {
        let mut table = filled(&[100, 90, 80, 70, 60, 50, 40, 30, 20, 10]);
        assert_eq!(table.insert(entry("TOP", 1000, "2026-09-05")), Some(0));
        assert_eq!(table.entries.len(), CAPACITY, "the tenth fell off the end");
        assert_eq!(table.entries[CAPACITY - 1].score, 20);
        assert_eq!(table.insert(entry("NO", 5, "2026-09-05")), None);
        assert_eq!(table.entries.len(), CAPACITY);
    }

    #[test]
    fn a_name_is_printable_ascii_and_never_empty() {
        // §12.6: up to twelve printable ASCII characters; an empty name becomes
        // ANON.
        assert_eq!(tidy_name(""), ANONYMOUS);
        assert_eq!(tidy_name("   "), ANONYMOUS);
        assert_eq!(tidy_name("msandiford"), "msandiford");
        assert_eq!(tidy_name("abcdefghijklmnop").len(), NAME_MAX);
        assert_eq!(
            tidy_name("a\u{7}b\u{1f600}c"),
            "abc",
            "controls and the rest"
        );
    }

    #[test]
    fn a_round_trip_through_the_store_keeps_every_entry() {
        let mut storage = Memory::new();
        let mut table = filled(&[500, 300]);
        table.insert(entry("NEW", 400, "2026-09-05"));
        table.save(&mut storage).expect("saved");

        let mut warnings = Vec::new();
        assert_eq!(load(&storage, &mut warnings), table);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn nothing_stored_is_an_empty_table_and_no_warning() {
        // §14: the game must never fail to start because of the table, and a
        // first run is not a problem worth mentioning.
        let mut warnings = Vec::new();
        assert_eq!(load(&Memory::new(), &mut warnings), Table::default());
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_malformed_table_is_an_empty_one_and_one_warning() {
        let storage = Memory::holding(Slot::HighScores, "{ not json at all");
        let mut warnings = Vec::new();
        assert_eq!(load(&storage, &mut warnings), Table::default());
        assert_eq!(warnings.len(), 1, "{warnings:?}");

        // A future version is refused the same way, rather than being read as
        // though its fields meant what they mean here.
        let storage = Memory::holding(Slot::HighScores, r#"{"version":2,"entries":[]}"#);
        let mut warnings = Vec::new();
        assert_eq!(load(&storage, &mut warnings), Table::default());
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn a_store_that_cannot_be_read_is_an_empty_table_and_one_warning() {
        // §14, §16: "missing, malformed, unreadable" all yield an empty table.
        // A platform with no data directory says so in its own words.
        let mut warnings = Vec::new();
        assert_eq!(load(&Homeless, &mut warnings), Table::default());
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn a_store_that_cannot_be_written_is_a_warning_and_nothing_else() {
        // §14: "any failure to write yields a warning at exit and is otherwise
        // ignored". The table in memory still holds the entry.
        let mut table = filled(&[500]);
        assert_eq!(table.insert(entry("NEW", 900, "2026-09-05")), Some(0));
        let error = table.save(&mut Unwritable).expect_err("refused");
        assert!(error.warning().is_some(), "{error:?}");
        assert_eq!(table.entries.len(), 2);
    }

    #[test]
    fn a_hand_edited_table_is_sorted_rather_than_rejected() {
        let mut table = Table::default();
        table.entries.push(entry("LOW", 10, "2026-01-01"));
        table.entries.push(entry("HIGH", 900, "2026-01-01"));
        table.entries.push(entry("MID", 500, "2026-01-01"));
        let storage = Memory::holding(
            Slot::HighScores,
            &serde_json::to_string(&table).expect("serialises"),
        );

        let mut warnings = Vec::new();
        let loaded = load(&storage, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            loaded
                .entries
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            ["HIGH", "MID", "LOW"],
        );
    }
}
