//! Top-level application state machine (§7) and the fixed-timestep event loop
//! (§15.2).
//!
//! The loop's whole job is to turn wall-clock time and key events into whole
//! ticks and `TickInput`s. It is the only place that touches a clock: the core
//! below it never does (§3.1), the DAS/ARR machine above it is handed a
//! `Duration` rather than reading one (§10.3), and the §12.5 animations get the
//! same `Instant` the frame was drawn at.

use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};

use crate::config::{MAX_CATCH_UP_TICKS, PresentationConfig, RulesConfig, TICK};
use crate::core::{Action, Actions, Game, GameEvent, GameView, Shift, TickInput};
use crate::input::{Bindings, InputMode, InputState};
use crate::ui::overlays::PauseChoice;
use crate::ui::theme::Theme;
use crate::ui::{self, Chrome, Cosmetics, Overlay, Tui};

// TODO(stage 11): the rest of the §7 state machine — attract, restart and name
// entry — replacing Stage 6's "play until quit". `Playing`, `Paused` and
// `GameOver` are here; what they have nowhere to go *to* is the attract screen.

/// §9.17: one second per number, three numbers.
const COUNTDOWN: Duration = Duration::from_secs(3);
/// §9.16: input is ignored for a second, so a keypress in flight at the moment
/// of death cannot dismiss the box before it has been read.
const GAME_OVER_LOCKOUT: Duration = Duration::from_secs(1);

/// Where the application is (§7), narrowed to the states a game can be in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Playing,
    Paused { selected: usize },
    Resuming { since: Instant },
    GameOver { since: Instant },
}

impl Phase {
    /// Whether the game clock advances (§7, §9.17): it does not in `Paused`,
    /// during the resume countdown, or once the game is over.
    const fn running(self) -> bool {
        matches!(self, Phase::Playing)
    }
}

/// What the loop should do after a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flow {
    Continue,
    Leave,
}

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
    phase: Phase,
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
            phase: Phase::Playing,
            dropped_ticks: 0,
        }
    }

    pub fn view(&self) -> GameView {
        self.game.view()
    }

    pub fn dropped_ticks(&self) -> u64 {
        self.dropped_ticks
    }

    /// What the screen should draw on top of the playfield (§12.6).
    fn overlay(&self, now: Instant) -> Overlay {
        match self.phase {
            Phase::Playing => Overlay::None,
            Phase::Paused { selected } => Overlay::Paused { selected },
            Phase::Resuming { since } => {
                let elapsed = now.saturating_duration_since(since);
                let left = COUNTDOWN.saturating_sub(elapsed);
                Overlay::Resuming {
                    count: (left.as_secs() as u8 + 1).min(3),
                }
            }
            Phase::GameOver { .. } => Overlay::GameOver,
        }
    }

    /// Fold in one key event, reporting whether the player asked to leave.
    fn key(&mut self, event: &KeyEvent, now: Instant) -> Flow {
        match self.phase {
            Phase::Playing => self.play_key(event),
            Phase::Paused { selected } => self.pause_key(event, selected, now),
            // §9.17: input other than pause is ignored during the countdown.
            Phase::Resuming { .. } => {
                if self.is_pause(event) {
                    self.pause(0);
                }
                Flow::Continue
            }
            // §9.16: any key, once the box has been up for a second.
            Phase::GameOver { since } => {
                let pressed = event.kind != KeyEventKind::Release;
                if pressed && now.saturating_duration_since(since) >= GAME_OVER_LOCKOUT {
                    // TODO(stage 11): to the attract screen, via name entry if
                    // the score qualifies (§7).
                    return Flow::Leave;
                }
                Flow::Continue
            }
        }
    }

    fn play_key(&mut self, event: &KeyEvent) -> Flow {
        let Some(action) = self.input.key(event, &self.bindings) else {
            return Flow::Continue;
        };
        match action {
            // §7 sends `Playing` + quit to the attract screen; until there is
            // one (Stage 11), it leaves the game.
            Action::Quit => return Flow::Leave,
            Action::Pause => self.pause(0),
            // TODO(stage 11): Restart, held for a second (§10.1). Inert here
            // rather than in the core, where a stray action could reset a
            // lock-delay timer (§10.1).
            Action::Restart => {}
            action => {
                let _ = self.pending.actions.push(action);
            }
        }
        Flow::Continue
    }

    /// §10.1: overlay navigation is `↑`/`↓`, `Enter`/`Space` and `Esc`,
    /// regardless of the game bindings — and the configured pause key toggles
    /// out again, because §9.17 calls it a toggle.
    fn pause_key(&mut self, event: &KeyEvent, selected: usize, now: Instant) -> Flow {
        if self.is_pause(event) {
            self.resume(now);
            return Flow::Continue;
        }
        let items = PauseChoice::ALL.len();
        match menu_action(event) {
            Some(Action::MenuUp) => {
                self.phase = Phase::Paused {
                    selected: (selected + items - 1) % items,
                }
            }
            Some(Action::MenuDown) => {
                self.phase = Phase::Paused {
                    selected: (selected + 1) % items,
                }
            }
            Some(Action::MenuBack) => self.resume(now),
            Some(Action::MenuSelect) => match PauseChoice::ALL[selected] {
                PauseChoice::Resume => self.resume(now),
                // TODO(stage 11): Restart starts a fresh game and Controls
                // opens the §13.5 panel; both need the state machine that
                // stage builds.
                PauseChoice::Restart | PauseChoice::Controls => {}
                PauseChoice::QuitToMenu => return Flow::Leave,
            },
            _ => {}
        }
        Flow::Continue
    }

    /// Whether this event is the configured pause key (§9.17).
    fn is_pause(&self, event: &KeyEvent) -> bool {
        self.input.binding(event, &self.bindings) == Some(Action::Pause)
    }

    fn pause(&mut self, selected: usize) {
        self.phase = Phase::Paused { selected };
        // In legacy mode a key is held until it falls quiet (§8.2), and nothing
        // expires it while the clock is stopped. Letting go of everything on
        // the way in is what stops a soft drop surviving the pause.
        self.input.release_all();
        self.pending = Pending::default();
    }

    fn resume(&mut self, now: Instant) {
        self.phase = Phase::Resuming { since: now };
    }

    /// Resolve DAS/ARR over one frame into the cells the core is owed
    /// (§15.2 step 3).
    ///
    /// §10.4: outside `Playing` the held state is still tracked, so DAS charge
    /// survives the line-clear pause and the countdown, but nothing it resolves
    /// is applied.
    fn resolve_shift(&mut self, dt: Duration) {
        let (shift, cells) = self.input.resolve(dt);
        if !self.phase.running() {
            self.pending = Pending::default();
            return;
        }
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
    fn advance(&mut self, ticks: u32, now: Instant) {
        // Cleared unconditionally: the events of a frame belong to that frame,
        // and a frame that ran no ticks produced none.
        self.events.clear();
        if !self.phase.running() {
            return;
        }
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
        if self
            .events
            .iter()
            .any(|event| matches!(event, GameEvent::ToppedOut(_)))
        {
            self.phase = Phase::GameOver { since: now };
        }
    }
}

