//! The state above a game (§7), and the screen it hands control to next.
//!
//! A run is not one game: the §13.5 Options panel is reachable from the
//! attract screen as well as from the pause menu, and the §14 table is what
//! the two screens have to agree about. [`Session`] is what outlives either of
//! them — the config, §16's warnings, the high-score table, the seed policy
//! and the three capabilities the front-end lends the shell (`FRONTEND.md`
//! F2-F4).
//!
//! Nothing here belongs to a front-end. What used to sit beside these fields
//! and did — §12.2's interned glyphs — moved out to `tui/run.rs` at `EGUI.md`
//! stage G4, because leaking three `&'static str` so a ratatui `Theme` can be
//! `Copy` is a terminal's problem and a browser tab that is reloaded
//! repeatedly is the wrong place to inherit it.

use crate::core::GameView;
use crate::shell::config::{self, ConfigFile, Startup};
use crate::shell::highscore::{self, Entry};
use crate::shell::host::Host;
use crate::shell::input::InputMode;
use crate::shell::menus::{MenuChoice, Setting};

/// Where the run goes next (§7).
///
/// The state machine is a loop over this: `Attract` and `Play` are the two
/// screens, and `Quit` is the only way out. A game never returns `Quit` —
/// §7 and §16 both send the quit key from `Playing` to the attract screen —
/// and it returns `Play` to mean "restart with a fresh game".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Attract,
    Play,
    Quit,
}

/// Everything that outlives one game (§7).
pub struct Session<'a> {
    /// The effective configuration (§6.1), which the Options panel edits in
    /// place and writes back on the way out.
    pub config: ConfigFile,
    /// §14's table, as both screens see it.
    pub scores: highscore::Table,
    /// The entry the run that just finished added, highlighted by §13.5's
    /// high-score screen.
    pub recent: Option<usize>,
    /// Which of §8.2's two input paths is live. A front-end that always has
    /// key releases says `Enhanced` and never thinks about it again.
    pub mode: InputMode,
    /// The rows the §13.5 Options panel offers, which is the front-end's own
    /// answer: a panel must offer what it can actually apply.
    ///
    /// [`Setting::ALL`] is a terminal's and is the default; the window front-end
    /// sets [`Setting::SHARED`], because §12.3's colour depth means nothing off
    /// a terminal (`GUI.md` §G5). Both screens that draw the panel navigate
    /// *this* list, so the cursor can never land on a row the screen is not
    /// showing.
    pub settings: &'static [Setting],
    /// The items §13.3's menu offers, which is the front-end's own answer for
    /// the same reason [`settings`](Self::settings) is: a menu must offer what
    /// its front-end can actually do.
    ///
    /// [`MenuChoice::ALL`] is §13.3's five and is the default; the web build
    /// sets [`MenuChoice::NO_QUIT`], because a tab cannot close itself
    /// (`GUI.md` §G8.1). The screen draws this list and
    /// [`Attract`](crate::shell::attract::Attract) navigates it.
    pub menu: &'static [MenuChoice],
    /// The three capabilities a run borrows from the front-end (§3.1,
    /// `FRONTEND.md` F2-F4): §6.2's and §14's bytes, the seed for the next
    /// game and §14's date stamp. The fourth, time, arrives as a
    /// [`Stamp`](crate::shell::time::Stamp) argument instead.
    pub(crate) host: Host<'a>,
    /// §16's warning vector, carried for the length of the run so that a file
    /// that could not be written still reaches stderr at exit.
    pub(crate) warnings: Vec<String>,
    /// Whether the panel ever wrote the config, so §6.2's first-clean-exit
    /// write does not undo what the player just saved.
    pub(crate) saved: bool,
    /// §6.4: with `--seed` every game of the run is the same one, and none of
    /// them is recorded (§14).
    pub(crate) seed: u64,
    pub(crate) seeded: bool,
    /// Bumped whenever something a screen shows changes that is neither in the
    /// `GameView` nor in the `Overlay` — the Options panel's values are the
    /// only such thing so far. A front-end that compares frames to decide
    /// whether to draw (§15.2 step 5) is otherwise blind to them.
    generation: u32,
}

