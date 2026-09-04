//! Termino — a guideline-conformant falling-block game for the terminal.
//!
//! Entry point: terminal setup/teardown and the panic hook (TERMINO.md §8).
//! Everything else lives in the library crate (`lib.rs`), so the integration
//! tests of §17.2 can drive the core headlessly.

#![forbid(unsafe_code)]

use std::io::{self, Stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    supports_keyboard_enhancement,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use termino::app;
use termino::config::ConfigFile;
use termino::input::InputMode;
use termino::ui::Tui;

/// Whether the enhancement flags of §8.2 were pushed and are still to be
/// popped. Teardown runs from three places — normal exit, error and the panic
/// hook — and has to be idempotent (§8.3).
static ENHANCED: AtomicBool = AtomicBool::new(false);

// TODO(stage 10): clap CLI (§6.4), the config file at the §6.2 path, and the
// warnings the loader collects along the way.

fn main() -> Result<()> {
    // §8.1 step 1: config first, before anything touches the terminal.
    let (rules, presentation) = ConfigFile::default().resolve();
    let mut warnings: Vec<String> = Vec::new();

    // §8.1 step 2: the panic hook goes in *before* raw mode, so that a crash
    // between here and the first frame still leaves a usable shell.
    install_panic_hook();

    let (mut terminal, mode) = setup()?;
    if mode == InputMode::Legacy {
        // §16: an unsupported enhancement degrades to a documented default and
        // adds a line to the warnings printed at exit.
        warnings.push(format!(
            "keyboard enhancement unsupported: using {} key handling, \
             so a held key expires 90 ms after its last repeat (§8.2)",
            mode.name(),
        ));
    }

    // TODO(stage 10): --seed, which also excludes the run from the high-score
    // table (§6.4, §14).
    let seed = rand::random::<u64>();
    let result = app::run(&mut terminal, rules, &presentation, mode, seed);

    restore();
    // §8.3: warnings are printed after teardown, on stderr, where they will
    // still be on screen once the alternate screen has gone.
    for warning in &warnings {
        eprintln!("termino: {warning}");
    }
    result
}

/// §8.1 steps 3-8, in that order.
fn setup() -> Result<(Tui, InputMode)> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, Hide)?;
    // Bracketed paste and mouse capture stay off (§8.1 step 6): neither is
    // enabled, which is how crossterm leaves them.

    // §8.2: query first, and push only what the terminal admits to supporting.
    let mode = match supports_keyboard_enhancement() {
        Ok(true) => {
            execute!(
                stdout,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                        | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                )
            )?;
            ENHANCED.store(true, Ordering::SeqCst);
            InputMode::Enhanced
        }
        _ => InputMode::Legacy,
    };

    let terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    Ok((terminal, mode))
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
