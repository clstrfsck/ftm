# Falling Tetromino Manager (ftm)

A guideline-conformant falling-block game in Rust. No server, no unsafe. The
name is the joke; `ftm` is the binary, the crate, and the config and data
directories. The specification is `FTM.md`, and since G0 it has companion
documents — four of them since P1 — see **The five documents** below. All eight
documents, the three plans included, live in **`spec/`**.

It is no longer only a terminal game, and no longer a single binary: `EGUI-PLAN.md`
builds a second and third front-end (a native egui window, and the same code as
wasm in a browser) and anticipates a fourth (Macroquad). As of G8 the tree is
`core/` + `shell/` + `native.rs` + `tui/` + `gui/` — and since P2 `pilot/`
beside them, which is not a front-end and not a layer — with `src/bin/ftm.rs` and
`src/bin/ftm-gui.rs` behind `--features gui`, and since P5 `src/bin/ftm-pilot.rs`
behind `--features bench`, which renders nothing; **the window opens and plays** —
`make run-gui` — and **so does a browser tab** — `make run-web`. G0 changed no
code; G1 moved the key vocabulary out of crossterm's hands; G2 was a move, a
rename and a feature gate, with no logic changed; G3 took the platform out from
under the shell; G4 turned §15.2's loop inside out, so the shell is now *pumped*
by a front-end rather than owning a `while`; G5 hung an `eframe` application on
the pump; G6 compiled the same application for wasm and gave it a browser's
four capabilities; G7 gave it the playing screen, G8 the boxes over it, G9
§12.5's animations under them, G10 the one core change the whole plan has —
a falling piece is drawn between two rows, and enters the well rather than
appearing in it — G11 §13's attract screen beside the game, which is what
turned §13's *words* from the terminal's into everyone's, G12 the config
and the command line, which is what made the two binaries safe to run over one
file, and G13 the tests and the sign-off.

