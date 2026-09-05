//! Top-level application state machine (§7) and the fixed-timestep event loop
//! (§15.2).
//!
//! Two loops, because §15 specifies two: [`round`] runs a game at 60 Hz with an
//! accumulator (§15.2), and [`attract`] runs the front screen at 10 fps with
//! none (§15.3). [`Session`] is what sits above both — the config, the
//! high-score table and the warnings all outlive any one game, and the §13.5
//! Options panel is reachable from either screen.
//!
//! The loops are the only places that touch a clock: the core below them never
//! does (§3.1), the DAS/ARR machine above is handed a `Duration` rather than
//! reading one (§10.3), and the §12.5 animations get the same `Instant` the
//! frame was drawn at.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Size;

use crate::config::{self, ConfigFile, DisplaySettings, MAX_CATCH_UP_TICKS, Startup, TICK};
use crate::core::{Action, Actions, Game, GameEvent, GameView, Shift, TickInput};
use crate::highscore::{self, Entry};
use crate::input::{Bindings, InputMode, InputState};
use crate::ui::attract::{self, Attract};
use crate::ui::overlays::{NameEntry, PauseChoice, Setting};
use crate::ui::theme::{Glyphs, Theme};
use crate::ui::{self, Chrome, Cosmetics, Debug, Hud, Overlay, Tui};

/// §9.17: one second per number, three numbers.
const COUNTDOWN: Duration = Duration::from_secs(3);
/// §9.16: input is ignored for a second, so a keypress in flight at the moment
/// of death cannot dismiss the box before it has been read.
const GAME_OVER_LOCKOUT: Duration = Duration::from_secs(1);
/// §10.1: the restart key must be held this long before it takes effect.
const RESTART_HOLD: Duration = Duration::from_secs(1);
/// §15.3: the attract screen runs at 10 fps.
const ATTRACT_FRAME: Duration = Duration::from_millis(100);

/// Where the run goes next (§7).
///
/// The state machine is a loop over this: `Attract` and `Play` are the two
/// screens, and `Quit` is the only way out. A game never returns `Quit` —
/// §7 and §16 both send the quit key from `Playing` to the attract screen —
/// and it returns `Play` to mean "restart with a fresh game".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Attract,
    Play,
    Quit,
}

/// Where a game is (§7), narrowed to the phases a game can be in.
///
/// §7's `AppState` is two levels here rather than one: [`Next`] chooses the
/// screen and `Phase` says where inside a game it is. The phases that are not
/// in §7's list are the two that have nowhere else to live — the §13.5 Options
/// panel, which §12.6 draws over the paused playfield, and §9.17's resume
/// countdown, during which the clock is still stopped.
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
        since: Instant,
    },
    GameOver {
        since: Instant,
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

/// What the loop should do after a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flow {
    Continue,
    Leave(Next),
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

/// Everything on the playing screen that the loop compares between frames
/// (§15.2 step 5).
///
/// The loop draws only when this has changed, so anything the screen comes to
/// show that is in none of these fields will not be redrawn — and, worse, will
/// not be *erased*. `generation` is the escape hatch for what genuinely cannot
/// be a field (the Options panel's values); `restart` and `cramped` are here
/// because they are both on screen and in neither the view nor the overlay.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Frame {
    view: GameView,
    overlay: Overlay,
    generation: u32,
    /// §10.1's restart hold, as a percentage of the second it needs.
    restart: Option<u8>,
    /// §12.1: the terminal's size, while it is below the minimum. `None` is
    /// the ordinary case of a terminal with room for the screen.
    cramped: Option<Size>,
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
    since: Option<Instant>,
    last: Option<Instant>,
}

/// How long a restart hold survives silence in legacy mode (§8.2).
const RESTART_QUIET: Duration = Duration::from_millis(700);

impl Confirm {
    fn press(&mut self, now: Instant) {
        self.since.get_or_insert(now);
        self.last = Some(now);
    }

    fn release(&mut self) {
        *self = Self::default();
    }

    /// Age out a legacy-mode hold that has fallen quiet (§8.2). Never called in
    /// enhanced mode, where the release event is authoritative.
    fn expire(&mut self, now: Instant) {
        if self
            .last
            .is_some_and(|last| now.saturating_duration_since(last) >= RESTART_QUIET)
        {
            self.release();
        }
    }

    /// How far the hold has come, as a percentage, or `None` if the key is up.
    fn progress(&self, now: Instant) -> Option<u8> {
        let since = self.since?;
        let elapsed = now.saturating_duration_since(since);
        Some((elapsed.as_millis() * 100 / RESTART_HOLD.as_millis()).min(100) as u8)
    }

