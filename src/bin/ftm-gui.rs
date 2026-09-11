//! Falling Tetromino Manager — the window front-end's entry point, natively
//! and in a browser tab.
//!
//! The window's answer to `src/bin/ftm.rs`, and shorter than it, because there
//! is no terminal to put into raw mode and none to restore (`GUI.md`): the
//! arguments, the four capabilities of `FRONTEND.md` F1-F4, the config, the
//! window, and §16's warnings once it has closed.
//!
//! One binary, two `main`s, because the two platforms differ in exactly the
//! things an entry point is made of: where the flags come from (argv, or the
//! page's query string), where F1-F4 come from (`src/native.rs`, or
//! `gui/host_web.rs`), and whether there is an "after" to print §16's warnings
//! in. `gui/app.rs` is the same code in both, and never asks.
//!
//! §8.3's panic hook is not here, deliberately. It exists to restore a
//! terminal *before* the backtrace is printed, and a window has nothing in
//! that state to give back. The web build's analogue is a legibility measure
//! rather than a repair, and `eframe::WebRunner` installs it (§G8.5).

#![forbid(unsafe_code)]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    desktop::main()
}

#[cfg(target_arch = "wasm32")]
fn main() {
    web::main();
}

#[cfg(not(target_arch = "wasm32"))]
mod desktop {
    use anyhow::{Result, anyhow};
    use clap::Parser;
    use ftm::gui;
    use ftm::gui::cli::Cli;
    use ftm::native::{self, Files};
    use ftm::shell::config::{self, Startup};
    use ftm::shell::host::Host;
    use ftm::shell::input::InputMode;
    use ftm::shell::session::Session;

    pub fn main() -> Result<()> {
        // §6.1: the command line over the config file over the defaults. §6.2's
        // location is this front-end's answer and no further up (§3.1) — what
        // the shell is handed is a `Storage` and §6.4's flags, decoded.
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
            // build, so it is reported by its message: the only failure this
            // can report is a window that would not open, and §16 wants that
            // on stderr with a non-zero exit.
            outcome.map_err(|error| anyhow!("could not open a window: {error}"))
        };

        // §6.2: the commented default file is written on the first clean exit,
        // and never over a file the player already has. What is written is the
        // file without the command line applied: a flag is for one run (§6.1).
        let write_defaults = !startup.existed && result.is_ok() && !startup.wrote_config;
        if write_defaults
            && let Err(error) = config::save(&mut files, &startup.on_disk)
            // §16: an unwritable config never aborts — and a store with nowhere
            // to keep it has already said so, at load.
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
}

#[cfg(target_arch = "wasm32")]
mod web {
    use ftm::gui;
    use ftm::gui::host_web::{self, Local};
    use ftm::gui::query;
    use ftm::shell::config::Startup;
    use ftm::shell::host::Host;
    use ftm::shell::input::InputMode;
    use ftm::shell::session::Session;

    pub fn main() {
        // §G8.5: the panic hook first, so that anything below which panics is
        // a message and a stack on the console rather than an `unreachable`
        // trap. It is the web's reading of §8.1's "installed before raw mode":
        // not a repair, since a tab has nothing to restore, but the same order
        // for the same reason.
        let runner = eframe::WebRunner::new();

        // §6.1, with the query string where argv would be (§G8.4).
        let (overrides, mut warnings) = query::overrides(&host_web::query());

        // A tab's run has no end to hand the store and the session back at: the
        // page is closed, not quit, and a reload discards the wasm instance
        // whole. So both live as long as the page, which is what leaking them
        // once, here, amounts to — and why `gui::start` takes them `'static`.
        let store: &'static mut Local = Box::leak(Box::new(Local::new()));
        let mut startup = Startup::resolve(&overrides, store, host_web::seed);

        // §16: what the query and the stored config had to say, now rather
        // than at an exit that will never come (§G8.6). The session reports
        // its own as they arise.
        warnings.append(&mut startup.warnings);
        host_web::report(&warnings);

        // §6.2's commented default document is *not* written: that happens on
        // the first clean exit, and a tab has none. The config slot is written
        // when the §13.5 Options panel saves, and not before (§G8.3).
        let session = Box::leak(Box::new(Session::new(
            &startup,
            InputMode::Enhanced,
            Host::new(store, host_web::seed, host_web::today),
        )));
        gui::start(runner, session);
    }
}
