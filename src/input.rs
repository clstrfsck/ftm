//! Key decoding, action mapping and DAS/ARR (§10).
//!
//! DAS/ARR are resolved here in the shell, never in the core (§10.3): the core
//! is told only *which* direction and *how many* whole cells to shift this
//! tick. Disabled bindings are dropped at this boundary so they cannot reset a
//! lock-delay timer as a side effect (§10.1).
//!
//! Nothing here reads a clock. Elapsed time arrives as a `Duration` from the
//! caller, which is what lets §17.1's T13 exercise the whole machine without
//! one.

use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::config::{KeyBindings, RulesConfig, TICK};
use crate::core::matrix::WIDTH;
use crate::core::{Action, Shift};

/// How long a key is considered held after its last event in legacy mode
/// (§8.2). Longer than any common terminal auto-repeat interval (30-50 ms) and
/// shorter than a deliberate re-press.
pub const HOLD_TIMEOUT: Duration = Duration::from_millis(90);

/// The most cells one tick may shift. Ten is the width of the matrix (§9.1), so
/// it is already "as far as the piece can go" — the core stops at the wall.
const MAX_SHIFT_CELLS: u8 = WIDTH as u8;

/// How the terminal reports keys (§8.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    /// The kitty protocol is available: true press/release, and the terminal's
    /// own auto-repeat is discarded.
    Enhanced,
    /// Presses only. A key is held until [`HOLD_TIMEOUT`] passes without one.
    Legacy,
}

impl InputMode {
    /// The name shown by `--print-config` and the controls panel (§8.2).
    pub const fn name(self) -> &'static str {
        match self {
            InputMode::Enhanced => "enhanced",
            InputMode::Legacy => "legacy",
        }
    }
}

/// A level-triggered key: the three that feed `Held` rather than an `Action`
/// (§10.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeldKey {
    Left,
    Right,
    SoftDrop,
}

/// What one bound key does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bound {
    /// Edge-triggered: acted on once per press.
    Act(Action),
    /// Level-triggered: sampled every frame.
    Held(HeldKey),
}

/// The resolved `[keys]` table (§6.3, §10.1).
///
/// Bindings whose setting is off are simply absent, which is how §10.1's
/// "discarded before it reaches the action model" is implemented: an inert key
/// is a key this table does not contain.
#[derive(Clone, Debug, Default)]
pub struct Bindings {
    table: Vec<(KeyCode, Bound)>,
}

impl Bindings {
    /// Resolve the configured key names against the rules that gate them.
    ///
    /// A name that is not one of §10.1's is skipped; reporting it is the
    /// loader's job (§6.2).
    pub fn new(keys: &KeyBindings, rules: &RulesConfig) -> Self {
        let mut table = Vec::new();
        let mut bind = |names: &[String], bound: Bound| {
            for name in names {
                if let Some(code) = parse_key(name) {
                    table.push((code, bound));
                }
            }
        };
        bind(&keys.move_left, Bound::Held(HeldKey::Left));
        bind(&keys.move_right, Bound::Held(HeldKey::Right));
        bind(&keys.soft_drop, Bound::Held(HeldKey::SoftDrop));
        bind(&keys.hard_drop, Bound::Act(Action::HardDrop));
        bind(&keys.rotate_cw, Bound::Act(Action::RotateCw));
        bind(&keys.rotate_ccw, Bound::Act(Action::RotateCcw));
        if rules.allow_180_rotation {
            bind(&keys.rotate_180, Bound::Act(Action::Rotate180));
        }
        if rules.hold_enabled {
            bind(&keys.hold, Bound::Act(Action::Hold));
        }
        bind(&keys.pause, Bound::Act(Action::Pause));
        bind(&keys.restart, Bound::Act(Action::Restart));
        bind(&keys.quit, Bound::Act(Action::Quit));
        Self { table }
    }

    /// The edge-triggered action this key carries, whatever its kind.
    ///
    /// [`InputState::key`] fires an action on the press and says nothing on the
    /// release, which is right for every action but one: §10.1's restart has to
    /// be *held*, and a hold needs both edges. This is how the shell picks that
    /// key out before the action model sees it.
    pub fn action_of(&self, event: &KeyEvent) -> Option<Action> {
        match self.get(event)? {
            Bound::Act(action) => Some(action),
            Bound::Held(_) => None,
        }
    }

