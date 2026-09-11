//! One game, as an object a front-end pumps (§7, §15.2).
//!
//! §15.2's seven numbered steps were once the body of a `while` loop in the
//! terminal front-end. They are methods here, because a windowing system calls
//! an application rather than being called by one (`FRONTEND.md` F7): the
//! front-end drains its own event queue into [`Round::key`], calls
//! [`Round::advance`] as often as it likes, draws whatever [`Round::frame`]
//! reports and waits at most [`Round::deadline`] before coming back.
//!
//! The steps are still in order, and the order is still §15.2's; what changed
//! at `EGUI.md` stage G4 is which side of the call they are on.
//!
//! Nothing here reads a clock. Every entry point takes the [`Stamp`] the
//! front-end was called at (F1), which is also what makes the whole of a game
//! testable without one — the property §17.1 gives the core, one layer out.

use std::time::Duration;

use crate::core::{Action, Actions, DebugView, Game, GameEvent, GameView, Shift, TickInput};
use crate::shell::config::{MAX_CATCH_UP_TICKS, TICK};
use crate::shell::cosmetics::Cosmetics;
use crate::shell::input::{Bindings, InputMode, InputState};
use crate::shell::keys::{Key, KeyEvent, KeyKind};
use crate::shell::menus::{NameEntry, Overlay, PauseChoice, Setting};
use crate::shell::session::{Next, Session};
use crate::shell::time::Stamp;

/// §9.17: one second per number, three numbers.
const COUNTDOWN: Duration = Duration::from_secs(3);
/// §9.16: input is ignored for a second, so a keypress in flight at the moment
/// of death cannot dismiss the box before it has been read.
const GAME_OVER_LOCKOUT: Duration = Duration::from_secs(1);
/// §10.1: the restart key must be held this long before it takes effect.
const RESTART_HOLD: Duration = Duration::from_secs(1);
/// How long a restart hold survives silence in legacy mode (§8.2).
const RESTART_QUIET: Duration = Duration::from_millis(700);

/// Where a game is (§7), narrowed to the phases a game can be in.
///
/// §7's state is two levels rather than one: [`Next`] chooses the screen and
/// `Phase` says where inside a game it is. The phases that are not in §7's
/// list are the two that have nowhere else to live — the §13.5 Options panel,
/// which §12.6 draws over the paused playfield, and §9.17's resume countdown,
/// during which the clock is still stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Playing,
    Paused {
        selected: usize,
    },
    /// The §13.5 Options panel, over the paused playfield (§12.6).
    Options {
        selected: usize,
    },
    /// §12.6's Controls item, over the same blanked playfield.
    Controls,
    Resuming {
        since: Stamp,
    },
    GameOver {
        since: Stamp,
    },
    /// §12.6, only when the score qualifies. `rank` is zero-based, as the
    /// table counts it.
    NameEntry {
        rank: usize,
    },
}

impl Phase {
    /// Whether the game clock advances (§7, §9.17): it does not in `Paused`,
    /// during the resume countdown, or once the game is over.
    const fn running(self) -> bool {
        matches!(self, Phase::Playing)
    }
}

/// What the caller should do after a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flow {
    Continue,
    Leave(Next),
}

/// Input the shell has resolved but no tick has consumed yet.
///
/// It has to be held rather than applied immediately: a frame can run zero
/// ticks (§15.2 step 6 wakes the loop early on a key press, and a GUI at
/// 144 Hz runs none more than half the time), and a tap shorter than 1/60 s
/// must still reach the core.
#[derive(Clone, Copy, Debug, Default)]
struct Pending {
    actions: Actions,
    shift: Option<Shift>,
    cells: u8,
}

/// §10.1's "hold 1 s" confirmation on the restart key.
///
/// The restart key is edge-triggered (§10.2), so holding it is not something
/// `InputState` tracks; this is the smallest thing that does. In enhanced mode
/// the release event ends the hold. In legacy mode there is none (§8.2), so the
/// hold ends when the key falls quiet — but the window has to outlast the
/// terminal's *first* auto-repeat, which is around half a second on macOS
/// defaults, not the 90 ms that separates repeats once they are flowing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Confirm {
    since: Option<Stamp>,
    last: Option<Stamp>,
}

impl Confirm {
    fn press(&mut self, now: Stamp) {
        self.since.get_or_insert(now);
        self.last = Some(now);
    }

    fn release(&mut self) {
        *self = Self::default();
    }

    /// Age out a legacy-mode hold that has fallen quiet (§8.2). Never called in
    /// enhanced mode, where the release event is authoritative.
    fn expire(&mut self, now: Stamp) {
        if self
            .last
            .is_some_and(|last| now.saturating_since(last) >= RESTART_QUIET)
        {
            self.release();
        }
    }

    /// How far the hold has come, as a percentage, or `None` if the key is up.
    fn progress(&self, now: Stamp) -> Option<u8> {
        let since = self.since?;
        let elapsed = now.saturating_since(since);
        Some((elapsed.as_millis() * 100 / RESTART_HOLD.as_millis()).min(100) as u8)
    }

    fn done(&self, now: Stamp) -> bool {
        self.progress(now) == Some(100)
    }
}

/// The debug strip's figures (§12.4), half from the shell and half from the
/// core.
///
/// The core's half arrives as a [`DebugView`] — a view type, not a `Game` — so
/// §12.7's layering rule holds here as it does everywhere else above the core.
/// The figures are front-end-agnostic even though §12.4's *layout* for them is
/// the terminal's; `fps` is the one a front-end counts for itself, because
/// what it means is "frames this front-end actually drew".
#[derive(Clone, Debug)]
pub struct Debug {
    /// Frames actually drawn in the last second (§15.2).
    pub fps: u32,
    /// Ticks abandoned to the catch-up cap since the game began (§15.2 step 4).
    pub dropped: u64,
    /// How far the held direction's DAS charge has come, as a percentage
    /// (§10.3).
    pub das_charge: u8,
    /// Which of §8.2's two paths is live.
    pub mode: InputMode,
    pub core: DebugView,
}

