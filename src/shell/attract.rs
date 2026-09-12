//! The attract screen's state machine (§13.1, §13.3, §13.5, §13.6).
//!
//! What is selected, which sub-screen is open, how far the panel's six-second
//! cycle has come and how long the keyboard has been quiet — and nothing about
//! how any of it is drawn. §13.4's drifting background is deliberately *not*
//! here: it is positioned in the units of whatever is drawing it, so each
//! front-end keeps its own and folds its "did anything move" answer in beside
//! [`Attract::step`]'s.
//!
//! What §13 *says*, though, is shared, exactly as §12.6's labels are
//! [`shell::menus`](crate::shell::menus)': the letterforms of §13.2's wordmark,
//! the colours they take, the reminders §13.3's panel rotates through and the
//! numbers §13.4 gives the drift. A second front-end that drew a *different*
//! wordmark would be a second piece of branding, which §1.3 is precisely about;
//! one that invented its own reminders would be a second §13.3. Where any of it
//! lands on a screen is the front-end's, and that is all that is.

use std::time::Duration;

use crate::core::PieceKind;
use crate::shell::config::ConfigFile;
use crate::shell::keys::{Key, KeyEvent, KeyKind};
use crate::shell::menus::{MenuChoice, Setting, Sub};
use crate::shell::session::{Next, Player, Session};
use crate::shell::time::Stamp;

// ---------------------------------------------------------------------------
// §13.2, §13.3, §13.4: what the screen says, in every front-end
// ---------------------------------------------------------------------------

/// The wordmark is five rows tall (§13.2).
pub const WORDMARK_ROWS: usize = 5;

/// The three letters of `FTM`, as a bitmap: `#` is a block, `.` is not (§13.2).
///
/// An **original** block-letter wordmark. The official logo must not be used,
/// reproduced or approximated, and no official colours-as-branding, styling or
/// artwork may be copied (§1.3) — which is also why there is one of these and
/// not one per front-end. A terminal draws each block as two characters so that
/// three letters still carry the screen; a window draws each as a mino.
pub const WORDMARK: [[&str; WORDMARK_ROWS]; 3] = [
    ["####", "#...", "###.", "#...", "#..."],
    ["####", ".##.", ".##.", ".##.", ".##."],
    ["#...#", "##.##", "#.#.#", "#...#", "#...#"],
];

/// A blank column between letters, so the strokes stay separate (§13.2).
pub const WORDMARK_GAP: usize = 1;

/// The wordmark's width in blocks: 4 + 1 + 4 + 1 + 5 (§13.2). Thirty characters
/// in a terminal, which doubles each block; fifteen cells in a window.
pub const WORDMARK_BLOCKS: usize = 15;

/// The full name, spelled out under the wordmark (§13.2).
pub const SUBTITLE: &str = "FALLING TETROMINO MANAGER";

/// §9.2's seven colours in §13.2's order, which is what §13.6's idle cycle
/// walks along.
pub const WORDMARK_CYCLE: [PieceKind; 7] = [
    PieceKind::I,
    PieceKind::J,
    PieceKind::L,
    PieceKind::O,
    PieceKind::S,
    PieceKind::T,
    PieceKind::Z,
];

/// Where each letter starts in [`WORDMARK_CYCLE`]: `I`, `S` and `T` — cyan,
/// green and purple (§13.2).
///
/// Indices rather than three `PieceKind`s, because §13.6's idle cycle advances
/// every letter one step along all seven colours. Three fixed colours would
/// have made that cycle repeat after three seconds instead of seven.
pub const WORDMARK_START: [usize; 3] = [0, 4, 5];

/// The colour letter `letter` takes, `shift` steps into §13.6's idle cycle.
pub fn wordmark_colour(letter: usize, shift: usize) -> PieceKind {
    WORDMARK_CYCLE[(WORDMARK_START[letter % WORDMARK_START.len()] + shift) % WORDMARK_CYCLE.len()]
}

