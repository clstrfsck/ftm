//! High-score table load and save (§14).
//!
//! The table is the one piece of state that outlives a run of the program, so
//! it is also the one place where a failed write must not cost anything: every
//! path here degrades to an empty table plus a line in the warnings printed at
//! exit (§16), and none of them can stop a game starting.
//!
//! Nothing in here reads a clock except [`today`], which is the only reason
//! `chrono` is a dependency at all. Every other function takes the date it is
//! to stamp, so the qualification and ordering rules of §14 are testable
//! without one.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::core::GameView;

/// §14: the table holds the top ten.
pub const CAPACITY: usize = 10;
/// §12.6: name entry accepts up to twelve printable ASCII characters.
pub const NAME_MAX: usize = 12;
/// §12.6: an empty name becomes this.
pub const ANONYMOUS: &str = "ANON";
/// The file name under the platform data directory (§14).
pub const FILE_NAME: &str = "highscores.json";
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
            duration_secs: view.ticks / crate::config::TICK_HZ,
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

    /// Write the table atomically (§14): a sibling temp file, then a rename.
    ///
    /// The rename is what makes it atomic, and it is only atomic because the
    /// temp file is in the same directory — a rename across filesystems is a
    /// copy.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, text)?;
        fs::rename(&temporary, path)
    }
}

/// `{data_dir}/ftm/highscores.json` (§14).
///
/// `None` only when the platform admits to no data directory, which is a
/// documented degradation: the game plays and simply records nothing.
pub fn default_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "ftm").map(|dirs| dirs.data_dir().join(FILE_NAME))
}

/// Read the table, degrading to an empty one for anything unusable (§14).
///
/// Never fails. "Missing, malformed, unreadable" all yield an empty table; only
/// the last two are worth a warning, because a missing file is what every first
/// run looks like.
pub fn load(path: Option<&Path>, warnings: &mut Vec<String>) -> Table {
    let Some(path) = path else {
        warnings.push("no data directory on this platform; scores are not recorded".to_string());
        return Table::default();
    };
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            if error.kind() != io::ErrorKind::NotFound {
                warnings.push(format!(
                    "{}: {error}; starting an empty table",
                    path.display()
                ));
            }
            return Table::default();
        }
    };
    let mut table: Table = match serde_json::from_str(&text) {
        Ok(table) => table,
        Err(error) => {
            warnings.push(format!(
                "{}: {error}; starting an empty table",
                path.display(),
            ));
            return Table::default();
        }
    };
    if table.version != VERSION {
        warnings.push(format!(
            "{}: version {} is not {VERSION}; starting an empty table",
            path.display(),
            table.version,
        ));
        return Table::default();
    }
    table.tidy();
    table
}

/// Today, as §14 stamps it. The one clock in this module.
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
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
    fn a_round_trip_through_the_file_keeps_every_entry() {
        let dir = std::env::temp_dir().join("ftm-highscore-round-trip");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(FILE_NAME);
        let mut table = filled(&[500, 300]);
        table.insert(entry("NEW", 400, "2026-09-05"));
        table.save(&path).expect("saved");

        let mut warnings = Vec::new();
        assert_eq!(load(Some(&path), &mut warnings), table);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(
            !dir.join("highscores.json.tmp").exists(),
            "the temp file is renamed, not left behind",
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_file_is_an_empty_table_and_no_warning() {
        // §14: the game must never fail to start because of the table, and a
        // first run is not a problem worth mentioning.
        let mut warnings = Vec::new();
        let path = std::env::temp_dir().join("ftm-no-such-highscores.json");
        let _ = std::fs::remove_file(&path);
        assert_eq!(load(Some(&path), &mut warnings), Table::default());
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_malformed_file_is_an_empty_table_and_one_warning() {
        let dir = std::env::temp_dir().join("ftm-highscore-malformed");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("made the directory");
        let path = dir.join(FILE_NAME);
        std::fs::write(&path, "{ not json at all").expect("wrote it");

        let mut warnings = Vec::new();
        assert_eq!(load(Some(&path), &mut warnings), Table::default());
        assert_eq!(warnings.len(), 1, "{warnings:?}");

        // A future version is refused the same way, rather than being read as
        // though its fields meant what they mean here.
        std::fs::write(&path, r#"{"version":2,"entries":[]}"#).expect("wrote it");
        let mut warnings = Vec::new();
        assert_eq!(load(Some(&path), &mut warnings), Table::default());
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hand_edited_file_is_sorted_rather_than_rejected() {
        let dir = std::env::temp_dir().join("ftm-highscore-unsorted");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("made the directory");
        let path = dir.join(FILE_NAME);
        let mut table = Table::default();
        table.entries.push(entry("LOW", 10, "2026-01-01"));
        table.entries.push(entry("HIGH", 900, "2026-01-01"));
        table.entries.push(entry("MID", 500, "2026-01-01"));
        std::fs::write(&path, serde_json::to_string(&table).unwrap()).expect("wrote it");

        let mut warnings = Vec::new();
        let loaded = load(Some(&path), &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            loaded
                .entries
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            ["HIGH", "MID", "LOW"],
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_date_stamp_is_the_format_the_spec_writes() {
        // §14's example entry: "2026-09-04".
        let today = today();
        assert_eq!(today.len(), 10, "{today}");
        assert!(
            today
                .chars()
                .enumerate()
                .all(|(at, c)| if at == 4 || at == 7 {
                    c == '-'
                } else {
                    c.is_ascii_digit()
                }),
            "{today}",
        );
    }
}
