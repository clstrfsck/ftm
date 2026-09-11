//! `egui` to [`shell::keys`](crate::shell::keys): the window front-end's key
//! adapter (`FRONTEND.md` F5, `GUI.md` §G2).
//!
//! This is the only module in `gui/` that names an `egui` key type, exactly as
//! `tui/keys.rs` is the only one that names a crossterm one. What goes up from
//! here is §10.1's vocabulary and nothing else, so a `[keys]` table written by
//! either binary means the same thing in both.
//!
//! Two things this adapter has that the terminal's does not.
//!
//! * **It is unconditionally §8.2's enhanced case.** `egui` reports a true
//!   release for every press and flags the operating system's auto-repeat, so
//!   the legacy hold timeout is never reached from here and
//!   [`InputMode::Legacy`](crate::shell::input::InputMode) is never
//!   constructed. Repeats are carried across as [`KeyKind::Repeat`] and
//!   discarded above, which is what keeps DAS on the clock (§10.3).
//! * **It has to answer for focus.** A window that loses focus stops being
//!   sent key events, and the release of a key let go outside it never
//!   arrives — so a held direction would keep sliding when focus came back.
//!   [`Keyboard`] therefore remembers what it has reported as held and
//!   synthesises the releases itself. That is legitimate rather than a hack:
//!   F5 makes the event stream synthesisable precisely so a front-end may
//!   manufacture it.

use crate::shell::keys::{Key, KeyEvent, KeyKind, Mods};

/// The adapter, and the little state that focus loss requires.
///
/// `held` is what this module has told the shell is down. It is not the game's
/// idea of a held key — that is `InputState`'s, one layer up — only what has
/// to be taken back if the window stops hearing about it.
#[derive(Clone, Debug, Default)]
pub struct Keyboard {
    held: Vec<Key>,
}

impl Keyboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Translate one frame's worth of `egui` events, appending to `out`.
    ///
    /// Appending rather than returning, so the front-end can keep one buffer
    /// for the life of the window; `out` is not cleared here.
    pub fn absorb(&mut self, events: &[egui::Event], out: &mut Vec<KeyEvent>) {
        for event in events {
            match *event {
                egui::Event::Key {
                    key,
                    pressed,
                    repeat,
                    modifiers,
                    ..
                } => {
                    let mods = mods(modifiers);
                    let Some(key) = self::key(key, mods.shift) else {
                        continue;
                    };
                    let kind = match (pressed, repeat) {
                        (true, false) => KeyKind::Press,
                        (true, true) => KeyKind::Repeat,
                        (false, _) => KeyKind::Release,
                    };
                    self.note(key, kind);
                    out.push(KeyEvent::new(key, mods, kind));
                }
                // The releases the window will never be sent. Order does not
                // matter — `InputState` cancels each key's timers on its own
                // release (§10.3 step 4) — but completeness does.
                egui::Event::WindowFocused(false) => {
                    for key in self.held.drain(..) {
                        out.push(KeyEvent::release(key));
                    }
                }
                _ => {}
            }
        }
    }

    /// Remember, or forget, a key this adapter has reported as down.
    ///
    /// A repeat is not a press and does not add a second entry; a release of
    /// something never pressed — which is what the first event after a focus
    /// loss can be — simply finds nothing to remove.
    fn note(&mut self, key: Key, kind: KeyKind) {
        match kind {
            KeyKind::Press => {
                if !self.held.contains(&key) {
                    self.held.push(key);
                }
            }
            KeyKind::Repeat => {}
            KeyKind::Release => self.held.retain(|held| *held != key),
        }
    }
}

/// One `egui` key as one of §10.1's, or `None` if the grammar has no name for
/// it.
///
/// `shift` is what decides a letter's case, because `egui` reports the key and
/// not the character: §10.1's names are case-sensitive and the default
/// bindings list both halves (`z` and `Z`), so `Shift`+`Z` has to arrive as
/// `Char('Z')` — which is exactly what a terminal sends. Nothing else consults
/// it: `egui` has its own variant for each shifted punctuation key it can
/// reach, and none at all for a shifted digit. `GUI.md` §G2.1 records the gap
/// that leaves.
fn key(key: egui::Key, shift: bool) -> Option<Key> {
    use egui::Key as E;
    Some(match key {
        E::ArrowLeft => Key::Left,
        E::ArrowRight => Key::Right,
        E::ArrowUp => Key::Up,
        E::ArrowDown => Key::Down,
        E::Enter => Key::Enter,
        E::Tab => Key::Tab,
        E::Escape => Key::Esc,
        E::Backspace => Key::Backspace,
        // §10.1 spells this `Space` and `parse_key` resolves it to a blank, so
        // the neutral form is a character like any other.
        E::Space => Key::Char(' '),
        E::F1 => Key::F(1),
        E::F2 => Key::F(2),
        E::F3 => Key::F(3),
        E::F4 => Key::F(4),
        E::F5 => Key::F(5),
        E::F6 => Key::F(6),
        E::F7 => Key::F(7),
        E::F8 => Key::F(8),
        E::F9 => Key::F(9),
        E::F10 => Key::F(10),
        E::F11 => Key::F(11),
        E::F12 => Key::F(12),
        other => Key::Char(character(other, shift)?),
    })
}

