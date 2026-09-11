//! The four capabilities of `FRONTEND.md` F1-F4, on a machine with a
//! filesystem, a clock, a calendar and an entropy source.
//!
//! §3.1 gives the shell none of the four, so this is where they come from —
//! and it is deliberately the *whole* of the platform the shell sees:
//! `std::fs`, `directories`, `chrono` and `rand::random` are named here and
//! nowhere above a front-end.
//!
//! **This is not a layer.** It is a desktop, and it sits beside the front-ends
//! rather than under the shell: `ftm` and `ftm-gui` both run on one, and §6.2
//! and §14 say they share one config file and one high-score table, so a
//! setting written by either is read by the other. `EGUI.md` G5 moved it here
//! from `tui/host.rs` for that reason — two copies of §14's atomic write is
//! two places for it to drift, and `CLAUDE.md` says that write has one home.
//! Each front-end still decides *whether* to use it: the browser tab of G6
//! answers the same four with `web_sys`, and takes nothing from here.
//!
//! It is about a hundred and fifty lines, most of them §6.2's and §14's paths
//! and §14's atomic write. That is the number `EGUI.md` G6 is betting on when
//! it calls the web build a fourth capability provider rather than a port.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use directories::ProjectDirs;

use crate::shell::storage::{Slot, Storage, StorageError};
use crate::shell::time::Stamp;

/// The file name under the platform config directory (§6.2).
pub const CONFIG_FILE: &str = "config.toml";
/// The file name under the platform data directory (§14).
pub const SCORES_FILE: &str = "highscores.json";

// ---------------------------------------------------------------------------
// F1: time
// ---------------------------------------------------------------------------

/// The monotonic clock, as an origin plus an `Instant` (F1).
///
/// `Instant` is monotonic and wall-clock-paced by construction on every
/// platform that has one, which is exactly what F1 asks for; the origin is
/// captured once so a `Stamp` is small, `Copy` and constructible in a test.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    origin: Instant,
}

impl Clock {
    /// Start the clock. The first stamp it hands out is near [`Stamp::ZERO`].
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }

    /// Now, as the shell measures it.
    pub fn now(&self) -> Stamp {
        Stamp::from_elapsed(self.origin.elapsed())
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// F3, F4: entropy and the calendar
// ---------------------------------------------------------------------------

/// One seed, and the whole entropy budget of the program (F3, §9.6).
pub fn seed() -> u64 {
    rand::random()
}

/// Today, as §14 stamps it: `YYYY-MM-DD`, local time (F4).
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

// ---------------------------------------------------------------------------
// F2: storage
// ---------------------------------------------------------------------------

/// §6.2's and §14's two files (F2).
///
/// Both natively-built front-ends use this, and that is the point: they share
/// one config file and one high-score table, so a setting written by either is
/// read by the other (§6.2).
///
/// A slot with no path is [`StorageError::Unavailable`] rather than a silent
/// success — `ProjectDirs` answers `None` only where the platform admits to no
/// such directory at all, which §6.2 and §14 both call a documented
/// degradation: the game plays, and says so once in the warnings (§16).
#[derive(Clone, Debug)]
pub struct Files {
    config: Option<PathBuf>,
    scores: Option<PathBuf>,
}

impl Files {
    /// §6.2's and §14's paths, with §6.4's `--config` overriding the first.
    pub fn new(config_override: Option<PathBuf>) -> Self {
        let dirs = ProjectDirs::from("", "", "ftm");
        Self {
            config: config_override
                .or_else(|| dirs.as_ref().map(|d| d.config_dir().join(CONFIG_FILE))),
            scores: dirs.as_ref().map(|d| d.data_dir().join(SCORES_FILE)),
        }
    }

    /// The two files, given explicitly. For a test that wants a throwaway
    /// directory rather than the player's own.
    #[cfg(test)]
    pub fn at(config: Option<PathBuf>, scores: Option<PathBuf>) -> Self {
        Self { config, scores }
    }

    fn path(&self, slot: Slot) -> Result<&Path, StorageError> {
        match slot {
            Slot::Config => self.config.as_deref(),
            Slot::HighScores => self.scores.as_deref(),
        }
        .ok_or(StorageError::Unavailable)
    }
}

impl Storage for Files {
    fn read(&self, slot: Slot) -> Result<Option<String>, StorageError> {
        let path = self.path(slot)?;
        match fs::read_to_string(path) {
            Ok(text) => Ok(Some(text)),
            // An absent file is the ordinary first run, not a problem.
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(failed(path, &error)),
        }
    }

    /// Write the slot, creating the directory if need be.
    ///
    /// The two slots are written differently, and deliberately. §14 requires
    /// the table to be **durable against a crash mid-write**, so it goes to a
    /// sibling temp file and is renamed over the target — atomic only because
    /// the temp file is in the same directory, since a rename across
    /// filesystems is a copy. §6.2 asks for no such thing of the config, and
    /// giving it the same treatment would quietly *change* what the terminal
    /// front-end does: a rename replaces a read-only file, where a plain write
    /// is refused by it, and a player who made their config read-only meant it.
    fn write(&mut self, slot: Slot, contents: &str) -> Result<(), StorageError> {
        let path = self.path(slot)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| failed(parent, &error))?;
        }
        match slot {
            Slot::Config => fs::write(path, contents).map_err(|error| failed(path, &error)),
            Slot::HighScores => {
                // Both steps report the *target*: the temp file is this store's
                // technique, and §16's line is read by a player who has never
                // heard of it and wants to know which file is unwritable.
                let temporary = path.with_extension("json.tmp");
                fs::write(&temporary, contents).map_err(|error| failed(path, &error))?;
                fs::rename(&temporary, path).map_err(|error| failed(path, &error))
            }
        }
    }
}

