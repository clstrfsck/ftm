//! The terminal's loop: §7's state machine and §15's two fixed-timestep loops.
//!
//! Two loops, because §15 specifies two: [`round`] runs a game at 60 Hz with an
//! accumulator (§15.2), and [`attract`] runs the front screen at 10 fps with
//! none (§15.3). What they drive lives in `shell/` — a [`Round`] and an
//! [`Attract`] over the [`Session`] both screens share — so this module is
//! what a terminal adds to them and nothing else: crossterm's event queue,
//! ratatui's `Size`, §12.2's interned glyphs, and §15.2 step 5's decision to
//! skip a frame that would look the same.
//!
//! That decision is the terminal's alone (`FRONTEND.md` F7). ratatui diffs
//! against a previous buffer, so an unchanged frame is cheap and skipping it
//! entirely is cheaper; an immediate-mode front-end rebuilds every repaint and
//! must not grow a comparison. [`Frame`] is that comparison, and what has to
//! be in it is `TUI.md` §12.4's business.
//!
//! The clock is the front-end's (F1): [`Clock`] is the only thing here that
//! reads one, and every `Stamp` below comes from it.

use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::layout::Size;

use crate::native::Clock;
use crate::shell::attract::Attract;
use crate::shell::config::{DisplaySettings, Startup};
use crate::shell::host::Host;
use crate::shell::input::InputMode;
use crate::shell::round::{Fps, FrameState, Round};
use crate::shell::session::{Next, Session};
use crate::tui::attract::{self, Background};
use crate::tui::keys::neutral;
use crate::tui::theme::{Glyphs, Theme};
use crate::tui::{self, Chrome, Hud, Tui};

/// Everything on the playing screen that the loop compares between frames
/// (§15.2 step 5).
///
/// The loop draws only when this has changed, so anything the screen comes to
/// show that is in none of these fields will not be redrawn — and, worse, will
/// not be *erased*. [`FrameState`] is the shared four-fifths of it;
/// `generation` inside it is the escape hatch for what genuinely cannot be a
/// field (the Options panel's values), and the size is the terminal's own
/// addition, because §12.1's message names it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Frame {
    state: FrameState,
    /// §12.1: the terminal's size, while it is below the minimum. `None` is
    /// the ordinary case of a terminal with room for the screen.
    size: Option<Size>,
}

/// The §7 state machine: attract, play, attract, until the player quits.
///
/// `startup` is borrowed rather than consumed because the §13.5 Options panel
/// edits the config in place and §16's warnings have to survive back to `main`,
/// which prints them after terminal teardown.
pub fn run(
    terminal: &mut Tui,
    startup: &mut Startup,
    mode: InputMode,
    host: Host<'_>,
) -> Result<()> {
    // `FRONTEND.md` F1: the clock starts here, and it is the front-end's, so
    // every `Stamp` below is measured from the same origin.
    let clock = Clock::new();
    // §12.2's glyphs are interned once, here: the theme is `Copy` and carries
    // them by reference for the life of the process, which is a concession to
    // threading one through every ratatui widget and belongs to no other
    // front-end.
    let glyphs = Glyphs::configured(&startup.file.display);
    let mut session = Session::new(startup, mode, host);
    let outcome = states(terminal, &mut session, &clock, glyphs);
    session.finish(startup);
    outcome
}

/// §7's transitions, as a loop over [`Next`].
fn states(
    terminal: &mut Tui,
    session: &mut Session<'_>,
    clock: &Clock,
    glyphs: Glyphs,
) -> Result<()> {
    let mut next = Next::Attract;
    loop {
        next = match next {
            Next::Attract => attract(terminal, session, clock, glyphs)?,
            Next::Play => round(terminal, session, clock, glyphs)?,
            Next::Quit => return Ok(()),
        };
    }
}

