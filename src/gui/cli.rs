//! `ftm-gui`'s command line (§6.4, `GUI.md` §G1, §G8.11).
//!
//! §6.4's *grammar* belongs to a front-end and its *meaning* to the shell, so
//! what crosses the boundary here is the same
//! [`Overrides`](crate::shell::config::Overrides) the terminal front-end hands
//! over — and the same value `gui/query.rs` fills in from a URL when the build
//! has no argv at all.
//!
//! §6.4 splits the way §6.2 does. Everything shared is here; `--color` is not,
//! because §12.3's colour depths are a character grid's and mean nothing in a
//! window (`TUI.md` §6.3); and `--scale` and `--fullscreen` are here and not in
//! `tui/cli.rs`, because they are `GUI.md` §G8.10's table. `--print-config` *is*
//! shared: what it writes is the whole document, `[gui]` included, and a player
//! comparing what the two binaries resolved is exactly who asks for it.
//!
//! §G8.11 lists which flag exists in which build, and the list is a test — the
//! one below, and its counterpart in `gui/query.rs`, which checks that a flag
//! and its query parameter ask for the same thing.

use std::path::PathBuf;

use clap::Parser;

use crate::shell::config::{LockDownRule, Overrides};

/// §6.4, less the two flags a window has no answer for.
#[derive(Debug, Default, Parser)]
#[command(
    name = "ftm-gui",
    version,
    about = "A guideline-conformant falling-block game, in a window",
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
    /// Interface scale, per cent [50-300]
    #[arg(long, value_name = "PERCENT")]
    pub scale: Option<u32>,
    /// Open full screen
    #[arg(long, overrides_with = "no_fullscreen")]
    fullscreen: bool,
    /// Open in a window
    #[arg(long, overrides_with = "fullscreen")]
    no_fullscreen: bool,
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
            // §12.3 is a terminal's; this build has no flag for it and never
            // overrides what the file says.
            color: None,
            scale: self.scale,
            fullscreen: Overrides::paired(self.fullscreen, self.no_fullscreen),
            seed: self.seed,
        }
    }
}

/// Every flag `ftm-gui` takes, as `GUI.md` §G8.11 lists them.
///
/// Spelled out rather than derived from the struct, because the point of the
/// list is to disagree loudly with the grammar if one of them is added on one
/// side only — and to be the same list §G8.11 prints.
#[cfg(test)]
pub const FLAGS: [&str; 13] = [
    "--preview",
    "--level",
    "--no-ghost",
    "--hold",
    "--no-hold",
    "--rot180",
    "--no-rot180",
    "--lock-down",
    "--scale",
    "--fullscreen",
    "--no-fullscreen",
    "--seed",
    "--config",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::config::{ConfigFile, range};

    #[test]
    fn the_grammar_parses_and_nothing_is_asked_for_by_default() {
        // §6.1: a flag that is not written leaves the file alone, so a bare
        // command line must override nothing at all.
        let cli = Cli::parse_from(["ftm-gui"]);
        assert_eq!(cli.overrides(), Overrides::default());

        let cli = Cli::parse_from(["ftm-gui", "--seed", "42", "--config", "/tmp/t.toml"]);
        assert_eq!(cli.overrides().seed, Some(42));
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/tmp/t.toml"))
        );
    }

    #[test]
    fn the_window_takes_every_shared_flag_and_the_two_that_are_its_own() {
        // §G8.11's list, as a grammar check: `try_parse_from` fails on a name
        // that is not there, so this is a transcription of the table.
        let all = Cli::parse_from([
            "ftm-gui",
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
        assert_eq!(file.gui.scale_percent, 150);
        assert!(file.gui.fullscreen);
        assert_eq!(all.overrides().seed, Some(42));
        assert!(all.print_config);
        // ...and it leaves §12.3 alone, whatever the file said.
        assert_eq!(file.display, ConfigFile::default().display);
    }

    #[test]
    fn a_terminals_flags_are_not_this_binarys() {
        // §G8.11: `--color` is §12.3's and there is no colour depth in a
        // window. A flag that quietly appeared here would be a setting the
        // player could ask for and never see applied.
        assert!(Cli::try_parse_from(["ftm-gui", "--color", "256"]).is_err());
        assert!(Cli::try_parse_from(["ftm-gui", "--colour", "256"]).is_err());
    }

    #[test]
    fn every_flag_in_the_documented_list_is_in_the_grammar() {
        use clap::CommandFactory as _;
        let command = Cli::command();
        let longs: Vec<String> = command
            .get_arguments()
            .filter_map(|arg| arg.get_long().map(|l| format!("--{l}")))
            .collect();
        for flag in FLAGS {
            assert!(longs.contains(&flag.to_string()), "{flag} is not a flag");
        }
        // The only extra is the one flag that is an action rather than a
        // setting, and so has no `Overrides` field to appear in. (`--help` and
        // `--version` are clap's and are not in `get_arguments` here.)
        let extra: Vec<&String> = longs
            .iter()
            .filter(|long| !FLAGS.contains(&long.as_str()))
            .collect();
        assert_eq!(
            extra,
            ["--print-config"],
            "a flag exists that §G8.11 does not list",
        );
    }

    #[test]
    fn full_screen_is_a_paired_flag_like_the_others() {
        // §6.4: "each paired flag overrides the corresponding config key in
        // both directions", which is what lets a `fullscreen = true` file be
        // opened in a window for one run.
        let mut file = ConfigFile::default();
        file.gui.fullscreen = true;
        Cli::parse_from(["ftm-gui", "--no-fullscreen"])
            .overrides()
            .apply(&mut file);
        assert!(!file.gui.fullscreen);
        Cli::parse_from(["ftm-gui", "--fullscreen"])
            .overrides()
            .apply(&mut file);
        assert!(file.gui.fullscreen);
    }

    #[test]
    fn a_scale_outside_the_range_is_clamped_and_reported_like_a_file_value() {
        // §6.3: the ranges are the setting's, not the file's, so a flag that
        // asks for something outside one is clamped and said so — which is
        // `Startup::resolve`'s validate pass over the flagged copy.
        use crate::shell::config::Startup;
        use crate::shell::storage::Memory;
        let storage = Memory::new();
        let overrides = Cli::parse_from(["ftm-gui", "--scale", "9000"]).overrides();
        let startup = Startup::resolve(&overrides, &storage, || 1);
        assert_eq!(startup.file.gui.scale_percent, *range::SCALE_PERCENT.end());
        assert_eq!(startup.warnings.len(), 1, "{:?}", startup.warnings);
        // §6.1: and it is never written back — a flag is for one run.
        assert_eq!(
            startup.on_disk.gui.scale_percent,
            ConfigFile::default().gui.scale_percent
        );
    }

    #[test]
    fn the_command_line_is_well_formed() {
        // clap's own audit of the derive: duplicate long flags and the like.
        use clap::CommandFactory as _;
        Cli::command().debug_assert();
    }
}
