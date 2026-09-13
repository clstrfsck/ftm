//! The menu and overlay *models* (§12.6, §13.3, §13.5).
//!
//! What each menu holds, what its items are and what a key does to it — but
//! not a line of drawing. Every front-end walks the same menus, so the state
//! they carry is shared and the boxes they are drawn in are not.
//!
//! Labels and values live here too: the words §12.6 and §13.5 print are the
//! specification's, and a second front-end that invented its own would be a
//! second specification.

use crate::shell::config::{ColorDepth, ConfigFile, LockDownRule, range};
use crate::shell::highscore::NAME_MAX;

/// What is drawn on top of the playfield, if anything (§12.6).
///
/// `Clone` rather than `Copy`: name entry carries the string being typed, and
/// the §15.2 loop only ever compares one of these with the previous frame's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Overlay {
    None,
    /// §9.17: the pause menu, over a blanked playfield.
    Paused {
        selected: usize,
    },
    /// The §13.5 Options panel, reached from the pause menu (§12.6). The
    /// playfield stays blanked underneath it, as it is for the menu itself.
    Options {
        selected: usize,
    },
    /// §9.17: the 3-2-1 resume countdown, over a playfield that is visible
    /// again — reading the board is what the countdown is for.
    Resuming {
        count: u8,
    },
    GameOver,
    /// §12.6: only when the score qualifies for the top ten. `rank` is
    /// one-based, as the box prints it.
    NameEntry {
        rank: usize,
        name: String,
    },
    /// §12.6's Controls item: the §10.1 binding table, over the blanked
    /// playfield, exactly as the Options panel is.
    Controls,
}

impl Overlay {
    /// Whether the playfield under this overlay is drawn empty (§9.17).
    ///
    /// The anti-pause-scumming rule: while the game is paused — the menu, and
    /// the two boxes it opens — the player gets no free look at the stack. It
    /// is a rule of the game rather than of a screen, so it is answered here
    /// once; the countdown shows the board again, because reading it is what
    /// the countdown is for, and the game-over and name-entry boxes sit over
    /// a game that is finished.
    pub const fn blanks(&self) -> bool {
        matches!(
            self,
            Overlay::Paused { .. } | Overlay::Options { .. } | Overlay::Controls
        )
    }
}

/// The pause menu of §9.17, in the order it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseChoice {
    Resume,
    Restart,
    /// The §13.5 panel; §6.1 calls it "the in-game Options screen".
    Options,
    Controls,
    QuitToMenu,
}

impl PauseChoice {
    pub const ALL: [PauseChoice; 5] = [
        PauseChoice::Resume,
        PauseChoice::Restart,
        PauseChoice::Options,
        PauseChoice::Controls,
        PauseChoice::QuitToMenu,
    ];

    /// Where this item sits in the menu.
    ///
    /// A sub-screen opened from the menu puts the cursor back on the item that
    /// opened it, which is what makes looking at the controls and then at the
    /// options two key presses rather than four.
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|choice| *choice == self)
            .unwrap_or(0)
    }

    pub const fn label(self) -> &'static str {
        match self {
            PauseChoice::Resume => "Resume",
            PauseChoice::Restart => "Restart",
            PauseChoice::Options => "Options",
            PauseChoice::Controls => "Controls",
            PauseChoice::QuitToMenu => "Quit to menu",
        }
    }
}

/// The name-entry buffer (§12.6).
///
/// The twelve-character rule lives here, beside the field that draws it: the
/// box is exactly wide enough for the longest name plus its cursor, so a cap
/// enforced anywhere else would be a cap that could drift from the layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameEntry {
    name: String,
}

impl NameEntry {
    /// Pre-filled from `$USER` (or `$USERNAME` on Windows), truncated (§12.6).
    ///
    /// The environment is the one platform facility the shell still reads, and
    /// it is left here deliberately: it is a *courtesy*, not a capability, and
    /// where there is no environment to read — a browser tab — the answer is
    /// an empty field, which is exactly what the player should see anyway.
    /// Nothing downstream distinguishes the two cases.
    pub fn prefilled() -> Self {
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_default();
        Self {
            name: crate::shell::highscore::tidy_name(&user),
        }
        .cleared_if_anonymous()
    }

