# Falling Tetromino Manager (ftm)

A guideline-conformant falling-block game in Rust. No server, no unsafe. The
name is the joke; `ftm` is the binary, the crate, and the config and data
directories. The specification is `FTM.md`, and since G0 it has three companion
documents — see **The four documents** below.

It is no longer only a terminal game, and no longer a single binary: `EGUI.md`
builds a second and third front-end (a native egui window, and the same code as
wasm in a browser) and anticipates a fourth (Macroquad). As of G8 the tree is
`core/` + `shell/` + `native.rs` + `tui/` + `gui/`, with `src/bin/ftm.rs` and
`src/bin/ftm-gui.rs` behind `--features gui`; **the window opens and plays** —
`make run-gui` — and **so does a browser tab** — `make run-web`. G0 changed no
code; G1 moved the key vocabulary out of crossterm's hands; G2 was a move, a
rename and a feature gate, with no logic changed; G3 took the platform out from
under the shell; G4 turned §15.2's loop inside out, so the shell is now *pumped*
by a front-end rather than owning a `while`; G5 hung an `eframe` application on
the pump; G6 compiled the same application for wasm and gave it a browser's
four capabilities; G7 gave it the playing screen, G8 the boxes over it, G9
§12.5's animations under them, G10 the one core change the whole plan has —
a falling piece is drawn between two rows, and enters the well rather than
appearing in it — and G11 §13's attract screen beside the game, which is what
turned §13's *words* from the terminal's into everyone's.

**Status: Stage 12 of `PLAN.md` complete — milestone M4, accepted; `EGUI.md`
stages G0-G11 complete. Start at G12.** All
twelve stages are done and §17.3's A1-A10 are signed off one by one (the table
below). Everything in §1.1 is implemented. `cargo run --release` opens on the
§13 attract screen — wordmark, menu, the six-second cycling panel, the drifting
background and the sixty-second idle colour cycle — and **PLAY** starts a game
on the full 44 x 23 screen of §12.4, with the pause menu, the 3-2-1 resume
countdown, the §12.6 game-over box, and the §13.5 Options and §10.1 controls
boxes reachable from both the pause menu and the attract screen. A top out
that earns a top-ten place opens name entry; `Enter` files it and it appears on
the attract screen. The config file loads and saves at the §6.2 path, the §6.4
CLI is complete, the §14 table is written atomically to the data directory, and
the §6.2 and §14 warnings reach stderr after teardown. Below §12.1's 60 x 24 a
resize replaces every screen with the too-small message and forces a game in
progress into `Paused` (§8.4).

`cargo run --release --features gui --bin ftm-gui` opens a window on `GUI.md`
§G4's playing screen over §G3's cell metric: the well with its ghost and grid,
the hold panel (absent when hold is off), one to six previews, six figures, the
status line, `show_debug`'s read-out, the too-small message below a 14-point
cell, and a pause forced whenever the window loses the keyboard. Over it are
§G5's boxes: the pause menu, §9.17's countdown, game over, name entry, the
Options panel (seven rows — §12.3's colour depth is the terminal's) and the
controls table, and under them §G6's animations — the clear flash, the
hard-drop trail, the lock flash, the two banners and the game-over wipe, all on
the same `Cosmetics` the terminal reads, and §G6.5's sub-cell gravity — at
level 1 the piece slides down its row rather than stepping once a second. Since
G11 it opens on §G7's attract screen instead — the wordmark drawn as minos, the
menu, the six-second panel, the drift as outlines over the whole viewport, and
§13.5's three sub-screens, two of which are §G5's own boxes over a second
screen. `make run-web` serves the
same thing in a browser tab through trunk (`index.html`, `Trunk.toml`), with
`?seed=N` in the URL for `--seed N`, scores in `localStorage`, and §16's
warnings on the console — and no **QUIT** in the menu there, because a tab
cannot close itself. `GUI.md` §G1-§G7 and the web build's half of §G8 are
written and normative; only §G9 is still reserved.