    fn done(&self, now: Instant) -> bool {
        self.progress(now) == Some(100)
    }
}

/// Everything that outlives one game (§7).
///
/// The config, the config path and §16's warnings are here rather than on
/// [`App`] because a run is not one game any more: the §13.5 Options panel is
/// reachable from the attract screen as well as from the pause menu, and the
/// high-score table is what the two screens have to agree about.
pub struct Session {
    /// The effective configuration (§6.1), which the Options panel edits in
    /// place and writes back on the way out.
    pub config: ConfigFile,
    config_path: Option<PathBuf>,
    scores: highscore::Table,
    scores_path: Option<PathBuf>,
    /// The entry the run that just finished added, highlighted by §13.5's
    /// high-score screen.
    recent: Option<usize>,
    /// §16's warning vector, carried for the length of the run so that a file
    /// that could not be written still reaches stderr at exit.
    warnings: Vec<String>,
    /// Whether the panel ever wrote the config, so §6.2's first-clean-exit
    /// write does not undo what the player just saved.
    saved: bool,
    mode: InputMode,
    /// §6.4: with `--seed` every game of the run is the same one, and none of
    /// them is recorded (§14).
    seed: u64,
    seeded: bool,
    /// §12.2's glyphs, interned once at start-up: the theme is `Copy` and
    /// carries them by reference for the life of the process.
    glyphs: Glyphs,
    /// Bumped whenever something the screen shows changes that is neither in
    /// the `GameView` nor in the `Overlay` — the Options panel's values are the
    /// only such thing so far. §15.2 step 5 compares frames to decide whether
    /// to draw at all, and is otherwise blind to them.
    generation: u32,
}

impl Session {
    /// Resolve what start-up found into what the two screens share.
    pub fn new(startup: &Startup, mode: InputMode) -> Self {
        let mut warnings = Vec::new();
        let scores_path = highscore::default_path();
        // §14: a table that cannot be read is an empty table and a warning; it
        // is never a reason not to start.
        let scores = highscore::load(scores_path.as_deref(), &mut warnings);
        Self {
            glyphs: Glyphs::configured(&startup.file.display),
            config: startup.file.clone(),
            config_path: startup.path.clone(),
            scores,
            scores_path,
            recent: None,
            warnings,
            saved: false,
            mode,
            seed: startup.seed,
            seeded: startup.seeded,
            generation: 0,
        }
    }

    /// The seed for the next game (§6.4).
    ///
    /// A seeded run replays the *same* game however many times it is restarted,
    /// which is what makes `--seed` useful; an unseeded one draws a fresh seed
    /// per game.
    fn next_seed(&self) -> u64 {
        if self.seeded {
            self.seed
        } else {
            rand::random()
        }
    }

    /// The presentation half of the `Chrome` (§12.4, §12.7).
    ///
    /// `hold_enabled` is the caller's answer rather than the config's: §13.5
    /// gives a running game the rules it started under, and the hold box's
    /// presence is a rule.
    fn chrome(&self, hold_enabled: bool) -> Chrome {
        chrome_for(&self.config.display, self.glyphs, hold_enabled)
    }

    /// §6.1: write the edited settings back. §16: an unwritable file never
    /// aborts — it adds a line to the warnings printed at exit.
    fn save_config(&mut self) {
        let Some(path) = self.config_path.as_deref() else {
            return;
        };
        match config::save(path, &self.config) {
            Ok(()) => self.saved = true,
            Err(error) => self.warn(format!("{}: {error}", path.display())),
        }
    }

    /// File a finished run (§14), reporting nothing: a score that did not make
    /// the table and a table that could not be written look the same from here.
    fn record(&mut self, name: &str, view: &GameView) {
        let entry = Entry::of(name, view, highscore::today());
        self.recent = self.scores.insert(entry);
        if self.recent.is_none() {
            return;
        }
        let Some(path) = self.scores_path.as_deref() else {
            return;
        };
        if let Err(error) = self.scores.save(path) {
            // §14: "any failure to write yields a warning at exit and is
            // otherwise ignored".
            self.warn(format!("{}: {error}", path.display()));
        }
    }

    /// Add a warning, once. A panel left twice over the same unwritable file
    /// should say so once, not twice.
    fn warn(&mut self, warning: String) {
        if !self.warnings.contains(&warning) {
            self.warnings.push(warning);
        }
    }

    /// Hand back what `main` prints after teardown (§16).
    fn finish(self, startup: &mut Startup) {
        startup.file = self.config;
        startup.wrote_config = self.saved;
        startup.warnings.extend(self.warnings);
    }
}