    /// `tidy_name` turns an absent `$USER` into `ANON`, which is the right
    /// answer for a name that was *entered* and the wrong one for a field that
    /// was never filled: the player should be typing into an empty box, not
    /// deleting four characters first.
    fn cleared_if_anonymous(mut self) -> Self {
        if self.name == crate::shell::highscore::ANONYMOUS {
            self.name.clear();
        }
        self
    }

    /// Accept one printable ASCII character, up to the twelfth (§12.6).
    pub fn push(&mut self, c: char) -> bool {
        if self.name.chars().count() >= NAME_MAX || !(c.is_ascii_graphic() || c == ' ') {
            return false;
        }
        self.name.push(c);
        true
    }

    /// `Backspace` deletes (§12.6).
    pub fn backspace(&mut self) -> bool {
        self.name.pop().is_some()
    }

    pub fn as_str(&self) -> &str {
        &self.name
    }
}

/// One row of the §13.5 Options panel.
///
/// The list is "the settings most worth changing without a text editor", not
/// all of §6.3: the rest stay in the file, where they can be commented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Preview,
    StartLevel,
    Ghost,
    Hold,
    Rotate180,
    LockDown,
    Colour,
    Grid,
    /// `GUI.md` §G8.10's interface scale. A window's, and a canvas's.
    Scale,
    /// `GUI.md` §G8.10's `fullscreen`. A window's alone: a canvas fills the
    /// page it is on, and the browser's own full-screen mode needs a gesture
    /// the game has no pointer path for (§1.2).
    Fullscreen,
}

impl Setting {
    /// §13.5, in the order it lists them: every setting a *terminal* can apply.
    pub const ALL: [Setting; 8] = [
        Setting::Preview,
        Setting::StartLevel,
        Setting::Ghost,
        Setting::Hold,
        Setting::Rotate180,
        Setting::LockDown,
        Setting::Colour,
        Setting::Grid,
    ];

    /// The settings that mean something in every front-end.
    ///
    /// [`Setting::Colour`] is §12.3's colour depth and is the terminal's alone —
    /// a window has no depths, no `mono` and no `$NO_COLOR` (`GUI.md`, "What
    /// does not carry over") — and the last three are the window's. A panel must
    /// offer what it can actually apply, so which list a front-end navigates is
    /// its own answer, carried on
    /// [`Session::settings`](crate::shell::session::Session::settings).
    ///
    /// Nothing sets this list on its own: it is the intersection the other three
    /// are built from, and it is what a fourth front-end starts from.
    pub const SHARED: [Setting; 7] = [
        Setting::Preview,
        Setting::StartLevel,
        Setting::Ghost,
        Setting::Hold,
        Setting::Rotate180,
        Setting::LockDown,
        Setting::Grid,
    ];

    /// The native window's rows: the shared seven, then `[gui]`'s two
    /// (`GUI.md` §G5.4, §G8.10, §G8.11).
    pub const WINDOW: [Setting; 9] = [
        Setting::Preview,
        Setting::StartLevel,
        Setting::Ghost,
        Setting::Hold,
        Setting::Rotate180,
        Setting::LockDown,
        Setting::Grid,
        Setting::Scale,
        Setting::Fullscreen,
    ];

