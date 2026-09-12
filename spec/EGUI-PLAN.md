# Falling Tetromino Manager — Front-End Plan

**Companion to:** [FTM.md](FTM.md) (the specification), [TERMINAL-PLAN.md](TERMINAL-PLAN.md)
(the twelve stages that built v1.0)
**Date:** 2026-09-06
**Status:** G0–G13 complete (**MG7**). The plan is finished; `GUI.md` §G9.3 is the sign-off.

This plan adds a second and a third front-end to FTM — a native windowed GUI on
`egui` / `eframe`, and the same GUI built for the browser as WebAssembly — and
restructures the tree so that a *fourth* front-end is an additive change rather
than another restructure. A Macroquad front-end is anticipated and is named
throughout as the thing the boundary must not foreclose; nothing in this plan
implements it.

It is written for the tree as it stands at the end of `TERMINAL-PLAN.md` Stage 12: the
core is sealed behind its façade (A10), the shell is `app.rs`, `config.rs`,
`input.rs`, `highscore.rs` and `ui/`, and `main.rs` is the terminal entry point.

Section references written `§n` are to `FTM.md` and, after Stage G0, to whichever
of the front-end documents keeps that number — see [The documentation
split](#stage-g0--the-documentation-split). GUI sections are written `§Gn` and
live in `GUI.md`. Stages are `G0`–`G13`; the existing stages 0–12 are not
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
- [Stage G10 — Sub-cell gravity](#stage-g10--sub-cell-gravity)
- [Stage G11 — The attract screen](#stage-g11--the-attract-screen)
- [Stage G12 — Config, CLI and persistence](#stage-g12--config-cli-and-persistence)
- [Stage G13 — Testing, CI and acceptance](#stage-g13--testing-ci-and-acceptance)
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

**Fourth, the config file (G12).** All three builds read and write §6.2's
configuration, two of them the same file. §6.3's loader is a value-by-value
parser that rewrites the whole document on save. A GUI run that silently
discards the terminal front-end's `[display]` table — or vice versa — is a
data-loss bug that a casual test will not catch, because each front-end's own
settings survive perfectly.

The two vertical slices (G5 native, G6 web) are deliberately thin and
deliberately early, for the same reason TERMINAL-PLAN.md's Stage 6 was: a window that
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
├── spec/                 # every document, and nothing else
│   ├── FTM.md            # front-end-agnostic specification
│   ├── FRONTEND.md       # the contract every front-end is written against
│   ├── TUI.md            # the terminal front-end (§8, §12, §13)
│   ├── GUI.md            # the egui front-end, native and web (§G1–)
│   ├── MACROQUAD.md      # written if and when it is built; not now
│   ├── TERMINAL-PLAN.md       # the twelve stages that built v1.0
│   └── EGUI-PLAN.md           # this plan
├── tests/
│   ├── scripted_game.rs      # I1, I2 + the §19.4 canary — untouched
│   ├── pump.rs               # NEW: the shell driven headlessly (G4)
│   ├── render_sizes.rs       # I4, TUI — behind `tui`
│   └── gui_render.rs         # NEW: the GUI's I4 (G13) — behind `gui`
├── tools/
│   └── drive.py              # the TUI pty harness — unchanged
└── src/
    ├── lib.rs
    ├── native.rs             # the desktop: F1-F4 for *both* native binaries (G5)
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
    │   ├── figures.rs        # how a score and a clock are written (G7)
    │   └── palette.rs        # §9.2 + the levelled lift, as plain RGB
    ├── tui/                  # #[cfg(feature = "tui")]
    │   ├── mod.rs            # Tui, fits, too_small, draw dispatch
    │   ├── keys.rs           # crossterm -> shell::keys adapter
    │   ├── cli.rs            # §6.4's grammar, as clap sees it
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
        ├── cli.rs            # §6.4's grammar for this front-end (G5, G12)
        ├── host_native.rs    # the desktop's four, over native.rs (G5)
        ├── host_web.rs       # the four capabilities in a browser (G6)
        ├── query.rs          # §6.4's flags as a URL query string (G6, G12)
        ├── layout.rs         # §G3: the integer-cell metric
        ├── paint.rs          # mino tiles, ghost, grid, boxes
        ├── playfield.rs      # §G4
        ├── overlays.rs       # §G5
        └── attract.rs        # §G7
```

`src/ui/` ceases to exist; `src/app.rs`, `src/config.rs`, `src/input.rs` and
`src/highscore.rs` move under `src/shell/`. `src/main.rs` becomes
`src/bin/ftm.rs` and keeps only what §8.1 puts before the loop.

**`src/native.rs` is an amendment this plan did not originally have.** It was
`tui/host.rs` from G3, and G5 moved it out rather than writing a second copy
under `gui/`: §6.2 and §14 give `ftm` and `ftm-gui` one config file and one
high-score table between them, so the paths, the clock, the seed, the date and
§14's atomic write are a *desktop's* answers and not a terminal's. It is not a
fourth layer — it is behind `any(feature = "tui", feature = "gui")`, it sits
beside the front-ends, and nothing in `shell/` or `core/` may name it.
`gui/host_native.rs` is what remains of the plan's file: the native arm of this
front-end's own split, which G6 pairs with `host_web.rs`.

### Cargo.toml shape

```toml
[features]
default = ["tui"]
tui = [
    "dep:ratatui", "dep:crossterm", "dep:directories", "dep:clap",
    "dep:chrono", "dep:anyhow", "rand/thread_rng",
]
gui = [
    "dep:eframe", "dep:egui", "dep:directories", "dep:clap",
    "dep:chrono", "dep:anyhow", "rand/thread_rng",
]

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

**As built at G6, this shape needed one correction.** A feature cannot say "on
this target only", and `gui` is both the native window and the web build — so
`gui` turning on `directories`, `clap`, `chrono` and `rand/thread_rng` would turn
them on for wasm too, and `getrandom` does not compile there. The native crates
are therefore optional dependencies of `cfg(not(target_arch = "wasm32"))` only,
rand's `thread_rng` is switched on by that table for every native build, and the
web crates are optional dependencies of wasm only; `gui` names both sets and each
target builds its own half. The web set is `wasm-bindgen`, `wasm-bindgen-futures`,
`web-sys` and `js-sys` — not `console_error_panic_hook`, because
`eframe::WebRunner` installs a hook of its own, and not `web-time`, because F1 in
a tab is `performance.now()` through `web-sys`.

`ratatui`, `crossterm`, `eframe`, `egui`, `directories` and `clap` all become
`optional = true`. `directories` and `clap` join the front-end features rather
than staying shared, because neither has any meaning in a browser tab — the
shell after G3 does not use them.

The shared, always-on dependencies shrink to `serde`, `toml`, `serde_json`,
`thiserror` and `rand` (the last only inside `core/bag.rs`, as §9.6's bit
source, and with `default-features = false` so that no `getrandom` comes with
it — an OS entropy source is F3's, and the front-end features turn it back on
with `rand/thread_rng`). `chrono` becomes a front-end dependency, because after
G3 the shell is handed a date rather than reading one; so does `anyhow`, which
only ever appeared in `main` and the terminal's loop.

**Consequence to plan for:** a bare `cargo test` builds only the `tui` half.
Every command in the Makefile grows `--all-features`, and CI runs that. This is
called out again in G13 because it is the single easiest thing to forget and the
failure mode is silent — the GUI simply stops being compiled.

---

## Working agreements

The `TERMINAL-PLAN.md` agreements all still hold. These are the ones this plan adds.

- **The core is touched exactly once, in exactly one place.** Every stage but
  one leaves `src/core/` alone; if a stage seems to need a core change, that is a
  design error in the stage. The single exception is
  [G10](#stage-g10--sub-cell-gravity), which adds one field to `GameView` and
  nothing else. It is isolated in its own stage precisely so that the exception
  is visible in the history rather than smuggled into a rendering commit.
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
| **MG6 — Parity** | G12 | Everything the terminal front-end does, the GUI does. |
| **MG7 — Accepted** | G13 | B1–B12 signed off; CI builds and tests all three targets. |

---

## Stage G0 — The documentation split

**Depends on:** nothing. **Touches:** documentation only. No code.

### The renumbering trap, and how this avoids it

The obvious split renumbers everything, and the cost is enormous and hidden:
`§12.4`, `§9.13`, `§8.2` and their siblings appear in several hundred doc
comments across `src/`, in `CLAUDE.md`, in `TERMINAL-PLAN.md` and in commit messages. A
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
| §6.3's `[display]` table | `TUI.md` | `cell_filled`, `cell_empty`, `cell_ghost`, `color_depth` are meaningless off a terminal. `show_grid` and `show_debug` are *shared* and stay in `FTM.md`. G12 finished this and added its mirror image: `GUI.md` §G8.10's `[gui]` table, meaningless on one. |
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

`src/tui/keys.rs`, new: `neutral(crossterm::event::KeyEvent) -> Option<shell::keys::KeyEvent>`.
`KeyEventKind::{Press, Repeat, Release}` map one to one. Any crossterm `KeyCode`
with no neutral equivalent (`Insert`, `Home`, media keys) maps to `None` — the
adapter returns `Option<KeyEvent>` and the loop drops what it cannot express,
which is what the bindings table does with those keys today anyway. It is a free
function and not a `From` impl because the conversion is partial and the orphan
rule refuses `impl From<crossterm::event::KeyEvent> for Option<KeyEvent>`:
`Option<Local>` is not itself a local type, `Option` not being fundamental.

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

### The spec amendment this stage forced

§13.6's "Any key stops it" was literally true while every crossterm key event
reached `Attract::key`. It is not, once the adapter drops what §10.1 cannot
name: an `Insert` press no longer stops the idle colour cycle. `TUI.md` §13.6
says so now. Nothing else in the game ever saw those keys — the bindings table
has no name for any of them.

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
   *models*; their `draw` functions stay behind in `tui/`. The `label` and
   `value` methods go with the models and become `pub`: the words §12.6 and
   §13.5 print are the specification's, and a second front-end that invented
   its own would be a second specification.
4. `ui/attract.rs`'s `Attract` splits. The state machine — `selected`, `sub`,
   `last_key`, `face`, `face_since`, `idle_shift`, `key`, `menu_key`,
   `options_key`, `Outcome` — goes to `shell/attract.rs`, and the three fields
   the drawing reads gain accessors rather than becoming `pub`. **`Background` stays in
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
6. The rest of `ui/` → `src/tui/`, gated on `feature = "tui"`. §8.1–§8.3's
   setup, teardown and panic hook go to `tui/term.rs` and the argument-parsing
   entry point to `src/bin/ftm.rs`; the whole of `app.rs` — `Session` and `App`
   as well as the two loops — becomes `tui/run.rs`. `Session` and the round are
   shell objects and they are named as such in the [target
   layout](#target-layout), but prising them out is
   [G4](#stage-g4--inverting-the-loop)'s work, not a move: they cannot leave
   `tui/` until they have stopped owning crossterm's event queue and ratatui's
   `Size`. Until then `cargo check --no-default-features` is what says so.
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
    pub const fn from_micros(micros: u64) -> Stamp;
    pub fn from_elapsed(elapsed: Duration) -> Stamp;  // an Instant origin
    pub fn from_secs_f64(seconds: f64) -> Stamp;      // macroquad's get_time(), later
    pub const fn as_micros(self) -> u64;
    pub fn saturating_since(self, earlier: Stamp) -> Duration;
    pub fn saturating_add(self, duration: Duration) -> Stamp;
}
// plus Add<Duration>, Sub<Duration> and AddAssign<Duration>, all saturating.
```

**Amended as built.** The plan first wrote the last of those as `checked_add`
returning a `Stamp`, which is two things at once and neither of them clearly:
`saturating_add` is what the shell actually needs and what the name then means.
The operators come with it because the arithmetic reads better as
`start + FACE * 3` than as a method chain, and because `Attract::step` adds to a
mark in place. Nothing saturates in practice — the range is 584,000 years — but
nothing can panic either, which is what §16 asks of a front-end that breaks F1.

Every `Instant` in `Cosmetics`, `Attract`, `Confirm`, `Fps` and `App` becomes a
`Stamp`. `Duration` stays — it is `core::time::Duration`, has no platform
dependency, and is what all the arithmetic is already in.

The front-end produces stamps: `tui/host.rs` and `gui/host_native.rs` (one
`src/native.rs` between them since G5) from an `Instant` captured at start-up,
`gui/host_web.rs` from `performance.now()` (via
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

`tui/host.rs` — `src/native.rs` since G5 — implements it over `directories` +
`std::fs`, preserving §6.2's path, §14's atomic write, and §16's "an unwritable
file is a warning, never an abort". `gui/host_native.rs` uses the same
implementation — the two native binaries share a config file, which is the point
of G12.

Two details the trait's two methods do not carry, and that turned out to matter:

- **`Ok(None)` and `Unavailable` are different answers.** Nothing stored yet is
  the ordinary first run and is never a warning; nowhere to store it is §6.2's
  and §14's "no config directory on this platform", which warns once — on the
  *read*, so the write that fails the same way a moment later stays quiet and
  the warning count is what it was before this stage.
- **The atomic write is §14's alone.** Giving §6.2's document the same treatment
  looks like tidiness and is a behaviour change: a rename replaces a read-only
  file, where the `fs::write` it had is refused by one. §17.3's sign-off checked
  the Options panel over a read-only config, and that is the case it would have
  quietly broken.

### 3. Entropy

`Session::next_seed` currently calls `rand::random()`. It becomes a
front-end-supplied `fn() -> u64`, exactly as `Startup::resolve` already takes
one. `core/bag.rs` is untouched — §9.6's PCG32 expansion and Lemire draw stay
where they are, `rand` remains the bit source, and the CLAUDE.md warning about
not "simplifying" them back to `rand`'s own API stands.

This is what keeps `getrandom`'s `wasm_js` backend and its build-time `--cfg`
out of the shared build entirely: the web host calls `Math.random()` twice and
composes a `u64`, and nothing else in the tree needs an entropy source.

### 4. Date

`highscore::today()` uses `chrono`. It becomes a front-end-supplied
`fn() -> String` producing §14's date stamp. `chrono` moves to the front-end
features; the web host uses `js_sys::Date`, avoiding `chrono`'s `wasmbind`
feature and the `wasm-bindgen` version coupling it brings.

### 5. The command line, which is a capability too

Not on the plan's original list of four, and it belongs with them: `clap` reads
an argv, a browser tab has none, and §6.4's flags become URL query parameters
there (G12). So §6.4 splits the way §6.2 does — the *grammar* is the
front-end's and moves to `tui/cli.rs`, and what crosses into the shell is
`config::Overrides`, a plain struct of `Option`s with `apply` and §6.4's
both-directions pairing rule on it. `Startup::resolve` takes that and a
`Storage` instead of a `Cli`, and `Startup` loses its `PathBuf`.

`ColorDepth` and `LockDownRule` keep their §6.3 spellings and lose their
`#[derive(ValueEnum)]`; `tui/cli.rs` writes the two impls out, which is also
where the test that they still match §6.3's tables belongs.

The three shell-side capabilities travel together as `shell::host::Host` — the
`&mut dyn Storage`, the seed and the date — because five arguments to
`Session::new` is not a design. It is a struct of values and must not become a
`trait Frontend`; `FRONTEND.md`'s "Adding a front-end" says why.

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
- New, in `tui/host.rs` (`src/native.rs` since G5): §14's temp file is renamed
  rather than left behind, a
  read-only config is refused rather than replaced, and a failed write names the
  *target* in §16's line and never the temp file.
- Moved, to `tui/cli.rs`: §6.4's synopsis, flag by flag, plus a value-by-value
  check of the two hand-written `ValueEnum` impls against §6.3's tables.

### Done when

`cargo check --no-default-features --target wasm32-unknown-unknown` is clean;
`make check` clean; A3/A6/A7 re-run on a pty with unchanged results, including
I3's "exactly one warning" for a config that is not TOML.

**MG1.**

> **Done.** `make portable` is the step, and CI installs the target for it. The
> shared dependency set is now exactly §3's five. A3, A4, A6, A7, I3 and all
> four of §16's failure paths were re-run on a pty and give the same answers;
> `CLAUDE.md`'s "What G3 settled" has what that turned up.

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
pub struct Round { /* App, Cosmetics, accumulator, last, settings generation, fits */ }

impl Round {
    /// §7: Attract -> Playing, or a restart. `now` is the moment the
    /// front-end is starting it at (F1): the accumulator and §12.5's timers
    /// both date from here.
    pub fn new(session: &Session, now: Stamp) -> Self;

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

    /// The three things `GameView` cannot answer: §13.5's rule about a running
    /// game's rules, §12.5's timers, and §12.4's figures.
    pub fn hold_enabled(&self) -> bool;
    pub fn cosmetics(&self) -> &Cosmetics;
    pub fn debug(&self, fps: u32) -> Debug;
}
```

`Cosmetics` lives *inside* `Round` rather than beside it, so that no front-end
has to remember to feed it and step 5's rule — nothing below it can reach the
core — is kept by the shape rather than by a comment.

`shell/attract.rs` grows the same methods with a 10 fps deadline (§15.3) and no
accumulator. Its `advance(now)` answers a `bool` rather than a `Next`, because
it has no core to advance and no way to end itself; its `key` takes the
`Session` so that §13.5's save is the shell's and not a thing each front-end
has to remember.

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
  the same reason: the desync it catches is cheap now and expensive in G11.
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

The thin slice, for the same reason TERMINAL-PLAN.md's Stage 6 was thin: prove the stack.

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
   build — `?seed=42&preview=3`. G12 does the general case; this stage needs only
   `seed`, and only to make the slice reproducible.

### Done when

`trunk serve` gives a playable falling piece in a browser; the same seed produces
the same game as the native binary; a page reload preserves whatever the slice
has written to `localStorage`; and a backgrounded tab, returned to, does not kill
the player.

**MG4.**

### What G6 found

All four criteria were checked in a real Chrome tab, and the first of them found
a real defect rather than a toolchain one — which is what the stage was placed
this early to do.

- **The same seed was not the same game.** `wasm32-unknown-unknown` is a 32-bit
  target, and `rand`'s `SmallRng` is a different generator on one: seed 42 dealt
  J L S O Z I T in the tab against J T S I L Z O natively. §9.6 now names
  `Xoshiro256PlusPlus`, which is the same stream `SmallRng` always was on a
  64-bit host, so no recorded seed changed meaning and the I1 snapshot did not
  move. It is a core change, and this plan reserves the core for G10; it went in
  as a commit of its own, because it adds nothing to the core and makes §9.6 true
  on a target it was already required to hold on. No test in the tree could have
  caught it — they all run on a 64-bit host — so the guard is a `const`
  assertion that `make portable` evaluates for wasm32. After the fix the tab
  and `tools/drive.py`'s next queue agree piece for piece.
- **A backgrounded tab is not "stopped".** `requestAnimationFrame` stops, but
  `eframe` 0.36 then drives `App::logic` from a timer the browser throttles, and
  §15.2 step 4's cap turns each call into at most a tenth of a second of play.
  The game creeps: 36 s minimised advanced it about 9 s in Chrome, with no
  burst on return. The criterion holds; the premise in item 2 above did not, and
  `GUI.md` §G8.7 records what actually happens, as input for G7's focus-loss
  rule.
- **`eframe` never focuses the canvas**, though it gives it a `tabindex` and
  keeps `Space`, `Tab`, `Backspace` and the arrows from the page while it has
  focus. The entry point focuses it once `eframe` has started, and both builds
  now draw **Click to play** when the keyboard is elsewhere (§G8.2). Checked:
  focus on load, no scroll and no focus change on `Space` or `Tab`, and the
  notice appearing on blur and clearing on a click.
- **A tab's run has no end**, so it has no §6.2 first-clean-exit write, no
  `Session::finish`, and no moment to print §16's warnings at. The session is
  leaked once per page; `Session::warnings()` lets the front-end report each
  warning on the console as it arises; and a game that hands back `Attract` or
  `Quit` starts a fresh one, because a tab cannot close itself.
- **Touch was decided, not deferred**: no touch controls, and the page says so
  (§1.2, §G8.8).
- Two crates the plan expected were not needed — see the Cargo.toml note under
  [Target layout](#target-layout) — and one CI job and two Makefile targets were
  added: `web-check` (clippy for wasm32, in `make check`), and `web` (`trunk
  build --release`, in a CI job of its own with a pinned trunk).

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

### What G7 found

- **Focus loss pauses, and the open decision is closed that way.** `GUI.md`
  §G2.3 said no and this stage's work list and B10 said yes; G6's measurement of
  a hidden tab — a game going on placing pieces for nobody — is what decided it.
  §G4.7 is normative and §G2.3, §G8.7 and §G1.3 are amended. The shell grew
  `Round::keyboard(heard, now)` beside `Round::viewport(fits)`: the same
  `cramp` path, held keys released, but with nothing replaced on screen. Two
  things about it were not obvious. It is **told every pump**, not on the
  change, because a countdown the player leaves running when they click away is
  not `Playing` and has nothing to pause — the pump it runs out on is the one
  that must pause; and it **settles the countdown first**, so that pump plays no
  tick. And it takes care of G6's creeping tab for free: a hidden document has
  no focus, so a backgrounded game is paused on the first `logic` pass.
- **The metric is in pixels and the minimum in points.** `cell` is floored in
  physical pixels so the grid is crisp at 1.25 and 1.5 as well as at 1 and 2;
  the minimum is 14 *points*, because legibility does not double with the
  density. At a fractional density those two disagree by up to a pixel a cell,
  so §G3.3's message computes what is *needed* at this density — 375 × 346 at
  1.25, not 364 × 336 — rather than printing a size at which it still would not
  fit. The window has no minimum size, so the too-small state is reachable
  natively too, not only in a tab.
- **Pixels changed three of §12.4's decisions, deliberately and in the spec.**
  The score is grouped in threes live (§12.4's bare digits were eight
  characters' concession); combo and back-to-back are figures in the stats
  panel rather than words on the status line; and a preview is centred by the
  cells it occupies, so an `I` sits in the middle of its slot. `LAYOUT_COLS` is
  26 and `LAYOUT_ROWS` 24; a next panel of one slot is exactly the hold panel's
  shape, and six fit beside the well with nothing to spare.
- **G5's scrim was leaking the stack.** It darkened the paused well to a
  quarter of its brightness, which is a free look §9.17 forbids. The well is now
  blanked, as the terminal's is — and the rule moved to the shell as
  `Overlay::blanks`, so the two front-ends cannot disagree about which overlays
  hide the board.
- **Four small things left `tui/` for the shell**, because the window needs the
  same answers: `shell::figures` (the grouped score and `MM:SS`),
  `Debug::figures` (the nine debug figures and their words) and `Fps` (frames
  drawn in the last second). The terminal's behaviour is unchanged — the §12.4
  mock-up test and the debug strip's are byte-for-byte what they were, and a
  `tools/drive.py` run draws the same screen.
- **`show_debug` is a plain panel over the bottom-left corner**, outside the
  metric, as the open decision suggested. It began at the top-left and hid the
  hold panel; the band under the well holds the least of the game.
- **A headless `egui` render test is cheap if it keeps one `Context`.** Making
  a context lays out the fonts; a test that made one per case took a minute and
  one that reuses it takes under a second. And a pass that is not handed to a
  renderer must say so (`FullOutput::drop_without_applying_deltas`), or `egui`'s
  debug assertion fires on the unapplied font atlas. The test in
  `gui/playfield.rs` draws every size from 0 × 0 up at three densities; G13's
  `tests/gui_render.rs` is still its harness-driven successor.
- **How it was checked, and what was not.** The web build was played in a
  headless Chrome driven over the DevTools protocol, because the desktop
  Chrome window was occluded and an occluded tab is `document.hidden` and never
  painted (G6's trap, again). Checked there: seed 42 dealing J T S I L Z, DAS
  sliding to the wall, hold, hard drop, a scripted `SINGLE` on the status line,
  the restart bar, `preview_count` 6 with the grid and the read-out, 1 with hold
  and the ghost off, the too-small message at 360 × 300, a top-out and the fresh
  game after it, and a blur mid-slide: paused with the well blank, the clock
  stopped through the blur and the countdown, and no slide on resuming. One
  headless trap worth knowing: an emulated device scale factor gives the canvas
  a 1× buffer while `egui` assumes 2×, so the page looks like a window half its
  size — use a scale of 1. **The native window was not looked at**: screen
  capture is not available to the session that built this, so the native build
  is covered by the same drawing code, the headless render test and `make
  check`, and by nothing that saw its pixels.

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

The Options panel needs `Setting::ALL` split — see G12. Only one of its eight
items is terminal-only (`Colour`, §12.3's depth); the other seven — preview,
start level, ghost, hold, 180 rotation, lock down and grid — are shared. So the
split is small, but it is not empty, and the GUI wants `[gui]` items of its own
in `Colour`'s place. Until G12 lands the split, the GUI's Options panel shows the
seven shared items and nothing else.

### Done when

A game can be played to a top out, the score entered, and the entry read back
from the file by the *terminal* binary. That cross-binary check is the point: it
is the first proof that the two native front-ends share §14's table correctly.

**MG5.**

### What G8 found

- **The Options panel needed the split to be *navigated*, not just drawn.** A
  panel that drew seven rows while `Round::key` walked eight would put the
  cursor on a row the screen was not showing — invisible, and one key from
  changing a setting the player cannot see. So which rows a panel offers became
  a front-end's answer this stage rather than G12's: `Session::settings`, set
  once at start-up, navigated by both `Round` and `Attract`, and reported by
  `Round::settings` for the screen to draw. `Setting::SHARED` is the seven;
  G12 still owns the rest of the split, and the `[gui]` rows that go in
  `Colour`'s place.
- **Three more shared answers left `tui/`**, for the same reason as G7's:
  `menus::controls` (§10.1's action words, their order, and §13.3's rule that a
  binding whose setting is off is not listed) and `figures::pps`. Both
  front-ends print the same table now, and the terminal's own mock-up tests did
  not move.
- **The boxes are ASCII and the cursor is a triangle.** §12.6 draws `▸` and
  `←→`; a window cannot assume its fonts carry either, and a glyph that goes
  missing is a box with a hole in it. The cursor is drawn as a shape and the
  hints are words.
- **The countdown is not a box.** §9.17 exists so the player can read the board
  before the clock starts, so drawing a box over the board would defeat it: the
  numeral is drawn large and part-transparent over the well, and it is the one
  overlay that does not dim what is behind it.
- **A game-over box wants the stack visible; a pause must not show it.** Those
  are two different rules — the dimming is this front-end's, §9.17's blanking is
  the game's (`Overlay::blanks`, G7) — and keeping them apart is what let the
  scrim stay under every box without leaking a paused stack.
- **Checked on the web, end to end**: paused, the panel edited (seven rows, the
  cursor wrapping at the seventh), the controls table, the countdown over a
  visible board, a top out, the game-over box, name entry typed into and
  confirmed, and the entry in `localStorage` with §14's fields.
- **The cross-binary half of "done when" is not fully checked, and that is
  honest.** The session that built this stage cannot drive the native window —
  no screen capture, no synthetic key events — so the window-to-terminal
  handoff is covered by a test over the real file store (`native.rs`: a score
  one `Session` files is read by the next through `Files`), by the shared code
  path itself (§6.2 and §14 are answered once, in `src/native.rs`, for both
  binaries), and by the web build's own top-out. Playing `ftm-gui` to a top out
  and seeing the entry on `ftm`'s attract screen is a person's check, and it is
  worth doing once.

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

### What this stage does not cover

Smooth sub-cell gravity is the other thing pixels allow, and it is a `GameView`
question rather than a rendering one. It has its own stage —
[G10](#stage-g10--sub-cell-gravity) — because it is the one change in this plan
that touches the core.

Horizontal movement and rotation are **not** interpolated, here or in G10. The
reasoning is in G10's second half; it is a rule about the whole project, not a
GUI preference, so it is stated once there and referenced from the
[invariants](#invariants-the-front-ends-must-not-break).

### What G9 found

- **The well is composed now, not painted in passes.** The flash and the wipe
  are *transformations of what is already in a cell* rather than things drawn
  over it, and a painter has no way to go back and change a rect it has
  already emitted. So `gui::playfield::compose` builds the field as colours
  first and draws it once — which is the terminal's shape, arrived at from the
  other end, and it is what makes every one of these animations a unit test
  over a pure function rather than a count of shapes.
- **`Cosmetics` is unchanged, and that constrained the drawing in a useful
  way.** Two of the three gradients pixels allow are *spatial* — the trail's
  fade and the wipe's soft front — so they come out of the geometry
  `Cosmetics` already reports. The third, the clear flash, has only a boolean
  to work with, and the answer is two strengths of one wash rather than a fade
  it would have needed a new timer for. §G6.1 records which is which.
- **The soft front cannot outlive the wipe.** Fading the four rows above the
  front is right while the front is moving and wrong the moment it reaches the
  floor: the bottom rows would keep a gradient §12.5 says settles on a grey
  stack. `wipe_percent` takes the front's position, not a depth, for that one
  reason — and the test that caught it is the one that asserts the settled
  state, not the moving one.
- **A percentage in a `u8` overflows at four rows**, which the same test found
  a minute earlier. The wash arithmetic is deliberately in the same whole-percent
  vocabulary as `shell::palette`'s brightness steps, so it is worth saying that
  the *intermediate* wants a `u16`.
- **Checked in a tab, with three of the six on camera.** The hard-drop trail
  fading up the well behind the piece, the lock flash keeping the piece's hue,
  and the game-over wipe caught mid-way with its gradient front, under §12.6's
  box. The line-clear flash and the two banners were **not** photographed: a
  scripted burst of keys cannot reliably clear a line, and the level-up banner
  wants ten of them. They are held by the unit tests over `compose` and by the
  size sweep, which now draws every animation at every size — and the lock
  flash's wash, which is on camera, is the same wash the row flash uses.
- **A tab that is not the frontmost window reports `document.hidden` and never
  paints.** Recorded at G6, met again here; a headless Chrome over the DevTools
  protocol is what this session used, with `--remote-allow-origins=*` (the
  handshake is refused without it) and a device scale factor of 1.

---

## Stage G10 — Sub-cell gravity

**Depends on:** G9. **Spec:** §9.9, §12.7, §19.2, `GUI.md` §G6.
**The one stage that touches the core.**

At level 1 a piece falls one row per second. In a terminal that lands as the
blocky aesthetic; on a forty-pixel tile it lands as a stutter, once a second,
for the whole first minute of every game. This stage fixes it, and it fixes it
by *revealing* state the rules already keep rather than by inventing any.

### Why gravity is the easy case

`Gravity` (`src/core/gravity.rs`) holds `accumulator`, a 16.16 remainder that is
always strictly below `period`. The piece's sub-row position is therefore exactly
`accumulator / period`, a value in `[0, 1)` that the rules already compute and
already depend on. Nothing is estimated, nothing is tweened, and nothing can
drift out of step with the rules, because it *is* the rules' own number.

It also scales itself, in the way one would have had to tune by hand otherwise.
At level 1 the period is 60 ticks per row, so the fraction takes sixty distinct
values on the way down — genuinely smooth. By level 10 it takes a handful. Above
1 G (level 13+, and under any soft drop) `accrue` returns whole rows per tick and
there is nothing left to interpolate — which is exactly where nobody could see it
anyway. Slow fall gets many sub-steps, fast fall gets few, with no constant to
choose.

The edge cases fall out rather than needing handling:

- §9.9 resets the accumulator on a blocked downward step, so a landed piece sits
  at exactly `0.0` and cannot appear to hover through its lock delay (§9.11).
- `current` is `None` during the clear and entry delays, so there is nothing to
  draw and nothing to offset.
- Soft drop is gravity, so it benefits for free: `soft_drop_period` divides the
  period, and the fraction stays meaningful all the way down.

### The change

One field on `GameView`:

```rust
/// How far the falling piece has come toward its next row, as a fraction of
/// the fall period in force (§9.9): 0 at the top of the row, 65535 just
/// before the next one. Zero whenever there is no piece, and zero while the
/// piece is landed, because §9.9 clears the accumulator on a blocked step.
///
/// Presentation only. The piece occupies `current.cells` and nothing else;
/// this is how far between rows it should be *drawn*, and no rule reads it.
pub fall_progress: u16,
```

Derived in `Game::view` from `accumulator` and the period in force. `&self`
still, and `Game::view` still may not mutate — T15's property is untouched.

**Not on `PieceView`**, and not on the ghost. The ghost marks a landing row,
which is a discrete fact; leaving it snapped while the piece slides is what makes
the gap close smoothly, and interpolating both would keep the gap constant and
look wrong.

### The §19 cost, stated accurately

An earlier draft of this plan gave the objection as "a field that changes sixty
times a second is a field on the wire". That is weaker than it sounded, and it
should not be what decides this: `GameView` already carries `ticks: u64` and the
whole two-hundred-cell `rows` array, both of which change every tick. One `u16`
is noise beside them.

The real cost is a §12.7 amendment and the discipline that goes with it — the
field is presentation, it is derived, no rule may read it, and a §19 server sends
it because a client cannot compute it without reimplementing §9.9. That is worth
writing down; it is not worth refusing over.

### The three alternatives, and why they lose

- **Derive it from `DebugView`.** `DebugView` carries `fall_period` but not the
  accumulator, and it is gated on `show_debug` and carries the hidden bag by
  design (§12.7). It is the wrong type for this in three separate ways.
- **Tween in the shell by watching for row changes.** No core change, and wrong:
  a shell only learns the piece moved *after* it moved, so it either renders
  behind the true position or extrapolates blind. It desyncs on soft drop, on a
  level-up, above 1 G, and on the blocked step that clears the accumulator —
  which is to say, everywhere it would be noticed. It is guessing at a number
  that exists exactly.
- **Raise the tick rate.** §15.1's 1/60 s tick is what the scoring, the speed
  curve, the timing table and every recorded seed are defined against. Not
  available, and named here only to be dismissed.

### Optional, and additive: sub-tick extrapolation

`fall_progress` changes sixty times a second, so on a 144 Hz display the motion
is smooth but quantised to the tick. A front-end may extrapolate within the tick
from its own elapsed time, **clamped so the drawn position can never pass the
ghost's row**. Purely cosmetic, never fed back, and worth doing only if the
quantisation is visible after the field lands. It needs no further spec change.

### Horizontal movement and rotation are not interpolated

The obvious next question, and the answer is no — for structural reasons, not
taste. This is stated here because it is the natural place to ask it, and it is
binding on every front-end.

1. **There is no fractional horizontal state to reveal.** `Game::tick` applies a
   shift as `for _ in 0..input.shift_cells { try_move(dx, 0) }` — a whole number
   of cells, atomically, inside one tick. No sub-cell column exists anywhere in
   the core. Interpolating it is the rejected shell-tween above, applied where
   there is not even an exact answer to fall back on.
2. **It would add lag to the signal the player is most sensitive to.** Gravity is
   the game moving a piece along a trajectory the player already knows; drawing
   that smoothly misleads nobody. Horizontal position is the player's primary
   control, read against the stack, and a tween draws the piece where it *was*.
   Tens of milliseconds are felt there, and they are felt as the game being
   unresponsive.
3. **DAS/ARR makes it incoherent anyway.** §10.3 step 3 gives `arr = 0` the
   meaning "move to the wall instantly" — up to ten cells in one tick. Any
   duration chosen to animate that contradicts the setting the player chose
   precisely to have no delay, and at small ARR values successive tweens would
   overlap and chase each other.
4. **Rotation is the same case, worse.** An SRS kick can translate a piece two
   cells and rotate it in a single atomic step (§9.5). There is no meaningful
   intermediate pose, and animating through one would draw the piece passing
   through occupied cells.

**What to do instead**, when the movement wants softening: convey the motion
without moving the piece off its true cell. A brief trailing smear over the
vacated cells, fading in 60–80 ms — the same shape as §12.5's hard-drop trail,
which already exists — or a short brightness pulse on arrival.

Both are free within the existing invariants. `GameEvent::PieceMoved` already
fires on every successful shift, so `Cosmetics` drives the timing from events and
a clock exactly as it does today. `PieceMoved` does not say *which* cells were
vacated, but a renderer may remember the previous frame's `PieceView` — which is
still drawing from `GameView` alone, so `Cosmetics` keeps its "no path to `Game`"
property and neither §12.7 nor §12.8 needs a second amendment.

The one case with a fair argument is a DAS auto-repeat slide, where the movement
is machine-paced rather than a discrete command. It still loses: the shell
resolves ARR to whole cells at a configurable rate including zero, so the rate
would have to be reconstructed in the renderer to tween against.

### A wrinkle worth knowing

§9.4 spawns every piece with minos in row 19 and then drops it one row, so a
freshly spawned `T` has a mino above the visible field, which `to_visible`
returns as `OFF_SCREEN` and the view omits. Three minos are drawn, not four,
until the piece falls again. That is pre-existing and accepted — it is why the
playfield was opened at the top — but sliding motion makes it more noticeable,
because the fourth mino now appears abruptly against neighbours that are moving
smoothly.

Carrying a row or two of the buffer zone in the view would fix it and is a much
larger §12.7 change, with §19 consequences of its own (the buffer zone is close
to hidden information). **Accept the pop-in.** It is recorded here so that the
next person to notice it knows it was seen.

> **Not accepted, in the end.** The pop was looked at once the slide was
> running and it is much worse against smooth neighbours than it was against a
> stepping piece. It is fixed, and the fix is smaller than this paragraph
> feared: the *falling piece* is no longer clipped — a mino above the field
> carries a negative row — while `rows`, the stack, still is. The §19 objection
> turned out not to apply to the half that mattered, because those minos belong
> to the player's own piece, whose kind and rotation they already know; what is
> genuinely hidden is the stack above the field, and that has not moved. The
> window draws the piece clipped to the well, so it grows in past the top edge.
> `GUI.md` §G6.6 is the section; §12.7 and §12.8 are amended.

### Tests

- **T15 still holds**: `Game::view` takes `&self` and mutates nothing.
- New: `fall_progress` is zero with no piece, zero while the piece is landed, and
  strictly increasing across ticks within one row at level 1.
- New: it never reaches 65536, and it resets on the row change — the property
  that keeps a piece from being drawn a full row low for one frame.
- New: above 1 G the field is well-defined and the piece still moves whole rows;
  nothing divides by zero when the period is 1.
- **The §19.4 canary and I1's snapshot must not move.** `fall_progress` is
  derived from state the core already had, so a snapshot that changes means the
  field was computed from the wrong thing — or, worse, that something started
  reading it. Read the diff before regenerating anything.

### Done when

A piece at level 1 falls visibly smoothly in both GUI builds; the terminal
front-end ignores the field and its §12.4 mock-up test is unchanged; `cargo test`
is green with no snapshot regenerated.

### What G10 found

- **"A landed piece sits at 0.0 because §9.9 clears the accumulator" is wrong,
  and the stage would have shipped a hovering piece on it.** The reset happens
  on a *blocked step*, and a step is only attempted when the accumulator clears
  the period — which at level 1 is tick 60, while §9.11's lock delay expires at
  tick 30. A piece resting on the stack normally locks without ever attempting
  the step. So the clause is explicit: `fall_progress` is 0 whenever the piece
  cannot move down, which is a question about the board and not about the
  accumulator. §9.9 and §12.7 are amended to say so, and it has a test.
- **The denominator has to be remembered, because soft drop is not in the
  state.** The accumulator is a remainder of the period *in force*, and only
  `Game::tick` knows whether the key was held. Divided by the plain period
  instead, a soft-dropping piece would crawl through the top twentieth of each
  row and jump the rest. `Gravity` therefore keeps the period its last `accrue`
  used. Nothing in the rules reads it — deleting the field would not change a
  single game.
- **The terminal had to be told to ignore the field.** `tui::run::Frame`
  compares the whole `FrameState`, so a field that changes sixty times a second
  would have turned every tick of every fall into a redraw of a byte-identical
  screen. `tui/run.rs` zeroes it before the comparison. That is F7's "the
  decision to compare is the front-end's alone" collecting on a debt, and it is
  the sort of thing that would never have shown up as a failing test.
- **The falling piece had to come out of `compose`'s grid**, because a grid of
  cells cannot hold "half a row down". It is drawn afterwards, with the colour
  and the washes the cell it occupies would have given it — which is why §12.5's
  two whole-row transformations became a per-cell function that both paths call,
  rather than a pass over the field.
- **No clipping was needed, and that is a consequence rather than luck.** The
  offset is non-zero only when the piece can move down, so every cell below it
  is empty and the slide can never overlap the stack, the floor or the walls.
- **Checked in a tab, by measurement rather than by eye.** A headless Chrome
  over the DevTools protocol, screenshotted on a timer: the piece descends 13
  pixels every 500 ms at a 26-pixel cell — one row a second, in sub-cell steps —
  and then stops dead for the whole of its lock delay. `--virtual-time-budget`
  is *not* a substitute: it advances `performance.now()` without running the
  frames, so every screenshot came back with the piece at spawn and the clock at
  `00:00`.

---

## Stage G11 — The attract screen

**Depends on:** G10. **Spec:** `GUI.md` §G7.

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

### What G11 found

- **A second front-end is what turns a shared *state machine* into shared
  *words*.** `shell::attract` held what was selected and how far the cycle had
  come; what it did not hold was §13.2's letterforms, §13.2's three colours,
  §13.3's reminders or §13.4's numbers, all of which were `tui/attract.rs`
  constants. Two of those would have been outright defects to copy: a second,
  differently-drawn wordmark is a second piece of branding under §1.3, and a
  second list of reminders is a second §13.3. They are all in `shell::attract`
  now, the letterforms as a **bitmap** — `#` and `.` — with each front-end
  saying what one block is drawn as: two characters in a terminal, one mino in
  a window. The terminal's §13.2 art test is unchanged and now proves the
  derivation.
- **QUIT in a browser tab was the stage's one real design question**, and the
  answer was the one the project had already made twice. A tab cannot close
  itself (§G8.1), so **QUIT is not offered there** — `Session::menu` is
  `MenuChoice::ALL` or `MenuChoice::NO_QUIT`, exactly as `Session::settings` is
  `Setting::ALL` or `Setting::SHARED`, and the screen draws the list the shell
  walks. Reloading the page instead was the alternative and is worse: it is not
  what the word means, and it would have thrown away a run's warnings and its
  in-memory table. §10.1's quit *key* stays live and comes back to the attract
  screen.
- **The attract screen takes §G3's grid, and that decision paid for itself
  three times**: one too-small threshold rather than two, no resize when a game
  ends, and no new arithmetic to test. The wordmark being fifteen blocks wide
  in a twenty-six-cell block is the only place a half cell shows up, and
  `Layout::in_slot` — written for a preview piece in G7 — already rounded that
  to a whole pixel.
- **§13.4's "painted after it, opaquely" is a character grid's sentence.**
  There a drifting `░░` *replaces* a glyph, so the regions that matter must be
  painted over it. A window draws the drift first and opaque text and minos on
  top, which satisfies what the rule is for without blanking anything; §G7.3
  says so rather than leaving the two front-ends looking as though they
  disagree.
- **The drift cannot use `rand::make_rng`.** It is the OS entropy source, and
  `wasm32-unknown-unknown` has none — the very thing G3 arranged the dependency
  tables around. F3's `host::seed` is the capability that already exists for
  this, and the generator is `Xoshiro256PlusPlus` rather than `SmallRng`, which
  would have been reaching for the one that is a *different generator* on a
  32-bit target (§G8.9).
- **A blank canvas on first load is the G6 trap, not a bug.** A tab that is not
  the visible one reports `document.hidden`, gets no animation frames, and is
  never painted however many screenshots are taken of it — `App::logic` runs,
  `App::ui` does not. The first key press activates the tab and the screen
  appears. Worth knowing before spending an hour on a rendering bug that is not
  there.
- **Looked at, in a browser, at every face.** The wordmark as minos, the menu
  with QUIT absent, all three panel faces, the full ten-row high-score
  sub-screen at `u32::MAX` scores, and the Options and Controls boxes — which
  are §G5's own, opened over this screen instead of over a game. The native
  window runs the same drawing code and was not looked at by the session that
  built it.

---

## Stage G12 — Config, CLI and persistence

**Depends on:** G11. **Spec:** §6 as amended, §14, `GUI.md` §G8.

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

### What it settled

- **`[gui]` is nine keys, and `GUI.md` §G8.10 is normative for them.** Window
  size and place, `remember_window`, `scale_percent`, `fullscreen`, `vsync` and
  `frame_cap`. Four take effect at start-up and four while the window runs;
  §G8.10's table says which, because the difference is not guessable from the
  key's name — `vsync` cannot be an Options row, since a swap interval is
  chosen when the surface is made.
- **`frame_cap` caps drawing and never the game.** It is a floor under §15.2
  step 6's deadline, so a capped window plays several ticks per repaint and
  plays the same game. That is cadence invariance reached from a *setting*
  rather than from a slow compositor, and it is why the property `tests/pump.rs`
  pins was worth pinning.
- **`remember_window` remembers size and place, and deliberately not
  `fullscreen`.** Full screen is a thing the player *chose* — in the file or in
  the panel — and `--fullscreen` is a flag, which §6.1 never writes back;
  observing it would turn one run's flag into a permanent setting. The
  write-back happens only when something moved, so an ordinary run does not
  rewrite the config, and a run over a read-only one does not add §16's warning
  to every exit.
- **The plan said `Setting::ALL` splits in two; it splits in four.** `SHARED` is
  the intersection and is what a fourth front-end starts from; `ALL`, `WINDOW`
  and `CANVAS` are the three lists actually navigated. A browser tab has no
  full-screen row for the same reason it has no **QUIT**.
- **`--print-config` is shared, and that was not obvious.** The plan listed it
  as shared and `gui/cli.rs` had a comment saying a window does not ask the
  question. The plan was right: what it prints is the whole document, `[gui]`
  and `[display]` alike, and the player comparing what the two binaries
  resolved from one file is exactly who asks for it.
- **`clap`'s `ValueEnum` impls had to leave `tui/cli.rs`.** Both native
  binaries take `--lock-down`, and an impl behind `feature = "tui"` is not
  there for a `--features gui` build. They are `src/argv.rs` now, beside
  `src/native.rs` and behind the same gate, for the same reason: an argv is a
  desktop capability, and the *spelling* of a §6.3 value is shared between the
  two binaries that write one file. This amends the invariant in `CLAUDE.md`
  that had them "beside the flags".
- **`vsync` is `glow_options.vsync`, not a `NativeOptions` field**, in `eframe`
  0.36 — the renderer's rather than the window's. It is set by assignment
  rather than in the struct literal, because naming its type means naming
  `egui_glow`, which §3's table does not list.
- **Two §16 warnings named a directory a tab has not got**, which §G8.3 had
  already flagged as this stage's to reword. They say *nowhere to keep the
  settings / the scores on this platform* now. The shell does not know where
  the bytes were going (§3.1), so it should never have said.
- **A URL is edited by hand, and the query grammar had to admit it.** A flag
  parameter takes five spellings for "on" and four for "off" (§G8.4). A command
  line has only "written or not written"; someone turning a shared link's
  setting off will reach for `=0` before they will delete the parameter.

---

## Stage G13 — Testing, CI and acceptance

**Depends on:** G12. **Spec:** §17 as amended, `GUI.md` §G9.

### Testing

- The GUI is driven headlessly to give it its **I4 analogue** —
  `tests/gui_render.rs`: every screen the program can show, at several window
  sizes including ones below the §G3 minimum, renders without panicking.
  **`egui_kittest` was not taken**, and that is a change to this plan:
  `egui::Context::run_ui` with a `RawInput` *is* the harness — it is what every
  test in `src/gui/` already uses — and the crate's own contribution is the
  AccessKit tree and the image snapshots, one of which this front-end has no
  widget tree for (§G1.1 declines `accesskit` for the same reason) and the other
  of which the next bullet excludes. A dependency whose only remaining use is
  excluded is not a dependency. `GUI.md` §G9.1 records it.
- **Image snapshots are deliberately not taken.** They need a GPU or a software
  rasteriser, and a CI job that flakes on driver differences is a CI job that
  gets marked ignored. The headless render test is the one that runs everywhere.
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
web-check: cargo clippy --no-default-features --features gui --target wasm32-unknown-unknown --lib --bins  # G6
web:     trunk build --release                                              # G6
build:   cargo build --release --features tui,gui
```

The two `check` lines are the load-bearing ones: they are the compiler holding
the shell's two boundaries, and they cost seconds.

**All of this was already in place before G13 began.** Every line above arrived
with the stage that needed it — `shell` at G2, `portable` (this plan's `wasm`)
at G3, `web-check` and the web job at G6 — because `make check` is the one list
CI runs and a stage that adds a step adds it there. The MSRV job moved to 1.95
at G5, in all three places at once. So G13 changed nothing here, and that is the
result rather than an omission: the CI half of this stage was paid for in
instalments.

`eframe` needs system libraries on a Linux runner (X11/Wayland development
headers for `winit`, and a GL stack for `glow`), so both host jobs carry an
`apt-get` step, and the web job carries `trunk` and the
`wasm32-unknown-unknown` target. That was the new class of CI failure this
repository had none of; it arrived at G5 and G6 rather than here.

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

**All twelve are signed off, one by one, and `GUI.md` §G9.3 says how each was
checked.** B8 is the one that earned the stage: a backgrounded tab did not
pause, because §G4.7's rule was right and the mechanism §G8.7 named for it was
not. Both documents are amended and `gui::host::visible` is the fix.

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
5. **Sub-cell gravity is already settled, and horizontal is settled against.**
   [G10](#stage-g10--sub-cell-gravity) lands `GameView::fall_progress`, so the
   front-end that most wants smooth motion finds it waiting rather than having to
   argue for a core change on day one. The other half of that stage binds
   Macroquad too: horizontal movement and rotation stay snapped to the cell, and
   a framework whose whole idiom is per-frame tweening is exactly where that rule
   will be most tempting to break.

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
- **Only gravity is interpolated.** `GameView::fall_progress` (G10) is the sole
  sub-cell position any front-end may draw. Horizontal movement and rotation are
  snapped to the cell in every front-end, because no fractional state for them
  exists in the core and inventing one costs input latency where players feel it
  most. Motion there is conveyed by trails and pulses off `GameEvent::PieceMoved`,
  never by moving the piece off its true cell.
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
| Key-name round trip | G1 | New. Every §10.1 name, through each front-end's adapter — crossterm's at G1, egui's when G5 adds it. |
| `cargo check --no-default-features` | G2 | New. The front-end boundary, held by the compiler. |
| `Stamp` arithmetic, `Storage` failure paths | G3 | New. §16's rules through the trait. |
| `--target wasm32-unknown-unknown` | G3 | New. **The platform boundary, held by the compiler.** The highest-value step in this plan per second of CI time. |
| `tests/pump.rs` | G4 | New. Cadence invariance, catch-up cap, phase transitions, deadlines. |
| `gui/query.rs` | G6 | New. `?seed=` into the same `Overrides` as `--seed`; unknown parameters silent, bad values one warning each. |
| The bag's generator size, on wasm32 | G6 | New, and a `const` assertion rather than a test, because tests run on a 64-bit host and cannot see it. `make portable` evaluates it. |
| `make web-check` | G6 | New. The web front-end linted for its own target; `--all-features` on the host never compiles it. |
| `gui/layout.rs` | G7 | New. §G3's metric without a window: whole pixels at six densities, the arrangement, centring, the minimum and its message, and no panic on a zero or nonsensical viewport. |
| `gui/playfield.rs` headless render | G7 | New. Every size from 0 × 0 to 4K at three densities through a real `egui::Context`, and §9.17's blank well. G13's harness test succeeds it. |
| `Round::keyboard` | G7 | New, in `tests/pump.rs` and `shell/round.rs`. B10's shell half: the pause, the released keys, and a countdown that runs out without the keyboard. |
| `Session::settings` | G8 | New, in `tests/pump.rs`. The panel offers what the front-end can apply, and the cursor stays inside that list. |
| Every overlay, headless | G8 | New, in `gui/playfield.rs`. All six drawn at every size, and `overlays::rect_of` asserted to fit inside the block. |
| The shared §14 table | G8 | New, in `native.rs`. A score one run files is read by the next through the real file store — the two native binaries' half of G8's acceptance. |
| Config round-trip preservation | G12 | New, in `shell/config.rs`. Every direction, compared **byte for byte within each table** — a save that reflowed the other front-end's half would be the same loss one step removed. Plus the panel case: one shared setting edited, every foreign key intact. |
| Query-parameter precedence | G12 | New, in `gui/query.rs`. §6.1 over a stored document, with the same clamping and the same warnings as the flags it mirrors, and the seed rule reached from a third build. |
| §G8.11's flag table | G12 | New, in `gui/cli.rs` and `gui/query.rs`, in both directions: `FLAGS` against clap's own grammar, `PARAMETERS` against `FLAGS`, and every shared flag parsed both ways and compared as `Overrides`. |
| `[gui]`'s defaults against §G3 | G12 | New, in `gui/layout.rs`. The shell holds the default window size and may not name a front-end's module; this is the join that stops it drifting from `INITIAL_SIZE`. |
| `keep_window` | G12 | New, in `gui/host_native.rs`. Moved, unmoved, `remember_window` off, and — the one that matters — a run with `--scale` and `--fullscreen` whose geometry is written back without them. |
| `fall_progress` behaviour | G10 | New. Zero when landed, resets on the row change, well-defined above 1 G. |
| §19.4 canary + I1 snapshot | G10 | **Unchanged, and that is the assertion.** A snapshot that moves means the new field was computed from the wrong state, or that a rule started reading it. |
| `tests/gui_render.rs` | G13 | New. The GUI's I4, headless through `egui::Context::run_ui` — no harness crate, see §G9.1. Seven sizes in physical pixels at four densities, every screen and overlay, and the minimum as a boundary one *pixel* wide. |
| B1–B12 | G13 | New. `GUI.md` §G9.3, checked one by one. B4's two numbers and B10's shell half are in `tests/pump.rs`; the rest wanted a screen and a keyboard. |

---

## Risk register

| Risk | Where it bites | Mitigation |
|---|---|---|
| ~~**MSRV conflict.**~~ **Retired at G5**: 0.36 taken, floor raised to 1.95 in one commit across `Cargo.toml`, §3 and the workflow. The three places still have to move together, and the `msrv` job is what catches it if they do not. | — | — |
| **The G3 abstractions are done half-way**, leaving `cfg(target_arch)` sprinkled through the shell. | G3, discovered in G6 as a slow, miserable stage. | The wasm CI check lands *in* G3 and is what defines the stage as finished. A `cfg` in `shell/` is a stage that is not done. |
| **The loop inversion changes TUI behaviour subtly.** A reordered step, a lost `dt`, a pause that no longer zeroes the accumulator. | G4, discovered in G11. | G4 lands with no GUI at all, and is validated by the terminal front-end's existing pty acceptance suite plus cadence invariance. |
| ~~**Tick/frame coupling.**~~ **Retired at G13.** | — | `ticks_due` is the only path and the invariance test runs at several cadences. B8 checked a real tab, and found the neighbouring defect instead: a hidden tab that went on playing because it still *looked* focused. |
| **The browser swallows the game's keys.** `Space` scrolls, `Tab` moves focus, the canvas never had focus. | G6, and every web build after. | `GUI.md` §G8.2 makes canvas focus normative, and G6 found `eframe` does not focus the canvas itself; B11 is a dedicated acceptance criterion, because this defect is invisible in every native test. |
| ~~**A 32-bit target plays a different game.**~~ **Found and retired at G6**: `SmallRng` is another generator on wasm32. | — | §9.6 names the generator; a `const` assertion in `make portable` holds it, because no test on a 64-bit host can. |
| **Config data loss between binaries.** | G12 ✅, in the field. | Retired as planned: `ConfigFile` keeps every table whichever binary is running, and the round trip is compared byte for byte within each table, in every direction. The second half — a run that *writes* the file for a reason of its own — is `remember_window`, and it writes only when something moved. |
| ~~**CI grows new classes of failure.**~~ **Retired, in instalments**: the `apt-get` arrived at G5 and the pinned `trunk` at G6, so G13 changed no workflow. | — | The headless render test uses no GPU and needs no harness crate; image snapshots stay out of CI deliberately (`GUI.md` §G9.1). |
| **The one core change grows.** `fall_progress` is a foothold, and the next request will be a second field — piece opacity, a spawn animation, a lock-delay fraction. | G10, and every stage after it. | The field is presentation, derived, and read by no rule; G10 is the only stage licensed to touch `src/core/`, and the I1 snapshot going red is what catches a rule that started reading it. Anything further is a §12.7 amendment on its own merits, not a follow-on. |
| **Feature-gate rot.** A bare `cargo test` stops compiling the GUI, and nobody notices for weeks. | Any stage after G2. | Every Makefile target takes `--all-features`, and the Makefile is the single source of truth CI runs. |
| **`egui` minor-version churn.** egui breaks API across minors more freely than ratatui does, and `eframe` and `web-sys` must move together. | Maintenance. | Pin the minor in `Cargo.toml` and record it in §3, as the existing table does for every other dependency. One crate fewer than this plan expected: G13 did not take `egui_kittest`. |
| **Scope creep into §18.** A window — and especially a web page — makes sound, themes, touch and mouse input feel newly reachable. | Everywhere. | §1.2 and §18 are unchanged: still not work items. Parity is the deliverable. Touch is the one that deserves a real answer rather than a reflex; see below. |

---

## Open decisions

Recorded here rather than settled, because each is a judgement the plan should
not make on its own.

- ~~**`eframe` 0.33 versus 0.36**, and with it the MSRV.~~ **Settled at G5:**
  0.36, and the MSRV raised to **1.95**, which is `egui`'s own floor. The
  recommendation in the risk register was taken — §3 already says the floor is
  set by a dependency and moves when one moves, and pinning to 0.33 to preserve
  1.88 means tracking a stale `egui` for the life of the project. `eframe` is
  taken with `default-features = false` and the `glow` backend rather than the
  default `wgpu`; `GUI.md` §G1.1 records why.
- ~~**Touch input for the web build.**~~ **Settled before G6:** accepted, and
  said on the page. There are no touch controls, and `index.html`'s footer tells
  a visitor the game is played with a keyboard; §1.2 says so and `GUI.md` §G8.8
  is normative. An on-screen control layer stays possible as a §1.2 amendment
  and a piece of design in its own right, and is not planned.
- ~~**Does losing focus pause a game?**~~ **Settled at G7: yes**, by §8.4's
  path (`Round::keyboard`), on every pump without the keyboard. `GUI.md` §G4.7
  is normative and §G2.3 is amended; a hidden tab no longer creeps (§G8.7) —
  though it took G13 to make that last clause true, because a hidden tab keeps
  the canvas's focus and `egui` had no idea. `host::visible` is the second half
  of the question.
- **Sub-tick extrapolation on top of `fall_progress`.** The field itself is
  settled — it is [G10](#stage-g10--sub-cell-gravity). What is not settled is
  whether a front-end should also extrapolate *within* a tick for displays above
  60 Hz. It needs no spec change and is purely cosmetic, so it can wait until the
  quantisation has been looked at on real hardware rather than argued about now.
- ~~**Whether the view should carry a buffer row**, so a piece entering the field
  slides in rather than popping.~~ **Settled at G10, against the recommendation
  above:** the pop-in was not acceptable once it was seen beside smooth
  neighbours — §9.4 spawns every piece but `I` with a mino above the field, so a
  `J` was drawn as three minos and then abruptly four. `PieceView::cells` is
  signed and unclipped, which gives the falling piece's own minos back without
  giving away the stack, so the §19 objection did not apply to the half that
  mattered. `GUI.md` §G6.6.
- ~~**Whether `ftm-gui` should remember its window geometry.**~~ **Settled at
  G12: size *and* position**, behind `remember_window`, which the player can
  turn off. The off-screen worry is real and is answered by the window manager
  rather than by the game; what the write-back refuses instead is a full-screen
  or maximised window, whose reported size is the display's, and a run that
  moved nothing — which would otherwise rewrite the config file every time the
  game was played. `GUI.md` §G8.10.
- ~~**What the GUI does with `show_debug`.**~~ **Settled at G7:** a plain
  panel over the window's bottom-left corner, outside §G3's metric (§G4.6). The
  nine figures and their words are `Debug::figures`, shared with the terminal's
  strip.
- **Where the web build is served, and whether high scores stay local.**
  `localStorage` makes every visitor's table private to their browser, which is
  the honest default and needs no server. A shared leaderboard is a §19 question
  wearing different clothes, and is not in this plan.
