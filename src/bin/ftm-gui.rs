//! Falling Tetromino Manager — the window front-end's entry point.
//!
//! The window's answer to `src/bin/ftm.rs`, and shorter than it, because there
//! is no terminal to put into raw mode and none to restore (`GUI.md`): the
//! arguments, the four capabilities of `FRONTEND.md` F1-F4, the config, the
//! window, and §16's warnings on stderr once it has closed.
//!
//! §8.3's panic hook is not here, deliberately. It exists to restore a
//! terminal *before* the backtrace is printed, and a window has nothing in
//! that state to give back; `EGUI.md` G6 adds the web analogue —
//! `console_error_panic_hook` — which is a legibility measure rather than a
//! repair.

#![forbid(unsafe_code)]

use anyhow::{Result, anyhow};
use clap::Parser;
use ftm::gui;
use ftm::gui::cli::Cli;
use ftm::native::{self, Files};
use ftm::shell::config::{self, Startup};
use ftm::shell::host::Host;
use ftm::shell::input::InputMode;
use ftm::shell::session::Session;

fn main() -> Result<()> {
    // §6.1: the command line over the config file over the defaults. §6.2's
    // location is this front-end's answer and no further up (§3.1) — what the
    // shell is handed is a `Storage` and §6.4's flags, decoded.
    let cli = Cli::parse();
    let mut files = Files::new(cli.config.clone());
    let mut startup = Startup::resolve(&cli.overrides(), &files, native::seed);

    // §8.2 has no second path here: `egui` reports a true release for every
    // press, so this front-end is unconditionally the enhanced case and the
    // hold timeout is never reached (`GUI.md` §G2).
    let result = {
        let mut session = Session::new(
            &startup,
            InputMode::Enhanced,
            Host::new(&mut files, native::seed, native::today),
        );
        // The window borrows the session and hands it back, so the Options
        // panel's edits and §16's warnings survive it (§13.5, §16).
        let outcome = gui::run(&mut session);
        session.finish(&mut startup);
        // `eframe::Error` is not `std::error::Error + Send + Sync` in every
        // build, so it is reported by its message: the only failure this can
        // report is a window that would not open, and §16 wants that on
        // stderr with a non-zero exit.
        outcome.map_err(|error| anyhow!("could not open a window: {error}"))
    };

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
    // §16: the warnings go to stderr once the window has gone, where the
    // terminal front-end prints them after teardown.
    for warning in &startup.warnings {
        eprintln!("ftm-gui: {warning}");
    }
    result
}
