//! Configuration: the on-disk schema (§6.3), the two resolved settings structs
//! (§6.5) and the millisecond-to-tick conversion (§6.6).
//!
//! Three layers, in the order the data moves through them:
//!
//! 1. [`ConfigFile`] — the TOML document of §6.3, in the units a human types.
//! 2. [`RulesConfig`] and [`PresentationConfig`] — what the rest of the program
//!    reads. They stay **separate structs** (§6.5): under §19 the rules class
//!    becomes server-authoritative while presentation stays local, and a single
//!    flat `Config` would have to be torn apart later.
//! 3. The core, which sees `RulesConfig` only, in ticks — never milliseconds.
//!
//! The conversion in step 2 happens exactly once, so two peers with the same
//! `[timing]` table derive the same tick counts on any platform (§6.6).

use std::fs;
use std::io;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::time::Duration;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::input::is_key_name;

/// Ticks per second. The core advances only in whole ticks (§15.1).
pub const TICK_HZ: u64 = 60;
/// The duration of one tick.
pub const TICK: Duration = Duration::from_nanos(1_000_000_000 / TICK_HZ);
/// The most ticks one frame may catch up, 100 ms of arrears (§15.1). Beyond
/// this the shell discards the backlog rather than sprinting through it.
pub const MAX_CATCH_UP_TICKS: u32 = 6;

/// The inclusive ranges of §6.3. A numeric value outside its range is clamped
/// to the nearest end and reported in the §6.2 warning; the lower bounds of
/// `LINES_PER_LEVEL` and `SOFT_DROP_FACTOR` are load-bearing, since both values
/// are divisors.
pub mod range {
    use std::ops::RangeInclusive;

    /// Upper bound is the height of the next box in §12.4.
    pub const PREVIEW_COUNT: RangeInclusive<u8> = 1..=6;
    /// The levels the §9.9 speed curve is defined for.
    pub const START_LEVEL: RangeInclusive<u32> = 1..=15;
    pub const LINES_PER_LEVEL: RangeInclusive<u32> = 1..=1000;
    pub const LOCK_DELAY_MS: RangeInclusive<u32> = 0..=5000;
    pub const DAS_MS: RangeInclusive<u32> = 0..=1000;
    pub const ARR_MS: RangeInclusive<u32> = 0..=1000;
    pub const SOFT_DROP_FACTOR: RangeInclusive<u32> = 1..=100;
    pub const LINE_CLEAR_DELAY_MS: RangeInclusive<u32> = 0..=2000;
    pub const ENTRY_DELAY_MS: RangeInclusive<u32> = 0..=2000;
}

/// Clamp to an inclusive range from §6.3.
fn clamped<T: Ord + Copy>(value: T, range: &RangeInclusive<T>) -> T {
    value.clamp(*range.start(), *range.end())
}

/// Convert milliseconds to whole ticks, rounding to nearest (§6.6).
///
/// `ticks = round(ms * 60 / 1000)`. Integer arithmetic throughout: the rules
/// carry no floating point (§9.9), and rounding must not vary by platform.
///
/// Use this only for values that may legitimately be zero — `entry_delay_ms` and
/// `arr_ms`, both meaning "same tick". Everything else uses
/// [`ms_to_ticks_at_least_one`].
pub const fn ms_to_ticks(ms: u32) -> u32 {
    ((ms as u64 * TICK_HZ + 500) / 1000) as u32
}

/// Convert milliseconds to whole ticks, never yielding zero (§6.6).
///
/// `ticks = max(1, round(ms * 60 / 1000))`. A delay a player asked for must not
/// vanish because it rounded down: `lock_delay_ms = 1` is 1 tick, not 0.
pub const fn ms_to_ticks_at_least_one(ms: u32) -> u32 {
    let ticks = ms_to_ticks(ms);
    if ticks == 0 { 1 } else { ticks }
}

/// The lock-down rule (§9.11).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LockDownRule {
    /// Extended placement: 15 resets, restored on reaching a new lowest row.
    #[default]
    Extended,
    /// As extended, but the reset count is never capped.
    Infinite,
    /// The timer is set on landing and never reset by moves or rotations.
    Classic,
}

/// The requested colour depth (§12.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorDepth {
    /// Detect from `$COLORTERM`, `$TERM` and `$NO_COLOR` (§12.3).
    #[default]
    Auto,
    Truecolor,
    #[serde(rename = "256")]
    Ansi256,
    #[serde(rename = "16")]
    Ansi16,
    Mono,
}

/// The `[gameplay]` table of §6.3.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameplaySettings {
    pub preview_count: u8,
    pub ghost_piece: bool,
    pub hold_enabled: bool,
    pub lock_down: LockDownRule,
    pub start_level: u32,
    pub lines_per_level: u32,
    pub allow_180_rotation: bool,
}

impl Default for GameplaySettings {
    fn default() -> Self {
        Self {
            preview_count: 5,
            ghost_piece: true,
            hold_enabled: true,
            lock_down: LockDownRule::Extended,
            start_level: 1,
            lines_per_level: 10,
            allow_180_rotation: true,
        }
    }
}

/// The `[timing]` table of §6.3, in milliseconds as written by a human.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimingSettings {
    pub lock_delay_ms: u32,
    pub das_ms: u32,
    pub arr_ms: u32,
    pub soft_drop_factor: u32,
    pub line_clear_delay_ms: u32,
    pub entry_delay_ms: u32,
}

impl Default for TimingSettings {
    fn default() -> Self {
        Self {
            lock_delay_ms: 500,
            das_ms: 170,
            arr_ms: 30,
            soft_drop_factor: 20,
            line_clear_delay_ms: 250,
            entry_delay_ms: 0,
        }
    }
}

/// The `[display]` table of §6.3.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    pub color_depth: ColorDepth,
    pub cell_filled: String,
    pub cell_empty: String,
    pub cell_ghost: String,
    pub show_grid: bool,
    pub show_debug: bool,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            color_depth: ColorDepth::Auto,
            cell_filled: "██".to_string(),
            cell_empty: "  ".to_string(),
            cell_ghost: "▒▒".to_string(),
            show_grid: false,
            show_debug: false,
        }
    }
}