**Status: both plans are finished. Stage 12 of `TERMINAL-PLAN.md` complete —
milestone M4, accepted; `EGUI-PLAN.md` stages G0-G13 complete — milestone MG7,
accepted, with B1-B12 signed off one by one in `GUI.md` §G9.3.** All
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
cannot close itself. **`GUI.md` is whole**: §G1-§G9 are written and normative,
and nothing in it is reserved any more.

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
borrowed bundle), and since G4 `session.rs` (`Session`, `Next`, and since P4
§P1's `Player`) and `round.rs` (`Round`, `FrameState`, `Debug`, and the `App`
inside them, which since P4 holds the `pilot::Pilot` when there is one).
`src/native.rs` is
the *desktop* — F1-F4 over `std::fs`, `directories`, `chrono` and `rand` — with
`src/argv.rs` beside it for the capability that is none of those four, an argv:
§6.4's two enumerated value spellings, which both native binaries share.
Both native front-ends take both. `tui/` is the terminal front-end: `keys.rs` (the
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
has, so `app.rs` never asks. `pilot/` is the automated player and is none of
the above — not a front-end, not a layer, and a sibling of `core/` and
`shell/`: `knowledge.rs` (§P2.4's bag counting), `fork.rs` (§P2.3's seam from
above) and `eval.rs` (§P5's features and weights), with `core/search.rs`
underneath it, and — since P3 — `placement.rs` (§P4.1's generator) and
`controller.rs` (§P3.4's `Pilot`, the plan and §P6.4's choice), which are the
two private modules in a directory that is otherwise public, and — since P5 —
`bench.rs` (§P8.2's report, played and counted with no clock in it), whose
other half is `src/bench.rs` beside `native.rs` and `argv.rs`.
T1-T17 all pass, plus I1-I4, `tests/pump.rs` and
`tests/gui_render.rs` — the window's I4, which is `tests/render_sizes.rs`'s
counterpart in pixels (§G9.1) — and the batch-invariance canary is in CI.

There is no Stage 13 of `TERMINAL-PLAN.md` and no Stage G14 of `EGUI-PLAN.md`:
**both plans are finished**, and there is no live plan. Read them for why
something is the shape it is, not for what to do next. A fourth front-end is a
fourth directory, a feature and a `[[bin]]`, and `FRONTEND.md` is what it is
written against — but it is not planned, and **Macroquad readiness** in
`EGUI-PLAN.md` is a list of constraints rather than a work item, exactly as §19
is.

**There is a live plan again: `PILOT-PLAN.md`, stages P0-P8**, which builds the
automated player of `PILOT.md` §P1-§P9 — a **PILOT** row on §13.3's menu that
plays the game while the player watches. It is a deliberate amendment to the
scope rule below: it promotes §18's first bullet and no other. Stages P1 to P5
are complete. P1 was the specification and the amendments to `FTM.md`,
`TUI.md` and `GUI.md`; P2 is the first code — §P2.3's fork in `core/search.rs`,
and **`src/pilot/`, a fourth directory beside `core/`, `shell/` and the two
front-ends**, holding §P2.4's bag observation, §P2.3's `Fork` and §P5's
features and weights. It is in `make shell` and `make portable` for exactly the
reason `shell/` is. **P3 is a player**: `pilot::Pilot` and `Settings` (§P3.4),
`placement.rs`'s §P4.1 generator and the one-ply choice with §P6.4's
tie-breaks. It plays a full game headlessly and plays it *well* — 20,000 pieces
on each of five seeds with no top out.

**P4 is the mode**, and it is offered now: a **PILOT** row under **PLAY** on
§13.3's menu in both native front-ends, `Next::Play(Player)` so a restart keeps
it, the controller beside the input state in `shell/round.rs`, §P3.1's per-tick
input path inside `App::advance`, §P7.3's spectator controls and cyan
indicator, and §P7.4's suppression of every path to §14's table. `cargo run
--release` and `make run-gui`, then **PILOT**: it clears fourteen lines in the
first six seconds on either screen.

**P5 is the instrument**: `ftm-pilot`, a third binary behind a `bench` feature
that plays batches headlessly and prints §P8.2's report — `make bench`. It is
two files, split at §P3.3's line: `pilot/bench.rs` plays and counts with no
clock, `src/bench.rs` holds §P8.1's grammar and the wall clock. The baseline it
recorded is in `PILOT-PLAN.md` P5 — 8 seeds × 2,000 pieces, no top out, 145 µs a
piece — and so is the node budget it was written to measure: **~2,000 nodes per
search**, which P6 is where `Settings::default().nodes` stops being 100,000.
What is left is the search of P6-P7 and P8's acceptance. See **Scope
discipline**, which says what is still out.

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

They are all in `spec/`, and everything below names them the way the source
does — `GUI.md §G7`, not `spec/GUI.md §G7`. A document is referred to by its
name because that is what several hundred doc comments say; the directory is
where it is kept, not what it is called.

1. **The specification**, in whichever of the five documents below owns the
   sections your work touches. Read those sections, not the whole thing. Start
   from `FRONTEND.md` if the work is a front-end's, and from `PILOT.md` if it
   is the automated player's.
2. **`PILOT-PLAN.md`** — stages P0-P8, the only live plan. P1 and P2 are done,
   and each has a "What it settled" of its own; read "The decisions this plan
   rests on" before writing any of the rest.
3. **`EGUI-PLAN.md`** — stages G0-G13, all complete. History now, not
   instructions, but the two sections that are *not* history are "The decisions
   this plan rests on" and "The central idea", which is what the shell being
   pumped rather than looping follows from. Its **Macroquad readiness** is a
   list of constraints, not a plan.
4. **`TERMINAL-PLAN.md`** — the twelve stages that built v1.0. History too.
   Worth reading when you want to know why something is the shape it is.

The specification is ground truth. **If the code and the spec disagree, the
spec is wrong until it is amended** — fix it in the same commit and say so in
the message. Never let the code silently diverge.

## The five documents, and the number-stability rule

G0 split the specification in four; P1 added a fifth. **Section numbers did not
move, and never do.** The
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
| **`GUI.md`** | §G1-§G9 | The egui front-end, native and web. Complete: §G1 (the application, the version pin, the loop) and §G2 (input) by G5, §G8's web build by G6, §G3 and §G4 by G7, §G5 by G8, §G6.1-§G6.4 by G9, §G6.5-§G6.6 by G10, §G7 by G11, §G8.10-§G8.11 by G12 and §G9 — the render test and B1-B12 — by G13. |
| **`PILOT.md`** | §P1-§P9 | The automated player: the mode, the information boundary, control and the input cap, placements, evaluation, search, the two screens, the headless benchmark, and C1-C13. Normative since P1; **no code yet**. |

An unqualified `§n` means `FTM.md` §n, except for the eleven numbers `TUI.md`
owns. `§Gn` means `GUI.md` and `§Pn` `PILOT.md`; a future `MACROQUAD.md` would
take `§M`. **A stage and a section share a letter and are told apart by the
`§`** — `§P4` is a section of `PILOT.md`, P4 is the stage of `PILOT-PLAN.md`
that makes something play, exactly as `§G3` and G3 already differ.

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
  same list** (§13.3, §13.5, `GUI.md` §G5.4, §G7.5, §G8.11). There are two of
  these. `Session::settings` is the §13.5 panel's — `Setting::ALL` for the
  terminal (the shared seven plus §12.3's colour depth), `Setting::WINDOW` for
  the native window (plus §G8.10's scale and full screen) and
  `Setting::CANVAS` for a tab (plus the scale alone, since a canvas cannot go
  full screen) — and `Session::menu` is §13.3's — `MenuChoice::ALL`, or
  `MenuChoice::NO_QUIT` in a browser tab, which cannot close itself.
  `Setting::SHARED` is the intersection the other three are built from and is
  what a fourth front-end starts from; nothing sets it directly. Drawing a
  different list from the one the shell walks puts the cursor on a row nobody
  can see.

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
  `keyboard` settles the countdown first so that pump plays no tick. The
  terminal does not call it.

- **"Have I the keyboard?" is two questions in a window, and `egui` answers
  only one** (`GUI.md` §G4.7, §G8.7). `gui/app.rs` ANDs `input.focused` with
  `host::visible()` — `!document.hidden` in a tab, a constant `true`
  natively — and **dropping the second half is a silent bug, not a compile
  error**. A hidden tab keeps the canvas's DOM focus and fires no `blur`, so
  `egui` reports a focused application that hears nothing, and the game plays
  on behind it: measured at G13, 13 seconds of play in 31 seconds hidden,
  locking pieces nobody placed. Natively the two agree, which is why the
  constant is not a stub — a window that is hidden, minimised or behind another
  has genuinely lost the focus and `egui` says so.

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
  lives in `tui/cli.rs` and `gui/cli.rs`; what crosses into `shell/config.rs` is
  `Overrides`, which `gui/query.rs` fills in from a URL just as well. A flag
  exists in a build when the setting it names does — `--color` is the
  terminal's, `--scale` and `--fullscreen` the window's, `--print-config`
  shared — and `GUI.md` §G8.11's table is the list, held by tests in both
  directions. `ColorDepth` and `LockDownRule` have hand-written `ValueEnum`
  impls rather than derives on the types, because a derive would put `clap` in
  `shell/config.rs`; since G12 they are in **`src/argv.rs`**, not beside the
  flags, because both native binaries take `--lock-down` and an impl behind
  `feature = "tui"` is not there for a `--features gui` build. `argv.rs` is
  `native.rs`'s neighbour and is gated the same way: an argv is a desktop
  capability.

- **Every binary holds every table, and only one acts on each** (§6.2,
  `GUI.md` §G8.10, `TUI.md` §6.3). `ConfigFile` has `[gui]` in the terminal
  build and `[display]`'s four glyph and colour keys in the window build,
  because `config::document` rewrites the whole file and a table the struct has
  no field for is a table the next save silently deletes. They are parsed,
  clamped and *warned about* by both — §6.2's warning is about the file and the
  player edits one file. Preserving unknown tables generically is the rejected
  alternative: §6.3's loader is value-by-value precisely so it can warn, and a
  table it does not understand is one it cannot warn about. A fourth
  front-end's table joins the struct.

- **`remember_window` writes only when something moved** (`GUI.md` §G8.10).
  `gui::host_native::remember` records the window's geometry into the session
  every pump; `keep_window` copies it onto `startup.on_disk` at exit and
  answers whether to save. Two things it deliberately does not do: it does not
  remember `fullscreen`, because that is a *choice* and `--fullscreen` is a
  flag §6.1 never writes back; and it does not save an unchanged file, because
  that would rewrite the player's config every run and add §16's warning to
  every run over a read-only one.

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

These five are `PILOT.md`'s. The first three have code behind them — the first
two since P2, the third since P3 — and are held the way the rest of this list is:
by a type, by `make portable`, and now by a debug assertion that runs on every
tick of every planned game in the test suite. The fourth and fifth are still
written-down-only: P4 has to keep them.

- **The planner never holds a `Game`** (`PILOT.md` §P2). A clone carries the
  real bag and the real generator, and `Game::bag_remaining` is an accessor onto
  hidden information, so fairness cannot be a convention. It holds a
  `SearchGame`: a fork whose randomiser has been **replaced** by a scripted
  queue the planner built from what it observed, and which refuses to spawn past
  the end of it. The hidden-future test is belt and braces over a property the
  types already hold — the same trade A10 made for the core's façade. And
  `core/mod.rs` re-exports the seam with `pub(crate) use`, so **A10 itself does
  not move**: the core's public surface is what it was. The replacement is in
  `core/bag.rs`, one level below `Game`: a `Bag` is a generator-and-its-bag *or*
  a list, so there is no randomiser in a fork that has merely been promised not
  to ask. §P2.3's method list is an **upper bound**, and each accessor lands
  with the stage that reads it. `pilot::Fork` is the same object seen from
  above, and exists because a crate-private type cannot appear in a public
  signature — which is also where the *queue* arrives, the half of fairness no
  type can hold.
- **The benchmark is split at the no-clock line, and that is what makes it two
  files** (`PILOT.md` §P8). `pilot/bench.rs` plays the batch and folds it into
  integers — no clock, `make portable` compiles it — and `src/bench.rs` holds
  §P8.1's grammar and the wall clock, beside `src/argv.rs` and behind a `bench`
  feature that is neither front-end's. The report has the same seam in it: the
  header, the per-seed rows and the aggregate are integers and reproduce byte
  for byte, and the throughput is a **trailing block headed as this machine's**
  and excluded from that promise. Folding a duration into a row would make the
  baseline undiffable, and timing the games from inside `pilot/` would put a
  clock in the one directory that may not have one.
- **The planner takes no clock, and that is a third instance of the house
  rule** (`PILOT.md` §P3.3). The core takes no clock, the shell takes no clock,
  and now the player does not either: no `Instant`, no frame count, no wall-time
  budget, and a search that is **never amortised across frames**. `src/pilot/`
  is in `make shell` and `make portable` for exactly the reason `shell/` is —
  since P2, and the Makefile says so. §P5's "integers only" is the same rule
  about a different quantity, and for §9.9's reason: the features, the weights
  and the evaluation are all `i32`, so a plan is identical on every target and
  in every profile. The property this buys is cadence invariance for a PILOT
  round, which is §19.4's canary one layer up and is asserted in
  `tests/pump.rs`.
- **The planner thinks once per piece, on the tick it spawns** (`PILOT.md`
  §P3.1), and emits one `TickInput` per tick until it locks. Since P3 that is
  `Pilot::input`: the plan is made on the first tick at which a piece is in play
  and no plan is running, which is exactly once per piece — a hold raises
  `PieceSpawned` too, and a planner that re-planned on the event would tear up a
  plan it was halfway through. Two things follow. `App::advance` gives a batch's
  edge actions to its *first* tick only, because a human produces input per
  frame; a planned round needs the batch's *n*th input on its *n*th tick, so
  since P4 that is a **branch inside `advance`** — four lines, with the human
  path byte-identical, which the terminal's mock-ups and `tests/pump.rs` are
  what say. `observe` is handed the slice of the frame's event buffer that tick
  appended, not the whole of it: the buffer belongs to the frame, because §12.5
  absorbs it once. And a plan is never recomputed from a partly executed state —
  a plan that diverges from what the search predicted is a bug, not a resync,
  and `Pilot::verify` is the debug assertion that says so tick by tick.
- **A plan ends at the tick that locks the piece, and the fork is what knows
  which tick that is** (`PILOT.md` §P3.1, §P4.1). The generator replays every
  candidate through the real rules rather than computing where a piece would
  land, so a kick, a wall and a gravity lock that beats the hard drop are all
  ordinary. The sequence is **truncated** at the lock: an input after it is one
  the next piece would receive. Two things the replay bought, which is why it is
  not a detail — the features are measured after §9.12's clear delay, because a
  board measured during it still holds the rows it is about to lose and a
  planner that scored it would never clear a line; and the divergence assertion
  can only compare as far as the fork's queue reached, because a hold at a
  `preview_count` of 1 uses it up and what the live game deals next was hidden
  when the plan was made.
- **A screen offers what its front-end can do, now for a third list**
  (`PILOT.md` §P1, §P7.1). `MenuChoice::ALL` is six items with PILOT;
  `MenuChoice::CANVAS` — renamed from `NO_QUIT` — is a tab's four, short of QUIT
  because a tab cannot close itself and short of PILOT because a tab would
  search on its frame thread. Nothing is `cfg`-ed out. The room it takes was
  *measured* before it was specified: the terminal's attract block goes 21 → 22
  rows or the footer is silently clipped, and the window needs no change at all
  because its menu band is reserved rather than fitted (`PILOT.md` §P7.2).
  Since P4 there is a third list *inside* the second: §13.3's panel cycle pauses
  on any item but the two that **start a game**, and which indices those are is
  the front-end's list's answer — so `shell::attract::Attract` remembers the
  `MenuChoice` the cursor is on and not only where it is.

- **A spectator's keys are dropped at the input boundary** (`PILOT.md` §P7.3,
  §10.1). While the pilot plays, §10.1's movement, rotation, hold and drop keys
  do nothing — and "do nothing" means `play_key` returns before `InputState`
  sees them, exactly where a disabled mechanic's key is dropped. Ignoring what
  they produce instead would leave a held direction charging DAS and a press
  resetting a lock-delay timer behind a game nobody is playing. Pause and
  everything the pause menu reaches, §10.1's restart hold, quit and §16's
  Ctrl-C stay live: a spectator can stop, look and leave.

- **The PILOT indicator is the `I`-piece cyan, in both front-ends, for the whole
  game** (`TUI.md` §12.4, `GUI.md` §G4.5, `PILOT.md` §P7.3). Left-aligned on the
  status row, over the centred content rather than joined to it — the longest
  thing that can be there is `PERFECT CLEAR` at thirteen characters, which
  centres well clear of column 5. Deliberately not a transient: a screenshot of
  an automated game must never be mistakable for a player's, and both front-ends
  assert the colour as well as the position, because the first implementation
  had it in the plain text colour and no test would have known.

## Working agreements

- Tests land **with** their stage, not after it. `TERMINAL-PLAN.md` maps every test in §17
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

- §18 (other modes, sound, replays, themes, per-piece statistics) and §19
  (networking) are **not work items**. §19 is a list of constraints to honour,
  already baked into Stages 3, 5 and 8. The only networking deliverable in the
  entire plan is that one CI test.
- **One §18 bullet was taken up, and only one**: the self-playing bot, now
  `PILOT.md` and `PILOT-PLAN.md`. It is a menu item the player chooses, not
  §13's idle demo — the attract screen still never starts playing by itself,
  which is the part of that bullet that stays out. Everything else in §18 is
  exactly as out of scope as it was, and this is not a precedent: the bullet was
  promoted by a decision, written down in `PILOT-PLAN.md`'s scope section and in
  §18 itself, rather than by a session deciding it was in.
- **`PILOT-PLAN.md`'s scope is its stages.** An automated player makes weight
  training, a replay format, an idle demo and a difficulty setting all feel
  newly reachable. None is in it. Automated weight tuning in particular is
  named as *not* in the plan, and would consume §P8's benchmark rather than
  replace it.
- The attract screen (§13) is explicitly provisional. It has now been built
  plainly and looked at; iterate on it if it wants it, but §17.3 never judged
  its looks and the plan is finished either way.
- **`EGUI-PLAN.md`'s scope is its stages and nothing beside them.** A window, and
  especially a browser tab, makes sound, themes, mouse and touch input feel
  newly reachable. §1.2 is unchanged: the game is keyboard-driven. Touch got a
  real answer rather than a reflex, before the web slice: **no touch controls**,
  and the page says so (§1.2, `GUI.md` §G8.8). Adding them is a §1.2 amendment.
- **Macroquad is `EGUI-PLAN.md`'s §19**: a list of constraints so the front-end stays
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
  to `EGUI-PLAN.md`'s target layout, which had a copy under `gui/`. See the
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
  store**, not by a person at a window: the session that built it believed it
  could not drive the native GUI. `native.rs` has the test. The acceptance
  check itself wanted a human once, and **it has had one** — see the end of
  "What G10 settled". (It need not have: the window *can* be scripted, which
  G12 worked out — see **Commands**. The test is still the right one, because
  what it checks is two binaries over one file store rather than a screen.)

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

## What G12 settled

- **The data-loss bug the stage exists for was real and is now a test.** A GUI
  run that saved the config would have erased `[display]`, because `document`
  rewrites the whole file. Both halves are in `ConfigFile` now — see the
  invariant above — and the round trip is compared **byte for byte within each
  table**, in both directions: a save that reflowed the other front-end's half
  would be the same loss one step removed.
- **`[gui]` is nine keys, four of them start-up answers.** `GUI.md` §G8.10's
  table says which take effect when, because it is not guessable: `vsync` is
  chosen when the GL surface is made and so can never be an Options row, while
  `scale_percent` and `fullscreen` are rows and apply the moment the panel is
  left. `frame_cap` is a floor under §15.2 step 6's deadline — it caps
  *drawing*, and the game plays several ticks per repaint and stays the same
  game, which is `tests/pump.rs`'s cadence invariance reached from a setting.
- **`vsync` lives on `glow_options`, not on `NativeOptions`**, in `eframe`
  0.36 — the renderer's rather than the window's. It is set by assignment
  rather than in the struct literal, because naming its type means naming
  `egui_glow`, which §3's dependency table does not list.
- **`clap`'s `ValueEnum` impls had to leave `tui/cli.rs`**, and `src/argv.rs`
  is where they went. Both native binaries take `--lock-down`; an impl behind
  `feature = "tui"` simply is not there for a `--features gui` build, and the
  failure is a compile error in the *other* front-end. See the amended
  invariant.
- **`--print-config` is shared after all.** `gui/cli.rs` carried a comment
  saying a window does not ask the question; the plan said otherwise and the
  plan was right — what it prints is the whole document, and the player
  comparing what two binaries resolved from one file is exactly who wants it.
- **Two §16 warnings named a directory a browser tab has not got**, which
  `GUI.md` §G8.3 had already booked as this stage's to fix. They say *nowhere
  to keep the settings / the scores on this platform* now: §3.1 means the shell
  does not know where the bytes were going, so it should never have said.
- **A URL is edited by hand and the grammar admits it.** A flag parameter takes
  five spellings for "on" and four for "off" (§G8.4); a command line has only
  "written or not written". `?fullscreen` in a tab is accepted and inert rather
  than warned about, because a link shared from a desktop should not scold
  whoever opens it.
- **The native window turned out to be scriptable, and that is the stage's
  other deliverable.** `osascript` sends it keys; see **Commands**. Every claim
  above was checked against the running binaries rather than against tests
  alone — the Options-panel save in both front-ends, the geometry write-back,
  `remember_window = false`, and the two runs at 125 % that caught the scale
  bug. `remember` still has no unit test, because reading `egui`'s viewport
  info needs a window manager; what it has instead is that it has been *run*,
  which for this function is the better of the two.

---

## What G13 settled

- **B8 found a real bug, and it is the reason this stage is not a formality.**
  A backgrounded browser tab did not pause: it went on playing, throttled — 31
  seconds hidden advanced a level-1 game by 13, which is G6's creep measured
  again a year of stages later. §G4.7's *rule* was right; the *mechanism*
  `GUI.md` §G8.7 named for it was wrong. Switching to another tab fires no
  `blur` at the canvas, so it keeps the document's focus and `egui` goes on
  reporting a focused application that cannot hear a key. See the invariant
  below; both sections are amended and `gui::host::visible` is the fix.
- **The lesson is worth more than the fix.** §G4.7 had a test and the test
  passed, because it asked the *shell* whether it pauses when told the keyboard
  is gone. What was broken was the front-end's answer to "is it?" — a question
  no test in the tree was in a position to ask. That is the shape of what is
  left after twelve stages of tests, and it is what B3-B11 are for.
- **`egui_kittest` was not taken**, against the plan. `egui::Context::run_ui`
  with a `RawInput` *is* the headless harness, and it is what every test in
  `src/gui/` already used; the crate's own contribution is an AccessKit tree
  this front-end has no widget hierarchy for (§G1.1 declines `accesskit` for the
  same reason) and image snapshots the plan already excluded from CI. A
  dependency whose only remaining use is excluded is not a dependency. §3's
  table and §G1.1 no longer name it.
- **The density is the axis a character grid has not got.** `tests/gui_render.rs`
  sweeps seven sizes in *physical pixels* at four densities, and the two are not
  interchangeable: §G3's cell is a whole number of pixels, so fourteen points is
  17.5 at 1.25 and the cell must be eighteen. The minimum window is therefore
  468 x 432 pixels there against 364 x 336 at 1x, and **the boundary the test
  asserts is one pixel wide, not one point**. It also asserts that §G3.3's
  message never asks for less than it takes, because a player who resizes to
  what they are told has to get the screen.
- **Two `egui` traps, for anyone writing a headless test here.** `RawInput`'s
  `screen_rect` is divided by the zoom factor, so `set_pixels_per_point` and a
  size are not independent — set the density on the viewport's
  `native_pixels_per_point` instead and `screen_rect` stays the points you
  meant. And the font atlas is 2048 pixels: a countdown numeral is six cells, so
  a viewport of a few thousand *points* at 3x asks for a glyph that will not fit
  and panics inside `epaint`. Sizes in pixels divided by the density keep every
  case a viewport that could exist.
- **CI needed nothing.** Every step this stage was supposed to add was already
  there — `shell` from G2, `portable` from G3, `web-check` and the web job from
  G6, the `apt-get` and the MSRV bump from G5. That is what `make check` being
  the only list CI runs buys: the stage that adds a check adds it there, and the
  acceptance stage finds the bill paid.
- **B4 is a test now, not a stopwatch.** §17.3's A4 measured a 50 ms tap and a
  0.6 s hold on a pty because the terminal's loop was the only way to reach
  them. Since G4 they are reachable from `tests/pump.rs`, and that is what makes
  "identical to the terminal front-end's" a fact rather than a comparison: it is
  the same `shell::input`. Do not try to measure this through a browser — a
  round trip to a tab is seconds, and the piece has moved on.

---

## What P2 settled

- **The randomiser is replaced in `core/bag.rs`, not worked around in `Game`.**
  A `Bag` is now a `Source`: §9.6's generator with the bag it shuffles, or a
  scripted list with neither. `Bag::next_piece` returns an `Option` and the
  `None` is reachable only from a fork, where `Game::spawn` declines to spawn
  and waits in the entry delay. The alternative — a `Game` that keeps its real
  bag and is asked politely not to deal from it — is what §P2.3 means by
  "replaced, not hidden", and it is the difference between fairness being a
  type and fairness being a habit.
- **The I1 snapshot and the §19.4 canary did not move, and that is the
  assertion.** `Bag` was restructured under the whole game; a snapshot that had
  shifted would have meant the live path had changed shape, not that the
  fixture was stale. Read it that way if it ever does move here.
- **A stage lands only the part of a seam that is read.** `#![allow(dead_code)]`
  is gone and the tree means it, so a `pub(crate)` accessor whose only caller is
  a `#[cfg(test)] mod` is a warning in the ordinary build, and `make check` is
  `-D warnings`. §P2.3's method list is amended to say it is an upper bound; P3
  adds the pose and the hold state when its generator reads them. The general
  lesson for the stages after this one: **a seam built a stage before its
  consumer needs a consumer**, and the honest one is usually the public face
  the layer was going to need anyway.
- **`pilot::Fork` is that face, and it is structural.** §P2.3 requires
  `SearchGame` to stay out of the core's façade, and Rust will not let a
  crate-private type appear in a public signature — so `tests/`, §P8's runner
  and anything else outside the crate search through `pilot`'s own wrapper. It
  is also where the scripted queue arrives, which is the half of fairness no
  type can hold: a `Fork` is exactly as fair as the list it was built from.
- **Two bag facts were bugs before they were specification.** The first piece of
  a game is dealt during `Game::new`, which **discards its `PieceSpawned`** — a
  tracker that waited for the event is one piece out for the whole game. And the
  open bag closes on the **seventh** deal, not the eighth; waiting reads
  "nothing can come next" at the one moment all seven can. Both are in §P2.4
  now.
- **A preview can cross a bag boundary**, so a piece on the screen may also be a
  legitimate hypothesis — out of the bag *after* the one it was dealt from. At
  the default preview of 5 this is ordinary, not exotic, and an assertion that
  said otherwise was wrong about §9.6 rather than about the code.
- **The features are §P5's literally, including the two that read backwards.**
  Row transitions count both walls as filled, so an *empty* board scores 40 and
  a full one 0; column transitions count the floor, so an empty board scores 10.
  Both are Dellacherie's formulation and both are meant to be minimised — the
  direction is in the weight, not in the feature. `covered` and `blockades` are
  deliberately two features and not one: a cell above two holes is counted twice
  by the first and once by the second.
- **Test fixtures in `src/pilot/` are held to §P3.4's list too.** The core's
  `from_bottom_rows` is `core::matrix`'s and the planner may not name it, so
  `eval.rs` has its own four-line field fixture; `RulesConfig` is built by field
  rather than through `from_settings`, because the planner may name the one and
  not the other two. A test that reaches past the boundary is testing a module
  allowed to do something the module under test is not.

---

## What P3 settled

- **A placement is played, not predicted**, and that one decision is why the
  generator is ninety lines. Every candidate — hold or not, four orientations,
  thirteen columns — is replayed on a fork through the real rules and measured
  from the board it leaves behind. There is no arithmetic anywhere about where a
  piece lands, so a rotation that kicks at a high stack, a shift that runs into
  a wall and a piece that gravity locks early are all ordinary rather than
  special cases. `PILOT.md`'s "both generators produce input sequences, never
  board positions" is the reason this is the cheap way round and not the
  expensive one.
- **`SearchGame` gained nothing this stage, against the plan's expectation.**
  §P2.3 had the pose and the hold state arriving with P3's generator; a
  generator that replays reads the position from `view()` and wants neither.
  They are §P4.2's — P7 — which will also have to answer how a pose crosses the
  boundary at all, since `ActivePiece` is not in the core's façade and may not
  join it. §P2.3 and the plan are amended.
- **The divergence assertion earned its keep on its first run.** It caught a
  case the design had not: a hold at a `preview_count` of 1 uses the fork's
  queue up, so the tick that locks the piece cannot say what spawns after it —
  the live game deals a piece that was hidden information when the plan was
  made. The prediction now carries whether the fork still knew, and the
  comparison stops exactly there. This is the shape of every fairness question
  in this plan: the answer was to compare *less*, not to give the fork more.
- **It does not play badly, which `PILOT-PLAN.md` P4 says to expect.** One ply
  over §P5's starting weights plays 20,000 pieces on each of five seeds with no
  top out — ~8,000 lines and level 800 apiece. P6's lookahead therefore has a
  harder baseline to beat than the plan assumed, and P5's benchmark is what will
  say whether it beats it.
- **The cost is ~155 µs a piece in release**, about 6,500 *pieces* a second, for
  the 104 candidates one ply generates. (Recorded here as "placements a second",
  which P5 corrected with an instrument: 6,500 is 155 µs's reciprocal and so
  counts pieces, and the placement rate is 104 times it.) A frame is 16 ms, so
  even a full `MAX_CATCH_UP_TICKS` batch of searches is nowhere near it; the
  number worth measuring after P5 is the one after P6's second ply.
- **`cargo test` runs the assertion, and that is the point of it.** Every
  planned game in the suite checks the plan against the fork tick by tick,
  because `cargo test` is a debug build. A release round pays nothing: the
  predictions are not even recorded.

---

## What P4 settled

- **It is offered, and it was watched.** A **PILOT** row under **PLAY** on both
  native menus, and a person looked at a game on each: fourteen lines and level
  two inside six seconds on the terminal at seed 42, the same in the window,
  with the indicator on the status row of both. `PILOT-PLAN.md` P4 said to
  expect it to play badly; P3 had already found otherwise and this is that
  finding with a screen in front of it. The pieces *travel* rather than
  teleport, which is §P3.2's one-cell cap earning its keep as a presentation
  decision and not only an honesty one.
- **The two amendments this stage needed were both the spec catching the
  code.** §13.3's panel cycle pauses on any item but the two that start a game,
  so PILOT had to join PLAY there — the first implementation compared an index
  to zero — and §12.4 and §G4.5 both say the indicator is drawn in the `I`-piece
  cyan, which the first implementation was not. Neither was a test failure; both
  were found by reading the sections the stage was implementing against. That is
  what "the spec is ground truth" buys, and it is why the invariants above now
  carry the colour.
- **`Next::Play(Player)` was the cheap half and the sixth menu item the dear
  one.** Every front-end already matched `Next` exhaustively, so the payload was
  a compiler-guided edit that could not be got wrong. The menu item reached into
  `tui::attract`'s block height (21 → 22, `PILOT.md` §P7.2), both render
  fixtures, and three of the shell's own tests that had counted `Down` presses
  to a row — those now ask `MenuChoice::ALL` where the row is.
- **The assertion that would have caught the silent clipping was already
  there.** §P7.2's hazard is that a menu taller than the block loses the *footer*
  by clipping, with no error — and `the_screen_holds_the_wordmark_the_menu_and_
  the_panel` looks for `ENTER start`. Worth knowing for the next thing that
  grows: the attract screen's tests are what hold its height.
- **Driving the native window is still a focus problem.** Three of five
  scripted runs sent their keys to whatever was in front instead, and
  `osascript` exits `0` either way (`EGUI-PLAN.md` G13 recorded this; it is
  worse than it reads). What works is **one shell invocation** that launches the
  binary, waits, sends the keys and screenshots, with nothing in between — a
  second invocation brings the terminal back to the front and the keys go
  there. `screencapture -x -o` plus `sips -c H W --cropOffset Y X` is still how
  the result is looked at.

---

## What P5 settled

- **There is a baseline now, and a command that reprints it.** `make bench` is
  8 seeds × 2,000 pieces in release: 16,000 pieces, 6,381 lines, **no top out**,
  a mean of 4,083,485 and level 80 at the cap, in 2.33 s — **145 µs a piece**,
  ~715,000 nodes a second. The longer run P3 asserted headlessly is an
  instrument reading now too: `--seeds 5 --pieces 20000` is 100,000 pieces and
  39,989 lines with no top out, in 14.2 s. One ply over §P5's starting weights
  is a real baseline for P6 to beat, not a placeholder to replace.
- **The node budget is ~2,000 per search**, and that was the open decision the
  stage existed to close. A frame affords ~11,900 nodes at the measured rate,
  and §15.2 step 4 may play `MAX_CATCH_UP_TICKS` ticks before it draws, so the
  worst case is six searches in one frame. `Settings::default().nodes` is
  100,000 — fifty times over, harmless while nothing reads it, and P6's first
  correction. It is conservative twice over on purpose: it comes off the *mean*
  per-piece cost, and six spawns in six consecutive ticks cannot happen at all,
  because §9.12's two delays sit between them.
- **The report's two halves are the no-clock rule showing through.** See the
  invariant above. The thing to remember when reading a report: everything above
  the `timing` heading is diffable and everything below it is not, and a run
  that "changed" is a run whose *rows* changed.
- **A figure derived in a commit message is not a figure an instrument
  printed.** P3's "~6,500 placements a second" is 155 µs's reciprocal and
  therefore counts pieces; the placement rate is 104 times larger. Nothing was
  broken by it and no test could have caught it, and it is exactly why §P8.2
  asks for `nodes` and `placements` by name rather than for "throughput".
- **The smoke batch is small because `cargo test` is a debug build**, where
  §P3.1's divergence assertion replays every plan a second time. Three seeds ×
  60 pieces, in `tests/snapshots/pilot_bench.txt`. **It is the opposite of I1**:
  a weight, generator or tie-break change is *supposed* to move it, and moving
  it is a diff to read rather than a bug to chase — which is why it lives in its
  own file and is regenerated the same way (`UPDATE_SNAPSHOT=1`).
- **§4's tree had drifted since P2, and the spec is corrected in this commit.**
  It named `placements.rs`, `evaluate.rs` and a `mod.rs` holding `Pilot`; none
  of the three was ever written. The rule the repo runs on says the spec is
  wrong until amended, and a tree nobody re-reads is where that goes unnoticed.

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
make bench           # PILOT.md §P8's baseline: 8 seeds x 2000 pieces, release.
                     # BENCH_ARGS="--seeds 5 --pieces 20000" for anything else;
                     # `--json` for the same figures with a `timing` key.
cargo run --release  # play it (`default-run` picks `ftm` of the three bins)
make run-gui         # the window (`cargo run --release --features gui --bin ftm-gui`)
make run-web         # the same in a tab: `trunk serve`, http://127.0.0.1:8080/?seed=42
make web             # the web artefact, `trunk build --release` into dist/ (G6)
make web-check       # clippy for the web front-end on wasm32 (G6)
cargo run -- --print-config    # effective config
cargo run -- --seed 42         # deterministic run, not recorded to high scores
tools/drive.py c c             # drive the release binary on a pty
tools/drive.py --arg=--seed=42 --arg=--config=/tmp/t.toml esc down down enter
tools/drive.py enter resize:20x50   # §8.4 and §12.1, without a window to drag

# The native window, driven from a script (macOS). See below.
./target/release/ftm-gui --config /tmp/t.toml &
osascript -e 'tell application "System Events" to keystroke "q"'
osascript -e 'tell application "System Events" to key code 125'   # Down
```

`HOME=/tmp/somewhere tools/drive.py ...` redirects both the §6.2 config path
and the §14 data path, which is how a full game can be played to a top out and
its score checked without touching the real files. A `--seed` run is never
recorded (§14) and so needs no such care.

`tests/pump.rs` is the shell driven headlessly — §15.2's steps called the way a
front-end calls them, with no screen and no clock. It is where cadence
invariance, the catch-up cap, §7's phases, `deadline`'s bounds, §8.4's two
forced pauses and §10.3's DAS and ARR by A4's own two numbers are checked, and
it is the test a fourth front-end inherits for free.

`tests/gui_render.rs` is the window's I4 — `tests/render_sizes.rs` asked in
pixels. Seven viewport sizes in *physical pixels* at four densities, every
screen the program can show, through a real `egui::Context` and no window
(`GUI.md` §G9.1). Between it and `tests/pump.rs`, the window front-end has what
`drive.py` gives the terminal; the native window can also be *driven*, which is
below under **Commands**, and that is what B3-B11 were checked with.

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

**The native window can be driven too, and G12 is when that was worked out.**
Earlier sessions recorded that they could not, and reached for a human; they
did not have to. On macOS, start the release binary in the background and send
it keys with `osascript`:

```bash
# The recipe G12 used, whole: drive the §13.5 Options panel and assert on the
# file it saves. `preview_count` moving 5 -> 6 is what says every key landed.
./target/release/ftm-gui --config /tmp/t.toml & pid=$!
sleep 5                                    # the window must take the keyboard
k() { osascript -e "tell application \"System Events\" to key code $1"; sleep 0.4; }
k 125; k 125; k 125    # Down x3 -> OPTIONS
k 36                   # Enter    -> open the panel
k 124                  # Right    -> step the selected row
k 53                   # Esc      -> §13.5 saves the config and returns
# The quit key on the attract screen closes the window, and a *clean* exit is
# what runs §6.2's first-exit write and §G8.10's geometry write-back. `kill` is
# the fallback for a script whose keys went astray, and skips both.
osascript -e 'tell application "System Events" to keystroke "q"'
sleep 2; kill -TERM $pid 2>/dev/null
grep -E '^preview_count' /tmp/t.toml       # the assertion
```

`key code` is what the keys §10.1 names but `keystroke` cannot spell:
125 Down, 126 Up, 123 Left, 124 Right, 36 Enter, 53 Esc, 49 Space. G12
exercised Down, Right, Enter, Esc and a `keystroke` letter; G13 added Space, in
runs of forty for a top out.

**And the window can be looked at, which G13 is when that was worked out.**
`screencapture -x -o shot.png` takes the screen without a shutter sound or a
cursor, and `sips -c H W --cropOffset Y X` trims it to the window — the two
together are how B3, B5, B9 and B10 were checked. It needs **Screen Recording**
permission, granted once beside the Accessibility one. This is what turns "a
person has to look at it" from a blocker into a step: G13 read the attract
screen, the six previews, the controls table with two bindings gone, and the
pause that another application stealing focus forced, off actual pixels. A
screenshot is worth taking even when a test passes — the §13.5 panel's nine
rows, and the fact that §12.3's colour depth is not among them, is not something
any assertion in the tree was going to tell you.

**A key is dropped now and then, and that is the thing to design around.** It
was measured here: the same six-key script into the §13.5 Options panel failed
once and passed once, and the failure was a lost `Down` — which put the cursor
on CONTROLS, made `Enter` open the controls box, and left the remaining keys
doing nothing at all. Nothing errored, and nothing looked wrong. So **script
towards an observable outcome and assert on it** — a config file that gained a
value, a process that exited — never on the exit status of `osascript`, which
is `0` whether or not the key arrived. A run that asserts nothing is a run that
proves nothing.

Three more things. It needs **Accessibility permission** for whatever runs the
command, granted once in System Settings, and does nothing without it. Keys go
to the **front** window, so nothing else may steal focus mid-script — and
§G4.7 pauses a game the moment something does, which is a correct result and
not a hang. And allow several seconds after launch before the first key: the
window has to exist and take the keyboard.

That is how G12 checked the Options-panel save in both front-ends, the §G8.10
geometry write-back, `remember_window = false`, and the 125 % scale bug that
shrank the window on every run — none of which any test in the tree can see.
It is worth more than it looks: it turns "a person has to look at it" into a
loop. The scale bug in particular was found by running the same binary *twice*
and diffing the file, which is exactly what a human at a window does not think
to do. Like `drive.py`, it substitutes for, but does not replace, playing the
game.