impl Debug {
    /// The nine figures of §12.4's debug read-out, labelled and written out,
    /// in three rows of three.
    ///
    /// What the figures are and how each is written is shared, so that the
    /// terminal's strip and the window's panel cannot disagree about a number;
    /// how they are set out is each front-end's (`TUI.md` §12.4, `GUI.md`
    /// §G4). `ticks` is the view's, which is the one figure `Debug` does not
    /// carry. Gravity is written from the core's integer milli-G rather than
    /// computed here, so the read-out cannot disagree with the rules about how
    /// fast a piece is falling (§9.9).
    pub fn figures(&self, ticks: u64) -> [[(&'static str, String); 3]; 3] {
        // The bag empties as the queue is topped up, so "nothing left" is the
        // common answer at the default `preview_count` and is worth showing as
        // something rather than as a blank.
        let bag: String = self.core.bag.iter().map(|kind| kind.glyph()).collect();
        [
            [
                ("FPS", self.fps.to_string()),
                ("TICKS", ticks.to_string()),
                ("DROPPED", self.dropped.to_string()),
            ],
            [
                ("G", gravity(self.core.milli_g)),
                ("LOCK", optional(self.core.lock_delay)),
                ("PERIOD", self.core.fall_period.to_string()),
            ],
            [
                ("DAS", format!("{}%", self.das_charge)),
                ("BAG", if bag.is_empty() { "-".to_string() } else { bag }),
                ("INPUT", self.mode.name().to_string()),
            ],
        ]
    }
}

/// Gravity in G, from the core's integer thousandths (§9.9).
fn gravity(milli_g: u32) -> String {
    format!("{}.{:03}", milli_g / 1000, milli_g % 1000)
}

/// A figure that is only sometimes there — the lock delay, while grounded.
fn optional(value: Option<u32>) -> String {
    value.map_or_else(|| "-".to_string(), |v| v.to_string())
}

/// Frames actually drawn in the last second: [`Debug::fps`] (§12.4).
///
/// Counted rather than derived from the frame time, because what the figure
/// means is how many frames the front-end *drew* — and a terminal skips a frame
/// that would change nothing (§15.2 step 5) while a window draws whenever the
/// compositor asks. The front-end calls [`Fps::drew`] once for each frame it
/// actually drew; what counts as one is its own business.
#[derive(Debug)]
pub struct Fps {
    since: Stamp,
    frames: u32,
    rate: u32,
}

impl Fps {
    pub fn new(now: Stamp) -> Self {
        Self {
            since: now,
            frames: 0,
            rate: 0,
        }
    }

    /// Count one drawn frame and report the rate.
    pub fn drew(&mut self, now: Stamp) -> u32 {
        self.frames += 1;
        if now.saturating_since(self.since) >= Duration::from_secs(1) {
            self.rate = self.frames;
            self.frames = 0;
            self.since = now;
        }
        self.rate
    }
}

/// Everything a front-end needs to draw one frame of a game (§15.2 step 5).
///
/// It is the *state*, not a decision to redraw. A retained-mode front-end may
/// compare it with the last one and skip an unchanged draw — `TUI.md` §12.4
/// says what that comparison has to include for it to be safe — and an
/// immediate-mode one rebuilds every repaint and must not grow a comparison
/// (`FRONTEND.md` F7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameState {
    pub view: GameView,
    pub overlay: Overlay,
    /// [`Session::generation`], as of the last pump: what the screen shows
    /// that is in neither of the two above.
    pub generation: u32,
    /// §10.1's restart hold, as a percentage of the second it needs.
    pub restart: Option<u8>,
    /// Whether the viewport is below the front-end's minimum (F6). What the
    /// message says, and the size in it, is the front-end's own.
    pub cramped: bool,
}

/// A game, its input state, and the bridge between them.
struct App {
    game: Game,
    input: InputState,
    bindings: Bindings,
    /// §13.5: the rules the game started under, whatever the config says now.
    hold_enabled: bool,
    /// The rows the Options panel offers in *this* front-end
    /// ([`Session::settings`]).
    settings: &'static [Setting],
    /// Reused across ticks: the core appends and the common tick appends
    /// nothing, so this must not be reallocated sixty times a second (§12.8).
    events: Vec<GameEvent>,
    pending: Pending,
    phase: Phase,
    /// §10.1's restart hold.
    restart: Confirm,
    /// §12.6's name field, pre-filled once so the player is not typing into it
    /// twice if they restart.
    name: NameEntry,
    /// Ticks abandoned because the front-end fell more than
    /// `MAX_CATCH_UP_TICKS` behind (§15.2 step 4).
    dropped_ticks: u64,
}

impl App {
    fn new(session: &Session<'_>) -> Self {
        let (rules, presentation) = session.config.resolve();
        Self {
            bindings: Bindings::new(&presentation.keys, &rules),
            input: InputState::new(&rules, session.mode),
            hold_enabled: rules.hold_enabled,
            settings: session.settings,
            game: Game::new(rules, session.next_seed()),
            events: Vec::new(),
            pending: Pending::default(),
            phase: Phase::Playing,
            restart: Confirm::default(),
            name: NameEntry::prefilled(),
            dropped_ticks: 0,
        }
    }

    fn view(&self) -> GameView {
        self.game.view()
    }

    /// The §12.4 debug strip's figures: the shell's own, plus the core's
    /// through a `DebugView` (§12.7).
    fn debug(&self, fps: u32) -> Debug {
        Debug {
            fps,
            dropped: self.dropped_ticks,
            das_charge: self.input.das_charge(),
            mode: self.input.mode(),
            core: self.game.debug(),
        }
    }

    /// What the screen should draw on top of the playfield (§12.6).
    fn overlay(&self, now: Stamp) -> Overlay {
        match self.phase {
            Phase::Playing => Overlay::None,
            Phase::Paused { selected } => Overlay::Paused { selected },
            Phase::Options { selected } => Overlay::Options { selected },
            Phase::Resuming { since } => {
                let elapsed = now.saturating_since(since);
                let left = COUNTDOWN.saturating_sub(elapsed);
                Overlay::Resuming {
                    count: (left.as_secs() as u8 + 1).min(3),
                }
            }
            Phase::Controls => Overlay::Controls,
            Phase::GameOver { .. } => Overlay::GameOver,
            Phase::NameEntry { rank } => Overlay::NameEntry {
                rank: rank + 1,
                name: self.name.as_str().to_string(),
            },
        }
    }

    /// Fold in one key event, reporting whether the player asked to leave.
    fn key(&mut self, session: &mut Session<'_>, event: &KeyEvent, now: Stamp) -> Flow {
        match self.phase {
            Phase::Playing => self.play_key(event, now),
            Phase::Paused { selected } => self.pause_key(event, selected, now),
            Phase::Options { selected } => self.options_key(session, event, selected),
            // §12.6: the binding table is a read-only box, so any of §10.1's
            // three ways out of an overlay closes it.
            Phase::Controls => {
                if menu_action(event).is_some() {
                    self.phase = Phase::Paused {
                        selected: PauseChoice::Controls.index(),
                    };
                }
                Flow::Continue
            }
            // §9.17: input other than pause is ignored during the countdown.
            Phase::Resuming { .. } => {
                if self.is_pause(event) {
                    self.pause(0);
                }
                Flow::Continue
            }
            // §9.16: any key, once the box has been up for a second.
            Phase::GameOver { since } => {
                let pressed = event.kind != KeyKind::Release;
                if pressed && now.saturating_since(since) >= GAME_OVER_LOCKOUT {
                    return self.finish(session);
                }
                Flow::Continue
            }
            Phase::NameEntry { .. } => self.name_key(session, event),
        }
    }

