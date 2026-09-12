//! §6.4's command line, as `clap` sees it.
//!
//! The *grammar* of a command line is a front-end's: a browser tab has no argv
//! and reads §6.4's flags off the URL instead (`EGUI-PLAN.md` G12), so `clap`
//! is a front-end dependency and what crosses into the shell is
//! [`Overrides`](crate::shell::config::Overrides) — the settings one run asked
//! for, with nothing left of how they were written.
//!
//! Two of the flags never reach the shell at all. `--config` names a *path*,
//! and §6.2 makes where the bytes live the front-end's question; `--print-config`
//! is a thing to do instead of playing, and §8.1 does it before the terminal is
//! ever opened.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::shell::config::{ColorDepth, LockDownRule, Overrides};

/// The §6.4 command line.
///
/// Every flag here overrides the config file for one run and is never written
/// back (§6.1): the file is the player's, and a flag is an experiment.
#[derive(Debug, Default, Parser)]
#[command(
    name = "ftm",
    version,
    about = "A guideline-conformant falling-block game for the terminal",
    // The struct's doc comment explains the type, not the program; without
    // this clap would print it as the long help.
    long_about = None,
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Pieces shown in the preview window [1-6]
    #[arg(long, value_name = "N")]
    pub preview: Option<u8>,
    /// Starting level [1-15]
    #[arg(long, value_name = "N")]
    pub level: Option<u32>,
    /// Disable the ghost piece
    #[arg(long)]
    pub no_ghost: bool,
    /// Enable the hold mechanic
    #[arg(long, overrides_with = "no_hold")]
    hold: bool,
    /// Disable the hold mechanic
    #[arg(long, overrides_with = "hold")]
    no_hold: bool,
    /// Enable 180-degree rotation
    #[arg(long, overrides_with = "no_rot180")]
    rot180: bool,
    /// Disable 180-degree rotation
    #[arg(long, overrides_with = "rot180")]
    no_rot180: bool,
    /// Lock-down rule
    #[arg(long, value_name = "RULE")]
    pub lock_down: Option<LockDownRule>,
    /// Colour depth
    #[arg(long = "color", value_name = "MODE")]
    pub color: Option<ColorDepth>,
    /// Seed the randomiser (implies no high-score recording)
    #[arg(long, value_name = "N")]
    pub seed: Option<u64>,
    /// Use an alternative config file
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
    /// Write the effective config to stdout and exit
    #[arg(long)]
    pub print_config: bool,
}

impl Cli {
    /// What the shell is told (§6.1 step 3).
    pub fn overrides(&self) -> Overrides {
        Overrides {
            preview: self.preview,
            level: self.level,
            // §6.4 gives the ghost only one half, so the flag's absence means
            // "leave the file alone" rather than "on".
            ghost: self.no_ghost.then_some(false),
            hold: Overrides::paired(self.hold, self.no_hold),
            rot180: Overrides::paired(self.rot180, self.no_rot180),
            lock_down: self.lock_down,
            color: self.color,
            seed: self.seed,
        }
    }
}

// `ValueEnum` is written out rather than derived, because the two types are the
// shell's and `clap` is not: `--lock-down` is a §6.4 spelling of a §6.3 value,
// and the spelling is the front-end's half. Both lists are the §6.3 tables,
// which is also what `config::document` writes into the commented file.

impl ValueEnum for LockDownRule {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            LockDownRule::Extended,
            LockDownRule::Infinite,
            LockDownRule::Classic,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(clap::builder::PossibleValue::new(match self {
            LockDownRule::Extended => "extended",
            LockDownRule::Infinite => "infinite",
            LockDownRule::Classic => "classic",
        }))
    }
}

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

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(clap::builder::PossibleValue::new(match self {
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
    use crate::shell::config::{ConfigFile, GameplaySettings};
    use std::path::Path;

    fn cli(args: &[&str]) -> Cli {
        let mut argv = vec!["ftm"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv).expect("parses")
    }

    #[test]
    fn the_command_line_matches_the_spec_synopsis() {
        // §6.4, flag by flag. `try_parse_from` fails on a name that is not
        // there, so this is a transcription check of the whole synopsis.
        let all = cli(&[
            "--preview",
            "3",
            "--level",
            "9",
            "--no-ghost",
            "--no-hold",
            "--no-rot180",
            "--lock-down",
            "classic",
            "--color",
            "256",
            "--seed",
            "42",
            "--config",
            "/tmp/x.toml",
            "--print-config",
        ]);
        let mut file = ConfigFile::default();
        all.overrides().apply(&mut file);
        assert_eq!(file.gameplay.preview_count, 3);
        assert_eq!(file.gameplay.start_level, 9);
        assert!(!file.gameplay.ghost_piece);
        assert!(!file.gameplay.hold_enabled);
        assert!(!file.gameplay.allow_180_rotation);
        assert_eq!(file.gameplay.lock_down, LockDownRule::Classic);
        assert_eq!(file.display.color_depth, ColorDepth::Ansi256);
        assert_eq!(all.overrides().seed, Some(42));
        assert_eq!(all.config.as_deref(), Some(Path::new("/tmp/x.toml")));
        assert!(all.print_config);
    }

    #[test]
    fn every_value_of_an_enumerated_flag_parses() {
        // The two hand-written `ValueEnum` impls, against §6.3's tables. A
        // spelling that drifted here would be rejected at the command line and
        // accepted in the file, which is exactly the sort of divergence §6.4's
        // synopsis exists to stop.
        for (name, expected) in [
            ("extended", LockDownRule::Extended),
            ("infinite", LockDownRule::Infinite),
            ("classic", LockDownRule::Classic),
        ] {
            assert_eq!(cli(&["--lock-down", name]).lock_down, Some(expected));
        }
        for (name, expected) in [
            ("auto", ColorDepth::Auto),
            ("truecolor", ColorDepth::Truecolor),
            ("256", ColorDepth::Ansi256),
            ("16", ColorDepth::Ansi16),
            ("mono", ColorDepth::Mono),
        ] {
            assert_eq!(cli(&["--color", name]).color, Some(expected));
        }
        assert!(Cli::try_parse_from(["ftm", "--color", "sepia"]).is_err());
    }

    #[test]
    fn a_paired_flag_overrides_the_file_in_both_directions() {
        // §6.4: "a setting turned off in the config file can still be turned on
        // for one run". A9 turns each of them both ways.
        let off = ConfigFile {
            gameplay: GameplaySettings {
                hold_enabled: false,
                allow_180_rotation: false,
                ..GameplaySettings::default()
            },
            ..ConfigFile::default()
        };
        let mut file = off.clone();
        cli(&["--hold", "--rot180"]).overrides().apply(&mut file);
        assert!(file.gameplay.hold_enabled);
        assert!(file.gameplay.allow_180_rotation);

        let mut file = ConfigFile::default();
        cli(&["--no-hold", "--no-rot180"])
            .overrides()
            .apply(&mut file);
        assert!(!file.gameplay.hold_enabled);
        assert!(!file.gameplay.allow_180_rotation);

        // Neither half given leaves the file's own answer alone.
        let mut file = off.clone();
        cli(&[]).overrides().apply(&mut file);
        assert_eq!(file, off);
    }

    #[test]
    fn the_last_half_of_a_pair_written_is_the_one_that_counts() {
        for (args, expected) in [
            (["--hold", "--no-hold"], false),
            (["--no-hold", "--hold"], true),
        ] {
            let mut file = ConfigFile::default();
            cli(&args).overrides().apply(&mut file);
            assert_eq!(file.gameplay.hold_enabled, expected, "{args:?}");
        }
    }
}
