//! §6.4's grammar as a URL query string, for the build with no argv
//! (`GUI.md` §G8.4).
//!
//! A browser tab has no command line. What it has is the page's URL, so §6.4's
//! flags become query parameters with the same names — `?seed=42` — and cross
//! into the shell as the same [`Overrides`] that `clap` fills in natively. The
//! grammar is the front-end's and the meaning is the shell's (§6.1).
//!
//! **This is `EGUI-PLAN.md` G6's half, and G6 needs one parameter**: `seed`,
//! which makes the web slice reproducible and comparable with the native one.
//! G12 adds the rest, together with §G8's list of which flag exists in which
//! build.
//!
//! It is plain Rust over a `&str` rather than `web_sys::UrlSearchParams`, so it
//! compiles — and is tested — on every target and not only in a browser.

use crate::shell::config::Overrides;

/// What a query string asked for, and §16's warnings about what it asked for
/// badly.
///
/// A leading `?` is optional, which is the difference between
/// `location.search` and the part after it. Two rules differ from the command
/// line, both because a URL is not only the game's:
///
/// * **A parameter the game does not know is ignored, silently.** A link picks
///   up a campaign tag or a share sheet's marker on its travels, and a warning
///   about each would be noise the player can do nothing about.
/// * **A value that does not parse is a warning, never an abort** (§16). `clap`
///   refuses a bad `--seed` and exits, which a tab cannot do; the run goes ahead
///   unseeded and the console says why.
///
/// A parameter given twice takes its last value, as a later flag overrides an
/// earlier one in §6.4's paired flags.
pub fn overrides(query: &str) -> (Overrides, Vec<String>) {
    let mut overrides = Overrides::default();
    let mut warnings = Vec::new();
    let query = query.strip_prefix('?').unwrap_or(query);
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if name == "seed" {
            match value.parse() {
                Ok(seed) => overrides.seed = Some(seed),
                Err(_) => warnings.push(format!(
                    "?seed={value} is not a whole number from 0 to {}; the run is not seeded",
                    u64::MAX,
                )),
            }
        }
    }
    (overrides, warnings)
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
        // `EGUI-PLAN.md` G6: "the same seed produces the same game as the
        // native binary". The core makes that true of a seed; this is what
        // makes it true of the *way the seed is written* in each build.
        use crate::gui::cli::Cli;
        use clap::Parser as _;
        let native = Cli::parse_from(["ftm-gui", "--seed", "42"]).overrides();
        assert_eq!(overrides("?seed=42").0, native);
    }

    #[test]
    fn a_parameter_the_game_does_not_know_is_ignored_without_a_word() {
        let (asked, warnings) = overrides("?utm_source=feed&seed=7&fbclid=x");
        assert_eq!(asked.seed, Some(7));
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_bad_seed_is_a_warning_and_the_run_goes_ahead_unseeded() {
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
    }

    #[test]
    fn the_last_of_two_seeds_is_the_one_that_counts() {
        assert_eq!(overrides("?seed=1&seed=2").0.seed, Some(2));
        // ...and a bad later one does not undo a good earlier one.
        let (asked, warnings) = overrides("?seed=1&seed=x");
        assert_eq!(asked.seed, Some(1));
        assert_eq!(warnings.len(), 1);
    }
}
