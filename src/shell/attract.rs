//! The attract screen's state machine (§13.1, §13.3, §13.5, §13.6).
//!
//! What is selected, which sub-screen is open, how far the panel's six-second
//! cycle has come and how long the keyboard has been quiet — and nothing about
//! how any of it is drawn. §13.4's drifting background is deliberately *not*
//! here: it is positioned in the units of whatever is drawing it, so each
//! front-end keeps its own and folds its "did anything move" answer in beside
//! [`Attract::step`]'s.

use std::time::Duration;

use crate::shell::config::ConfigFile;
use crate::shell::keys::{Key, KeyEvent, KeyKind};
use crate::shell::menus::{MenuChoice, Setting, Sub};
use crate::shell::session::{Next, Session};
use crate::shell::time::Stamp;

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
    /// **PLAY**: start a fresh game.
    Play,
    /// **QUIT**, or the quit key.
    Quit,
    /// The Options panel was left: §13.5 asks for the config to be saved and
    /// the presentation half applied at once.
    OptionsClosed,
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
        // §13.3: the cycle pauses while a menu item other than PLAY is
        // selected. Holding the mark at `now` keeps the elapsed time at zero,
        // so the face that is up stays up rather than jumping when it resumes.
        if self.selected == 0 && self.sub.is_none() {
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
        let settings = session.settings;
        match self.dispatch(event, &mut session.config, settings, now) {
            Outcome::Stay => None,
            Outcome::Play => Some(Next::Play),
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
        settings: &'static [Setting],
        now: Stamp,
    ) -> Outcome {
        if event.kind == KeyKind::Release {
            return Outcome::Stay;
        }
        // §13.6: any key stops the idle colour cycle.
        self.last_key = now;
        match self.sub {
            Some(Sub::Options { selected }) => self.options_key(event, config, settings, selected),
            Some(_) => {
                if matches!(event.key, Key::Esc | Key::Enter | Key::Char(' ')) {
                    self.sub = None;
                }
                Outcome::Stay
            }
            None => self.menu_key(event),
        }
    }

    fn menu_key(&mut self, event: &KeyEvent) -> Outcome {
        let items = MenuChoice::ALL.len();
        match event.key {
            Key::Up => self.selected = (self.selected + items - 1) % items,
            Key::Down => self.selected = (self.selected + 1) % items,
            Key::Enter | Key::Char(' ') => match MenuChoice::ALL[self.selected] {
                MenuChoice::Play => return Outcome::Play,
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
            &Setting::ALL,
            later,
        );
        assert_eq!(state.idle_shift(), 0);
    }

    fn press(key: Key) -> KeyEvent {
        KeyEvent::press(key)
    }

    #[test]
    fn the_menu_wraps_and_play_is_the_first_item() {
        let now = Stamp::ZERO;
        let mut state = Attract::new(now);
        let mut config = ConfigFile::default();
        assert_eq!(MenuChoice::ALL[0], MenuChoice::Play);
        assert_eq!(
            state.dispatch(&press(Key::Enter), &mut config, &Setting::ALL, now),
            Outcome::Play
        );

        state.dispatch(&press(Key::Up), &mut config, &Setting::ALL, now);
        assert_eq!(
            state.selected,
            MenuChoice::ALL.len() - 1,
            "up wraps to QUIT"
        );
        assert_eq!(
            state.dispatch(&press(Key::Enter), &mut config, &Setting::ALL, now),
            Outcome::Quit
        );
        state.dispatch(&press(Key::Down), &mut config, &Setting::ALL, now);
        assert_eq!(state.selected, 0, "and down wraps back to PLAY");
    }

    #[test]
    fn every_sub_screen_opens_and_esc_returns() {
        // §13.5.
        let now = Stamp::ZERO;
        let mut config = ConfigFile::default();
        for (steps, sub) in [
            (1, Sub::HighScores),
            (2, Sub::Controls),
            (3, Sub::Options { selected: 0 }),
        ] {
            let mut state = Attract::new(now);
            for _ in 0..steps {
                state.dispatch(&press(Key::Down), &mut config, &Setting::ALL, now);
            }
            state.dispatch(&press(Key::Enter), &mut config, &Setting::ALL, now);
            assert_eq!(state.sub, Some(sub));
            state.dispatch(&press(Key::Esc), &mut config, &Setting::ALL, now);
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
        state.dispatch(&press(Key::Right), &mut config, &Setting::ALL, now);
        assert_eq!(config.gameplay.preview_count, 6);
        assert_eq!(
            state.dispatch(&press(Key::Esc), &mut config, &Setting::ALL, now),
            Outcome::OptionsClosed,
        );
    }

    #[test]
    fn the_panel_cycles_every_six_seconds_and_pauses_off_play() {
        // §13.3: "cycles every 6 seconds between three faces... The cycle
        // pauses while a menu item other than PLAY is selected."
        let start = Stamp::ZERO;
        let mut state = Attract::new(start);
        assert_eq!(state.face, 0);
        assert!(state.advance(start + FACE), "the face changed");
        assert_eq!(state.face, 1);
        state.advance(start + FACE * 3);
        assert_eq!(state.face, 3, "and round to the first face again");

        state.selected = 1;
        state.advance(start + FACE * 9);
        assert_eq!(state.face, 3, "held while HIGH SCORES is selected");
        state.selected = 0;
        state.advance(start + FACE * 9 + FACE - Duration::from_millis(1));
        assert_eq!(state.face, 3, "and it resumes from where it paused");
        state.advance(start + FACE * 10);
        assert_eq!(state.face, 4);
    }
}