    fn play_key(&mut self, event: &KeyEvent, now: Stamp) -> Flow {
        // §10.1: the restart key is held rather than pressed, so it is taken
        // off the edge-triggered path before `InputState` sees it.
        if self.bindings.action_of(event) == Some(Action::Restart) {
            if event.kind == KeyKind::Release {
                self.restart.release();
            } else {
                self.restart.press(now);
            }
            return Flow::Continue;
        }
        let Some(action) = self.input.key(event, &self.bindings) else {
            return Flow::Continue;
        };
        match action {
            // §7, §16: quit from a game goes to the attract screen. The run is
            // abandoned, not scored (§11).
            Action::Quit => return Flow::Leave(Next::Attract),
            Action::Pause => self.pause(0),
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
    fn pause_key(&mut self, event: &KeyEvent, selected: usize, now: Stamp) -> Flow {
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
                PauseChoice::Options => self.phase = Phase::Options { selected: 0 },
                // Choosing it from a menu is already the deliberate act that
                // §10.1's one-second hold on the key is there to require.
                PauseChoice::Restart => return Flow::Leave(Next::Play),
                PauseChoice::Controls => self.phase = Phase::Controls,
                PauseChoice::QuitToMenu => return Flow::Leave(Next::Attract),
            },
            _ => {}
        }
        Flow::Continue
    }

    /// §13.5: `↑`/`↓` choose a setting, `←`/`→` change it, `Esc` saves the
    /// config file and returns to the pause menu.
    ///
    /// The write happens here, on the way out, which is what §6.1 means by
    /// "written back to the config file immediately on leaving that screen".
    fn options_key(
        &mut self,
        session: &mut Session<'_>,
        event: &KeyEvent,
        selected: usize,
    ) -> Flow {
        if event.kind == KeyKind::Release {
            return Flow::Continue;
        }
        // The rows this front-end offers, not every row §13.5 lists
        // (`Session::settings`): a cursor that can reach a setting the screen
        // is not drawing is a cursor the player cannot see.
        let items = self.settings.len();
        match event.key {
            Key::Up => {
                self.phase = Phase::Options {
                    selected: (selected + items - 1) % items,
                }
            }
            Key::Down => {
                self.phase = Phase::Options {
                    selected: (selected + 1) % items,
                }
            }
            Key::Left | Key::Right => {
                self.settings[selected].step(&mut session.config, event.key == Key::Right);
                session.bump();
            }
            Key::Esc | Key::Enter => {
                session.save_config();
                self.phase = Phase::Paused {
                    selected: PauseChoice::Options.index(),
                };
            }
            _ => {}
        }
        Flow::Continue
    }

    /// §12.6: up to twelve printable ASCII characters, `Backspace` deletes,
    /// `Enter` confirms, `Esc` cancels and discards the score.
    ///
    /// The rank the box is showing is not read here: it was decided when the
    /// game ended, and `Table::insert` settles the entry's place for itself.
    fn name_key(&mut self, session: &mut Session<'_>, event: &KeyEvent) -> Flow {
        if event.kind == KeyKind::Release {
            return Flow::Continue;
        }
        // §16: Ctrl-C is a key event in raw mode, and it means leave — not a
        // `c` in the name field.
        if event.mods.ctrl {
            if event.is_ctrl_c() {
                return Flow::Leave(Next::Attract);
            }
            return Flow::Continue;
        }
        match event.key {
            Key::Char(c) => {
                self.name.push(c);
            }
            Key::Backspace => {
                self.name.backspace();
            }
            Key::Enter => {
                // An empty name becomes ANON, which `Entry::of` does for us.
                session.record(self.name.as_str(), &self.game.view());
                return Flow::Leave(Next::Attract);
            }
            Key::Esc => return Flow::Leave(Next::Attract),
            _ => {}
        }
        Flow::Continue
    }

    /// §7: a finished game goes to name entry if the score qualifies and the
    /// run was not seeded, and to the attract screen otherwise.
    fn finish(&mut self, session: &Session<'_>) -> Flow {
        // §6.4, §14: a seeded run is reproducible and is never recorded.
        let rank = (!session.seeded)
            .then(|| session.scores.rank_for(self.game.view().score))
            .flatten();
        match rank {
            Some(rank) => {
                self.phase = Phase::NameEntry { rank };
                Flow::Continue
            }
            None => Flow::Leave(Next::Attract),
        }
    }

    /// Whether this event is the configured pause key (§9.17).
    fn is_pause(&self, event: &KeyEvent) -> bool {
        self.input.binding(event, &self.bindings) == Some(Action::Pause)
    }

    /// §8.4: a viewport below the front-end's minimum forces a game in
    /// progress into `Paused`, so the player is never killed by a resize.
    ///
    /// Nothing here undoes itself when there is room again: §9.17's pause is
    /// left for the player to leave, which also gives them the 3-2-1 countdown
    /// before the clock starts again.
    fn cramp(&mut self) {
        if self.phase.running() {
            self.pause(0);
        }
    }

    fn pause(&mut self, selected: usize) {
        self.phase = Phase::Paused { selected };
        // In legacy mode a key is held until it falls quiet (§8.2), and nothing
        // expires it while the clock is stopped. Letting go of everything on
        // the way in is what stops a soft drop surviving the pause.
        self.input.release_all();
        self.restart.release();
        self.pending = Pending::default();
    }