/// §10.1's fixed overlay navigation, which is deliberately *not* rebindable.
fn menu_action(event: &KeyEvent) -> Option<Action> {
    if event.kind == KeyEventKind::Release {
        return None;
    }
    Some(match event.code {
        KeyCode::Up => Action::MenuUp,
        KeyCode::Down => Action::MenuDown,
        KeyCode::Enter | KeyCode::Char(' ') => Action::MenuSelect,
        KeyCode::Esc => Action::MenuBack,
        _ => return None,
    })
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
    let chrome = Chrome {
        theme: Theme::resolve(presentation.display.color_depth),
        show_grid: presentation.display.show_grid,
        hold_enabled: rules.hold_enabled,
    };
    // TODO(stage 10): the §12.4 debug stats box, which `show_debug` turns on and
    // which nothing can reach until the config file and CLI of Stage 10 exist.
    let clear_delay = TICK * rules.line_clear_delay_ticks;
    let mut app = App::new(rules, presentation, mode, seed);
    let mut fx = Cosmetics::new(clear_delay, Instant::now());
    let mut previous: Option<(GameView, Overlay)> = None;
    let mut accumulator = Duration::ZERO;
    let mut last = Instant::now();

    loop {
        // 1. Wall-clock time since the last iteration. §9.17: while the clock
        //    is stopped the time simply does not accumulate, so unpausing does
        //    not pay out the pause as catch-up ticks.
        let now = Instant::now();
        let dt = now.saturating_duration_since(last);
        last = now;
        if app.phase.running() {
            accumulator += dt;
        } else {
            accumulator = Duration::ZERO;
        }
        // The countdown is the one non-running phase that ends by itself.
        if let Phase::Resuming { since } = app.phase {
            if now.saturating_duration_since(since) >= COUNTDOWN {
                app.phase = Phase::Playing;
            }
        }

        // 2. Drain *every* event that is already waiting; reading one per frame
        //    would leave fast typing lagging behind.
        let mut invalidated = false;
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) => {
                    if app.key(&key, now) == Flow::Leave {
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
        app.advance(ticks, now);
        app.dropped_ticks += dropped;

        // 5. Hand the tick's events to the cosmetics (§12.5, §12.8). Nothing
        //    below this line can reach the core, which is what makes the whole
        //    of §12.5 provably free of side effects on the game.
        fx.absorb(&app.events, now);

        // 6. Draw, but only when there is something new to look at: ratatui
        //    diffs against its previous buffer, so an unchanged frame is cheap,
        //    and skipping it entirely is cheaper. An animation in flight
        //    changes the screen without changing the view, so it counts as new.
        let frame = (app.view(), app.overlay(now));
        if invalidated || fx.animating() || previous.as_ref() != Some(&frame) {
            // §16: a frame lost to a write failure is simply lost. Leaving
            // `previous` behind is what makes the next frame try again.
            if terminal
                .draw(|f| ui::draw(f, &frame.0, &chrome, &fx, frame.1))
                .is_ok()
            {
                previous = Some(frame);
            }
        }

        // 7. Wait out the rest of the tick. Polling rather than sleeping means
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

    fn app() -> App {
        let rules = RulesConfig::default();
        let presentation = PresentationConfig::default();
        App::new(rules, &presentation, InputMode::Enhanced, 42)
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, crossterm::event::KeyModifiers::NONE)
    }

    #[test]
    fn pause_stops_the_clock_and_resumes_through_a_countdown() {
        // §9.17: the game clock does not advance while paused, and unpausing
        // runs a 3-2-1 countdown during which it still does not.
        let mut app = app();
        let now = Instant::now();
        assert!(app.phase.running());

        assert_eq!(app.key(&press(KeyCode::Esc), now), Flow::Continue);
        assert_eq!(app.phase, Phase::Paused { selected: 0 });
        assert!(!app.phase.running(), "the clock is stopped");

        // The core is not advanced at all while the clock is stopped.
        let before = app.view();
        app.advance(60, now);
        assert_eq!(app.view(), before, "a paused game does not tick");

        assert_eq!(app.key(&press(KeyCode::Esc), now), Flow::Continue);
        assert_eq!(app.phase, Phase::Resuming { since: now });
        assert!(!app.phase.running(), "nor does it during the countdown");
        assert_eq!(app.overlay(now), Overlay::Resuming { count: 3 });
        assert_eq!(
            app.overlay(now + Duration::from_millis(1_500)),
            Overlay::Resuming { count: 2 },
        );
        assert_eq!(
            app.overlay(now + Duration::from_millis(2_500)),
            Overlay::Resuming { count: 1 },
        );
    }

    #[test]
    fn the_pause_menu_wraps_and_resume_is_the_first_item() {
        let mut app = app();
        let now = Instant::now();
        app.pause(0);
        app.key(&press(KeyCode::Down), now);
        assert_eq!(app.phase, Phase::Paused { selected: 1 });
        app.key(&press(KeyCode::Up), now);
        app.key(&press(KeyCode::Up), now);
        assert_eq!(
            app.phase,
            Phase::Paused { selected: 3 },
            "up from the top wraps to Quit to menu",
        );
        assert_eq!(app.key(&press(KeyCode::Enter), now), Flow::Leave);

        app.pause(0);
        assert_eq!(app.key(&press(KeyCode::Enter), now), Flow::Continue);
        assert!(
            matches!(app.phase, Phase::Resuming { .. }),
            "Resume resumes"
        );
    }

    #[test]
    fn a_pause_swallows_the_input_that_was_in_flight() {
        // §9.17 stops the timers; a rotation queued a moment before must not
        // be waiting to fire when the countdown ends.
        let mut app = app();
        let now = Instant::now();
        app.key(&press(KeyCode::Up), now);
        assert_ne!(app.pending.actions, Actions::default());
        app.key(&press(KeyCode::Esc), now);
        assert_eq!(app.pending.actions, Actions::default());
    }

    #[test]
    fn the_game_over_box_cannot_be_dismissed_for_a_second() {
        // §9.16: input is ignored for 1 s, so the keypress that killed you
        // does not also dismiss the box.
        let mut app = app();
        let now = Instant::now();
        app.phase = Phase::GameOver { since: now };
        assert_eq!(app.key(&press(KeyCode::Char('x')), now), Flow::Continue);
        assert_eq!(
            app.key(&press(KeyCode::Char('x')), now + Duration::from_millis(999)),
            Flow::Continue,
        );
        assert_eq!(
            app.key(&press(KeyCode::Char('x')), now + GAME_OVER_LOCKOUT),
            Flow::Leave,
        );
    }
}