/// The `[keys]` table of §6.3. Each action maps to a list of key names; any
/// listed key triggers it (§10.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyBindings {
    pub move_left: Vec<String>,
    pub move_right: Vec<String>,
    pub soft_drop: Vec<String>,
    pub hard_drop: Vec<String>,
    pub rotate_cw: Vec<String>,
    pub rotate_ccw: Vec<String>,
    pub rotate_180: Vec<String>,
    pub hold: Vec<String>,
    pub pause: Vec<String>,
    pub quit: Vec<String>,
    pub restart: Vec<String>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        let names = |keys: &[&str]| keys.iter().map(|k| k.to_string()).collect();
        Self {
            move_left: names(&["Left"]),
            move_right: names(&["Right"]),
            soft_drop: names(&["Down"]),
            hard_drop: names(&["Space"]),
            rotate_cw: names(&["Up"]),
            rotate_ccw: names(&["z", "Z"]),
            rotate_180: names(&["a", "A"]),
            hold: names(&["c", "C"]),
            pause: names(&["Esc", "F1"]),
            quit: names(&["q", "Q"]),
            restart: names(&["r", "R"]),
        }
    }
}

impl KeyBindings {
    /// Every action and its bound key names, in the order §6.3 writes them.
    ///
    /// The loader and the document writer both need to walk the table by name;
    /// spelling the eleven actions out twice is how a twelfth would go missing.
    pub fn each(&self) -> [(&'static str, &Vec<String>); 11] {
        [
            ("move_left", &self.move_left),
            ("move_right", &self.move_right),
            ("soft_drop", &self.soft_drop),
            ("hard_drop", &self.hard_drop),
            ("rotate_cw", &self.rotate_cw),
            ("rotate_ccw", &self.rotate_ccw),
            ("rotate_180", &self.rotate_180),
            ("hold", &self.hold),
            ("pause", &self.pause),
            ("quit", &self.quit),
            ("restart", &self.restart),
        ]
    }

    /// The same, for the loader to fill in.
    fn each_mut(&mut self) -> [(&'static str, &mut Vec<String>); 11] {
        [
            ("move_left", &mut self.move_left),
            ("move_right", &mut self.move_right),
            ("soft_drop", &mut self.soft_drop),
            ("hard_drop", &mut self.hard_drop),
            ("rotate_cw", &mut self.rotate_cw),
            ("rotate_ccw", &mut self.rotate_ccw),
            ("rotate_180", &mut self.rotate_180),
            ("hold", &mut self.hold),
            ("pause", &mut self.pause),
            ("quit", &mut self.quit),
            ("restart", &mut self.restart),
        ]
    }
}

/// The config file as written on disk (§6.3).
///
/// Every table is `#[serde(default)]` and none denies unknown fields: §6.2
/// requires a missing or unrecognised key to be tolerated, not fatal. A file
/// listing one key the player invented still supplies the other ten.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigFile {
    pub gameplay: GameplaySettings,
    pub timing: TimingSettings,
    pub display: DisplaySettings,
    pub keys: KeyBindings,
}

/// Settings that change **what happens** (§6.5): the `[gameplay]` and
/// `[timing]` classes, with every duration already in ticks.
///
/// The core sees this and nothing else. Two peers running the same
/// `RulesConfig` and seed must produce identical games, which is why it is
/// `PartialEq` and why it holds no milliseconds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulesConfig {
    pub preview_count: u8,
    pub ghost_piece: bool,
    pub hold_enabled: bool,
    pub lock_down: LockDownRule,
    pub start_level: u32,
    pub lines_per_level: u32,
    pub allow_180_rotation: bool,
    /// §9.11. Never zero.
    pub lock_delay_ticks: u32,
    /// §10.3. Consumed by the shell, not the core, but a rules-class setting by
    /// §6.5 — the class is a property of the setting, not of who reads it.
    pub das_ticks: u32,
    /// §10.3. May be 0, meaning "move to the wall instantly"; one tick is the
    /// finest repeat the tick grid can express.
    pub arr_ticks: u32,
    /// Soft drop is this many times normal gravity (§9.10).
    pub soft_drop_factor: u32,
    /// §9.12. Never zero.
    pub line_clear_delay_ticks: u32,
    /// ARE, §9.12. May be 0, meaning the next piece enters on the same tick.
    pub entry_delay_ticks: u32,
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self::from_settings(&GameplaySettings::default(), &TimingSettings::default())
    }
}

impl RulesConfig {
    /// Resolve the rules class: clamp to the §6.3 ranges, then convert every
    /// duration to ticks by the two rules of §6.6.
    ///
    /// Clamping happens in milliseconds, before the conversion, so the range in
    /// the spec is the range the player reads in their own file. Reporting what
    /// was clamped is the loader's job (§6.2).
    pub fn from_settings(gameplay: &GameplaySettings, timing: &TimingSettings) -> Self {
        Self {
            preview_count: clamped(gameplay.preview_count, &range::PREVIEW_COUNT),
            ghost_piece: gameplay.ghost_piece,
            hold_enabled: gameplay.hold_enabled,
            lock_down: gameplay.lock_down,
            start_level: clamped(gameplay.start_level, &range::START_LEVEL),
            lines_per_level: clamped(gameplay.lines_per_level, &range::LINES_PER_LEVEL),
            allow_180_rotation: gameplay.allow_180_rotation,
            lock_delay_ticks: ms_to_ticks_at_least_one(clamped(
                timing.lock_delay_ms,
                &range::LOCK_DELAY_MS,
            )),
            das_ticks: ms_to_ticks_at_least_one(clamped(timing.das_ms, &range::DAS_MS)),
            arr_ticks: ms_to_ticks(clamped(timing.arr_ms, &range::ARR_MS)),
            soft_drop_factor: clamped(timing.soft_drop_factor, &range::SOFT_DROP_FACTOR),
            line_clear_delay_ticks: ms_to_ticks_at_least_one(clamped(
                timing.line_clear_delay_ms,
                &range::LINE_CLEAR_DELAY_MS,
            )),
            entry_delay_ticks: ms_to_ticks(clamped(timing.entry_delay_ms, &range::ENTRY_DELAY_MS)),
        }
    }
}

/// Settings that change only **what it looks like** and which key does what
/// (§6.5): the `[display]` and `[keys]` classes. Always owned by the player at
/// the terminal, never by a peer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PresentationConfig {
    pub display: DisplaySettings,
    pub keys: KeyBindings,
}

impl ConfigFile {
    /// Split the file into the two classes of §6.5.
    pub fn resolve(&self) -> (RulesConfig, PresentationConfig) {
        (
            RulesConfig::from_settings(&self.gameplay, &self.timing),
            PresentationConfig {
                display: self.display.clone(),
                keys: self.keys.clone(),
            },
        )
    }
}

// ---------------------------------------------------------------------------
// §6.2: the file on disk
// ---------------------------------------------------------------------------

/// The file name under the platform config directory (§6.2).
pub const FILE_NAME: &str = "config.toml";

/// The display columns one cell glyph must occupy (§12.2).
pub const CELL_COLUMNS: usize = 2;

