//! Falling Tetromino Manager — the terminal front-end's entry point.
//!
//! Nothing but §8.1's opening steps and §8.3's closing ones: the arguments,
//! the four capabilities of `FRONTEND.md` F1-F4, the config, the panic hook,
//! the terminal, the loop, and §16's warnings on the way out. Everything else
//! lives behind `lib.rs`, so the integration tests of §17.2 can drive the core
//! headlessly.

#![forbid(unsafe_code)]

use anyhow::Result;
use clap::Parser;
use ftm::native::{self, Files};
use ftm::shell::config::{self, Startup};
use ftm::shell::host::Host;
use ftm::shell::input::InputMode;
use ftm::tui::cli::Cli;
use ftm::tui::{run, term};

fn main() -> Result<()> {
    // §8.1 step 1: arguments and config first, before anything touches the
    // terminal — including `--print-config`, which never opens one at all.
    // §6.2's location is this front-end's answer and no further up (§3.1):
    // what the shell is handed is a `Storage` and §6.4's flags, decoded.
    let cli = Cli::parse();
    let mut files = Files::new(cli.config.clone());
    let mut startup = Startup::resolve(&cli.overrides(), &files, native::seed);

    // §8.1 step 2: the panic hook goes in *before* raw mode, so that a crash
    // between here and the first frame still leaves a usable shell. It is also
    // before the §8.2 query below, which is the first thing that touches the
    // terminal at all.
    term::install_panic_hook();

    if cli.print_config {
        print!("{}", config::document(&startup.file));
        // §8.2: "the active mode must be reported by `--print-config`". As a
        // comment, because what this prints is a config file the player can
        // save, and the input mode is detected rather than configured.
        println!(
            "\n# Input mode: {} (§8.2). Detected at start-up, not a setting.",
            term::detect_mode().name(),
        );
        report(&startup.warnings);
        return Ok(());
    }

    let (mut terminal, mode) = term::setup()?;
    if mode == InputMode::Legacy {
        // §16: an unsupported enhancement degrades to a documented default and
        // adds a line to the warnings printed at exit.
        startup.warnings.push(format!(
            "keyboard enhancement unsupported: using {} key handling, \
             so a held key expires 90 ms after its last repeat (§8.2)",
            mode.name(),
        ));
    }

    // The Options panel may edit `startup.file` and add to its warnings on the
    // way through (§13.5, §16), so `run` borrows the whole bundle. The host is
    // moved in and dropped when the loop hands back, which is what lets the
    // §6.2 write below reach the same files again.
    let result = run::run(
        &mut terminal,
        &mut startup,
        mode,
        Host::new(&mut files, native::seed, native::today),
    );

    term::restore();
    // §6.2: the commented default file is written on the first clean exit, and
    // never over a file the player already has. What is written is the file
    // without the command line applied: a flag is for one run (§6.1).
    let write_defaults = !startup.existed && result.is_ok() && !startup.wrote_config;
    if write_defaults
        && let Err(error) = config::save(&mut files, &startup.on_disk)
        // §16: an unwritable config never aborts — and a store with nowhere to
        // keep it has already said so, at load.
        && let Some(line) = error.warning()
    {
        startup.warnings.push(line.to_string());
    }
    report(&startup.warnings);
    result
}

/// §8.3, §16: warnings go to stderr **after** teardown, where they will still
/// be on screen once the alternate screen has gone.
fn report(warnings: &[String]) {
    for warning in warnings {
        eprintln!("ftm: {warning}");
    }
}