impl<'a> Session<'a> {
    /// Resolve what start-up found into what the two screens share.
    pub fn new(startup: &Startup, mode: InputMode, host: Host<'a>) -> Self {
        let mut warnings = Vec::new();
        // §14: a table that cannot be read is an empty table and a warning; it
        // is never a reason not to start.
        let scores = highscore::load(host.storage, &mut warnings);
        Self {
            config: startup.file.clone(),
            scores,
            recent: None,
            mode,
            // A terminal's list, which is every setting §13.5 lists; a
            // front-end with fewer says so (`GUI.md` §G5).
            settings: &Setting::ALL,
            // §13.3's five, which is every front-end that can close itself.
            menu: &MenuChoice::ALL,
            host,
            warnings,
            saved: false,
            seed: startup.seed,
            seeded: startup.seeded,
            generation: 0,
        }
    }

    /// The counter a front-end compares to notice a settings change it cannot
    /// see any other way (§15.2 step 5).
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Note that something outside the view and the overlay changed.
    pub(crate) fn bump(&mut self) {
        self.generation += 1;
    }

    /// The seed for the next game (§6.4).
    ///
    /// A seeded run replays the *same* game however many times it is restarted,
    /// which is what makes `--seed` useful; an unseeded one draws a fresh seed
    /// per game.
    pub(crate) fn next_seed(&self) -> u64 {
        if self.seeded {
            self.seed
        } else {
            (self.host.seed)()
        }
    }

    /// §6.1: write the edited settings back. §16: an unwritable file never
    /// aborts — it adds a line to the warnings printed at exit.
    pub(crate) fn save_config(&mut self) {
        match config::save(self.host.storage, &self.config) {
            Ok(()) => self.saved = true,
            // §16: a store with nowhere to keep it said so at load, and has
            // nothing to add here.
            Err(error) => {
                if let Some(line) = error.warning() {
                    self.warn(line.to_string());
                }
            }
        }
    }

    /// File a finished run (§14), reporting nothing: a score that did not make
    /// the table and a table that could not be written look the same from here.
    pub(crate) fn record(&mut self, name: &str, view: &GameView) {
        let entry = Entry::of(name, view, (self.host.today)());
        self.recent = self.scores.insert(entry);
        if self.recent.is_none() {
            return;
        }
        if let Err(error) = self.scores.save(self.host.storage) {
            // §14: "any failure to write yields a warning at exit and is
            // otherwise ignored".
            if let Some(line) = error.warning() {
                self.warn(line.to_string());
            }
        }
    }

    /// §16's warnings so far, oldest first.
    ///
    /// A front-end whose run has an end takes them from
    /// [`finish`](Self::finish) and prints them after teardown. One whose run
    /// has none — a browser tab is closed, not quit — reads them here as they
    /// arise instead, and remembers how many it has already reported: the
    /// list only grows, and [`warn`](Self::warn) never adds a line twice.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Add a warning, once. A panel left twice over the same unwritable file
    /// should say so once, not twice.
    pub(crate) fn warn(&mut self, warning: String) {
        if !self.warnings.contains(&warning) {
            self.warnings.push(warning);
        }
    }

    /// Hand back what the front-end prints after teardown (§16).
    pub fn finish(self, startup: &mut Startup) {
        startup.file = self.config;
        startup.wrote_config = self.saved;
        startup.warnings.extend(self.warnings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::storage::{Memory, Slot};

    #[test]
    fn warnings_can_be_read_as_they_arise_and_only_ever_grow() {
        // §16, for a front-end with no exit to print them at (`GUI.md` §G8):
        // it reports what is new since it last looked, which is only sound if
        // the list keeps its order and never repeats a line.
        let mut storage = Memory::holding(Slot::HighScores, "not a table");
        let file = ConfigFile::default();
        let startup = Startup {
            on_disk: file.clone(),
            file,
            existed: false,
            wrote_config: false,
            seed: 42,
            seeded: false,
            warnings: Vec::new(),
        };
        let mut session = Session::new(
            &startup,
            InputMode::Enhanced,
            Host::new(&mut storage, || 42, || "2026-09-11".to_string()),
        );
        // §14: a table that will not parse is a warning at start-up.
        let first = session.warnings().to_vec();
        assert_eq!(first.len(), 1, "{first:?}");

        session.warn("later".to_string());
        session.warn("later".to_string());
        assert_eq!(session.warnings()[..1], first[..], "oldest first");
        assert_eq!(&session.warnings()[1..], ["later".to_string()]);
    }
}