/// `{config_dir}/termino/config.toml` (§6.2).
///
/// `None` only when the platform admits to no config directory at all, which is
/// a documented degradation rather than a failure: the game runs on defaults
/// and says so in the warnings (§16).
pub fn default_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "termino").map(|dirs| dirs.config_dir().join(FILE_NAME))
}

/// The outcome of a load (§6.2).
///
/// `existed` is what decides whether the commented default file is written on
/// the way out: §6.2 writes it on the first clean exit and never overwrites a
/// file the player already has.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub file: ConfigFile,
    pub path: Option<PathBuf>,
    pub existed: bool,
}

/// Read the config file, degrading to defaults for anything unusable (§6.2).
///
/// Never fails. A file that cannot be read, cannot be parsed, or holds values
/// the schema cannot use adds a line to `warnings` — printed after terminal
/// teardown (§16) — and leaves a playable game.
pub fn load(path: Option<&Path>, warnings: &mut Vec<String>) -> Loaded {
    let Some(path) = path.map(Path::to_path_buf).or_else(default_path) else {
        warnings.push("no config directory on this platform; using defaults".to_string());
        return Loaded {
            file: ConfigFile::default(),
            path: None,
            existed: false,
        };
    };
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            // An absent file is the ordinary first run, not a problem: §6.2
            // answers it by writing the defaults out on the first clean exit.
            if error.kind() != io::ErrorKind::NotFound {
                warnings.push(format!("{}: {error}; using defaults", path.display()));
            }
            return Loaded {
                file: ConfigFile::default(),
                path: Some(path),
                existed: false,
            };
        }
    };
    let mut file = parse(&text, warnings);
    validate(&mut file, warnings);
    Loaded {
        file,
        path: Some(path),
        // The file is there and was read: leave it alone, whatever it held.
        existed: true,
    }
}

/// Write the commented document of §6.2, creating the directory if need be.
pub fn save(path: &Path, file: &ConfigFile) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, document(file))
}

/// Parse the document one value at a time (§6.2, §6.3).
///
/// Deserialising the whole file in one go would be shorter, and would also make
/// a single mistyped value fatal to every setting after it. §6.3 asks for the
/// opposite: a value of the wrong type is rejected *by itself* and the default
/// used, so each key is pulled out and converted on its own.
///
/// A document that is not TOML at all is the one thing that cannot be salvaged
/// key by key, and it is the case §17.2's I3 pins: exactly one warning.
fn parse(text: &str, warnings: &mut Vec<String>) -> ConfigFile {
    let root: toml::Table = match text.parse() {
        Ok(root) => root,
        Err(error) => {
            warnings.push(format!(
                "config file is not valid TOML ({}); using defaults",
                first_line(&error.to_string()),
            ));
            return ConfigFile::default();
        }
    };
    report_unknown(&root, "", &TABLES, warnings);
    let mut file = ConfigFile::default();

    if let Some(table) = section(&root, "gameplay", warnings) {
        report_unknown(table, "gameplay", &GAMEPLAY_KEYS, warnings);
        let it = &mut file.gameplay;
        it.preview_count = field(
            table,
            "gameplay",
            "preview_count",
            it.preview_count,
            warnings,
        );
        it.ghost_piece = field(table, "gameplay", "ghost_piece", it.ghost_piece, warnings);
        it.hold_enabled = field(table, "gameplay", "hold_enabled", it.hold_enabled, warnings);
        it.lock_down = field(table, "gameplay", "lock_down", it.lock_down, warnings);
        it.start_level = field(table, "gameplay", "start_level", it.start_level, warnings);
        it.lines_per_level = field(
            table,
            "gameplay",
            "lines_per_level",
            it.lines_per_level,
            warnings,
        );
        it.allow_180_rotation = field(
            table,
            "gameplay",
            "allow_180_rotation",
            it.allow_180_rotation,
            warnings,
        );
    }
    if let Some(table) = section(&root, "timing", warnings) {
        report_unknown(table, "timing", &TIMING_KEYS, warnings);
        let it = &mut file.timing;
        for (key, value) in [
            ("lock_delay_ms", &mut it.lock_delay_ms),
            ("das_ms", &mut it.das_ms),
            ("arr_ms", &mut it.arr_ms),
            ("soft_drop_factor", &mut it.soft_drop_factor),
            ("line_clear_delay_ms", &mut it.line_clear_delay_ms),
            ("entry_delay_ms", &mut it.entry_delay_ms),
        ] {
            *value = field(table, "timing", key, *value, warnings);
        }
    }
    if let Some(table) = section(&root, "display", warnings) {
        report_unknown(table, "display", &DISPLAY_KEYS, warnings);
        let it = &mut file.display;
        it.color_depth = field(table, "display", "color_depth", it.color_depth, warnings);
        it.show_grid = field(table, "display", "show_grid", it.show_grid, warnings);
        it.show_debug = field(table, "display", "show_debug", it.show_debug, warnings);
        for (key, glyph) in [
            ("cell_filled", &mut it.cell_filled),
            ("cell_empty", &mut it.cell_empty),
            ("cell_ghost", &mut it.cell_ghost),
        ] {
            *glyph = field(table, "display", key, std::mem::take(glyph), warnings);
        }
    }
    if let Some(table) = section(&root, "keys", warnings) {
        report_unknown(table, "keys", &KEY_ACTIONS, warnings);
        for (action, bound) in file.keys.each_mut() {
            *bound = field(table, "keys", action, std::mem::take(bound), warnings);
        }
    }
    file
}

/// The §6.3 tables, in the order they are written.
const TABLES: [&str; 4] = ["gameplay", "timing", "display", "keys"];
const GAMEPLAY_KEYS: [&str; 7] = [
    "preview_count",
    "ghost_piece",
    "hold_enabled",
    "lock_down",
    "start_level",
    "lines_per_level",
    "allow_180_rotation",
];
const TIMING_KEYS: [&str; 6] = [
    "lock_delay_ms",
    "das_ms",
    "arr_ms",
    "soft_drop_factor",
    "line_clear_delay_ms",
    "entry_delay_ms",
];
const DISPLAY_KEYS: [&str; 6] = [
    "color_depth",
    "cell_filled",
    "cell_empty",
    "cell_ghost",
    "show_grid",
    "show_debug",
];
/// The `[keys]` actions of §6.3, in the order §10.1 lists them.
const KEY_ACTIONS: [&str; 11] = [
    "move_left",
    "move_right",
    "soft_drop",
    "hard_drop",
    "rotate_cw",
    "rotate_ccw",
    "rotate_180",
    "hold",
    "pause",
    "quit",
    "restart",
];

/// One `[table]` of the document, or `None` if it is absent or is not a table.
fn section<'a>(
    root: &'a toml::Table,
    name: &str,
    warnings: &mut Vec<String>,
) -> Option<&'a toml::Table> {
    match root.get(name)? {
        toml::Value::Table(table) => Some(table),
        other => {
            warnings.push(format!(
                "[{name}] is {}, not a table; using the defaults for it",
                other.type_str(),
            ));
            None
        }
    }
}

