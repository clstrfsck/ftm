//! Top-level application state machine (§7) and the fixed-timestep event loop
//! (§15.2).
//!
//! The loop's whole job is to turn wall-clock time and key events into whole
//! ticks and `TickInput`s. It is the only place that touches a clock: the core
//! below it never does (§3.1), and the DAS/ARR machine above it is handed a
//! `Duration` rather than reading one (§10.3).

use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyEvent};

use crate::config::{MAX_CATCH_UP_TICKS, PresentationConfig, RulesConfig, TICK};
use crate::core::{Action, Actions, Game, GameEvent, GameView, Shift, TickInput};
use crate::input::{Bindings, InputMode, InputState};
use crate::ui::{self, Tui};

// TODO(stage 11): the full §7 state machine — attract, pause, game over and
// name entry — replacing Stage 6's "play until quit".

/// Input the shell has resolved but no tick has consumed yet.
///
/// It has to be held rather than applied immediately: a frame can run zero
/// ticks (the loop wakes early on a key press, §15.2 step 6), and a tap shorter
/// than 1/60 s must still reach the core.
#[derive(Clone, Copy, Debug, Default)]
struct Pending {
    actions: Actions,
    shift: Option<Shift>,
    cells: u8,
}

/// A game, its input state, and the bridge between them.
pub struct App {
    game: Game,
    input: InputState,
    bindings: Bindings,
    /// Reused across ticks: the core appends and the common tick appends
    /// nothing, so this must not be reallocated sixty times a second (§12.8).
    events: Vec<GameEvent>,
    pending: Pending,
    /// Ticks abandoned because the loop fell more than `MAX_CATCH_UP_TICKS`
    /// behind (§15.2 step 4).
    dropped_ticks: u64,
}

impl App {
    pub fn new(
        rules: RulesConfig,
        presentation: &PresentationConfig,
        mode: InputMode,
        seed: u64,
    ) -> Self {
        Self {
            bindings: Bindings::new(&presentation.keys, &rules),
            input: InputState::new(&rules, mode),
            game: Game::new(rules, seed),
            events: Vec::new(),
            pending: Pending::default(),
            dropped_ticks: 0,
        }
    }

    pub fn view(&self) -> GameView {
        self.game.view()
    }

    pub fn dropped_ticks(&self) -> u64 {
        self.dropped_ticks
    }

    /// Fold in one key event, reporting whether the player asked to leave.
    fn key(&mut self, event: &KeyEvent) -> bool {
        let Some(action) = self.input.key(event, &self.bindings) else {
            return false;
        };
        match action {
            // §7 sends `Playing` + quit to the attract screen; until there is
            // one (Stage 11), it leaves the game.
            Action::Quit => return true,
            // TODO(stage 9): Pause (§9.17). TODO(stage 11): Restart, held for
            // a second (§10.1). Inert for now, and inert here rather than in
            // the core, where a stray action could reset a timer (§10.1).
            Action::Pause | Action::Restart => {}
            action => {
                let _ = self.pending.actions.push(action);
            }
        }
        false
    }

    /// Resolve DAS/ARR over one frame into the cells the core is owed
    /// (§15.2 step 3).
    fn resolve_shift(&mut self, dt: Duration) {
        let (shift, cells) = self.input.resolve(dt);
        if shift.is_none() {
            // Whatever is already owed still is: the key may have been tapped
            // and released inside a single frame.
            return;
        }
        if shift != self.pending.shift {
            self.pending.shift = shift;
            self.pending.cells = 0;
        }
        self.pending.cells = self.pending.cells.saturating_add(cells);
    }

    /// Run `ticks` ticks of the core (§15.2 step 4).
    ///
    /// Edge-triggered actions and the DAS-resolved shift are consumed by the
    /// first tick of the batch only; the held soft drop applies to every tick
    /// in it.
    fn advance(&mut self, ticks: u32) {
        if ticks == 0 {
            return;
        }
        // TODO(stage 9): hand the events to the animation and status-line state
        // (§12.5) instead of dropping them. Dropping them changes nothing about
        // the game, which is the point of §12.8.
        self.events.clear();
        for tick in 0..ticks {
            let first = tick == 0;
            let input = TickInput {
                actions: if first {
                    std::mem::take(&mut self.pending.actions)
                } else {
                    Actions::default()
                },
                soft_drop: self.input.soft_drop(),
                shift: if first { self.pending.shift } else { None },
                shift_cells: if first { self.pending.cells } else { 0 },
            };
            self.game.tick(&input, &mut self.events);
        }
        self.pending.cells = 0;
    }
}

