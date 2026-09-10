//! The neutral key event, and §10.1's key-name grammar (`FRONTEND.md` F5).
//!
//! Nothing above the core may name a front-end's toolkit, and a key event is
//! where that would otherwise happen first: §10.1's key names are what a
//! player's `[keys]` table is written in, so the vocabulary has to be shared
//! property rather than crossterm's, egui's or Macroquad's. Each front-end
//! adapts its own keyboard into these types and hands the shell one event at a
//! time.
//!
//! [`KeyEvent`] is an *event*, and not every front-end has events: one with
//! polled per-frame input synthesises the stream by diffing its key set
//! between frames. Nothing here or above may therefore depend on receiving
//! every intermediate event, on sub-frame ordering between two keys, or on an
//! event arriving anywhere but a frame boundary. §10.3's DAS/ARR already
//! satisfies that, because it works off accumulated time and not off event
//! counts.

/// A key, in §10.1's vocabulary.
///
/// `Space` is `Char(' ')`, exactly as [`parse_key`] resolves the name — the
/// grammar is unchanged, so no config file's `[keys]` table changes meaning.
/// A key this cannot express — `Insert`, a media key — is dropped by the
/// front-end's adapter, which is what the bindings table does with it anyway.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Enter,
    Tab,
    Esc,
    Backspace,
    F(u8),
    Char(char),
}

/// The modifiers held with a key.
///
/// `logo` is the Super/Command/Windows key, whatever the platform calls it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool,
}

impl Mods {
    /// No modifiers at all.
    pub const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        logo: false,
    };

    /// Ctrl alone, which is the one combination the game reads (§16).
    pub const CTRL: Self = Self {
        ctrl: true,
        ..Self::NONE
    };

    /// Whether these modifiers rule the key out as a game binding.
    ///
    /// §10.1's names are all bare keys, and Ctrl-C in particular means
    /// something else (§16). Shift is not in the list: it is how `Z` is typed.
    pub const fn not_a_game_key(self) -> bool {
        self.ctrl || self.alt || self.logo
    }
}

/// Whether a key event is a press, the operating system's auto-repeat, or a
/// release.
///
/// The three-way distinction is kept because a repeat is not a press: §8.2's
/// enhanced path and egui both report repeats, and both must discard them so
/// that DAS is driven by the clock rather than by the terminal's or the
/// window system's repeat rate. A front-end that cannot report releases at all
/// says so, and the shell falls back to §8.2's legacy timeout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyKind {
    Press,
    Repeat,
    Release,
}

/// One key event, as every front-end delivers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub mods: Mods,
    pub kind: KeyKind,
}

impl KeyEvent {
    pub const fn new(key: Key, mods: Mods, kind: KeyKind) -> Self {
        Self { key, mods, kind }
    }

    /// An unmodified press, which is what nearly every key is.
    pub const fn press(key: Key) -> Self {
        Self::new(key, Mods::NONE, KeyKind::Press)
    }

    /// An unmodified release.
    pub const fn release(key: Key) -> Self {
        Self::new(key, Mods::NONE, KeyKind::Release)
    }

    /// Whether this event is Ctrl-C, which means quit wherever it lands (§16).
    ///
    /// It is checked before the bindings table, because `c` is bound to hold
    /// by default.
    pub const fn is_ctrl_c(&self) -> bool {
        self.mods.ctrl && matches!(self.key, Key::Char('c'))
    }
}

/// Whether `name` is a key name §10.1 recognises.
///
/// The grammar lives here, with the parser that owns it, and the config loader
/// asks rather than keeping a second copy that could drift (§6.2).
pub fn is_key_name(name: &str) -> bool {
    parse_key(name).is_some()
}

/// A key name from §10.1 as a [`Key`].
///
/// `Left`, `Right`, `Up`, `Down`, `Space`, `Enter`, `Tab`, `Esc`, `Backspace`,
/// `F1`-`F12`, and single characters, case-sensitive.
pub(crate) fn parse_key(name: &str) -> Option<Key> {
    Some(match name {
        "Left" => Key::Left,
        "Right" => Key::Right,
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Space" => Key::Char(' '),
        "Enter" => Key::Enter,
        "Tab" => Key::Tab,
        "Esc" => Key::Esc,
        "Backspace" => Key::Backspace,
        _ => {
            if let Some(number) = name.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
                if (1..=12).contains(&number) {
                    return Some(Key::F(number));
                }
                return None;
            }
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Key::Char(c),
                _ => return None,
            }
        }
    })
}

/// Every key name §10.1 defines, for the tests that check a front-end's
/// adapter covers the whole grammar.
#[cfg(test)]
pub(crate) const KEY_NAMES: [&str; 22] = [
    "Left",
    "Right",
    "Up",
    "Down",
    "Space",
    "Enter",
    "Tab",
    "Esc",
    "Backspace",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
    "z",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_names_of_the_spec_all_parse() {
        // §10.1's list, exactly.
        for (name, key) in [
            ("Left", Key::Left),
            ("Right", Key::Right),
            ("Up", Key::Up),
            ("Down", Key::Down),
            ("Space", Key::Char(' ')),
            ("Enter", Key::Enter),
            ("Tab", Key::Tab),
            ("Esc", Key::Esc),
            ("Backspace", Key::Backspace),
            ("F1", Key::F(1)),
            ("F12", Key::F(12)),
            ("z", Key::Char('z')),
            ("Z", Key::Char('Z')),
            ("F", Key::Char('F')),
        ] {
            assert_eq!(parse_key(name), Some(key), "{name}");
        }
        for name in ["", "F0", "F13", "Ctrl", "left", "PageUp"] {
            assert_eq!(parse_key(name), None, "{name}");
        }
    }

    #[test]
    fn every_spec_key_name_is_a_key_name() {
        // The grammar and the question the loader asks are the same thing
        // (§6.2), so `is_key_name` must accept exactly what `parse_key` does.
        for name in KEY_NAMES {
            assert!(is_key_name(name), "{name}");
            assert!(parse_key(name).is_some(), "{name}");
        }
        assert!(!is_key_name("PageUp"));
    }

    #[test]
    fn ctrl_alt_and_logo_disqualify_a_key_but_shift_does_not() {
        // §10.1's names are bare keys; `Z` is one of them, and it is typed
        // with shift.
        assert!(!Mods::NONE.not_a_game_key());
        assert!(
            !Mods {
                shift: true,
                ..Mods::NONE
            }
            .not_a_game_key()
        );
        for mods in [
            Mods::CTRL,
            Mods {
                alt: true,
                ..Mods::NONE
            },
            Mods {
                logo: true,
                ..Mods::NONE
            },
        ] {
            assert!(mods.not_a_game_key(), "{mods:?}");
        }
    }

    #[test]
    fn ctrl_c_is_recognised_whatever_it_is_bound_to() {
        // §16: it is checked before the bindings table, because `c` is hold.
        assert!(KeyEvent::new(Key::Char('c'), Mods::CTRL, KeyKind::Press).is_ctrl_c());
        assert!(!KeyEvent::press(Key::Char('c')).is_ctrl_c());
        assert!(!KeyEvent::new(Key::Char('x'), Mods::CTRL, KeyKind::Press).is_ctrl_c());
    }
}