    fn resume(&mut self, now: Stamp) {
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
    fn advance(&mut self, ticks: u32, now: Stamp) {
        // Cleared unconditionally, and *before* the early return: the events of
        // a frame belong to that frame, and the cosmetics absorb this buffer
        // every frame (§12.5). Leaving the last frame's events in it would
        // restart their animations sixty times a second.
        self.events.clear();
        // §15.2 step 6 wakes the loop early on a key press, so a frame can
        // legitimately run no ticks at all. Input the shell has already
        // resolved is held until a tick consumes it -- clearing `pending` here
        // would swallow every tap that landed between two ticks.
        if ticks == 0 || !self.phase.running() {
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
            self.restart.release();
        }
    }

    /// Age the restart hold and report whether it has been held long enough
    /// (§10.1).
    fn restart_due(&mut self, now: Stamp) -> bool {
        if self.input.mode() == InputMode::Legacy {
            self.restart.expire(now);
        }
        self.phase.running() && self.restart.done(now)
    }
}

/// §15.2's loop body, as an object (`FRONTEND.md` F7).
///
/// The front-end owns the loop and this owns the game. It is correct at any
/// cadence — 60 Hz, 144 Hz, twice in a millisecond, or once after a minute in
/// a backgrounded tab — because [`Round::advance`]'s accumulator is over real
/// elapsed time and never over frames (§15.2 step 4, §19.2).
pub struct Round {
    app: App,
    /// §12.5's timers, which see the event stream and a stamp and nothing else
    /// (§12.8). They live here so that no front-end has to remember to feed
    /// them, and step 5's rule — nothing below it can reach the core — is kept
    /// by the shape rather than by a comment.
    fx: Cosmetics,
    /// Sub-tick time carried between pumps (§15.2 step 4). Always below
    /// [`TICK`] when `advance` returns.
    accumulator: Duration,
    /// The stamp the last pump was at, which is what `dt` is measured from.
    last: Stamp,
    /// [`Session::generation`] as of the last pump, so [`Round::frame`] can
    /// report it without being handed the session again.
    settings: u32,
    /// F6: whether the viewport can host the screen.
    fits: bool,
}

impl Round {
    /// Start a fresh game (§7: `Attract` -> `Playing`, or a restart).
    ///
    /// `now` is the moment the front-end is starting it at (F1): the
    /// accumulator and §12.5's timers both date from here.
    pub fn new(session: &Session<'_>, now: Stamp) -> Self {
        let clear_delay = TICK * session.config.resolve().0.line_clear_delay_ticks;
        Self {
            app: App::new(session),
            fx: Cosmetics::new(clear_delay, now),
            accumulator: Duration::ZERO,
            last: now,
            settings: session.generation(),
            fits: true,
        }
    }

    /// §15.2 steps 1 and 3-5: advance the clock, resolve DAS/ARR, run whole
    /// ticks, feed the cosmetics.
    ///
    /// Returns `Some(next)` when the round is over — which from here means only
    /// §10.1's held restart key, since every other way out of a game is a key
    /// and is [`Round::key`]'s answer.
    pub fn advance(&mut self, session: &mut Session<'_>, now: Stamp) -> Option<Next> {
        self.settings = session.generation();
        self.settle(now);
        // 1. Wall-clock time since the last pump. §9.17: while the clock is
        //    stopped the time simply does not accumulate, so unpausing does not
        //    pay out the pause as catch-up ticks.
        let dt = now.saturating_since(self.last);
        self.last = now;
        if self.app.phase.running() {
            self.accumulator += dt;
        } else {
            self.accumulator = Duration::ZERO;
        }
        // §10.1: the restart key, once it has been held for its second.
        if self.app.restart_due(now) {
            return Some(Next::Play);
        }
        // 3. Resolve DAS/ARR against the wall clock, not the tick rate (§10.3).
        self.app.resolve_shift(dt);
        // 4. Advance the core in whole ticks, discarding any arrears.
        let (ticks, dropped) = ticks_due(&mut self.accumulator);
        self.app.advance(ticks, now);
        self.app.dropped_ticks += dropped;
        // 5. Hand the tick's events to the cosmetics (§12.5, §12.8). Nothing
        //    below this line can reach the core, which is what makes the whole
        //    of §12.5 provably free of side effects on the game.
        self.fx.absorb(&self.app.events, now);
        None
    }

    /// §15.2 step 2, one event at a time: the front-end drains its own queue
    /// and hands the events over, and must not withhold one until the next
    /// frame.
    pub fn key(&mut self, session: &mut Session<'_>, event: &KeyEvent, now: Stamp) -> Option<Next> {
        self.settle(now);
        let flow = self.app.key(session, event, now);
        // The panel edits the config as a side effect of a key, so what
        // `frame` reports has to be read back after it (§13.5).
        self.settings = session.generation();
        match flow {
            Flow::Continue => None,
            Flow::Leave(next) => Some(next),
        }
    }

    /// F6: whether the viewport can host the screen.
    ///
    /// The *minimum* is the front-end's — 60 x 24 characters in `TUI.md`
    /// §12.1, a minimum cell size in `GUI.md` §G3 — and the consequence is the
    /// shell's: §8.4 forces a game in progress into `Paused` before the screen
    /// is replaced, and releases its held keys.
    pub fn viewport(&mut self, fits: bool) {
        self.fits = fits;
        if !fits {
            self.app.cramp();
        }
    }

    /// Whether the front-end can hear the keyboard: §8.4's forced pause, for a
    /// reason that is not the viewport's (`GUI.md` §G4.7).
    ///
    /// Not hearing it takes the same path [`Round::viewport`] takes below the
    /// minimum — a game in progress goes to `Paused` and its held keys are
    /// released — and like it, nothing undoes itself: the player leaves the
    /// pause, and gets §9.17's countdown for it. A front-end that can tell
    /// reports on *every* pump, not only when it changes. The countdown is not
    /// `Playing`, so a pause forced during it would find nothing to pause;
    /// settling it first is what catches the pump it runs out on, before
    /// `advance` can play a tick the player cannot answer.
    ///
    /// Unlike the viewport, this changes nothing about what is drawn: a
    /// front-end that has lost the keyboard can still show the screen, and says
    /// so in its own words. A front-end that cannot tell — a terminal that has
    /// not asked for focus reports — never calls it.
    pub fn keyboard(&mut self, heard: bool, now: Stamp) {
        if !heard {
            self.settle(now);
            self.app.cramp();
        }
    }

    /// Everything a front-end needs to draw, as one value (§15.2 step 5).
    pub fn frame(&self, now: Stamp) -> FrameState {
        FrameState {
            view: self.app.view(),
            overlay: self.app.overlay(now),
            generation: self.settings,
            restart: self.app.restart.progress(now),
            cramped: !self.fits,
        }
    }

    /// How long the front-end may wait before pumping again (§15.2 step 7).
    ///
    /// **Advice, not a frame rate.** A front-end may be woken sooner by a key,
    /// by a compositor or by a tab regaining focus, and one that renders at
    /// vsync may ignore it entirely; `advance` is correct either way. What it
    /// promises is the other direction: waiting this long does not cost a tick.
    pub fn deadline(&self, now: Stamp) -> Duration {
        let due = self.accumulator + now.saturating_since(self.last);
        TICK.saturating_sub(due)
    }

    /// §13.5: the rules the running game started under, whatever the config
    /// says now — and the one layout question `GameView` cannot answer, since
    /// an empty hold slot and an absent hold mechanic are both `hold: None`
    /// (§12.4, §12.7).
    pub fn hold_enabled(&self) -> bool {
        self.app.hold_enabled
    }

    /// The rows the §13.5 Options panel is offering, for the screen that draws
    /// it: the same list [`Round::key`] navigates ([`Session::settings`]).
    pub fn settings(&self) -> &'static [Setting] {
        self.app.settings
    }

    /// §12.5's timers, for the front-end that draws them.
    pub fn cosmetics(&self) -> &Cosmetics {
        &self.fx
    }