`Game::tick(&TickInput, &mut Vec<GameEvent>)` is still the single entry point
and `Game::view()` still the only way to see the result — with `Game::debug()`
beside it for the strip. Above it, `shell/` is what no front-end owns *and no
platform reaches* — `config.rs` (§6), `input.rs` (§10), `highscore.rs` (§14),
`keys.rs` (F5's neutral `Key`/`KeyEvent` and §10.1's name grammar), `menus.rs`
(the §12.6 and §13 menu models, §10.1's controls table and which settings a
panel offers), `attract.rs` (§13's state machine),
`cosmetics.rs` (§12.5's timers), `palette.rs` (§9.2 levelled), `figures.rs`
(the grouped score, `MM:SS` and pieces per second), `time.rs` (F1's
`Stamp`), `storage.rs` (F2's `Slot`/`Storage`), `host.rs` (F2-F4 as one
borrowed bundle), and since G4 `session.rs` (`Session`, `Next`) and `round.rs`
(`Round`, `FrameState`, `Debug`, and the `App` inside them). `src/native.rs` is
the *desktop* — F1-F4 over `std::fs`, `directories`, `chrono` and `rand` — and
both native front-ends take it. `tui/` is the terminal front-end: `keys.rs` (the
crossterm adapter), `cli.rs` (§6.4's grammar), `term.rs` (§8.1-§8.3), `run.rs`
(§7's state machine and both loops of §15, which is now what a *terminal* adds
to the pump and nothing else), `mod.rs`, `theme.rs`, `cells.rs`, `playfield.rs`,
`overlays.rs` and `attract.rs` (§12, §13). `gui/` is the window: `app.rs`
(`impl eframe::App`, the pump), `keys.rs` (the egui adapter), `layout.rs`
(§G3's metric), `paint.rs` (colours and primitives), `playfield.rs` (§G4, and
§G6's animations),
`overlays.rs` (§G5), `attract.rs` (§G7, and §13.4's drift in a window's
units),
`query.rs` (§6.4 as a URL query string), `cli.rs` and `host_native.rs` for the
desktop, and `host_web.rs` for a tab — `gui::host` is whichever one the target
has, so `app.rs` never asks. T1-T17 all pass, plus I1-I4 and `tests/pump.rs`,
and the batch-invariance canary is in CI.

There is no Stage 13 of `PLAN.md`, and there will not be: that plan is
finished. **The live work is `EGUI.md`, stages G0-G13**, which adds the egui
and web front-ends and restructures the tree so a fourth front-end is additive.
Start at G12. §18 remains out of scope and §19 remains a list of constraints to
honour rather than a work item — see **Scope discipline** below.

## The §17.3 sign-off

Each of these was checked on its own, most of them on a pty through
`tools/drive.py`. Re-run any of them the same way.

| | Criterion | How it was checked |
|---|---|---|
| A1 | Build clean | `cargo build --release` and `cargo clippy -- -D warnings`, both silent. `#![allow(dead_code)]` is gone. |
| A2 | §17.1 and §17.2 pass | `cargo test --all-features`: 314 unit, 5 + 7 integration. |
| A3 | Attract on launch, PLAY starts a game | `tools/drive.py --size 24x60 enter`. |
| A4 | §10.1 controls, working DAS | A 50 ms kitty tap moves exactly one cell either way; a 0.6 s hold slides to the wall and stops. T13 pins the arithmetic. |
| A5 | `preview_count` 1-6, both sources | Next-box height measured for all six from `--preview` and from the file: 5, 8, 11, 14, 17, 20 rows, matching §12.4's `2 + 1 + 2n + (n-1)`. |
| A6 | Full game recorded, on the attract screen | `HOME=<throwaway> tools/drive.py enter <40 x space> enter`; the JSON is written, and a fresh process shows it on the panel and the sub-screen. |
| A7 | Terminal restored | `stty -a` byte-identical either side of a run on the same pty, and the teardown emits §8.3's sequence in order. |
| A8 | `mono` and `NO_COLOR` | Both play, with §9.2's letters and `..` ghosts. Counting SGR sequences in the raw capture: truecolor 16, `256` 17, `16` 8, `mono` 0, `NO_COLOR=1` 0. |
| A9 | Hold and 180 off/on, three ways | Off from the file and from `--no-hold`/`--no-rot180`: the key is inert, the hold box is gone, and both bindings vanish from the controls overlay and the attract panel. On from the Options panel, which writes the file. |
| A10 | UI builds against the view alone | The compiler's job now: every module in `core` is `pub(crate)`. See the invariant below. |

Two things A8 turned up that are worth knowing. `--color 16` reaches the
terminal as `38;5;N`, not as `30`-`37`: that is crossterm's encoding of a named
colour and not ours, and §12.3 does not specify a wire form. And a `--legacy`
run pays about two seconds before its first frame, waiting for a capability
query the terminal will never answer — crossterm's timeout, not the game's, and
the same two seconds `--print-config` now pays on such a terminal.

§16's failure paths were exercised deliberately, each end to end on a pty:
an unwritable config (the Options panel over a read-only file), unwritable high
scores (a top out over a read-only data directory), a config that is not TOML
(I3: exactly one warning), and a panic mid-frame — patched in temporarily,
which restored the terminal *before* printing, left `stty -a` unchanged, gave a
readable backtrace and exited 101.

## Read these first

1. **`EGUI.md`** — the live plan, stages G0-G13. Find the current stage; it
   names the spec sections it depends on and the tests that close it. Read that
   stage, plus "The decisions this plan rests on" and "The central idea", which
   are what the rest of it follows from.
2. **The specification**, in whichever of the four documents below owns the
   sections the current stage names. Read those sections, not the whole thing.
3. **`PLAN.md`** — the twelve stages that built v1.0. History, not instructions.
   Worth reading when you want to know why something is the shape it is.

The specification is ground truth. **If the code and the spec disagree, the
spec is wrong until it is amended** — fix it in the same commit and say so in
the message. Never let the code silently diverge.

## The four documents, and the number-stability rule

G0 split the specification in four. **Section numbers did not move.** The
sections that left `FTM.md` kept their numbers in the file they moved to, so
every `§12.4` in the source still resolves — to `TUI.md` rather than to
`FTM.md` — and `FTM.md` keeps a stub at each vacated number saying where it
went. **Do not renumber anything.** A renumber would silently invalidate
several hundred doc comments, and each one would still *read* fine.

| Document | Owns | Is |
|---|---|---|
| **`FTM.md`** | §1-§7, §9-§11, §12.7, §12.8, §14-§19 | The front-end-agnostic specification: rules, config, states, controls, the view model and the event stream, high scores, timing, errors, testing, §19. |
| **`FRONTEND.md`** | no numbers | The contract any front-end is written against: F1-F7, what it may assume, what it must never do. The document a fourth front-end reads first. |
| **`TUI.md`** | §8, §12.1-§12.6, §13, §6.3's four glyph and colour keys, §17.3's A1-A10 | The terminal front-end. Raw mode, the 60 x 24 minimum, colour depth, the 44 x 23 layout, the attract screen, the acceptance table below. |
| **`GUI.md`** | §G1-§G9 | The egui front-end, native and web. §G1 (the application, the version pin, the loop) and §G2 (input) are written, by G5; §G3 and §G4 by G7, §G5 by G8, §G6.1-§G6.4 by G9 and §G6.5-§G6.6 by G10; G11-G13 fill the rest stage by stage. |

An unqualified `§n` means `FTM.md` §n, except for the eleven numbers `TUI.md`
owns. `§Gn` means `GUI.md`; a future `MACROQUAD.md` would take `§M`.

Two rules that keep the split honest: **§12.7 and §12.8 stayed in `FTM.md`**,
because they are what a front-end is written *against* rather than a rendering
technique — a stub in the moved-out half is a sign something was filed wrong.
And **`FRONTEND.md` owns no section numbers**: every rule in it is normative
somewhere in `FTM.md`, and it collects rather than legislates.

## Invariants that are easy to break

These are the ones a fresh session gets wrong. Each is normative in the spec.

- **Coordinates are y-down** (§5): `row` 0 is the top of the 40-row buffer, 39 is
  the floor, positive `dy` moves a piece **down**. The SRS kick tables in §9.5
  are **already converted** to this convention. Do not negate them again.
- **The core takes no clock and no I/O** (§3.1), **and neither does the shell**.
  Time enters the core only as ticks, and enters the shell only as a
  `shell::time::Stamp` the front-end hands over. Nothing in `shell/` may name
  `Instant::now`, `std::fs`, `rand::random` or a calendar — `make portable`
  (`cargo check --no-default-features --target wasm32-unknown-unknown`) is the
  compiler holding that, exactly as A10 holds the core's façade. A
  `cfg(target_arch)` in `shell/` passes that check and is a bug, not a fix.
- **The core advances in fixed 1/60 s ticks** (§15.1), never a variable
  `Duration`, and must be **deterministic**: same `RulesConfig` + seed + inputs ⇒
  byte-identical state.
- **No floating point in the rules** (§9.9, §6.6). Gravity is an integer *fall
  period* in 16.16 ticks-per-row — a period, not a rate, so level 1 falls on tick
  60 exactly. The accumulator is a **remainder**: always below the period in
  force, so a period that shortens under it (soft drop, a level-up) cannot cash
  in charge banked at the slower rate. Durations are integer ticks, converted once at config load.
- **The renderer draws only from `GameView`** (§12.7) and never reads `Game`.
  Cosmetics are driven by `GameEvent` (§12.8); dropping every event must change
  nothing about the game. The core appends to the caller's event buffer and
  never reads it back.
- **View and event coordinates are visible-field coordinates** — `(col, row)`
  with row 0 the topmost *visible* row, matrix row 20. **`PieceView::cells` is
  the one signed pair**, and the one thing not clipped: a negative row is a mino
  of the falling piece above the field, all four minos are always present, and
  there is no sentinel in the array. Everything else is unsigned and clipped in
  `core/view.rs` — `GameView::rows` is the visible field alone, and an event
  coordinate above it is omitted, encoded as `(255, 255)`. The asymmetry is
  §12.7's and §12.8's, and it is the difference between the two streams: an
  event starts an animation and there is nothing to animate where nobody can
  see, while the falling piece is *where it is* and §9.4 puts part of it above
  the field on every spawn but `I`'s. A front-end draws as many rows above the
  field as it has room for — the terminal none, the window one, clipped to the
  well so a piece grows in rather than appearing (`GUI.md` §G6.6). Neither has
  to know a buffer zone exists to do that.
- **`RulesConfig` and `PresentationConfig` are separate structs** (§6.5). Do not
  merge them into one `Config`.
- **DAS/ARR live in the shell**, not the core (§10.3). The core is told a
  direction and a whole number of cells; it has no notion of a key being down.
  Input the shell has resolved is held until a tick consumes it, because a frame
  may legitimately run zero ticks (§15.2 step 6 wakes the loop early on a key).
- **Terminal teardown is idempotent and runs from three places** (§8.3): normal
  exit, an error, and the panic hook — which is installed *before* raw mode, so
  a crash restores the terminal before it prints. Both §8.2 input paths are
  live; do not let one of them rot.
- **T-spin**: the "last action was a rotation" flag must survive a hard drop, and
  kick test 5 always means a proper T-spin (§9.13). This is the most commonly
  botched rule in the spec.
- **Scoring reads the level at the lock**, before §9.12 step 7 advances it, and
  the perfect-clear bonus reads the same level and the same back-to-back flag as
  the clear that earned it — not the chain state afterwards, which the clear may
  just have switched on. `Game::clearing_b2b` is what carries that flag across
  the clear pause.
- **`back_to_back` is two different questions.** `GameView::back_to_back` is
  whether the chain is *live*; `LinesCleared::b2b` is whether *that clear* was
  paid at the chained rate. They differ on the clear that starts a chain (§9.15,
  §12.8).
- **Hold** clears its lock-out when the next piece **locks**, not when it spawns
  (§9.7).
- **A four-line clear is a `QUAD`** (§1.3, §2, §9.14), never a `TETRIS` — in the
  interface, in the spec and in the source, where the variant is
  `ClearKind::Quad`. The word is trademarked and is kept out of the game
  entirely; the spec keeps it only for the trademark and for the Guideline it
  cites.
- **Disabled keys** (`hold_enabled`, `allow_180_rotation` off) are dropped at the
  input boundary, so they cannot reset a lock-delay timer as a side effect
  (§10.1).
- **Animations see events and a clock, and nothing else.** `shell::cosmetics::Cosmetics` takes
  `&[GameEvent]` and an `Instant`; it has no path to `Game`, which is what makes
  §12.5 provably free of side effects (§12.8). Keep it that way — if an
  animation seems to need to ask the core something, the answer belongs in
  `GameView` or in the event.
- **Overlays are centred over the whole 44 x 23 block**, not over the
  playfield's interior: §12.6's game-over box is 24 characters wide and the
  interior is 20. For a box that does fit it comes to the same thing, because
  the playfield is itself centred in the block.
- **`DebugView` is a second view type, not a field on `GameView`** (§12.7).
  `Game::debug()` sits beside `Game::view()`. Two reasons: `show_debug` is a
  presentation setting so the core cannot be told whether anyone is looking,
  and — the load-bearing one — the bag beyond `preview_count` is **hidden
  information**, and under §19 `GameView` is what a server sends a player. Do
  not fold it in to save a method.
- **The Options panel applies presentation at once and rules never** (§13.5).
  A game keeps the rules it started under; colour depth and the grid take
  effect on leaving the panel. `Chrome::hold_enabled` therefore comes from the
  running game's `RulesConfig`, not from `App::config`.
- **Clamping is silent in `RulesConfig::from_settings` and loud in the loader**
  (§6.2, §6.3). `from_settings` is also the path a §19 peer's rules take, where
  there is nobody to warn; `config::validate` is what tells the player what
  their file asked for and what it got.
- **The config file is parsed value by value**, not deserialised whole (§6.3):
  a value of the wrong type is rejected by itself and the default used. Only a
  document that is not TOML at all falls back wholesale, and that is the one
  case I3 pins at exactly one warning.
- **`tui::theme::Glyphs` are leaked, once, at start-up** so `Theme` stays `Copy`
  (§12.2). `Glyphs::configured` is a start-up call, not a per-frame one.
- **`Session::generation` is one of the frame's five components, and the
  comparison is the *terminal's*.** §15.2 step 5 draws only when the frame
  changed, and `tui::run::Frame` compares `shell::round::FrameState` — the
  `GameView`, the `Overlay`, the generation counter, §10.1's restart bar and
  whether the viewport is cramped — plus §12.1's terminal size, which is the
  terminal's own addition because its message names it. Anything else the
  screen comes to show has to join that struct or bump the counter, or it will
  not be redrawn — and, worse, will not be *erased*. Two things have already
  walked into this: the restart bar, and the too-small message, which changes
  as the window is dragged. **An immediate-mode front-end must not port this**
  (`FRONTEND.md` F7): `frame()` returns the state, and the decision to compare
  is the front-end's alone.
- **A score of 0 never qualifies, ties keep the older entry above, and a
  seeded run is never recorded** (§14, §6.4). All three live in
  `highscore::Table`; the seed rule is `App::finish`'s, because it is the only
  place that knows the run was seeded.
- **The attract screen has been seen now.** §13 stays provisional, but its
  layout is pinned by tests and its mock-up in §13.3 is what the code draws.
  The panel is four rows, not three: seven control entries do not fit in six
  slots.
- **The core's public surface is its façade, and the compiler holds it there**
  (§3.1, §17.3 A10). Every module inside `core` is `pub(crate)`; `core/mod.rs`
  re-exports exactly what the shell may name. Adding a `pub use` there is a
  decision about §19's wire vocabulary, not a plumbing convenience — the list
  and the reasoning are in the amended A10.
- **§12.1's minimum is 60 x 24 and it is the spec's, not the layout's.** The
  playing screen's block is 44 x 23 and the attract screen's 36 x 20, so both
  fit with room to spare; `tui::fits` is the one place that decides, and both
  `tui::draw` and `tui::attract::draw` check it themselves so no caller can reach a
  layout that assumes room it has not got. `show_debug`'s strip makes the block
  44 x 28 and deliberately does *not* move the minimum: a short terminal is
  drawn without it.
- **A resize forces a game into `Paused` before the screen goes** (§8.4). That
  is `App::cramp`, and it releases the held keys the way the pause menu does —
  nothing expires a held key while the clock is stopped (§8.2). It does not
  undo itself when the terminal grows again; the player leaves the pause, and
  gets the 3-2-1 countdown for it.
- **What is drawn is §12.3's levelled palette, not §9.2's table** (§9.2, §12.3).
  §9.2's seven are equally saturated but not equally bright — luma 17 for blue
  against 223 for yellow — so `theme::levelled` lifts purple, red and blue, and
  every piece colour on every screen goes through it: the field, the ghost, the
  previews, the hold box, §13.4's drift and §13.2's wordmark all call
  `Theme::piece`. The three do not land on one number: purple reaches orange's
  165, red and blue stop at 102 and 84, because blending toward white buys
  brightness with saturation and those two turn into salmon and lavender long
  before purple stops being purple. The lift is **shared presentation and lives
  in `shell/palette.rs`**, because the luma problem is a property of §9.2's
  colours and not of terminals (G2 amended §12.3 to say so); what stays in
  `tui/theme.rs` is how a colour lands on *this* display — the 256-colour
  entry, the 16-colour name, `DIM`. `Colour::rgb` is still §9.2, which is what
  a §19 client is handed, and the levelled value is the *base* the §12.3
  dimming scale runs from, so a piece and its ghost are one hue.

- **The shell is pumped, not looped** (§7, §15.2, `FRONTEND.md` F7). §15.2's
  seven numbered steps are methods on `shell::round::Round` — `key` is step 2,
  `advance` is steps 1 and 3-5, `frame` is what step 5 draws from, `deadline`
  is step 6's advice and `viewport` is §8.4 — and the front-end owns the loop
  around them. Two consequences are easy to lose. **`advance` must stay correct
  at any cadence**, which it is because its accumulator is over real elapsed
  time and never over frames; nothing may start counting frames. And
  **`deadline` is advice, not a frame rate**: a front-end may be woken sooner
  by a key, a compositor or a tab regaining focus, and one that renders at
  vsync may ignore it. `tests/pump.rs` is what holds both, and it is §19.4's
  sibling — the same desync, one layer up.

- **The window's grid is whole pixels, and its minimum is in points** (`GUI.md`
  §G3). `gui::layout::Measure` floors the cell in *physical* pixels and centres
  the block on a whole pixel, so every rect it hands out is crisp at 1.25 as
  well as at 2; the minimum is 14 *points*, because legibility does not double
  with the density. Everything on the playing screen is placed through
  `Layout` — a rect computed in points beside it will land between pixels. Text
  is the exception and is placed freely; `egui` rounds it.

- **A screen offers what its front-end can do, and the shell navigates the
  same list** (§13.3, §13.5, `GUI.md` §G5, §G7.5). There are two of these now.
  `Session::settings` is the §13.5 panel's — `Setting::ALL` for the terminal,
  `Setting::SHARED` for the window, which has no §12.3 colour depth — and
  `Session::menu` is §13.3's — `MenuChoice::ALL`, or `MenuChoice::NO_QUIT` in a
  browser tab, which cannot close itself. Drawing a different list from the one
  the shell walks puts the cursor on a row nobody can see. G12 finishes the
  settings split and adds the `[gui]` rows.

- **§13's words are shared, and only the blocks are a front-end's** (§13.2,
  §13.3, §13.4, `GUI.md` §G7). §13.2's letterforms live in `shell/attract.rs`
  as a bitmap, with §13.6's colour cycle, §13.3's reminders and §13.4's
  numbers; a terminal draws one block as two characters and a window as one
  mino. A second wordmark would be a second piece of branding under §1.3, and a
  second list of reminders a second §13.3. What stays in each front-end is
  where a thing lands and how the drift is positioned — that much is genuinely
  measured in characters or in pixels.

- **Losing the keyboard pauses a game, every pump, and draws nothing**
  (`GUI.md` §G4.7). `Round::keyboard(heard, now)` is §8.4's `cramp` path beside
  `Round::viewport`, and `gui/app.rs` calls it on every `logic` pass rather than
  on the change: a countdown the player leaves running when they click away is
  not `Playing`, so the pump it runs out on is the one that must pause, and
  `keyboard` settles the countdown first so that pump plays no tick. A hidden
  tab has no focus, so this is also why a backgrounded game no longer creeps.
  The terminal does not call it.

- **§9.17's blank well is `Overlay::blanks`, not a front-end's `matches!`.**
  Both front-ends ask it. G5's scrim let the paused stack show through at a
  quarter brightness, which was exactly the free look §9.17 forbids. It gates
  the *whole* of the window's composition, not each animation in turn — a
  flash or a wipe that outlived the pause would be the same free look.

- **`fall_progress` is the only field on `GameView` no rule may read, and the
  terminal drops it before comparing frames** (§9.9, §12.7, `GUI.md` §G6.5).
  It is §9.9's accumulator over the period in force — the piece's exact sub-row
  position, not a tween — and it is zero whenever the piece cannot move down.
  That last clause is **explicit and not a consequence of §9.9's reset on a
  blocked step**: at level 1 the lock delay expires at tick 30 and the period at
  tick 60, so a resting piece normally locks without ever attempting the step
  that would clear the accumulator. The denominator is remembered in `Gravity`
  because only `Game::tick` knows whether soft drop was held. And because the
  field changes sixty times a second, `tui/run.rs` **zeroes it before building
  its `Frame`** — a character cell cannot draw it, and leaving it in would make
  every tick of every fall a redraw of a byte-identical screen. Only the
  falling piece moves: the ghost marks a landing row and stays snapped, which
  is what closes the gap smoothly. Horizontal movement and rotation are never
  interpolated, in any front-end — `GUI.md` §G6.5 has the four reasons.

- **A window's animations are transformations, not layers** (`GUI.md` §G6.2).
  §12.5's flash and wipe change the colour a cell was already going to be
  drawn, so `gui::playfield::compose` builds the well as colours and paints it
  once; a painter cannot revisit a rect it has emitted. That is also what makes
  each of them a unit test over a pure function. The timings and the triggers
  stay `shell::cosmetics`', and **nothing in a front-end may ask `Cosmetics`
  for a number the other one does not have** — the trail's fade and the wipe's
  soft front come out of the geometry it already reports, and the clear flash's
  two halves are two strengths of one wash because a boolean is all there is.

- **The four capabilities are the front-end's, and three of them travel as
  `shell::host::Host`** (§3.1, `FRONTEND.md` F1-F4). Storage, the seed and the
  date are values a run borrows; time is the fourth and arrives as a `Stamp`
  argument instead, because every entry point already takes the moment it is
  called at. `Host` is a struct of *values* and must not become a
  `trait Frontend` — the front-ends share the shell by calling it.

- **`src/native.rs` is a desktop, not a fourth layer** (§3.1, §4, `FRONTEND.md`).
  It is behind `any(feature = "tui", feature = "gui")` — and `not(target_arch =
  "wasm32")`, since `gui` is also the web build — and sits *beside* the
  front-ends: both native binaries answer F1-F4 from it, because §6.2 and §14
  give `ftm` and `ftm-gui` one config file and one high-score table between them
  and §14's atomic write has one home. G5 moved it out of `tui/host.rs` rather
  than copying it under `gui/`. Nothing in `shell/` or `core/` may name it, which
  is what `make shell` and `make portable` keep true; a third *native* front-end
  adds nothing here, and the web build takes nothing from it.

- **`Ok(None)` and `StorageError::Unavailable` are different answers** (§6.2,
  §14, §16). Nothing stored yet is the ordinary first run and is never a
  warning; nowhere to store it warns **once, on the read**, and
  `StorageError::warning()` returns `None` for it so the write that fails the
  same way a moment later stays quiet. Getting this wrong changes I3's warning
  count without changing any test.

- **§14's atomic write is §14's alone** (§6.2, §14). `native::Files` renames
  a temp file into place for the high-score table and does a plain `fs::write`
  for the config, deliberately: a rename goes straight over a read-only file
  where a write is refused by one, and §17.3 checked the Options panel over a
  read-only config. §16's line names the *target* either way — a player has
  never heard of the temp file.

- **§6.4's grammar is a front-end's and its meaning is the shell's.** `clap`
  lives in `tui/cli.rs`; what crosses into `shell/config.rs` is `Overrides`,
  which a URL query string will fill in just as well (G12). `ColorDepth` and
  `LockDownRule` therefore have hand-written `ValueEnum` impls beside the flags
  rather than derives on the types, and the test that they still match §6.3's
  tables lives with them.

- **Keys reach the shell neutral, and only the adapter knows otherwise**
  (`FRONTEND.md` F5). `shell::keys::{Key, Mods, KeyKind, KeyEvent}` is the
  vocabulary above the front-end, and §10.1's name grammar — `parse_key`,
  `is_key_name` — lives with it, because the `[keys]` table a player writes is
  shared property. `tui/keys.rs` is the only module in the terminal front-end
  that may name a crossterm key type, and it converts at the point the loop
  reads. A `KeyCode` §10.1 has no name for is dropped there and never reaches
  the shell, which is why §13.6 says "any key the game can *name*".
  `KeyEvent` must stay **synthesisable**: a front-end with polled input will
  manufacture the stream by diffing frames, so nothing above may depend on
  seeing every intermediate event, on sub-frame ordering, or on an event
  arriving anywhere but a frame boundary.

- **The piece sequence is the spec's, not `rand`'s** (§9.6). `bag::seeded`
  expands a `u64` seed with PCG32 and `Bag::uniform_inclusive` draws a range with
  Lemire's method, both written out. Do **not** "simplify" them back to
  `SmallRng::seed_from_u64` and `random_range`: `rand` has changed each of them
  once already — the seeding by 0.10, the range draw by 0.9 — and either change
  silently makes every recorded seed name a different game. `rand` supplies the
  generator and nothing else. The I1 snapshot is what catches a mistake here,
  and it caught this one. **The generator is named, too**: `Xoshiro256PlusPlus`,
  never `SmallRng`, which is `Xoshiro128PlusPlus` on a 32-bit target — and
  wasm32 is one, so the web build dealt a different game for every seed until
  G6 found it. The I1 snapshot *cannot* catch that one, because the tests run on
  a 64-bit host; a `const` assertion on the generator's size in `core/bag.rs`
  does, in `make portable`.

- **`Chrome` carries what `GameView` cannot.** `hold_enabled` is the one layout
  question the view cannot answer — an empty hold slot and an absent hold
  mechanic are both `hold: None` — so it travels with the theme and `show_grid`
  rather than being smuggled into the view (§12.4, §12.7).

## Working agreements

- Tests land **with** their stage, not after it. `PLAN.md` maps every test in §17
  to an owning stage.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` all clean at
  every stage boundary.
- `#![forbid(unsafe_code)]`.
- One coherent change per commit; every commit builds.
- **The batch-invariance test is the canary** (§19.4): the same input log fed as
  1 × 6 ticks and as 6 × 1 ticks must produce identical snapshots. It lives in
  `tests/scripted_game.rs`, has been in CI since Stage 5, and must never be
  marked ignored. If it fails, stop and find the desync — do not proceed.
- **The I1 snapshot** (`tests/snapshots/scripted_game.txt`) is regenerated with
  `UPDATE_SNAPSHOT=1 cargo test --test scripted_game`. Stage 7 has to, when the
  score stops being zero. Read the diff before committing it: a snapshot that
  moves for no reason is exactly the bug the test exists to catch.
- **§12.4's mock-up is a test.** `tui::playfield::tests::the_screen_matches_the_
  spec_mock_up` renders the exact state the mock-up depicts through a
  `TestBackend` and compares it character for character. It is not in §17.1 —
  that list is core-only by design — but it is the acceptance criterion for
  Stage 9, and the cheapest way to notice a layout that has drifted by one
  column. §12.6's two boxes are checked the same way.

## Scope discipline

- §18 (other modes, sound, replays, themes, demo bot) and §19 (networking) are
  **not work items**. §19 is a list of constraints to honour, already baked into
  Stages 3, 5 and 8. The only networking deliverable in the entire plan is that
  one CI test.
- The attract screen (§13) is explicitly provisional. It has now been built
  plainly and looked at; iterate on it if it wants it, but §17.3 never judged
  its looks and the plan is finished either way.
- **`EGUI.md`'s scope is its stages and nothing beside them.** A window, and
  especially a browser tab, makes sound, themes, mouse and touch input feel
  newly reachable. §1.2 is unchanged: the game is keyboard-driven. Touch got a
  real answer rather than a reflex, before the web slice: **no touch controls**,
  and the page says so (§1.2, `GUI.md` §G8.8). Adding them is a §1.2 amendment.
- **Macroquad is `EGUI.md`'s §19**: a list of constraints so the front-end stays
  cheap later, not a thing to build. Do not write `MACROQUAD.md`, and do not add
  a `trait Frontend` — the front-ends share the shell by calling it, not by
  satisfying an interface designed before the third one existed.

## What Stage 11 settled, and what it left

- **`Session` is the state above a game**, and it is where the config, the
  config path, §16's warnings, the §14 table and the seed policy live. `App` is
  one game and the input state that drives it. `run` is a loop over `Next`
  (`Attract` / `Play` / `Quit`); §15 asks for two loops and there are two —
  `attract` at 10 fps with no accumulator, `round` at 60 Hz with one.
- **§7 is two levels, not one enum**, and the spec says so now. `Next` chooses
  the screen; `Phase` says where inside a game. The phases §7's original list
  did not have are the ones drawn *over* a game: `Options`, `Controls` and
  `Resuming`.
- **The restart key is the only held `Action`.** §10.2's `Held` is the three
  movement keys and nothing else, so `Confirm` in `tui/run.rs` tracks the
  restart hold, off the action path entirely (`Bindings::action_of` is what
  lets the shell pick it out before `InputState` sees it). In legacy mode it
  survives silence for `RESTART_QUIET` — 700 ms, chosen to outlast the OS's
  *first* auto-repeat, not the 90 ms `HOLD_TIMEOUT` that separates later ones.
- **The high-score path is not overridable**, so a `drive.py` run writes to the
  real data directory unless it is given `--seed` (never recorded) or a
  throwaway `HOME`. `HOME=/tmp/... tools/drive.py ...` is how A6 was checked.

## What Stage 12 settled

- **A10 is the compiler's now, not an audit's.** Performing a one-off audit
  proves nothing about the commit after it, so every module in `core` became
  `pub(crate)`. Doing it found one real leak — `shell/input.rs` reaching for
  `core::matrix::WIDTH` where it wanted `VIEW_WIDTH`, the same ten in the
  vocabulary the shell is entitled to — and one imprecision in A10's own
  wording, which §17.3 is amended for: `PieceKind`, `Colour` and `Rotation` are
  view vocabulary, because a client handed a `GameView` cannot draw it without
  §9.3's cell patterns.
- **`#![allow(dead_code)]` is gone.** It was there because the core was built
  stages ahead of its callers. Removing it left exactly two warnings, both
  honest: `LockDown::is_landed` and `resets_used` are read only by T7, and are
  `#[cfg(test)]` now.
- **`tools/drive.py` can resize.** `resize:ROWSxCOLS` is a pseudo-key that
  resizes the pty and signals the child; each frame is replayed at the size in
  force when it was taken. §8.4 and §12.1 cannot be reached any other way
  without a human dragging a window.
- **§12.1's message is centred in the room there is**, not in the room it
  wants. Padding it to its full width pushes the first line off the left of a
  narrow screen, and 1 x 1 — which I4 requires and a dragged window passes
  through — comes back blank instead of showing a `T`.

## What G2 settled

- **The shell boundary is the compiler's, exactly as A10 made the core's.**
  `cargo check --no-default-features` builds `core/` + `shell/` with neither
  front-end feature on, and it is a `make check` step. **Everything else in the
  Makefile grew `--all-features`**, and this is the single easiest thing to
  forget: a bare `cargo test` builds only the `tui` half, and the failure mode
  is silent — the other front-end simply stops being compiled.
- **`Session` and `App` stayed in `tui/run.rs`, and that was deliberate.** They
  are shell objects, but they could not move while they owned crossterm's event
  queue and ratatui's `Size`. G2 only moved files; G4 inverted the loop and
  then they moved, to `shell/session.rs` and `shell/round.rs`.
- **`Attract` lost its `Background`.** §13.4's drift is positioned in matrix
  cells of a *character grid*, so it stays in `tui/attract.rs`; a window will
  want its own. `Attract::step(now)` reports whether the *state* changed and
  the front-end folds its drift's answer in beside it. Both halves are stepped
  unconditionally — short-circuiting `step` would stop the panel's six-second
  cycle rather than the redraw.
- **The menu models are the specification's words.** `label` and `value` moved
  to `shell/menus.rs` with their types and became `pub`, because a second
  front-end that invented its own labels would be a second §13.5.

## What G3 settled

- **The shell is portable, and the compiler says so.** `make portable` is
  `cargo check --no-default-features --target wasm32-unknown-unknown`, it is a
  `make check` step, and CI installs the target for it. It is the stage's real
  deliverable: it is what makes G6 a short stage rather than a long one.
- **`rand` is the one that would have crept back.** `getrandom` refuses to
  *compile* for `wasm32-unknown-unknown` without a build-time `--cfg`, so the
  shared `rand` is `default-features = false` and `tui` turns the OS source back
  on with `rand/thread_rng`. `core/bag.rs` is untouched, and the warning above
  about not simplifying §9.6 back into `rand`'s own API stands unchanged.
- **The shared dependency set is now exactly §3's five**: `rand`, `serde`,
  `toml`, `serde_json`, `thiserror`. `clap`, `directories`, `chrono` and
  `anyhow` all moved behind `tui`; `thiserror`, which had become unused, is
  what `StorageError` is written with, so §3's row for it is true again.
- **The failure paths were re-run end to end, and one of them found a bug in
  this stage.** Writing the config atomically "for symmetry" would have silently
  started replacing a read-only config file, and naming the temp file in §16's
  warning would have told the player about a file they have never seen. Both are
  fixed and both now have tests; §17.3's A7 and I3 are unchanged.
- **`Session` grew a lifetime**, because it borrows the front-end's store for
  the length of the run and `main` needs it back afterwards for §6.2's
  first-clean-exit write. That is `Session<'a>` until G4 moves it to
  `shell/session.rs`, where it will still have one.
- **Two identical warnings are still possible**, and were before this stage: a
  config that fails to save from the Options panel *and* on the first-clean-exit
  write says so twice, because `Session::warn` dedupes and `main`'s own push
  does not. It is only reachable when the file did not already exist, which is
  why §17.3 never saw it. Left alone deliberately — it is a §16 wording question
  rather than a G3 one.

## What G4 settled

- **`Round` is §15.2's loop body as an object, and `tests/pump.rs` is the proof
  it can be driven by something that is not a terminal.** Cadence invariance is
  the test that matters: the same key log over the same span of stamps, at
  60 Hz, at 144 Hz and at a jittery cadence with several zero-length frames,
  gives a byte-identical `GameView`. It also plays a whole game to a top out
  and through name entry, headless, in half a second — the part of §17.3's A6
  that used to need a pty.
- **Key stamps in that test sit at multiples of 50 ms, and that is load-bearing
  rather than tidy.** A key is consumed by the first tick that runs at or after
  it reaches the shell, so "the same inputs at the same moments" has to mean
  the same *tick*: a front-end that polled at 10 fps would genuinely deliver
  its keys later and would genuinely play a different game. Every cadence in
  the test is woken at each key's stamp (§15.2 step 6 wakes a loop early on a
  key), and no cadence runs more than one tick in a frame, so every cadence
  hands each key to the same tick. Loosen either and the test starts failing
  for a reason that is not a bug.
- **The countdown is settled at the top of `key` as well as `advance`.** §9.17's
  3-2-1 is the one non-running phase that ends by itself, and it ended *before*
  the event queue was drained when these steps were a loop body. `Round::settle`
  is what keeps that true now that they are not: a key arriving in the frame the
  countdown expires is the player's first input of the resumed game, not one
  swallowed by the overlay.
- **`Glyphs` left `Session`.** Leaking three `&'static str` so a ratatui `Theme`
  can be `Copy` is a terminal's concession; `tui::run::run` interns them once and
  carries them down. A browser tab that is reloaded repeatedly is exactly the
  wrong place to have inherited it.
- **`Cosmetics` lives inside `Round`**, not beside it, so no front-end has to
  remember to feed it — and §15.2 step 5's rule that nothing below it can reach
  the core is kept by the shape rather than by a comment.
- **`Attract::key` takes the `Session` and saves the config itself.** §13.5's
  "leaving the panel writes the file" is the specification's, and a second
  front-end that forgot it would be a second §13.5. What the front-end notices
  instead is `Session::generation`, which the panel bumps.
- **The terminal's behaviour is unchanged, and that was checked rather than
  assumed.** A3, A4, A6 and A7 were re-run on a pty: the attract screen on
  launch, a 50 ms kitty tap moving exactly one cell either way against a 0.6 s
  hold sliding to the wall, a full game recorded to a throwaway `HOME` and
  shown by a fresh process, `stty -a` byte-identical either side, and §8.3's
  teardown bytes in order. §8.4's forced pause and §12.1's message were checked
  with `resize:`.

## What G5 settled

- **`eframe` 0.36, and the MSRV is 1.95.** The plan's open decision, taken as
  recommended: pinning to 0.33 to keep 1.88 means tracking a stale `egui` for
  the life of the project, and §3 already says the floor moves with a
  dependency. **The number lives in three places** — `rust-version` in
  `Cargo.toml`, §3, and the `msrv` job in `.github/workflows/ci.yml` — and they
  move together or that job stops checking what it claims to. `eframe` is taken
  with `default-features = false` and `glow`, not the default `wgpu`: the
  front-end draws rectangles, and OpenGL reaches a browser as WebGL2 without a
  WebGPU fallback path.
- **`tui/host.rs` became `src/native.rs`**, and that is a deliberate amendment
  to `EGUI.md`'s target layout, which had a copy under `gui/`. See the
  invariant above.
- **`eframe` 0.36 splits its callback in two, and the split is §15.2's.**
  `App::logic` is steps 1-4 and 6-7; `App::ui` is step 5. `logic` is called
  while the window is *hidden* and `ui` is not, so a hidden window keeps playing
  and simply is not drawn — G6's backgrounded tab, arriving three stages early,
  and §15.2 step 4's catch-up cap is what makes that safe. A `FrameState` is
  carried between the two halves so they agree about the moment.
- **The GUI is unconditionally §8.2's enhanced case**, so `HOLD_TIMEOUT` and
  `RESTART_QUIET` are unreachable from it and `--legacy` has no meaning there.
- **Focus loss synthesises releases**, because the release of a key let go
  outside the window never arrives and a held direction would keep charging DAS.
  `gui::keys::Keyboard` remembers what it reported as held; F5 makes the stream
  synthesisable precisely so a front-end may do this. At G5 it did not pause
  the game; **G7 reversed that** (`GUI.md` §G4.7, and the invariant above), and
  the releases still come first.
- **A tap shorter than a frame is lost, in both front-ends.** Press and release
  of a movement key inside one pump cancel before any tick consumes the pending
  cell (`KeyTimer::release` resets `initial`). This is pre-existing and shared —
  the terminal loop drains every waiting event before `advance` too — and it is
  why §17.3's A4 measures a *50 ms* tap. A synthetic instantaneous key-down/up
  is not a human tap, and a test that sends one will conclude movement is
  broken when it is not.
- **`egui` reports a key, not a character**, so the adapter decides a *letter's*
  case from the shift modifier — and the **punctuation needs an explicit
  table**, because `egui::Key::name` spells those out (`"Minus"`, not `"-"`)
  where it gives the letters and digits a single character. §10.1 lets a player
  bind any single character and §6.2 gives the two binaries one file, so the
  invariant is: every `Char(c)` the adapter can produce must be the key
  `parse_key` resolves `c` to. That is a test, and it is what caught this. The
  one gap left is the shifted digits — `egui` has a variant for `?` and `|` but
  none for `!` — recorded in `GUI.md` §G2.1 as accepted. `Event::Text` is
  deliberately unused: §12.6's name-entry rules are `shell::menus`'s.
- **CI grew an `apt-get`.** `eframe` needs X11/Wayland/GL *headers* to compile,
  in both jobs. Nothing in `cargo test` opens a window.

## What G6 settled

- **The web build is `gui` on wasm32, and the difference is said by target,
  not by feature.** A feature cannot tell a desktop from a tab, so `clap`,
  `directories`, `chrono`, `anyhow` and rand's `thread_rng` are declared under
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`, and
  `wasm-bindgen`, `wasm-bindgen-futures`, `web-sys` and `js-sys` under the wasm
  table; `gui` names both sets and each target builds its own half. One
  consequence: a native `make shell` now compiles `getrandom`, so it no longer
  proves the shell has no OS entropy source — `make portable` does, as it was
  always the one that could.
- **A 32-bit target found a determinism bug the whole test suite could not.**
  `SmallRng` is a different generator on wasm32, so seed 42 dealt a different
  game in a tab. The fix is its own commit, and the one core change outside
  G10: see the §9.6 invariant above. **A difference only a 32-bit target has is
  invisible to every test in the tree**, because they all run on a 64-bit host;
  a `const` assertion evaluated by `make portable` is where such a guard has to
  live. It was checked in a real tab against `tools/drive.py`'s next queue, and
  under Node against a throwaway wasm32 build of the core.
- **`gui::host` is whichever of `host_native` and `host_web` the target has**,
  so `app.rs` names neither. The one platform difference `app.rs` does see is a
  fact rather than a choice, and it asks `eframe::Frame::is_web()` for it: a
  tab cannot close itself, so leaving a game there starts a fresh one.
- **A tab's run has no end.** No `finish`, no §6.2 default write, nothing handed
  back: the store and the session are leaked once per page load. §16's warnings
  therefore go to the console *as they arise* — `Session::warnings()` is the
  read-only list `gui/app.rs` reports from, through `host::report` (a no-op
  natively, where `main` still prints them at exit).
- **Two crates the plan named are not taken.** `console_error_panic_hook`,
  because `eframe::WebRunner::new` installs a hook that logs message and stack —
  the runner is made *first* in the wasm `main` for that reason. And `web-time`,
  because F1 is `performance.now()` through `web-sys` directly.
- **The canvas takes the keyboard on load, and says so when it has not.**
  `eframe` gives it a `tabindex` but never focuses it, and it only calls
  `preventDefault` on `Space`, `Tab` and the arrows while it has focus. "Click
  to play" is drawn in both builds when `egui` reports no focus; it pauses
  nothing.
- **A hidden tab creeps; it does not stop.** The plan expected
  `requestAnimationFrame` to stop, and it does, but `eframe` then drives
  `logic` from a throttled timer, so each call plays `MAX_CATCH_UP_TICKS` and
  discards the rest. Measured: 36 s hidden advanced a level-1 game ~9 s, with
  no burst on return. `GUI.md` §G8.7 records it as input for G7.
- **Checking the web build needs a visible browser window.** A tab behind other
  windows reports `document.hidden` and is never painted, whatever a screenshot
  shows. Two more traps from the same session: a scripted key press is a
  key-down and key-up inside one frame, so a movement tap is lost exactly as
  G5 recorded — send held keys as `KeyboardEvent`s with a real gap (80 ms
  moves one cell); and **`trunk serve` kept the same hashed file name across
  rebuilds** and serves no `Cache-Control`, so after it rebuilds a plain reload
  can run the old wasm. Hard-reload.
- **CI builds the artefact in a job of its own**, with trunk 0.21.14 fetched as
  a release binary. `make check` gained `web-check` — clippy for the web
  front-end on wasm32, because `--all-features` on the host never compiles
  `host_web.rs` — and does not run trunk, so a developer needs only the target.

## What G8 settled

- **§12.6's boxes are `gui/overlays.rs`, and what they *do* is still
  `shell/menus.rs`.** The window walks the same menus, the same settings and
  the same name-entry rules as the terminal; only the boxes are new.
  `overlays::rect_of` holds every box's size, so "does it fit in the block?" is
  a question a test can ask without drawing.
- **Name entry is not an `egui::TextEdit`**, deliberately: §12.6's twelve
  printable ASCII and the `ANON` default are §14's rules, not a widget's, and
  keeping the field neutral is also what keeps the soft-keyboard question shut
  (§1.2).
- **No non-ASCII in a box.** `▸` and `←→` are terminal glyphs; a window's fonts
  may not carry them. The menu cursor is a drawn triangle and the hints are
  words.
- **The countdown is a numeral over the board, not a box**, because §9.17 is
  for reading the board — and it is the one overlay that does not dim it.
- **Two more shared answers left `tui/`**: `menus::controls` (§10.1's words and
  §13.3's gating rule) and `figures::pps`.
- **The window-to-terminal handoff is proven by a test over the real file
  store**, not by a person at a window: the session that built it could not
  drive the native GUI. `native.rs` has the test. The acceptance check itself
  wanted a human once, and **it has had one** — see the end of "What G10
  settled".

## What G7 settled

- **The playing screen is 26 x 24 cells** (`GUI.md` §G3.2): a one-cell margin,
  a six-cell column, a gutter, the ten-cell well, a gutter, a six-cell column, a
  margin; the mouth, twenty rows, two status rows, a margin. A next panel of one
  slot is the hold panel's shape, and six fit beside the well exactly.
- **Pixels changed three of §12.4's decisions, in the spec.** The score is
  grouped live, combo and back-to-back are figures in the stats panel, and a
  preview is centred by the cells it occupies. `TUI.md` is unchanged; `GUI.md`
  §G4.4 says why each differs.
- **Shared answers moved out of `tui/`**: `shell::figures` (`thousands`,
  `clock`), `Debug::figures` and `Fps` (`shell/round.rs`), `Overlay::blanks`
  (`shell/menus.rs`). The terminal's output is byte-for-byte what it was — its
  mock-up tests say so.
- **A headless `egui` test must reuse one `Context`**, and must call
  `FullOutput::drop_without_applying_deltas` on a pass nobody renders. The first
  costs a minute if forgotten; the second is a debug-assertion panic.
- **Checking the web build without a visible Chrome**: a headless Chrome driven
  over the DevTools protocol paints, because its page is never occluded — but
  an emulated device scale factor leaves the canvas a 1× buffer under a 2×
  `egui`, so the page looks half-size. Use a scale factor of 1. The native
  window has not been looked at by the session that built G7 (no screen
  capture); it runs the same drawing code.

---

## What G10 settled

- **The core grew one field, and only one.** `GameView::fall_progress`, derived
  in `Game::view` from state §9.9 already kept. The I1 snapshot and the §19.4
  canary did not move, which is the assertion rather than a convenience: a
  snapshot that shifted would have meant the field was computed from the wrong
  thing, or that a rule had started reading it.
- **The plan's reasoning about landed pieces was wrong, and the spec is amended
  for it.** See the invariant above; it is the one thing in this stage that a
  test suite would not have caught, because nothing else in the tree looks at
  the number. §9.9 now says the reset on a blocked step is not enough on its
  own, and §12.7 states the requirement.
- **`compose` no longer holds the falling piece.** A grid of cells cannot say
  "half a row down", so the piece is drawn after the grid, and §12.5's flash and
  wipe became `washed`, a per-cell function both paths call. At rest the
  composition is byte-identical to G9's — the shape sweep and the animation
  tests say so.
- **No clipping was needed.** The offset is non-zero only when the piece can
  move down, so the cells below it are empty by construction and a sliding piece
  can never overlap the stack, the floor or the walls.
- **The pop-in the plan told us to accept was not acceptable after all.** §9.4
  spawns every piece but `I` with a mino above the field, `GameView` clipped it,
  and a `J` was drawn as three minos and then abruptly four. Invisible while the
  piece stepped; obvious against smooth neighbours. The fix is the signed row in
  the invariant above, and it is small because the §19 objection did not apply
  to the half that mattered — those minos are the player's own piece, not the
  hidden stack. `GUI.md` §G6.6.
- **`--virtual-time-budget` is not a way to screenshot this.** It advances
  `performance.now()` without running the frames, so every capture came back
  with the piece at spawn and the clock at `00:00`. What works is the G7 recipe
  — headless Chrome over the DevTools protocol, scale factor 1 — screenshotted
  on a real timer; `--enable-unsafe-swiftshader` is needed for WebGL, and
  `--disable-gpu` gets §G8.5's "could not start" message instead. Measured
  there: 13 pixels per 500 ms at a 26-pixel cell, then dead still for the lock
  delay.
- **The native window has now been looked at, by a person**, after G10 — the
  first time any session's work on it has been seen running. The slide and the
  entering piece are right there too, which is what one would expect from shared
  drawing code but had never actually been confirmed.
- **§G8's window-to-terminal high-score handoff has been checked too**, by the
  same person and at the same time: a top out in `ftm-gui` reaches `ftm`'s
  attract screen. That was the last thing in this front-end held only by a test
  standing in for a human, and it is not outstanding any more. §6.2's one config
  file and §14's one table between the two binaries (`src/native.rs`) have now
  been seen working end to end, not just asserted.

---

## What G11 settled

- **§13's words left `tui/`, and that was the stage's real work.** The state
  machine was already shared; the letterforms, their colours, the reminders and
  §13.4's numbers were not. See the invariant above. The terminal's §13.2 art
  test is untouched and now proves the derivation rather than a literal.
- **A tab has no QUIT.** `Session::menu` beside `Session::settings`, and the
  web build sets `MenuChoice::NO_QUIT`. The rejected alternative was reloading
  the page: it is not what the word means, and it would throw away the run's
  §16 warnings and its in-memory table. The quit *key* still works and comes
  back to the attract screen.
- **Both screens are laid out in §G3's grid**, so the window has one too-small
  threshold rather than two and ending a game resizes nothing. §G7.1.
- **§13.4's "painted over it, opaquely" is a character grid's rule**, and §G7.3
  says what it means in a window: the drift is drawn first and everything else
  is opaque on top of it. Nothing is blanked to make room.
- **`rand::make_rng` is not available to the window's drift**, because it is
  the OS entropy source and wasm32 has none. F3's `host::seed` is the
  capability that already exists for exactly this, and the generator is
  `Xoshiro256PlusPlus` — reaching for `SmallRng` here would have been reaching
  for the one that is a different generator on a 32-bit target.
- **A canvas that is not the visible tab is never painted**, so the attract
  screen came up blank on first load and appeared the moment a key was pressed.
  That is G6's recorded trap, not a bug: `App::logic` runs and `App::ui` does
  not. Worth remembering before debugging a renderer that works. It bit twice:
  the second time the tab would not paint at all, and the fix that needed
  looking at had to be checked by a person at the native window instead.
- **The native window's attract screen has been seen, by a person**, and it
  found two things a test had not: the menu sat 0.4 of a cell right of centre,
  and a panel face shorter than four rows sat against the top of the panel
  where the terminal centres it. Both are fixed, and both now have a test that
  measures where the text actually landed — which is the shape this front-end's
  layout regressions want, since nothing else in the tree can see them.

---

## Open decisions

- **The legacy key path's feel (§8.2).** Measured over two seconds of holding
  left: enhanced moves at 0 ms then every 33 ms from 166 ms; legacy moves at
  0 ms, stalls until the OS's first auto-repeat (~500 ms on macOS defaults),
  then runs identically from ~670 ms. Releasing a held direction overshoots by
  two or three cells. Both are inherent to a 90 ms hold timeout, not tuning.
  Whether §8.2 should say so is undecided — ask before amending it.
- **`RESTART_QUIET` is a second constant of the same shape**, and it is 700 ms
  rather than 90 for exactly the reason above: §10.1's restart has to survive
  the OS's *first* auto-repeat, and a soft drop does not. If §8.2 is ever
  amended to describe the legacy path's feel, that number belongs in the same
  paragraph rather than in `tui/run.rs` on its own.

## Commands

```
make check           # everything CI runs but trunk: fmt, clippy, test, shell,
                     # portable, web-check, release build
cargo check          # fast feedback
cargo test --all-features      # unit + integration
cargo test --all-features --test pump   # the shell, pumped headlessly (G4)
cargo check --no-default-features   # core + shell alone: the G2 boundary
make portable        # ...and with no platform under them: the G3 boundary
                     # (needs `rustup target add wasm32-unknown-unknown`)
cargo clippy --all-features --all-targets -- -D warnings
cargo fmt --check
cargo run --release  # play it (`default-run` picks `ftm` of the two bins)
make run-gui         # the window (`cargo run --release --features gui --bin ftm-gui`)
make run-web         # the same in a tab: `trunk serve`, http://127.0.0.1:8080/?seed=42
make web             # the web artefact, `trunk build --release` into dist/ (G6)
make web-check       # clippy for the web front-end on wasm32 (G6)
cargo run -- --print-config    # effective config
cargo run -- --seed 42         # deterministic run, not recorded to high scores
tools/drive.py c c             # drive the release binary on a pty
tools/drive.py --arg=--seed=42 --arg=--config=/tmp/t.toml esc down down enter
tools/drive.py enter resize:20x50   # §8.4 and §12.1, without a window to drag
```

`HOME=/tmp/somewhere tools/drive.py ...` redirects both the §6.2 config path
and the §14 data path, which is how a full game can be played to a top out and
its score checked without touching the real files. A `--seed` run is never
recorded (§14) and so needs no such care.

`tests/pump.rs` is the shell driven headlessly — §15.2's steps called the way a
front-end calls them, with no screen and no clock. It is where cadence
invariance, the catch-up cap, §7's phases and `deadline`'s bounds are checked,
and it is the test a fourth front-end inherits for free.

`tools/drive.py` is the only way to check the terminal layer without a human at
a terminal: §17.1 is "core, no terminal" by design, and what `cargo test` does
reach of `tui/` it reaches through a `TestBackend` — nothing in it opens a real
terminal, or touches `src/bin/ftm.rs`, `tui/term.rs` or `tui/run.rs`'s loops.
It drives the **release** binary on a pty,
sends a scripted burst of keys and replays the capture into a character grid,
printing one frame per keystroke. Two things to know: pass binary arguments through
`--arg`, glued on with `=` both times (`--arg=--seed=42`, `--arg=--config=...`)
or `argparse` claims them — without a seed every run is a different game and
only frames *within* one run may be compared; and it answers the §8.2
capability queries, so pass `--legacy` to exercise the fallback path. A
"key" of the form `resize:ROWSxCOLS` is not a key: it resizes the pty and
signals the child, which is how §8.4 and §12.1 are reached. Its
docstring has the rest. It substitutes for, but
does not replace, playing the game on a real terminal.
