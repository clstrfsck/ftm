//! The `eframe` application: §15.2's seven steps, on the other side of a
//! callback (`GUI.md` §G1, `FRONTEND.md` F7).
//!
//! A terminal owns its loop and a window does not: the compositor calls the
//! application and the application asks to be called again. That is the whole
//! of the difference, and `EGUI.md` G4 is what made it a difference of a few
//! lines rather than of a rewrite — [`Round`] is §15.2's loop body as an
//! object, and this module is what a *window* adds to it: `egui`'s event
//! queue, a painter, and a repaint request.
//!
//! `eframe` 0.36 splits its callback in two, and the split lands exactly where
//! §15.2's does. [`App::logic`](eframe::App::logic) is steps 1-4 and 6-7 and is
//! called even while the window is hidden; [`App::ui`](eframe::App::ui) is step
//! 5 and is called only when there is something to paint into. So the pump
//! runs behind a hidden window and simply is not drawn, and a backgrounded tab
//! is the same case: `eframe` keeps calling `logic` there on a timer the
//! browser throttles, and §15.2 step 4's catch-up cap turns each call into at
//! most a tenth of a second of play (§G8.7). A *game* in either is paused, not
//! played, because neither has the keyboard (§G4.7).
//!
//! What this deliberately does **not** add is §15.2 step 5's frame comparison.
//! That exists because `ratatui` diffs against a previous buffer; immediate
//! mode rebuilds every repaint, so there is nothing to compare and none of the
//! "anything the screen comes to show must join the struct" hazard to inherit.
//! [`Session::generation`](crate::shell::session::Session::generation) is
//! ignored here for the same reason.
//!
//! The same code runs natively and in a browser tab, and does not ask which
//! except in one place, where the answer is a fact about the platform rather
//! than a choice: a tab cannot close itself (see `leave`).

use crate::gui::attract::{self, Drift};
use crate::gui::host::{self, Clock};
use crate::gui::keys::Keyboard;
use crate::gui::layout::{Layout, Measure};
use crate::gui::overlays::{self, Panels};
use crate::gui::paint;
use crate::gui::playfield::{self, Chrome};
use crate::shell::attract::Attract;
use crate::shell::keys::KeyEvent;
use crate::shell::round::{Fps, FrameState, Round};
use crate::shell::session::{Next, Session};
use crate::shell::time::Stamp;

/// §7's two screens, as a field rather than a `match` in a `loop`.
///
/// The terminal's state machine is a loop over [`Next`] and this is the same
/// machine turned inside out, exactly as §15.2's steps were (`EGUI.md` G4): the
/// compositor calls, and what it finds is whichever screen the run is on. §15's
/// *two* loops are here too — the attract screen asks to be repainted ten times
/// a second and has no accumulator (§15.3), a game sixty (§15.2) — as two
/// answers to [`Round::deadline`](crate::shell::round::Round::deadline)'s
/// question rather than as two `while`s.
enum Screen {
    /// §13, with the drift that belongs to this front-end (§13.4, §G7.3).
    Attract(Attract, Drift),
    /// One game. Boxed because a [`Round`] is an order of magnitude the larger
    /// of the two, and the enum is a field of the application either way.
    Play(Box<Round>),
}

impl Screen {
    /// A fresh attract screen. The drift's entropy is F3's, because a tab has
    /// no OS source to reach for (§G7.3).
    fn attract(now: Stamp) -> Self {
        Screen::Attract(Attract::new(now), Drift::new(host::seed(), now))
    }
}

/// One window, one session, one game.
///
/// The session is **borrowed**, not owned, and that is what makes §16 work
/// without a shutdown hook: `eframe::run_native` hands control back when the
/// window closes, and the caller still has the warnings, the edited config and
/// the §14 table to deal with (§6.2, §16). In a tab the borrow is `'static`,
/// because a tab's run has no end to hand anything back at (§G8.1).
pub struct Gui<'session, 'host> {
    session: &'session mut Session<'host>,
    /// F1: the front-end's clock, and the only thing here that reads one.
    clock: Clock,
    keyboard: Keyboard,
    /// One frame's neutral key events, reused rather than reallocated.
    keys: Vec<KeyEvent>,
    /// §7: which screen the run is on.
    screen: Screen,
    /// What step 5 last reported, for the paint that follows it, or `None` when
    /// the screen is not a game. Carried rather than recomputed so that the two
    /// halves of one callback are looking at the same moment (F1).
    state: Option<FrameState>,
    /// The moment `state` is of: what the paint counts its frame at.
    now: Stamp,
    /// §G3's answer about the viewport, as `logic` measured it — carried for
    /// the same reason `state` is, so the pause that §8.4 forces and the screen
    /// that is drawn agree about whether there was room.
    measure: Option<Measure>,
    /// Whether the keyboard is ours. Without it the game cannot hear the
    /// player — so a game in progress is paused (§G4.7) — and in a tab the
    /// page scrolls on the keys meant for it (§G8.2), so the paint says so
    /// rather than looking merely ignored.
    focused: bool,
    /// Frames `ui` has drawn, for `show_debug`'s read-out (§G4.6).
    fps: Fps,
    /// How many of the session's §16 warnings have been handed to
    /// [`host::report`] — all of them, on a host that reports as they arise.
    reported: usize,
}

