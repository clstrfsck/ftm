# Falling Tetromino Manager — Front-End Plan

**Companion to:** [FTM.md](FTM.md) (the specification), [PLAN.md](PLAN.md)
(the twelve stages that built v1.0)
**Date:** 2026-09-06
**Status:** proposed; no stage started.

This plan adds a second and a third front-end to FTM — a native windowed GUI on
`egui` / `eframe`, and the same GUI built for the browser as WebAssembly — and
restructures the tree so that a *fourth* front-end is an additive change rather
than another restructure. A Macroquad front-end is anticipated and is named
throughout as the thing the boundary must not foreclose; nothing in this plan
implements it.

It is written for the tree as it stands at the end of `PLAN.md` Stage 12: the
core is sealed behind its façade (A10), the shell is `app.rs`, `config.rs`,
`input.rs`, `highscore.rs` and `ui/`, and `main.rs` is the terminal entry point.

Section references written `§n` are to `FTM.md` and, after Stage G0, to whichever
of the front-end documents keeps that number — see [The documentation
split](#stage-g0--the-documentation-split). GUI sections are written `§Gn` and
live in `GUI.md`. Stages are `G0`–`G12`; the existing stages 0–12 are not
renumbered.

---

## Contents

- [The decisions this plan rests on](#the-decisions-this-plan-rests-on)
- [The central idea: the shell loses its clock too](#the-central-idea-the-shell-loses-its-clock-too)
- [Sequencing rationale](#sequencing-rationale)
- [Target layout](#target-layout)
- [Working agreements](#working-agreements)
- [Milestones](#milestones)
- [Stage G0 — The documentation split](#stage-g0--the-documentation-split)
- [Stage G1 — Front-end-neutral keys](#stage-g1--front-end-neutral-keys)
- [Stage G2 — The shell/front-end move](#stage-g2--the-shellfront-end-move)
- [Stage G3 — A platform-free shell](#stage-g3--a-platform-free-shell)
- [Stage G4 — Inverting the loop](#stage-g4--inverting-the-loop)
- [Stage G5 — GUI vertical slice](#stage-g5--gui-vertical-slice)
- [Stage G6 — The web slice](#stage-g6--the-web-slice)
- [Stage G7 — The playing screen](#stage-g7--the-playing-screen)
- [Stage G8 — Overlays and the pause path](#stage-g8--overlays-and-the-pause-path)
- [Stage G9 — Animations](#stage-g9--animations)
- [Stage G10 — The attract screen](#stage-g10--the-attract-screen)
- [Stage G11 — Config, CLI and persistence](#stage-g11--config-cli-and-persistence)
- [Stage G12 — Testing, CI and acceptance](#stage-g12--testing-ci-and-acceptance)
- [Macroquad readiness](#macroquad-readiness)
- [Invariants the front-ends must not break](#invariants-the-front-ends-must-not-break)
- [Hazards that do not carry over](#hazards-that-do-not-carry-over)
- [Test coverage map](#test-coverage-map)
- [Risk register](#risk-register)
- [Open decisions](#open-decisions)

---

## The decisions this plan rests on

These were settled before the plan was written. Everything below follows from
them; changing one changes the plan.

1. **The GUI is a native GUI, not a terminal in a window.** Pixel-native
   rendering: mino tiles drawn as rects, proportional side panels, a resizable
   window. It honours §12.7's information and §9.2's palette, but not §12.4's
   44 × 23 character grid.
2. **One crate, several binaries.** `src/shell/` is front-end-agnostic,
   `src/tui/` and `src/gui/` are the front-ends behind cargo features, with a
   `[[bin]]` target each and `required-features`.
3. **Full parity, staged.** The GUI ends up with everything the TUI has, but the
   playfield and the pause path land first and are separately acceptable; the
   attract screen and the sub-screens follow.
4. **The spec splits.** `FTM.md` becomes the front-end-agnostic specification;
   the terminal-specific sections move to `TUI.md`; a new `GUI.md` is normative
   for the egui front-end in both its native and web builds; a new
   `FRONTEND.md` states the contract *any* front-end is written against.
5. **A web build is in scope.** The egui front-end is compiled to
   `wasm32-unknown-unknown` and served in a browser. It is the same `gui/`
   code, not a fork.
6. **A Macroquad front-end is anticipated, not built.** It plays the role §19
   plays for networking: a list of constraints to honour now so that it stays
   cheap later. See [Macroquad readiness](#macroquad-readiness).

---

## The central idea: the shell loses its clock too

§3.1 gives the core two properties that made everything else possible: **no I/O
and no clock**, and **deterministic**. The shell was allowed the clock, the
filesystem, the terminal and ambient randomness, because there was only ever
going to be one shell and it ran on a machine with all four.

That assumption is what wasm breaks, and it breaks all of it at once:

| The shell does | On `wasm32-unknown-unknown` |
|---|---|
| `Instant::now()`, and stores `Instant` in `Cosmetics`, `Attract`, `Confirm` | `Instant::now()` **panics** — there is no such clock in the platform |
| `std::fs` + `directories` for §6.2's config and §14's scores | No filesystem. `localStorage`, keyed per origin |
| `rand::random::<u64>()` for the seed | `getrandom` needs an explicit `wasm_js` backend and a build-time `--cfg` |
| `chrono`'s `clock` feature for §14's date stamp | Needs `wasmbind`, which pulls in `wasm-bindgen` |
| `clap` over `std::env::args` | No argv. Settings arrive as URL query parameters |

Each of those has a per-platform shim, and taking the shims would work. This
plan does not take them, because there is a better answer that falls out of a
rule the project already believes:

> **§3.1's rule extends one layer out. The shell gains no clock, no filesystem,
> no entropy and no calendar of its own. All four are capabilities the
> front-end supplies.**

Concretely, the shell never calls `Instant::now`, `fs::read`, `rand::random` or
`chrono::Local::now`. It is handed a monotonic timestamp, a `Storage`
implementation, a seed and a date. The front-end — which knows whether it is a
terminal, a window or a browser tab — is the only thing that decides where those
come from.

Three things follow, and they are why this is the centrepiece rather than a
footnote:

- **`shell/` compiles for `wasm32-unknown-unknown` with no shims and no `cfg`.**
  That is a compiler-checkable property, and G3 puts it in CI, in exactly the
  spirit A10 put the core's façade there. A platform dependency cannot creep
  back in unnoticed.
- **The web build stops being a port.** It becomes a fourth capability provider,
  perhaps two hundred lines, and it cannot diverge from the native build because
  it *is* the native build.
- **Macroquad gets much cheaper**, because macroquad supplies time as an `f64`
  of seconds from `get_time()` and has its own asset and storage story. A shell
  that takes time as a value does not care which of the three produced it.

The cost is honest and worth stating: `Instant` disappears from the shell's
public API and is replaced by a newtype, which touches `Cosmetics`, `Attract`,
`Confirm` and `Round` — a wide but shallow edit — and `config.rs` and
`highscore.rs` grow a trait between them and their bytes. That is Stage G3, and
it is deliberately placed *before* the loop inversion, because the loop
inversion is the stage that fixes the shape of the shell's public API and it
should only be done once.

---

## Sequencing rationale

The risk in this project is not the GUI drawing code. `egui` is an immediate-mode
library over a `GameView` that is already a flat, owned, serialisable snapshot;
painting it is the easy half.

The risk is in four places, and the stage order retires them in order of how
expensive they are to discover late.

**First, the input boundary (G1).** `crossterm::event::KeyEvent` is threaded
through `input.rs`, `app.rs` and `ui/attract.rs`. §10.1's key-name grammar, the
DAS/ARR machine and the `[keys]` table are all shared property, so a neutral key
type is a prerequisite. It is first because it is small, mechanical, and fully
covered by the existing T13.

**Second, the platform boundary (G3).** Described above. It is before the loop
inversion because both stages change the shell's public API and doing that twice
is how a refactor stops being reviewable.

**Third, the loop inversion (G4).** The terminal front-end *owns* the clock: it
blocks in `event::poll`, decides when to advance, and decides when to draw. An
`eframe` application does not own its loop — the windowing system calls
`update()` and the application asks to be called again. Every piece of shared
state above the core is currently phrased as "the body of a `while` loop", and it
has to become "an object with `key`, `advance`, `frame` and `deadline`". That is
the one change that touches §7's state machine, §15's timing and §10's input
buffering at once, and the only one that can silently break determinism. It is
done **before any GUI code exists**, so that the terminal front-end — which has a
signed-off acceptance suite and a pty harness — is the thing that proves the
inversion was faithful.

**Fourth, the config file (G11).** All three builds read and write §6.2's
configuration, two of them the same file. §6.3's loader is a value-by-value
parser that rewrites the whole document on save. A GUI run that silently
discards the terminal front-end's `[display]` table — or vice versa — is a
data-loss bug that a casual test will not catch, because each front-end's own
settings survive perfectly.

The two vertical slices (G5 native, G6 web) are deliberately thin and
deliberately early, for the same reason PLAN.md's Stage 6 was: a window that
opens, shows a falling piece and responds to a hard drop retires the whole
eframe/winit/GPU stack as a question, and the same slice in a browser tab
retires the wasm toolchain, before nine stages of layout work are sunk into
either.

---

## Target layout

```
ftm/
├── Cargo.toml            # three [[bin]]-worth of features: tui (default), gui
├── Trunk.toml            # the web build (G6)
├── index.html            # the web build's shell page (G6)
├── FTM.md                # front-end-agnostic specification
├── FRONTEND.md           # the contract every front-end is written against
├── TUI.md                # the terminal front-end (§8, §12, §13)
├── GUI.md                # the egui front-end, native and web (§G1–)
├── MACROQUAD.md          # written if and when it is built; not now
├── PLAN.md               # the twelve stages that built v1.0
├── EGUI.md               # this plan
├── tests/
│   ├── scripted_game.rs      # I1, I2 + the §19.4 canary — untouched
│   ├── pump.rs               # NEW: the shell driven headlessly (G4)
│   ├── render_sizes.rs       # I4, TUI — behind `tui`
│   └── gui_render.rs         # NEW: the GUI's I4 (G12) — behind `gui`
├── tools/
│   └── drive.py              # the TUI pty harness — unchanged
└── src/
    ├── lib.rs
    ├── bin/
    │   ├── ftm.rs            # required-features = ["tui"]
    │   └── ftm-gui.rs        # required-features = ["gui"]
    ├── core/                 # UNCHANGED. Still pub(crate) behind its façade.
    ├── shell/                # front-end-agnostic AND platform-free.
    │   ├── mod.rs            #   builds for wasm32-unknown-unknown, no cfg.
    │   ├── time.rs           # NEW: the monotonic timestamp newtype (G3)
    │   ├── storage.rs        # NEW: the Storage trait (G3)
    │   ├── keys.rs           # NEW: the neutral Key / KeyEvent (G1)
    │   ├── config.rs         # §6, parsing and serialising only — no fs
    │   ├── input.rs          # §10.2, §10.3 — DAS/ARR, bindings
    │   ├── highscore.rs      # §14, over Storage
    │   ├── session.rs        # Session: config, scores, warnings, seed policy
    │   ├── round.rs          # App + the pumpable 60 Hz round (§15.2)
    │   ├── attract.rs        # the attract state machine (§13.1, §13.3, §13.6)
    │   ├── menus.rs          # PauseChoice, Setting, NameEntry, MenuChoice, Overlay
    │   ├── cosmetics.rs      # §12.5 animation timers, from events + a clock
    │   └── palette.rs        # §9.2 + the levelled lift, as plain RGB
    ├── tui/                  # #[cfg(feature = "tui")]
    │   ├── mod.rs            # Tui, fits, too_small, draw dispatch
    │   ├── keys.rs           # crossterm -> shell::keys adapter
    │   ├── host.rs           # the four capabilities, natively (G3)
    │   ├── term.rs           # §8.1–§8.3: raw mode, alt screen, panic hook
    │   ├── run.rs            # the poll loop; pumps shell::round / shell::attract
    │   ├── theme.rs          # §12.3 colour depth, Glyphs
    │   ├── cells.rs          # §12.2
    │   ├── playfield.rs      # §12.4
    │   ├── overlays.rs       # §12.6 drawing
    │   └── attract.rs        # §13 drawing + §13.4 cell-space drift
    └── gui/                  # #[cfg(feature = "gui")]
        ├── mod.rs
        ├── app.rs            # impl eframe::App; pumps the same shell objects
        ├── keys.rs           # egui -> shell::keys adapter
        ├── host_native.rs    # the four capabilities on a desktop (G5)
        ├── host_web.rs       # the four capabilities in a browser (G6)
        ├── layout.rs         # §G3: the integer-cell metric
        ├── paint.rs          # mino tiles, ghost, grid, boxes
        ├── playfield.rs      # §G4
        ├── overlays.rs       # §G5
        └── attract.rs        # §G7
```

`src/ui/` ceases to exist; `src/app.rs`, `src/config.rs`, `src/input.rs` and
`src/highscore.rs` move under `src/shell/`. `src/main.rs` becomes
`src/bin/ftm.rs` and keeps only what §8.1 puts before the loop.

### Cargo.toml shape

```toml
[features]
default = ["tui"]
tui = ["dep:ratatui", "dep:crossterm", "dep:directories", "dep:clap"]
gui = ["dep:eframe", "dep:egui"]

[[bin]]
name = "ftm"
required-features = ["tui"]

[[bin]]
name = "ftm-gui"
required-features = ["gui"]

# The web build is `ftm-gui` for wasm32-unknown-unknown; trunk drives it.
[target.'cfg(target_arch = "wasm32")'.dependencies]
# wasm-bindgen, web-sys (Storage, Location), console_error_panic_hook, web-time
```

`ratatui`, `crossterm`, `eframe`, `egui`, `directories` and `clap` all become
`optional = true`. `directories` and `clap` join the front-end features rather
than staying shared, because neither has any meaning in a browser tab — the
shell after G3 does not use them.

The shared, always-on dependencies shrink to `serde`, `toml`, `serde_json` and
`rand` (the last only inside `core/bag.rs`, as §9.6's bit source). `chrono`
becomes a front-end dependency, because after G3 the shell is handed a date
rather than reading one.

**Consequence to plan for:** a bare `cargo test` builds only the `tui` half.
Every command in the Makefile grows `--all-features`, and CI runs that. This is
called out again in G12 because it is the single easiest thing to forget and the
failure mode is silent — the GUI simply stops being compiled.

---

## Working agreements

The `PLAN.md` agreements all still hold. These are the ones this plan adds.

- **The core is not touched.** Not one line in `src/core/`. If a stage seems to
  need a core change, that is a design error in the stage — with exactly one
  candidate exception, sub-cell interpolation, which is named and deferred in
  [G9](#stage-g9--animations).
- **The terminal front-end's behaviour does not change.** G1–G4 are refactors.
  After each of them, §17.3's A3, A4, A6 and A7 are re-run through
  `tools/drive.py` and must give the same answers. A stage that changes what the
  TUI does is a stage that got the extraction wrong.
- **The batch-invariance canary (§19.4) never goes red and is never marked
  ignored.** It does not move, it does not gain a feature gate, and it runs on
  the default feature set.
- **`shell/` names no front-end crate and no platform facility.** Enforced by
  the compiler in two CI steps from G3 onward: `cargo check
  --no-default-features` and `cargo check --no-default-features --target
  wasm32-unknown-unknown`. The second is the stronger of the two and is the one
  that keeps the web and Macroquad builds cheap.
- **Every front-end preserves the others' config.** A `[display]` table written
  by the terminal front-end survives a GUI run untouched, and a `[gui]` table
  survives a terminal run. Tested, not asserted.
- **Spec first, in the same commit.** When the code and one of the front-end
  documents disagree, the document is wrong until amended — the existing rule,
  now with four documents to keep honest.

---

## Milestones

| Milestone | After stage | Means |
|---|---|---|
| **MG1 — Shell is platform-free** | G3 | `shell/` compiles for `wasm32-unknown-unknown` with no shims. The terminal binary is behaviourally identical. |
| **MG2 — Front-ends decoupled** | G4 | The shell is drivable headlessly, at any cadence. The terminal acceptance suite still passes. |
| **MG3 — First pixels** | G5 | A window opens, a piece falls, the keys work; the eframe stack is retired as a risk. |
| **MG4 — First pixels in a browser** | G6 | The same slice, served as wasm; the toolchain and the four web capabilities are retired as a risk. |
| **MG5 — GUI playable** | G8 | A complete game can be played, paused, lost and recorded, natively and on the web. |
| **MG6 — Parity** | G11 | Everything the terminal front-end does, the GUI does. |
| **MG7 — Accepted** | G12 | B1–B12 signed off; CI builds and tests all three targets. |

---

## Stage G0 — The documentation split

**Depends on:** nothing. **Touches:** documentation only. No code.

### The renumbering trap, and how this avoids it

The obvious split renumbers everything, and the cost is enormous and hidden:
`§12.4`, `§9.13`, `§8.2` and their siblings appear in several hundred doc
comments across `src/`, in `CLAUDE.md`, in `PLAN.md` and in commit messages. A
renumber invalidates every one of them silently — the reference still *reads*
fine, it just points somewhere else.

**Section numbers therefore do not change.** The sections that move to `TUI.md`
keep their numbers there. A code comment saying `§12.4` still resolves; it simply
resolves to a different file. `GUI.md` uses a fresh `§G` namespace so it can
never collide, and a future `MACROQUAD.md` would use `§M`.

Each document opens with a header saying which numbers it owns, and `FTM.md`
keeps a stub for each moved section reading, e.g., *"§12 — Rendering. Moved to
`TUI.md`. The front-end contract that was §12.7 and §12.8 stays here as §12.7 and
§12.8."*

### The division

| Sections | Document | Why |
|---|---|---|
| §1–§7, §9–§11, §14–§19 | `FTM.md` | Rules, config, states, controls, high scores, timing, errors, testing, network readiness. None of it mentions a terminal except in the wording, which is amended. |
| §12.7 (view model), §12.8 (event stream) | `FTM.md` | **The front-end contract.** These stay put precisely because they are what a front-end is written against. §12.1–§12.6 leave around them; the stubs say so. |
| — | `FRONTEND.md` | **New, and the document a fourth front-end reads first.** What a front-end must provide (the four capabilities of G3, a key event stream, a draw surface), what it may assume, and what it must never do — reach into `core`, own the tick rate, or read a clock the shell has not been told about. It is a short document and it is the one that makes "add a front-end" a bounded task. |
| §8 (terminal handling), §12.1–§12.6, §13 | `TUI.md` | Raw mode, the alternate screen, keyboard-enhancement flags, the 60 × 24 minimum, cell glyphs, colour depth, the 44 × 23 layout, the character-grid attract screen. |
| §6.3's `[display]` table | `TUI.md` | `cell_filled`, `cell_empty`, `cell_ghost`, `color_depth` are meaningless off a terminal. `show_grid` and `show_debug` are *shared* and stay in `FTM.md` — see G11. |
| §17.3's A1–A10 | `TUI.md` | They are terminal acceptance criteria and always were. B1–B12 are `GUI.md`'s. |
| §G1– | `GUI.md` | New. Written stage by stage, not up front. Covers both the native and the web build, with the differences called out in place rather than in a separate document — they are the same code. |

### Amendments to `FTM.md` in this stage

- **§1** — "runs in a text terminal" becomes "is playable through interchangeable
  front-ends", naming the three documents. §1.1's "smooth, flicker-free rendering
  in any ANSI-capable terminal" moves to `TUI.md`'s goals.
- **§1.2** — "Mouse input" stays a non-goal for every front-end, and says so
  explicitly: the game is keyboard-driven and §10.1's bindings are the whole
  input surface. This is worth being deliberate about — a windowed game, and
  especially one in a browser, attracts click-to-select and touch requests, and
  every one of them is a second input path through the menus. (Touch is
  separately noted in [Open decisions](#open-decisions), because a web build with
  no touch input is a web build that does not work on a phone.)
- **§3** — the dependency table splits into shared, `tui`-only, `gui`-only and
  `gui`-on-wasm. The "no other runtime dependencies" rule becomes per-front-end,
  and gains a sentence acknowledging that `eframe` brings a windowing and
  rendering stack (`winit`, `glow`) that is not enumerable crate by crate — the
  rule is about *direct* dependencies.
- **§3.1** — the layering rule is where the real change lands. It grows a third
  layer, **core → shell → front-end**, and its "no I/O and no clock" property is
  restated to cover the shell as well, in the four-capability form described
  above. This is the amendment the rest of the plan leans on; it is worth
  writing carefully.
- **§4** — the project layout is replaced with the one above.
- **§6.2** — the config *location* becomes a front-end question. The path rule is
  the native front-ends'; the web build's answer is `localStorage`, and §6.2 says
  so and defers to `GUI.md` §G8 for the key.
- **§7** — already two levels (`Next` / `Phase`) as of Stage 11. Add that the
  state machine is *pumped by* a front-end rather than being a loop, which is
  what G4 makes true.
- **§14** — the same treatment as §6.2: the table, its rules and its JSON are the
  shell's; where the bytes live is the front-end's.
- **§15.2** — generalise. The step list stays; step 5's "draw only when the frame
  changed" becomes a front-end-specific optimisation with a note that an
  immediate-mode front-end redraws unconditionally and needs no such comparison.
  Step 6/7's `event::poll` becomes "wait for the front-end's deadline".
- **§16** — the failure paths that are terminal-specific (restore the terminal
  before printing) move to `TUI.md`; the shared ones (unwritable config,
  unwritable high scores, non-TOML config, warnings surfaced after teardown)
  stay, with "to stderr" generalised to "by whatever means the front-end has" —
  which for the web build is the browser console.

### Done when

All four documents exist, every cross-reference resolves, no `§` reference in
`src/` has been invalidated, and `CLAUDE.md` names the documents and the
number-stability rule. No code has changed and `make check` is untouched.

---

## Stage G1 — Front-end-neutral keys

**Depends on:** G0. **Spec:** §10.1, §10.2, §10.3, §8.2, `FRONTEND.md`.

### The work

`src/shell/keys.rs`, new:

```rust
pub enum Key { Left, Right, Up, Down, Enter, Tab, Esc, Backspace, F(u8), Char(char) }
pub struct Mods { pub ctrl: bool, pub alt: bool, pub shift: bool, pub logo: bool }
pub enum KeyKind { Press, Repeat, Release }
pub struct KeyEvent { pub key: Key, pub mods: Mods, pub kind: KeyKind }
```

`Space` stays `Key::Char(' ')`, exactly as `parse_key` resolves it today — the
name grammar is unchanged, so no config file's `[keys]` table changes meaning.

`parse_key` and `is_key_name` move here verbatim, retargeted at `Key`.
`Bindings`, `InputState`, `Bindings::action_of`, `InputState::key` and
`InputState::binding` are retyped onto the neutral `KeyEvent`. The
`NOT_A_GAME_KEY` modifier test becomes a method on `Mods`. §16's Ctrl-C check is
unchanged in behaviour.

`src/tui/keys.rs`, new: `From<crossterm::event::KeyEvent> for shell::keys::KeyEvent`.
`KeyEventKind::{Press, Repeat, Release}` map one to one. Any crossterm `KeyCode`
with no neutral equivalent (`Insert`, `Home`, media keys) maps to `None` — the
adapter returns `Option<KeyEvent>` and the loop drops what it cannot express,
which is what the bindings table does with those keys today anyway.

`app.rs` and `ui/attract.rs` are retyped to take neutral events; the TUI loop
converts at the point it reads from crossterm, and nowhere else.

### Design note: keep the event stream synthesisable

`KeyEvent` is an *event*, and two of the three anticipated front-ends deliver
events natively. Macroquad does not: it exposes per-frame polled state
(`is_key_down`) alongside edge helpers, so its adapter will synthesise the
stream by diffing the key set between frames. Nothing in the shell may therefore
depend on receiving every intermediate event, on sub-frame ordering between two
keys, or on an event arriving at a moment other than a frame boundary. DAS/ARR
already satisfies this — it works off accumulated `Duration`, not off event
counts — and the rule is written into `FRONTEND.md` here so it stays true.

### Watch for

- `menu_action` in `app.rs` and the `Esc`/`Enter`/`Space` tests in `attract.rs`
  pattern-match `KeyCode` directly. They move to `Key` unchanged in meaning.
- `KeyEventKind::Repeat` is only meaningful in enhanced mode (§8.2). The neutral
  type keeps the three-way distinction because `egui` *also* reports repeats and
  must discard them for the same reason — see [G5](#stage-g5--gui-vertical-slice).

### Tests

- T13 (DAS/ARR) retyped. Its arithmetic must not move by a tick.
- New: every §10.1 key name round-trips `is_key_name` → `parse_key` → adapter.
- New: the crossterm adapter maps each of §10.1's names from the `KeyCode` a real
  terminal sends.

### Done when

`make check` clean; A4 re-measured on a pty (a 50 ms kitty tap moves exactly one
cell; a 0.6 s hold slides to the wall and stops).

---

## Stage G2 — The shell/front-end move

**Depends on:** G1. **Spec:** §3.1 as amended, §4.

### The work

A move, a rename and a feature gate. No logic changes.

1. `config.rs`, `input.rs`, `highscore.rs` → `src/shell/`.
2. Out of `ui/mod.rs` into `shell/cosmetics.rs`: `Cosmetics`, `Banner`,
   `Running`, `Trail`, `expire`, `trail_cells`, and the §12.5 duration
   constants. This module already takes only `&[GameEvent]` and an `Instant`, so
   it moves without an edit — which is the payoff for §12.8 having been kept
   honest.
3. Out of `ui/overlays.rs` and `ui/attract.rs` into `shell/menus.rs`: `Overlay`,
   `PauseChoice`, `Setting`, `NameEntry`, `MenuChoice`, `Sub`. These are menu
   *models*; their `draw` functions stay behind in `tui/`.
4. `ui/attract.rs`'s `Attract` splits. The state machine — `selected`, `sub`,
   `last_key`, `face`, `face_since`, `idle_shift`, `key`, `menu_key`,
   `options_key`, `Outcome` — goes to `shell/attract.rs`. **`Background` stays in
   `tui/`**: §13.4's drift is positioned in matrix cells of a character grid, and
   each GUI front-end wants its own. `Attract::step` loses the `cells` argument
   and returns whether the *state* changed; the drift's "did it move" answer is
   the front-end's to fold in.
5. `ui/theme.rs`'s `levelled`, the §9.2 table it reads, and the brightness
   percentages (`FULL`, `SLOT_NEAR`, `SLOT_FAR`, `GHOST`) move to
   `shell/palette.rs`, as plain `(u8, u8, u8)` and `u8`. `Theme`, `Glyphs`,
   `Depth`, `ansi16`, `ansi256`, `cube` and the `Style` construction stay in
   `tui/`. **This is a spec amendment**: §12.3's levelling note currently says
   the lift "stops at `theme.rs`". It becomes: the lift is shared presentation,
   because the luma problem is a property of §9.2's colours and not of terminals
   — blue at luma 17 is as hard to read on a monitor as it is in a terminal.
   `Colour::rgb` is still §9.2 exactly, and is still what a §19 client is handed.
6. The rest of `ui/` → `src/tui/`, gated on `feature = "tui"`. `main.rs` splits:
   §8.1–§8.3's setup, teardown and panic hook to `tui/term.rs`, the loop to
   `tui/run.rs`, and the argument-parsing entry point to `src/bin/ftm.rs`.
7. `Cargo.toml` gains the features and the `[[bin]]` sections. `ftm-gui.rs` is a
   stub that prints "not built yet" — it exists so the target and its feature
   gate are wired before there is anything to put in it.

### Tests

- Existing tests move with their modules. `tests/render_sizes.rs` gains
  `#![cfg(feature = "tui")]`.
- New CI step: `cargo check --no-default-features`. This is the compiler holding
  the shell boundary the way A10 made it hold the core's — with neither front-end
  feature on, `shell/` and `core/` must compile alone. It is worth more than any
  audit, for the same reason A10 was.

### Done when

`make check` (with `--all-features`) clean, `cargo check --no-default-features`
clean, A3/A6/A7 re-run on a pty with unchanged results.

---

## Stage G3 — A platform-free shell

**Depends on:** G2. **Spec:** §3.1 as amended, §6.2, §14, §16, `FRONTEND.md`.

This is [the central idea](#the-central-idea-the-shell-loses-its-clock-too) made
real. Four capabilities leave the shell.

### 1. Time

`src/shell/time.rs`, new. A monotonic timestamp as a newtype, not
`std::time::Instant`:

```rust
/// Monotonic time since the front-end started, in microseconds.
/// Microseconds, not milliseconds: §12.5's flash alternates at 12 Hz and
/// §10.3's ARR can be one tick, so millisecond resolution is visibly coarse.
/// u64 microseconds is ~584,000 years of range.
pub struct Stamp(u64);

impl Stamp {
    pub const ZERO: Stamp;
    pub fn from_micros(us: u64) -> Stamp;
    pub fn from_secs_f64(s: f64) -> Stamp;   // macroquad's get_time(), later
    pub fn saturating_since(self, earlier: Stamp) -> Duration;
    pub fn checked_add(self, d: Duration) -> Stamp;
}
```

Every `Instant` in `Cosmetics`, `Attract`, `Confirm`, `Fps` and `App` becomes a
`Stamp`. `Duration` stays — it is `core::time::Duration`, has no platform
dependency, and is what all the arithmetic is already in.

The front-end produces stamps: `tui/host.rs` and `gui/host_native.rs` from an
`Instant` captured at start-up, `gui/host_web.rs` from `performance.now()` (via
`web-time`, or `eframe`'s own frame time), a future macroquad host from
`get_time()`.

**Two properties `FRONTEND.md` states normatively**, because the shell's timing
is only as good as they are: a stamp is **monotonic** — never earlier than one
already handed over — and it is **wall-clock-paced**. Every subtraction in the
shell is already `saturating_`, so a violation degrades rather than panics, but
a front-end that violates it makes DAS wrong and the §12.5 animations stutter.

A pleasant side effect worth naming: `tests/pump.rs` in G4 can construct stamps
arithmetically, so the whole shell becomes testable without a clock — the same
property §17.1 gives the core, one layer out.

### 2. Storage

`src/shell/storage.rs`, new:

```rust
pub trait Storage {
    fn read(&self, slot: Slot) -> Result<Option<String>, StorageError>;
    fn write(&mut self, slot: Slot, contents: &str) -> Result<(), StorageError>;
}
pub enum Slot { Config, HighScores }
```

Two slots, both text, because §6.2 is TOML and §14 is JSON and neither is large.
A `Slot` rather than a path, because a browser has no paths.

`config.rs` and `highscore.rs` keep every rule — §6.3's value-by-value parsing,
the clamping, the warnings, §14's capacity, tie-breaking and the zero-score rule
— and lose `std::fs`, `directories` and `PathBuf`. §14's **atomic write** (write
a temp file, rename) is a filesystem technique and moves into the native
`Storage` implementation, where it belongs; the trait promises durability, not a
technique.

`tui/host.rs` implements it over `directories` + `std::fs`, preserving §6.2's
path, §14's atomic write, and §16's "an unwritable file is a warning, never an
abort". `gui/host_native.rs` uses the same implementation — the two native
binaries share a config file, which is the point of G11.

### 3. Entropy

`Session::next_seed` currently calls `rand::random()`. It becomes a
front-end-supplied `FnMut() -> u64`, exactly as `Startup::resolve` already takes
one. `core/bag.rs` is untouched — §9.6's PCG32 expansion and Lemire draw stay
where they are, `rand` remains the bit source, and the CLAUDE.md warning about
not "simplifying" them back to `rand`'s own API stands.

This is what keeps `getrandom`'s `wasm_js` backend and its build-time `--cfg`
out of the shared build entirely: the web host calls `Math.random()` twice and
composes a `u64`, and nothing else in the tree needs an entropy source.

### 4. Date

`highscore::today()` uses `chrono`. It becomes a front-end-supplied
`FnOnce() -> String` producing §14's date stamp. `chrono` moves to the
front-end features; the web host uses `js_sys::Date`, avoiding `chrono`'s
`wasmbind` feature and the `wasm-bindgen` version coupling it brings.

### The CI step that makes it stick

```
cargo check --no-default-features --target wasm32-unknown-unknown
```

Added here, and this is the stage's real deliverable. It is the compiler holding
a boundary that no amount of review holds reliably, and it is what makes G6 a
short stage instead of a long one. If it passes, the shell is portable; if
someone reaches for `Instant::now()` in `shell/` two years from now, it goes red
in the same commit.

### Tests

- Existing config and high-score tests retargeted at an in-memory `Storage`
  (which they will want anyway — several currently write to temp files).
- New: `Stamp` arithmetic, including that `saturating_since` on a
  non-monotonic pair yields zero rather than panicking.
- New: §16's failure paths through the trait — a `Storage` that always fails to
  write produces exactly the warnings §16 requires, and never an abort.

### Done when

`cargo check --no-default-features --target wasm32-unknown-unknown` is clean;
`make check` clean; A3/A6/A7 re-run on a pty with unchanged results, including
I3's "exactly one warning" for a config that is not TOML.

**MG1.**

---

## Stage G4 — Inverting the loop

**Depends on:** G3. **Spec:** §7, §15.2, §15.3, §8.4. **The load-bearing stage.**

### The problem

`app::round` is a `loop` whose body is seven numbered steps, and it owns the
clock, the event source and the terminal. `eframe` will call an `update(&mut
self, ctx, frame)` method and expects to be told when to call it again. The steps
must become methods on an object that any front-end can drive.

### The shape

`shell/round.rs`:

```rust
pub struct Round { /* App, accumulator, last, fps, settings generation */ }

impl Round {
    pub fn new(session: &Session) -> Self;

    /// Steps 1 and 3-5: advance the clock, resolve DAS/ARR, run whole ticks,
    /// feed the cosmetics. Returns Some(next) when the round is over.
    pub fn advance(&mut self, session: &mut Session, now: Stamp) -> Option<Next>;

    /// Step 2, one event at a time. The front-end drains its own queue.
    pub fn key(&mut self, session: &mut Session, event: &KeyEvent, now: Stamp) -> Option<Next>;

    /// §8.4: the viewport can or cannot host the screen. The *minimum* is the
    /// front-end's; the forced pause is the shell's.
    pub fn viewport(&mut self, fits: bool);

    /// Everything a front-end needs to draw, as one value (§15.2 step 5).
    pub fn frame(&self, now: Stamp) -> FrameState;

    /// How long the front-end may wait before calling `advance` again (step 7).
    pub fn deadline(&self, now: Stamp) -> Duration;
}
```

`shell/attract.rs` grows the same methods with a 10 fps deadline (§15.3) and no
accumulator.

`tui/run.rs` becomes: drain crossterm events into `key`, call `advance`, compare
`frame()` with the previous one, draw if it differs, `event::poll(deadline())`.
The seven steps are still there and still in order; they are simply on the other
side of a call.

### The five things that make this stage delicate

1. **`Session::glyphs` is a terminal thing.** `Glyphs::configured` leaks three
   `&'static str` so `Theme` can be `Copy`. That belongs to `tui/`, not to the
   shell. `Session` loses the field; `tui/run.rs` interns once at start-up and
   carries it in its own struct. `Chrome` stays a TUI type.

2. **`Chrome::hold_enabled` keeps its provenance.** §13.5's rule — a running game
   keeps the rules it started under — is `Round`'s to answer, not the config's.
   `Round` exposes `hold_enabled()` and every front-end asks it.

3. **`FrameState` is not the same thing as "redraw".** The five-component
   comparison (§15.2 step 5) exists because ratatui's diff is cheaper than
   redrawing but not free. In `egui` it is unnecessary — immediate mode rebuilds
   the frame every repaint. `frame()` therefore returns the *state*, and the
   decision to compare is `tui/run.rs`'s alone. `cramped: Option<Size>` becomes
   `cramped: bool` in the shared type, with the TUI keeping the size it prints in
   its own comparison. See [Hazards that do not carry
   over](#hazards-that-do-not-carry-over).

4. **`deadline` must not become a frame rate.** The terminal loop waits
   `TICK - accumulator`; the GUI will call `ctx.request_repaint_after(deadline)`
   and may *also* be repainted sooner by the window system, the compositor, or a
   browser tab regaining focus. `advance` therefore has to be correct when called
   at any cadence — 60 Hz, 144 Hz, or twice in a millisecond — which it already
   is, because `ticks_due` is an accumulator over real elapsed time. Nothing
   about `ticks_due` changes. This is the property the new test pins.

5. **Held input still survives a zero-tick pump.** `Pending` exists because a
   frame may legitimately run no ticks (§15.2 step 6). A GUI frame at 144 Hz runs
   no ticks *more than half the time*, so this stops being an edge case and
   becomes the common path. `App::advance`'s existing behaviour is right; the
   existing unit test `a_frame_that_runs_no_ticks_keeps_the_input_it_resolved`
   becomes considerably more load-bearing than it was.

### Tests

`tests/pump.rs`, new — the headless substitute for `tools/drive.py`, and the
thing that makes every future front-end testable without a window:

- **Cadence invariance.** Drive a `Round` with the same synthetic key log over
  the same span of stamps, at 60 Hz, at 144 Hz, and at a deliberately jittery
  cadence including several frames with no elapsed time at all. All must produce
  the identical `GameView`. This is the §19.4 canary's sibling and it is here for
  the same reason: the desync it catches is cheap now and expensive in G10.
- **A long-suspended front-end.** A single frame with ten seconds of elapsed
  stamps must run `MAX_CATCH_UP_TICKS` and discard the rest (§15.2 step 4) — the
  browser-tab-in-the-background case, which is far more common than a suspended
  laptop was.
- **Phase transitions.** The existing `app.rs` unit tests (`cramp` forcing a
  pause, the restart hold, name entry) rephrased against `Round`'s public
  methods, so they cover what a front-end can actually reach.
- **`deadline` is never zero and never longer than a tick while playing.**

### Done when

`make check` clean; A3, A4, A6 and A7 all re-run on a pty with unchanged results;
`tests/scripted_game.rs` untouched and green.

**MG2.**

---

## Stage G5 — GUI vertical slice

**Depends on:** G4. **Spec:** `GUI.md` §G1 (the front-end), §G2 (input).

The thin slice, for the same reason PLAN.md's Stage 6 was thin: prove the stack.

### The work

- `eframe` and `egui` as optional dependencies. **Version decision required** —
  see the [risk register](#risk-register): `eframe` 0.33 has an MSRV of 1.88 and
  matches the project's floor exactly; 0.36 requires 1.95. The recommendation is
  0.36 with the MSRV raised, because §3 already says the floor is set by a
  dependency and moves when one moves, and `ratatui` is no longer the binding
  constraint. Whichever is chosen, `Cargo.toml`'s `rust-version`, §3, and the
  CI `msrv` job change in the same commit.
- `src/bin/ftm-gui.rs`: parse arguments, build `gui::host_native::Host`, then
  `eframe::run_native`. No terminal, no raw mode, no §8.3 teardown — §16's
  warnings still reach stderr after the window closes.
- `gui/app.rs`: `struct Gui { session: Session, host: H, screen: Screen }` where
  `Screen` is `Attract(..)` or `Round(..)` — §7's `Next` loop, expressed as a
  field instead of a `match` in a `loop`.
- `gui/keys.rs`: `egui::Event::Key { key, pressed, repeat, modifiers }` →
  `shell::keys::KeyEvent`. Note `egui::Key::Space` → `Key::Char(' ')`;
  `egui::Event::Text` is **not** used for the name-entry field, because §12.6's
  rules (twelve printable ASCII, `ANON` when empty) belong to `shell::menus`.
- `update()`: drain `ctx.input(|i| i.events.clone())` into `key`, call `advance`,
  paint, then `ctx.request_repaint_after(round.deadline(now))`.
- Painting: the playfield rows and the current piece as `Rect`s through
  `egui::Painter`, coloured from `shell::palette`. Nothing else. No hold box, no
  next queue, no stats, no grid.

### The GUI is always "enhanced" (§8.2)

`egui` reports true press and release with a `repeat` flag, so §8.2's legacy path
does not exist here. `InputState` is constructed with `InputMode::Enhanced`
unconditionally, repeats are discarded exactly as the enhanced terminal path
discards them, and DAS is driven by the clock alone. `GUI.md` says so normatively,
and `HOLD_TIMEOUT` / `RESTART_QUIET` are never reached from the GUI.

### Done when

`cargo run --features gui --bin ftm-gui` opens a window; a piece falls at the
right speed; left, right, soft drop, hard drop and rotate all work; `Esc` quits
cleanly. Cadence invariance is visible as well as tested — the same seed on a
60 Hz and a 120 Hz display plays the same game.

**MG3.**

---

## Stage G6 — The web slice

**Depends on:** G5. **Spec:** `GUI.md` §G8 (the web build).

The same slice, in a browser tab. Short, because G3 did the hard part — but only
if G3 did it properly, which is exactly why this stage is here and not at the
end.

### The work

- `index.html`, `Trunk.toml`, and a `wasm32-unknown-unknown` target in CI.
  `trunk serve` for development, `trunk build --release` for the artefact.
- `src/bin/ftm-gui.rs` grows a `#[cfg(target_arch = "wasm32")]` entry point using
  `eframe::WebRunner` against a canvas, alongside the native `run_native`.
  `gui/app.rs` is shared verbatim.
- `gui/host_web.rs`, the four capabilities:
  - **Time** — `web-time`'s `Instant` (`performance.now()`), or `eframe`'s own
    frame timestamp, converted to `Stamp`.
  - **Storage** — `web_sys::Storage` (`localStorage`), one key per `Slot`.
    §16's rules apply unchanged: a quota error or a browser with storage disabled
    is a warning, never an abort, and the game is fully playable without it.
  - **Entropy** — `Math.random()` composed into a `u64`.
  - **Date** — `js_sys::Date`.
- `console_error_panic_hook`, so a panic is legible in the console rather than
  an `unreachable` trap. This is the web analogue of §8.1 step 2's panic hook,
  and `GUI.md` says so.

### The four web-specific things that will bite

1. **The browser eats the game's keys.** §10.1 binds `Space` to hard drop and the
   arrows to movement; in a browser those scroll the page, and `Tab` moves focus.
   `eframe` calls `preventDefault` for keys it consumes, but only while the
   canvas has focus — so the canvas must take focus on load and visibly indicate
   when it has not. `GUI.md` §G8 states the required behaviour; it is the single
   most likely "it works locally and not on the web" defect.
2. **A backgrounded tab stops being called.** Browsers throttle
   `requestAnimationFrame` to ~1 Hz or stop it entirely. That is the ten-seconds-
   of-arrears case, and §15.2 step 4's catch-up cap already handles it correctly —
   the game does not resume into an instant death. The G4 test covers it, and
   this stage confirms it in a real tab.
3. **`localStorage` is per-origin and is not the native file.** A player's web
   scores and their desktop scores are separate tables, by construction. §14 says
   so after G0's amendment; it is not a bug to be fixed.
4. **There is no argv.** §6.4's CLI becomes URL query parameters for the web
   build — `?seed=42&preview=3`. G11 does the general case; this stage needs only
   `seed`, and only to make the slice reproducible.

### Done when

`trunk serve` gives a playable falling piece in a browser; the same seed produces
the same game as the native binary; a page reload preserves whatever the slice
has written to `localStorage`; and a backgrounded tab, returned to, does not kill
the player.

**MG4.**

---

## Stage G7 — The playing screen

**Depends on:** G6. **Spec:** `GUI.md` §G3 (layout), §G4 (the playing screen).

### The layout metric

The one decision that makes GUI layout code tractable is to derive everything
from a single integer:

```
cell = floor(min(available_width / LAYOUT_COLS, available_height / LAYOUT_ROWS))
```

`LAYOUT_COLS` and `LAYOUT_ROWS` are the playing screen's extent in cells,
including the panels. Every rect in the screen is then an integer multiple of
`cell` offset from an integer origin, which is what keeps the mino grid crisp at
any window size and any DPI without fighting `egui`'s float coordinates. Panels
are laid out in the same unit, so a resize scales the whole screen rather than
reflowing it — and the web build gets responsive sizing for free, which matters
because a browser window is whatever the visitor's window happens to be.

`GUI.md` §G3 specifies the metric, `LAYOUT_COLS`/`LAYOUT_ROWS`, the minimum
`cell` (below which the §G3 too-small state applies) and the arrangement.

### The work

- Playfield: locked cells from `view.rows`, `current`, `ghost` (respecting
  `ghost_piece`), the §12.4-equivalent grid when `show_grid` is on.
- Hold box, honouring `Round::hold_enabled()` — absent, not empty, when the
  mechanic is off. This is the `Chrome::hold_enabled` invariant, restated for a
  front-end that cannot ask the config either.
- Next queue, `preview_count` 1..=6, each slot dimmer than the last, using the
  brightness percentages G2 moved into `shell/palette.rs`.
- Stats: score (grouped in threes), level, lines, time, combo, back-to-back.
- Status line: `Cosmetics::clear_name()`.
- The debug strip, when `show_debug` is on, from `Round`'s `Debug` — which is why
  `Debug` and `DebugView` were kept as view types.
- §G3's too-small state: below the minimum `cell`, the screen is replaced by a
  message and `Round::viewport(false)` forces the §8.4 pause. The rule is shared;
  only the threshold is the GUI's.
- **Focus loss pauses the game.** A GUI-only rule with no terminal analogue, and
  a real one: a window — or a browser tab — that loses focus is one whose keys
  stop arriving, which in a game with lock delay means a piece locks where the
  player did not put it. `GUI.md` §G4 makes it normative, and it uses the same
  `Round` path §8.4 uses, including releasing held keys.

### Done when

A full game is playable and legible, natively and on the web, at several window
sizes, including with `preview_count` at 1 and at 6.

---

## Stage G8 — Overlays and the pause path

**Depends on:** G7. **Spec:** `GUI.md` §G5.

Pause menu, resume countdown, game over, name entry, Options, Controls — all
drawn natively, all driven by the `shell/menus.rs` models that G2 extracted, so
the front-ends cannot disagree about what the menus *do*.

Name entry uses `shell::menus::NameEntry` fed neutral keys, not an
`egui::TextEdit`. §12.6's twelve-character ASCII limit and the `ANON` default are
rules about the high-score table (§14), not about a text field. This also keeps
the web build honest: a mobile browser's soft keyboard is a separate question,
noted in [Open decisions](#open-decisions), and it must not be answered by
accident here.

The Options panel needs `Setting::ALL` split — see G11. Only one of its eight
items is terminal-only (`Colour`, §12.3's depth); the other seven — preview,
start level, ghost, hold, 180 rotation, lock down and grid — are shared. So the
split is small, but it is not empty, and the GUI wants `[gui]` items of its own
in `Colour`'s place. Until G11 lands the split, the GUI's Options panel shows the
seven shared items and nothing else.

### Done when

A game can be played to a top out, the score entered, and the entry read back
from the file by the *terminal* binary. That cross-binary check is the point: it
is the first proof that the two native front-ends share §14's table correctly.

**MG5.**

---

## Stage G9 — Animations

**Depends on:** G8. **Spec:** `GUI.md` §G6.

`Cosmetics` already produces everything, driven by events and a stamp, and G2
moved it into the shell unchanged. The GUI renders the same seven animations
natively: the line-clear flash, the hard-drop trail, the lock flash, the level-up
banner, the perfect-clear colour cycle, the game-over wipe and the status line.

Pixels allow what characters did not — alpha fades rather than a two-state
alternation, a trail that fades along its length, a wipe that is a gradient. Each
is a rendering choice inside the same timings; none of them changes `Cosmetics`.

### The one core change that is tempting, and is deferred

Smooth sub-cell gravity — a piece that slides between rows rather than stepping —
is the obvious thing a GUI wants and the TUI cannot have, and it is the single
strongest argument for the Macroquad front-end. It is **not** in this plan, for a
specific reason: `GameView` carries integer cell positions only, and the
fractional part lives in the core's 16.16 fall-period accumulator (§9.9), which
is deliberately not exposed. Adding it means:

- a new field on `GameView`, which is a §12.7 amendment;
- and therefore a §19 decision, because `GameView` is what a server sends a
  player — a field that changes sixty times a second is a field on the wire.

The alternative — interpolating in the shell by watching for row changes — is
worse: it guesses at the core's state, and it will visibly desync at high levels
and during soft drop, which is exactly where it would be most noticed.

If it is wanted, it is its own decision with its own stage, and it should be
taken *before* Macroquad rather than during it — it is a `GameView` question, not
a rendering question, and answering it under the pressure of a new front-end is
how it gets answered badly. It is recorded in [Open
decisions](#open-decisions).

---

## Stage G10 — The attract screen

**Depends on:** G9. **Spec:** `GUI.md` §G7.

The wordmark (§13.2 — original block letters; §1.3's trademark constraint applies
in full and is restated in `GUI.md`, and applies to the window title, the web
page title and any favicon or icon the GUI acquires), the five-item menu, the
six-second cycling panel, a GUI-native drifting background, the sixty-second idle
colour cycle, and the three sub-screens (high scores, controls, options).

The state machine is `shell/attract.rs` from G2 and is shared; only the drawing
and the drift are new. §13.4's two exclusions — `mono` and `show_debug` — become
one in the GUI, since `mono` has no meaning off a terminal.

### Done when

`ftm-gui` opens on the attract screen natively and on the web, PLAY starts a
game, a recorded score appears on the panel and on the sub-screen, and QUIT
closes the window (and is hidden, or reloads, in the browser — a tab cannot close
itself, and `GUI.md` §G8 says which).

---

## Stage G11 — Config, CLI and persistence

**Depends on:** G10. **Spec:** §6 as amended, §14, `GUI.md` §G8.

### The config file is shared, and that is the hazard

The two native binaries read and write one `config.toml` at §6.2's path. §6.3's
loader parses value by value and `config::document` rewrites the whole document
on save. Left alone, a GUI run that saves the config **erases the `[display]`
table**, because the GUI's `ConfigFile` has no such field to write back.

The fix, and the work:

1. `[gameplay]`, `[timing]` and `[keys]` are shared and stay in `FTM.md` §6.3.
2. `[display]` splits: `show_grid` and `show_debug` are shared; `color_depth`,
   `cell_filled`, `cell_empty`, `cell_ghost` are terminal-only and move to
   `TUI.md`.
3. A new `[gui]` table in `GUI.md` §G8: window size, remembered position, scale,
   fullscreen, vsync/frame cap. The web build honours the subset that means
   anything in a canvas and ignores the rest.
4. **`ConfigFile` keeps every table, whichever binary is running.** The
   front-end-specific tables are parsed, validated and written back verbatim by
   every binary; only the front-end that owns a table *acts* on it. This is a
   deliberate design choice over a cleverer one (preserving unknown keys
   generically), because §6.3's loader is a value-by-value parser precisely so
   that it can warn about what it found, and a table it does not understand is a
   table it cannot warn about.
5. `Setting::ALL` splits into shared settings and per-front-end settings, so each
   Options panel offers what it can actually apply.

### CLI, and its web equivalent

`Cli` splits the same way. Shared: `--config`, `--print-config`, `--seed`,
`--preview`, `--no-hold`, `--no-rot180`. Terminal-only: `--color`, `--legacy`.
GUI-only: whatever `[gui]` grows an override for.

The web build has no argv, so §6.4's flags become URL query parameters with the
same names and the same precedence (§6.1: command line over file over default).
`?seed=42&preview=3` is the whole mechanism. `--print-config` has no web
equivalent and is simply absent there; `GUI.md` §G8 lists which flags exist in
which build, and that list is a test.

### Tests

- **Round-trip preservation, every direction.** A config with a `[display]`
  table, saved by the GUI, is byte-identical in that table. A config with a
  `[gui]` table, saved by the TUI, likewise. This is the test that catches the
  data-loss bug described above, and it is the reason this is a stage rather than
  a footnote to G5.
- **Query parameters obey §6.1's precedence**, with the same clamping and the
  same warnings as the flags they mirror.
- A seeded run is never recorded (§14, §6.4) — the same rule, the same
  `Session::finish` path, now reached from three builds.

### Done when

All three builds share the shell's config and score handling, and neither native
binary loses the other's settings.

**MG6.**

---

## Stage G12 — Testing, CI and acceptance

**Depends on:** G11. **Spec:** §17 as amended, `GUI.md` §G9.

### Testing

- `egui_kittest` (pinned to the chosen `egui` minor) drives the GUI headlessly:
  the AccessKit-based harness constructs an `egui::Context`, feeds `RawInput` and
  runs `update` with no window. That gives the GUI its **I4 analogue** —
  `tests/gui_render.rs`: every screen the program can show, at several window
  sizes including one below the §G3 minimum, renders without panicking.
- **Image snapshots are available and are deliberately not put in CI.**
  `egui_kittest`'s snapshot rendering needs a GPU or a software rasteriser, and a
  CI job that flakes on driver differences is a CI job that gets marked ignored.
  They are a local tool; the headless render test is the CI one.
- `tools/drive.py` gets no GUI counterpart. `tests/pump.rs` from G4 and
  `tests/gui_render.rs` from here are the substitute, and `GUI.md` says so —
  along with the honest caveat `drive.py` already carries: it substitutes for,
  but does not replace, playing the game.

### CI and the Makefile

```make
fmt:     cargo fmt --check
clippy:  cargo clippy --all-features --all-targets -- -D warnings
test:    cargo test --all-features
shell:   cargo check --no-default-features                                  # G2
wasm:    cargo check --no-default-features --target wasm32-unknown-unknown  # G3
web:     trunk build --release                                              # G6
build:   cargo build --release --features tui,gui
```

The two `check` lines are the load-bearing ones: they are the compiler holding
the shell's two boundaries, and they cost seconds.

The MSRV job's toolchain moves if G5 chose `eframe` 0.36. It must be changed in
`Cargo.toml`, `.github/workflows/ci.yml` and §3 together, exactly as the existing
CI comment demands.

`eframe` needs system libraries on a Linux runner (X11/Wayland development
headers for `winit`, and a GL stack for `glow`). The CI job grows an `apt-get`
step, and the web job needs `trunk` plus the `wasm32-unknown-unknown` target.
This is ordinary for egui projects, but it is a new class of CI failure for this
repository, which currently has none.

### Acceptance: B1–B12

`GUI.md` §G9, mirroring §17.3's A1–A10 and checked one by one the way those were.
Every criterion is checked in **both** builds unless marked native or web.

| | Criterion |
|---|---|
| B1 | Build clean: clippy silent on `--all-features --all-targets`; both binaries build in release; `trunk build --release` succeeds. |
| B2 | §17.1/§17.2 pass on `--all-features`, including the §19.4 canary and `tests/pump.rs`. |
| B3 | Opens on the attract screen; PLAY starts a game. |
| B4 | §10.1's bindings work, with DAS/ARR measurably identical to the terminal front-end's. |
| B5 | `preview_count` 1–6, from the file and from `--preview` / `?preview=`. |
| B6 | A full game recorded. Native: readable by the *other* binary. Web: survives a page reload. |
| B7 | Clean exit; §16's warnings surface (stderr natively, console on the web); a panic is legible and exits non-zero natively. |
| B8 | Cadence invariance: the same seed on displays of different refresh rates, and in a tab that has been backgrounded and restored, plays the same game. |
| B9 | Hold and 180 off/on, from the file, from the flags/query, and from the Options panel. |
| B10 | Focus loss pauses a game in progress and releases held keys (§8.4's path). |
| B11 | *Web*: the canvas takes keyboard focus on load; `Space`, the arrows and `Tab` reach the game and do not scroll or move focus. |
| B12 | The front-ends build against `shell` and `core::GameView` alone. `cargo check --no-default-features` and its wasm counterpart both pass, and no module in `gui/` names a `core` internal. |

**MG7.**

---

## Macroquad readiness

This section is to Macroquad what §19 is to networking: **nothing here is
implemented**, and it exists so that a few decisions taken in G1–G4 are not
quietly undone before the front-end that needs them is written.

Macroquad is attractive for a specific reason — it is a game framework rather
than a widget toolkit, so per-frame animation, easing, particles and sub-cell
motion are its native idiom rather than something worked around. If the §12.5
animations are ever to become the *point* rather than the decoration, that is
where it happens.

### What it will need, and where this plan already provides it

| Macroquad needs | Provided by |
|---|---|
| To own its own loop (`loop { … next_frame().await }`) | G4's pumpable `Round` — which is if anything a *closer* fit than `eframe`'s callback, because macroquad's loop is shaped like the terminal one. |
| Time as `f64` seconds from `get_time()` | G3's `Stamp::from_secs_f64`. The conversion to integer microseconds happens at the boundary, once, and never in the rules. |
| No filesystem on its web target | G3's `Storage` trait. Macroquad has its own storage and asset APIs; a third implementation is a small file. |
| Menus drawn by hand | G2's `shell/menus.rs`. The menu models are data with no widget dependency, which is what makes a framework with no widgets viable at all. |
| The §9.2 palette | G2's `shell/palette.rs`, as plain RGB. |

### The constraints to honour now

1. **Do not let the shell require an event stream it cannot get.** Macroquad's
   input is polled per frame. Its adapter will synthesise `KeyEvent`s by diffing
   the key set between frames, which means the shell must never depend on
   sub-frame ordering, on event counts, or on an event arriving between two
   frames. G1 writes this into `FRONTEND.md`; it is the constraint most easily
   broken by an innocent-looking change.
2. **Do not let `deadline` become a contract.** Macroquad renders every frame at
   vsync and will ignore it. `deadline` is advice a front-end may take; `advance`
   must be correct without it. G4's cadence-invariance test is what keeps this
   true.
3. **Keep `shell/` compiling for `wasm32-unknown-unknown`.** Macroquad's web
   target is a different wasm environment from `eframe`'s (it does not use
   `wasm-bindgen` in the same way), so the value of the shell needing *nothing*
   from the platform is higher here than anywhere else.
4. **Do not build a front-end abstraction trait.** There is no `trait Frontend`
   in this plan, and there should not be one. The front-ends share the shell by
   *calling* it, not by implementing a common interface — the same reasoning
   §19.2 uses for not building a transport before there is a second peer. Three
   front-ends that each call `Round` are simpler than three that each satisfy an
   interface designed before the third existed.
5. **Settle sub-cell interpolation before starting, not during.** See
   [G9](#stage-g9--animations). It is a `GameView` question with §19
   consequences, and Macroquad is precisely the front-end that will want it on
   day one.

`MACROQUAD.md` is written when the work starts, not before.

---

## Invariants the front-ends must not break

Every invariant in `CLAUDE.md` still applies. These are the ones a GUI or web
front-end is *specifically* liable to break, and each has a stage that guards it.

- **The core takes no clock and no I/O, and advances in fixed 1/60 s ticks** —
  and after G3, **so does the shell**. A windowed game invites "just advance by
  `ctx.input().stable_dt`". It must not. `ticks_due` and the accumulator are the
  only path, and G4's cadence-invariance test is what proves it.
- **Determinism is per-`RulesConfig`-plus-seed, not per-front-end.** The same
  seed must produce the same game in `ftm`, in `ftm-gui`, and in a browser tab.
  Nothing in a front-end may feed the core anything another would not.
- **The renderer draws only from `GameView`, and cosmetics only from
  `GameEvent` plus a stamp.** No front-end module may name a `core` internal, and
  B12 is the compiler's answer to that, as A10 was.
- **`RulesConfig` and `PresentationConfig` stay separate**, and the new `[gui]`
  table is presentation. A game keeps the rules it started under (§13.5), in
  every front-end.
- **Disabled keys are dropped at the input boundary.** The neutral `Bindings`
  table from G1 is that boundary for every front-end; a disabled key must not
  reset a lock-delay timer in the GUI either.
- **Input the shell has resolved is held until a tick consumes it.** Far more
  load-bearing in the GUI than in the TUI, because a high-refresh window runs
  zero ticks on most frames.
- **A four-line clear is a `QUAD`**, and §1.3's trademark rules extend to the
  window title, the page title, the wordmark and any icon or favicon.
- **A score of 0 never qualifies, ties keep the older entry, a seeded run is
  never recorded.** One implementation, in `shell/`, reached from three builds.

---

## Hazards that do not carry over

Naming these matters as much as naming the invariants, because the temptation is
to port the terminal front-end's structure wholesale and inherit its constraints
for no benefit.

- **`Frame`'s five-component comparison (§15.2 step 5).** It exists because
  ratatui diffs against a previous buffer. Immediate mode rebuilds every frame,
  so the GUI has no such comparison — and therefore none of the "anything the
  screen comes to show must join the struct or bump the generation" hazard that
  has already caught two things. Do not port it.
- **`Session::generation`.** Same reason. It stays for the TUI; the GUI ignores
  it.
- **`Theme` being `Copy`, and the leaked `Glyphs`.** A concession to threading a
  theme through every ratatui widget. The GUI has no cell glyphs and no reason to
  leak anything — and leaking in a browser tab that is reloaded repeatedly is a
  worse idea than leaking in a process that exits.
- **§12.3's colour depths and `NO_COLOR`.** Meaningless off a terminal. `mono`
  and the `16`/`256` fallbacks do not exist in the GUI; the `[display]` colour
  keys are ignored, and `GUI.md` says so.
- **§8.2's legacy key path, `HOLD_TIMEOUT` and `RESTART_QUIET`.** `egui` always
  reports releases. The GUI is unconditionally enhanced-mode.
- **§8.3's teardown and the panic hook.** There is no terminal to restore. The
  native GUI's error path is a window that fails to open, reported to stderr with
  a non-zero exit; the web build's is `console_error_panic_hook`.
- **§12.1's 60 × 24 minimum.** Each GUI build has its own minimum, expressed as
  a minimum `cell` (§G3). §8.4's *rule* — a game in progress is forced into
  `Paused` before the screen goes — is shared and has one implementation.
- **§14's atomic write.** A filesystem technique, not a durability requirement.
  It lives in the native `Storage` implementation; `localStorage` has no rename.

---

## Test coverage map

| Test | Stage | Notes |
|---|---|---|
| T1–T17 (§17.1) | — | Untouched. The core does not change. |
| T13 (DAS/ARR) | G1 | Retyped onto the neutral `KeyEvent`; arithmetic unchanged. |
| I1–I3, §19.4 canary | — | Untouched, on the default feature set. |
| I4 (`render_sizes.rs`) | G2 | Gated on `feature = "tui"`. |
| §12.4 mock-up test | G2 | Moves to `tui/playfield.rs`. Still the TUI's acceptance criterion. |
| Key-name round trip | G1 | New. Every §10.1 name, through both adapters. |
| `cargo check --no-default-features` | G2 | New. The front-end boundary, held by the compiler. |
| `Stamp` arithmetic, `Storage` failure paths | G3 | New. §16's rules through the trait. |
| `--target wasm32-unknown-unknown` | G3 | New. **The platform boundary, held by the compiler.** The highest-value step in this plan per second of CI time. |
| `tests/pump.rs` | G4 | New. Cadence invariance, catch-up cap, phase transitions, deadlines. |
| Config round-trip preservation | G11 | New. Every direction. The data-loss guard. |
| Query-parameter precedence | G11 | New. §6.1, on the web build. |
| `tests/gui_render.rs` | G12 | New. The GUI's I4, via `egui_kittest`, headless. |
| B1–B12 | G12 | New. `GUI.md` §G9, checked one by one. |

---

## Risk register

| Risk | Where it bites | Mitigation |
|---|---|---|
| **MSRV conflict.** `eframe` 0.36 requires Rust 1.95; 0.33 requires 1.88, the current floor. | G5, and the CI `msrv` job. | Decide in G5, in one commit across `Cargo.toml`, §3 and the workflow. Recommendation: take 0.36 and raise the floor — §3 already states the floor moves with a dependency, and pinning to 0.33 to preserve a number means tracking a stale egui for the life of the project. |
| **The G3 abstractions are done half-way**, leaving `cfg(target_arch)` sprinkled through the shell. | G3, discovered in G6 as a slow, miserable stage. | The wasm CI check lands *in* G3 and is what defines the stage as finished. A `cfg` in `shell/` is a stage that is not done. |
| **The loop inversion changes TUI behaviour subtly.** A reordered step, a lost `dt`, a pause that no longer zeroes the accumulator. | G4, discovered in G10. | G4 lands with no GUI at all, and is validated by the terminal front-end's existing pty acceptance suite plus cadence invariance. |
| **Tick/frame coupling.** The easy GUI bug: advancing by frame time, or once per repaint. | G5 onward. | `ticks_due` is the only path; the invariance test runs at several cadences; B8 checks it on real hardware and in a real tab. |
| **The browser swallows the game's keys.** `Space` scrolls, `Tab` moves focus, the canvas never had focus. | G6, and every web build after. | `GUI.md` §G8 makes canvas focus normative; B11 is a dedicated acceptance criterion, because this defect is invisible in every native test. |
| **Config data loss between binaries.** | G11, in the field. | Round-trip preservation tests in every direction, and `ConfigFile` keeping every table whichever binary is running. |
| **CI grows new classes of failure.** `eframe` needs X11/Wayland headers and a GL stack; the web job needs `trunk` and a wasm target. | G12. | An `apt-get` step and a pinned `trunk`; the headless render test uses no GPU; image snapshots stay out of CI deliberately. |
| **Feature-gate rot.** A bare `cargo test` stops compiling the GUI, and nobody notices for weeks. | Any stage after G2. | Every Makefile target takes `--all-features`, and the Makefile is the single source of truth CI runs. |
| **`egui` minor-version churn.** egui breaks API across minors more freely than ratatui does, and `eframe`, `egui_kittest` and `web-sys` must move together. | Maintenance. | Pin the minor in `Cargo.toml` and record it in §3, as the existing table does for every other dependency. |
| **Scope creep into §18.** A window — and especially a web page — makes sound, themes, touch and mouse input feel newly reachable. | Everywhere. | §1.2 and §18 are unchanged: still not work items. Parity is the deliverable. Touch is the one that deserves a real answer rather than a reflex; see below. |

---

## Open decisions

Recorded here rather than settled, because each is a judgement the plan should
not make on its own.

- **`eframe` 0.33 versus 0.36**, and with it the MSRV. See the risk register for
  the recommendation. Must be settled at G5.
- **Touch input for the web build.** §1.2 makes mouse input a non-goal, and this
  plan keeps that. But a web build is a link someone opens on a phone, and with
  no touch input it is a game that visibly does not work there. The options are
  to accept that and say so on the page, or to add an on-screen control layer as
  a deliberate, specified web-only feature — which is a §1.2 amendment and a real
  piece of design, not a small addition. **Worth deciding before G6**, because it
  changes what "the web build is done" means.
- **Sub-cell interpolation of gravity.** Deferred with reasons in
  [G9](#stage-g9--animations). It is a §12.7 change and therefore a §19 decision.
  It should be settled before any Macroquad work begins, since that front-end
  will want it immediately.
- **Whether `ftm-gui` should remember its window geometry.** §G8's `[gui]` table
  has a place for it, but a game that reopens where it was last is also a game
  that can reopen off-screen. Suggest: remember size, not position.
- **What the GUI does with `show_debug`.** The strip is §12.4's, in characters.
  The figures are front-end-agnostic (`Debug` + `DebugView`); the layout is not.
  Suggest a plain overlay panel, but it is not specified until G7.
- **Where the web build is served, and whether high scores stay local.**
  `localStorage` makes every visitor's table private to their browser, which is
  the honest default and needs no server. A shared leaderboard is a §19 question
  wearing different clothes, and is not in this plan.
