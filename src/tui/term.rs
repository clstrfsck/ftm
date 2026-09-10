//! Terminal setup, teardown and the panic hook (§8.1-§8.3).
//!
//! The three things that happen either side of the loop, and the one rule that
//! ties them together: teardown is **idempotent** and runs from three places —
//! a normal exit, an error, and the panic hook, which is installed *before*
//! raw mode so that a crash restores the terminal before it prints (§16).

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

use crate::shell::input::InputMode;
use crate::tui::Tui;

/// Whether the enhancement flags of §8.2 were pushed and are still to be
/// popped. Teardown runs from three places — normal exit, error and the panic
/// hook — and has to be idempotent (§8.3).
static ENHANCED: AtomicBool = AtomicBool::new(false);

/// §8.1 steps 3-8, in that order.
pub fn setup() -> Result<(Tui, InputMode)> {
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
pub fn detect_mode() -> InputMode {
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
pub fn restore() {
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
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
}
