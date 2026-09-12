//! The window front-end, natively and in a browser tab.
//!
//! `GUI.md` is normative for everything here. It is one front-end with two
//! hosts rather than two front-ends: `app.rs`, `keys.rs`, `paint.rs` and
//! `query.rs` are shared verbatim, and only `host_native.rs` and `host_web.rs`
//! know which platform is underneath. Each is compiled for its own target only,
//! and [`host`] is whichever one this build has, under one name, so the shared
//! modules never say which.
//!
//! What it draws today is the playing screen of `GUI.md` §G4 over §G3's cell
//! metric (`EGUI-PLAN.md` G7) — [`layout`] says where, [`paint`] how, and
//! [`playfield`] what — served natively and as wasm. §12.6's overlays, §12.5's
//! animations and §13's attract screen arrive at G8-G11, each over the same
//! [`Round`](crate::shell::round::Round) this one already pumps.
//!
//! Three things this front-end inherits from nowhere, all of them recorded in
//! `GUI.md`'s "What does not carry over": there are no colour depths and no
//! `NO_COLOR`, because those are a terminal's; there is no §8.2 legacy key
//! path, because `egui` always reports a release; and there is no §8.3
//! teardown, because there is no terminal to restore — a window that fails to
//! open says so on stderr and exits non-zero, and a canvas that fails to start
//! says so on the console and on the page.

pub mod app;
pub mod attract;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
#[cfg(not(target_arch = "wasm32"))]
pub mod host_native;
#[cfg(target_arch = "wasm32")]
pub mod host_web;
pub mod keys;
pub mod layout;
pub mod overlays;
pub mod paint;
pub mod playfield;
pub mod query;

/// This build's four capabilities (`FRONTEND.md` F1-F4), under one name.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use self::host_native as host;
/// This build's four capabilities (`FRONTEND.md` F1-F4), under one name.
#[cfg(target_arch = "wasm32")]
pub(crate) use self::host_web as host;

use crate::gui::app::Gui;
use crate::gui::host::Clock;
#[cfg(target_arch = "wasm32")]
use crate::shell::menus::MenuChoice;
use crate::shell::menus::Setting;
use crate::shell::session::Session;

/// Open the window and play, returning when it closes.
///
/// The session is borrowed for the length of the run and handed back intact,
/// which is what lets `main` do §6.2's first-clean-exit write and print §16's
/// warnings afterwards — the window's equivalent of §8.3's "after teardown".
#[cfg(not(target_arch = "wasm32"))]
pub fn run(session: &mut Session<'_>) -> eframe::Result {
    // §G5: this front-end's Options panel offers the shared rows and not
    // §12.3's colour depth, which means nothing in a window. Said once, here,
    // because it is a property of the front-end rather than of a game.
    session.settings = &Setting::SHARED;
    // F1: the clock starts here, and it is the front-end's, so every `Stamp`
    // in the run is measured from one origin.
    let clock = Clock::new();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size(layout::INITIAL_SIZE)
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

/// The `id` of `index.html`'s canvas, which the game is drawn into (§G8.1).
#[cfg(target_arch = "wasm32")]
pub const CANVAS_ID: &str = "ftm";
/// The `id` of `index.html`'s status line: "Loading", until the game starts,
/// and why it could not if it does not (§G8.5, §16).
#[cfg(target_arch = "wasm32")]
pub const STATUS_ID: &str = "status";

/// Start the game in `index.html`'s canvas, and return at once (§G8.1).
///
/// There is no "when it closes" in a tab: the page is closed, not quit, and
/// takes the wasm instance with it. So the session is `'static` — the entry
/// point leaks it, once per page load — and nothing is handed back. §16's
/// warnings are reported as they arise instead (§G8.6).
///
/// `runner` is passed in rather than made here because making it is what
/// installs the panic hook, and the entry point wants that done before it
/// touches anything that could panic (§G8.5).
///
/// # Panics
///
/// If the page has no `<canvas id="ftm">`, which is a mistake in
/// `index.html` rather than anything a player can cause; the hook reports it.
#[cfg(target_arch = "wasm32")]
pub fn start(runner: eframe::WebRunner, session: &'static mut Session<'static>) {
    use wasm_bindgen::JsCast as _;

    // §G5, as natively: the panel offers the shared rows.
    session.settings = &Setting::SHARED;
    // §13.3, §G8.1: and the menu offers four items rather than five. A tab
    // cannot close itself — `window.close()` is refused to a page the player
    // opened — so **QUIT** here would be an item that did nothing. The shell
    // navigates this same list, so the cursor cannot reach what is not drawn.
    session.menu = &MenuChoice::NO_QUIT;

    let document = web_sys::window()
        .and_then(|window| window.document())
        .expect("the web build runs in a page");
    let canvas = document
        .get_element_by_id(CANVAS_ID)
        .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        .expect("index.html has a <canvas id=\"ftm\">");
    let status = document.get_element_by_id(STATUS_ID);
    // F1: as natively, the clock starts here.
    let clock = Clock::new();

    wasm_bindgen_futures::spawn_local(async move {
        let started = runner
            .start(
                canvas.clone(),
                eframe::WebOptions::default(),
                Box::new(move |_cc| Ok(Box::new(Gui::new(session, clock)))),
            )
            .await;
        match started {
            Ok(()) => {
                if let Some(status) = status {
                    status.remove();
                }
                // §G8.2: the canvas takes the keyboard on load. `eframe` gives
                // it a `tabindex` so that it *can* be focused, and calls
                // `preventDefault` on `Space`, `Tab` and the arrows only while
                // it is — so a canvas left unfocused is a page that scrolls
                // when the player hard-drops.
                let _ = canvas.focus();
            }
            // §16: the error that reaches the entry point is reported. There is
            // no exit status in a tab, so it goes to the console, and to the
            // page so that a player without one open is not left staring at
            // "Loading".
            Err(error) => {
                let line = format!("ftm-gui: could not start: {}", host::describe(&error));
                web_sys::console::error_1(&line.as_str().into());
                if let Some(status) = status {
                    status.set_text_content(Some(&line));
                }
            }
        }
    });
}

/// §1.3: the window title is one of the places the trademark rules reach, so
/// it is the game's own name and nothing else's. `index.html`'s `<title>` is
/// the same string, for the same reason.
#[cfg(not(target_arch = "wasm32"))]
const TITLE: &str = "Falling Tetromino Manager";