/// One value from a table, or `default` with a warning (§6.3).
fn field<T: for<'de> Deserialize<'de>>(
    table: &toml::Table,
    section: &str,
    key: &str,
    default: T,
    warnings: &mut Vec<String>,
) -> T {
    let Some(value) = table.get(key) else {
        return default;
    };
    T::deserialize(value.clone()).unwrap_or_else(|_| {
        warnings.push(format!(
            "{section}.{key} = {value} is not a value this setting can take; using the default",
        ));
        default
    })
}

/// §6.2: unknown keys are ignored for forwards compatibility, but reported.
fn report_unknown(table: &toml::Table, section: &str, known: &[&str], warnings: &mut Vec<String>) {
    let unknown: Vec<&str> = table
        .keys()
        .map(String::as_str)
        .filter(|key| !known.contains(key))
        .collect();
    if unknown.is_empty() {
        return;
    }
    let where_ = if section.is_empty() {
        "the config file".to_string()
    } else {
        format!("[{section}]")
    };
    warnings.push(format!("{where_}: ignoring {}", unknown.join(", ")));
}

/// Report what §6.3 clamps, and restore the two settings it will not do without.
///
/// The clamping itself belongs to [`RulesConfig::from_settings`], which is
/// silent by design — it is also the path a §19 peer's rules take, where there
/// is nobody to warn. This is the loader's half: it says what the file asked
/// for and what it got instead.
fn validate(file: &mut ConfigFile, warnings: &mut Vec<String>) {
    let g = &mut file.gameplay;
    clamp_reported(
        "gameplay.preview_count",
        &mut g.preview_count,
        &range::PREVIEW_COUNT,
        warnings,
    );
    clamp_reported(
        "gameplay.start_level",
        &mut g.start_level,
        &range::START_LEVEL,
        warnings,
    );
    clamp_reported(
        "gameplay.lines_per_level",
        &mut g.lines_per_level,
        &range::LINES_PER_LEVEL,
        warnings,
    );
    let t = &mut file.timing;
    for (name, value, range) in [
        (
            "timing.lock_delay_ms",
            &mut t.lock_delay_ms,
            &range::LOCK_DELAY_MS,
        ),
        ("timing.das_ms", &mut t.das_ms, &range::DAS_MS),
        ("timing.arr_ms", &mut t.arr_ms, &range::ARR_MS),
        (
            "timing.soft_drop_factor",
            &mut t.soft_drop_factor,
            &range::SOFT_DROP_FACTOR,
        ),
        (
            "timing.line_clear_delay_ms",
            &mut t.line_clear_delay_ms,
            &range::LINE_CLEAR_DELAY_MS,
        ),
        (
            "timing.entry_delay_ms",
            &mut t.entry_delay_ms,
            &range::ENTRY_DELAY_MS,
        ),
    ] {
        clamp_reported(name, value, range, warnings);
    }

    // §12.2: all three glyphs are exactly two display columns, or the field
    // stops being a rectangle. The check belongs here rather than to the
    // renderer, which assumes two columns everywhere.
    let defaults = DisplaySettings::default();
    for (name, glyph, default) in [
        (
            "cell_filled",
            &mut file.display.cell_filled,
            defaults.cell_filled,
        ),
        (
            "cell_empty",
            &mut file.display.cell_empty,
            defaults.cell_empty,
        ),
        (
            "cell_ghost",
            &mut file.display.cell_ghost,
            defaults.cell_ghost,
        ),
    ] {
        if display_columns(glyph) != Some(CELL_COLUMNS) {
            warnings.push(format!(
                "display.{name} = {} is not {CELL_COLUMNS} display columns wide; using {}",
                quoted(glyph),
                quoted(&default),
            ));
            *glyph = default;
        }
    }

    // §6.3: between them `pause` and `quit` are the only way out of a game, so
    // an empty list for either is rejected rather than honoured.
    let fallback = KeyBindings::default();
    for (name, bound, default) in [
        ("pause", &mut file.keys.pause, fallback.pause),
        ("quit", &mut file.keys.quit, fallback.quit),
    ] {
        if bound.is_empty() {
            warnings.push(format!(
                "keys.{name} cannot be unbound; restoring the default"
            ));
            *bound = default;
        }
    }
    for (action, names) in file.keys.each() {
        for name in names {
            if !is_key_name(name) {
                warnings.push(format!(
                    "keys.{action}: {} is not a key name (§10.1)",
                    quoted(name),
                ));
            }
        }
    }
}

/// Clamp one value to its §6.3 range, saying so if it moved.
fn clamp_reported<T: Ord + Copy + std::fmt::Display>(
    name: &str,
    value: &mut T,
    range: &RangeInclusive<T>,
    warnings: &mut Vec<String>,
) {
    let wanted = *value;
    *value = clamped(wanted, range);
    if *value != wanted {
        warnings.push(format!(
            "{name} = {wanted} is outside {}..={}; using {}",
            range.start(),
            range.end(),
            *value,
        ));
    }
}

/// How many terminal columns `text` occupies, or `None` if it holds something
/// with no width at all — a control character or a newline.
///
/// This is not a full East Asian Width implementation: §3 pins the dependency
/// table and there is no width crate in it, and §12.2 asks only whether a glyph
/// is exactly two columns. Combining marks and the zero-width formatting
/// characters count nothing, the wide and fullwidth blocks count two, and
/// everything else counts one — which is right for every character anyone would
/// plausibly put in `cell_filled`.
pub fn display_columns(text: &str) -> Option<usize> {
    /// The East Asian Wide and Fullwidth blocks.
    const WIDE: [(u32, u32); 15] = [
        (0x1100, 0x115F),
        (0x2E80, 0x303E),
        (0x3041, 0x33FF),
        (0x3400, 0x4DBF),
        (0x4E00, 0x9FFF),
        (0xA000, 0xA4CF),
        (0xA960, 0xA97F),
        (0xAC00, 0xD7A3),
        (0xF900, 0xFAFF),
        (0xFE10, 0xFE19),
        (0xFE30, 0xFE6F),
        (0xFF00, 0xFF60),
        (0xFFE0, 0xFFE6),
        (0x1F300, 0x1FAFF),
        (0x20000, 0x3FFFD),
    ];
    /// Combining marks and the zero-width formatting characters.
    const ZERO: [(u32, u32); 5] = [
        (0x0300, 0x036F),
        (0x200B, 0x200F),
        (0x20D0, 0x20FF),
        (0xFE00, 0xFE0F),
        (0xFE20, 0xFE2F),
    ];
    let within = |ranges: &[(u32, u32)], point: u32| {
        ranges
            .iter()
            .any(|(low, high)| (*low..=*high).contains(&point))
    };

    let mut columns = 0;
    for c in text.chars() {
        if c.is_control() {
            return None;
        }
        let point = c as u32;
        columns += if within(&ZERO, point) {
            0
        } else if within(&WIDE, point) {
            2
        } else {
            1
        };
    }
    Some(columns)
}