/// The attract screen's loop (§15.3): 10 fps, no accumulator, and a redraw only
/// when something moved.
fn attract(
    terminal: &mut Tui,
    session: &mut Session<'_>,
    clock: &Clock,
    glyphs: Glyphs,
) -> Result<Next> {
    let mut state = Attract::new(clock.now());
    // §13.4's drift is the terminal's own: it is positioned in the character
    // grid's cells, so it lives beside the drawing rather than in the state
    // machine, and its "did anything move" answer is folded in below.
    let mut background = Background::new(clock.now());
    let mut chrome = chrome_for(
        &session.config.display,
        glyphs,
        session.config.gameplay.hold_enabled,
    );
    let mut settings = session.generation();
    let mut dirty = true;
    // §8.4: the size is tracked from the resize events rather than asked for
    // every frame. It is the same answer, and it is the event that says the
    // whole frame is invalid.
    let mut area = terminal.size()?;
    loop {
        let now = clock.now();
        // Drain every event that is already waiting; reading one per frame
        // would leave fast typing lagging behind.
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                // §10.1's vocabulary is the shell's; crossterm's stops here
                // (`FRONTEND.md` F5).
                Event::Key(key) => {
                    let Some(key) = neutral(key) else {
                        continue;
                    };
                    dirty = true;
                    if let Some(next) = state.key(session, &key, now) {
                        return Ok(next);
                    }
                }
                // §8.4: a resize invalidates the whole frame.
                Event::Resize(width, height) => {
                    area = Size { width, height };
                    dirty = true;
                }
                _ => {}
            }
        }
        // §13.5: presentation takes effect the moment the panel is left. The
        // panel itself is the shell's, and this is how a front-end notices
        // that it changed something neither screen state shows.
        if settings != session.generation() {
            settings = session.generation();
            chrome = chrome_for(
                &session.config.display,
                glyphs,
                session.config.gameplay.hold_enabled,
            );
            dirty = true;
        }

        // §12.1: below the minimum the attract screen is replaced by the
        // message, and the drifting background is not stepped — there is
        // nowhere to draw it, and it would only make the frame look dirty.
        if !tui::fits(area) {
            if dirty {
                dirty = terminal
                    .draw(|frame| tui::too_small(frame, chrome.theme))
                    .is_err();
            }
            event::poll(state.deadline())?;
            continue;
        }

        let cells = (area.width / 2, area.height);
        // §13.4 is disabled in `mono` and when `show_debug` is on; asking the
        // theme rather than the config is what makes `NO_COLOR` count too.
        let animate = chrome.theme.depth() != crate::tui::theme::Depth::Mono
            && !session.config.display.show_debug;
        // Both halves are stepped, whatever the other answers: `advance` is
        // what moves the panel's cycle on, and short-circuiting it would stop
        // the clock rather than the redraw.
        let stepped = state.advance(now);
        let drifted = animate && background.step(now, cells);
        if stepped || drifted || dirty {
            let context = attract::Context {
                chrome: &chrome,
                config: &session.config,
                settings: session.settings,
                menu: session.menu,
                scores: &session.scores,
                recent: session.recent,
                mode: session.mode,
            };
            // §16: a frame lost to a write failure is simply lost; leaving
            // `dirty` set is what makes the next frame try again.
            dirty = terminal
                .draw(|frame| attract::draw(frame, &state, &background, &context))
                .is_err();
        }

        // §15.3: no accumulator, and a key wakes the loop early, so idle CPU
        // stays near zero while a keypress still lands within a frame.
        event::poll(state.deadline())?;
    }
}

