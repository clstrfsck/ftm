# Falling Tetromino Manager — Software Specification

**Version:** 1.0
**Date:** 2026-09-05
**Target language:** Rust (edition 2024, MSRV 1.85)
**Name:** Falling Tetromino Manager (a terminal tetromino game; binary name `ftm`)

---

## Contents

| § | Section | § | Section |
|---|---|---|---|
| 1 | [Purpose and scope](#1-purpose-and-scope) | 11 | [Game mode](#11-game-mode) |
| 2 | [Terminology](#2-terminology) | 12 | [Rendering](#12-rendering) |
| 3 | [Technology and dependencies](#3-technology-and-dependencies) | 13 | [Attract screen](#13-attract-screen) |
| 4 | [Project layout](#4-project-layout) | 14 | [High scores](#14-high-scores) |
| 5 | [Coordinate conventions](#5-coordinate-conventions) | 15 | [Game loop and timing](#15-game-loop-and-timing) |
| 6 | [Configuration](#6-configuration) | 16 | [Error handling](#16-error-handling) |
| 7 | [Application states](#7-application-states) | 17 | [Testing and acceptance](#17-testing-and-acceptance) |
| 8 | [Terminal handling](#8-terminal-handling) | 18 | [Deferred and open items](#18-deferred-and-open-items) |
| 9 | [Game rules](#9-game-rules) | 19 | [Network readiness](#19-network-readiness) |
| 10 | [Controls](#10-controls) | | |

The parts most likely to be read while coding are §5 (coordinates — read it
before §9.5), §9 (all the rules and tables), §12 (layout, and the `GameView` /
`GameEvent` contracts) and §15 (the fixed-timestep loop). §19 is not
implementation work; it is the list of things v1.0 must not do, so that a
client/server split stays possible later.

---

## 1. Purpose and scope

This document specifies a complete, playable, single-player falling-block puzzle
game that runs in a text terminal. It is intended to be sufficient, on its own, to
implement the game without reference to any other source: all rotation tables,
scoring tables, timing constants and screen layouts are reproduced here in full.

External references are given for provenance only, not as required reading:

- Tetris Guideline summary: <https://tetris.wiki/Tetris_Guideline>
- Super Rotation System: <https://tetris.wiki/Super_Rotation_System>

Where this specification and those pages disagree, **this specification wins**.

### 1.1 Goals

- Faithful implementation of standard guideline gameplay: Super Rotation System
  (SRS) with wall kicks, 7-bag randomiser, hold piece, ghost piece, next-piece
  preview, extended-placement lock down, guideline scoring including T-spins,
  back-to-back bonuses, combos and perfect clears.
- Two top-level screens: an **attract screen** shown when no game is in progress,
  and the **playfield** for an active game.
- A configurable next-piece preview count.
- Smooth, flicker-free rendering in any ANSI-capable terminal of at least
  60 columns × 24 rows.
- Deterministic, testable core: the game rules must be exercisable without a
  terminal attached.

### 1.2 Non-goals (for version 1.0)

- Multiplayer, garbage lines, network play. A future client/server split is
  anticipated, and §19 states the constraints v1.0 must respect so that it stays
  possible; nothing in §19 is itself implemented in v1.0.
- Sound.
- Game modes other than Marathon (Sprint / Ultra / Zen are noted as future work
  in §18).
- Mouse input.

### 1.3 Naming and trademark

The game is called **Falling Tetromino Manager**, abbreviated **FTM**, which is
also the binary's name. The full name is what the attract screen spells out
under the wordmark (§13.2); `FTM` is what everything shorter uses — the binary,
the config and data directories, the crate.

"Tetris" and the official Tetris logo are trademarks of Tetris Holding, LLC. The
game must not be named "Tetris", and **the official logo must not be used,
reproduced, or approximated.** The attract screen uses an original block-letter
wordmark (§13.2). No official branding, colours-as-logo, or marketing artwork is
to be reproduced.

A four-line clear is therefore called a **QUAD** (§2, §9.14, §12.4, §13.3),
not a "Tetris". The nominative use — naming the clear the way "T-spin" names a
manoeuvre — would have been defensible, but it is the only thing the word was
needed for, and giving it up costs one syllable and keeps the trademark off the
screen altogether. `QUAD` is the preferred term throughout: in this document, in
the interface, and in anything written about the game.

The word "Tetris" survives in this document only where it refers to the
trademark itself (this section, §13.2) or to the Tetris Guideline and the
tetris.wiki references the rules are drawn from (§1, §3).

Falling Tetromino Manager is a personal, non-commercial implementation of the
well-known public rules.

---

## 2. Terminology

| Term | Meaning |
|---|---|
| **Mino** | A single filled cell of a piece. |
| **Tetromino / piece** | One of the seven shapes `I O T S Z J L`, each four minos. |
| **Matrix** | The 10 × 40 grid of cells that pieces fall into (§9.1). |
| **Playfield** | The visible bottom 20 rows of the matrix. |
| **Buffer zone** | The hidden top 20 rows of the matrix. |
| **Locked** | A piece that has become part of the matrix and no longer moves. |
| **Orientation** | One of four rotation states: `North`, `East`, `South`, `West` (also written `0`, `R`, `2`, `L`). |
| **Kick** | A translation applied after a rotation to make it fit (§9.5). |
| **Gravity** | Automatic downward movement over time. |
| **G** | One cell of gravity per tick (1 G = 1 cell/tick = 60 cells/s). |
| **Lock delay** | The grace period between landing and locking. |
| **DAS** | Delayed Auto Shift: the pause before a held movement key repeats. |
| **ARR** | Auto Repeat Rate: the interval between repeats once DAS has elapsed. |
| **ARE** | Entry delay: the pause between one piece locking and the next spawning. |
| **Quad** | A four-line clear (§9.14). Elsewhere called a "Tetris"; FTM does not use that name (§1.3). |
| **B2B** | Back-to-back: consecutive "difficult" line clears (§9.15). |

---

## 3. Technology and dependencies

The implementation is a single Rust binary crate, **edition 2024**. Edition 2024
is stable from Rust 1.85, which is therefore the MSRV; the toolchain is pinned no
further than that.

| Crate | Version | Purpose |
|---|---|---|
| `ratatui` | 0.29 | Widget layout and double-buffered terminal drawing. |
| `crossterm` | 0.28 | Terminal backend: raw mode, alternate screen, key events, keyboard-enhancement flags. |
| `rand` | 0.8 | `SmallRng` for the bag randomiser; seedable for tests. |
| `serde` + `serde_derive` | 1 | Config and high-score (de)serialisation. |
| `toml` | 0.8 | Config file format. |
| `serde_json` | 1 | High-score file format. |
| `clap` | 4 (derive) | Command-line argument parsing. |
| `directories` | 5 | Platform config/data directories. |
| `anyhow` | 1 | Error propagation in `main` and I/O paths. |
| `thiserror` | 1 | Typed errors in the config loader. |
| `chrono` | 0.4 | Date stamps on high-score entries. |

No other runtime dependencies. `unsafe` is forbidden
(`#![forbid(unsafe_code)]` at crate root).

### 3.1 Layering rule

The crate is split into a **core** (pure rules, no I/O) and a **shell** (terminal,
config, persistence). Three properties of the core are hard requirements, not
preferences: the acceptance tests in §17 depend on them, and so does the
client/server split described in §19.

1. **No I/O and no clock.** The core must compile and be fully unit-testable with
   no terminal present. It never reads the clock; time enters only as an explicit
   argument.
2. **Advanced in fixed ticks.** The core advances one **tick** at a time
   (`Game::tick`, §15.1), never by a variable `Duration`. A tick is 1/60 s.
3. **Deterministic.** Given the same starting seed and the same sequence of
   per-tick inputs, the core must produce byte-identical state. No `HashMap`
   iteration order, no floating-point accumulation that depends on frame pacing,
   no ambient randomness.

The shell owns everything else: the clock, terminal I/O, key decoding, DAS/ARR
timing, animation timing, config files and persistence. The core exposes its
state to the shell only through a **view model** (§12.7) and an **event stream**
(§12.8) — the renderer must not read core internals directly.

---

## 4. Project layout

```
ftm/
├── Cargo.toml
├── FTM.md                # this document
├── README.md
├── tests/                    # integration tests (§17.2), driven through lib.rs
└── src/
    ├── lib.rs                # the crate proper; main.rs is a thin wrapper
    ├── main.rs               # entry point, terminal setup/teardown, panic hook
    ├── app.rs                # top-level state machine (§7), event loop (§15)
    ├── config.rs             # Config struct, TOML load/save, CLI merge (§6)
    ├── input.rs              # key decoding, action mapping, DAS/ARR (§10)
    ├── highscore.rs          # high-score table load/save (§14)
    ├── core/
    │   ├── mod.rs            # re-exports; `Game` façade
    │   ├── geometry.rs       # Point, Rotation, direction helpers
    │   ├── piece.rs          # Tetromino, cell patterns, spawn data (§9.2–9.4)
    │   ├── matrix.rs         # Matrix storage, collision, line clearing (§9.1)
    │   ├── srs.rs            # wall-kick tables and rotation resolution (§9.5)
    │   ├── bag.rs            # 7-bag randomiser (§9.6)
    │   ├── gravity.rs        # speed curve, level progression (§9.9)
    │   ├── lockdown.rs       # extended placement lock down (§9.11)
    │   ├── tspin.rs          # T-spin / mini detection (§9.13)
    │   ├── scoring.rs        # score table, B2B, combo, perfect clear (§9.14)
    │   ├── view.rs           # GameView: the serialisable render model (§12.7)
    │   ├── events.rs         # GameEvent: what happened this tick (§12.8)
    │   └── game.rs           # Game state, `Game::tick` (§15.1)
    └── ui/
        ├── mod.rs            # screen dispatch, terminal-too-small screen
        ├── theme.rs          # colour palette, colour-depth fallback (§12.3)
        ├── cells.rs          # cell glyph rendering primitives (§12.2)
        ├── playfield.rs      # in-game screen (§12.4)
        ├── attract.rs        # attract screen (§13)
        └── overlays.rs       # pause, game-over, name-entry overlays
```

The crate is a **library plus a thin binary**. `main.rs` holds only the entry
point; everything else lives behind `lib.rs`. This is what lets the integration
tests of §17.2 — the scripted game and the batch-invariance canary of §19.4 —
drive the core from `tests/`, which a binary-only crate cannot do.

---

## 5. Coordinate conventions

These conventions are used **everywhere in this document and in the code**, and
differ from the conventions used by most published SRS documentation. Read this
section before §9.5.

- A matrix cell is addressed as `(col, row)`.
- `col` increases to the **right**, from `0` (leftmost) to `9` (rightmost).
- `row` increases **downward**, from `0` (top of the buffer zone) to `39`
  (bottom of the playfield).
- The visible playfield is rows `20..=39`. Row `20` is the topmost visible row.
- "Above" means a numerically smaller `row`; "below" means larger.
- A translation is written `(dx, dy)` where positive `dy` moves the piece
  **down**.

> **Note on published kick tables.** The canonical SRS tables published online use
> a y-up convention (positive `y` moves the piece up). The tables in §9.5 of this
> document have already been converted to the y-down convention above. Do not
> negate them again.

---

## 6. Configuration

### 6.1 Sources and precedence

Settings are resolved in this order, later sources overriding earlier ones:

1. Built-in defaults (§6.3).
2. The config file (§6.2).
3. Command-line arguments (§6.4).

Settings changed in the in-game Options screen are written back to the config
file immediately on leaving that screen.

### 6.2 Config file

- Path: `{config_dir}/ftm/config.toml`, where `{config_dir}` is
  `directories::ProjectDirs::from("", "", "ftm").config_dir()`
  (`~/Library/Application Support/ftm/` on macOS,
  `~/.config/ftm/` on Linux, `%APPDATA%\ftm\` on Windows).
- Format: TOML.
- If the file is absent, defaults are used and a fully-commented file with the
  default values is written on first clean exit.
- If the file is present but malformed, the game **must not** crash: it logs a
  one-line warning to stderr after terminal teardown, uses defaults for the
  unreadable keys, and leaves the file untouched.
- Unknown keys are ignored (forwards compatibility), but reported in the same
  warning, as are values clamped or rejected by the ranges in §6.3.

### 6.3 Schema and defaults

Every range given below is **inclusive**, and every one of them is enforced. A
value outside its range is **clamped** to the nearest end; a value of the wrong
type, or a string outside its enumerated set, is **rejected** and the default
used. Either way the file is left untouched and one line is added to the warning
of §6.2. Clamping rather than rejecting numbers is deliberate: `preview_count =
9` says clearly enough what the player wanted.

The upper bounds exist to keep a typo playable, not to police taste. The lower
bounds are load-bearing — `lines_per_level` and `soft_drop_factor` are both
divisors, and a zero reaching the core is a crash.

```toml
[gameplay]
# Number of upcoming pieces shown in the preview window.
# Range: 1..=6. The upper bound is the height of the next box in §12.4.
preview_count      = 5
# Show the translucent ghost piece at the landing position.
ghost_piece        = true
# Make the hold mechanic available. When false the hold key is inert, the hold
# box is not drawn, and the binding is hidden from the controls panels.
hold_enabled       = true
# Lock-down rule: "extended" | "infinite" | "classic"  (see §9.11)
lock_down          = "extended"
# Starting level. Range: 1..=15, the levels the §9.9 speed curve is defined for.
start_level        = 1
# Lines required to advance one level (§9.9). Range: 1..=1000. A large value is
# a legitimate way to hold the speed constant; 0 is not, since the level
# threshold is a multiple of it.
lines_per_level    = 10
# Make 180-degree rotation available (an extension beyond the guideline; see
# §9.5). When false the 180 key is inert and the binding is hidden from the
# controls panels.
allow_180_rotation = true

[timing]
# All values in milliseconds, converted once to whole ticks at load (§6.6).
# Every one of them is a duration, so the ranges below are the durations that
# leave a playable game; a value that rounds to 0 ticks is raised to 1 except
# where §6.6 says 0 is meaningful.
lock_delay_ms       = 500   # §9.11.  Range: 0..=5000. For a delay that never
                            #         expires use lock_down = "infinite".
das_ms              = 170   # §10.3.  Range: 0..=1000.
arr_ms              = 30    # §10.3.  Range: 0..=1000. 0 means "every tick".
soft_drop_factor    = 20    # soft drop is this many times normal gravity
                            # (§9.10). Range: 1..=100. 1 is no speed-up at all;
                            # above about 20 it is a hard drop in all but name.
line_clear_delay_ms = 250   # §9.12.  Range: 0..=2000.
entry_delay_ms      = 0     # ARE, §9.12. Range: 0..=2000. 0 means the next
                            #         piece enters on the same tick.

[display]
# "auto" | "truecolor" | "256" | "16" | "mono"   (see §12.3)
color_depth   = "auto"
# Characters used to paint one occupied cell. Each must be exactly 2 display
# columns wide; one that is not is rejected with a warning and the default used
# (§12.2).
cell_filled   = "██"
cell_empty    = "  "
cell_ghost    = "▒▒"
# Draw a faint dotted grid in the empty playfield.
show_grid     = false
# Show frame rate, tick rate and internal timers.
show_debug    = false

[keys]
# See §10. Each action maps to a list of key names; any listed key triggers it.
# An empty list leaves that action unbound, which is a supported way to disable
# it. `pause` and `quit` are the exceptions: an empty list for either is
# rejected with a warning and the default restored, because between them they
# are the only way out of a game.
move_left     = ["Left"]
move_right    = ["Right"]
soft_drop     = ["Down"]
hard_drop     = ["Space"]
rotate_cw     = ["Up"]
rotate_ccw    = ["z", "Z"]
rotate_180    = ["a", "A"]
hold          = ["c", "C"]
pause         = ["Esc", "F1"]
quit          = ["q", "Q"]
restart       = ["r", "R"]
```

### 6.4 Command-line interface

```
ftm [OPTIONS]

Options:
      --preview <N>        Pieces shown in the preview window [1-6]
      --level <N>          Starting level [1-15]
      --no-ghost           Disable the ghost piece
      --hold / --no-hold   Enable or disable the hold mechanic
      --rot180 / --no-rot180
                           Enable or disable 180-degree rotation
      --lock-down <RULE>   extended | infinite | classic
      --color <MODE>       auto | truecolor | 256 | 16 | mono
      --seed <N>           Seed the randomiser (implies no high-score recording)
      --config <PATH>      Use an alternative config file
      --print-config       Write the effective config to stdout and exit
  -h, --help               Print help
  -V, --version            Print version
```

Each paired flag overrides the corresponding config key in both directions, so a
setting turned off in the config file can still be turned on for one run.

`--seed` makes a run fully deterministic and is intended for testing and for
reproducing bug reports. A seeded run is never written to the high-score table
(§14).

### 6.5 Rules settings versus presentation settings

Every setting belongs to exactly one of two classes, and the class is a property
of the setting, not of where it was written:

| Class | Sections | Owner |
|---|---|---|
| **Rules** | `[gameplay]`, `[timing]` | The party running the core. |
| **Presentation** | `[display]`, `[keys]` | Always the player at the terminal. |

Rules settings change what happens; presentation settings change only what it
looks like and which key produces which action. In v1.0 both classes come from
the same file, so this distinction has no visible effect — but the code must keep
them in separate structs (`RulesConfig`, `PresentationConfig`), because under
§19 the rules class becomes server-authoritative while the presentation class
stays local. A single flat `Config` struct would have to be torn apart later.

`RulesConfig` must be `Serialize + Deserialize + PartialEq`, and two peers
running the same `RulesConfig` and seed must produce identical games.

### 6.6 Milliseconds to ticks

The core measures time in ticks of 1/60 s (§15.1), but the config is written in
milliseconds because that is what a human wants to type. Every `[timing]` value
is therefore converted once, at config load, to an integer tick count:

```
ticks = max(1, round(ms × 60 / 1000))        for values that must not vanish
ticks = round(ms × 60 / 1000)                for values that may legitimately be 0
```

`entry_delay_ms` and `arr_ms` may be 0 (meaning "same tick"); every other value
is clamped to a minimum of one tick. The conversion happens in the shell, and the
core sees only ticks, so two peers with the same `[timing]` table derive the same
tick counts regardless of platform. The defaults in §6.3 convert as:

| Setting | ms | ticks |
|---|---|---|
| `lock_delay_ms` | 500 | 30 |
| `das_ms` | 170 | 10 |
| `arr_ms` | 30 | 2 |
| `line_clear_delay_ms` | 250 | 15 |
| `entry_delay_ms` | 0 | 0 |

Because `das_ms` and `arr_ms` are consumed by the shell (§10.3), they are
converted for consistency but may be applied at sub-tick resolution locally.

---

## 7. Application states

```
            ┌───────────────────────────────────────────────┐
            │                                               │
            ▼                                               │
      ┌──────────┐  Play          ┌─────────┐  quit         │
      │ Attract  │───────────────▶│ Playing │───────────────┤
      │          │                │         │               │
      └──────────┘                └─────────┘               │
        ▲    │ Options/Controls      │    ▲                 │
        │    ▼                       │    │ resume          │
        │  ┌──────────┐              │  ┌────────┐          │
        │  │ Menu     │              ├─▶│ Paused │          │
        │  │ overlay  │              │  └────────┘          │
        │  └──────────┘              │                      │
        │                            ▼ top out              │
        │                     ┌────────────┐                │
        │                     │ Game Over  │                │
        │                     └────────────┘                │
        │                            │                      │
        │                            ▼ (if qualifying)      │
        │                     ┌────────────┐                │
        └─────────────────────│ Name Entry │────────────────┘
                              └────────────┘
```

The state is in **two levels**, because §15 specifies two loops: which screen
is running, and — while a game is running — where inside it the player is.

```rust
enum Screen {                 // which loop is running
    Attract,                  // §15.3, 10 fps, no game
    Playing,                  // §15.2, 60 Hz with an accumulator
    Quitting,
}

enum Phase {                  // where a game is; only under `Playing`
    Playing,
    Paused { selected: usize },
    Options { selected: usize },   // §13.5's panel, over the paused playfield
    Controls,                      // §10.1's binding table, likewise
    Resuming { since: Instant },   // §9.17's 3-2-1 countdown
    GameOver { since: Instant },
    NameEntry { rank: usize },
}
```

One flat enum would have to hold a `Game` in six of its seven variants and move
it on every transition; splitting it puts the game and its input state in one
place and leaves the screen a three-way choice. The extra phases are the ones
that are drawn over a game rather than instead of it: the two sub-screens the
pause menu opens, and the countdown, during which the clock is still stopped.

`Attract` and `NameEntry` carry state of their own; nothing else does.

Transitions:

| From | Trigger | To |
|---|---|---|
| `Attract` | menu item **Play** activated | `Playing` (fresh `Game`) |
| `Attract` | **Quit** activated, `quit` key, or `Esc` | `Quitting` |
| `Attract` | **High scores** / **Controls** / **Options** | the sub-screen; `Esc` returns |
| `Playing` | `pause` key | `Paused` |
| `Playing` | `quit` key | `Attract` (game abandoned, not scored) |
| `Playing` | `restart` key, held 1 s | `Playing` (fresh `Game`) |
| `Playing` | top out (§9.16) | `GameOver` |
| `Paused` | `pause` key, `Esc`, or **Resume** | `Resuming`, then `Playing` |
| `Paused` | **Restart** | `Playing` (fresh `Game`) |
| `Paused` | **Options** / **Controls** | that phase; any menu key returns |
| `Paused` | **Quit to menu** | `Attract` |
| `GameOver` | score qualifies for the table and run is unseeded | `NameEntry` |
| `GameOver` | otherwise; any key after a 1 s lockout | `Attract` |
| `NameEntry` | `Enter` | `Attract` (score saved) |
| `NameEntry` | `Esc` | `Attract` (score discarded) |

The pause menu's **Restart** does not need §10.1's one-second hold: choosing an
item from a menu is already the deliberate act the hold is there to require.

The game clock does not advance in `Paused`, `Options`, `Controls`, `Resuming`,
`GameOver` or `NameEntry`.

---

## 8. Terminal handling

### 8.1 Startup

On start, in this order:

1. Parse CLI arguments; load and merge config (§6).
2. Install a panic hook that restores the terminal (§8.3) **before** printing the
   panic message, so a crash never leaves the user with a broken shell.
3. Enable raw mode.
4. Enter the alternate screen (`EnterAlternateScreen`).
5. Hide the cursor.
6. Enable bracketed paste **off**, mouse capture **off**.
7. Attempt to push keyboard enhancement flags (§8.2).
8. Create the `ratatui` terminal with the `CrosstermBackend` over `stdout`.

### 8.2 Key-release detection

Standard terminals report only key presses; a held key produces a stream of
auto-repeat presses at the operating system's repeat rate and produces no event
at all when released. This is inadequate for a game that needs to know whether
left is *currently* held.

The implementation must therefore support two input modes:

**Enhanced mode (preferred).** Push
`KeyboardEnhancementFlags::REPORT_EVENT_TYPES | DISAMBIGUATE_ESCAPE_CODES` via
crossterm's `PushKeyboardEnhancementFlags`, and verify support by querying
`crossterm::terminal::supports_keyboard_enhancement()`. When supported, the
terminal delivers `KeyEventKind::Press`, `::Repeat` and `::Release`, and the game
tracks true held-key state. DAS and ARR (§10.3) are then driven entirely by the
game clock, and terminal auto-repeat events are **ignored** (`KeyEventKind::Repeat`
is discarded).

**Legacy mode (fallback).** When enhancement is unsupported, there are no release
events. A key is treated as held from its first press until `hold_timeout`
milliseconds have elapsed with no further event for that key, where
`hold_timeout = 90 ms`. This is longer than any common terminal auto-repeat
interval (typically 30–50 ms) and shorter than a deliberate re-press. In legacy
mode:

- DAS and ARR are still driven by the game clock, not by the terminal's repeat
  rate; incoming repeat events only refresh the "still held" timestamp.
- Soft drop is applied while the down key is considered held, and stops
  `hold_timeout` after the last event. The resulting up-to-90 ms of overshoot is
  accepted.
- The status bar shows a small `legacy-keys` indicator when `show_debug` is on.

The active mode must be reported by `--print-config` and on the Controls panel of
the attract screen, because it materially changes feel.

### 8.3 Shutdown

Teardown runs on normal exit, on error, and from the panic hook, and is
idempotent:

1. Pop keyboard enhancement flags (if pushed).
2. Show the cursor.
3. Leave the alternate screen.
4. Disable raw mode.
5. Flush stdout.

Any warning accumulated during the run (bad config keys, unwritable high-score
file) is printed to stderr **after** teardown.

### 8.4 Resize

`Event::Resize` invalidates the whole frame and triggers a full redraw. If the
terminal is smaller than the minimum size (§12.1), the game switches to the
"terminal too small" screen; if a game was in progress it is forced into
`Paused` first, so the player is never killed by a window resize.

---

## 9. Game rules

### 9.1 The matrix

- The matrix is 10 columns × 40 rows (§5). Rows `20..=39` are visible; rows
  `0..=19` form the buffer zone and are never drawn.
- Storage: `[[Option<Cell>; 10]; 40]` or an equivalent flat array. `None` is an
  empty cell; `Some(Cell)` records which tetromino type filled it, so that locked
  minos keep their colour.
- Cells outside the matrix are treated as **solid** for collision purposes on the
  left, right and bottom edges, and as **empty** above row 0. A piece may never
  be positioned with any mino above row 0; spawn is defined so this cannot occur.

### 9.2 Tetrominoes and colours

Seven pieces, with the guideline colours:

| Piece | Colour name | RGB (truecolor) | 256-colour | 16-colour | Mono glyph |
|---|---|---|---|---|---|
| `I` | Cyan | `#00F0F0` | 51 | BrightCyan | `I` |
| `O` | Yellow | `#F0F000` | 226 | BrightYellow | `O` |
| `T` | Purple | `#A000F0` | 129 | Magenta | `T` |
| `S` | Green | `#00F000` | 46 | BrightGreen | `S` |
| `Z` | Red | `#F00000` | 196 | BrightRed | `Z` |
| `J` | Blue | `#0000F0` | 21 | BrightBlue | `J` |
| `L` | Orange | `#F0A000` | 208 | Yellow | `L` |

The ghost piece uses the piece's colour at reduced intensity (§12.3).

This table is what the **core** names a piece by, and it is the guideline's. It
is not quite what reaches the terminal: purple, red and blue are too dark to
draw against a dark background, so §12.3 levels them on the way out. That is a
presentation decision and it stops at the renderer — a §19 client is handed the
colour above and may draw it however it likes.

### 9.3 Orientations and cell patterns

Each piece is defined by a square bounding box whose contents rotate in place.
`I` uses a 4 × 4 box, `O` a 2 × 2 box, and the other five a 3 × 3 box. The box's
position in the matrix is given by its **origin**: the matrix coordinate of the
box's top-left corner. Rotating changes only the box contents, never the origin;
any positional adjustment comes from the kick tables (§9.5).

Orientations are numbered `0 = North` (spawn), `1 = East` (one clockwise turn
from spawn), `2 = South`, `3 = West`. Clockwise increments the number modulo 4.

Rows are listed top to bottom, columns left to right.

**I** (4 × 4)

```
   North (0)      East (1)       South (2)      West (3)
  . . . .        . . I .        . . . .        . I . .
  I I I I        . . I .        . . . .        . I . .
  . . . .        . . I .        I I I I        . I . .
  . . . .        . . I .        . . . .        . I . .
```

**O** (2 × 2) — identical in all four orientations; `O` never kicks.

```
  O O
  O O
```

**T** (3 × 3)

```
   North          East           South          West
  . T .          . T .          . . .          . T .
  T T T          . T T          T T T          T T .
  . . .          . T .          . T .          . T .
```

**S** (3 × 3)

```
   North          East           South          West
  . S S          . S .          . . .          S . .
  S S .          . S S          . S S          S S .
  . . .          . . S          S S .          . S .
```

**Z** (3 × 3)

```
   North          East           South          West
  Z Z .          . . Z          . . .          . Z .
  . Z Z          . Z Z          Z Z .          Z Z .
  . . .          . Z .          . Z Z          Z . .
```

**J** (3 × 3)

```
   North          East           South          West
  J . .          . J J          . . .          . J .
  J J J          . J .          J J J          . J .
  . . .          . J .          . . J          J J .
```

**L** (3 × 3)

```
   North          East           South          West
  . . L          . L .          . . .          L L .
  L L L          . L .          L L L          . L .
  . . .          . L L          L . .          . L .
```

Implementations may store these as 16-bit bitmasks (one bit per box cell, row
major) or as arrays of four `(dx, dy)` offsets per orientation. Either way the
patterns above are normative; a unit test must assert that every piece has
exactly four minos in every orientation, and that rotating a piece four times
clockwise returns the original pattern.

### 9.4 Spawn

- Every piece spawns in orientation `North`.
- Spawn origins (§9.3), in matrix coordinates `(col, row)`:

| Piece | Box | Spawn origin | Resulting minos |
|---|---|---|---|
| `I` | 4 × 4 | `(3, 18)` | `(3,19) (4,19) (5,19) (6,19)` |
| `O` | 2 × 2 | `(4, 18)` | `(4,18) (5,18) (4,19) (5,19)` |
| `T` | 3 × 3 | `(3, 18)` | `(4,18) (3,19) (4,19) (5,19)` |
| `S` | 3 × 3 | `(3, 18)` | `(4,18) (5,18) (3,19) (4,19)` |
| `Z` | 3 × 3 | `(3, 18)` | `(3,18) (4,18) (4,19) (5,19)` |
| `J` | 3 × 3 | `(3, 18)` | `(3,18) (3,19) (4,19) (5,19)` |
| `L` | 3 × 3 | `(3, 18)` | `(5,18) (3,19) (4,19) (5,19)` |

  Every piece therefore spawns with its lowest minos in row 19 — one row above
  the visible playfield — horizontally centred, with the flat side down.

- **Immediately after spawning**, the piece drops one row if that position is
  unobstructed. This is a single unconditional attempt, not a gravity step, and
  it neither scores nor resets any timer. After it, a piece on an empty board has
  its lowest minos in row 20, the topmost visible row.
- If the spawn position itself is obstructed, the game ends by **Block Out**
  (§9.16). The drop-one attempt is skipped in that case.

### 9.5 Rotation: Super Rotation System

A rotation attempt from orientation `from` to orientation `to` proceeds as:

1. Compute the rotated cell pattern for `to`.
2. For each `(dx, dy)` in the kick table row for `(from, to)`, in order, test the
   piece at origin `(origin.x + dx, origin.y + dy)`.
3. The first offset that produces no collision with a filled cell or a wall/floor
   is accepted: the origin and orientation are updated, the offset index is
   recorded (needed for T-spin detection, §9.13), and the rotation **succeeds**.
4. If no offset fits, the rotation **fails**: nothing changes, no timer is reset,
   and no sound or score results.

`O` never kicks: its rotation always succeeds with offset `(0, 0)` and no visual
change.

> Remember §5: positive `dy` is **downward**. These tables are already converted.

**Kick table for `J`, `L`, `S`, `T`, `Z`:**

| From → To | Test 1 | Test 2 | Test 3 | Test 4 | Test 5 |
|---|---|---|---|---|---|
| `0 → R` | (0, 0) | (−1, 0) | (−1, −1) | (0, +2) | (−1, +2) |
| `R → 0` | (0, 0) | (+1, 0) | (+1, +1) | (0, −2) | (+1, −2) |
| `R → 2` | (0, 0) | (+1, 0) | (+1, +1) | (0, −2) | (+1, −2) |
| `2 → R` | (0, 0) | (−1, 0) | (−1, −1) | (0, +2) | (−1, +2) |
| `2 → L` | (0, 0) | (+1, 0) | (+1, −1) | (0, +2) | (+1, +2) |
| `L → 2` | (0, 0) | (−1, 0) | (−1, +1) | (0, −2) | (−1, −2) |
| `L → 0` | (0, 0) | (−1, 0) | (−1, +1) | (0, −2) | (−1, −2) |
| `0 → L` | (0, 0) | (+1, 0) | (+1, −1) | (0, +2) | (+1, +2) |

**Kick table for `I`:**

| From → To | Test 1 | Test 2 | Test 3 | Test 4 | Test 5 |
|---|---|---|---|---|---|
| `0 → R` | (0, 0) | (−2, 0) | (+1, 0) | (−2, +1) | (+1, −2) |
| `R → 0` | (0, 0) | (+2, 0) | (−1, 0) | (+2, −1) | (−1, +2) |
| `R → 2` | (0, 0) | (−1, 0) | (+2, 0) | (−1, −2) | (+2, +1) |
| `2 → R` | (0, 0) | (+1, 0) | (−2, 0) | (+1, +2) | (−2, −1) |
| `2 → L` | (0, 0) | (+2, 0) | (−1, 0) | (+2, −1) | (−1, +2) |
| `L → 2` | (0, 0) | (−2, 0) | (+1, 0) | (−2, +1) | (+1, −2) |
| `L → 0` | (0, 0) | (−1, 0) | (+2, 0) | (−1, −2) | (+2, +1) |
| `0 → L` | (0, 0) | (+1, 0) | (−2, 0) | (+1, +2) | (−2, −1) |

**180° rotation** is an extension beyond the guideline and is available only when
`allow_180_rotation = true` (the default). When available: the piece is tested
only at offset `(0, 0)` for the opposite orientation; if that collides, the
rotation fails. There are no 180° kick tests. A successful 180° rotation resets
the lock-down move counter like any other rotation, and can produce a T-spin only
by the corner rule with `kick_index = 0` (§9.13).

When `allow_180_rotation = false`, the `rotate_180` key is inert — it produces no
action, no lock-delay reset and no sound — and the binding is omitted from the
in-game controls overlay and from the attract screen's controls panel (§13.3,
§13.5). Nothing else in the rules changes, and the setting may be toggled between
games but not during one.

### 9.6 Randomiser: 7-bag

- A bag contains one of each of the seven tetrominoes.
- When the bag is empty it is refilled with all seven and shuffled with a
  Fisher–Yates shuffle using the run's RNG.
- Pieces are drawn from the front of the bag.
- The **next queue** is kept filled to at least `preview_count + 1` pieces at all
  times by drawing from the bag, refilling the bag as needed. Because the queue
  may straddle a bag boundary, the maximum meaningful preview is 6; this is why
  `preview_count` is clamped to `1..=6`.
- The RNG is `rand::rngs::SmallRng`, seeded from the OS by default or from
  `--seed`. Given a fixed seed the entire piece sequence must be reproducible.

The first piece of a game must never be `S` or `Z` (a guideline courtesy): if the
first bag's first piece is `S` or `Z`, swap it with the first non-`S`/`Z` piece in
that bag before play begins. This applies only to the very first bag of a game.

### 9.7 Hold

- Bound to `C` (§10), and available only when `hold_enabled = true` (the
  default). When `hold_enabled = false` the hold key is inert, the hold box is
  not drawn (the left column then holds only the stats box, §12.4), and the
  binding is omitted from both controls panels. The setting may be toggled
  between games but not during one.
- The hold slot starts empty.
- Pressing hold with an **empty** slot: the current piece is moved to the hold
  slot, and the next piece is pulled from the queue and spawned normally (§9.4).
- Pressing hold with an **occupied** slot: the current piece and the held piece
  are exchanged; the piece coming out of hold is spawned normally (§9.4), in
  orientation `North`, with its spawn origin — not at the current piece's
  position or orientation.
- Hold may be used **once per piece**. The lock-out flag is set on use and
  cleared when the next piece locks (not when it spawns). While locked out, the
  hold key does nothing and the hold box is drawn dimmed.
- A hold never awards score, never resets the drop timer's line count, and clears
  the current piece's lock-delay state entirely (the incoming piece begins fresh).
- If spawning the piece taken out of hold is obstructed, the game ends by Block
  Out (§9.16).

### 9.8 Ghost piece

- When `ghost_piece = true`, a translucent copy of the current piece is drawn at
  the position it would occupy after a hard drop: same columns and orientation,
  moved down as far as it will go.
- The ghost is recomputed after any movement, rotation, hold or gravity step.
- The ghost is drawn **behind** the current piece: where the two overlap, the
  current piece's cells are drawn.
- The ghost never affects collision, scoring or lock down.

### 9.9 Gravity, levels and speed

- The game starts at `start_level` (default 1).
- Level advances when cumulative cleared lines reach `lines_per_level × level`
  (default: every 10 lines). Level advances at most once per line clear event
  even if a single clear would cross two thresholds; any surplus lines carry
  over.
- Fall speed in **seconds per row** is:

  ```
  seconds_per_row(level) = (0.8 − ((level − 1) × 0.007)) ^ (level − 1)
  ```

- The level used for this formula is clamped to 15. Levels above 15 continue to
  increment for scoring and display, but do not get faster. (Without the clamp
  the base term goes negative at very high levels.)

| Level | s/row | | Level | s/row |
|---|---|---|---|---|
| 1 | 1.00000 | | 9 | 0.09388 |
| 2 | 0.79300 | | 10 | 0.06415 |
| 3 | 0.61780 | | 11 | 0.04298 |
| 4 | 0.47273 | | 12 | 0.02822 |
| 5 | 0.35520 | | 13 | 0.01815 |
| 6 | 0.26200 | | 14 | 0.01144 |
| 7 | 0.18968 | | 15 | 0.00706 |
| 8 | 0.13473 | | 16+ | 0.00706 |

  The table is informative; the formula is normative. Implementations must
  compute it, not hard-code the table.

- Gravity is accumulated with integer arithmetic, not as a timer that fires once
  per tick. On each level change the speed is converted once into a **fall period
  measured in ticks per row, in 16.16 fixed point**:

  ```
  fall_period = max(1, round(seconds_per_row(level) × TICK_HZ × 65536))   // u32
  ```

  Each tick, an accumulator (`u32`, same units) increases by `65536`; while the
  accumulator is ≥ `fall_period` the piece attempts to move down one row and
  `fall_period` is subtracted. The accumulator carries over across level changes.

  The accumulator is a **remainder**: it is always less than the period in
  force. Subtraction maintains that on its own while the period is constant, so
  the rule only bites when the period **shortens under it** — soft drop being
  pressed (§9.10), or a level-up — and there the accumulator is capped at one
  row's worth of the new period before the tick is accrued. The accrued
  fraction of a row is therefore kept, but a fraction can never pay out more
  than a row. Without the cap, charge banked at the slower period is re-read as
  whole rows at the faster one: fifty ticks of level-1 charge is seventeen rows
  at the default soft-drop period, so pressing `Down` late in a fall lands the
  piece on the floor — an accidental hard drop — and a level-up mid-fall pays
  out a hundred and forty-one rows at level 15.

| Level | `fall_period` | ticks per row | rows per tick |
|---|---|---|---|
| 1 | 3 932 160 | 60.000 | 0.02 |
| 5 | 1 396 691 | 21.312 | 0.05 |
| 10 | 252 254 | 3.849 | 0.26 |
| 13 | 71 382 | 1.089 | 0.92 |
| 15 | 27 756 | 0.424 | 2.36 |

  This table is informative in the same sense as the one above: it is the
  formula applied to the levels most worth checking, and the formula wins if
  they ever disagree. Note in particular that `fall_period` is computed from
  `seconds_per_row` at full precision — **not** from the five-decimal values
  printed in the speed table, which differ by up to 13 units of 1/65536.

  Expressing the speed as a *period* rather than as a rate is deliberate: at
  level 1 the period is exactly 3 932 160, so the piece falls on tick 60 and not
  on tick 61, whereas a per-tick rate of `round(65536/60)` accumulates to 65 520
  after 60 ticks and arrives a tick late. It also makes speeds above 1 G
  (levels 13+) fall out naturally — the piece can descend several rows in one
  tick.

  The float formula is evaluated only at level change and its result rounded to
  an integer immediately, so the rules themselves contain no floating point and
  the piece sequence is bit-identical on every platform (§3.1, §15.4).
- If a downward step is blocked, the accumulator is reset to 0 and the lock-down
  state machine takes over (§9.11).

### 9.10 Soft drop and hard drop

**Soft drop** (`Down`, held):

- While held, the fall period is divided by `soft_drop_factor` (default 20):
  `soft_period = max(1, fall_period / soft_drop_factor)` (integer division). It
  never makes the piece slower, since dividing a period can only shorten it. At
  level 1 this gives one row every 3 ticks; at level 15, roughly 47 rows per
  tick, which is a hard drop in all but name.
- Pressing soft drop part-way through a fall owes **at most one row** on the
  tick it is pressed, because the accumulator is capped at the shorter period
  first (§9.9). Soft drop is a faster fall, never a way to cash in the charge
  banked while the piece was falling slowly.
- Awards **1 point per row** actually descended under soft drop, unmultiplied by
  level.
- Soft drop does not lock the piece. Landing while soft-dropping begins normal
  lock delay (§9.11).
- Releasing and re-pressing soft drop has no special effect.

**Hard drop** (`Space`, on press):

- The piece is moved down until it collides, then **locked immediately** — lock
  delay is skipped entirely.
- Awards **2 points per row** descended, unmultiplied by level.
- A hard drop of zero rows (piece already resting) still locks the piece and
  awards zero drop points.
- The key must act on the press event only; auto-repeat and release events for
  the hard-drop key are ignored, so holding `Space` does not chain-drop pieces.

### 9.11 Lock down

The default rule is **Extended Placement**, per the guideline:

- The piece becomes *landed* when a downward move is blocked.
- On landing, a lock-delay counter of `lock_delay_ticks` (default 30 ticks =
  500 ms, §6.6) starts and counts down by one each tick.
- Any successful move (left, right) or rotation while landed **resets the timer to
  full** and increments a `move_counter`.
- `move_counter` is capped at **15**. Once 15 resets have been used, further moves
  and rotations are still permitted but no longer reset the timer.
- `move_counter` is reset to 0, and the timer cancelled, whenever the piece
  reaches a row **lower than any row it has previously occupied** during its
  lifetime (tracked as `lowest_row_reached`). Moving back up via a kick does not
  reset it.
- When the timer expires, or when the piece would be forced to lock by a hard
  drop, the piece locks: its minos are written into the matrix.
- If, at the moment the timer expires, the piece is no longer resting on anything
  (a move or rotation left it in mid-air), it does not lock; the timer is
  cancelled and normal gravity resumes.

Alternative rules selectable via `lock_down`:

- `"infinite"` — as above but `move_counter` is never capped.
- `"classic"` — the timer is set on landing and is never reset by moves or
  rotations.

### 9.12 Line clears

On lock:

1. Write the piece's minos into the matrix.
2. Determine the T-spin status of the lock (§9.13) **before** clearing rows.
3. Find all rows in `0..=39` that are completely filled.
4. Award score (§9.14) using the number of complete rows and the T-spin status.
5. If any rows are complete, enter a **line-clear pause** of
   `line_clear_delay_ms` (default 250 ms) during which the clearing rows are
   drawn with a flash animation (§12.5) and no input except pause is processed.
6. Remove the complete rows; all rows above each removed row shift down by one
   (naive gravity — no cascading, no sticky cells). New empty rows appear at the
   top of the buffer zone.
7. Update line count and level (§9.9).
8. Wait `entry_delay_ms` (ARE, default 0).
9. Clear the hold lock-out and spawn the next piece (§9.4).

### 9.13 T-spin detection

A lock is examined for T-spin status only if **all** of these hold:

- The piece is a `T`.
- The last successful action applied to the piece before locking was a
  **rotation** (not a move, not gravity, not a hold). A hard drop performed
  immediately after a rotation preserves the rotation as the last action.

Then, using the T's 3 × 3 bounding box, define the four corner cells of that box.
Two of them are the **front** corners — the two adjacent to the side the T points
towards — and two are the **back** corners:

| Orientation | Front corners (box-local) | Back corners (box-local) |
|---|---|---|
| North (points up) | top-left, top-right | bottom-left, bottom-right |
| East (points right) | top-right, bottom-right | top-left, bottom-left |
| South (points down) | bottom-left, bottom-right | top-left, top-right |
| West (points left) | top-left, bottom-left | top-right, bottom-right |

A corner counts as **occupied** if it holds a locked mino or lies outside the
matrix's left, right or bottom bounds. Cells above row 0 count as empty.

Classification:

- **T-Spin (proper)** if both front corners are occupied and at least one back
  corner is occupied.
- **T-Spin (proper)** regardless of the above if the rotation that placed the
  piece used **kick test 5** (the last row of the kick table); this is the rule
  that makes the classic T-spin triple score correctly.
- **T-Spin Mini** if exactly one front corner is occupied and both back corners
  are occupied.
- **No T-spin** otherwise (fewer than three occupied corners).

### 9.14 Scoring

All values below are multiplied by the current **level** except where stated.

| Action | Base | Back-to-back |
|---|---|---|
| Single (1 line) | 100 | — |
| Double (2 lines) | 300 | — |
| Triple (3 lines) | 500 | — |
| Quad (4 lines) | 800 | 1200 |
| T-Spin, no lines | 400 | — |
| T-Spin Single | 800 | 1200 |
| T-Spin Double | 1200 | 1800 |
| T-Spin Triple | 1600 | 2400 |
| T-Spin Mini, no lines | 100 | — |
| T-Spin Mini Single | 200 | 300 |
| T-Spin Mini Double | 400 | 600 |
| Combo (per clear after the first) | 50 × combo count | — |
| Soft drop | 1 per row (× 1, not × level) | — |
| Hard drop | 2 per row (× 1, not × level) | — |

Perfect clear bonuses (§9.15), added on top of the line-clear score and also
multiplied by level:

| Perfect clear with | Base | Back-to-back |
|---|---|---|
| Single | 800 | 1200 |
| Double | 1200 | 1800 |
| Triple | 1800 | 2400 |
| Quad | 2000 | 3200 |

Score is a `u64` and is not capped. The displayed score is the running total,
updated immediately, including drop points earned mid-piece.

### 9.15 Back-to-back, combo, perfect clear

**Difficult clears** are: any Quad, and any T-spin or T-spin Mini that clears at
least one line.

- **Back-to-back** applies when a difficult clear immediately follows another
  difficult clear with no non-difficult *line clear* between them. Locks that
  clear no lines do not break the chain.
- Any line clear of 1–3 lines that is not a T-spin breaks the chain.
- The B2B column in §9.14 replaces the base value; it is not added to it.
- The status bar shows `B2B` while the chain is active — that is, whenever the
  last line clear was a difficult one. A clear that *starts* a chain lights the
  indicator without itself being paid at the chained rate.

**Combo:**

- A combo counter starts at −1 and increments on every lock that clears at least
  one line; it resets to −1 on any lock that clears none.
- When the counter is ≥ 1, award `50 × counter × level`.
- The status bar shows `COMBO ×N` while the counter is ≥ 1.

**Perfect clear** (also "all clear"): after a line clear has been applied, the
entire matrix contains no locked minos. The bonus in §9.14 is awarded, and a
`PERFECT CLEAR` banner is shown for 1.5 s. Its back-to-back column is selected
by the same flag as the clear that emptied the board, not by the state of the
chain afterwards: a first Quad is 800 + 2000, and only the second is
1200 + 3200. Because the bonus is multiplied by level and §9.12 orders the award
before the line count advances the level, it uses the level the clear was scored
at.

### 9.16 Game over

The game ends in either of these ways:

- **Block Out** — a newly spawned piece (from the queue or from hold) overlaps a
  locked mino at its spawn position.
- **Lock Out** — a piece locks with **all four** of its minos above row 20, i.e.
  entirely inside the buffer zone.

On game over the final piece is drawn in place, the matrix is greyed out over
0.5 s top-to-bottom, and the `GameOver` overlay (§12.6) is shown. Input is
ignored for 1 s to prevent an accidental dismissal.

### 9.17 Pause

- `Esc` or `F1` toggles pause from `Playing`.
- While paused: all timers stop, the playfield is blanked (cells drawn as empty)
  to prevent pause-scumming, and a menu overlay offers **Resume**, **Restart**,
  **Controls**, **Quit to menu**.
- Unpausing resumes with a 3-2-1 countdown of 1 s per number, during which input
  other than pause is ignored and no timers run.

---

## 10. Controls

### 10.1 Default bindings

| Action | Primary | Alternate | Available |
|---|---|---|---|
| Move left | `←` | — | always |
| Move right | `→` | — | always |
| Soft drop | `↓` | — | always |
| Hard drop | `Space` | — | always |
| Rotate clockwise | `↑` | — | always |
| Rotate counter-clockwise | `Z` | — | always |
| Rotate 180° | `A` | — | when `allow_180_rotation` (§9.5) |
| Hold | `C` | — | when `hold_enabled` (§9.7) |
| Pause | `Esc` | `F1` | always |
| Restart (hold 1 s) | `R` | — | always |
| Quit to menu / quit | `Q` | — | always |

A binding marked *Available* conditionally is inert when its setting is off: the
key press is discarded before it reaches the action model (§10.2), so it cannot
reset a lock-delay timer or dismiss a banner as a side effect.

These are the guideline's recommended primary bindings. All are rebindable in
`[keys]` (§6.3). Key names in the config are: `Left`, `Right`, `Up`, `Down`,
`Space`, `Enter`, `Tab`, `Esc`, `Backspace`, `F1`–`F12`, and single characters
(case-sensitive; list both cases to accept both).

Attract-screen and overlay navigation always uses `↑`/`↓` to move, `Enter` or
`Space` to activate, and `Esc` to go back, regardless of the game bindings.

### 10.2 Action model

Input is converted from key events into a set of **actions**, and separately into
a set of **held states**:

```rust
enum Action {                 // edge-triggered, acted on once per press
    RotateCw, RotateCcw, Rotate180, Hold, HardDrop,
    Pause, Restart, Quit, MenuUp, MenuDown, MenuSelect, MenuBack,
}

struct Held {                 // level-triggered, sampled every frame
    left: bool,
    right: bool,
    soft_drop: bool,
}
```

Movement left/right and soft drop are driven from `Held`; everything else is an
`Action`. This separation is what makes DAS possible and must not be collapsed.

### 10.3 DAS and ARR

DAS and ARR are resolved **in the shell**, on the wall clock, and reach the core
only as `TickInput::shift` plus a whole number of cells to shift this tick
(§15.1). Keeping them out of the core is what allows an `arr_ms` finer than one
tick, and it is what makes §19's client-side input handling possible.

For each of left and right, independently:

1. On the transition from not-held to held, move one cell immediately.
2. Start the DAS timer. While the key remains held and the timer has not reached
   `das_ms` (default 170 ms), do nothing further.
3. After `das_ms`, move one cell every `arr_ms` (default 30 ms). Accumulate
   fractional time so that an `arr_ms` shorter than a frame produces multiple
   cells in one tick; `arr_ms = 0` means "move to the wall instantly".
4. On release, cancel both timers.

If both left and right are held, the **most recently pressed** direction wins,
and its DAS state is preserved rather than restarted when the other key is
released while it is still held.

Blocked movement (against a wall or stack) does not consume the DAS/ARR timers —
they keep running, so the piece slides as soon as the obstruction clears.

### 10.4 Input during non-play states

- During the line-clear pause and the unpause countdown, `Held` continues to be
  tracked (so DAS charge is not lost) but no movement is applied.
- During `GameOver` and `NameEntry`, game bindings are inactive; only the overlay
  bindings apply.

---

## 11. Game mode

Version 1.0 implements a single mode, **Marathon**:

- Endless play at increasing speed until top out.
- Level starts at `start_level`, advances every `lines_per_level` lines, and is
  unbounded (speed clamps at level 15, §9.9).
- Statistics tracked and displayed: score, level, lines cleared, elapsed time,
  pieces placed, and pieces-per-second (computed over the whole run, shown to one
  decimal place). Elapsed time is counted in **ticks** and converted for display
  (`ticks / 60`), so it advances with the game rather than with the wall clock
  and cannot drift from it.
- The run ends only by top out or by the player quitting; quitting abandons the
  run without recording a score.

---

## 12. Rendering

### 12.1 Terminal size

- **Minimum supported size: 60 columns × 24 rows.**
- Below the minimum, all screens are replaced by a centred message:

  ```
  Terminal too small
  Need 60x24, have 48x20
  Resize to continue
  ```

  and, if a game was in progress, it is forced into `Paused` (§8.4).
- Above the minimum, the whole UI is centred as a fixed-size block; extra space
  becomes margin. The UI does **not** stretch, because the playfield's aspect
  ratio must stay correct.

### 12.2 Cell rendering

- One matrix cell is drawn as **two terminal columns**, so that a cell is roughly
  square in typical fonts. All widths in this section are given in cells; the
  character width is twice that.
- An occupied cell is `cell_filled` (default `██`) in the piece's foreground
  colour, on the default background.
- An empty cell is `cell_empty` (default two spaces), or `··` dimmed when
  `show_grid = true`.
- A ghost cell is `cell_ghost` (default `▒▒`) in the piece's colour, dimmed.
- All three glyph strings must be exactly two display columns wide; a config that
  violates this is rejected with a warning and the default is used.

### 12.3 Colour depth

`color_depth = "auto"` selects, in order:

1. `mono` if `$NO_COLOR` is set (any value) or stdout is not a TTY. This test
   comes first because it *overrides* the others: a terminal that advertises
   truecolor and sets `$NO_COLOR` wants no colour.
2. `truecolor` if `$COLORTERM` is `truecolor` or `24bit`.
3. `256` if `$TERM` contains `256color`.
4. `16` otherwise.

In `mono`, cells are drawn as the piece's mono glyph (§9.2) — `I`, `O`, `T`, `S`,
`Z`, `J`, `L` — doubled (`II`), the ghost as `..`, and all emphasis uses bold and
reverse video only. The game must be fully playable in `mono`.

#### The levelled palette

§9.2's seven colours are equally saturated but not equally bright: their Rec.709
luma runs from blue's 17 to yellow's 223. A `J` piece at 17 is hard to pick out
of a dark terminal at all, and §13.2's wordmark, whose letters sit side by side,
reads as two different weights. So everything drawn in a piece colour — the
field, the ghost, the previews, the hold box, §13.4's drifting background and
§13.2's wordmark — draws from this palette instead:

| Colour | §9.2 | luma | Drawn | luma | 256-colour |
|---|---|---|---|---|---|
| Cyan | `#00F0F0` | 189 | `#00F0F0` | 189 | 51 |
| Green | `#00F000` | 172 | `#00F000` | 172 | 46 |
| Orange | `#F0A000` | 165 | `#F0A000` | 165 | 208 |
| Yellow | `#F0F000` | 223 | `#F0F000` | 223 | 226 |
| Purple | `#A000F0` | 51 | `#D58FF8` | 165 | 177 |
| Red | `#F00000` | 51 | `#F44040` | 102 | 203 |
| Blue | `#0000F0` | 17 | `#4848F4` | 84 | 63 |

A hue is lifted by blending it toward white, which is the only direction
available: a saturated blue or purple cannot be made as bright as cyan on any
display, so the brightness is bought with saturation. How much of that is worth
spending differs by hue, so the three do **not** land on one number. Purple
already carries two primaries and reaches 165 — orange's, the dimmest of the
four that were already bright — while still reading as purple. Red and blue
carry one primary each and gray out far faster: at 165 they are salmon and
lavender rather than red and blue, so they are lifted 45% of that far instead,
keeping about three-quarters of their saturation. A saturated hue also *looks*
brighter than its luma says (Helmholtz–Kohlrausch), most so for blue, which
closes much of the gap the numbers still show.

The four colours §12.3 leaves alone keep §9.2's own 256-colour entry, which is
authoritative — its orange is a deliberate choice, not the nearest cube cell.
A lifted colour has no such entry, so it takes the cube cell nearest the value
drawn. At 16 colours and in monochrome the palette cannot express a luminance
and §9.2 stands as written.

#### Dimming

Dimming for ghosts and inactive UI uses: an RGB scale of 0.45 in truecolor, a
darker palette entry in 256-colour, and the `DIM` attribute in 16-colour. The
scale runs from the **levelled** colour, not from §9.2's, so a piece and its
ghost are the same hue.

### 12.4 Playfield screen layout

The screen is a fixed block **44 characters wide by 23 rows tall**, centred in
the terminal. All widths below are given in characters; remember that one matrix
cell is two characters wide (§12.2).

```
 hold/stats      playfield        next
┌──────────┐ ┌──────────────┐ ┌──────────┐
│  10 ch   │ │    22 ch     │ │  10 ch   │    10 + 1 + 22 + 1 + 10 = 44 ch
└──────────┘ └──────────────┘ └──────────┘
```

Concrete mock-up at `preview_count = 5`, drawn to exact size (44 × 23):

```
┌────────┐ ┌────────────────────┐ ┌────────┐
│ HOLD   │ │                    │ │ NEXT   │
│  ██    │ │                    │ │  ████  │
│██████  │ │                    │ │  ████  │
└────────┘ │        ██          │ │        │
           │      ██████        │ │██      │
┌────────┐ │                    │ │██████  │
│ SCORE  │ │                    │ │        │
│  12480 │ │                    │ │    ██  │
│        │ │                    │ │██████  │
│ LEVEL  │ │                    │ │        │
│      4 │ │                    │ │        │
│        │ │                    │ │████████│
│ LINES  │ │                    │ │        │
│     37 │ │                    │ │  ██    │
│        │ │                    │ │██████  │
│ TIME   │ │                    │ └────────┘
│  02:14 │ │        ▒▒          │           
└────────┘ │      ▒▒▒▒▒▒        │           
           │      ██████████    │           
           │██████████████████  │           
           └────────────────────┘           
               B2B  COMBO x3                
```

Rules for the layout:

- **Playfield box**: 20 characters (10 cells) of interior, 20 rows tall, single-
  line border. Only matrix rows `20..=39` are drawn. A piece straddling row 20 is
  clipped: minos above row 20 are simply not drawn.
- **Hold box**: interior 4 cells × 2 cells, enough for any piece in `North`
  orientation. The piece is centred horizontally in the box. Drawn dimmed when
  hold is locked out for the current piece. Omitted entirely when
  `hold_enabled = false`, and the left column then contains only the stats box.
- **Next box**: interior 4 cells (8 characters) wide; one slot per previewed
  piece, each slot 2 cell-rows tall, with one blank row between adjacent slots.
  Its height is `2 (border) + 1 (label) + 2 × count + (count − 1)` rows: 17 rows
  at `preview_count = 5` and 20 at 6, so it always fits inside the 22-row
  playfield box and never needs to scroll at the minimum terminal size.
  Slot 0 (the next piece to spawn) is at the top and is drawn at full
  brightness; later slots are drawn progressively dimmer, in three steps
  (100 %, 75 %, and 55 % for slot 2 and beyond) where the colour depth allows.
- Should a future layout leave too little room for every slot, as many as fit are
  drawn and the last visible row of the box shows `+N` right-aligned.
- **Stats box**: score, level, lines, time (`MM:SS`, capped at `99:59`).
- **Debug strip**: with `show_debug = true` a bordered strip 44 characters wide
  and 5 rows tall is drawn **directly beneath the block**, making the whole
  thing 44 × 28. It shows the frame rate, ticks elapsed, any dropped ticks
  (§15.2), gravity in G, `fall_period`, lock-delay ticks remaining, DAS charge,
  current bag contents, and the input mode (`enhanced` or `legacy`).

  Beneath rather than inside the left column, because the column's interior is
  8 characters wide and the block has at most 3 spare rows under the stats box:
  those nine figures do not fit there at any height. The strip is a developer's
  read-out and not a supported layout, so it does not change the minimum
  terminal size of §12.1 — a terminal with fewer than 28 rows is simply drawn
  without it, and the game is unaffected.
- **Status line** (bottom, centred): shows `B2B` when the back-to-back chain is
  active, `COMBO xN` when the combo counter is ≥ 1, and the most recent clear's
  name (`QUAD`, `T-SPIN DOUBLE`, `PERFECT CLEAR`, …) for 1.5 s after it occurs.

### 12.5 Animations

All animations are purely cosmetic and must not affect the rules or timing of the
core. Each is **started by a `GameEvent`** (§12.8) and then runs on the shell's
own wall clock, independently of the tick rate. The core never knows an animation
is in progress.

| Animation | Duration | Description |
|---|---|---|
| Line clear flash | `line_clear_delay_ms` | Cleared rows alternate white / piece-colour at 12 Hz, then collapse. |
| Hard-drop trail | 120 ms | The columns the piece passed through are drawn dimmed behind it. |
| Lock flash | 80 ms | The locked piece is drawn white for one frame set. |
| Level-up banner | 1.2 s | `LEVEL 5` centred over the playfield, fading. |
| Perfect clear banner | 1.5 s | `PERFECT CLEAR` centred, in the seven piece colours cycling. |
| Game-over wipe | 500 ms | The stack greys from the top row downwards. |

If the frame rate cannot be sustained, animations are skipped rather than slowed.

### 12.6 Overlays

Overlays are drawn centred over the playfield with a cleared (space-filled)
background and a double-line border.

**Pause**

```
╔══════════════════╗
║      PAUSED      ║
║                  ║
║    ▸ Resume      ║
║      Restart     ║
║      Options     ║
║      Controls    ║
║      Quit to menu║
╚══════════════════╝
```

**Options** opens the §13.5 Options panel over the paused playfield. §6.1 calls
it "the in-game Options screen", and this is the in-game way in; the attract
screen's OPTIONS item (§13.5) reaches the same panel. **Controls** likewise
opens the §10.1 binding table over the same blanked playfield, and is the same
box the attract screen's CONTROLS item shows. Both return to the pause menu,
with the cursor back on the item that opened them; neither abandons the run.

**Game over**

```
╔══════════════════════╗
║      GAME OVER       ║
║                      ║
║  SCORE       12480   ║
║  LEVEL           4   ║
║  LINES          37   ║
║  TIME        02:14   ║
║  PIECES        128   ║
║  PPS           0.9   ║
║                      ║
║    Press any key     ║
╚══════════════════════╝
```

**Name entry** (only when the score qualifies for the top ten):

```
╔══════════════════════╗
║    NEW HIGH SCORE    ║
║          #3          ║
║                      ║
║   Name: msandifo_    ║
║                      ║
║   Enter to confirm   ║
╚══════════════════════╝
```

Name entry accepts up to 12 printable ASCII characters, `Backspace` deletes,
`Enter` confirms (an empty name becomes `ANON`), `Esc` cancels and discards the
score. The field is pre-filled with `$USER` (or `$USERNAME` on Windows),
truncated to 12 characters.

### 12.7 The view model

The renderer never reads `Game` directly. After each tick the core produces a
**`GameView`**: a flat, owned, serialisable snapshot containing everything any
screen needs to draw, and nothing else.

```rust
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct GameView {
    pub rows:      [[Option<PieceKind>; 10]; 20], // visible rows 20..=39 only
    pub current:   Option<PieceView>,             // absent during clear/entry delay
    pub ghost:     Option<PieceView>,             // absent when disabled
    pub hold:      Option<PieceKind>,
    pub hold_locked: bool,
    pub next:      Vec<PieceKind>,                // exactly `preview_count` entries
    pub score:     u64,
    pub level:     u32,
    pub lines:     u32,
    pub ticks:     u64,                           // elapsed game time, in ticks
    pub pieces:    u32,
    pub combo:     i32,
    pub back_to_back: bool,
    pub state:     PlayState,                     // Falling | Clearing | Entry | ToppedOut
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct PieceView { pub kind: PieceKind, pub cells: [(u8, u8); 4] }
```

Requirements:

- The view carries **only the visible rows** (20..=39), already clipped, so the
  buffer zone is never transmitted or drawn.
- `PieceView::cells` are absolute visible-field coordinates, already clipped;
  minos above row 20 are omitted, which is why the array may hold fewer than four
  distinct drawable cells (an omitted mino is encoded as `(255, 255)`).
- The view is **derived**, never authoritative: building it must not mutate the
  game. `Game::view(&self) -> GameView` is `&self`.
- `GameView` must not reference core internals by lifetime — it is owned, so it
  can be serialised, cached, queued, or diffed against the previous frame.
- It is cheap enough to build every tick (roughly 200 bytes plus the row array);
  no allocation beyond `next`.

This costs one struct in v1.0 and buys the renderer a stable contract: §12.4 is
written against `GameView`, so a change to the rules cannot silently break the
screen, and §19 gets its wire format for free.

**`DebugView`.** §12.4's debug strip needs four figures the view above does not
carry: gravity in G, `fall_period`, the lock-delay ticks remaining, and the
contents of the current bag. They travel in a second view type,

```rust
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct DebugView {
    pub milli_g:      u32,             // rows per tick x 1000, integer (§9.9)
    pub fall_period:  u32,             // 16.16 ticks per row (§9.9)
    pub lock_delay:   Option<u32>,     // ticks left, absent when not grounded
    pub bag:          Vec<PieceKind>,  // what is left of the current bag (§9.6)
}
```

built by `Game::debug(&self) -> DebugView`, a second `&self` constructor beside
`Game::view`, and **not** a field of `GameView`. Two reasons, and the second is
the load-bearing one:

- `show_debug` is a presentation setting (§6.5), so the core cannot be told
  whether to produce the strip. A `debug` field on `GameView` would therefore be
  filled on every frame of every game, whether or not anyone was looking.
- The bag beyond `preview_count` is **hidden information**. Under §19 `GameView`
  is what a server sends a player, and a field on it that reveals the rest of
  the bag is a field that has to be stripped before transmission. A separate
  type is not sent by accident.

The layering rule of §12.7 is unchanged by this: the renderer is handed a
`DebugView`, never a `Game`.

### 12.8 The event stream

State snapshots say what *is*; animations, banners and (later) sound need to know
what *happened*. `Game::tick` therefore returns the events raised by that tick:

```rust
pub enum GameEvent {
    PieceSpawned(PieceKind),
    PieceMoved,                       // any successful translation
    PieceRotated { kick_index: u8 },
    RotationFailed,
    HoldUsed,
    HoldRejected,
    PieceLocked { cells: [(u8, u8); 4], kind: PieceKind },
    HardDropped { rows: u8 },
    LinesCleared { rows: Vec<u8>, clear: ClearKind, b2b: bool, combo: i32 },
    PerfectClear,
    LevelUp(u32),
    ScoreAwarded { points: u64, reason: ScoreReason },
    ToppedOut(TopOutCause),           // BlockOut | LockOut
}
```

Rules:

- Events are emitted in the order the rules produced them within the tick.
- Event coordinates are **visible-field coordinates**, the same ones `GameView`
  uses (§12.7), so nothing downstream of the core has to know the buffer zone
  exists. A mino above the visible field is omitted from `PieceLocked::cells`,
  encoded as `(255, 255)`; a cleared row above it is omitted from
  `LinesCleared::rows`, which may therefore be shorter than the number of rows
  the clear removed.
- `PieceMoved` covers a player shift and a gravity step. The one-row drop that
  is part of spawning is reported by `PieceSpawned`, and the rows covered by a
  hard drop by `HardDropped`; neither raises `PieceMoved` as well.
- `LinesCleared` is raised when the rows are found, at the start of the
  line-clear pause (§9.12 step 5), so the flash animation covers the pause
  exactly. `PerfectClear` and `LevelUp` follow at the end of it, when the rows
  have actually been removed and the line count has moved.
- `LinesCleared::b2b` says whether *this* clear was paid at the back-to-back
  rate, which is what its banner announces; the standing `B2B` indicator of
  §9.15 is `GameView::back_to_back` and the two differ on the clear that starts
  a chain. `LinesCleared::combo` is the counter with this clear already counted,
  so the first clear of a run reports 0.
- `ScoreAwarded` is raised for each component separately — the clear, the combo,
  the perfect clear and the drop are four events, not one total — and never for
  zero points, so a hard drop of no rows is silent. The running total in
  `GameView::score` is exactly the sum of them.
- A lock that spun without completing a row raises no `LinesCleared`: the only
  notice of it is `ScoreAwarded` with a `ClearKind` of `TSpin` or `TSpinMini`.
- An empty vector is the common case and must not allocate (return
  `SmallVec<[GameEvent; 4]>` or reuse a caller-supplied buffer).
- Events are a **notification**, never a mechanism: the core's own behaviour must
  be identical whether or not anyone consumes them. Dropping every event must
  change nothing about the game.
- Everything cosmetic in §12.5 and §12.4's status line is driven from events:
  `LinesCleared` starts the flash and sets the status text, `LevelUp` raises the
  banner, `PerfectClear` raises its banner, `PieceLocked` triggers the lock
  flash, `HardDropped` the trail.

---

## 13. Attract screen

> This section is deliberately provisional: it is the one part of the
> specification expected to be refined (§18), and the only one whose looks
> §17.3 never judged. It has been built and looked at, and what is written here
> is what the code draws — §13.3's mock-up is a test — but the rest of the
> specification does not depend on its details, and changes here should not
> disturb §9.

### 13.1 Purpose and behaviour

The attract screen is what the program shows whenever no game is in progress. It
must: identify the game, show how to start it, teach the controls without being
asked, and be pleasant to leave running.

### 13.2 Wordmark

Drawn from block characters (not game cells, so the whole screen still fits in
60 columns), 30 characters wide and 5 rows tall, with each character of the
letterforms doubled horizontally so that three letters still carry the screen:

```
████████  ████████  ██      ██
██          ████    ████  ████
██████      ████    ██  ██  ██
██          ████    ██      ██
██          ████    ██      ██
```

The three letters take the `I`, `S` and `T` tetromino colours — cyan, green and
purple — left to right, from §12.3's levelled palette, which is what every other
piece colour on the screen is drawn from too. Levelling them is what stops the
purple `M` reading as a lighter weight of the same letterform than the cyan `F`;
§12.3 has the table and the derivation. §13.6's idle cycle walks all seven, so
the four the static wordmark never shows are levelled by the same rule.

This is an original block-letter wordmark. **The official Tetris logo must not be
used, reproduced, or approximated**, and no official colours-as-branding, styling
or artwork may be copied (§1.3).

### 13.3 Layout

```
               ████████  ████████  ██      ██
               ██          ████    ████  ████
               ██████      ████    ██  ██  ██
               ██          ████    ██      ██
               ██          ████    ██      ██

                 FALLING TETROMINO MANAGER

                     ▸ PLAY
                       HIGH SCORES
                       CONTROLS
                       OPTIONS
                       QUIT

            ┌──────────────────────────────────┐
            │    ←→ move          ↑ rotate     │
            │     ↓ soft drop SPACE drop       │
            │     Z rotate ccw    A rotate 180 │
            │     C hold                       │
            └──────────────────────────────────┘
              v1.0   ↑↓ select   ENTER start
```

The screen is a fixed block **36 characters wide by 21 rows tall**, centred in
the terminal like every other screen (§12.1). The mock-up above shows it centred
in 60 columns. The width is the controls panel's, not the wordmark's: the
wordmark is 30 characters and is centred within the block.

The 21 rows are wordmark (5), blank, subtitle, blank, menu (5), blank, panel
(6), footer — the footer sits directly under the panel, with no gap.

- The controls panel lists only the bindings that are actually available: the
  `C hold` entry is omitted when `hold_enabled = false`, and the 180° entry is
  shown only when `allow_180_rotation = true`. The remaining entries reflow to
  fill the panel, two to a row.
- The panel is **four rows** tall inside its border. Two of the seven control
  entries are optional, so three rows of two cannot hold them all with every
  setting turned on; four rows is also what the high-score face wants, which is
  a heading and the top three.
- The menu is navigated with `↑`/`↓` (wrapping) and activated with `Enter` or
  `Space`. The selected item is marked `▸` and drawn in the `I`-piece cyan.
- The panel beneath the menu **cycles every 6 seconds** between three faces:
  1. the quick control summary shown above;
  2. the top three high scores, under a `HIGH SCORES` heading;
  3. a one-line rules reminder (`Clear 4 rows at once for a QUAD` and similar,
     rotating through a short list).
  The cycle pauses while a menu item other than **PLAY** is selected.

### 13.4 Background animation

Behind the wordmark and menu, a slow ambient animation runs at 10 fps:

- Tetromino outlines (drawn with `░░` cells in each piece's colour, heavily
  dimmed) drift down the screen from the top, one new piece every ~1.2 s at a
  random column, falling one row every ~0.6 s, each in a random orientation.
- Pieces are removed when they leave the bottom of the screen; at most 12 exist
  at once.
- The animation never draws over the wordmark, menu, panel or footer: those
  regions are painted after it, opaquely.
- The animation is disabled in `mono` colour mode and when `show_debug` is on.

### 13.5 Sub-screens

- **HIGH SCORES** — the full top-ten table (rank, name, score, level, lines,
  date), with the most recently added entry highlighted. `Esc` returns.
- **CONTROLS** — the full binding table from §10.1, plus the active input mode
  (§8.2). `Esc` returns.
- **OPTIONS** — an editable list of the settings most worth changing without a
  text editor: preview count (1–6), starting level (1–15), ghost piece on/off,
  **hold on/off**, **180° rotation on/off**, lock-down rule, colour depth, grid
  on/off. `←`/`→` change the selected value, wrapping at each end, and `Esc`
  saves the config file (§6.2) and returns.

  The same panel is reached from the pause menu (§12.6), which is what §6.1
  means by "the in-game Options screen". Presentation settings — colour depth
  and the grid — take effect the moment the panel is left. Rules settings do
  not: a game already in progress keeps the rules it started under, so
  toggling hold, 180° rotation, the preview count, the starting level or the
  lock-down rule takes effect for the next game. This is what §6.5's split is
  for, and it is also the only answer that keeps a run deterministic (§15.4).

### 13.6 Idle behaviour

After 60 seconds with no key press on the attract screen, the wordmark's
per-letter colours begin a slow cycle (one step per second) to show the program
is alive. Any key stops it. (A self-playing demo is deliberately **not** in scope
for v1.0; see §18.)

---

## 14. High scores

- Path: `{data_dir}/ftm/highscores.json`, where `{data_dir}` is
  `ProjectDirs::data_dir()`.
- Format:

```json
{
  "version": 1,
  "entries": [
    { "name": "ANON", "score": 12480, "level": 4, "lines": 37,
      "duration_secs": 134, "date": "2026-09-04" }
  ]
}
```

- The table holds the **top 10** entries, sorted by score descending; ties are
  broken by earlier date first (so an existing entry keeps the better rank).
- A score qualifies if it is greater than the tenth entry's score, or the table
  has fewer than 10 entries. A score of 0 never qualifies.
- The file is written atomically: write to `highscores.json.tmp` in the same
  directory, then rename over the target.
- Any failure to read (missing, malformed, unreadable) yields an empty table and a
  warning at exit; the game must never fail to start because of it. Any failure
  to write yields a warning at exit and is otherwise ignored.
- Runs started with `--seed` are never recorded.

---

## 15. Game loop and timing

### 15.1 Fixed timestep

The core advances in **fixed ticks of 1/60 s**, never by a variable delta:

```rust
pub const TICK_HZ: u64 = 60;
pub const TICK: Duration = Duration::from_nanos(1_000_000_000 / TICK_HZ);
pub const MAX_CATCH_UP_TICKS: u32 = 6;     // 100 ms of arrears
```

```rust
impl Game {
    /// Advance exactly one tick. Pure: no clock, no I/O.
    pub fn tick(&mut self, input: &TickInput, out: &mut Vec<GameEvent>);
    pub fn view(&self) -> GameView;
}

pub struct TickInput {
    pub actions: Actions,                 // edge-triggered, this tick only
    pub soft_drop: bool,                  // level-triggered
    pub shift: Option<Shift>,             // Left | Right, already DAS-resolved
    pub shift_cells: u8,                  // cells to shift this tick (ARR ≥ 1)
}
```

`Actions` is a fixed-capacity inline list of at most four actions, not a `Vec`:
the core is called sixty times a second and must not allocate in order to do
nothing (§12.8). Four is more distinct actions than a player can produce in
16 ms, and a fifth in one tick is dropped. It is a handful of lines in the core
rather than a dependency, because §3's table is exhaustive.

A fixed timestep is required, not merely tidy. With a variable `Duration` the
gravity accumulator (§9.9) and the lock-delay timer (§9.11) depend on frame
pacing, so the same inputs on two machines — or on a client and a server —
produce different games. Every duration in the rules is therefore an integer tick
count derived once at config load (§6.6), and no rules timing is expressed as a
float.

Rendering is decoupled and may run at any rate. It is driven by the latest
`GameView` (§12.7); §12.5 animations interpolate on the wall clock between ticks.

### 15.2 The loop

```rust
let mut accumulator = Duration::ZERO;
let mut last = Instant::now();
```

Each iteration:

1. `let now = Instant::now(); accumulator += now - last; last = now;`
2. Drain **all** currently available terminal events with
   `event::poll(Duration::ZERO)` + `event::read()`, converting each into actions
   and held-state changes (§10.2). Draining rather than reading one event per
   frame is required, or fast typing lags behind.
3. Resolve DAS/ARR (§10.3) against the wall clock into this tick's `TickInput`.
   DAS lives in the shell, so its resolution is not limited to the tick rate.
4. While `accumulator >= TICK`, call `Game::tick` and subtract `TICK`, up to
   `MAX_CATCH_UP_TICKS` iterations; then, if the accumulator is still ≥ `TICK`,
   **discard the arrears** and record a dropped-ticks count for the debug
   display. Discarding rather than catching up is what stops a suspended laptop
   or a scrolled terminal from resuming into an instant death. Edge-triggered
   actions are consumed by the first tick of the batch only; held state applies
   to every tick in it.
5. Feed the accumulated `GameEvent`s to the animation and status-line state
   (§12.5, §12.8), then build the `GameView` and draw if it differs from the
   previous frame or an animation is running. `ratatui` diffs against the
   previous buffer, so an unchanged frame costs nothing.
6. Sleep for the remainder of the tick: `event::poll(time_to_next_tick)` is used
   instead of `thread::sleep`, so an incoming key wakes the loop early. This
   keeps input latency at roughly one tick while keeping idle CPU near zero.

### 15.3 Attract screen loop

The attract screen has no core to advance, so it runs the same loop at **10 fps**
with no accumulator, redrawing only when the background animation (§13.4) steps
or the selection changes. Idle CPU use on the attract screen must be under 2 % on
a modern machine.

### 15.4 Determinism obligations

- `Game::tick` reads no clock and performs no I/O.
- Given the same `RulesConfig`, the same seed and the same sequence of
  `TickInput`s, two runs must produce identical `GameView`s at every tick. This
  is asserted by the snapshot test in §17.2 and is the precondition for
  everything in §19.
- No rules decision may depend on the render rate, on how many ticks were batched
  in one iteration, or on how long a tick took to compute.

---

## 16. Error handling

- `main` returns `anyhow::Result<()>`; any error propagated to it is printed
  after terminal teardown (§8.3) with a non-zero exit code.
- Recoverable problems (bad config values, unreadable or unwritable high scores,
  unsupported keyboard enhancement) never abort: they degrade to a documented
  default and add a line to a `Vec<String>` of warnings printed at exit.
- Writes to stdout during play are ignored on failure; the frame is simply lost.
- The panic hook restores the terminal before the default panic handler runs, so
  a bug produces a readable backtrace rather than a wrecked terminal.
- `SIGINT` (Ctrl-C) is not trapped: crossterm in raw mode delivers it as a key
  event, which is mapped to "quit to menu" from `Playing` and "quit" from
  `Attract`.

---

## 17. Testing and acceptance

### 17.1 Required unit tests (core, no terminal)

1. **Piece geometry** — every piece has exactly 4 minos in all 4 orientations;
   four clockwise rotations restore the original pattern; the spawn coordinates in
   §9.4 match the table exactly.
2. **Kick tables** — both tables have 8 rows × 5 entries; `0→R` followed by `R→0`
   on an empty board returns the piece to its original origin.
3. **Wall kicks in practice** — an `I` piece against the left wall in `East`
   rotating to `North` ends at the expected origin; a `T` in a well one cell wide
   performs the T-spin triple kick.
4. **7-bag** — over 7000 pieces every type appears exactly 1000 times; no type
   appears three times within any window of 7 consecutive pieces; a fixed seed
   produces a fixed sequence; the first piece of a game is never `S` or `Z`.
5. **Gravity curve** — `seconds_per_row` matches the §9.9 table to 5 decimal
   places for levels 1–15 and is constant for levels 16–100; the derived
   `gravity_per_tick` is monotonically non-decreasing in level and never zero.
6. **Gravity arithmetic** — at level 1 a piece on an empty board falls on tick
   60 exactly, and on every 60th tick thereafter; at level 15 it falls more than
   one row in a single tick; with soft drop held at level 1 it falls one row
   every 3 ticks; `fall_period` matches the §9.9 table exactly for the listed
   levels and is never 0.
7. **Lock down** — extended placement locks after exactly 30 ticks; 15 resets extend it and
   the 16th does not; moving down to a new lowest row restores the reset budget;
   a piece that is no longer grounded when the timer expires does not lock.
8. **Line clears** — rows collapse correctly with naive gravity; clearing a row
   that has filled rows both above and below it preserves their contents in
   order.
9. **T-spin detection** — the canonical T-spin single, double and triple set-ups
   are detected as proper; the canonical mini set-up is detected as mini; a T
   moved (not rotated) into a three-corner slot is not a T-spin; a kick-test-5
   rotation is always proper.
10. **Scoring** — every row of the §9.14 table; B2B chains form and break
    correctly; combo counting; perfect clear detection and bonus.
11. **Top out** — Block Out on an obstructed spawn; Block Out on an obstructed
    hold swap; Lock Out when all four minos lock above row 20.
12. **Optional mechanics** — with `hold_enabled = false` the hold action is a
    no-op and the piece sequence is unaffected; with `allow_180_rotation = false`
    the 180 action is a no-op and does not reset the lock-delay timer; with both
    enabled, a 180° rotation into a blocked cell fails and changes nothing.
13. **DAS/ARR** — one cell on press, nothing until `das_ms`, then one cell per
    `arr_ms`; `arr_ms = 0` slides to the wall in one tick; the most recent of
    two held directions wins.
14. **Determinism** — the same `RulesConfig`, seed and `TickInput` sequence
    produce identical `GameView`s at every tick across two independent `Game`
    instances; replaying a recorded input log reproduces the final state exactly;
    batching ticks differently (1 × 6 versus 6 × 1) changes nothing.
15. **View model** — `Game::view` does not mutate (asserted by comparing the game
    state before and after); the view contains no buffer-zone rows; a piece
    straddling row 20 is clipped in the view, not by the renderer;
    `GameView` survives a serde round-trip unchanged.
16. **Event stream** — a tick that clears lines emits `LinesCleared` with the
    right rows, `ClearKind` and B2B flag; discarding every event leaves the game
    state bit-identical; the common tick emits no events and allocates nothing.
17. **Millisecond conversion** — the §6.6 table; `entry_delay_ms = 0` yields 0
    ticks; `lock_delay_ms = 1` yields 1 tick, not 0.

### 17.2 Required integration tests

- A scripted game: given `--seed 42` and a recorded input sequence, the final
  score, level, lines and matrix contents must match a checked-in snapshot. The
  same test re-run with the ticks fed in different batch sizes must produce the
  same snapshot — this is the desync canary for §19.
- Config round-trip: defaults → TOML → parse → identical struct.
- A malformed config file loads with defaults and produces exactly one warning.
- Rendering does not panic at 60 × 24, 80 × 24, 200 × 60, or 1 × 1.

### 17.3 Acceptance criteria

The implementation is complete when:

1. `cargo build --release` produces a binary with no warnings, and
   `cargo clippy -- -D warnings` is clean.
2. All tests in §17.1 and §17.2 pass.
3. The attract screen appears on launch, and **PLAY** starts a game.
4. All controls in §10.1 behave as specified, with working DAS.
5. `preview_count` is honoured for every value 1–6, from both the config file and
   `--preview`, and the layout adapts.
6. A full game can be played to a top out, the score is recorded, and it appears
   on the attract screen's high-score panel.
7. Quitting at any point restores the terminal exactly as it was found —
   verified by `stty -a` before and after.
8. The game is playable with `--color mono` and with `NO_COLOR=1`.
9. Hold and 180° rotation can each be turned off and on from the config file, the
   command line and the Options screen; when off, the key does nothing and the
   binding disappears from the hold box, the controls overlay and the attract
   screen's controls panel.
10. The renderer compiles against `GameView` alone. This is enforced by the
    compiler rather than audited: every module inside `core` is `pub(crate)`,
    so the core's whole public surface is its façade, and nothing under `ui/`
    can name `Game` or a rules module even by accident.

    The façade is `Game` itself (`new`, `tick`, `view`, `debug`), the input
    types `Action`, `Actions`, `Shift` and `TickInput` — which only `app` and
    `input` use — and the view and event types with the vocabulary they are
    written in: `GameView`, `PieceView`, `DebugView`, `VIEW_WIDTH`,
    `VIEW_HEIGHT`, `PlayState`, `GameEvent`, `ClearKind`, `ScoreReason`,
    `TopOutCause`, `OFF_SCREEN`, `PieceKind`, `Colour` and `Rotation`.

    The last three are in the list because a client handed a `GameView` has to
    *draw* it: the view's cells and its hold and next slots are `PieceKind`s,
    and turning one into minos on a screen needs §9.3's cell patterns and the
    `Rotation` they are indexed by, and §9.2's `Colour`. None of them is a
    rule. This is the check that §19 stays reachable.

---

## 18. Deferred and open items

These are recorded so that later refinement has a home; none are required for
v1.0.

- **Attract mode refinement** (§13) — the layout, the cycling panel and the idle
  behaviour are a first pass and are expected to change. A self-playing demo
  driven by a simple heuristic bot is the most likely addition; it would need a
  placement search and would reuse `Game` unchanged.
- **Additional modes** — Sprint (fastest 40 lines), Ultra (highest score in two
  minutes), Zen (no top out), and a Marathon variant that ends at level 15.
- **Sound** — terminal bell or an optional audio backend.
- **Replays** — since a seeded run plus an input log is fully deterministic
  (§15.4, §17.2), recording and playback is cheap to add, and is the natural
  first consumer of the machinery §19 requires.
- **Per-piece statistics** — a count of each tetromino received, shown on the
  game-over screen.
- **Colour themes** — a `[theme]` config table overriding the §9.2 palette.

---

## 19. Network readiness

**Nothing in this section is implemented in v1.0.** It exists because a future
client/server split — game logic on a server, possibly multi-user, with the
display on each player's terminal — is anticipated, and a few v1.0 decisions
would otherwise foreclose it. Those decisions are stated here, in one place, with
their reasons, so that they are not quietly undone during implementation.

### 19.1 The intended shape

```
   ┌────────────── client (per player) ──────────────┐   ┌──── server ────┐
   │  terminal → key decode → DAS/ARR → TickInput ───┼──▶│                │
   │  §8, §10.2, §10.3                               │   │  Game::tick    │
   │                                                 │   │  §15.1         │
   │  renderer ◀── GameView + GameEvent ─────────────┼───│  authoritative │
   │  §12                                            │   │                │
   └─────────────────────────────────────────────────┘   └────────────────┘
```

The split falls exactly on the core/shell boundary of §3.1. The server runs one
`Game` per player and never renders; the client renders and never decides
anything about the rules.

### 19.2 Constraints v1.0 must respect

These are already normative in the sections named; they are collected here
because each one is cheap to honour now and expensive to retrofit.

| # | Constraint | Section | What breaks without it |
|---|---|---|---|
| 1 | Core has no I/O and no clock | §3.1 | The rules cannot run headless on a server. |
| 2 | Core advances in fixed 1/60 s ticks | §15.1 | Client and server disagree about elapsed time; every timer desyncs. |
| 3 | Rules timing is integer ticks; gravity is fixed point | §6.6, §9.9 | Float accumulation diverges between peers and platforms. |
| 4 | Core is deterministic given seed + inputs | §15.4 | No prediction, no rollback, no replay, no desync detection. |
| 5 | Renderer draws only from `GameView` | §12.7 | The client would need the whole `Game`, so the server could not be authoritative. |
| 6 | Cosmetics are driven by `GameEvent`, not by polling state | §12.8 | Animations would need per-tick state diffs over the wire. |
| 7 | Rules config and presentation config are separate structs | §6.5 | The server could not impose the rules while the player keeps their own colours and keys. |
| 8 | DAS/ARR resolved client-side, not in the core | §10.3 | Input would require reliable held-key state over the network — impossible on terminals without key-release reporting (§8.2). |

### 19.3 What still has to be designed

Deliberately unresolved. Recording them here is not a commitment to any of them.

- **Transport and framing.** Most likely a length-prefixed stream of
  `bincode`- or CBOR-encoded frames over TCP or a Unix socket, with `GameView`
  sent as a delta against the last acknowledged view rather than in full.
- **Latency handling.** Two viable models: *thin client* (send input, wait for a
  view — simple, but adds a round trip to every keypress and will feel wrong
  above ~40 ms), or *client-side prediction with rollback* (the client runs the
  same deterministic core locally, and re-simulates from the last confirmed
  server state when a correction arrives). Constraints 2–4 exist so that the
  second option remains available; it is the only one that will feel right over
  the internet.
- **Trust.** With DAS on the client (constraint 8), the server receives
  already-interpreted movement and cannot distinguish a fast human from a bot. A
  server for friendly or co-operative play can accept this. A ranked one cannot,
  and would need raw key events plus client-side timestamps, which in turn needs
  the enhanced keyboard mode (§8.2) as a hard requirement rather than a
  preference.
- **Multi-user rules.** Garbage lines and an attack table, shared versus
  per-player bag seeds (a shared seed gives every player the same pieces and is
  the fairest and the simplest to verify), match lifecycle, spectating, and what
  happens to a match when one player disconnects. None of these threaten the
  architecture; all are additive.
- **Server-side persistence.** High scores (§14) become per-account and
  server-held; the local file becomes a fallback for offline play.
- **Attract screen in a networked client.** §13 is entirely local and needs no
  server; the menu simply gains a "Connect" item.

### 19.4 The single test that protects all of this

The batch-invariance assertion in §17.2 — the same input log fed in different
tick batchings must produce the same snapshot — is the cheapest available proxy
for "this core can be run authoritatively somewhere else". If that test passes,
constraints 2, 3 and 4 are holding. It should be run in CI from the first commit,
long before any networking exists.