    /// What this key event is bound to, if anything.
    ///
    /// A key carrying Ctrl, Alt or Super is not a game binding: the §10.1 names
    /// are all bare keys, and Ctrl-C in particular means something else (§16).
    fn get(&self, event: &KeyEvent) -> Option<Bound> {
        const NOT_A_GAME_KEY: KeyModifiers = KeyModifiers::CONTROL
            .union(KeyModifiers::ALT)
            .union(KeyModifiers::SUPER);
        if event.modifiers.intersects(NOT_A_GAME_KEY) {
            return None;
        }
        self.table
            .iter()
            .find(|(code, _)| *code == event.code)
            .map(|(_, bound)| *bound)
    }
}

/// Whether `name` is a key name §10.1 recognises.
///
/// The grammar lives here, with the parser that owns it, and the config loader
/// asks rather than keeping a second copy that could drift (§6.2).
pub fn is_key_name(name: &str) -> bool {
    parse_key(name).is_some()
}

/// A key name from §10.1 as a `KeyCode`.
///
/// `Left`, `Right`, `Up`, `Down`, `Space`, `Enter`, `Tab`, `Esc`, `Backspace`,
/// `F1`-`F12`, and single characters, case-sensitive.
fn parse_key(name: &str) -> Option<KeyCode> {
    Some(match name {
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Space" => KeyCode::Char(' '),
        "Enter" => KeyCode::Enter,
        "Tab" => KeyCode::Tab,
        "Esc" => KeyCode::Esc,
        "Backspace" => KeyCode::Backspace,
        _ => {
            if let Some(number) = name.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
                if (1..=12).contains(&number) {
                    return Some(KeyCode::F(number));
                }
                return None;
            }
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => KeyCode::Char(c),
                _ => return None,
            }
        }
    })
}

/// One key's held state, and the DAS/ARR timers that go with it (§10.3).
///
/// `Default` is the released state, which is also §10.3 step 4: releasing
/// cancels both timers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Key {
    held: bool,
    /// Time held since the press that started it — the DAS charge.
    elapsed: Duration,
    /// Cells already emitted since the DAS charge completed. Counting what is
    /// *due* against what was emitted is what makes an `arr` shorter than a
    /// frame produce several cells in one tick without drifting.
    emitted: u32,
    /// The one immediate cell of §10.3 step 1, not yet delivered.
    initial: bool,
    /// Legacy mode (§8.2): time since the last event for this key.
    quiet: Duration,
}

impl Key {
    /// A press, or in legacy mode a terminal auto-repeat.
    ///
    /// Only the first press starts the timers: an auto-repeat must refresh the
    /// "still held" timestamp and nothing else, or the terminal's repeat rate
    /// would drive the game instead of the clock (§8.2).
    fn press(&mut self) {
        self.quiet = Duration::ZERO;
        if self.held {
            return;
        }
        *self = Self {
            held: true,
            initial: true,
            ..Self::default()
        };
    }

    fn release(&mut self) {
        *self = Self::default();
    }

    /// Age the legacy "still held" timer, releasing the key once it expires.
    fn expire(&mut self, dt: Duration) {
        if !self.held {
            return;
        }
        self.quiet += dt;
        if self.quiet >= HOLD_TIMEOUT {
            self.release();
        }
    }

    /// The cells this key asks for over `dt` (§10.3 steps 1-3).
    fn step(&mut self, dt: Duration, das: Duration, arr: Duration) -> u8 {
        if !self.held {
            return 0;
        }
        let mut cells = 0u32;
        if self.initial {
            self.initial = false;
            cells += 1;
        }
        self.elapsed += dt;
        if self.elapsed >= das {
            if arr.is_zero() {
                // §10.3 step 3: `arr = 0` means "move to the wall instantly".
                cells += u32::from(MAX_SHIFT_CELLS);
            } else {
                let due = ((self.elapsed - das).as_nanos() / arr.as_nanos()) as u32 + 1;
                cells += due.saturating_sub(self.emitted);
                self.emitted = due;
            }
        }
        cells.min(u32::from(MAX_SHIFT_CELLS)) as u8
    }
}

