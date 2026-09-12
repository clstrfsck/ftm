//! §6.4's grammar as a URL query string, for the build with no argv
//! (`GUI.md` §G8.4, §G8.11).
//!
//! A browser tab has no command line. What it has is the page's URL, so §6.4's
//! flags become query parameters **with the same names** — `?seed=42`,
//! `?no-hold`, `?lock-down=classic` — and cross into the shell as the same
//! [`Overrides`] that `clap` fills in natively. The grammar is the front-end's
//! and the meaning is the shell's (§6.1), and the test at the bottom of this
//! file is what says the two grammars ask for the same thing.
//!
//! Two of the native flags have no parameter here, and §G8.11 says why:
//! `--config` names a path and a tab has one storage key (§G8.3), and
//! `--print-config` is a thing to do instead of playing, which a page cannot
//! be asked to do. `--fullscreen` is here but does nothing a tab can honour;
//! it is accepted rather than warned about because a link shared from a
//! desktop should not scold whoever opens it.
//!
//! It is plain Rust over a `&str` rather than `web_sys::UrlSearchParams`, so it
//! compiles — and is tested — on every target and not only in a browser.

use crate::shell::config::{LockDownRule, Overrides};

/// Every parameter this build understands, as `GUI.md` §G8.11 lists them.
///
/// Each is a §6.4 flag with its `--` taken off. Spelled out rather than
/// gathered from the match below, so that a parameter added to one and not the
/// other is a test failure rather than a silently dead link.
pub const PARAMETERS: [&str; 12] = [
    "preview",
    "level",
    "no-ghost",
    "hold",
    "no-hold",
    "rot180",
    "no-rot180",
    "lock-down",
    "scale",
    "fullscreen",
    "no-fullscreen",
    "seed",
];

/// What a query string asked for, and §16's warnings about what it asked for
/// badly.
///
/// A leading `?` is optional, which is the difference between
/// `location.search` and the part after it. Three rules differ from the command
/// line, all because a URL is not only the game's (§G8.4):
///
/// * **A parameter the game does not know is ignored, silently.** A link picks
///   up a campaign tag or a share sheet's marker on its travels, and a warning
///   about each would be noise the player can do nothing about.
/// * **A value that does not parse is a warning, never an abort** (§16). `clap`
///   refuses a bad `--seed` and exits, which a tab cannot do; the run goes ahead
///   without that setting and the console says why.
/// * **A parameter given twice takes its last value**, as a later flag
///   overrides an earlier one in §6.4's paired flags.
pub fn overrides(query: &str) -> (Overrides, Vec<String>) {
    let mut overrides = Overrides::default();
    let mut warnings = Vec::new();
    let query = query.strip_prefix('?').unwrap_or(query);
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = match pair.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            // `?no-hold` with no `=` at all: a flag written as a flag.
            None => (pair, None),
        };
        let mut warn = |what: &str| {
            warnings.push(format!(
                "?{name}={} is not {what}; ignoring it",
                value.unwrap_or(""),
            ));
        };
        match name {
            "preview" => number(value, &mut warn, "a preview count", &mut overrides.preview),
            "level" => number(value, &mut warn, "a starting level", &mut overrides.level),
            "scale" => number(
                value,
                &mut warn,
                "a scale in per cent",
                &mut overrides.scale,
            ),
            "seed" => number(value, &mut warn, "a whole number", &mut overrides.seed),
            "lock-down" => match lock_down(value.unwrap_or("")) {
                Some(rule) => overrides.lock_down = Some(rule),
                None => warn("extended, infinite or classic"),
            },
            // §6.4's one-sided flag, and its four paired ones. The `no-` half
            // is a parameter of its own because §G8.4 keeps the flag's name.
            "no-ghost" => flag(value, &mut warn, &mut overrides.ghost, false),
            "hold" => flag(value, &mut warn, &mut overrides.hold, true),
            "no-hold" => flag(value, &mut warn, &mut overrides.hold, false),
            "rot180" => flag(value, &mut warn, &mut overrides.rot180, true),
            "no-rot180" => flag(value, &mut warn, &mut overrides.rot180, false),
            "fullscreen" => flag(value, &mut warn, &mut overrides.fullscreen, true),
            "no-fullscreen" => flag(value, &mut warn, &mut overrides.fullscreen, false),
            // Silently. See above.
            _ => {}
        }
    }
    (overrides, warnings)
}

/// A parameter that carries a number (§6.3's ranges are the shell's, and
/// `Startup::resolve` clamps and reports them exactly as it does the file's).
fn number<T: std::str::FromStr>(
    value: Option<&str>,
    warn: &mut impl FnMut(&str),
    what: &str,
    into: &mut Option<T>,
) {
    match value.unwrap_or("").parse() {
        Ok(number) => *into = Some(number),
        // A bad value leaves the earlier good one alone: `?seed=1&seed=x` is
        // seeded 1, because the second says nothing usable rather than
        // retracting the first.
        Err(_) => warn(what),
    }
}