/// A string as TOML would write it, for a warning that quotes what it read.
fn quoted(text: &str) -> String {
    toml::Value::from(text).to_string()
}

/// The first line of a message, for a warning that has to fit on one (§6.2).
fn first_line(message: &str) -> &str {
    message.lines().next().unwrap_or(message).trim_end()
}

/// The fully-commented document of §6.3, carrying `file`'s values (§6.2).
///
/// Written out by hand rather than serialised, because §6.2 asks for a
/// *commented* file: the comments are what make the settings discoverable
/// without the specification, and `toml::to_string` cannot produce them. The
/// values are formatted through `toml::Value`, so quoting and escaping are
/// TOML's own.
pub fn document(file: &ConfigFile) -> String {
    let g = &file.gameplay;
    let t = &file.timing;
    let d = &file.display;
    let keys: String = file
        .keys
        .each()
        .iter()
        .map(|(action, names)| format!("{action:<13} = {}\n", list(names)))
        .collect();
    format!(
        "\
# Termino configuration (TERMINO.md §6.3). Every setting here has a default;
# deleting a line, a table or the whole file simply restores it.

[gameplay]
# Number of upcoming pieces shown in the preview window.
# Range: {preview_range}. The upper bound is the height of the next box.
preview_count      = {preview}
# Show the translucent ghost piece at the landing position.
ghost_piece        = {ghost}
# Make the hold mechanic available. When false the hold key is inert, the hold
# box is not drawn, and the binding is hidden from the controls panels.
hold_enabled       = {hold}
# Lock-down rule: \"extended\" | \"infinite\" | \"classic\"
lock_down          = {lock_down}
# Starting level. Range: {level_range}, the levels the speed curve is defined for.
start_level        = {level}
# Lines required to advance one level. Range: {lines_range}. A large value is a
# legitimate way to hold the speed constant; 0 is not, since the level
# threshold is a multiple of it.
lines_per_level    = {lines}
# Make 180-degree rotation available. When false the 180 key is inert and the
# binding is hidden from the controls panels.
allow_180_rotation = {rot180}

[timing]
# All values in milliseconds, converted once to whole ticks at load. Every one
# of them is a duration, so a value that rounds to 0 ticks is raised to 1
# except where 0 is meaningful.
lock_delay_ms       = {lock_delay}   # Range: {lock_delay_range}. For a delay that never
                            #         expires use lock_down = \"infinite\".
das_ms              = {das}   # Range: {das_range}.
arr_ms              = {arr}   # Range: {arr_range}. 0 means \"every tick\".
soft_drop_factor    = {soft_drop}   # soft drop is this many times normal gravity.
                            # Range: {soft_drop_range}. 1 is no speed-up at all; above
                            # about 20 it is a hard drop in all but name.
line_clear_delay_ms = {line_clear}   # Range: {line_clear_range}.
entry_delay_ms      = {entry}   # ARE. Range: {entry_range}. 0 means the next piece
                            #         enters on the same tick.

[display]
# \"auto\" | \"truecolor\" | \"256\" | \"16\" | \"mono\"
color_depth   = {depth}
# Characters used to paint one occupied cell. Each must be exactly {CELL_COLUMNS} display
# columns wide; one that is not is rejected with a warning and the default used.
cell_filled   = {filled}
cell_empty    = {empty}
cell_ghost    = {ghost_cell}
# Draw a faint dotted grid in the empty playfield.
show_grid     = {grid}
# Show frame rate, tick rate and internal timers.
show_debug    = {debug}

[keys]
# Each action maps to a list of key names; any listed key triggers it. Names are
# Left, Right, Up, Down, Space, Enter, Tab, Esc, Backspace, F1-F12, and single
# characters, case-sensitive. An empty list leaves that action unbound, which is
# a supported way to disable it -- except for pause and quit, which between them
# are the only way out of a game and are restored if unbound.
{keys}",
        preview_range = range_text(&range::PREVIEW_COUNT),
        preview = g.preview_count,
        ghost = g.ghost_piece,
        hold = g.hold_enabled,
        lock_down = value(&g.lock_down),
        level_range = range_text(&range::START_LEVEL),
        level = g.start_level,
        lines_range = range_text(&range::LINES_PER_LEVEL),
        lines = g.lines_per_level,
        rot180 = g.allow_180_rotation,
        lock_delay = pad(t.lock_delay_ms),
        lock_delay_range = range_text(&range::LOCK_DELAY_MS),
        das = pad(t.das_ms),
        das_range = range_text(&range::DAS_MS),
        arr = pad(t.arr_ms),
        arr_range = range_text(&range::ARR_MS),
        soft_drop = pad(t.soft_drop_factor),
        soft_drop_range = range_text(&range::SOFT_DROP_FACTOR),
        line_clear = pad(t.line_clear_delay_ms),
        line_clear_range = range_text(&range::LINE_CLEAR_DELAY_MS),
        entry = pad(t.entry_delay_ms),
        entry_range = range_text(&range::ENTRY_DELAY_MS),
        depth = value(&d.color_depth),
        filled = quoted(&d.cell_filled),
        empty = quoted(&d.cell_empty),
        ghost_cell = quoted(&d.cell_ghost),
        grid = d.show_grid,
        debug = d.show_debug,
    )
}

/// A setting as TOML would write it.
fn value<T: Serialize>(setting: &T) -> String {
    toml::Value::try_from(setting).map_or_else(|_| "?".to_string(), |v| v.to_string())
}

/// A list of key names as a TOML array.
fn list(names: &[String]) -> String {
    let items: Vec<toml::Value> = names
        .iter()
        .map(|n| toml::Value::from(n.as_str()))
        .collect();
    toml::Value::Array(items).to_string()
}

/// A `[timing]` value, padded so the trailing comments stay in a column.
fn pad(ms: u32) -> String {
    format!("{ms:<3}")
}