/// The shell's input state: what is held, and how much of it the core is owed
/// this tick (§10.2, §10.3).
#[derive(Clone, Debug)]
pub struct InputState {
    mode: InputMode,
    /// §10.3, in wall-clock time. The config resolved these to whole ticks once
    /// (§6.6), so this is the tick grid expressed as a duration, and two peers
    /// with the same `[timing]` table charge DAS at the same rate.
    das: Duration,
    arr: Duration,
    left: Key,
    right: Key,
    soft_drop: Key,
    /// The most recently pressed direction, which wins while both are held
    /// (§10.3).
    priority: Option<Shift>,
}

impl InputState {
    pub fn new(rules: &RulesConfig, mode: InputMode) -> Self {
        Self {
            mode,
            das: TICK * rules.das_ticks,
            arr: TICK * rules.arr_ticks,
            left: Key::default(),
            right: Key::default(),
            soft_drop: Key::default(),
            priority: None,
        }
    }

    pub fn mode(&self) -> InputMode {
        self.mode
    }

    /// Whether the soft-drop key is down now (§10.2).
    pub fn soft_drop(&self) -> bool {
        self.soft_drop.held
    }

    /// Fold one key event into the state, reporting the edge-triggered action
    /// it produced.
    pub fn key(&mut self, event: &KeyEvent, bindings: &Bindings) -> Option<Action> {
        if event.kind == KeyEventKind::Release {
            // Only held keys care about a release; an action already fired on
            // its press.
            if let Some(Bound::Held(key)) = bindings.get(event) {
                self.key_mut(key).release();
            }
            return None;
        }
        // §16: SIGINT is not trapped — raw mode delivers it as a key event, and
        // it means quit. This is checked before the bindings because Ctrl-C
        // would otherwise land on whatever `c` is bound to.
        if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('c') {
            return Some(Action::Quit);
        }
        if event.kind == KeyEventKind::Repeat && self.mode == InputMode::Enhanced {
            // §8.2: with true release events, the terminal's auto-repeat is
            // noise — DAS is driven by the clock alone.
            return None;
        }
        match bindings.get(event)? {
            Bound::Act(action) => Some(action),
            Bound::Held(key) => {
                if let Some(direction) = shift_of(key) {
                    if !self.key_mut(key).held {
                        self.priority = Some(direction);
                    }
                }
                self.key_mut(key).press();
                None
            }
        }
    }

    /// What this key is bound to, without folding it into the held state.
    ///
    /// The overlays of §12.6 need to recognise the pause key without letting a
    /// stray direction charge DAS behind a menu (§10.4).
    pub fn binding(&self, event: &KeyEvent, bindings: &Bindings) -> Option<Action> {
        if event.kind == KeyEventKind::Release {
            return None;
        }
        match bindings.get(event)? {
            Bound::Act(action) => Some(action),
            Bound::Held(_) => None,
        }
    }

    /// Let go of every held key.
    ///
    /// §9.17 stops the timers, and in legacy mode nothing else would: a key is
    /// held until it falls quiet (§8.2), and `expire` only runs while the game
    /// does.
    pub fn release_all(&mut self) {
        self.left = Key::default();
        self.right = Key::default();
        self.soft_drop = Key::default();
        self.priority = None;
    }

    /// Resolve DAS/ARR over `dt` into the direction to shift and the whole
    /// cells owed (§10.3, §15.2 step 3).
    pub fn resolve(&mut self, dt: Duration) -> (Option<Shift>, u8) {
        if self.mode == InputMode::Legacy {
            self.left.expire(dt);
            self.right.expire(dt);
            self.soft_drop.expire(dt);
        }
        // Both directions are stepped even when only one is acted on, so the
        // loser's DAS state is preserved rather than restarted when the winner
        // is released while it is still held (§10.3).
        let left = self.left.step(dt, self.das, self.arr);
        let right = self.right.step(dt, self.das, self.arr);
        match self.active() {
            Some(Shift::Left) => (Some(Shift::Left), left),
            Some(Shift::Right) => (Some(Shift::Right), right),
            None => (None, 0),
        }
    }

