//! §6.4's enumerated values, as `clap` spells them.
//!
//! The *grammar* of a command line belongs to a front-end and its meaning to
//! the shell (§6.4), so the flags themselves live in `tui/cli.rs` and
//! `gui/cli.rs` — but a value both binaries accept has to be spelled the same
//! way by both, and `--lock-down classic` is one: it names a §6.3 rule, and
//! §6.2 gives `ftm` and `ftm-gui` one config file to write it into.
//!
//! So this is `src/native.rs`'s counterpart for the one capability that is
//! neither a clock, a filesystem, an entropy source nor a calendar: an argv. It
//! is behind the same gate for the same reason — a browser tab has no argv
//! either, and reads §6.4 off the URL instead (`GUI.md` §G8.4).
//!
//! Written out rather than derived on the shell's types, because a `ValueEnum`
//! derive would put `clap` in `shell/config.rs` and the boundary
//! `cargo check --no-default-features` holds is worth more than the nine lines
//! it saves. The tests below are what keep the two spellings — §6.3's table and
//! this one — from drifting apart.

use clap::ValueEnum;
use clap::builder::PossibleValue;

use crate::shell::config::{ColorDepth, LockDownRule};

/// §6.3's `lock_down`, which both native binaries take (`GUI.md` §G8.11).
impl ValueEnum for LockDownRule {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            LockDownRule::Extended,
            LockDownRule::Infinite,
            LockDownRule::Classic,
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(match self {
            LockDownRule::Extended => "extended",
            LockDownRule::Infinite => "infinite",
            LockDownRule::Classic => "classic",
        }))
    }
}

/// §12.3's colour depth, which only `ftm` takes — `--color` is a terminal's
/// flag (`TUI.md` §6.3) and `gui/cli.rs` has no such thing. The spelling lives
/// here beside its neighbour all the same, so that the one test below covers
/// both of §6.3's enumerated tables.
impl ValueEnum for ColorDepth {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            ColorDepth::Auto,
            ColorDepth::Truecolor,
            ColorDepth::Ansi256,
            ColorDepth::Ansi16,
            ColorDepth::Mono,
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(match self {
            ColorDepth::Auto => "auto",
            ColorDepth::Truecolor => "truecolor",
            ColorDepth::Ansi256 => "256",
            ColorDepth::Ansi16 => "16",
            ColorDepth::Mono => "mono",
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What §6.3 writes in the file, for each of the two enumerated settings.
    /// A spelling that drifted would be rejected at the command line and
    /// accepted in the file, which is exactly the divergence §6.4's synopsis
    /// exists to stop.
    #[test]
    fn every_value_is_spelled_as_the_config_file_spells_it() {
        for rule in LockDownRule::value_variants() {
            let name = rule.to_possible_value().unwrap();
            let name = name.get_name().to_string();
            let written = toml::Value::try_from(rule).unwrap();
            assert_eq!(written, toml::Value::String(name), "{rule:?}");
        }
        for depth in ColorDepth::value_variants() {
            let name = depth.to_possible_value().unwrap();
            let name = name.get_name().to_string();
            let written = toml::Value::try_from(depth).unwrap();
            assert_eq!(written, toml::Value::String(name), "{depth:?}");
        }
    }

    #[test]
    fn the_lists_are_the_whole_of_each_setting() {
        // A variant added to either enum and not here would simply be
        // unreachable from a command line, silently.
        assert_eq!(LockDownRule::value_variants().len(), 3);
        assert_eq!(ColorDepth::value_variants().len(), 5);
    }
}
