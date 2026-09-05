//! Falling Tetromino Manager — a guideline-conformant falling-block game
//! for the terminal.
//!
//! Entry point: terminal setup/teardown and the panic hook (FTM.md §8).
//! Everything else lives in the library crate (`lib.rs`), so the integration
//! tests of §17.2 can drive the core headlessly.

#![forbid(unsafe_code)]

use std::io::{self, Stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use clap::Parser;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    supports_keyboard_enhancement,
};
use ftm::app;
use ftm::config::{self, Cli, Startup};
use ftm::input::InputMode;
use ftm::ui::Tui;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// Whether the enhancement flags of §8.2 were pushed and are still to be
/// popped. Teardown runs from three places — normal exit, error and the panic
/// hook — and has to be idempotent (§8.3).
static ENHANCED: AtomicBool = AtomicBool::new(false);

fn main() -> Result<()> {
    // §8.1 step 1: arguments and config first, before anything touches the
    // terminal — including `--print-config`, which never opens one at all.
    let cli = Cli::parse();
    let mut startup = Startup::resolve(&cli, rand::random::<u64>);

    // §8.1 step 2: the panic hook goes in *before* raw mode, so that a crash
    // between here and the first frame still leaves a usable shell. It is also
    // before the §8.2 query below, which is the first thing that touches the
    // terminal at all.
    install_panic_hook();

    if cli.print_config {
        print!("{}", config::document(&startup.file));
        // §8.2: "the active mode must be reported by `--print-config`". As a
        // comment, because what this prints is a config file the player can
        // save, and the input mode is detected rather than configured.
        println!(
            "\n# Input mode: {} (§8.2). Detected at start-up, not a setting.",
            detect_mode().name(),
        );
        report(&startup.warnings);
        return Ok(());
    }

    let (mut terminal, mode) = setup()?;
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
    // way through (§13.5, §16), so `run` borrows the whole bundle.
    let result = app::run(&mut terminal, &mut startup, mode);

    restore();
    // §6.2: the commented default file is written on the first clean exit, and
    // never over a file the player already has. What is written is the file
    // without the command line applied: a flag is for one run (§6.1).
    let write_defaults = !startup.existed && result.is_ok() && !startup.wrote_config;
    if let (true, Some(path)) = (write_defaults, startup.path.as_deref())
        && let Err(error) = config::save(path, &startup.on_disk)
    {
        // §16: an unwritable config never aborts.
        startup
            .warnings
            .push(format!("{}: {error}", path.display()));
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

/// §8.1 steps 3-8, in that order.
fn setup() -> Result<(Tui, InputMode)> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, Hide)?;
    // Bracketed paste and mouse capture stay off (§8.1 step 6): neither is
    // enabled, which is how crossterm leaves them.

    // §8.2: query first, and push only what the terminal admits to supporting.
    let mode = detect_mode();
    if mode == InputMode::Enhanced {
        execute!(
            stdout,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
            )
        )?;
        ENHANCED.store(true, Ordering::SeqCst);
    }

    let terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    Ok((terminal, mode))
}

/// Which of §8.2's two paths this terminal can take.
///
/// A query, not a change: it pushes nothing and is safe to call from the
/// `--print-config` path, which never opens a screen. A terminal that does not
/// answer is timed out inside crossterm and treated as unsupported, which is
/// the same answer as declining.
fn detect_mode() -> InputMode {
    match supports_keyboard_enhancement() {
        Ok(true) => InputMode::Enhanced,
        _ => InputMode::Legacy,
    }
}

/// §8.3, in that order, and safe to run twice.
///
/// Every step ignores its own failure: teardown is the last thing that happens
/// on the way out of a panic, and a terminal that has already gone is not a
/// reason to abandon the steps after it.
fn restore() {
    let mut stdout: Stdout = io::stdout();
    if ENHANCED.swap(false, Ordering::SeqCst) {
        let _ = execute!(stdout, PopKeyboardEnhancementFlags);
    }
    let _ = execute!(stdout, Show, LeaveAlternateScreen);
    let _ = disable_raw_mode();
    let _ = stdout.flush();
}

/// Restore the terminal **before** the panic message is printed (§8.1 step 2),
/// so a bug produces a readable backtrace rather than a wrecked shell (§16).
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
}