    /// How far the active direction's DAS charge has come, as a percentage,
    /// for §12.4's debug strip. Zero when nothing is held.
    pub fn das_charge(&self) -> u8 {
        let key = match self.active() {
            Some(Shift::Left) => &self.left,
            Some(Shift::Right) => &self.right,
            None => return 0,
        };
        if self.das.is_zero() {
            return 100;
        }
        ((key.elapsed.as_nanos() * 100 / self.das.as_nanos()).min(100)) as u8
    }

    /// The direction in force: the most recently pressed one that is still
    /// held (§10.3).
    fn active(&self) -> Option<Shift> {
        match self.priority {
            Some(Shift::Left) if self.left.held => Some(Shift::Left),
            Some(Shift::Right) if self.right.held => Some(Shift::Right),
            _ if self.left.held => Some(Shift::Left),
            _ if self.right.held => Some(Shift::Right),
            _ => None,
        }
    }

    fn key_mut(&mut self, key: HeldKey) -> &mut Key {
        match key {
            HeldKey::Left => &mut self.left,
            HeldKey::Right => &mut self.right,
            HeldKey::SoftDrop => &mut self.soft_drop,
        }
    }
}

const fn shift_of(key: HeldKey) -> Option<Shift> {
    match key {
        HeldKey::Left => Some(Shift::Left),
        HeldKey::Right => Some(Shift::Right),
        HeldKey::SoftDrop => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GameplaySettings, KeyBindings, TimingSettings};

    /// The default rules, so `das_ticks = 10` and `arr_ticks = 2` (§6.6).
    fn rules() -> RulesConfig {
        RulesConfig::default()
    }

    fn state(mode: InputMode) -> (InputState, Bindings) {
        let rules = rules();
        let bindings = Bindings::new(&KeyBindings::default(), &rules);
        (InputState::new(&rules, mode), bindings)
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn kind(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind)
    }

    /// One frame of exactly one tick, the pacing of §15.2 on an idle machine.
    fn frame(input: &mut InputState) -> (Option<Shift>, u8) {
        input.resolve(TICK)
    }

    /// `frames` frames, each reported as the cells it asked for.
    fn frames(input: &mut InputState, count: usize) -> Vec<u8> {
        (0..count).map(|_| frame(input).1).collect()
    }

    #[test]
    fn one_cell_on_press_then_nothing_until_das() {
        // T13, §10.3 steps 1-2. das_ticks = 10, so the charge completes on the
        // tenth frame after the press and the piece does not move before it.
        let (mut input, bindings) = state(InputMode::Enhanced);
        input.key(&press(KeyCode::Left), &bindings);
        assert_eq!(
            frame(&mut input),
            (Some(Shift::Left), 1),
            "the immediate cell"
        );
        assert_eq!(frames(&mut input, 8), vec![0; 8], "nothing until das_ms");
    }

    #[test]
    fn after_das_one_cell_per_arr() {
        // T13, §10.3 step 3. arr_ticks = 2: a cell on the tick DAS completes,
        // then one every second tick, and never one in between.
        let (mut input, bindings) = state(InputMode::Enhanced);
        input.key(&press(KeyCode::Right), &bindings);
        assert_eq!(frames(&mut input, 9), vec![1, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            frames(&mut input, 8),
            vec![1, 0, 1, 0, 1, 0, 1, 0],
            "the tenth frame charges DAS, then one cell every two ticks",
        );
        assert_eq!(frame(&mut input).0, Some(Shift::Right));
    }

    #[test]
    fn a_short_arr_delivers_several_cells_in_one_tick() {
        // §10.3 step 3: fractional time is accumulated, so an `arr` finer than
        // a frame is not rounded up to one cell per frame. Half a tick of ARR
        // owes two cells per tick.
        let rules = RulesConfig::from_settings(
            &GameplaySettings::default(),
            &TimingSettings {
                arr_ms: 8, // half a tick, which rounds to 0 ticks...
                ..TimingSettings::default()
            },
        );
        assert_eq!(rules.arr_ticks, 0, "...and 0 ticks means the wall (§10.3)");
        // A whole-tick ARR is the finest the config can express (§6.6), so the
        // interesting case is proved directly on the timer instead.
        let mut key = Key::default();
        key.press();
        let das = TICK * 10;
        let arr = TICK / 2;
        assert_eq!(key.step(TICK, das, arr), 1);
        let _ = key.step(TICK * 9, das, arr);
        assert_eq!(key.step(TICK, das, arr), 2, "two cells in one tick");
    }

    #[test]
    fn an_arr_of_zero_slides_to_the_wall() {
        // T13: `arr_ms = 0` means "move to the wall instantly" (§10.3 step 3).
        // Ten cells is the width of the matrix, so the core stops at the wall.
        let rules = RulesConfig::from_settings(
            &GameplaySettings::default(),
            &TimingSettings {
                arr_ms: 0,
                ..TimingSettings::default()
            },
        );
        assert_eq!(rules.arr_ticks, 0);
        let bindings = Bindings::new(&KeyBindings::default(), &rules);
        let mut input = InputState::new(&rules, InputMode::Enhanced);
        input.key(&press(KeyCode::Left), &bindings);
        assert_eq!(frames(&mut input, 9), vec![1, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(frame(&mut input).1, 10, "the whole board in one tick");
        assert_eq!(frame(&mut input).1, 10, "and it stays against the wall");
    }

    #[test]
    fn the_most_recently_pressed_direction_wins() {
        // T13, §10.3. Left is fully charged and repeating when right is
        // pressed; right takes over with its own immediate cell.
        let (mut input, bindings) = state(InputMode::Enhanced);
        input.key(&press(KeyCode::Left), &bindings);
        let _ = frames(&mut input, 12);
        input.key(&press(KeyCode::Right), &bindings);
        assert_eq!(frame(&mut input), (Some(Shift::Right), 1));
        assert_eq!(frames(&mut input, 4), vec![0; 4], "right is charging DAS");
    }

    #[test]
    fn the_loser_keeps_its_das_charge() {
        // §10.3: "its DAS state is preserved rather than restarted when the
        // other key is released while it is still held". Left must resume
        // repeating at once, not serve another full DAS charge.
        let (mut input, bindings) = state(InputMode::Enhanced);
        input.key(&press(KeyCode::Left), &bindings);
        let _ = frames(&mut input, 12);
        input.key(&press(KeyCode::Right), &bindings);
        let _ = frames(&mut input, 3);
        input.key(&kind(KeyCode::Right, KeyEventKind::Release), &bindings);

        let resumed = frames(&mut input, 3);
        assert_eq!(input.active(), Some(Shift::Left));
        assert!(
            resumed.iter().sum::<u8>() > 0,
            "left resumed at its ARR rate, not after a fresh DAS: {resumed:?}",
        );
    }

    #[test]
    fn releasing_cancels_both_timers() {
        // §10.3 step 4. The next press starts from scratch: one cell, then a
        // full DAS charge.
        let (mut input, bindings) = state(InputMode::Enhanced);
        input.key(&press(KeyCode::Left), &bindings);
        let _ = frames(&mut input, 12);
        input.key(&kind(KeyCode::Left, KeyEventKind::Release), &bindings);
        assert_eq!(frame(&mut input), (None, 0));
        input.key(&press(KeyCode::Left), &bindings);
        assert_eq!(frames(&mut input, 9), vec![1, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn enhanced_mode_ignores_the_terminals_auto_repeat() {
        // §8.2: with true release events, DAS is driven entirely by the game
        // clock and `KeyEventKind::Repeat` is discarded — a repeat must not
        // restart the charge or add a cell.
        let (mut input, bindings) = state(InputMode::Enhanced);
        input.key(&press(KeyCode::Left), &bindings);
        assert_eq!(frame(&mut input).1, 1);
        for _ in 0..8 {
            input.key(&kind(KeyCode::Left, KeyEventKind::Repeat), &bindings);
            assert_eq!(frame(&mut input).1, 0);
        }
        assert_eq!(frame(&mut input).1, 1, "DAS completed on the clock");
    }

    #[test]
    fn legacy_mode_holds_a_key_until_the_timeout() {
        // §8.2: no release events, so a key is held from its first press until
        // `hold_timeout` has passed with no further event for it.
        let (mut input, bindings) = state(InputMode::Legacy);
        input.key(&press(KeyCode::Down), &bindings);
        assert!(input.soft_drop());
        // Auto-repeat at a typical 40 ms keeps it held indefinitely.
        for _ in 0..10 {
            let _ = input.resolve(Duration::from_millis(40));
            input.key(&press(KeyCode::Down), &bindings);
            assert!(input.soft_drop());
        }
        let _ = input.resolve(HOLD_TIMEOUT);
        assert!(
            !input.soft_drop(),
            "released {HOLD_TIMEOUT:?} after the last event"
        );
    }

    #[test]
    fn a_legacy_repeat_refreshes_the_hold_without_restarting_das() {
        // §8.2: "incoming repeat events only refresh the still-held timestamp".
        // The repeats must not re-arm the immediate cell of §10.3 step 1.
        let (mut input, bindings) = state(InputMode::Legacy);
        input.key(&press(KeyCode::Left), &bindings);
        let mut cells = vec![frame(&mut input).1];
        for _ in 0..11 {
            input.key(&press(KeyCode::Left), &bindings);
            cells.push(frame(&mut input).1);
        }
        assert_eq!(cells, vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1]);
    }

    #[test]
    fn the_key_names_of_the_spec_all_parse() {
        // §10.1's list, exactly.
        for (name, code) in [
            ("Left", KeyCode::Left),
            ("Right", KeyCode::Right),
            ("Up", KeyCode::Up),
            ("Down", KeyCode::Down),
            ("Space", KeyCode::Char(' ')),
            ("Enter", KeyCode::Enter),
            ("Tab", KeyCode::Tab),
            ("Esc", KeyCode::Esc),
            ("Backspace", KeyCode::Backspace),
            ("F1", KeyCode::F(1)),
            ("F12", KeyCode::F(12)),
            ("z", KeyCode::Char('z')),
            ("Z", KeyCode::Char('Z')),
            ("F", KeyCode::Char('F')),
        ] {
            assert_eq!(parse_key(name), Some(code), "{name}");
        }
        for name in ["", "F0", "F13", "Ctrl", "left", "PageUp"] {
            assert_eq!(parse_key(name), None, "{name}");
        }
    }

    #[test]
    fn the_default_bindings_are_the_ones_in_the_spec() {
        let (mut input, bindings) = state(InputMode::Enhanced);
        for (code, action) in [
            (KeyCode::Char(' '), Action::HardDrop),
            (KeyCode::Up, Action::RotateCw),
            (KeyCode::Char('z'), Action::RotateCcw),
            (KeyCode::Char('Z'), Action::RotateCcw),
            (KeyCode::Char('a'), Action::Rotate180),
            (KeyCode::Char('c'), Action::Hold),
            (KeyCode::Esc, Action::Pause),
            (KeyCode::F(1), Action::Pause),
            (KeyCode::Char('r'), Action::Restart),
            (KeyCode::Char('q'), Action::Quit),
        ] {
            assert_eq!(input.key(&press(code), &bindings), Some(action), "{code:?}");
        }
        assert_eq!(input.key(&press(KeyCode::Char('x')), &bindings), None);
    }

    #[test]
    fn a_disabled_key_never_reaches_the_action_model() {
        // §10.1: an inert binding is discarded at this boundary, so it cannot
        // reset a lock-delay timer or dismiss a banner as a side effect.
        let rules = RulesConfig::from_settings(
            &GameplaySettings {
                hold_enabled: false,
                allow_180_rotation: false,
                ..GameplaySettings::default()
            },
            &TimingSettings::default(),
        );
        let bindings = Bindings::new(&KeyBindings::default(), &rules);
        let mut input = InputState::new(&rules, InputMode::Enhanced);
        assert_eq!(input.key(&press(KeyCode::Char('c')), &bindings), None);
        assert_eq!(input.key(&press(KeyCode::Char('a')), &bindings), None);
        assert_eq!(
            input.key(&press(KeyCode::Up), &bindings),
            Some(Action::RotateCw),
            "the rest of the keyboard is unaffected",
        );
    }

    #[test]
    fn ctrl_c_quits_rather_than_holding() {
        // §16: SIGINT is not trapped; raw mode delivers it as a key event. `c`
        // is bound to hold by default, so the check has to come first.
        let (mut input, bindings) = state(InputMode::Enhanced);
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(input.key(&ctrl_c, &bindings), Some(Action::Quit));
        // Other modified keys are simply not game bindings.
        let ctrl_left = KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL);
        assert_eq!(input.key(&ctrl_left, &bindings), None);
        assert_eq!(frame(&mut input), (None, 0));
    }
}