    /// A browser tab's rows: the shared seven and the scale.
    ///
    /// The one row the two window builds differ by, for the same reason
    /// [`MenuChoice::CANVAS`] is short of **QUIT** — a tab cannot do it.
    pub const CANVAS: [Setting; 8] = [
        Setting::Preview,
        Setting::StartLevel,
        Setting::Ghost,
        Setting::Hold,
        Setting::Rotate180,
        Setting::LockDown,
        Setting::Grid,
        Setting::Scale,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Setting::Preview => "Preview",
            Setting::StartLevel => "Start level",
            Setting::Ghost => "Ghost piece",
            Setting::Hold => "Hold",
            Setting::Rotate180 => "180 rotation",
            Setting::LockDown => "Lock down",
            Setting::Colour => "Colour",
            Setting::Grid => "Grid",
            Setting::Scale => "Scale",
            Setting::Fullscreen => "Full screen",
        }
    }

    /// The setting's value as the panel shows it.
    pub fn value(self, file: &ConfigFile) -> String {
        let switch = |on: bool| if on { "on" } else { "off" }.to_string();
        match self {
            Setting::Preview => file.gameplay.preview_count.to_string(),
            Setting::StartLevel => file.gameplay.start_level.to_string(),
            Setting::Ghost => switch(file.gameplay.ghost_piece),
            Setting::Hold => switch(file.gameplay.hold_enabled),
            Setting::Rotate180 => switch(file.gameplay.allow_180_rotation),
            Setting::LockDown => match file.gameplay.lock_down {
                LockDownRule::Extended => "extended",
                LockDownRule::Infinite => "infinite",
                LockDownRule::Classic => "classic",
            }
            .to_string(),
            Setting::Colour => match file.display.color_depth {
                ColorDepth::Auto => "auto",
                ColorDepth::Truecolor => "truecolor",
                ColorDepth::Ansi256 => "256",
                ColorDepth::Ansi16 => "16",
                ColorDepth::Mono => "mono",
            }
            .to_string(),
            Setting::Grid => switch(file.display.show_grid),
            Setting::Scale => format!("{}%", file.gui.scale_percent),
            Setting::Fullscreen => switch(file.gui.fullscreen),
        }
    }

    /// Move one step along the setting's values (§13.5), wrapping at each end.
    ///
    /// Wrapping rather than stopping, because every one of these lists is short
    /// enough to walk round and a player holding `→` on a two-value switch
    /// expects it to toggle.
    pub fn step(self, file: &mut ConfigFile, forward: bool) {
        let g = &mut file.gameplay;
        match self {
            Setting::Preview => {
                g.preview_count = wrap(g.preview_count, forward, &range::PREVIEW_COUNT);
            }
            Setting::StartLevel => {
                g.start_level = wrap(g.start_level, forward, &range::START_LEVEL);
            }
            Setting::Ghost => g.ghost_piece = !g.ghost_piece,
            Setting::Hold => g.hold_enabled = !g.hold_enabled,
            Setting::Rotate180 => g.allow_180_rotation = !g.allow_180_rotation,
            Setting::LockDown => {
                const RULES: [LockDownRule; 3] = [
                    LockDownRule::Extended,
                    LockDownRule::Infinite,
                    LockDownRule::Classic,
                ];
                g.lock_down = cycle(&RULES, g.lock_down, forward);
            }
            Setting::Colour => {
                const DEPTHS: [ColorDepth; 5] = [
                    ColorDepth::Auto,
                    ColorDepth::Truecolor,
                    ColorDepth::Ansi256,
                    ColorDepth::Ansi16,
                    ColorDepth::Mono,
                ];
                file.display.color_depth = cycle(&DEPTHS, file.display.color_depth, forward);
            }
            Setting::Grid => file.display.show_grid = !file.display.show_grid,
            Setting::Scale => {
                // Steps rather than `range::SCALE_PERCENT` one per cent: the
                // panel is walked a keystroke at a time, and 250 presses to
                // cross the range is not a setting anyone would change here.
                // Every step is in the range the loader clamps to, so a value
                // the file asked for that is not on the list joins it at the
                // first press (`cycle` takes the head).
                const STEPS: [u32; 9] = [50, 75, 100, 125, 150, 175, 200, 250, 300];
                file.gui.scale_percent = cycle(&STEPS, file.gui.scale_percent, forward);
            }
            Setting::Fullscreen => file.gui.fullscreen = !file.gui.fullscreen,
        }
    }
}

/// The next value in an inclusive numeric range, wrapping.
fn wrap<T>(value: T, forward: bool, range: &std::ops::RangeInclusive<T>) -> T
where
    T: Copy + PartialOrd + std::ops::Add<Output = T> + std::ops::Sub<Output = T> + From<u8>,
{
    let one = T::from(1u8);
    if forward {
        if value >= *range.end() {
            *range.start()
        } else {
            value + one
        }
    } else if value <= *range.start() {
        *range.end()
    } else {
        value - one
    }
}

/// The next entry of a short list, wrapping. An unrecognised current value
/// takes the first, which is the only sane answer and cannot arise.
fn cycle<T: Copy + PartialEq>(values: &[T], current: T, forward: bool) -> T {
    let at = values.iter().position(|v| *v == current).unwrap_or(0);
    let count = values.len();
    let next = if forward {
        (at + 1) % count
    } else {
        (at + count - 1) % count
    };
    values[next]
}

