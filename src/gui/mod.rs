//! The window front-end, native and — from `EGUI.md` G6 — in a browser tab.
//!
//! `GUI.md` is normative for everything here. It is one front-end with two
//! hosts rather than two front-ends: `app.rs`, `keys.rs` and `paint.rs` are
//! shared verbatim, and only `host_native.rs` (and, at G6, `host_web.rs`)
//! knows which platform is underneath.
//!
//! What it draws today is `EGUI.md` G5's vertical slice: a window, a well, a
//! falling piece and §10.1's keys. §12.4's information, §12.6's overlays,
//! §12.5's animations and §13's attract screen arrive at G7-G11, each over the
//! same [`Round`](crate::shell::round::Round) this one already pumps.
//!
//! Three things this front-end inherits from nowhere, all of them recorded in
//! `GUI.md`'s "What does not carry over": there are no colour depths and no
//! `NO_COLOR`, because those are a terminal's; there is no §8.2 legacy key
//! path, because `egui` always reports a release; and there is no §8.3
//! teardown, because there is no terminal to restore — a window that fails to
//! open says so on stderr and exits non-zero.

pub mod app;
pub mod cli;
pub mod host_native;
pub mod keys;
pub mod paint;

use crate::gui::app::Gui;
use crate::gui::host_native::Clock;
use crate::shell::session::Session;

/// Open the window and play, returning when it closes.
///
/// The session is borrowed for the length of the run and handed back intact,
/// which is what lets `main` do §6.2's first-clean-exit write and print §16's
/// warnings afterwards — the window's equivalent of §8.3's "after teardown".
pub fn run(session: &mut Session<'_>) -> eframe::Result {
    // F1: the clock starts here, and it is the front-end's, so every `Stamp`
    // in the run is measured from one origin.
    let clock = Clock::new();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size(app::INITIAL_SIZE)
            .with_app_id("ftm"),
        ..Default::default()
    };
    // `run_native` takes a non-`'static` app, which is what lets `Gui` borrow
    // the session rather than swallowing it.
    eframe::run_native(
        TITLE,
        options,
        Box::new(move |_cc| Ok(Box::new(Gui::new(session, clock)))),
    )
}

/// §1.3: the window title is one of the places the trademark rules reach, so
/// it is the game's own name and nothing else's.
const TITLE: &str = "Falling Tetromino Manager";