/// A parameter that is a flag: `?no-hold`, `?no-hold=`, or `?no-hold=1`.
///
/// A URL is written by hand and pasted by people, so the three spellings a
/// person would reach for all mean the same thing — and `=0` means "never
/// mind", which is what someone editing a shared link expects it to.
fn flag(value: Option<&str>, warn: &mut impl FnMut(&str), into: &mut Option<bool>, sets: bool) {
    match value.unwrap_or("") {
        "" | "1" | "true" | "on" | "yes" => *into = Some(sets),
        "0" | "false" | "off" | "no" => {}
        _ => warn("on or off"),
    }
}

/// `--lock-down`'s three values, as §6.3's table spells them.
///
/// `clap`'s `ValueEnum` does this natively and lives in `src/argv.rs`, which a
/// browser tab is not compiled with. The words are the same words, and the
/// cross-check against the native grammar above is what says so — written as a
/// match rather than a `FromStr` on the shell's type, because a trait impl that
/// exists only when a front-end feature is on is a surprise waiting for the
/// third front-end.
fn lock_down(name: &str) -> Option<LockDownRule> {
    match name {
        "extended" => Some(LockDownRule::Extended),
        "infinite" => Some(LockDownRule::Infinite),
        "classic" => Some(LockDownRule::Classic),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_query_asks_for_nothing() {
        // §6.1: what is not written leaves the file alone.
        for query in ["", "?", "?&&"] {
            assert_eq!(
                overrides(query),
                (Overrides::default(), Vec::new()),
                "{query:?}"
            );
        }
    }

    #[test]
    fn a_seed_is_read_with_or_without_the_question_mark() {
        for query in ["?seed=42", "seed=42"] {
            let (asked, warnings) = overrides(query);
            assert_eq!(asked.seed, Some(42), "{query:?}");
            assert!(warnings.is_empty(), "{warnings:?}");
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_query_and_the_command_line_ask_for_the_same_thing() {
        // `GUI.md` §G8.4: "§6.4's flags, with the same names and the same
        // precedence". The strongest form of that claim, flag by flag: the two
        // grammars must produce byte-identical `Overrides`, because everything
        // downstream of them is one shell.
        use crate::gui::cli::Cli;
        use clap::Parser as _;
        let same = |query: &str, argv: &[&str]| {
            let mut full = vec!["ftm-gui"];
            full.extend_from_slice(argv);
            let native = Cli::parse_from(full).overrides();
            let (web, warnings) = overrides(query);
            assert!(warnings.is_empty(), "{query}: {warnings:?}");
            assert_eq!(web, native, "{query} against {argv:?}");
        };
        same("?seed=42", &["--seed", "42"]);
        same("?preview=3", &["--preview", "3"]);
        same("?level=9", &["--level", "9"]);
        same("?no-ghost", &["--no-ghost"]);
        same("?hold", &["--hold"]);
        same("?no-hold", &["--no-hold"]);
        same("?rot180", &["--rot180"]);
        same("?no-rot180", &["--no-rot180"]);
        same("?lock-down=classic", &["--lock-down", "classic"]);
        same("?lock-down=infinite", &["--lock-down", "infinite"]);
        same("?lock-down=extended", &["--lock-down", "extended"]);
        same("?scale=150", &["--scale", "150"]);
        same("?fullscreen", &["--fullscreen"]);
        same("?no-fullscreen", &["--no-fullscreen"]);
        // ...and all of them at once, in a URL someone would actually paste.
        same(
            "?seed=42&preview=3&level=9&no-ghost&no-hold&no-rot180\
             &lock-down=classic&scale=150&fullscreen",
            &[
                "--seed",
                "42",
                "--preview",
                "3",
                "--level",
                "9",
                "--no-ghost",
                "--no-hold",
                "--no-rot180",
                "--lock-down",
                "classic",
                "--scale",
                "150",
                "--fullscreen",
            ],
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn every_documented_parameter_is_a_flag_of_the_native_build() {
        // §G8.11's two columns, checked against each other. `--config` and
        // `--print-config` are the deliberate difference and are named here so
        // that adding a third goes through this test.
        use crate::gui::cli::FLAGS;
        let web: Vec<String> = PARAMETERS.iter().map(|p| format!("--{p}")).collect();
        for flag in FLAGS {
            let expected = flag != "--config";
            assert_eq!(
                web.contains(&flag.to_string()),
                expected,
                "{flag} and its query parameter disagree",
            );
        }
        for parameter in &web {
            assert!(
                FLAGS.contains(&parameter.as_str()),
                "?{parameter} is not a flag of the native build",
            );
        }
    }

    #[test]
    fn a_parameter_the_game_does_not_know_is_ignored_without_a_word() {
        let (asked, warnings) = overrides("?utm_source=feed&seed=7&fbclid=x");
        assert_eq!(asked.seed, Some(7));
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_bad_value_is_a_warning_and_the_run_goes_ahead_without_it() {
        // §16: a tab cannot exit the way `clap` does on a bad flag.
        for bad in [
            "?seed=abc",
            "?seed=",
            "?seed",
            "?seed=-1",
            "?seed=18446744073709551616",
        ] {
            let (asked, warnings) = overrides(bad);
            assert_eq!(asked.seed, None, "{bad:?}");
            assert_eq!(warnings.len(), 1, "{bad:?}: {warnings:?}");
            assert!(warnings[0].starts_with("?seed="), "{warnings:?}");
        }
        for (bad, what) in [
            ("?preview=lots", "a preview count"),
            ("?level=", "a starting level"),
            ("?scale=big", "a scale in per cent"),
            ("?lock-down=eventually", "extended, infinite or classic"),
            ("?no-hold=maybe", "on or off"),
        ] {
            let (asked, warnings) = overrides(bad);
            assert_eq!(asked, Overrides::default(), "{bad:?}");
            assert_eq!(warnings.len(), 1, "{bad:?}: {warnings:?}");
            assert!(warnings[0].contains(what), "{warnings:?}");
        }
    }

    #[test]
    fn a_flag_is_accepted_in_the_three_spellings_a_person_would_write() {
        for query in ["?no-hold", "?no-hold=", "?no-hold=1", "?no-hold=true"] {
            let (asked, warnings) = overrides(query);
            assert_eq!(asked.hold, Some(false), "{query:?}");
            assert!(warnings.is_empty(), "{warnings:?}");
        }
        // ...and switched off, which leaves the file's own answer alone.
        for query in ["?no-hold=0", "?no-hold=false", "?no-hold=off"] {
            let (asked, warnings) = overrides(query);
            assert_eq!(asked.hold, None, "{query:?}");
            assert!(warnings.is_empty(), "{warnings:?}");
        }
    }

    #[test]
    fn the_last_of_two_values_is_the_one_that_counts() {
        assert_eq!(overrides("?seed=1&seed=2").0.seed, Some(2));
        // ...and a bad later one does not undo a good earlier one.
        let (asked, warnings) = overrides("?seed=1&seed=x");
        assert_eq!(asked.seed, Some(1));
        assert_eq!(warnings.len(), 1);
        // The paired halves override each other the way §6.4's flags do.
        assert_eq!(overrides("?hold&no-hold").0.hold, Some(false));
        assert_eq!(overrides("?no-hold&hold").0.hold, Some(true));
    }

    #[test]
    fn a_query_obeys_section_6_1s_precedence_over_the_stored_config() {
        // §6.1, on the web build: the query over the file over the defaults,
        // with the same clamping and the same warnings as the flags it mirrors.
        use crate::shell::config::{Startup, document, range};
        use crate::shell::storage::{Memory, Slot};
        let mut stored = crate::shell::config::ConfigFile::default();
        stored.gameplay.preview_count = 2;
        stored.gameplay.start_level = 4;
        let storage = Memory::holding(Slot::Config, &document(&stored));

        let (asked, warnings) = overrides("?preview=6");
        assert!(warnings.is_empty(), "{warnings:?}");
        let startup = Startup::resolve(&asked, &storage, || 1);
        assert_eq!(startup.file.gameplay.preview_count, 6, "the query wins");
        assert_eq!(
            startup.file.gameplay.start_level, 4,
            "the file still counts"
        );
        assert!(startup.warnings.is_empty(), "{:?}", startup.warnings);

        // Out of range: clamped and reported, as a flag would be (§6.3).
        let (asked, _) = overrides("?preview=99");
        let startup = Startup::resolve(&asked, &storage, || 1);
        assert_eq!(
            startup.file.gameplay.preview_count,
            *range::PREVIEW_COUNT.end()
        );
        assert_eq!(startup.warnings.len(), 1, "{:?}", startup.warnings);
        // §6.1: and the query is never written back — it is one run's.
        assert_eq!(startup.on_disk.gameplay.preview_count, 2);
    }

    #[test]
    fn a_seeded_run_is_not_recorded_however_the_seed_was_written() {
        // §14, §6.4: the same rule reached from a third build.
        use crate::shell::config::Startup;
        use crate::shell::storage::Memory;
        let storage = Memory::new();
        let startup = Startup::resolve(&overrides("?seed=42").0, &storage, || 7);
        assert!(startup.seeded);
        assert_eq!(startup.seed, 42);
        let startup = Startup::resolve(&overrides("").0, &storage, || 7);
        assert!(!startup.seeded);
        assert_eq!(startup.seed, 7, "F3's entropy, when the URL gave none");
    }
}