/// One game (§15.2), from the first piece to the attract screen or a restart.
///
/// The seven numbered steps are still here and still in order; four of them
/// are now on the other side of a call (`EGUI-PLAN.md` G4). What stayed is
/// what a terminal does about them: draining crossterm's queue, deciding
/// §12.1's answer about the size it has, and skipping a draw that would change
/// nothing.
fn round(
    terminal: &mut Tui,
    session: &mut Session<'_>,
    clock: &Clock,
    glyphs: Glyphs,
) -> Result<Next> {
    let show_debug = session.config.display.show_debug;
    let mut game = Round::new(session, clock.now());
    let mut chrome = chrome_for(&session.config.display, glyphs, game.hold_enabled());
    let mut previous: Option<Frame> = None;
    let mut fps = Fps::new(clock.now());
    let mut settings = session.generation();
    // §8.4: tracked from the resize events, which are also what invalidates
    // the frame.
    let mut area = terminal.size()?;

    loop {
        let now = clock.now();

        // 2. Drain *every* event that is already waiting; reading one per frame
        //    would leave fast typing lagging behind, and §15.2 forbids holding
        //    one back until the next frame.
        let mut invalidated = false;
        let mut leaving = None;
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                // §10.1's vocabulary is the shell's; crossterm's stops here
                // (`FRONTEND.md` F5).
                Event::Key(key) => {
                    if let Some(key) = neutral(key)
                        && let Some(next) = game.key(session, &key, now)
                    {
                        leaving = Some(next);
                    }
                }
                // §8.4: a resize invalidates the whole frame.
                Event::Resize(width, height) => {
                    area = Size { width, height };
                    invalidated = true;
                }
                _ => {}
            }
        }
        // §8.4, §12.1: a terminal that has shrunk below the minimum forces a
        // game in progress into `Paused` *before* the screen goes, so the
        // player is never killed by a window resize — and so that no ticks run
        // behind a screen they cannot see. The minimum is this front-end's;
        // the consequence is the shell's (`FRONTEND.md` F6).
        let room = tui::fits(area);
        game.viewport(room);
        if let Some(next) = leaving {
            return Ok(next);
        }

        // 1, 3, 4, 5. The clock, DAS/ARR, whole ticks and the cosmetics — and
        //    §10.1's restart key, once it has been held for its second.
        if let Some(next) = game.advance(session, now) {
            return Ok(next);
        }

        // §13.5: leaving the Options panel applies the presentation half at
        // once. The rules half is deliberately not applied — the game keeps
        // what it started under, which is also the only answer that leaves the
        // run deterministic (§15.4).
        if settings != session.generation() {
            settings = session.generation();
            chrome = chrome_for(&session.config.display, glyphs, game.hold_enabled());
            invalidated = true;
        }

        // 6. Draw, but only when there is something new to look at: ratatui
        //    diffs against its previous buffer, so an unchanged frame is cheap,
        //    and skipping it entirely is cheaper. An animation in flight
        //    changes the screen without changing the view, so it counts as new.
        let mut state = game.frame(now);
        // `GameView::fall_progress` is a window's sub-cell offset (`GUI.md`
        // §G6.5) and a character cell has nowhere to put it, so §12.4 draws a
        // piece on its row and nothing else. Left in, it would change sixty
        // times a second and make every tick of every fall a redraw of a screen
        // that is byte-for-byte the one already there. Dropping a field the
        // terminal does not draw is exactly what F7 leaves to the front-end.
        state.view.fall_progress = 0;
        let frame = Frame {
            state,
            // §12.1's message names the size it has, so the size is on the
            // screen and belongs in the comparison like everything else.
            size: (!room).then_some(area),
        };
        // The strip's own figures change every frame, so with it on there is
        // always something new to look at (§12.4).
        if invalidated
            || show_debug
            || game.cosmetics().animating()
            || previous.as_ref() != Some(&frame)
        {
            let debug = show_debug.then(|| game.debug(fps.drew(now)));
            let hud = Hud {
                overlay: &frame.state.overlay,
                config: &session.config,
                settings: game.settings(),
                debug: debug.as_ref(),
                mode: session.mode,
                restart: frame.state.restart,
            };
            // §16: a frame lost to a write failure is simply lost. Leaving
            // `previous` behind is what makes the next frame try again.
            if terminal
                .draw(|f| tui::draw(f, &frame.state.view, &chrome, game.cosmetics(), &hud))
                .is_ok()
            {
                previous = Some(frame);
            }
        }

        // 7. Wait out the rest of the tick. Polling rather than sleeping means
        //    a key wakes the loop early, so input latency stays near one tick
        //    while idle CPU stays near zero. The deadline is advice and this
        //    front-end takes it literally (§15.2 step 6).
        event::poll(game.deadline(now))?;
    }
}

/// The presentation half of the `Chrome` (§12.4, §12.7).
///
/// `hold_enabled` is the caller's answer rather than the config's: §13.5
/// gives a running game the rules it started under, and the hold box's
/// presence is a rule.
fn chrome_for(display: &DisplaySettings, glyphs: Glyphs, hold_enabled: bool) -> Chrome {
    Chrome {
        theme: Theme::resolve(display.color_depth, glyphs),
        show_grid: display.show_grid,
        hold_enabled,
    }
}
