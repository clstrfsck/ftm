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

use std::time::Duration;

use serde::{Deserialize, Serialize};

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
fn clamped<T: Ord + Copy>(value: T, range: &std::ops::RangeInclusive<T>) -> T {
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

// TODO(stage 10): load/save at the §6.2 path, the §6.4 CLI merge, and the
// warning vector (§6.2, §16). The clamping in `RulesConfig::from_settings` is
// silent; the loader is what reports it, along with unknown keys, the §12.2
// cell-glyph width rule, and the §6.3 rule that an empty `pause` or `quit`
// binding is rejected and the default restored.

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
    /// §10.3. May be 0, meaning "every tick".
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
}
