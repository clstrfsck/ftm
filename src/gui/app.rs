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
//! 5 and is called only when there is something to paint into. So the game
//! advances behind a hidden window and simply is not drawn — which is the
//! backgrounded-tab case of `EGUI.md` G6 arriving early, and §15.2 step 4's
//! catch-up cap is what keeps it from resuming into an instant death.
//!
//! What this deliberately does **not** add is §15.2 step 5's frame comparison.
//! That exists because `ratatui` diffs against a previous buffer; immediate
//! mode rebuilds every repaint, so there is nothing to compare and none of the
//! "anything the screen comes to show must join the struct" hazard to inherit.
//! [`Session::generation`](crate::shell::session::Session::generation) is
//! ignored here for the same reason.

use crate::gui::host_native::Clock;
use crate::gui::keys::Keyboard;
use crate::gui::paint;
use crate::shell::keys::KeyEvent;
use crate::shell::menus::Overlay;
use crate::shell::round::{FrameState, Round};
use crate::shell::session::{Next, Session};
use crate::shell::time::Stamp;

/// The window's initial size, in points. Room for a 20-row well and the
/// §12.4 furniture that `EGUI.md` G7 puts beside it; §G3 gives this a metric
/// of its own then.
pub const INITIAL_SIZE: [f32; 2] = [520.0, 760.0];

/// One window, one session, one game.
///
/// The session is **borrowed**, not owned, and that is what makes §16 work
/// without a shutdown hook: `eframe::run_native` hands control back when the
/// window closes, and the caller still has the warnings, the edited config and
/// the §14 table to deal with (§6.2, §16).
pub struct Gui<'session, 'host> {
    session: &'session mut Session<'host>,
    /// F1: the front-end's clock, and the only thing here that reads one.
    clock: Clock,
    keyboard: Keyboard,
    /// One frame's neutral key events, reused rather than reallocated.
    keys: Vec<KeyEvent>,
    /// §7, as a field rather than a `match` in a `loop`. The G5 slice has one
    /// screen; `EGUI.md` G11 adds the attract screen beside it, at which point
    /// this becomes the `Screen` enum the plan describes.
    round: Round,
    /// What step 5 last reported, for the paint that follows it. Carried
    /// rather than recomputed so that the two halves of one callback are
    /// looking at the same moment (F1).
    state: FrameState,
}

impl<'session, 'host> Gui<'session, 'host> {
    /// Open on a fresh game.
    pub fn new(session: &'session mut Session<'host>, clock: Clock) -> Self {
        let now = clock.now();
        let round = Round::new(session, now);
        let state = round.frame(now);
        Self {
            session,
            clock,
            keyboard: Keyboard::new(),
            keys: Vec::new(),
            round,
            state,
        }
    }

    /// §7: where the run goes when a game hands back.
    ///
    /// `Play` is §10.1's held restart. The other two are the ways out of a
    /// game — the quit key, and Ctrl-C (§16) — and both close the window,
    /// because there is no attract screen to return to until `EGUI.md` G11.
    fn leave(&mut self, next: Next, ctx: &egui::Context, now: Stamp) {
        match next {
            Next::Play => self.round = Round::new(self.session, now),
            Next::Attract | Next::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }
}

impl eframe::App for Gui<'_, '_> {
    /// §15.2 steps 1-4 and 6-7, once per callback and also while hidden.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // F1: one stamp for the whole frame, so every step below agrees about
        // when it is happening.
        let now = self.clock.now();

        // 2. Drain `egui`'s queue and hand every event over. Nothing is held
        //    back for the next frame, which §15.2 forbids and which a
        //    high-refresh window would otherwise make tempting.
        self.keys.clear();
        ctx.input(|input| self.keyboard.absorb(&input.events, &mut self.keys));
        let mut leaving = None;
        for event in &self.keys {
            if let Some(next) = self.round.key(self.session, event, now) {
                leaving = Some(next);
            }
        }

        // F6: this front-end has no minimum yet — §G3 states one at `EGUI.md`
        // G7 and §8.4's forced pause follows from it. Until then the window
        // always fits, which is `Round`'s default and needs saying rather than
        // calling.

        // 1, 3, 4, 5. The clock, DAS/ARR, whole ticks and §12.5's timers — and
        //    §10.1's restart key, once it has been held for its second. The
        //    accumulator inside is over elapsed time and never over frames, so
        //    this is the same game at 60 Hz and at 144 Hz (§15.2 step 4).
        let leaving = leaving.or_else(|| self.round.advance(self.session, now));
        if let Some(next) = leaving {
            self.leave(next, ctx, now);
        }
        self.state = self.round.frame(now);

        // 6, 7. Ask to be woken in time for the next tick. `deadline` is
        //    advice (§15.2 step 6): the compositor may call back sooner, a key
        //    will, and `advance` is correct either way.
        ctx.request_repaint_after(self.round.deadline(now));
    }

    /// §15.2 step 5: draw what the last pump reported.
    ///
    /// Every repaint, unconditionally. The comparison the terminal front-end
    /// makes is a retained-mode concern and is not ported (`FRONTEND.md` F7).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(paint::BACKGROUND))
            .show(ui, |ui| {
                paint::playfield(
                    ui.painter(),
                    ui.max_rect(),
                    &self.state.view,
                    self.state.overlay != Overlay::None,
                );
            });
    }
}
