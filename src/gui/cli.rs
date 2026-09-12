//! `ftm-gui`'s command line (§6.4, `GUI.md` §G1).
//!
//! §6.4's *grammar* belongs to a front-end and its *meaning* to the shell, so
//! what crosses the boundary here is the same
//! [`Overrides`](crate::shell::config::Overrides) the terminal front-end hands
//! over — and the same value a URL query string will fill in when the web
//! build has no argv at all (`EGUI-PLAN.md` G12).
//!
//! **This is the vertical slice's half of §6.4, not the whole of it.** Two
//! flags, chosen because they are what makes a slice checkable: `--seed`,
//! which makes a run reproducible so that the same game can be played on a 60
//! Hz display, a 144 Hz display and in a browser tab and compared; and
//! `--config`, which keeps a test off the player's own file. `EGUI-PLAN.md`
//! G12 fills in the rest together with the query-string form, at which point
//! §6.4's tables are shared with `tui/cli.rs` rather than split between them.
//! What is **not** here is deliberate: `--color` is §12.3's and means nothing
//! off a terminal (`GUI.md`), and `--print-config` is §8.1's answer to a
//! question a window does not ask.

use std::path::PathBuf;

use clap::Parser;

use crate::shell::config::Overrides;

/// §6.4, as much of it as the G5 slice defines.
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
    /// Seed the randomiser (implies no high-score recording)
    #[arg(long, value_name = "N")]
    pub seed: Option<u64>,
    /// Use an alternative config file
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
}

impl Cli {
    /// What the shell is told (§6.1 step 3).
    pub fn overrides(&self) -> Overrides {
        Overrides {
            seed: self.seed,
            ..Overrides::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_command_line_is_well_formed() {
        // clap's own audit of the derive: duplicate long flags and the like.
        use clap::CommandFactory as _;
        Cli::command().debug_assert();
    }
}