/// The character a key types, for the keys §10.1 names by their character.
///
/// Two halves. The letters and the digits come from `egui::Key::name`, which is
/// a single character for exactly those — `"A"`, `"0"` — so a key whose name is
/// longer (`F13`, `PageUp`, `BrowserBack`) falls out as `None`, which is the
/// same answer §10.1's grammar gives it. The punctuation does **not**: `name`
/// spells those out (`"Minus"`, `"Colon"`), so they need the table below, and
/// they need it because §10.1 lets a player bind any single character and a
/// `[keys]` table must mean the same thing in both front-ends (§6.2).
fn character(key: egui::Key, shift: bool) -> Option<char> {
    use egui::Key as E;
    let punctuation = match key {
        E::Backtick => '`',
        E::Minus => '-',
        E::Equals => '=',
        E::OpenBracket => '[',
        E::CloseBracket => ']',
        E::Backslash => '\\',
        E::Semicolon => ';',
        E::Quote => '\'',
        E::Comma => ',',
        E::Period => '.',
        E::Slash => '/',
        // `egui` reaches these only in their shifted form, so they carry their
        // own character and the modifier says nothing further.
        E::Exclamationmark => '!',
        E::Plus => '+',
        E::Colon => ':',
        E::Questionmark => '?',
        E::Pipe => '|',
        E::OpenCurlyBracket => '{',
        E::CloseCurlyBracket => '}',
        _ => {
            let name = key.name();
            let mut chars = name.chars();
            return match (chars.next(), chars.next()) {
                (Some(c), None) if shift => Some(c),
                (Some(c), None) => Some(c.to_ascii_lowercase()),
                _ => None,
            };
        }
    };
    Some(punctuation)
}