/// §10.1's actions and the keys bound to each, as the controls box lists them.
///
/// Both front-ends show this box — from the pause menu (§12.6) and from the
/// attract screen (§13.5) — so what it says is the specification's and not a
/// screen's: the words, their order, and the rule that a binding whose setting
/// is off is not listed at all (§13.3, §17.3 A9). How the two columns are set
/// out is each front-end's.
pub fn controls(file: &ConfigFile) -> Vec<(&'static str, String)> {
    /// The eleven actions of §10.1, by the `[keys]` name that carries them.
    const ACTIONS: [(&str, &str); 11] = [
        ("move_left", "Move left"),
        ("move_right", "Move right"),
        ("soft_drop", "Soft drop"),
        ("hard_drop", "Hard drop"),
        ("rotate_cw", "Rotate clockwise"),
        ("rotate_ccw", "Rotate counter-clockwise"),
        ("rotate_180", "Rotate 180\u{b0}"),
        ("hold", "Hold"),
        ("pause", "Pause"),
        ("restart", "Restart (hold 1 s)"),
        ("quit", "Quit to menu"),
    ];
    let bound = file.keys.each();
    ACTIONS
        .iter()
        .filter(|(name, _)| match *name {
            "rotate_180" => file.gameplay.allow_180_rotation,
            "hold" => file.gameplay.hold_enabled,
            _ => true,
        })
        .map(|(name, label)| {
            let keys = bound
                .iter()
                .find(|(bound, _)| bound == name)
                .map(|(_, names)| names.join(", "))
                .unwrap_or_default();
            (*label, keys)
        })
        .collect()
}

/// §13.3's menu, in the order it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuChoice {
    Play,
    /// `PILOT.md` §P1: the automated player, which starts a game the player
    /// watches rather than plays.
    Pilot,
    HighScores,
    Controls,
    Options,
    Quit,
}

impl MenuChoice {
    /// §13.3's six, in the order it draws them (`PILOT.md` §P7.1).
    pub const ALL: [MenuChoice; 6] = [
        MenuChoice::Play,
        MenuChoice::Pilot,
        MenuChoice::HighScores,
        MenuChoice::Controls,
        MenuChoice::Options,
        MenuChoice::Quit,
    ];

    /// A browser tab's five (`PILOT.md` §P7.1).
    ///
    /// It is the browser's list rather than "the same list without quit",
    /// exactly as [`Setting::CANVAS`] is the browser's panel, and it is short of
    /// exactly one item. A tab is closed, not quit: `window.close()` is refused
    /// to a page the player opened (`GUI.md` §G8.1), so **QUIT** there would be
    /// an item that does nothing, which is worse than an item that is not
    /// offered.
    ///
    /// **PILOT was the second missing item and is not any more** (`PILOT.md`
    /// §P1). The reason given was that a tab would run §P6's search on its frame
    /// thread; the reason it no longer holds is that the search was measured
    /// rather than assumed, in a tab and on a desktop, and §P6.4's budget fits a
    /// frame in both. It is the same budget in both — a web-specific `nodes`
    /// would be a shallower planner wearing the deep planner's weights (§P6.4).
    ///
    /// Which list a front-end shows is its own answer, carried on
    /// [`Session::menu`](crate::shell::session::Session::menu) — and the screen
    /// and the shell walk that same list, exactly as they do
    /// [`Setting::WINDOW`] and [`Setting::CANVAS`], so the cursor can never land
    /// on an item nobody can see.
    pub const CANVAS: [MenuChoice; 5] = [
        MenuChoice::Play,
        MenuChoice::Pilot,
        MenuChoice::HighScores,
        MenuChoice::Controls,
        MenuChoice::Options,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            MenuChoice::Play => "PLAY",
            MenuChoice::Pilot => "PILOT",
            MenuChoice::HighScores => "HIGH SCORES",
            MenuChoice::Controls => "CONTROLS",
            MenuChoice::Options => "OPTIONS",
            MenuChoice::Quit => "QUIT",
        }
    }
}