/// The one-line rules reminders the third panel face rotates through (§13.3).
pub const REMINDERS: [&str; 5] = [
    "Clear 4 rows at once for a QUAD",
    "Back-to-back QUADs score 1.5x",
    "A T-spin double outscores a QUAD",
    "Hold parks a piece for later",
    "Every soft-dropped row is a point",
];

/// A new drifting piece roughly this often (§13.4).
pub const DRIFT_SPAWN: Duration = Duration::from_millis(1_200);
/// At most this many exist at once (§13.4).
pub const DRIFTERS: usize = 12;
/// A drifting piece falls one row about this often (§13.4), jittered per piece
/// so they do not march in lockstep.
pub const DRIFT_FALL: Duration = Duration::from_millis(600);
/// How far either side of [`DRIFT_FALL`] that jitter reaches.
pub const DRIFT_JITTER: Duration = Duration::from_millis(150);

/// The panel cycles every six seconds (§13.3).
const FACE: Duration = Duration::from_secs(6);
/// §13.6: the wordmark's colours start cycling after a minute of no keys...
const IDLE: Duration = Duration::from_secs(60);
/// ...one step per second.
const IDLE_STEP: Duration = Duration::from_secs(1);
/// §15.3: the attract screen runs at 10 fps, with no accumulator — there is no
/// core under it to advance.
const FRAME: Duration = Duration::from_millis(100);

/// What the attract screen asks the caller to do next (§7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing to do; the screen may or may not have changed.
    Stay,
    /// **PLAY** or **PILOT**: start a fresh game, with whoever is holding the
    /// controls (`PILOT.md` §P1).
    Play(Player),
    /// **QUIT**, or the quit key.
    Quit,
    /// The Options panel was left: §13.5 asks for the config to be saved and
    /// the presentation half applied at once.
    OptionsClosed,
}

/// What the front-end drawing this screen is offering (§13.3, §13.5).
///
/// Both lists are [`Session`]'s, and both are here for the same reason: the
/// screen draws them and this state machine walks them, so they cannot come to
/// disagree.
#[derive(Clone, Copy)]
struct Offered {
    menu: &'static [MenuChoice],
    settings: &'static [Setting],
}

/// The attract screen's whole state (§13).
pub struct Attract {
    now: Stamp,
    selected: usize,
    sub: Option<Sub>,
    /// §13.6: when a key was last pressed.
    last_key: Stamp,
    /// §13.3: when the panel's face last changed. Held at `now` — so the
    /// elapsed time stays zero — while the cycle is paused.
    face_since: Stamp,
    /// What [`selected`](Self::selected) is *on*, rather than where it is.
    ///
    /// §13.3 pauses the cycle on any item but the two that start a game, and
    /// which index those are is the front-end's list's answer: PILOT is the
    /// second item of `MenuChoice::ALL` and is not in a tab's list at all
    /// (`PILOT.md` §P7.1). [`advance`](Self::advance) is not handed that list,
    /// so the item is remembered as the cursor moves over it. Every list starts
    /// on PLAY.
    choice: MenuChoice,
    /// Counts faces shown, not the face on show: the third face's reminder is
    /// `face / FACES` so the tips rotate without a second counter.
    face: usize,
}

impl Attract {
    pub fn new(now: Stamp) -> Self {
        Self {
            now,
            selected: 0,
            sub: None,
            last_key: now,
            face_since: now,
            face: 0,
            choice: MenuChoice::Play,
        }
    }

    /// Which menu item the cursor is on (§13.3).
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The sub-screen over the menu, if one is open (§13.5).
    pub fn sub(&self) -> Option<Sub> {
        self.sub
    }

    /// How many faces the six-second panel cycle has shown (§13.3).
    ///
    /// A count, not the face on show: the reminders are the third face's, and
    /// `face / 3` is what rotates them without a second counter.
    pub fn face(&self) -> usize {
        self.face
    }