/// `egui`'s modifiers as §10.1's.
///
/// `mac_cmd` is the Super/Command key; `command` is deliberately ignored,
/// because it is `ctrl` again on every platform but macOS and reading both
/// would say nothing new. What the shell does with these is decide that the
/// key is not a game binding at all (§10.1, §16).
fn mods(modifiers: egui::Modifiers) -> Mods {
    Mods {
        ctrl: modifiers.ctrl,
        alt: modifiers.alt,
        shift: modifiers.shift,
        logo: modifiers.mac_cmd,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::keys::{KEY_NAMES, parse_key};

    fn pressed(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn one(event: egui::Event) -> Option<KeyEvent> {
        let mut out = Vec::new();
        Keyboard::new().absorb(&[event], &mut out);
        out.pop()
    }

    /// The `egui::Key` a window is sent for each of §10.1's key names.
    fn sent_for(name: &str) -> egui::Key {
        match name {
            "Left" => egui::Key::ArrowLeft,
            "Right" => egui::Key::ArrowRight,
            "Up" => egui::Key::ArrowUp,
            "Down" => egui::Key::ArrowDown,
            "Space" => egui::Key::Space,
            "Enter" => egui::Key::Enter,
            "Tab" => egui::Key::Tab,
            "Esc" => egui::Key::Escape,
            "Backspace" => egui::Key::Backspace,
            _ => match name.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
                Some(number) => *egui::Key::ALL
                    .iter()
                    .find(|key| key.name() == format!("F{number}"))
                    .expect("egui has F1-F35"),
                None => *egui::Key::ALL
                    .iter()
                    .find(|key| key.name().eq_ignore_ascii_case(name))
                    .expect("§10.1's characters are all egui keys"),
            },
        }
    }

    #[test]
    fn every_spec_key_name_round_trips_through_the_adapter() {
        // The same join `tui/keys.rs` checks, for the other front-end: the
        // name a player writes in `[keys]`, the key their window reports, and
        // the neutral key the bindings table matches on are one key. This is
        // the one thing a new adapter has to get right (`FRONTEND.md` F5).
        for name in KEY_NAMES {
            let event = one(pressed(sent_for(name), egui::Modifiers::NONE)).expect("a neutral key");
            assert_eq!(Some(event.key), parse_key(name), "{name}");
            assert_eq!(event.kind, KeyKind::Press, "{name}");
            assert_eq!(event.mods, Mods::NONE, "{name}");
        }
    }

    #[test]
    fn every_character_this_adapter_produces_is_one_a_player_could_have_written() {
        // §10.1 lets a `[keys]` entry be any single character, and §6.2 gives
        // the two binaries one config file — so a key that arrives here as
        // `Char(c)` must be the key the grammar resolves `c` to, or the same
        // file would mean two different things. This is the check that caught
        // `egui::Key::Minus` naming itself "Minus": the punctuation needs a
        // table, where the letters and digits do not.
        let mut punctuation = 0;
        for key in egui::Key::ALL {
            for shift in [false, true] {
                let Some(Key::Char(c)) = self::key(*key, shift) else {
                    continue;
                };
                assert_eq!(
                    parse_key(&c.to_string()),
                    Some(Key::Char(c)),
                    "{key:?} produced {c:?}, which §10.1 does not name",
                );
                if !c.is_ascii_alphanumeric() && *key != egui::Key::Space {
                    punctuation += 1;
                }
            }
        }
        assert!(
            punctuation >= 18,
            "only {punctuation} punctuation keys reached the grammar",
        );
    }

    #[test]
    fn the_punctuation_keys_carry_their_character_and_not_their_name() {
        for (key, expected) in [
            (egui::Key::Minus, '-'),
            (egui::Key::Period, '.'),
            (egui::Key::Slash, '/'),
            (egui::Key::Semicolon, ';'),
            (egui::Key::Backtick, '`'),
            // Reached only shifted, so it carries its own character and the
            // modifier says nothing further.
            (egui::Key::Questionmark, '?'),
        ] {
            let event = one(pressed(key, egui::Modifiers::NONE)).expect("a neutral key");
            assert_eq!(event.key, Key::Char(expected), "{key:?}");
        }
    }

    #[test]
    fn shift_selects_the_upper_case_name_and_does_not_disqualify_the_key() {
        // §10.1's names are case-sensitive and the default bindings list both
        // halves, so `Z` has to be reachable — and shift alone must not make
        // the key a non-binding (§10.1).
        let event = one(pressed(egui::Key::Z, egui::Modifiers::SHIFT)).expect("a neutral key");
        assert_eq!(event.key, Key::Char('Z'));
        assert!(!event.mods.not_a_game_key());
        let event = one(pressed(egui::Key::Z, egui::Modifiers::NONE)).expect("a neutral key");
        assert_eq!(event.key, Key::Char('z'));
    }

    #[test]
    fn a_key_the_neutral_type_cannot_express_is_dropped() {
        // §10.1 has no name for any of these, so the bindings table already
        // ignored them; the adapter drops them one step earlier, which is why
        // §13.6 can say "any key the game can *name*".
        for key in [
            egui::Key::Insert,
            egui::Key::Home,
            egui::Key::PageUp,
            egui::Key::Delete,
            egui::Key::F13,
            egui::Key::BrowserBack,
        ] {
            assert!(
                one(pressed(key, egui::Modifiers::NONE)).is_none(),
                "{key:?}",
            );
        }
    }

    #[test]
    fn a_repeat_is_reported_as_a_repeat_and_not_as_a_press() {
        // §8.2: the window front-end is unconditionally the enhanced case, and
        // the distinction is what lets the shell discard the operating
        // system's repeat rate and drive DAS from the clock instead (§10.3).
        let event = egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: egui::Modifiers::NONE,
        };
        assert_eq!(one(event).expect("a neutral key").kind, KeyKind::Repeat);
    }

    #[test]
    fn ctrl_c_survives_the_adapter() {
        // §16 is checked in the shell, so the adapter carries the modifier
        // across rather than swallowing it.
        let event = one(pressed(egui::Key::C, egui::Modifiers::CTRL)).expect("a neutral key");
        assert!(event.is_ctrl_c());
    }

    #[test]
    fn losing_focus_releases_every_key_the_window_reported_as_held() {
        // The release of a key let go outside the window never arrives, so a
        // held direction would slide on when focus came back. F5 makes the
        // stream synthesisable; this is what that is for.
        let mut keyboard = Keyboard::new();
        let mut out = Vec::new();
        keyboard.absorb(
            &[
                pressed(egui::Key::ArrowLeft, egui::Modifiers::NONE),
                pressed(egui::Key::ArrowDown, egui::Modifiers::NONE),
                // A repeat must not enqueue a second release.
                egui::Event::Key {
                    key: egui::Key::ArrowLeft,
                    physical_key: None,
                    pressed: true,
                    repeat: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            &mut out,
        );
        out.clear();
        keyboard.absorb(&[egui::Event::WindowFocused(false)], &mut out);
        assert_eq!(
            out,
            vec![KeyEvent::release(Key::Left), KeyEvent::release(Key::Down),],
        );
        // And having given them back, it does not give them back twice.
        out.clear();
        keyboard.absorb(&[egui::Event::WindowFocused(false)], &mut out);
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn a_key_released_normally_is_not_released_again_by_a_focus_loss() {
        let mut keyboard = Keyboard::new();
        let mut out = Vec::new();
        keyboard.absorb(
            &[
                pressed(egui::Key::ArrowLeft, egui::Modifiers::NONE),
                egui::Event::Key {
                    key: egui::Key::ArrowLeft,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::WindowFocused(false),
            ],
            &mut out,
        );
        assert_eq!(
            out,
            vec![KeyEvent::press(Key::Left), KeyEvent::release(Key::Left),],
        );
    }
}