impl<'session, 'host> Gui<'session, 'host> {
    /// Open on the attract screen (§13.1, §G7): what the program shows whenever
    /// no game is in progress, and that includes the moment it starts.
    pub fn new(session: &'session mut Session<'host>, clock: Clock) -> Self {
        let now = clock.now();
        Self {
            session,
            clock,
            keyboard: Keyboard::new(),
            keys: Vec::new(),
            screen: Screen::attract(now),
            state: None,
            now,
            // Until `logic` has measured one: `eframe` calls it first.
            measure: None,
            // Until `egui` says otherwise: a window opens focused, and a
            // canvas is focused by `gui::start` the moment it can be.
            focused: true,
            fps: Fps::new(now),
            reported: 0,
        }
    }

    /// §7: where the run goes when a screen hands back.
    ///
    /// `Play` is **PLAY** and §10.1's held restart; `Attract` is the quit key
    /// out of a game and the way a finished one ends. `Quit` is the menu's
    /// **QUIT**, and the one of the three that a browser tab cannot do: a page
    /// the player opened may not close itself, so **QUIT** is not offered there
    /// at all (`Session::menu`, §G8.1) and the quit *key*, which is always
    /// live, simply comes back here (§G7.5).
    fn leave(&mut self, next: Next, ctx: &egui::Context, web: bool, now: Stamp) {
        self.screen = match next {
            Next::Play => Screen::Play(Box::new(Round::new(self.session, now))),
            Next::Attract => Screen::attract(now),
            Next::Quit if web => Screen::attract(now),
            Next::Quit => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        };
    }

    /// §13.4's drift is spread over the whole viewport, so it is measured in
    /// cells of it rather than in cells of the block.
    fn viewport_cells(layout: &Layout, area: egui::Rect) -> (i32, i32) {
        let cell = layout.cell().max(1.0);
        (
            (area.width() / cell).ceil() as i32,
            (area.height() / cell).ceil() as i32,
        )
    }
}

impl eframe::App for Gui<'_, '_> {
    /// §15.2 steps 1-4 and 6-7, once per callback and also while hidden.
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // F1: one stamp for the whole frame, so every step below agrees about
        // when it is happening.
        let now = self.clock.now();

        // 2. Drain `egui`'s queue and hand every event over. Nothing is held
        //    back for the next frame, which §15.2 forbids and which a
        //    high-refresh window would otherwise make tempting.
        //
        //    One thing to know about a hidden tab: `eframe` hands the same
        //    unconsumed input to every pass until one paints, so while hidden
        //    this sees each event more than once. The only events a hidden tab
        //    can receive are focus changes, and the adapter's answer to those
        //    is idempotent — a key already released is not released again.
        self.keys.clear();
        ctx.input(|input| {
            self.keyboard.absorb(&input.events, &mut self.keys);
            self.focused = input.focused;
        });
        let mut leaving = None;
        for event in &self.keys {
            let next = match &mut self.screen {
                Screen::Attract(state, _) => state.key(self.session, event, now),
                Screen::Play(round) => round.key(self.session, event, now),
            };
            if let Some(next) = next {
                leaving = Some(next);
            }
        }