    /// Open a sub-screen directly, so a front-end's rendering tests can draw
    /// one without walking the menu to it.
    #[cfg(test)]
    pub(crate) fn open(&mut self, sub: Sub) {
        self.sub = Some(sub);
    }

    /// Advance the clock, reporting whether the *state* changed.
    ///
    /// [`Round::advance`](crate::shell::round::Round::advance)'s opposite
    /// number, and the reason it answers a `bool` rather than a
    /// [`Next`]: the attract screen has no core to advance and no way to end
    /// itself, so what §15.3 wants to know is whether anything moved. This is
    /// one of the two halves of that — the panel's cycle and §13.6's idle
    /// colours. §13.4's drift is the other, and it belongs to whichever
    /// front-end is drawing it, so its answer is folded in by the caller.
    pub fn advance(&mut self, now: Stamp) -> bool {
        let was = (self.face, self.idle_shift());
        self.now = now;
        // §13.3: the cycle pauses while a menu item other than PLAY or PILOT is
        // selected — the two that start a game, where the player is about to
        // leave the screen rather than read it. Holding the mark at `now` keeps
        // the elapsed time at zero, so the face that is up stays up rather than
        // jumping when it resumes.
        let starts_a_game = matches!(self.choice, MenuChoice::Play | MenuChoice::Pilot);
        if starts_a_game && self.sub.is_none() {
            while now.saturating_since(self.face_since) >= FACE {
                self.face_since += FACE;
                self.face += 1;
            }
        } else {
            self.face_since = now;
        }
        was != (self.face, self.idle_shift())
    }

    /// §15.2 step 2, for the front screen: fold in one key and say whether
    /// the run leaves for another screen (§7).
    ///
    /// §13.5's save happens here rather than in the front-end, because the
    /// rule is the specification's and a second front-end that forgot it would
    /// be a second §13.5. The generation bump is what tells a front-end
    /// comparing frames that a presentation setting moved under it (§15.2
    /// step 5).
    pub fn key(&mut self, session: &mut Session<'_>, event: &KeyEvent, now: Stamp) -> Option<Next> {
        let offered = Offered {
            menu: session.menu,
            settings: session.settings,
        };
        match self.dispatch(event, &mut session.config, offered, now) {
            Outcome::Stay => None,
            Outcome::Play(player) => Some(Next::Play(player)),
            Outcome::Quit => Some(Next::Quit),
            // §13.5: presentation takes effect the moment the panel is left,
            // and the config is written there and then.
            Outcome::OptionsClosed => {
                session.save_config();
                session.bump();
                None
            }
        }
    }

    /// §15.3's deadline: how long the front-end may wait before pumping again.
    ///
    /// Advice, exactly as [`Round::deadline`](crate::shell::round::Round::deadline)
    /// is — but a flat 10 fps, because there is no accumulator to be partway
    /// through. §15.3 asks for idle CPU under 2 %, and this is how.
    pub fn deadline(&self) -> Duration {
        FRAME
    }

    /// Fold in one key (§10.1: `↑`/`↓`, `Enter`/`Space`, `Esc`, always).
    ///
    /// `config` is borrowed because the Options sub-screen edits it in place
    /// (§13.5); nothing else here touches it.
    fn dispatch(
        &mut self,
        event: &KeyEvent,
        config: &mut ConfigFile,
        offered: Offered,
        now: Stamp,
    ) -> Outcome {
        if event.kind == KeyKind::Release {
            return Outcome::Stay;
        }
        // §13.6: any key stops the idle colour cycle.
        self.last_key = now;
        match self.sub {
            Some(Sub::Options { selected }) => {
                self.options_key(event, config, offered.settings, selected)
            }
            Some(_) => {
                if matches!(event.key, Key::Esc | Key::Enter | Key::Char(' ')) {
                    self.sub = None;
                }
                Outcome::Stay
            }
            None => self.menu_key(event, offered.menu),
        }
    }