/// How many ticks are due now, and how many were discarded (§15.2 step 4).
///
/// Beyond `MAX_CATCH_UP_TICKS` the arrears are thrown away rather than played
/// out, so a suspended laptop or a scrolled terminal does not resume into an
/// instant death.
fn ticks_due(accumulator: &mut Duration) -> (u32, u64) {
    let mut ticks = 0;
    while *accumulator >= TICK && ticks < MAX_CATCH_UP_TICKS {
        *accumulator -= TICK;
        ticks += 1;
    }
    let dropped = (accumulator.as_nanos() / TICK.as_nanos()) as u64;
    if dropped > 0 {
        *accumulator = Duration::ZERO;
    }
    (ticks, dropped)
}

/// Play until the player quits or the process is killed (§15.2).
pub fn run(
    terminal: &mut Tui,
    rules: RulesConfig,
    presentation: &PresentationConfig,
    mode: InputMode,
    seed: u64,
) -> Result<()> {
    let mut app = App::new(rules, presentation, mode, seed);
    let mut previous: Option<GameView> = None;
    let mut accumulator = Duration::ZERO;
    let mut last = Instant::now();

    loop {
        // 1. Wall-clock time since the last iteration.
        let now = Instant::now();
        let dt = now.saturating_duration_since(last);
        accumulator += dt;
        last = now;

        // 2. Drain *every* event that is already waiting; reading one per frame
        //    would leave fast typing lagging behind.
        let mut invalidated = false;
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) => {
                    if app.key(&key) {
                        return Ok(());
                    }
                }
                // §8.4: a resize invalidates the whole frame.
                Event::Resize(..) => invalidated = true,
                _ => {}
            }
        }

        // 3. Resolve DAS/ARR against the wall clock, not the tick rate (§10.3).
        app.resolve_shift(dt);

        // 4. Advance the core in whole ticks, discarding any arrears.
        let (ticks, dropped) = ticks_due(&mut accumulator);
        app.advance(ticks);
        app.dropped_ticks += dropped;

        // 5. Draw, but only when there is something new to look at: ratatui
        //    diffs against its previous buffer, so an unchanged frame is cheap,
        //    and skipping it entirely is cheaper.
        let view = app.view();
        if invalidated || previous.as_ref() != Some(&view) {
            // §16: a frame lost to a write failure is simply lost. Leaving
            // `previous` behind is what makes the next frame try again.
            if terminal.draw(|frame| ui::draw(frame, &view)).is_ok() {
                previous = Some(view);
            }
        }

        // 6. Wait out the rest of the tick. Polling rather than sleeping means
        //    a key wakes the loop early, so input latency stays near one tick
        //    while idle CPU stays near zero.
        event::poll(TICK.saturating_sub(accumulator))?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_steady_machine_runs_one_tick_per_frame() {
        let mut accumulator = TICK;
        assert_eq!(ticks_due(&mut accumulator), (1, 0));
        assert_eq!(accumulator, Duration::ZERO);
        // A frame that arrives a whisker early runs nothing and keeps the time.
        let mut accumulator = TICK - Duration::from_nanos(1);
        assert_eq!(ticks_due(&mut accumulator), (0, 0));
        assert_eq!(accumulator, TICK - Duration::from_nanos(1));
    }

    #[test]
    fn a_slow_frame_catches_up_and_keeps_the_remainder() {
        // §15.2 step 4: up to MAX_CATCH_UP_TICKS ticks in one iteration, and
        // the sub-tick remainder is carried, not lost.
        let mut accumulator = TICK * 3 + Duration::from_millis(5);
        assert_eq!(ticks_due(&mut accumulator), (3, 0));
        assert_eq!(accumulator, Duration::from_millis(5));
    }

    #[test]
    fn arrears_beyond_the_cap_are_discarded() {
        // §15.2 step 4: the whole point — a suspended laptop must not resume
        // into an instant death. Ten seconds of arrears runs six ticks, not
        // six hundred.
        let mut accumulator = Duration::from_secs(10);
        let (ticks, dropped) = ticks_due(&mut accumulator);
        assert_eq!(ticks, MAX_CATCH_UP_TICKS);
        assert_eq!(dropped, 10 * 60 - u64::from(MAX_CATCH_UP_TICKS));
        assert_eq!(accumulator, Duration::ZERO, "the backlog is thrown away");
    }
}