        // F6, §8.4: below §G3's minimum the screen is replaced, and a game in
        //    progress is forced into `Paused` *before* it goes. The minimum is
        //    this front-end's; the consequence is the shell's. Measured here,
        //    in `logic`, because a hidden window has no `ui` and still has a
        //    size — and some platforms give a minimised one none at all.
        let content = ctx.content_rect();
        let measure = Measure::of(content, ctx.pixels_per_point());
        self.measure = Some(measure);
        // §G4.7: a window or a tab that cannot hear the keyboard does not play
        //    on without it — §8.4's path again, keys released and all. Told on
        //    every pass, not only when it changes: a countdown the player left
        //    running when they clicked away has to be caught when it runs out.
        //    A hidden tab reports no focus, so this is also what stops a
        //    backgrounded game creeping on (§G8.7). Neither rule has anything
        //    to say to the attract screen: there is no game on it to pause, and
        //    §13 is what the program shows when there is none (§G7.5).
        //
        // 1, 3, 4, 5. The clock, DAS/ARR, whole ticks and §12.5's timers — and
        //    §10.1's restart key, once it has been held for its second. The
        //    accumulator inside is over elapsed time and never over frames, so
        //    this is the same game at 60 Hz and at 144 Hz (§15.2 step 4). The
        //    attract screen's half of this is §15.3's: a cycle and a colour,
        //    and no core underneath to advance.
        let advanced = match &mut self.screen {
            Screen::Play(round) => {
                round.viewport(measure.fits());
                round.keyboard(self.focused, now);
                let next = round.advance(self.session, now);
                self.state = Some(round.frame(now));
                next
            }
            Screen::Attract(state, drift) => {
                state.advance(now);
                // §13.4's one exclusion here is `show_debug`; its other, `mono`,
                // has no meaning off a terminal (§G7.3).
                if let Measure::Fits(layout) = &measure
                    && !self.session.config.display.show_debug
                {
                    drift.step(now, Self::viewport_cells(layout, content));
                }
                self.state = None;
                None
            }
        };
        if let Some(next) = leaving.or(advanced) {
            self.leave(next, ctx, frame.is_web(), now);
        }
        self.now = now;

        // §16: anything the pass above had to warn about — a high-score table
        // the store refused, say. Natively this does nothing and `main` prints
        // them once the window has gone; a tab has no such moment (§G8.6).
        let warnings = self.session.warnings();
        host::report(&warnings[self.reported..]);
        self.reported = warnings.len();

        // 6, 7. Ask to be woken in time for the next tick. `deadline` is
        //    advice (§15.2 step 6): the compositor may call back sooner, a key
        //    will, and `advance` is correct either way. §15's two loops are
        //    these two answers: 60 Hz with an accumulator under it, and the
        //    attract screen's flat 10 fps with none.
        let deadline = match &self.screen {
            Screen::Play(round) => round.deadline(now),
            Screen::Attract(state, _) => state.deadline(),
        };
        ctx.request_repaint_after(deadline);
    }

    /// §15.2 step 5: draw what the last pump reported.
    ///
    /// Every repaint, unconditionally. The comparison the terminal front-end
    /// makes is a retained-mode concern and is not ported (`FRONTEND.md` F7).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let fps = self.fps.drew(self.now);
        // §13.5: presentation is read as it stands, every repaint. The Options
        // panel edits it in place, and immediate mode has no chrome to rebuild
        // and no generation to watch for (§G1.3). Hold is the running game's
        // answer, not the config's: a game keeps the rules it started under.
        let config = &self.session.config;
        let show_debug = config.display.show_debug;
        // §13.5's panel and §12.6's Controls box read the config as it stands,
        // and the panel offers the rows the shell is navigating (§G5). Both
        // screens open those two boxes, so both are handed this.
        let panels = Panels {
            config,
            settings: self.session.settings,
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(paint::BACKGROUND))
            .show(ui, |ui| {
                let (painter, area) = (ui.painter(), ui.max_rect());
                let Some(measure) = self.measure else {
                    return;
                };
                let layout = match measure {
                    Measure::Fits(layout) => layout,
                    // §G3.3, §12.1: below the minimum *every* screen is
                    // replaced by the message, the attract screen included.
                    Measure::TooSmall { need, have } => {
                        paint::too_small(painter, area, need, have);
                        return;
                    }
                };
                match (&self.screen, &self.state) {
                    (Screen::Play(round), Some(state)) => {
                        let chrome = Chrome {
                            show_grid: config.display.show_grid,
                            // Hold is the running game's answer, not the
                            // config's: a game keeps the rules it started
                            // under (§13.5).
                            hold_enabled: round.hold_enabled(),
                        };
                        playfield::draw(painter, &layout, state, chrome, round.cosmetics());
                        // §12.6 over §G4, in that order: a box is drawn on top
                        // of a screen that is complete underneath it.
                        overlays::draw(painter, &layout, state, &panels);
                        if show_debug {
                            let debug = round.debug(fps);
                            playfield::debug(painter, area, &debug, state.view.ticks);
                        }
                    }
                    (Screen::Attract(state, drift), _) => {
                        let cx = attract::Context {
                            config,
                            panels,
                            menu: self.session.menu,
                            scores: &self.session.scores,
                            recent: self.session.recent,
                        };
                        attract::draw(painter, &layout, area, state, drift, &cx);
                    }
                    // A game whose frame has not been pumped yet: `eframe`
                    // calls `logic` first, so this cannot happen — and if it
                    // ever did, an empty ground is the right answer.
                    (Screen::Play(_), None) => {}
                }
                if !self.focused {
                    paint::unfocused(painter, area);
                }
            });
    }
}
