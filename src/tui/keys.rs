//! crossterm to [`shell::keys`](crate::shell::keys): the terminal front-end's
//! key adapter (`FRONTEND.md` F5).
//!
//! This is the only place in the terminal front-end that names a crossterm key
//! type. The loop converts what it reads and hands the shell neutral events;
//! nothing above this module knows what decoded them.
//!
//! It is a function rather than a `From` impl because the conversion is
//! partial — a `KeyCode` with no §10.1 equivalent has no neutral form — and
//! the orphan rule forbids `impl From<crossterm::event::KeyEvent> for
//! Option<KeyEvent>`, both types being foreign to the impl's crate in the
//! sense that check cares about.

use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

use crate::shell::keys::{Key, KeyEvent, KeyKind, Mods};

/// One crossterm key event as a neutral one, or `None` if §10.1's vocabulary
/// cannot express it.
///
/// `Insert`, `Home`, `BackTab` and the media keys are the `None` cases. Losing
/// them costs nothing: the bindings table has no name for any of them, so they
/// were already inert (§10.1).
pub fn neutral(event: crossterm::event::KeyEvent) -> Option<KeyEvent> {
    Some(KeyEvent {
        key: key(event.code)?,
        mods: mods(event.modifiers),
        kind: kind(event.kind),
    })
}

fn key(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Enter => Key::Enter,
        KeyCode::Tab => Key::Tab,
        KeyCode::Esc => Key::Esc,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::F(n) => Key::F(n),
        KeyCode::Char(c) => Key::Char(c),
        _ => return None,
    })
}

/// crossterm reports Hyper and Meta as well; neither is one of §10.1's, and
/// neither changes the answer [`Mods::not_a_game_key`] gives.
fn mods(modifiers: KeyModifiers) -> Mods {
    Mods {
        ctrl: modifiers.contains(KeyModifiers::CONTROL),
        alt: modifiers.contains(KeyModifiers::ALT),
        shift: modifiers.contains(KeyModifiers::SHIFT),
        logo: modifiers.contains(KeyModifiers::SUPER),
    }
}

/// §8.2's three kinds, one to one. In legacy mode the terminal sends only
/// `Press`, and the shell's hold timeout stands in for the missing releases.
fn kind(kind: KeyEventKind) -> KeyKind {
    match kind {
        KeyEventKind::Press => KeyKind::Press,
        KeyEventKind::Repeat => KeyKind::Repeat,
        KeyEventKind::Release => KeyKind::Release,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::keys::{KEY_NAMES, parse_key};

    /// The `KeyCode` a real terminal sends for each of §10.1's key names.
    fn sent_for(name: &str) -> KeyCode {
        match name {
            "Left" => KeyCode::Left,
            "Right" => KeyCode::Right,
            "Up" => KeyCode::Up,
            "Down" => KeyCode::Down,
            "Space" => KeyCode::Char(' '),
            "Enter" => KeyCode::Enter,
            "Tab" => KeyCode::Tab,
            "Esc" => KeyCode::Esc,
            "Backspace" => KeyCode::Backspace,
            _ => match name.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
                Some(number) => KeyCode::F(number),
                None => KeyCode::Char(name.chars().next().unwrap()),
            },
        }
    }

    #[test]
    fn every_spec_key_name_round_trips_through_the_adapter() {
        // The name a player writes in `[keys]`, the `KeyCode` their terminal
        // sends when they press it, and the neutral key the bindings table
        // matches on must all be the same key. This is the join `Bindings`
        // depends on, and it is the one thing a new front-end's adapter has to
        // get right.
        for name in KEY_NAMES {
            let event = crossterm::event::KeyEvent::new(sent_for(name), KeyModifiers::NONE);
            let neutral = neutral(event).expect("§10.1's keys all have a neutral form");
            assert_eq!(Some(neutral.key), parse_key(name), "{name}");
            assert_eq!(neutral.kind, KeyKind::Press, "{name}");
            assert_eq!(neutral.mods, Mods::NONE, "{name}");
        }
    }

    #[test]
    fn a_key_the_neutral_type_cannot_express_is_dropped() {
        // §10.1 has no name for any of these, so the bindings table already
        // ignored them; the adapter drops them one step earlier.
        for code in [
            KeyCode::Insert,
            KeyCode::Home,
            KeyCode::PageUp,
            KeyCode::Delete,
            KeyCode::BackTab,
        ] {
            let event = crossterm::event::KeyEvent::new(code, KeyModifiers::NONE);
            assert!(neutral(event).is_none(), "{code:?}");
        }
    }

    #[test]
    fn the_three_kinds_map_one_to_one() {
        // §8.2: the distinction is what lets the enhanced path discard the
        // terminal's auto-repeat.
        for (from, to) in [
            (KeyEventKind::Press, KeyKind::Press),
            (KeyEventKind::Repeat, KeyKind::Repeat),
            (KeyEventKind::Release, KeyKind::Release),
        ] {
            let event =
                crossterm::event::KeyEvent::new_with_kind(KeyCode::Left, KeyModifiers::NONE, from);
            assert_eq!(neutral(event).unwrap().kind, to);
        }
    }

    #[test]
    fn ctrl_c_survives_the_adapter() {
        // §16 is checked in the shell, so the adapter has to carry the
        // modifier across rather than swallowing it.
        let event = crossterm::event::KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(neutral(event).unwrap().is_ctrl_c());
        // Shift is not a disqualifier: `Z` is a §10.1 name.
        let event = crossterm::event::KeyEvent::new(KeyCode::Char('Z'), KeyModifiers::SHIFT);
        assert!(!neutral(event).unwrap().mods.not_a_game_key());
    }
}