/// §16's line, in the shape the terminal front-end has always printed it: the
/// path, then what went wrong with it.
fn failed(path: &Path, error: &io::Error) -> StorageError {
    StorageError::Failed(format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&dir);
        dir
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

    #[test]
    fn the_clock_is_monotonic_and_starts_near_zero() {
        // `FRONTEND.md` F1's two obligations, as far as a test can see them.
        let clock = Clock::new();
        let first = clock.now();
        assert!(first.as_micros() < 1_000_000, "{first:?}");
        let second = clock.now();
        assert!(second >= first, "{first:?} then {second:?}");
    }

    #[test]
    fn a_slot_round_trips_and_leaves_no_temp_file_behind() {
        // §14's atomic write, and §6.2's directory creation. The temp file is
        // renamed, not left lying beside the target.
        let dir = scratch("ftm-host-round-trip");
        let mut files = Files::at(Some(dir.join(CONFIG_FILE)), Some(dir.join(SCORES_FILE)));
        assert_eq!(files.read(Slot::Config), Ok(None), "the first run");

        files.write(Slot::Config, "hello").expect("writes");
        files.write(Slot::HighScores, "{}").expect("writes");
        assert_eq!(files.read(Slot::Config), Ok(Some("hello".to_string())));
        assert_eq!(files.read(Slot::HighScores), Ok(Some("{}".to_string())));
        assert!(
            !dir.join("highscores.json.tmp").exists(),
            "§14's temp file is renamed, not left behind",
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_score_one_binary_files_is_read_by_the_other() {
        // §6.2 and §14 give `ftm` and `ftm-gui` **one** config file and one
        // table between them, and this module is where that is decided — both
        // native binaries answer `FRONTEND.md` F2 from here, so a score the
        // window files at name entry is a score the terminal reads at start-up
        // (`EGUI.md` G8). The two runs below are the two binaries: the only
        // thing either of them adds is which front-end drew the box.
        use crate::shell::config::{ConfigFile, Startup};
        use crate::shell::host::Host;
        use crate::shell::input::InputMode;
        use crate::shell::session::Session;

        let dir = scratch("ftm-host-shared-table");
        let paths = || Files::at(Some(dir.join(CONFIG_FILE)), Some(dir.join(SCORES_FILE)));
        let startup = |file: ConfigFile| Startup {
            on_disk: file.clone(),
            file,
            existed: false,
            wrote_config: false,
            seed: 1,
            // Unseeded: §6.4's seeded runs are never recorded.
            seeded: false,
            warnings: Vec::new(),
        };
        let view = {
            let rules = ConfigFile::default().resolve().0;
            let mut game = crate::core::Game::new(rules, 42);
            let mut events = Vec::new();
            game.tick(&crate::core::TickInput::default(), &mut events);
            let mut view = game.view();
            view.score = 12_480;
            view
        };

        let mut store = paths();
        let mut window = Session::new(
            &startup(ConfigFile::default()),
            InputMode::Enhanced,
            Host::new(&mut store, || 1, || "2026-09-12".to_string()),
        );
        window.record("MS", &view);
        assert_eq!(window.recent, Some(0), "it made the table");
        drop(window);

        let mut store = paths();
        let terminal = Session::new(
            &startup(ConfigFile::default()),
            InputMode::Legacy,
            Host::new(&mut store, || 2, || "2026-09-13".to_string()),
        );
        assert_eq!(terminal.scores.entries.len(), 1);
        assert_eq!(terminal.scores.entries[0].name, "MS");
        assert_eq!(terminal.scores.entries[0].score, 12_480);
        assert!(terminal.warnings().is_empty(), "{:?}", terminal.warnings());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_slot_with_no_path_is_unavailable_rather_than_a_silent_success() {
        // §6.2, §14: a platform with no config or data directory. The shell
        // warns once, on the read (§16).
        let mut files = Files::at(None, None);
        assert_eq!(files.read(Slot::Config), Err(StorageError::Unavailable));
        assert_eq!(
            files.write(Slot::HighScores, "{}"),
            Err(StorageError::Unavailable),
        );
    }

    #[test]
    fn a_read_only_config_is_refused_rather_than_replaced() {
        // §6.2 asks for durability of the *table* (§14) and not of the config,
        // and the difference is visible: a rename would go straight over a
        // read-only file. §17.3's sign-off checked this case on a real one.
        let dir = scratch("ftm-host-read-only-config");
        fs::create_dir_all(&dir).expect("made the directory");
        let path = dir.join(CONFIG_FILE);
        fs::write(&path, "kept").expect("wrote it");
        let mut permissions = fs::metadata(&path).expect("stat").permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).expect("made it read-only");

        let mut files = Files::at(Some(path.clone()), None);
        let error = files.write(Slot::Config, "replaced").expect_err("refused");
        assert!(error.warning().is_some(), "{error:?}");
        assert_eq!(fs::read_to_string(&path).expect("still there"), "kept");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_write_names_the_target_and_never_the_temp_file() {
        // §16's line is read by a player who then goes and fixes it, so it has
        // to name a file they recognise. The temp file of §14's atomic write
        // is this module's business and nobody else's.
        let dir = scratch("ftm-host-names-the-target");
        fs::create_dir_all(&dir).expect("made the directory");
        let mut permissions = fs::metadata(&dir).expect("stat").permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&dir, permissions).expect("made it read-only");

        let mut files = Files::at(Some(dir.join(CONFIG_FILE)), None);
        let error = files.write(Slot::Config, "x").expect_err("refused");
        let line = error.warning().expect("a line for §16");
        assert!(line.contains(CONFIG_FILE), "{line}");
        assert!(!line.contains(".tmp"), "{line}");

        let mut permissions = fs::metadata(&dir).expect("stat").permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        let _ = fs::set_permissions(&dir, permissions);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unwritable_location_names_itself_in_the_warning() {
        // §16: the player reads this at exit and then goes and fixes it, so
        // the path is what it has to say. `/dev/null` is a file rather than a
        // directory on every platform this builds for, so `create_dir_all`
        // cannot make a home under it.
        let mut files = Files::at(Some(PathBuf::from("/dev/null/ftm/config.toml")), None);
        let error = files.write(Slot::Config, "x").expect_err("refused");
        assert!(
            error
                .warning()
                .is_some_and(|line| line.contains("/dev/null")),
            "{error:?}",
        );
    }

    #[test]
    fn the_config_path_can_be_overridden_and_the_score_path_cannot() {
        // §6.4's `--config`; §14 has no equivalent, which is why a `drive.py`
        // run needs a throwaway `HOME` to keep off the real table.
        let files = Files::new(Some(PathBuf::from("/tmp/elsewhere.toml")));
        assert_eq!(
            files.config.as_deref(),
            Some(Path::new("/tmp/elsewhere.toml"))
        );
        assert_eq!(files.scores, Files::new(None).scores);
    }
}