    /// The debug strip's figures (§12.4), when a front-end is showing them.
    /// `fps` is the front-end's own count of the frames it drew.
    pub fn debug(&self, fps: u32) -> Debug {
        self.app.debug(fps)
    }

    /// End §9.17's countdown when its three seconds are up.
    ///
    /// Called from both entry points, and that is deliberate: the countdown is
    /// the one non-running phase that ends by itself, and it ended *before* the
    /// event queue was drained when these steps were a loop body. A key that
    /// arrives in the frame the countdown expires is the player's first input
    /// of the resumed game, not one swallowed by the overlay.
    fn settle(&mut self, now: Stamp) {
        if let Phase::Resuming { since } = self.app.phase
            && now.saturating_since(since) >= COUNTDOWN
        {
            self.app.phase = Phase::Playing;
        }
    }
}

/// §10.1's fixed overlay navigation, which is deliberately *not* rebindable.
fn menu_action(event: &KeyEvent) -> Option<Action> {
    if event.kind == KeyKind::Release {
        return None;
    }
    Some(match event.key {
        Key::Up => Action::MenuUp,
        Key::Down => Action::MenuDown,
        Key::Enter | Key::Char(' ') => Action::MenuSelect,
        Key::Esc => Action::MenuBack,
        _ => return None,
    })
}

/// How many ticks are due now, and how many were discarded (§15.2 step 4).
///
/// Beyond `MAX_CATCH_UP_TICKS` the arrears are thrown away rather than played
/// out, so a suspended laptop, a scrolled terminal or a backgrounded browser
/// tab does not resume into an instant death.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::config::{self, ConfigFile, Startup};
    use crate::shell::host::Host;
    use crate::shell::storage::{Memory, Slot, Storage, Unwritable};

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
        // into an instant death, and neither must a browser tab that spent ten
        // seconds in the background. Ten seconds of arrears runs six ticks,
        // not six hundred.
        let mut accumulator = Duration::from_secs(10);
        let (ticks, dropped) = ticks_due(&mut accumulator);
        assert_eq!(ticks, MAX_CATCH_UP_TICKS);
        assert_eq!(dropped, 10 * 60 - u64::from(MAX_CATCH_UP_TICKS));
        assert_eq!(accumulator, Duration::ZERO, "the backlog is thrown away");
    }

    /// §14's date stamp, fixed: the front-end supplies it (`FRONTEND.md` F4),
    /// so a test supplies its own and the ordering rules stay testable.
    fn date() -> String {
        "2026-09-10".to_string()
    }

    /// A different seed every call, which is what `FRONTEND.md` F3 promises an
    /// unseeded run — without reaching for a platform to get it.
    fn varying_seed() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        NEXT.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed)
    }

    /// A session over the store the test hands it. `Memory` is the usual one,
    /// so nothing here reaches a file unless the test asked for one.
    fn session(storage: &mut dyn Storage) -> Session<'_> {
        let file = ConfigFile::default();
        let startup = Startup {
            on_disk: file.clone(),
            file,
            existed: false,
            wrote_config: false,
            // Seeded, so the tests that do not care get the same game twice.
            seed: 42,
            seeded: true,
            warnings: Vec::new(),
        };
        Session::new(
            &startup,
            InputMode::Enhanced,
            Host::new(storage, || 42, date),
        )
    }

    fn app(storage: &mut dyn Storage) -> (App, Session<'_>) {
        let session = session(storage);
        (App::new(&session), session)
    }

    fn press(key: Key) -> KeyEvent {
        KeyEvent::press(key)
    }

    fn release(key: Key) -> KeyEvent {
        KeyEvent::release(key)
    }

    #[test]
    fn a_frame_that_runs_no_ticks_keeps_the_input_it_resolved() {
        // §15.2 step 6: the loop wakes early on a key press, so a frame can
        // legitimately run zero ticks. Input the shell has already resolved
        // must survive until a tick consumes it, or every tap that lands
        // between two ticks -- which is most of them -- is silently lost.
        //
        // Load-bearing since G4: a GUI frame at 144 Hz runs no ticks more than
        // half the time, so this stopped being an edge case.
        let mut storage = Memory::new();
        let (mut app, _session) = app(&mut storage);
        let now = Stamp::ZERO;
        app.pending.shift = Some(Shift::Left);
        app.pending.cells = 1;
        let _ = app.pending.actions.push(Action::RotateCw);

        app.advance(0, now);
        assert_eq!(app.pending.cells, 1, "the cell is still owed");
        assert_ne!(
            app.pending.actions,
            Actions::default(),
            "and so is the turn"
        );

        app.advance(1, now);
        assert_eq!(app.pending.cells, 0, "and one tick consumes both");
        assert_eq!(app.pending.actions, Actions::default());
    }

    #[test]
    fn a_viewport_below_the_minimum_forces_a_game_into_pause() {
        // §8.4: "if a game was in progress it is forced into `Paused` first,
        // so the player is never killed by a window resize". A soft drop that
        // was being held when the window shrank must not survive it either:
        // nothing expires a held key while the clock is stopped (§8.2).
        let mut storage = Memory::new();
        let (mut app, _session) = app(&mut storage);
        let now = Stamp::ZERO;
        app.pending.shift = Some(Shift::Left);
        app.pending.cells = 3;
        assert!(app.phase.running());

        app.cramp();
        assert_eq!(app.phase, Phase::Paused { selected: 0 });
        assert_eq!(app.pending.cells, 0, "and the held input is let go");

        // Still cramped a frame later: the pause is not re-entered, so the
        // menu selection the player has moved to is left alone.
        app.phase = Phase::Paused { selected: 2 };
        app.cramp();
        assert_eq!(app.phase, Phase::Paused { selected: 2 });

        // And it does not disturb a game that has already ended.
        app.phase = Phase::GameOver { since: now };
        app.cramp();
        assert_eq!(app.phase, Phase::GameOver { since: now });
    }

    #[test]
    fn a_frame_that_runs_no_ticks_reports_no_events() {
        // The other half of the same guard: `events` is what the §12.5
        // animations are fed, so a frame that did nothing must say nothing.
        let mut storage = Memory::new();
        let (mut app, _session) = app(&mut storage);
        let now = Stamp::ZERO;
        let _ = app.pending.actions.push(Action::HardDrop);
        app.advance(1, now);
        assert!(!app.events.is_empty(), "a hard drop is eventful");

        app.advance(0, now);
        assert!(app.events.is_empty(), "the next frame did nothing");
    }

    #[test]
    fn pause_stops_the_clock_and_resumes_through_a_countdown() {
        // §9.17: the game clock does not advance while paused, and unpausing
        // runs a 3-2-1 countdown during which it still does not.
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        let now = Stamp::ZERO;
        assert!(app.phase.running());

        assert_eq!(app.key(&mut session, &press(Key::Esc), now), Flow::Continue);
        assert_eq!(app.phase, Phase::Paused { selected: 0 });
        assert!(!app.phase.running(), "the clock is stopped");

        // The core is not advanced at all while the clock is stopped.
        let before = app.view();
        app.advance(60, now);
        assert_eq!(app.view(), before, "a paused game does not tick");

        assert_eq!(app.key(&mut session, &press(Key::Esc), now), Flow::Continue);
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
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        let now = Stamp::ZERO;
        app.pause(0);
        app.key(&mut session, &press(Key::Down), now);
        assert_eq!(app.phase, Phase::Paused { selected: 1 });
        app.key(&mut session, &press(Key::Up), now);
        app.key(&mut session, &press(Key::Up), now);
        assert_eq!(
            app.phase,
            Phase::Paused {
                selected: PauseChoice::ALL.len() - 1
            },
            "up from the top wraps to Quit to menu",
        );
        assert_eq!(
            app.key(&mut session, &press(Key::Enter), now),
            Flow::Leave(Next::Attract)
        );

        app.pause(0);
        assert_eq!(
            app.key(&mut session, &press(Key::Enter), now),
            Flow::Continue
        );
        assert!(
            matches!(app.phase, Phase::Resuming { .. }),
            "Resume resumes"
        );
    }

    #[test]
    fn a_pause_swallows_the_input_that_was_in_flight() {
        // §9.17 stops the timers; a rotation queued a moment before must not
        // be waiting to fire when the countdown ends.
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        let now = Stamp::ZERO;
        app.key(&mut session, &press(Key::Up), now);
        assert_ne!(app.pending.actions, Actions::default());
        app.key(&mut session, &press(Key::Esc), now);
        assert_eq!(app.pending.actions, Actions::default());
    }

    #[test]
    fn the_game_over_box_cannot_be_dismissed_for_a_second() {
        // §9.16: input is ignored for 1 s, so the keypress that killed you
        // does not also dismiss the box.
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        let now = Stamp::ZERO;
        app.phase = Phase::GameOver { since: now };
        assert_eq!(
            app.key(&mut session, &press(Key::Char('x')), now),
            Flow::Continue
        );
        assert_eq!(
            app.key(
                &mut session,
                &press(Key::Char('x')),
                now + Duration::from_millis(999)
            ),
            Flow::Continue,
        );
        assert_eq!(
            app.key(
                &mut session,
                &press(Key::Char('x')),
                now + GAME_OVER_LOCKOUT
            ),
            Flow::Leave(Next::Attract),
        );
    }

    #[test]
    fn the_pause_menu_opens_the_options_panel() {
        // §12.6 as amended, and §6.1's "in-game Options screen".
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        let now = Stamp::ZERO;
        app.pause(0);
        for _ in 0..2 {
            app.key(&mut session, &press(Key::Down), now);
        }
        assert_eq!(
            PauseChoice::ALL[2],
            PauseChoice::Options,
            "third item down (§12.6)",
        );
        app.key(&mut session, &press(Key::Enter), now);
        assert_eq!(app.phase, Phase::Options { selected: 0 });
    }

    #[test]
    fn the_options_panel_edits_and_writes_back_on_the_way_out() {
        // §13.5: `←`/`→` change the selected value, `Esc` saves and returns.
        // §6.1: "written back to the config file immediately on leaving".
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        let now = Stamp::ZERO;
        app.phase = Phase::Options { selected: 0 };

        app.key(&mut session, &press(Key::Right), now);
        assert_eq!(session.config.gameplay.preview_count, 6);
        app.key(&mut session, &press(Key::Down), now);
        app.key(&mut session, &press(Key::Left), now);
        assert_eq!(
            session.config.gameplay.start_level, 15,
            "the second row, wrapping off the bottom",
        );
        assert_eq!(
            session.host.storage.read(Slot::Config),
            Ok(None),
            "nothing is written while the panel is up",
        );

        assert_eq!(app.key(&mut session, &press(Key::Esc), now), Flow::Continue);
        assert_eq!(
            app.phase,
            Phase::Paused {
                selected: PauseChoice::Options.index(),
            },
            "back to the menu, on the item that opened the panel",
        );
        assert!(
            session.saved && session.warnings.is_empty(),
            "{:?}",
            session.warnings
        );

        let mut warnings = Vec::new();
        let written = config::load(session.host.storage, &mut warnings).file;
        assert_eq!(written, session.config);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_config_that_cannot_be_written_warns_rather_than_aborting() {
        // §16: recoverable problems degrade to a documented default and add a
        // line to the warnings printed at exit. A store that refuses every
        // write is what a read-only config directory looks like from here, and
        // §17.3's sign-off exercised that case on a real one.
        let mut storage = Unwritable;
        let mut session = session(&mut storage);
        let mut app = App::new(&session);
        app.phase = Phase::Options { selected: 0 };
        app.key(&mut session, &press(Key::Esc), Stamp::ZERO);
        assert!(!session.saved);
        assert_eq!(session.warnings.len(), 1, "{:?}", session.warnings);
        assert_eq!(
            app.phase,
            Phase::Paused {
                selected: PauseChoice::Options.index(),
            },
            "and the game carries on regardless",
        );
    }

    #[test]
    fn changing_a_value_bumps_the_generation() {
        // §15.2 step 5: a front-end that compares frames to decide whether to
        // draw sees the view and the overlay — and a panel value lives in
        // neither, so the counter is what tells it.
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        let now = Stamp::ZERO;
        app.phase = Phase::Options { selected: 0 };
        let before = session.generation();
        app.key(&mut session, &press(Key::Down), now);
        assert_eq!(
            session.generation(),
            before,
            "moving the cursor changes the overlay"
        );
        app.key(&mut session, &press(Key::Right), now);
        assert_ne!(session.generation(), before, "changing a value does not");
    }

    /// Play one hard drop, so the run has a score worth recording (§9.14).
    fn scored(app: &mut App, now: Stamp) -> u64 {
        let _ = app.pending.actions.push(Action::HardDrop);
        app.advance(1, now);
        let score = app.view().score;
        assert!(score > 0, "a hard drop is worth two points a row");
        score
    }

    #[test]
    fn a_qualifying_score_goes_to_name_entry() {
        // §7: `GameOver` -> `NameEntry` when the score qualifies for the table
        // and the run is unseeded.
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        session.seeded = false;
        let now = Stamp::ZERO;
        scored(&mut app, now);
        app.phase = Phase::GameOver { since: now };
        assert_eq!(
            app.key(
                &mut session,
                &press(Key::Char('x')),
                now + GAME_OVER_LOCKOUT
            ),
            Flow::Continue,
        );
        assert_eq!(app.phase, Phase::NameEntry { rank: 0 });
        assert_eq!(
            app.overlay(now),
            Overlay::NameEntry {
                rank: 1,
                name: app.name.as_str().to_string(),
            },
            "and the box counts from one",
        );
    }

    #[test]
    fn a_seeded_run_is_never_offered_the_table() {
        // §6.4, §14: "runs started with `--seed` are never recorded".
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        assert!(session.seeded);
        let now = Stamp::ZERO;
        scored(&mut app, now);
        app.phase = Phase::GameOver { since: now };
        assert_eq!(
            app.key(
                &mut session,
                &press(Key::Char('x')),
                now + GAME_OVER_LOCKOUT
            ),
            Flow::Leave(Next::Attract),
        );
        assert!(session.scores.entries.is_empty());
    }

    #[test]
    fn a_score_that_does_not_qualify_goes_straight_back() {
        // §14: a score of 0 never qualifies, so a game abandoned before the
        // first lock skips name entry entirely.
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        session.seeded = false;
        let now = Stamp::ZERO;
        assert_eq!(app.view().score, 0);
        app.phase = Phase::GameOver { since: now };
        assert_eq!(
            app.key(
                &mut session,
                &press(Key::Char('x')),
                now + GAME_OVER_LOCKOUT
            ),
            Flow::Leave(Next::Attract),
        );
    }

    #[test]
    fn enter_records_the_score_and_esc_discards_it() {
        // §12.6: "`Enter` confirms ... `Esc` cancels and discards the score."
        let now = Stamp::ZERO;
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        session.seeded = false;
        let score = scored(&mut app, now);
        app.phase = Phase::NameEntry { rank: 0 };
        // Clear whatever `$USER` pre-filled, then type a name of our own.
        for _ in 0..crate::shell::highscore::NAME_MAX {
            app.key(&mut session, &press(Key::Backspace), now);
        }
        for c in "MS".chars() {
            app.key(&mut session, &press(Key::Char(c)), now);
        }
        assert_eq!(
            app.key(&mut session, &press(Key::Enter), now),
            Flow::Leave(Next::Attract),
        );
        assert_eq!(session.scores.entries.len(), 1);
        assert_eq!(session.scores.entries[0].name, "MS");
        assert_eq!(session.scores.entries[0].score, score);
        assert_eq!(session.recent, Some(0), "§13.5 highlights it");

        let mut app = App::new(&session);
        scored(&mut app, now);
        app.phase = Phase::NameEntry { rank: 0 };
        assert_eq!(
            app.key(&mut session, &press(Key::Esc), now),
            Flow::Leave(Next::Attract),
        );
        assert_eq!(session.scores.entries.len(), 1, "nothing was added");
    }

    #[test]
    fn an_empty_name_becomes_anon() {
        // §12.6.
        let now = Stamp::ZERO;
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        session.seeded = false;
        scored(&mut app, now);
        app.phase = Phase::NameEntry { rank: 0 };
        for _ in 0..crate::shell::highscore::NAME_MAX {
            app.key(&mut session, &press(Key::Backspace), now);
        }
        app.key(&mut session, &press(Key::Enter), now);
        assert_eq!(
            session.scores.entries[0].name,
            crate::shell::highscore::ANONYMOUS
        );
    }

    #[test]
    fn the_restart_key_must_be_held_for_a_second() {
        // §10.1: "Restart (hold 1 s)". A tap does nothing, which is the whole
        // point: `r` is next to nothing dangerous, and the game is at stake.
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        let now = Stamp::ZERO;
        assert_eq!(app.restart.progress(now), None, "the key is up");

        app.key(&mut session, &press(Key::Char('r')), now);
        assert_eq!(app.restart.progress(now), Some(0));
        assert_eq!(app.restart.progress(now + RESTART_HOLD / 2), Some(50));
        assert!(!app.restart_due(now + RESTART_HOLD - Duration::from_millis(1)));
        assert!(app.restart_due(now + RESTART_HOLD));

        // Letting go cancels it (§10.3 step 4's rule, applied to a hold).
        app.key(&mut session, &release(Key::Char('r')), now);
        assert_eq!(app.restart.progress(now), None);
        assert!(!app.restart_due(now + RESTART_HOLD));
    }

    #[test]
    fn a_legacy_restart_hold_outlasts_the_first_auto_repeat() {
        // §8.2: there are no release events, so the hold ends when the key
        // falls quiet — and the window has to outlast the terminal's *first*
        // auto-repeat, which is far longer than the gap between later ones.
        let mut storage = Memory::new();
        let mut session = session(&mut storage);
        session.mode = InputMode::Legacy;
        let mut app = App::new(&session);
        let now = Stamp::ZERO;
        app.key(&mut session, &press(Key::Char('r')), now);
        assert!(
            RESTART_QUIET > crate::shell::input::HOLD_TIMEOUT,
            "a soft drop's 90 ms would drop the hold before the first repeat",
        );
        assert!(!app.restart_due(now + RESTART_QUIET - Duration::from_millis(1)));
        assert!(!app.restart_due(now + RESTART_QUIET), "silence releases it");
        assert_eq!(app.restart.progress(now + RESTART_QUIET), None);

        // A repeat before the window is out keeps the hold going, and the hold
        // is timed from the first press rather than the last repeat.
        let mut app = App::new(&session);
        app.key(&mut session, &press(Key::Char('r')), now);
        for step in 1..=3u32 {
            let at = now + RESTART_QUIET / 2 * step;
            assert!(app.restart_due(at) == (at >= now + RESTART_HOLD), "{step}");
            app.key(&mut session, &press(Key::Char('r')), at);
        }
    }

    #[test]
    fn a_pause_cancels_a_restart_in_flight() {
        // §9.17 stops the timers, and the restart hold is one of them.
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        let now = Stamp::ZERO;
        app.key(&mut session, &press(Key::Char('r')), now);
        app.key(&mut session, &press(Key::Esc), now);
        assert_eq!(app.restart.progress(now), None);
        assert!(!app.restart_due(now + RESTART_HOLD));
    }

    #[test]
    fn the_pause_menu_shows_the_controls_without_abandoning_the_game() {
        // §12.6 lists Controls beside Options, and §13.5 makes them the same
        // two boxes; a player checking which key holds must not lose the run
        // to do it.
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        let now = Stamp::ZERO;
        app.pause(0);
        for _ in 0..PauseChoice::Controls.index() {
            app.key(&mut session, &press(Key::Down), now);
        }
        assert_eq!(
            app.key(&mut session, &press(Key::Enter), now),
            Flow::Continue,
        );
        assert_eq!(app.phase, Phase::Controls);
        assert_eq!(app.overlay(now), Overlay::Controls);

        app.key(&mut session, &press(Key::Esc), now);
        assert_eq!(
            app.phase,
            Phase::Paused {
                selected: PauseChoice::Controls.index(),
            },
            "and back to the item that opened it",
        );
    }

    #[test]
    fn the_pause_menu_restarts_and_the_quit_key_goes_to_the_attract_screen() {
        // §7: `Playing` + quit -> `Attract` (abandoned, not scored); the pause
        // menu's Restart -> a fresh `Playing`.
        let mut storage = Memory::new();
        let (mut app, mut session) = app(&mut storage);
        let now = Stamp::ZERO;
        assert_eq!(
            app.key(&mut session, &press(Key::Char('q')), now),
            Flow::Leave(Next::Attract),
        );

        app.pause(0);
        app.key(&mut session, &press(Key::Down), now);
        assert_eq!(PauseChoice::ALL[1], PauseChoice::Restart);
        assert_eq!(
            app.key(&mut session, &press(Key::Enter), now),
            Flow::Leave(Next::Play),
        );
    }

    #[test]
    fn a_fresh_game_keeps_a_seeded_run_reproducible() {
        // §6.4: `--seed` is for reproducing a game, so every game of a seeded
        // run is the same one; an unseeded one asks the front-end for bits
        // afresh each time (`FRONTEND.md` F3).
        let mut storage = Memory::new();
        let mut session = session(&mut storage);
        assert!(session.seeded);
        assert_eq!(App::new(&session).view(), App::new(&session).view());

        session.seeded = false;
        session.host.seed = varying_seed;
        let mut seen = std::collections::HashSet::new();
        for _ in 0..8 {
            seen.insert(App::new(&session).view().next.clone());
        }
        assert!(seen.len() > 1, "eight games, all the same queue");
    }

    #[test]
    fn a_high_score_table_that_cannot_be_written_warns_rather_than_aborting() {
        // §14, §16: "any failure to write yields a warning at exit and is
        // otherwise ignored".
        let now = Stamp::ZERO;
        let mut storage = Unwritable;
        let mut session = session(&mut storage);
        session.seeded = false;
        let mut app = App::new(&session);
        scored(&mut app, now);
        app.phase = Phase::NameEntry { rank: 0 };
        app.key(&mut session, &press(Key::Enter), now);
        assert_eq!(session.scores.entries.len(), 1, "the table still took it");
        assert_eq!(session.warnings.len(), 1, "{:?}", session.warnings);
    }

    // -----------------------------------------------------------------------
    // The pump itself (§15.2, `FRONTEND.md` F7). What a front-end can reach
    // through it is `tests/pump.rs`'s business; these are the two answers that
    // need `Round`'s insides to be checked at all.

    #[test]
    fn the_deadline_is_the_rest_of_the_tick_and_never_a_frame_rate() {
        // §15.2 step 6: the advice is "you may wait this long without costing
        // a tick", so it is never zero straight after a pump and never longer
        // than one tick.
        let mut storage = Memory::new();
        let mut session = session(&mut storage);
        let start = Stamp::ZERO;
        let mut round = Round::new(&session, start);
        assert_eq!(round.deadline(start), TICK, "nothing banked yet");

        // Ten milliseconds in — under a tick, so nothing ran and the whole of
        // it is banked.
        let now = start + Duration::from_millis(10);
        round.advance(&mut session, now);
        assert_eq!(round.deadline(now), TICK - Duration::from_millis(10));

        // A frame that ran ticks leaves only the sub-tick remainder, so the
        // advice is always inside the tick and never nothing.
        let now = now + Duration::from_millis(100);
        round.advance(&mut session, now);
        assert!(round.deadline(now) > Duration::ZERO, "never a busy loop");
        assert!(round.deadline(now) <= TICK, "and never a frame rate");

        // And it counts down as real time passes, without another pump.
        assert_eq!(round.deadline(now + TICK), Duration::ZERO);
    }

    #[test]
    fn a_countdown_that_expired_this_frame_lets_the_frames_keys_through() {
        // §9.17's countdown is the one non-running phase that ends by itself,
        // and it ended *before* the event queue was drained when these steps
        // were the body of a loop. Pumping must not swallow the first key of
        // the resumed game.
        let mut storage = Memory::new();
        let mut session = session(&mut storage);
        let start = Stamp::ZERO;
        let mut round = Round::new(&session, start);
        round.app.phase = Phase::Resuming { since: start };

        let now = start + COUNTDOWN;
        assert_eq!(round.key(&mut session, &press(Key::Up), now), None);
        assert_ne!(
            round.app.pending.actions,
            Actions::default(),
            "the rotation reached the game, not the overlay",
        );
    }

    #[test]
    fn a_countdown_that_runs_out_without_the_keyboard_pauses_rather_than_plays() {
        // `GUI.md` §G4.7: a front-end that has lost the keyboard says so on
        // every pump. The countdown is not `Playing`, so there is nothing to
        // pause while it runs — but the pump it runs out in must pause the
        // game before `advance` plays a tick nobody can answer.
        let mut storage = Memory::new();
        let mut session = session(&mut storage);
        let start = Stamp::ZERO;
        let mut round = Round::new(&session, start);
        round.app.phase = Phase::Resuming { since: start };

        round.keyboard(false, start + COUNTDOWN / 2);
        assert_eq!(
            round.app.phase,
            Phase::Resuming { since: start },
            "the countdown carries on",
        );

        let now = start + COUNTDOWN;
        round.keyboard(false, now);
        assert_eq!(round.app.phase, Phase::Paused { selected: 0 });
        let before = round.frame(now).view;
        round.advance(&mut session, now + Duration::from_secs(1));
        assert_eq!(round.frame(now).view, before, "and nothing was played");
        assert!(!round.frame(now).cramped, "the screen is not replaced");
    }

    #[test]
    fn gravity_is_written_from_the_cores_own_integer() {
        // §9.9: no floating point in the rules, and none introduced here — the
        // read-out cannot disagree with the core about how fast a piece falls.
        assert_eq!(gravity(16), "0.016");
        assert_eq!(gravity(1_250), "1.250");
        assert_eq!(gravity(0), "0.000");
        assert_eq!(optional(None), "-");
        assert_eq!(optional(Some(30)), "30");
    }

    #[test]
    fn the_frame_rate_is_frames_drawn_not_frames_due() {
        // §12.4: the interesting number is how many frames were drawn, and
        // §15.2 step 5 skips a frame that would change nothing — so counting
        // is right and deriving it from the frame time is not.
        let start = Stamp::ZERO;
        let mut fps = Fps::new(start);
        assert_eq!(fps.drew(start), 0, "nothing to report in the first second");
        for _ in 0..40 {
            fps.drew(start + Duration::from_millis(500));
        }
        assert_eq!(fps.drew(start + Duration::from_secs(1)), 42);
        assert_eq!(
            fps.drew(start + Duration::from_millis(1_500)),
            42,
            "and it holds until the next second is up",
        );
    }
}