/// A game, its input state, and the bridge between them.
pub struct App {
    game: Game,
    input: InputState,
    bindings: Bindings,
    /// §13.5: the rules the game started under, whatever the config says now.
    hold_enabled: bool,
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
    /// Ticks abandoned because the loop fell more than `MAX_CATCH_UP_TICKS`
    /// behind (§15.2 step 4).
    dropped_ticks: u64,
}

impl App {
    pub fn new(session: &Session) -> Self {
        let (rules, presentation) = session.config.resolve();
        Self {
            bindings: Bindings::new(&presentation.keys, &rules),
            input: InputState::new(&rules, session.mode),
            hold_enabled: rules.hold_enabled,
            game: Game::new(rules, session.next_seed()),
            events: Vec::new(),
            pending: Pending::default(),
            phase: Phase::Playing,
            restart: Confirm::default(),
            name: NameEntry::prefilled(),
            dropped_ticks: 0,
        }
    }

    pub fn view(&self) -> GameView {
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
    fn overlay(&self, now: Instant) -> Overlay {
        match self.phase {
            Phase::Playing => Overlay::None,
            Phase::Paused { selected } => Overlay::Paused { selected },
            Phase::Options { selected } => Overlay::Options { selected },
            Phase::Resuming { since } => {
                let elapsed = now.saturating_duration_since(since);
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
    fn key(&mut self, session: &mut Session, event: &KeyEvent, now: Instant) -> Flow {
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
                let pressed = event.kind != KeyEventKind::Release;
                if pressed && now.saturating_duration_since(since) >= GAME_OVER_LOCKOUT {
                    return self.finish(session);
                }
                Flow::Continue
            }
            Phase::NameEntry { .. } => self.name_key(session, event),
        }
    }

    fn play_key(&mut self, event: &KeyEvent, now: Instant) -> Flow {
        // §10.1: the restart key is held rather than pressed, so it is taken
        // off the edge-triggered path before `InputState` sees it.
        if self.bindings.action_of(event) == Some(Action::Restart) {
            if event.kind == KeyEventKind::Release {
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
    fn options_key(&mut self, session: &mut Session, event: &KeyEvent, selected: usize) -> Flow {
        if event.kind == KeyEventKind::Release {
            return Flow::Continue;
        }
        let items = Setting::ALL.len();
        match event.code {
            KeyCode::Up => {
                self.phase = Phase::Options {
                    selected: (selected + items - 1) % items,
                }
            }
            KeyCode::Down => {
                self.phase = Phase::Options {
                    selected: (selected + 1) % items,
                }
            }
            KeyCode::Left | KeyCode::Right => {
                Setting::ALL[selected].step(&mut session.config, event.code == KeyCode::Right);
                session.generation += 1;
            }
            KeyCode::Esc | KeyCode::Enter => {
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
    fn name_key(&mut self, session: &mut Session, event: &KeyEvent) -> Flow {
        if event.kind == KeyEventKind::Release {
            return Flow::Continue;
        }
        // §16: Ctrl-C is a key event in raw mode, and it means leave — not a
        // `c` in the name field.
        if event.modifiers.contains(KeyModifiers::CONTROL) {
            if event.code == KeyCode::Char('c') {
                return Flow::Leave(Next::Attract);
            }
            return Flow::Continue;
        }
        match event.code {
            KeyCode::Char(c) => {
                self.name.push(c);
            }
            KeyCode::Backspace => {
                self.name.backspace();
            }
            KeyCode::Enter => {
                // An empty name becomes ANON, which `Entry::of` does for us.
                session.record(self.name.as_str(), &self.game.view());
                return Flow::Leave(Next::Attract);
            }
            KeyCode::Esc => return Flow::Leave(Next::Attract),
            _ => {}
        }
        Flow::Continue
    }

    /// §7: a finished game goes to name entry if the score qualifies and the
    /// run was not seeded, and to the attract screen otherwise.
    fn finish(&mut self, session: &Session) -> Flow {
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

    /// §8.4: a terminal that has shrunk below §12.1's minimum forces a game in
    /// progress into `Paused`, so the player is never killed by a window
    /// resize.
    ///
    /// Nothing here undoes itself when the terminal grows again: §9.17's pause
    /// is left for the player to leave, which also gives them the 3-2-1
    /// countdown before the clock starts again.
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
    fn restart_due(&mut self, now: Instant) -> bool {
        if self.input.mode() == InputMode::Legacy {
            self.restart.expire(now);
        }
        self.phase.running() && self.restart.done(now)
    }
}

/// Frames actually drawn in the last second (§12.4).
///
/// Counted rather than derived from the frame time, because §15.2 step 5 skips
/// a frame that would not change anything: the interesting number is how many
/// were drawn, not how fast one of them was.
#[derive(Debug)]
struct Fps {
    since: Instant,
    frames: u32,
    rate: u32,
}

impl Fps {
    fn new(now: Instant) -> Self {
        Self {
            since: now,
            frames: 0,
            rate: 0,
        }
    }

    /// Count one drawn frame and report the rate.
    fn drew(&mut self, now: Instant) -> u32 {
        self.frames += 1;
        if now.saturating_duration_since(self.since) >= Duration::from_secs(1) {
            self.rate = self.frames;
            self.frames = 0;
            self.since = now;
        }
        self.rate
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

/// The §7 state machine: attract, play, attract, until the player quits.
///
/// `startup` is borrowed rather than consumed because the §13.5 Options panel
/// edits the config in place and §16's warnings have to survive back to `main`,
/// which prints them after terminal teardown.
pub fn run(terminal: &mut Tui, startup: &mut Startup, mode: InputMode) -> Result<()> {
    let mut session = Session::new(startup, mode);
    let outcome = states(terminal, &mut session);
    session.finish(startup);
    outcome
}

/// §7's transitions, as a loop over [`Next`].
fn states(terminal: &mut Tui, session: &mut Session) -> Result<()> {
    let mut next = Next::Attract;
    loop {
        next = match next {
            Next::Attract => attract(terminal, session)?,
            Next::Play => round(terminal, session)?,
            Next::Quit => return Ok(()),
        };
    }
}

/// The attract screen's loop (§15.3): 10 fps, no accumulator, and a redraw only
/// when something moved.
fn attract(terminal: &mut Tui, session: &mut Session) -> Result<Next> {
    let mut state = Attract::new(Instant::now());
    let mut chrome = session.chrome(session.config.gameplay.hold_enabled);
    let mut dirty = true;
    // §8.4: the size is tracked from the resize events rather than asked for
    // every frame. It is the same answer, and it is the event that says the
    // whole frame is invalid.
    let mut area = terminal.size()?;
    loop {
        let now = Instant::now();
        // Drain every event that is already waiting; reading one per frame
        // would leave fast typing lagging behind.
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) => {
                    dirty = true;
                    match state.key(&key, &mut session.config, now) {
                        attract::Outcome::Stay => {}
                        attract::Outcome::Play => return Ok(Next::Play),
                        attract::Outcome::Quit => return Ok(Next::Quit),
                        // §13.5: presentation takes effect the moment the panel
                        // is left, and the config is written there and then.
                        attract::Outcome::OptionsClosed => {
                            session.save_config();
                            chrome = session.chrome(session.config.gameplay.hold_enabled);
                        }
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

        // §12.1: below the minimum the attract screen is replaced by the
        // message, and the drifting background is not stepped — there is
        // nowhere to draw it, and it would only make the frame look dirty.
        if !ui::fits(area) {
            if dirty {
                dirty = terminal
                    .draw(|frame| ui::too_small(frame, chrome.theme))
                    .is_err();
            }
            event::poll(ATTRACT_FRAME)?;
            continue;
        }

        let cells = (area.width / 2, area.height);
        // §13.4 is disabled in `mono` and when `show_debug` is on; asking the
        // theme rather than the config is what makes `NO_COLOR` count too.
        let animate = chrome.theme.depth() != crate::ui::theme::Depth::Mono
            && !session.config.display.show_debug;
        if state.step(now, cells, animate) || dirty {
            let context = attract::Context {
                chrome: &chrome,
                config: &session.config,
                scores: &session.scores,
                recent: session.recent,
                mode: session.mode,
            };
            // §16: a frame lost to a write failure is simply lost; leaving
            // `dirty` set is what makes the next frame try again.
            dirty = terminal
                .draw(|frame| attract::draw(frame, &state, &context))
                .is_err();
        }

        // §15.3: no accumulator, and a key wakes the loop early, so idle CPU
        // stays near zero while a keypress still lands within a frame.
        event::poll(ATTRACT_FRAME)?;
    }
}

/// One game (§15.2), from the first piece to the attract screen or a restart.
fn round(terminal: &mut Tui, session: &mut Session) -> Result<Next> {
    let show_debug = session.config.display.show_debug;
    let mut app = App::new(session);
    let clear_delay = TICK * session.config.resolve().0.line_clear_delay_ticks;
    let mut chrome = session.chrome(app.hold_enabled);
    let mut fx = Cosmetics::new(clear_delay, Instant::now());
    let mut previous: Option<Frame> = None;
    let mut accumulator = Duration::ZERO;
    let mut last = Instant::now();
    let mut fps = Fps::new(last);
    let mut settings = session.generation;
    // §8.4: tracked from the resize events, which are also what invalidates
    // the frame.
    let mut area = terminal.size()?;

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
        let mut leaving = None;
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) => {
                    if let Flow::Leave(next) = app.key(session, &key, now) {
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
        // behind a screen they cannot see.
        let room = ui::fits(area);
        if !room {
            app.cramp();
        }
        if let Some(next) = leaving {
            return Ok(next);
        }
        // §10.1: the restart key, once it has been held for its second.
        if app.restart_due(now) {
            return Ok(Next::Play);
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

        // §13.5: leaving the Options panel applies the presentation half at
        // once. The rules half is deliberately not applied — the game keeps
        // what it started under, which is also the only answer that leaves the
        // run deterministic (§15.4).
        if settings != session.generation {
            settings = session.generation;
            chrome = session.chrome(app.hold_enabled);
            invalidated = true;
        }

        // 6. Draw, but only when there is something new to look at: ratatui
        //    diffs against its previous buffer, so an unchanged frame is cheap,
        //    and skipping it entirely is cheaper. An animation in flight
        //    changes the screen without changing the view, so it counts as new.
        // §10.1's restart bar is the fourth component: it is on the screen and
        // in neither the view nor the overlay, so a frame that differs only by
        // it — the moment the key is let go, and the bar has to be rubbed out —
        // would otherwise not be drawn at all.
        let frame = Frame {
            view: app.view(),
            overlay: app.overlay(now),
            generation: session.generation,
            restart: app.restart.progress(now),
            // §12.1's message names the size it has, so the size is on the
            // screen and belongs in the comparison like everything else.
            cramped: (!room).then_some(area),
        };
        // The strip's own figures change every frame, so with it on there is
        // always something new to look at (§12.4).
        if invalidated || show_debug || fx.animating() || previous.as_ref() != Some(&frame) {
            let debug = show_debug.then(|| app.debug(fps.drew(now)));
            let hud = Hud {
                overlay: &frame.overlay,
                config: &session.config,
                debug: debug.as_ref(),
                mode: session.mode,
                restart: frame.restart,
            };
            // §16: a frame lost to a write failure is simply lost. Leaving
            // `previous` behind is what makes the next frame try again.
            if terminal
                .draw(|f| ui::draw(f, &frame.view, &chrome, &fx, &hud))
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

/// The presentation half of the `Chrome` (§12.4, §12.7).
fn chrome_for(display: &DisplaySettings, glyphs: Glyphs, hold_enabled: bool) -> Chrome {
    Chrome {
        theme: Theme::resolve(display.color_depth, glyphs),
        show_grid: display.show_grid,
        hold_enabled,
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

    /// A session with no files behind it: nothing here writes one unless the
    /// test hands it a path.
    fn session() -> Session {
        Session {
            config: ConfigFile::default(),
            config_path: None,
            scores: highscore::Table::default(),
            scores_path: None,
            recent: None,
            warnings: Vec::new(),
            saved: false,
            mode: InputMode::Enhanced,
            seed: 42,
            seeded: true,
            glyphs: Glyphs::DEFAULT,
            generation: 0,
        }
    }

    fn app() -> (App, Session) {
        let session = session();
        (App::new(&session), session)
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, crossterm::event::KeyModifiers::NONE)
    }

    #[test]
    fn a_frame_that_runs_no_ticks_keeps_the_input_it_resolved() {
        // §15.2 step 6: the loop wakes early on a key press, so a frame can
        // legitimately run zero ticks. Input the shell has already resolved
        // must survive until a tick consumes it, or every tap that lands
        // between two ticks -- which is most of them -- is silently lost.
        let (mut app, _session) = app();
        let now = Instant::now();
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
    fn a_terminal_below_the_minimum_forces_a_game_into_pause() {
        // §8.4: "if a game was in progress it is forced into `Paused` first,
        // so the player is never killed by a window resize". A soft drop that
        // was being held when the window shrank must not survive it either:
        // nothing expires a held key while the clock is stopped (§8.2).
        let (mut app, _session) = app();
        let now = Instant::now();
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
        let (mut app, _session) = app();
        let now = Instant::now();
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
        let (mut app, mut session) = app();
        let now = Instant::now();
        assert!(app.phase.running());

        assert_eq!(
            app.key(&mut session, &press(KeyCode::Esc), now),
            Flow::Continue
        );
        assert_eq!(app.phase, Phase::Paused { selected: 0 });
        assert!(!app.phase.running(), "the clock is stopped");

        // The core is not advanced at all while the clock is stopped.
        let before = app.view();
        app.advance(60, now);
        assert_eq!(app.view(), before, "a paused game does not tick");

        assert_eq!(
            app.key(&mut session, &press(KeyCode::Esc), now),
            Flow::Continue
        );
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
        let (mut app, mut session) = app();
        let now = Instant::now();
        app.pause(0);
        app.key(&mut session, &press(KeyCode::Down), now);
        assert_eq!(app.phase, Phase::Paused { selected: 1 });
        app.key(&mut session, &press(KeyCode::Up), now);
        app.key(&mut session, &press(KeyCode::Up), now);
        assert_eq!(
            app.phase,
            Phase::Paused {
                selected: PauseChoice::ALL.len() - 1
            },
            "up from the top wraps to Quit to menu",
        );
        assert_eq!(
            app.key(&mut session, &press(KeyCode::Enter), now),
            Flow::Leave(Next::Attract)
        );

        app.pause(0);
        assert_eq!(
            app.key(&mut session, &press(KeyCode::Enter), now),
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
        let (mut app, mut session) = app();
        let now = Instant::now();
        app.key(&mut session, &press(KeyCode::Up), now);
        assert_ne!(app.pending.actions, Actions::default());
        app.key(&mut session, &press(KeyCode::Esc), now);
        assert_eq!(app.pending.actions, Actions::default());
    }

    #[test]
    fn the_game_over_box_cannot_be_dismissed_for_a_second() {
        // §9.16: input is ignored for 1 s, so the keypress that killed you
        // does not also dismiss the box.
        let (mut app, mut session) = app();
        let now = Instant::now();
        app.phase = Phase::GameOver { since: now };
        assert_eq!(
            app.key(&mut session, &press(KeyCode::Char('x')), now),
            Flow::Continue
        );
        assert_eq!(
            app.key(
                &mut session,
                &press(KeyCode::Char('x')),
                now + Duration::from_millis(999)
            ),
            Flow::Continue,
        );
        assert_eq!(
            app.key(
                &mut session,
                &press(KeyCode::Char('x')),
                now + GAME_OVER_LOCKOUT
            ),
            Flow::Leave(Next::Attract),
        );
    }
    #[test]
    fn the_frame_rate_is_frames_drawn_not_frames_due() {
        // §12.4: the interesting number is how many frames were drawn, and
        // §15.2 step 5 skips a frame that would change nothing — so counting
        // is right and deriving it from the frame time is not.
        let start = Instant::now();
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
    #[test]
    fn the_pause_menu_opens_the_options_panel() {
        // §12.6 as amended, and §6.1's "in-game Options screen".
        let (mut app, mut session) = app();
        let now = Instant::now();
        app.pause(0);
        for _ in 0..2 {
            app.key(&mut session, &press(KeyCode::Down), now);
        }
        assert_eq!(
            PauseChoice::ALL[2],
            PauseChoice::Options,
            "third item down (§12.6)",
        );
        app.key(&mut session, &press(KeyCode::Enter), now);
        assert_eq!(app.phase, Phase::Options { selected: 0 });
    }

    #[test]
    fn the_options_panel_edits_and_writes_back_on_the_way_out() {
        // §13.5: `←`/`→` change the selected value, `Esc` saves and returns.
        // §6.1: "written back to the config file immediately on leaving".
        let dir = std::env::temp_dir().join("termino-options-panel-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(crate::config::FILE_NAME);
        let mut session = Session {
            config_path: Some(path.clone()),
            ..session()
        };
        let mut app = App::new(&session);
        let now = Instant::now();
        app.phase = Phase::Options { selected: 0 };

        app.key(&mut session, &press(KeyCode::Right), now);
        assert_eq!(session.config.gameplay.preview_count, 6);
        app.key(&mut session, &press(KeyCode::Down), now);
        app.key(&mut session, &press(KeyCode::Left), now);
        assert_eq!(
            session.config.gameplay.start_level, 15,
            "the second row, wrapping off the bottom",
        );
        assert!(!path.exists(), "nothing is written while the panel is up");

        assert_eq!(
            app.key(&mut session, &press(KeyCode::Esc), now),
            Flow::Continue
        );
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
        let written = crate::config::load(Some(&path), &mut warnings).file;
        assert_eq!(written, session.config);
        assert!(warnings.is_empty(), "{warnings:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_config_that_cannot_be_written_warns_rather_than_aborting() {
        // §16: recoverable problems degrade to a documented default and add a
        // line to the warnings printed at exit.
        let mut session = Session {
            // A path under a file rather than a directory: `create_dir_all`
            // cannot make it, whatever the platform.
            config_path: Some(PathBuf::from("/dev/null/termino/config.toml")),
            ..session()
        };
        let mut app = App::new(&session);
        app.phase = Phase::Options { selected: 0 };
        app.key(&mut session, &press(KeyCode::Esc), Instant::now());
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
    fn changing_a_value_makes_the_loop_redraw() {
        // §15.2 step 5 draws only when the frame changed, and compares the view
        // and the overlay — neither of which a panel value lives in.
        let (mut app, mut session) = app();
        let now = Instant::now();
        app.phase = Phase::Options { selected: 0 };
        let before = session.generation;
        app.key(&mut session, &press(KeyCode::Down), now);
        assert_eq!(
            session.generation, before,
            "moving the cursor changes the overlay"
        );
        app.key(&mut session, &press(KeyCode::Right), now);
        assert_ne!(session.generation, before, "changing a value does not");
    }

    fn release(code: KeyCode) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::NONE, KeyEventKind::Release)
    }

    /// Play one hard drop, so the run has a score worth recording (§9.14).
    fn scored(app: &mut App, now: Instant) -> u64 {
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
        let (mut app, mut session) = app();
        session.seeded = false;
        let now = Instant::now();
        scored(&mut app, now);
        app.phase = Phase::GameOver { since: now };
        assert_eq!(
            app.key(
                &mut session,
                &press(KeyCode::Char('x')),
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
        let (mut app, mut session) = app();
        assert!(session.seeded);
        let now = Instant::now();
        scored(&mut app, now);
        app.phase = Phase::GameOver { since: now };
        assert_eq!(
            app.key(
                &mut session,
                &press(KeyCode::Char('x')),
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
        let (mut app, mut session) = app();
        session.seeded = false;
        let now = Instant::now();
        assert_eq!(app.view().score, 0);
        app.phase = Phase::GameOver { since: now };
        assert_eq!(
            app.key(
                &mut session,
                &press(KeyCode::Char('x')),
                now + GAME_OVER_LOCKOUT
            ),
            Flow::Leave(Next::Attract),
        );
    }

    #[test]
    fn enter_records_the_score_and_esc_discards_it() {
        // §12.6: "`Enter` confirms ... `Esc` cancels and discards the score."
        let now = Instant::now();
        let (mut app, mut session) = app();
        session.seeded = false;
        let score = scored(&mut app, now);
        app.phase = Phase::NameEntry { rank: 0 };
        // Clear whatever `$USER` pre-filled, then type a name of our own.
        for _ in 0..crate::highscore::NAME_MAX {
            app.key(&mut session, &press(KeyCode::Backspace), now);
        }
        for c in "MS".chars() {
            app.key(&mut session, &press(KeyCode::Char(c)), now);
        }
        assert_eq!(
            app.key(&mut session, &press(KeyCode::Enter), now),
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
            app.key(&mut session, &press(KeyCode::Esc), now),
            Flow::Leave(Next::Attract),
        );
        assert_eq!(session.scores.entries.len(), 1, "nothing was added");
    }

    #[test]
    fn an_empty_name_becomes_anon() {
        // §12.6.
        let now = Instant::now();
        let (mut app, mut session) = app();
        session.seeded = false;
        scored(&mut app, now);
        app.phase = Phase::NameEntry { rank: 0 };
        for _ in 0..crate::highscore::NAME_MAX {
            app.key(&mut session, &press(KeyCode::Backspace), now);
        }
        app.key(&mut session, &press(KeyCode::Enter), now);
        assert_eq!(session.scores.entries[0].name, crate::highscore::ANONYMOUS);
    }

    #[test]
    fn the_restart_key_must_be_held_for_a_second() {
        // §10.1: "Restart (hold 1 s)". A tap does nothing, which is the whole
        // point: `r` is next to nothing dangerous, and the game is at stake.
        let (mut app, mut session) = app();
        let now = Instant::now();
        assert_eq!(app.restart.progress(now), None, "the key is up");

        app.key(&mut session, &press(KeyCode::Char('r')), now);
        assert_eq!(app.restart.progress(now), Some(0));
        assert_eq!(app.restart.progress(now + RESTART_HOLD / 2), Some(50));
        assert!(!app.restart_due(now + RESTART_HOLD - Duration::from_millis(1)));
        assert!(app.restart_due(now + RESTART_HOLD));

        // Letting go cancels it (§10.3 step 4's rule, applied to a hold).
        app.key(&mut session, &release(KeyCode::Char('r')), now);
        assert_eq!(app.restart.progress(now), None);
        assert!(!app.restart_due(now + RESTART_HOLD));
    }

    #[test]
    fn a_legacy_restart_hold_outlasts_the_first_auto_repeat() {
        // §8.2: there are no release events, so the hold ends when the key
        // falls quiet — and the window has to outlast the terminal's *first*
        // auto-repeat, which is far longer than the gap between later ones.
        let mut session = Session {
            mode: InputMode::Legacy,
            ..session()
        };
        let mut app = App::new(&session);
        let now = Instant::now();
        app.key(&mut session, &press(KeyCode::Char('r')), now);
        assert!(
            RESTART_QUIET > crate::input::HOLD_TIMEOUT,
            "a soft drop's 90 ms would drop the hold before the first repeat",
        );
        assert!(!app.restart_due(now + RESTART_QUIET - Duration::from_millis(1)));
        assert!(!app.restart_due(now + RESTART_QUIET), "silence releases it");
        assert_eq!(app.restart.progress(now + RESTART_QUIET), None);

        // A repeat before the window is out keeps the hold going, and the hold
        // is timed from the first press rather than the last repeat.
        let mut app = App::new(&session);
        app.key(&mut session, &press(KeyCode::Char('r')), now);
        for step in 1..=3u32 {
            let at = now + RESTART_QUIET / 2 * step;
            assert!(app.restart_due(at) == (at >= now + RESTART_HOLD), "{step}");
            app.key(&mut session, &press(KeyCode::Char('r')), at);
        }
    }

    #[test]
    fn a_pause_cancels_a_restart_in_flight() {
        // §9.17 stops the timers, and the restart hold is one of them.
        let (mut app, mut session) = app();
        let now = Instant::now();
        app.key(&mut session, &press(KeyCode::Char('r')), now);
        app.key(&mut session, &press(KeyCode::Esc), now);
        assert_eq!(app.restart.progress(now), None);
        assert!(!app.restart_due(now + RESTART_HOLD));
    }

    #[test]
    fn the_pause_menu_shows_the_controls_without_abandoning_the_game() {
        // §12.6 lists Controls beside Options, and §13.5 makes them the same
        // two boxes; a player checking which key holds must not lose the run
        // to do it.
        let (mut app, mut session) = app();
        let now = Instant::now();
        app.pause(0);
        for _ in 0..PauseChoice::Controls.index() {
            app.key(&mut session, &press(KeyCode::Down), now);
        }
        assert_eq!(
            app.key(&mut session, &press(KeyCode::Enter), now),
            Flow::Continue,
        );
        assert_eq!(app.phase, Phase::Controls);
        assert_eq!(app.overlay(now), Overlay::Controls);

        app.key(&mut session, &press(KeyCode::Esc), now);
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
        let (mut app, mut session) = app();
        let now = Instant::now();
        assert_eq!(
            app.key(&mut session, &press(KeyCode::Char('q')), now),
            Flow::Leave(Next::Attract),
        );

        app.pause(0);
        app.key(&mut session, &press(KeyCode::Down), now);
        assert_eq!(PauseChoice::ALL[1], PauseChoice::Restart);
        assert_eq!(
            app.key(&mut session, &press(KeyCode::Enter), now),
            Flow::Leave(Next::Play),
        );
    }

    #[test]
    fn a_fresh_game_keeps_a_seeded_run_reproducible() {
        // §6.4: `--seed` is for reproducing a game, so every game of a seeded
        // run is the same one; an unseeded run draws afresh each time.
        let session = session();
        assert!(session.seeded);
        assert_eq!(App::new(&session).view(), App::new(&session).view());

        let unseeded = Session {
            seeded: false,
            ..session
        };
        let mut seen = std::collections::HashSet::new();
        for _ in 0..8 {
            seen.insert(App::new(&unseeded).view().next.clone());
        }
        assert!(seen.len() > 1, "eight games, all the same queue");
    }

    #[test]
    fn a_high_score_table_that_cannot_be_written_warns_rather_than_aborting() {
        // §14, §16: "any failure to write yields a warning at exit and is
        // otherwise ignored".
        let now = Instant::now();
        let mut session = Session {
            seeded: false,
            // A path under a file rather than a directory (see the config test).
            scores_path: Some(PathBuf::from("/dev/null/termino/highscores.json")),
            ..session()
        };
        let mut app = App::new(&session);
        scored(&mut app, now);
        app.phase = Phase::NameEntry { rank: 0 };
        app.key(&mut session, &press(KeyCode::Enter), now);
        assert_eq!(session.scores.entries.len(), 1, "the table still took it");
        assert_eq!(session.warnings.len(), 1, "{:?}", session.warnings);
    }
}