/// A sub-screen over the attract screen (§13.5). `Esc` returns from each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sub {
    HighScores,
    Controls,
    Options { selected: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_paused_game_hides_the_stack_and_the_countdown_shows_it_again() {
        // §9.17: blanked while paused, in every front-end. The two boxes the
        // pause menu opens are still the pause; the countdown is not, because
        // it exists so the player can read the board before play resumes.
        assert!(Overlay::Paused { selected: 0 }.blanks());
        assert!(Overlay::Options { selected: 3 }.blanks());
        assert!(Overlay::Controls.blanks());
        assert!(!Overlay::Resuming { count: 3 }.blanks());
        assert!(!Overlay::None.blanks());
        assert!(!Overlay::GameOver.blanks());
        let entry = Overlay::NameEntry {
            rank: 1,
            name: String::new(),
        };
        assert!(!entry.blanks());
    }

    #[test]
    fn the_field_takes_twelve_printable_ascii_characters() {
        // §12.6: "up to 12 printable ASCII characters, Backspace deletes".
        let mut entry = NameEntry {
            name: String::new(),
        };
        for c in "msandiford".chars() {
            assert!(entry.push(c));
        }
        assert!(!entry.push('\u{e9}'), "not ASCII");
        assert!(!entry.push('\u{7}'), "not printable");
        assert!(entry.push(' '), "but a space is both");
        assert!(entry.push('X'));
        assert_eq!(entry.as_str().chars().count(), NAME_MAX);
        assert!(!entry.push('Y'), "and the thirteenth is refused");

        assert!(entry.backspace());
        assert_eq!(entry.as_str(), "msandiford ");
        let mut empty = NameEntry {
            name: String::new(),
        };
        assert!(!empty.backspace(), "an empty field has nothing to delete");
    }

    #[test]
    fn an_absent_user_leaves_the_field_empty_rather_than_anon() {
        // §12.6 pre-fills from `$USER`; `ANON` is what an *entered* empty name
        // becomes (§14), not what an unfilled field should start as.
        assert_eq!(
            NameEntry {
                name: crate::shell::highscore::ANONYMOUS.to_string(),
            }
            .cleared_if_anonymous()
            .as_str(),
            "",
        );
    }

    #[test]
    fn every_setting_walks_round_its_own_values() {
        // §13.5: `←`/`→` change the selected value, wrapping at each end.
        let mut file = ConfigFile::default();
        assert_eq!(file.gameplay.preview_count, 5);
        Setting::Preview.step(&mut file, true);
        assert_eq!(file.gameplay.preview_count, 6);
        Setting::Preview.step(&mut file, true);
        assert_eq!(
            file.gameplay.preview_count,
            *range::PREVIEW_COUNT.start(),
            "off the top and round to the bottom",
        );
        Setting::Preview.step(&mut file, false);
        assert_eq!(file.gameplay.preview_count, *range::PREVIEW_COUNT.end());

        Setting::StartLevel.step(&mut file, false);
        assert_eq!(file.gameplay.start_level, *range::START_LEVEL.end());

        for (setting, before, after) in [
            (Setting::Ghost, true, false),
            (Setting::Hold, true, false),
            (Setting::Rotate180, true, false),
            (Setting::Grid, false, true),
        ] {
            let read = |file: &ConfigFile| setting.value(file) == "on";
            assert_eq!(read(&file), before, "{setting:?}");
            setting.step(&mut file, true);
            assert_eq!(read(&file), after, "{setting:?}");
            setting.step(&mut file, false);
            assert_eq!(read(&file), before, "{setting:?} and back");
        }

        Setting::LockDown.step(&mut file, false);
        assert_eq!(file.gameplay.lock_down, LockDownRule::Classic, "wraps back");
        Setting::LockDown.step(&mut file, true);
        assert_eq!(file.gameplay.lock_down, LockDownRule::Extended);

        Setting::Colour.step(&mut file, false);
        assert_eq!(file.display.color_depth, ColorDepth::Mono);
        Setting::Colour.step(&mut file, true);
        assert_eq!(file.display.color_depth, ColorDepth::Auto);
    }

    #[test]
    fn a_stepped_value_never_leaves_its_range() {
        // §6.3's ranges are enforced everywhere, and the panel is the one place
        // a value is changed without going through the loader.
        let mut file = ConfigFile::default();
        for forward in [true, false] {
            for _ in 0..40 {
                Setting::Preview.step(&mut file, forward);
                Setting::StartLevel.step(&mut file, forward);
                assert!(range::PREVIEW_COUNT.contains(&file.gameplay.preview_count));
                assert!(range::START_LEVEL.contains(&file.gameplay.start_level));
            }
        }
    }
}