    fn menu_key(&mut self, event: &KeyEvent, menu: &'static [MenuChoice]) -> Outcome {
        // The items this front-end offers, not §13.3's six: a cursor that can
        // reach an item the screen is not drawing is a cursor the player
        // cannot see (`Session::menu`).
        let items = menu.len();
        self.selected = self.selected.min(items.saturating_sub(1));
        match event.key {
            Key::Up => self.selected = (self.selected + items - 1) % items,
            Key::Down => self.selected = (self.selected + 1) % items,
            Key::Enter | Key::Char(' ') => match menu[self.selected] {
                MenuChoice::Play => return Outcome::Play(Player::Human),
                // `PILOT.md` §P1: the same game, watched rather than played.
                MenuChoice::Pilot => return Outcome::Play(Player::Pilot),
                MenuChoice::HighScores => self.sub = Some(Sub::HighScores),
                MenuChoice::Controls => self.sub = Some(Sub::Controls),
                MenuChoice::Options => self.sub = Some(Sub::Options { selected: 0 }),
                MenuChoice::Quit => return Outcome::Quit,
            },
            // §16: Ctrl-C is delivered as a key event and means quit from the
            // attract screen. §10.1's `q` is the ordinary way.
            Key::Char('c') if event.mods.ctrl => return Outcome::Quit,
            Key::Char('q') | Key::Char('Q') | Key::Esc => return Outcome::Quit,
            _ => {}
        }
        // Where the cursor ended up, as an *item*: §13.3's cycle asks which one
        // it is on and the list it is an index into is the front-end's.
        self.choice = menu[self.selected];
        Outcome::Stay
    }

    /// §13.5, the same panel the pause menu opens: `↑`/`↓` choose, `←`/`→`
    /// change, `Esc` saves and returns.
    fn options_key(
        &mut self,
        event: &KeyEvent,
        config: &mut ConfigFile,
        settings: &'static [Setting],
        selected: usize,
    ) -> Outcome {
        // The rows this front-end offers, not every row §13.5 lists: a cursor
        // that can reach a setting the screen is not drawing is a cursor the
        // player cannot see (`Session::settings`).
        let items = settings.len();
        match event.key {
            Key::Up => {
                self.sub = Some(Sub::Options {
                    selected: (selected + items - 1) % items,
                })
            }
            Key::Down => {
                self.sub = Some(Sub::Options {
                    selected: (selected + 1) % items,
                })
            }
            Key::Left | Key::Right => {
                settings[selected].step(config, event.key == Key::Right);
            }
            Key::Esc | Key::Enter => {
                self.sub = None;
                return Outcome::OptionsClosed;
            }
            _ => {}
        }
        Outcome::Stay
    }