/// An inclusive range as §6.3 writes it.
fn range_text<T: std::fmt::Display>(range: &RangeInclusive<T>) -> String {
    format!("{}..={}", range.start(), range.end())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The conversion table of §6.6, transcribed literally.
    const MS_TO_TICKS_TABLE: [(&str, u32, u32); 5] = [
        ("lock_delay_ms", 500, 30),
        ("das_ms", 170, 10),
        ("arr_ms", 30, 2),
        ("line_clear_delay_ms", 250, 15),
        ("entry_delay_ms", 0, 0),
    ];

    #[test]
    fn the_spec_conversion_table_holds() {
        // T17.
        for (name, ms, ticks) in MS_TO_TICKS_TABLE {
            assert_eq!(ms_to_ticks(ms), ticks, "{name} = {ms} ms");
        }
        let rules = RulesConfig::default();
        assert_eq!(rules.lock_delay_ticks, 30);
        assert_eq!(rules.das_ticks, 10);
        assert_eq!(rules.arr_ticks, 2);
        assert_eq!(rules.line_clear_delay_ticks, 15);
        assert_eq!(rules.entry_delay_ticks, 0);
    }

    #[test]
    fn a_delay_that_was_asked_for_never_rounds_away() {
        // T17's edge case: 1 ms is a sixteenth of a tick, but a lock delay the
        // player configured must not vanish.
        assert_eq!(ms_to_ticks(1), 0);
        assert_eq!(ms_to_ticks_at_least_one(1), 1);
        assert_eq!(ms_to_ticks_at_least_one(0), 1);
        let rules = RulesConfig::from_settings(
            &GameplaySettings::default(),
            &TimingSettings {
                lock_delay_ms: 1,
                line_clear_delay_ms: 1,
                ..TimingSettings::default()
            },
        );
        assert_eq!(rules.lock_delay_ticks, 1);
        assert_eq!(rules.line_clear_delay_ticks, 1);
    }

    #[test]
    fn zero_is_preserved_where_zero_is_meaningful() {
        // §6.6: entry_delay_ms and arr_ms may legitimately be 0, meaning "same
        // tick". They use the other rounding rule for exactly this reason.
        let rules = RulesConfig::from_settings(
            &GameplaySettings::default(),
            &TimingSettings {
                arr_ms: 0,
                entry_delay_ms: 0,
                ..TimingSettings::default()
            },
        );
        assert_eq!(rules.arr_ticks, 0);
        assert_eq!(rules.entry_delay_ticks, 0);
    }

    #[test]
    fn rounding_is_to_nearest_and_integral() {
        // 8.333 ms is half a tick; below it rounds down, above it rounds up.
        assert_eq!(ms_to_ticks(8), 0);
        assert_eq!(ms_to_ticks(9), 1);
        assert_eq!(ms_to_ticks(16), 1);
        assert_eq!(ms_to_ticks(17), 1);
        assert_eq!(ms_to_ticks(25), 2);
        assert_eq!(ms_to_ticks(1000), 60);
        // Monotonic, and never a surprise near the top of the range.
        let mut previous = 0;
        for ms in 0..10_000 {
            let ticks = ms_to_ticks(ms);
            assert!(ticks >= previous, "{ms} ms went backwards");
            previous = ticks;
        }
        assert_eq!(ms_to_ticks(u32::MAX), 257_698_038);
    }

    #[test]
    fn tick_constants_match_the_spec() {
        assert_eq!(TICK_HZ, 60);
        assert_eq!(TICK, Duration::from_nanos(16_666_666));
        assert_eq!(MAX_CATCH_UP_TICKS, 6);
        // §15.1 calls the catch-up cap "100 ms of arrears".
        assert!(TICK * MAX_CATCH_UP_TICKS <= Duration::from_millis(100));
    }

    #[test]
    fn defaults_match_the_spec_schema() {
        let file = ConfigFile::default();
        assert_eq!(file.gameplay.preview_count, 5);
        assert!(file.gameplay.ghost_piece);
        assert!(file.gameplay.hold_enabled);
        assert_eq!(file.gameplay.lock_down, LockDownRule::Extended);
        assert_eq!(file.gameplay.start_level, 1);
        assert_eq!(file.gameplay.lines_per_level, 10);
        assert!(file.gameplay.allow_180_rotation);
        assert_eq!(file.timing.soft_drop_factor, 20);
        assert_eq!(file.display.color_depth, ColorDepth::Auto);
        assert_eq!(file.display.cell_filled, "██");
        assert_eq!(file.display.cell_ghost, "▒▒");
        assert!(!file.display.show_grid);
        assert!(!file.display.show_debug);
        assert_eq!(file.keys.rotate_ccw, ["z", "Z"]);
        assert_eq!(file.keys.pause, ["Esc", "F1"]);
    }

    #[test]
    fn defaults_round_trip_through_toml() {
        // Part of I2. The file layer is what round-trips: it is the one written
        // in the units the player typed.
        let file = ConfigFile::default();
        let text = toml::to_string(&file).expect("serialises");
        let parsed: ConfigFile = toml::from_str(&text).expect("parses");
        assert_eq!(parsed, file);
        // The §6.3 table names must survive, or the file the player edits is
        // not the file the program reads.
        for table in ["[gameplay]", "[timing]", "[display]", "[keys]"] {
            assert!(text.contains(table), "{table} missing from\n{text}");
        }
        assert!(text.contains("lock_down = \"extended\""), "{text}");
        assert!(text.contains("color_depth = \"auto\""), "{text}");
    }

    #[test]
    fn the_rules_class_round_trips_on_its_own() {
        // §6.5/§19: RulesConfig travels to a peer by itself, in ticks.
        let rules = RulesConfig::default();
        let text = toml::to_string(&rules).expect("serialises");
        let parsed: RulesConfig = toml::from_str(&text).expect("parses");
        assert_eq!(parsed, rules);
        assert!(
            !text.contains("_ms"),
            "the core must never see milliseconds"
        );
    }

    #[test]
    fn colour_depth_names_match_the_spec() {
        for (name, depth) in [
            ("auto", ColorDepth::Auto),
            ("truecolor", ColorDepth::Truecolor),
            ("256", ColorDepth::Ansi256),
            ("16", ColorDepth::Ansi16),
            ("mono", ColorDepth::Mono),
        ] {
            let toml_text = format!("color_depth = {name:?}\n");
            let parsed: DisplaySettings = toml::from_str(&toml_text).expect(name);
            assert_eq!(parsed.color_depth, depth);
        }
        for (name, rule) in [
            ("extended", LockDownRule::Extended),
            ("infinite", LockDownRule::Infinite),
            ("classic", LockDownRule::Classic),
        ] {
            let toml_text = format!("lock_down = {name:?}\n");
            let parsed: GameplaySettings = toml::from_str(&toml_text).expect(name);
            assert_eq!(parsed.lock_down, rule);
        }
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        // §6.3: "Values outside the range are clamped."
        for (given, expected) in [(0, 1), (1, 1), (6, 6), (7, 6), (255, 6)] {
            let rules = RulesConfig::from_settings(
                &GameplaySettings {
                    preview_count: given,
                    ..GameplaySettings::default()
                },
                &TimingSettings::default(),
            );
            assert_eq!(rules.preview_count, expected, "preview_count = {given}");
        }
        for (given, expected) in [(0, 1), (1, 1), (15, 15), (99, 15)] {
            let rules = RulesConfig::from_settings(
                &GameplaySettings {
                    start_level: given,
                    ..GameplaySettings::default()
                },
                &TimingSettings::default(),
            );
            assert_eq!(rules.start_level, expected, "start_level = {given}");
        }
        for (given, expected) in [(0, 1), (1, 1), (10, 10), (1000, 1000), (99_999, 1000)] {
            let rules = RulesConfig::from_settings(
                &GameplaySettings {
                    lines_per_level: given,
                    ..GameplaySettings::default()
                },
                &TimingSettings::default(),
            );
            assert_eq!(rules.lines_per_level, expected, "lines_per_level = {given}");
        }
    }

    #[test]
    fn the_load_bearing_lower_bounds_hold() {
        // §6.3: lines_per_level and soft_drop_factor are divisors, and a zero
        // reaching the core is a crash, not a slow game.
        let rules = RulesConfig::from_settings(
            &GameplaySettings {
                lines_per_level: 0,
                ..GameplaySettings::default()
            },
            &TimingSettings {
                soft_drop_factor: 0,
                ..TimingSettings::default()
            },
        );
        assert_eq!(rules.lines_per_level, 1);
        assert_eq!(rules.soft_drop_factor, 1);
        assert_eq!(*range::LINES_PER_LEVEL.start(), 1);
        assert_eq!(*range::SOFT_DROP_FACTOR.start(), 1);
    }

    #[test]
    fn timing_is_clamped_in_milliseconds_before_conversion() {
        // §6.3's ranges are the ones the player reads in their own file, so
        // they are applied to the milliseconds, not to the derived ticks.
        let rules = RulesConfig::from_settings(
            &GameplaySettings::default(),
            &TimingSettings {
                lock_delay_ms: 60_000,
                das_ms: 60_000,
                arr_ms: 60_000,
                soft_drop_factor: 10_000,
                line_clear_delay_ms: 60_000,
                entry_delay_ms: 60_000,
            },
        );
        assert_eq!(rules.lock_delay_ticks, ms_to_ticks(5000));
        assert_eq!(rules.das_ticks, ms_to_ticks(1000));
        assert_eq!(rules.arr_ticks, ms_to_ticks(1000));
        assert_eq!(rules.soft_drop_factor, 100);
        assert_eq!(rules.line_clear_delay_ticks, ms_to_ticks(2000));
        assert_eq!(rules.entry_delay_ticks, ms_to_ticks(2000));
        // Every default sits inside its own range, or the spec contradicts
        // itself.
        let defaults = TimingSettings::default();
        assert!(range::LOCK_DELAY_MS.contains(&defaults.lock_delay_ms));
        assert!(range::DAS_MS.contains(&defaults.das_ms));
        assert!(range::ARR_MS.contains(&defaults.arr_ms));
        assert!(range::SOFT_DROP_FACTOR.contains(&defaults.soft_drop_factor));
        assert!(range::LINE_CLEAR_DELAY_MS.contains(&defaults.line_clear_delay_ms));
        assert!(range::ENTRY_DELAY_MS.contains(&defaults.entry_delay_ms));
        let gameplay = GameplaySettings::default();
        assert!(range::PREVIEW_COUNT.contains(&gameplay.preview_count));
        assert!(range::START_LEVEL.contains(&gameplay.start_level));
        assert!(range::LINES_PER_LEVEL.contains(&gameplay.lines_per_level));
    }

    #[test]
    fn a_partial_or_unfamiliar_file_still_loads() {
        // §6.2: a missing key takes its default, and an unknown key is ignored
        // rather than fatal. Reporting it in the warning is Stage 10's job; not
        // crashing is this layer's.
        let text = "\
[gameplay]
preview_count = 3
future_option = \"whatever\"

[timing]
lock_delay_ms = 1000
";
        let file: ConfigFile = toml::from_str(text).expect("tolerates the unknown key");
        assert_eq!(file.gameplay.preview_count, 3);
        assert!(
            file.gameplay.ghost_piece,
            "unmentioned keys keep their default"
        );
        assert_eq!(file.timing.lock_delay_ms, 1000);
        assert_eq!(file.timing.das_ms, TimingSettings::default().das_ms);
        assert_eq!(file.display, DisplaySettings::default());
        assert_eq!(file.resolve().0.lock_delay_ticks, 60);
    }

    #[test]
    fn the_two_classes_are_split_by_setting_not_by_table() {
        // §6.5. [timing] is a rules setting even though the shell is what reads
        // das/arr, and [keys] is presentation even though it decides what the
        // player can do.
        let (rules, presentation) = ConfigFile::default().resolve();
        assert_eq!(rules, RulesConfig::default());
        assert_eq!(presentation, PresentationConfig::default());
        assert_eq!(rules.das_ticks, 10);
        assert_eq!(presentation.keys.hold, ["c", "C"]);
    }
    // -- §6.2: the file on disk ---------------------------------------------

    #[test]
    fn the_written_document_reads_back_as_what_was_written() {
        // I2, the whole way round: defaults -> commented TOML -> parse ->
        // identical struct. `document` is written by hand rather than
        // serialised, so nothing but this test says the two agree.
        let file = ConfigFile::default();
        let text = document(&file);
        let mut warnings = Vec::new();
        let parsed = parse(&text, &mut warnings);
        assert_eq!(parsed, file);
        assert!(warnings.is_empty(), "{warnings:?}");
        // And it round-trips for a file that is not the defaults, which is what
        // the Options screen writes back (§6.1).
        let changed = ConfigFile {
            gameplay: GameplaySettings {
                preview_count: 2,
                hold_enabled: false,
                lock_down: LockDownRule::Classic,
                start_level: 9,
                ..GameplaySettings::default()
            },
            display: DisplaySettings {
                color_depth: ColorDepth::Ansi256,
                cell_filled: "[]".to_string(),
                show_debug: true,
                ..DisplaySettings::default()
            },
            timing: TimingSettings {
                arr_ms: 0,
                lock_delay_ms: 1000,
                ..TimingSettings::default()
            },
            keys: KeyBindings {
                hold: vec!["Tab".to_string()],
                rotate_180: Vec::new(),
                ..KeyBindings::default()
            },
        };
        let mut warnings = Vec::new();
        assert_eq!(parse(&document(&changed), &mut warnings), changed);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn the_document_is_commented_and_names_every_setting() {
        // §6.2 asks for a *fully-commented* file: the comments are what make
        // the settings discoverable without the specification.
        let text = document(&ConfigFile::default());
        for table in TABLES {
            assert!(text.contains(&format!("[{table}]")), "[{table}] missing");
        }
        for key in GAMEPLAY_KEYS
            .iter()
            .chain(&TIMING_KEYS)
            .chain(&DISPLAY_KEYS)
            .chain(&KEY_ACTIONS)
        {
            assert!(text.contains(key), "{key} missing from the document");
        }
        let comments = text
            .lines()
            .filter(|l| l.trim_start().starts_with('#'))
            .count();
        assert!(comments > 20, "only {comments} comment lines");
    }

    #[test]
    fn a_file_that_is_not_toml_produces_exactly_one_warning() {
        // I3. One line, defaults throughout, and a playable game.
        let mut warnings = Vec::new();
        let file = parse("this is not [ toml at all", &mut warnings);
        assert_eq!(file, ConfigFile::default());
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("not valid TOML"), "{warnings:?}");
    }

    #[test]
    fn one_unusable_value_does_not_take_the_rest_of_the_file_with_it() {
        // §6.3: a value of the wrong type is rejected *by itself* and the
        // default used. Deserialising the document in one go would make the
        // first mistake fatal to every key after it.
        let text = "\
[gameplay]
preview_count = \"lots\"
start_level = 7
lock_down = \"eventually\"

[display]
show_grid = true
";
        let mut warnings = Vec::new();
        let file = parse(text, &mut warnings);
        assert_eq!(file.gameplay.preview_count, 5, "the default");
        assert_eq!(file.gameplay.start_level, 7, "and the good key still lands");
        assert_eq!(file.gameplay.lock_down, LockDownRule::Extended);
        assert!(file.display.show_grid, "as does the table after it");
        assert_eq!(warnings.len(), 2, "{warnings:?}");
    }

    #[test]
    fn unknown_keys_are_ignored_but_reported() {
        // §6.2: forwards compatibility, out loud.
        let text = "\
[gameplay]
preview_count = 3
future_option = \"whatever\"

[nonsense]
x = 1
";
        let mut warnings = Vec::new();
        let file = parse(text, &mut warnings);
        assert_eq!(file.gameplay.preview_count, 3);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("future_option")));
        assert!(warnings.iter().any(|w| w.contains("nonsense")));
    }

    #[test]
    fn a_clamped_value_says_what_it_was_and_what_it_became() {
        // §6.3: the clamping is silent in `from_settings`, because that path is
        // also a §19 peer's; the loader is what tells the player.
        let mut file = ConfigFile {
            gameplay: GameplaySettings {
                preview_count: 9,
                start_level: 99,
                ..GameplaySettings::default()
            },
            ..ConfigFile::default()
        };
        let mut warnings = Vec::new();
        validate(&mut file, &mut warnings);
        assert_eq!(file.gameplay.preview_count, 6);
        assert_eq!(file.gameplay.start_level, 15);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].contains("preview_count = 9"), "{warnings:?}");
        assert!(warnings[0].contains("using 6"), "{warnings:?}");
    }

    #[test]
    fn a_glyph_that_is_not_two_columns_is_rejected() {
        // §12.2, and the exit criterion that the field stays a rectangle. The
        // check is the loader's; `ui::cells` assumes two columns everywhere.
        assert_eq!(display_columns("██"), Some(2));
        assert_eq!(display_columns("[]"), Some(2));
        assert_eq!(display_columns("  "), Some(2));
        assert_eq!(display_columns("*"), Some(1));
        assert_eq!(display_columns("███"), Some(3));
        assert_eq!(display_columns(""), Some(0));
        assert_eq!(display_columns("\u{ff21}"), Some(2), "a fullwidth A");
        assert_eq!(display_columns("e\u{301}"), Some(1), "a combining acute");
        assert_eq!(
            display_columns("\t"),
            None,
            "a control character has no width"
        );

        let mut file = ConfigFile {
            display: DisplaySettings {
                cell_filled: "***".to_string(),
                cell_ghost: "-".to_string(),
                ..DisplaySettings::default()
            },
            ..ConfigFile::default()
        };
        let mut warnings = Vec::new();
        validate(&mut file, &mut warnings);
        let defaults = DisplaySettings::default();
        assert_eq!(file.display.cell_filled, defaults.cell_filled);
        assert_eq!(file.display.cell_ghost, defaults.cell_ghost);
        assert_eq!(file.display.cell_empty, defaults.cell_empty, "left alone");
        assert_eq!(warnings.len(), 2, "{warnings:?}");
    }

    #[test]
    fn the_two_ways_out_of_a_game_cannot_be_unbound() {
        // §6.3: an empty list is a supported way to disable an action, except
        // for `pause` and `quit`, which between them are the only way out.
        let mut file = ConfigFile {
            keys: KeyBindings {
                pause: Vec::new(),
                quit: Vec::new(),
                hold: Vec::new(),
                rotate_ccw: vec!["Wiggle".to_string()],
                ..KeyBindings::default()
            },
            ..ConfigFile::default()
        };
        let mut warnings = Vec::new();
        validate(&mut file, &mut warnings);
        let defaults = KeyBindings::default();
        assert_eq!(file.keys.pause, defaults.pause);
        assert_eq!(file.keys.quit, defaults.quit);
        assert!(file.keys.hold.is_empty(), "hold may be unbound");
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert!(
            warnings.iter().any(|w| w.contains("Wiggle")),
            "{warnings:?}"
        );
    }

    #[test]
    fn an_absent_file_is_not_a_problem_and_is_not_a_warning() {
        // §6.2: the ordinary first run. The defaults are used and the file is
        // written on the first clean exit, which `existed` is what decides.
        let path = std::env::temp_dir().join("termino-absent-config-test.toml");
        let _ = fs::remove_file(&path);
        let mut warnings = Vec::new();
        let loaded = load(Some(&path), &mut warnings);
        assert_eq!(loaded.file, ConfigFile::default());
        assert!(!loaded.existed);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_saved_file_loads_back_unchanged() {
        let path = std::env::temp_dir().join("termino-roundtrip-config-test/config.toml");
        let _ = fs::remove_dir_all(path.parent().expect("a parent"));
        let file = ConfigFile {
            gameplay: GameplaySettings {
                preview_count: 1,
                ..GameplaySettings::default()
            },
            ..ConfigFile::default()
        };
        save(&path, &file).expect("writes, creating the directory");
        let mut warnings = Vec::new();
        let loaded = load(Some(&path), &mut warnings);
        assert_eq!(loaded.file, file);
        assert!(loaded.existed);
        assert!(warnings.is_empty(), "{warnings:?}");
        let _ = fs::remove_dir_all(path.parent().expect("a parent"));
    }
}