    /// §13.6: how many steps the wordmark's colours have rotated.
    pub fn idle_shift(&self) -> usize {
        let idle = self.now.saturating_since(self.last_key);
        let Some(cycling) = idle.checked_sub(IDLE) else {
            return 0;
        };
        (cycling.as_nanos() / IDLE_STEP.as_nanos()) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_idle_cycle_starts_after_a_minute_and_steps_once_a_second() {
        // §13.6.
        let start = Stamp::ZERO;
        let mut state = Attract::new(start);
        state.advance(start + IDLE - Duration::from_millis(1));
        assert_eq!(state.idle_shift(), 0, "not yet");
        state.advance(start + IDLE);
        assert_eq!(state.idle_shift(), 0, "the first step is a second later");
        state.advance(start + IDLE + IDLE_STEP);
        assert_eq!(state.idle_shift(), 1);
        state.advance(start + IDLE + IDLE_STEP * 7);
        assert_eq!(state.idle_shift(), 7, "and it does not stop");
    }

    #[test]
    fn any_key_stops_the_idle_cycle() {
        let start = Stamp::ZERO;
        let mut state = Attract::new(start);
        let later = start + IDLE + IDLE_STEP * 3;
        state.advance(later);
        assert_eq!(state.idle_shift(), 3);
        state.dispatch(
            &press(Key::Char('x')),
            &mut ConfigFile::default(),
            everything(),
            later,
        );
        assert_eq!(state.idle_shift(), 0);
    }

    fn press(key: Key) -> KeyEvent {
        KeyEvent::press(key)
    }

    /// A front-end that offers all of §13.3 and all of §13.5: the terminal's.
    fn everything() -> Offered {
        Offered {
            menu: &MenuChoice::ALL,
            settings: &Setting::ALL,
        }
    }

    #[test]
    fn the_menu_wraps_and_play_is_the_first_item() {
        let now = Stamp::ZERO;
        let mut state = Attract::new(now);
        let mut config = ConfigFile::default();
        assert_eq!(MenuChoice::ALL[0], MenuChoice::Play);
        assert_eq!(
            state.dispatch(&press(Key::Enter), &mut config, everything(), now),
            Outcome::Play(Player::Human)
        );

        state.dispatch(&press(Key::Up), &mut config, everything(), now);
        assert_eq!(
            state.selected,
            MenuChoice::ALL.len() - 1,
            "up wraps to QUIT"
        );
        assert_eq!(
            state.dispatch(&press(Key::Enter), &mut config, everything(), now),
            Outcome::Quit
        );
        state.dispatch(&press(Key::Down), &mut config, everything(), now);
        assert_eq!(state.selected, 0, "and down wraps back to PLAY");
    }

    #[test]
    fn pilot_starts_a_game_the_player_watches() {
        // `PILOT.md` §P1, §P7.1: a row of its own under PLAY, which starts the
        // same game with somebody else holding the controls. Which one the
        // screen hands back is the only difference between the two rows, and it
        // is what a restart has to carry.
        let now = Stamp::ZERO;
        let mut state = Attract::new(now);
        let mut config = ConfigFile::default();
        assert_eq!(MenuChoice::ALL[1], MenuChoice::Pilot, "under PLAY");
        state.dispatch(&press(Key::Down), &mut config, everything(), now);
        assert_eq!(
            state.dispatch(&press(Key::Enter), &mut config, everything(), now),
            Outcome::Play(Player::Pilot),
        );
    }

    #[test]
    fn a_browser_tab_offers_neither_quit_nor_pilot() {
        // §13.3, `GUI.md` §G8.1, `PILOT.md` §P7.1: a tab cannot close itself
        // and would search on its frame thread, so its menu is `CANVAS` — and
        // the cursor must not be able to reach an item that is not being drawn.
        let now = Stamp::ZERO;
        let mut config = ConfigFile::default();
        let offered = Offered {
            menu: &MenuChoice::CANVAS,
            settings: &Setting::SHARED,
        };
        assert!(
            !MenuChoice::CANVAS.contains(&MenuChoice::Pilot)
                && !MenuChoice::CANVAS.contains(&MenuChoice::Quit),
        );
        let mut state = Attract::new(now);
        state.dispatch(&press(Key::Up), &mut config, offered, now);
        assert_eq!(
            MenuChoice::CANVAS[state.selected],
            MenuChoice::Options,
            "up from PLAY wraps to the last item there is",
        );
        // And down from PLAY is HIGH SCORES, not the row this front-end is not
        // offering: the list the shell walks is the list the screen draws.
        let mut walked = Attract::new(now);
        walked.dispatch(&press(Key::Down), &mut config, offered, now);
        assert_eq!(MenuChoice::CANVAS[walked.selected], MenuChoice::HighScores);
        assert_eq!(
            state.dispatch(&press(Key::Enter), &mut config, offered, now),
            Outcome::Stay,
        );
        assert_eq!(state.sub, Some(Sub::Options { selected: 0 }));

        // A cursor left on QUIT by a longer list is brought back into this one
        // rather than indexing off the end.
        let mut state = Attract::new(now);
        state.selected = MenuChoice::ALL.len() - 1;
        state.dispatch(&press(Key::Enter), &mut config, offered, now);
        assert_eq!(state.sub, Some(Sub::Options { selected: 0 }));
    }

    #[test]
    fn every_sub_screen_opens_and_esc_returns() {
        // §13.5.
        let now = Stamp::ZERO;
        let mut config = ConfigFile::default();
        // How far down each item is, asked of the list rather than counted out
        // here: §P7.1 put PILOT second and every row below it moved.
        let row = |choice: MenuChoice| {
            MenuChoice::ALL
                .iter()
                .position(|item| *item == choice)
                .expect("§13.3 offers it")
        };
        for (steps, sub) in [
            (row(MenuChoice::HighScores), Sub::HighScores),
            (row(MenuChoice::Controls), Sub::Controls),
            (row(MenuChoice::Options), Sub::Options { selected: 0 }),
        ] {
            let mut state = Attract::new(now);
            for _ in 0..steps {
                state.dispatch(&press(Key::Down), &mut config, everything(), now);
            }
            state.dispatch(&press(Key::Enter), &mut config, everything(), now);
            assert_eq!(state.sub, Some(sub));
            state.dispatch(&press(Key::Esc), &mut config, everything(), now);
            assert_eq!(state.sub, None, "{sub:?}");
        }
    }

    #[test]
    fn leaving_the_options_panel_asks_for_a_save() {
        // §13.5: "`Esc` saves the config file (§6.2) and returns". The panel
        // itself only edits; saving is the caller's, as it is from the pause
        // menu.
        let now = Stamp::ZERO;
        let mut state = Attract::new(now);
        let mut config = ConfigFile::default();
        state.sub = Some(Sub::Options { selected: 0 });
        state.dispatch(&press(Key::Right), &mut config, everything(), now);
        assert_eq!(config.gameplay.preview_count, 6);
        assert_eq!(
            state.dispatch(&press(Key::Esc), &mut config, everything(), now),
            Outcome::OptionsClosed,
        );
    }

    #[test]
    fn the_panel_cycles_every_six_seconds_and_pauses_off_the_two_that_play() {
        // §13.3 as `PILOT.md` §P7.1 amended it: "cycles every 6 seconds between
        // three faces... The cycle pauses while a menu item other than PLAY or
        // PILOT is selected" — the two that start a game, where the player is
        // about to leave the screen rather than read it.
        let start = Stamp::ZERO;
        let mut state = Attract::new(start);
        let mut config = ConfigFile::default();
        assert_eq!(state.face, 0);
        assert!(state.advance(start + FACE), "the face changed");
        assert_eq!(state.face, 1);
        state.advance(start + FACE * 3);
        assert_eq!(state.face, 3, "and round to the first face again");

        // Down once is PILOT, and the cycle carries on.
        state.dispatch(&press(Key::Down), &mut config, everything(), start);
        assert_eq!(state.choice, MenuChoice::Pilot);
        state.advance(start + FACE * 4);
        assert_eq!(state.face, 4, "PILOT starts a game too");

        // Down again is HIGH SCORES, and it stops.
        state.dispatch(&press(Key::Down), &mut config, everything(), start);
        state.advance(start + FACE * 10);
        assert_eq!(state.face, 4, "held while a reading item is selected");
        state.dispatch(&press(Key::Up), &mut config, everything(), start);
        state.dispatch(&press(Key::Up), &mut config, everything(), start);
        assert_eq!(state.choice, MenuChoice::Play);
        state.advance(start + FACE * 10 + FACE - Duration::from_millis(1));
        assert_eq!(state.face, 4, "and it resumes from where it paused");
        state.advance(start + FACE * 11);
        assert_eq!(state.face, 5);
    }
}
