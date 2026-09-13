# Falling Tetromino Manager (ftm)

A guideline-conformant falling-block game in Rust. No server, no unsafe. The
name is the joke; `ftm` is the binary, the crate, and the config and data
directories. The five specification documents and the three plans live in
**`spec/`**, along with `NOTES.md` — the per-stage history and the measurements
behind the rules below, which this file used to carry inline.

**All three plans are finished and there is no live plan.** `TERMINAL-PLAN.md`
stages 1-12 (milestone M4, §17.3's A1-A10 signed off one by one), `EGUI-PLAN.md`
G0-G13 (MG7, `GUI.md` §G9.3's B1-B12) and `PILOT-PLAN.md` P0-P8 (§P9's C1-C13).
Read them for why something is the shape it is, not for what to do next. A
fourth front-end or a §18 bullet would need a plan written first.

Three binaries over one core:

| Binary | Command | Opens on |
|---|---|---|
| `ftm` | `cargo run --release` | The terminal: §13's attract screen, §12.4's 44 x 23 playing screen, §12.1's 60 x 24 minimum |
| `ftm-gui` | `make run-gui`, `make run-web` | A native egui window, and the same code as wasm in a browser tab (`--features gui`) |
| `ftm-pilot` | `make bench` | Nothing — headless batches and §P8.2's report (`--features bench`) |

The tree is `core/` + `shell/` + `tui/` + `gui/` + `pilot/`, with
`src/native.rs`, `src/argv.rs` and `src/bench.rs` beside the front-ends rather
than under them.

- **`core/`** — the rules. `Game::tick(&TickInput, &mut Vec<GameEvent>)` is the
  single entry point, `Game::view()` the only way to see the result, with
  `Game::debug()` beside it. Every module inside is `pub(crate)`.
- **`shell/`** — what no front-end owns *and no platform reaches*: `config` (§6),
  `input` (§10), `highscore` (§14), `keys` (F5's neutral vocabulary and §10.1's
  name grammar), `menus` (§12.6 and §13's models), `attract` (§13), `cosmetics`
  (§12.5), `palette` (§9.2 levelled), `figures`, `time` (F1's `Stamp`),
  `storage` (F2), `host` (F2-F4 bundled), `session` (`Session`, `Next`,
  `Player`) and `round` (`Round`, `FrameState`, `Debug`, `App`).
- **`tui/`** — the terminal: `keys` (crossterm adapter), `cli` (§6.4), `term`
  (§8.1-§8.3), `run` (§7 and §15's two loops), `theme`, `cells`, `playfield`,
  `overlays`, `attract`.
- **`gui/`** — the window: `app` (`impl eframe::App`, the pump), `keys` (egui
  adapter), `layout` (§G3's metric), `paint`, `playfield` (§G4, §G6),
  `overlays` (§G5), `attract` (§G7), `query` (§6.4 as a URL), `cli`, and
  `host_native` / `host_web` — `gui::host` is whichever the target has.
- **`pilot/`** — the automated player, a sibling of `core/` and `shell/` and
  neither a front-end nor a layer: `knowledge` (§P2.4), `fork` (§P2.3),
  `eval` (§P5), `placement` (§P4.1), `reachable` (§P4.2), `controller`
  (§P3.4's `Pilot`), `search` (§P6), `bench` (§P8.2), over `core/search.rs`.

T1-T17, I1-I4, `tests/pump.rs` and `tests/gui_render.rs` all pass, and the
batch-invariance canary is in CI.

## Read these first

Everything below names the documents the way the source does — `GUI.md §G7`,
not `spec/GUI.md §G7` — because that is what several hundred doc comments say.

**Read the sections your work touches, not the whole document.** Start from
`FRONTEND.md` if the work is a front-end's, and `PILOT.md` if it is the
automated player's.

| Document | Owns | Is |
|---|---|---|
| **`FTM.md`** | §1-§7, §9-§11, §12.7, §12.8, §14-§19 | The front-end-agnostic specification: rules, config, states, controls, the view model and the event stream, high scores, timing, errors, §19. |
| **`FRONTEND.md`** | no numbers | The contract any front-end is written against: F1-F7. The document a fourth front-end reads first. |
| **`TUI.md`** | §8, §12.1-§12.6, §13, §6.3's four glyph and colour keys, §17.3's A1-A10 | The terminal front-end. Raw mode, the 60 x 24 minimum, colour depth, the 44 x 23 layout, the attract screen. |
| **`GUI.md`** | §G1-§G9 | The egui front-end, native and web. Whole and normative; nothing in it is reserved. |
| **`PILOT.md`** | §P1-§P9 | The automated player: the mode, the information boundary, control, placements, evaluation, search, the two screens, the headless benchmark. |

`TERMINAL-PLAN.md`, `EGUI-PLAN.md` and `PILOT-PLAN.md` are history. Two things
in them are not: each plan's **"The decisions this plan rests on"**, and
`EGUI-PLAN.md`'s **"The central idea"** (the shell is pumped, not looped) and
its **Macroquad readiness**, which is a list of constraints rather than a work
item. **`PILOT-PLAN.md` P8's two findings are the ones to read before touching
the planner**: the search was returning a shallower answer than its settings
claimed, in two different ways, and both had passing tests over them.

**The specification is ground truth. If the code and the spec disagree, the
spec is wrong until it is amended** — fix it in the same commit and say so in
the message. Never let the code silently diverge.

**Section numbers never move.** G0 split the specification in four and P1 added
a fifth; the sections that left `FTM.md` kept their numbers, so every `§12.4` in
the source still resolves, and `FTM.md` keeps a stub at each vacated number.
**Do not renumber anything** — it would silently invalidate several hundred doc
comments, each of which would still *read* fine. An unqualified `§n` means
`FTM.md`, except the eleven numbers `TUI.md` owns; `§Gn` is `GUI.md` and `§Pn`
is `PILOT.md`. **A stage and a section share a letter and are told apart by the
`§`**: `§P4` is a section, P4 is the stage. §12.7 and §12.8 stayed in `FTM.md`
because they are what a front-end is written *against*, and `FRONTEND.md` owns
no numbers because it collects rather than legislates.

## Invariants that are easy to break

These are the ones a fresh session gets wrong. Each is normative in the spec;
`NOTES.md` has the measurements and the rejected alternatives.

### The core and the rules

- **Coordinates are y-down** (§5): `row` 0 is the top of the 40-row buffer, 39
  the floor, positive `dy` moves **down**. §9.5's SRS kick tables are **already
  converted**. Do not negate them again.
- **The core takes no clock and no I/O** (§3.1), **and neither does the shell**.
  Time enters the core as ticks and the shell as a `shell::time::Stamp` the
  front-end hands over. Nothing in `shell/` may name `Instant::now`, `std::fs`,
  `rand::random` or a calendar; `make portable` is the compiler holding that. A
  `cfg(target_arch)` in `shell/` passes that check and is a bug, not a fix.
- **The core advances in fixed 1/60 s ticks** (§15.1), never a variable
  `Duration`, and is **deterministic**: same `RulesConfig` + seed + inputs ⇒
  byte-identical state.
- **No floating point in the rules** (§9.9, §6.6). Gravity is an integer *fall
  period* in 16.16 ticks-per-row — a period, not a rate, so level 1 falls on
  tick 60 exactly. The accumulator is a **remainder**, always below the period
  in force, so a period that shortens under it (soft drop, a level-up) cannot
  cash in charge banked at the slower rate. Durations are integer ticks,
  converted once at config load.
- **T-spin**: the "last action was a rotation" flag must survive a hard drop,
  and kick test 5 always means a proper T-spin (§9.13). The most commonly
  botched rule in the spec.
- **Scoring reads the level at the lock**, before §9.12 step 7 advances it, and
  the perfect-clear bonus reads the same level and the same back-to-back flag as
  the clear that earned it — not the chain state afterwards.
  `Game::clearing_b2b` carries that flag across the clear pause.
- **`back_to_back` is two different questions.** `GameView::back_to_back` is
  whether the chain is *live*; `LinesCleared::b2b` is whether *that clear* was
  paid at the chained rate. They differ on the clear that starts a chain (§9.15,
  §12.8).
- **Hold** clears its lock-out when the next piece **locks**, not when it spawns
  (§9.7).
- **A four-line clear is a `QUAD`** (§1.3, §2, §9.14), never a `TETRIS` — in the
  interface, the spec and the source, where the variant is `ClearKind::Quad`.
  The word is trademarked and stays out of the game.
- **The piece sequence is the spec's, not `rand`'s** (§9.6). `bag::seeded`
  expands a `u64` with PCG32 and `Bag::uniform_inclusive` uses Lemire's method,
  both written out. Do **not** simplify them back to `SmallRng::seed_from_u64`
  and `random_range`: `rand` has changed each once already, and either change
  silently makes every recorded seed name a different game. **The generator is
  named too** — `Xoshiro256PlusPlus`, never `SmallRng`, which is
  `Xoshiro128PlusPlus` on a 32-bit target; wasm32 is one, and the web build
  dealt a different game for every seed until G6 found it. The I1 snapshot
  catches the first mistake and cannot catch the second (the tests run on a
  64-bit host) — a `const` assertion on the generator's size in `core/bag.rs`
  does, under `make portable`.
- **The core's public surface is its façade, and the compiler holds it there**
  (§3.1, §17.3 A10). Every module inside `core` is `pub(crate)`; `core/mod.rs`
  re-exports exactly what the shell may name. Adding a `pub use` there is a
  decision about §19's wire vocabulary, not a plumbing convenience.

### The view, the events and the shell

- **The renderer draws only from `GameView`** (§12.7) and never reads `Game`.
  Cosmetics are driven by `GameEvent` (§12.8); dropping every event must change
  nothing about the game. The core appends to the caller's buffer and never
  reads it back.
- **View and event coordinates are visible-field coordinates** — `(col, row)`
  with row 0 the topmost *visible* row, matrix row 20. **`PieceView::cells` is
  the one signed pair and the one thing not clipped**: a negative row is a mino
  above the field, all four minos are always present, no sentinel. Everything
  else is unsigned and clipped in `core/view.rs`, and an event coordinate above
  the field is omitted as `(255, 255)`. The asymmetry is the difference between
  the two streams — an event starts an animation and there is nothing to animate
  where nobody can see, while the falling piece is *where it is* and §9.4 puts
  part of it above the field on every spawn but `I`'s. A front-end draws as many
  rows above the field as it has room for (`GUI.md` §G6.6).
- **`DebugView` is a second view type, not a field on `GameView`** (§12.7).
  `show_debug` is a presentation setting, so the core cannot be told whether
  anyone is looking — and the load-bearing reason: the bag beyond
  `preview_count` is **hidden information**, and under §19 `GameView` is what a
  server sends a player. Do not fold it in to save a method.
- **`Chrome` carries what `GameView` cannot** (§12.4, §12.7). `hold_enabled` is
  the one layout question the view cannot answer — an empty hold slot and an
  absent hold mechanic are both `hold: None`.
- **`fall_progress` is the only field on `GameView` no rule may read, and the
  terminal drops it before comparing frames** (§9.9, §12.7, `GUI.md` §G6.5). It
  is §9.9's accumulator over the period in force — the exact sub-row position,
  not a tween — and it is zero whenever the piece cannot move down. That clause
  is **explicit and not a consequence of §9.9's reset on a blocked step**: at
  level 1 the lock delay expires at tick 30 and the period at tick 60, so a
  resting piece normally locks without ever attempting the step that would clear
  it. `tui/run.rs` zeroes it before building its `Frame` — a character cell
  cannot draw it, and leaving it in would redraw a byte-identical screen sixty
  times a second. Horizontal movement and rotation are never interpolated, in
  any front-end.
- **`RulesConfig` and `PresentationConfig` are separate structs** (§6.5). Do not
  merge them into one `Config`.
- **DAS/ARR live in the shell**, not the core (§10.3). The core is told a
  direction and a whole number of cells and has no notion of a key being down.
  Resolved input is held until a tick consumes it, because a frame may
  legitimately run zero ticks (§15.2 step 6 wakes the loop early on a key).
- **Disabled keys** (`hold_enabled`, `allow_180_rotation` off) are dropped at
  the input boundary, so they cannot reset a lock-delay timer as a side effect
  (§10.1).
- **Keys reach the shell neutral, and only the adapter knows otherwise**
  (`FRONTEND.md` F5). `shell::keys::{Key, Mods, KeyKind, KeyEvent}` is the
  vocabulary above the front-end, and §10.1's name grammar lives with it because
  the `[keys]` table a player writes is shared property. `tui/keys.rs` is the
  only terminal module that may name a crossterm key type; a `KeyCode` §10.1 has
  no name for is dropped there, which is why §13.6 says "any key the game can
  *name*". **`KeyEvent` must stay synthesisable**: a front-end with polled input
  will manufacture the stream by diffing frames, so nothing above may depend on
  seeing every intermediate event, on sub-frame ordering, or on an event
  arriving anywhere but a frame boundary.
- **Animations see events and a clock, and nothing else.**
  `shell::cosmetics::Cosmetics` takes `&[GameEvent]` and an `Instant` and has no
  path to `Game`, which is what makes §12.5 provably free of side effects
  (§12.8). If an animation seems to need to ask the core something, the answer
  belongs in `GameView` or in the event.
- **The shell is pumped, not looped** (§7, §15.2, `FRONTEND.md` F7). §15.2's
  seven steps are methods on `shell::round::Round` — `key` is step 2, `advance`
  is 1 and 3-5, `frame` is what step 5 draws from, `deadline` is step 6's advice,
  `viewport` is §8.4 — and the front-end owns the loop. **`advance` must stay
  correct at any cadence**: its accumulator is over real elapsed time, and
  nothing may start counting frames. **`deadline` is advice, not a frame rate** —
  a front-end may be woken sooner by a key, a compositor or a tab regaining
  focus, and one that renders at vsync may ignore it. `tests/pump.rs` holds both.
- **The four capabilities are the front-end's, and three travel as
  `shell::host::Host`** (§3.1, `FRONTEND.md` F1-F4). Storage, the seed and the
  date are values a run borrows; time is the fourth and arrives as a `Stamp`
  argument, because every entry point already takes the moment it is called at.
  `Host` is a struct of *values* and must not become a `trait Frontend` — the
  front-ends share the shell by calling it.
- **`src/native.rs` is a desktop, not a fourth layer** (§3.1, §4). It sits
  *beside* the front-ends, behind `any(feature = "tui", feature = "gui")` and
  `not(target_arch = "wasm32")`: both native binaries answer F1-F4 from it,
  because §6.2 and §14 give `ftm` and `ftm-gui` one config file and one
  high-score table between them. Nothing in `shell/` or `core/` may name it,
  which is what `make shell` and `make portable` keep true.

### Config, storage and the command line

- **Clamping is silent in `RulesConfig::from_settings` and loud in the loader**
  (§6.2, §6.3). `from_settings` is also the path a §19 peer's rules take, where
  there is nobody to warn; `config::validate` tells the player what their file
  asked for and what it got.
- **The config file is parsed value by value**, not deserialised whole (§6.3): a
  value of the wrong type is rejected by itself and the default used. Only a
  document that is not TOML at all falls back wholesale, which is the one case
  I3 pins at exactly one warning.
- **Every binary holds every table, and only one acts on each** (§6.2, `GUI.md`
  §G8.10, `TUI.md` §6.3). `ConfigFile` has `[gui]` in the terminal build and
  `[display]`'s glyph and colour keys in the window build, because
  `config::document` rewrites the whole file and a table the struct has no field
  for is one the next save silently deletes. Both parse, clamp and *warn* about
  both — the player edits one file. Preserving unknown tables generically is the
  rejected alternative: §6.3's loader is value-by-value precisely so it can warn.
- **`Ok(None)` and `StorageError::Unavailable` are different answers** (§6.2,
  §14, §16). Nothing stored yet is the ordinary first run and never a warning;
  nowhere to store it warns **once, on the read**, and
  `StorageError::warning()` returns `None` for it so the write that fails the
  same way stays quiet. Getting this wrong changes I3's warning count without
  changing any test.
- **§14's atomic write is §14's alone** (§6.2, §14). `native::Files` renames a
  temp file into place for the high-score table and does a plain `fs::write` for
  the config, deliberately: a rename goes straight over a read-only file where a
  write is refused by one. §16's line names the *target* either way — a player
  has never heard of the temp file.
- **A score of 0 never qualifies, ties keep the older entry above, and a seeded
  run is never recorded** (§14, §6.4). The first two are `highscore::Table`'s;
  the seed rule is `App::finish`'s, the only place that knows.
- **§6.4's grammar is a front-end's and its meaning is the shell's.** `clap`
  lives in `tui/cli.rs` and `gui/cli.rs`; what crosses into `shell/config.rs` is
  `Overrides`, which `gui/query.rs` fills in from a URL just as well. A flag
  exists in a build when the setting it names does (`GUI.md` §G8.11's table is
  the list, held by tests in both directions). `ColorDepth` and `LockDownRule`
  have hand-written `ValueEnum` impls in **`src/argv.rs`** rather than derives,
  because a derive would put `clap` in `shell/config.rs` and an impl behind
  `feature = "tui"` is not there for a `--features gui` build.
- **`remember_window` writes only when something moved** (`GUI.md` §G8.10). It
  does not remember `fullscreen`, because that is a *choice* and `--fullscreen`
  is a flag §6.1 never writes back; and it does not save an unchanged file,
  because that would rewrite the player's config every run and add §16's warning
  to every run over a read-only one.

### Both front-ends

- **A screen offers what its front-end can do, and the shell navigates the same
  list** (§13.3, §13.5, `GUI.md` §G5.4, §G7.5, `PILOT.md` §P7.1). There are two
  lists. `Session::settings` is §13.5's panel — `Setting::ALL` (terminal, the
  shared seven plus §12.3's colour depth), `Setting::WINDOW` (plus scale and
  full screen), `Setting::CANVAS` (plus scale alone; a canvas cannot go full
  screen), built from `Setting::SHARED`, which nothing sets directly. And
  `Session::menu` is §13.3's — `MenuChoice::ALL` is six items with PILOT,
  `MenuChoice::CANVAS` is a tab's four, short of QUIT because a tab cannot close
  itself and short of PILOT because a tab would search on its frame thread.
  Nothing is `cfg`-ed out. **Drawing a different list from the one the shell
  walks puts the cursor on a row nobody can see.** Since P4 there is a third
  list inside the second: §13.3's panel cycle pauses on any item but the two
  that **start a game**, so `Attract` remembers the `MenuChoice` the cursor is
  on and not only where it is.
- **§13's words are shared, and only the blocks are a front-end's** (§13.2-§13.4,
  `GUI.md` §G7). The letterforms live in `shell/attract.rs` as a bitmap, with
  §13.6's colour cycle, §13.3's reminders and §13.4's numbers; a terminal draws
  one block as two characters and a window as one mino. A second wordmark would
  be a second piece of branding under §1.3. What stays in each front-end is
  where a thing lands and how the drift is positioned.
- **What is drawn is §12.3's levelled palette, not §9.2's table** (§9.2, §12.3).
  §9.2's seven are equally saturated but not equally bright — luma 17 for blue
  against 223 for yellow — so `shell/palette.rs` lifts purple, red and blue, and
  every piece colour on every screen goes through `Theme::piece`. The three do
  not land on one number (purple reaches 165, red and blue stop at 102 and 84):
  blending toward white buys brightness with saturation, and those two turn into
  salmon and lavender long before purple stops being purple. The lift is
  **shared presentation** because the luma problem is a property of §9.2's
  colours and not of terminals; what stays in `tui/theme.rs` is how a colour
  lands on *this* display. `Colour::rgb` is still §9.2, which is what a §19
  client is handed, and the levelled value is the *base* §12.3's dimming scale
  runs from, so a piece and its ghost are one hue.
- **§9.17's blank well is `Overlay::blanks`, not a front-end's `matches!`.** Both
  front-ends ask it. G5's scrim let the paused stack show through at a quarter
  brightness, which was exactly the free look §9.17 forbids. It gates the
  *whole* of the window's composition, not each animation in turn.
- **The Options panel applies presentation at once and rules never** (§13.5). A
  game keeps the rules it started under; colour depth and the grid take effect
  on leaving the panel. `Chrome::hold_enabled` therefore comes from the running
  game's `RulesConfig`, not from `App::config`.
- **The attract screen (§13) stays provisional**, but its layout is pinned by
  tests and §13.3's mock-up is what the code draws. The panel is four rows, not
  three: seven control entries do not fit in six slots.

### The terminal

- **Terminal teardown is idempotent and runs from three places** (§8.3): normal
  exit, an error, and the panic hook — installed *before* raw mode, so a crash
  restores the terminal before it prints. Both §8.2 input paths are live; do not
  let one rot.
- **§12.1's minimum is 60 x 24 and it is the spec's, not the layout's.** The
  playing block is 44 x 23 and the attract block 36 x 20, so both fit with room
  to spare. `tui::fits` is the one place that decides, and `tui::draw` and
  `tui::attract::draw` check it themselves so no caller can reach a layout that
  assumes room it has not got. `show_debug`'s strip makes the block 44 x 28 and
  deliberately does *not* move the minimum.
- **A resize forces a game into `Paused` before the screen goes** (§8.4). That
  is `App::cramp`, and it releases the held keys the way the pause menu does —
  nothing expires a held key while the clock is stopped (§8.2). It does not undo
  itself when the terminal grows again; the player leaves the pause and gets the
  3-2-1 countdown for it.
- **Overlays are centred over the whole 44 x 23 block**, not over the
  playfield's interior: §12.6's game-over box is 24 characters wide and the
  interior is 20.
- **`Session::generation` is one of the frame's five components, and the
  comparison is the *terminal's*.** §15.2 step 5 draws only when the frame
  changed, and `tui::run::Frame` compares `shell::round::FrameState` — the
  `GameView`, the `Overlay`, the generation counter, §10.1's restart bar and
  whether the viewport is cramped — plus §12.1's terminal size, the terminal's
  own addition because its message names it. **Anything else the screen comes to
  show has to join that struct or bump the counter**, or it will not be
  redrawn — and, worse, will not be *erased*. Two things have already walked into
  this. **An immediate-mode front-end must not port this** (`FRONTEND.md` F7).
- **`tui::theme::Glyphs` are leaked, once, at start-up** so `Theme` stays `Copy`
  (§12.2). `Glyphs::configured` is a start-up call, not a per-frame one.

### The window

- **The window's grid is whole pixels, and its minimum is in points** (`GUI.md`
  §G3). `gui::layout::Measure` floors the cell in *physical* pixels and centres
  the block on a whole pixel, so every rect is crisp at 1.25 as well as at 2;
  the minimum is 14 *points*, because legibility does not double with the
  density. Everything on the playing screen is placed through `Layout` — a rect
  computed in points beside it will land between pixels. Text is the exception
  and is placed freely; `egui` rounds it.
- **Losing the keyboard pauses a game, every pump, and draws nothing** (`GUI.md`
  §G4.7). `Round::keyboard(heard, now)` is §8.4's `cramp` path beside
  `Round::viewport`, and `gui/app.rs` calls it on every `logic` pass rather than
  on the change: a countdown the player leaves running when they click away is
  not `Playing`, so the pump it runs out on is the one that must pause, and
  `keyboard` settles the countdown first so that pump plays no tick. The
  terminal does not call it.
- **"Have I the keyboard?" is two questions in a window, and `egui` answers only
  one** (`GUI.md` §G4.7, §G8.7). `gui/app.rs` ANDs `input.focused` with
  `host::visible()` — `!document.hidden` in a tab, a constant `true` natively —
  and **dropping the second half is a silent bug, not a compile error**. A
  hidden tab keeps the canvas's DOM focus and fires no `blur`, so `egui` reports
  a focused application that hears nothing and the game plays on behind it:
  measured at G13, 13 seconds of play in 31 seconds hidden. Natively the two
  agree, which is why the constant is not a stub.
- **A window's animations are transformations, not layers** (`GUI.md` §G6.2).
  §12.5's flash and wipe change the colour a cell was already going to be drawn,
  so `gui::playfield::compose` builds the well as colours and paints it once; a
  painter cannot revisit a rect it has emitted. That is also what makes each a
  unit test over a pure function. Timings and triggers stay `shell::cosmetics`',
  and **nothing in a front-end may ask `Cosmetics` for a number the other one
  does not have**.

### The pilot

Every one of these is held by a type, by `make portable`, by a module that
cannot name `Game`, or by a debug assertion that runs on every tick of every
planned game in the test suite.

- **The planner never holds a `Game`** (`PILOT.md` §P2). A clone would carry the
  real bag and `Game::bag_remaining` is an accessor onto hidden information, so
  fairness cannot be a convention. It holds a `SearchGame`: a fork whose
  randomiser has been **replaced** by a scripted queue the planner built from
  what it observed, and which refuses to spawn past the end of it. The
  replacement is in `core/bag.rs`, one level below `Game` — a `Bag` is a
  generator-and-its-bag *or* a list, so there is no randomiser in a fork that
  has merely been promised not to ask. `core/mod.rs` re-exports the seam with
  `pub(crate) use`, so **A10 itself does not move**. `pilot::Fork` is the same
  object seen from above, and exists because a crate-private type cannot appear
  in a public signature — it is also where the *queue* arrives, the half of
  fairness no type can hold. §P2.3's method list is an **upper bound**; each
  accessor lands with the stage that reads it.
- **The search cannot name a `Game`, and the *roots* are where fairness is
  decided** (`PILOT.md` §P2.1, §P6.3). `pilot/search.rs` imports `Fork` and never
  `Game`. `controller.rs` builds the positions a search is handed: one fork on
  the visible preview, or — when §P6.1's depth reaches past it — **one per
  hypothesis**, blended 80/20 expected-to-worst. A chance node is a list of
  positions somebody else decided the search was allowed to consider.
- **The planner takes no clock** (`PILOT.md` §P3.3) — the house rule's third
  instance. No `Instant`, no frame count, no wall-time budget, and a search
  **never amortised across frames**. `src/pilot/` is in `make shell` and `make
  portable` for exactly the reason `shell/` is. §P5's "integers only" is the same
  rule about a different quantity, for §9.9's reason: the features, weights and
  evaluation are all `i32`, so a plan is identical on every target and in every
  profile. What this buys is cadence invariance for a PILOT round, asserted in
  `tests/pump.rs`.
- **The benchmark is split at the no-clock line, and that is what makes it two
  files** (`PILOT.md` §P8). `pilot/bench.rs` plays the batch and folds it into
  integers — no clock, `make portable` compiles it — and `src/bench.rs` holds
  §P8.1's grammar and the wall clock, behind a `bench` feature that is neither
  front-end's. The report has the same seam: the header, the per-seed rows and
  the aggregate are integers and reproduce byte for byte, and the throughput is
  a **trailing block headed as this machine's**, excluded from that promise.
  Folding a duration into a row would make the baseline undiffable. Everything
  above the `timing` heading is diffable and everything below it is not.
- **The planner thinks once per piece, on the tick it spawns** (`PILOT.md`
  §P3.1), and emits one `TickInput` per tick until it locks. A hold raises
  `PieceSpawned` too, and a planner that re-planned on the *event* would tear up
  a plan it was halfway through. Two things follow. `App::advance` gives a
  batch's edge actions to its *first* tick only, because a human produces input
  per frame; a planned round needs the batch's *n*th input on its *n*th tick, so
  that is a **branch inside `advance`** with the human path byte-identical. And
  `observe` is handed the slice of the frame's event buffer that tick appended,
  not the whole of it. **A plan is never recomputed from a partly executed
  state** — divergence is a bug, not a resync, and `Pilot::verify` is the debug
  assertion that says so tick by tick.
- **A plan ends at the tick that locks the piece, and the fork knows which tick
  that is** (`PILOT.md` §P3.1, §P4.1). The sequence is **truncated** at the lock;
  an input after it is one the next piece would receive. The replay bought two
  things: features are measured after §9.12's clear delay, because a board
  measured during it still holds the rows it is about to lose and a planner that
  scored it would never clear a line; and the divergence assertion can only
  compare as far as the fork's queue reached, because a hold at a
  `preview_count` of 1 uses it up.
- **Both generators produce input sequences and neither predicts** (`PILOT.md`
  §P4.1, §P4.2). `placement.rs` rotates at spawn, shifts and hard-drops;
  `reachable.rs` walks the graph of positions a tick at a time. They are
  interchangeable where `search.rs` calls one, and **both replay every candidate
  through the real rules**, so a kick, a wall and a gravity lock that beats the
  hard drop are ordinary in both. A third generator joins the same seam or it is
  not one.
- **Legality is the fork's and never the key's** (`PILOT.md` §P4.2). Every edge
  of the walk is a real `Game::tick`, so a walk that slid a piece past §9.11's
  reset budget does not produce an illegal placement — the fork locks the piece
  and the walk files that as the placement it turned out to be. Nothing in
  `reachable.rs` decides what the rules allow, which is what makes "exact" mean
  exact rather than "exact as far as this module models it".
- **Exactness in the walk is argued three times, and each argument is why
  something is *missing*** (`PILOT.md` §P4.2). A hard drop is played only from
  poses that **rest**, because a drop from mid-air lands on a resting pose the
  descent edges reach anyway. Hold is played only at the **root**, because §9.7
  allows one a piece. And **lock state is not in the state key**, because
  breadth-first order reaches a pose by its shortest path and that path has spent
  the least of §9.11's delay — keying on it is thirty thousand states instead of
  one thousand. §9.9's sub-row accumulator is left out too, and that one is a
  genuine gap rather than a dominated one.
- **A descent is soft drop, and that is the difference between a walk and a
  hang** (`PILOT.md` §P4.2, §9.9, §9.10). A plain tick falls at §9.9's period —
  sixty ticks to the row at level 1 — so a graph whose only downward edge was
  "wait" would need sixty thousand ticks to cross the well. Soft drop divides the
  period and is neither an action nor a shift, so §P3.2's cap is untouched.
  Related: **this game has no high gravity.** §9.9's curve bottoms out at
  `MAX_SPEED_LEVEL` 15 and `--start-level` is capped at the same 15, so a
  level-15 walk is *identical* to a level-1 one; the only way to reach a row a
  tick is `soft_drop_factor`.
- **The pose crossing §P2.3's seam is five numbers, not an `ActivePiece`**
  (`PILOT.md` §P2.3, §P4.2, §17.3 A10). `ActivePiece` is not in the core's façade
  and may not join it, so `SearchGame::pose()` returns a crate-private `Pose`
  and `Fork::pose` is `pub(crate)` — nothing outside the crate deduplicates
  positions. Two of the five are not geometry and are the reason `view()` will
  not do: §9.13's "last action was a rotation" and the kick index. A pose reached
  by turning and the same pose reached by shifting are **two different
  placements**.
- **The board is charged at the leaf and the events at every ply** (`PILOT.md`
  §P5). `Features::evaluate` is a leaf's; `Outcome::interior` is an interior
  ply's and is deliberately **not** the whole of `Outcome`: a clear, a perfect
  clear and a top out are *events* charged where they happen, while §9.15's combo
  and back-to-back are **state**, read once where the branch ends. Charging a
  chain at every ply it survives pays for one back-to-back as many times as the
  search is deep.
- **A quad is bought with a price on the alternative and a reward for the
  progress — never with a bonus on the quad** (`PILOT.md` §P5). Rewarding the
  quad itself gives **byte-identical** reports at any size, because at two plies
  a quad is invisible until the stack that earns one already exists. The two
  levers that work are visible at *every* ply: a **negative** weight on the cheap
  clear, and **`well_rows`**. `wells` exempting the **deepest single column** is
  necessary for both and sufficient for neither — an exemption removes a penalty,
  and a planner still needs a reason to build the thing. **`clears[4]` is inert
  without `well_rows`**: a reward and the thing that makes it reachable are one
  decision, which is twice now that a weight looked worthless because a
  *different* weight was missing.
- **`well_rows` is capped at four, and the cap is the whole of its safety**
  (`PILOT.md` §P5, §9.14). An `I` is four cells, so a fifth well row is one
  nothing can clear. Uncapped — or, equivalently, exempting the well column from
  `row_transitions`, which was tried first — a planner digs a well it can never
  cash and **tops out seven games in eight**. Anything added here that rewards a
  shape rather than an outcome wants the same question asked of it.
- **A spin is priced by its `ClearKind`, and the fold used to throw that away**
  (`PILOT.md` §P5, §9.13, §9.14). `Outcome::clears` and `Weights::clears` are
  indexed by §9.14's *kind*, not by rows. They were indexed by rows until a
  player asked why there was no T-spin scoring: a T-spin double — 1,200 points
  and a live back-to-back — was counted as the plain double it has the row count
  of and charged that double's penalty. §P5 was not failing to reward a spin; it
  was punishing one. The `LinesCleared` event has carried its `ClearKind` all
  along, so this was a *fact* being discarded rather than an opinion being wrong.
- **`t_slots` is the one weight that belongs to the generator rather than to the
  board** (`PILOT.md` §P5, §P4.1, §P4.2). A cavity shaped like a `T`'s South
  footprint with three of §9.13's four corners filled, capped at one: positive
  under `exact` and **negative** without it, because §P4.1 can *build* a slot and
  can never turn a piece into one. Hence two weight sets, `Weights::default` and
  `Weights::exact`, differing in exactly this number, which a test asserts. The
  far side is a **cliff, not a plateau** — one step past the peak tops a game
  out — and it is the first feature in §P5 that is **not additive** with the
  others: a quad wants a flat nine-wide stack with one clean column, which is a
  board with no overhang and therefore no T-slot.
- **A ply is the granularity of "fully evaluated", and a ply is the whole beam**
  (`PILOT.md` §P6.4). The node budget is checked between one ply and the next,
  never inside a generation, so a search may overrun it by the ply it was in the
  middle of — comparing half a generation against a whole one is exactly the
  dependence on *where the count ran out* that §P6.4 forbids. The first ply is
  always paid for, so there is always a fully evaluated answer. **And a beam the
  budget cuts short is discarded entirely**: `subtree` charges nothing for a fork
  that has topped out or run out of its queue, so the branches that survive an
  exhausted budget are precisely the ones that lose the game. The rule is not
  "keep what finished"; it is all of the ply or none of it.
- **`Settings`'s three defaults are one decision, not three** (`PILOT.md` §P6.4).
  Two plies over a beam of 16 costs 1,784 nodes against a budget of 2,000, which
  is one frame's ~11,900 divided by a full `MAX_CATCH_UP_TICKS` batch. Widening
  the beam or adding a ply does not buy a deeper search; it buys a search the
  budget truncates, and §P6.4's answer quietly becomes shallower than `depth`
  asked for. §P4.2's walk spends ~6,400 nodes in **one** generation, so `exact`
  at the default budget is the one-ply answer — it is off by default for that
  reason and not for a preference.
- **§P6.2's beam is divided by the number of roots** (`PILOT.md` §P6.2-§P6.4). A
  beam candidate costs one expansion *per root*, and §P6.3 hands the search one
  root per hypothesis. `preview_count` 1 is the only configuration that reaches
  this at two plies (2 to 6 are byte-identical), and there a fixed beam is
  ~12,000 nodes against 2,000: the budget truncates it, §P6.4 correctly returns
  the *one-ply* answer, and the planner silently stops being the two-ply planner
  its weights were tuned for. With one root the division is a no-op.
- **A weight set belongs to a search configuration, not only to a generator**
  (`PILOT.md` §P6.4, §P8.1, §P5). `Weights::exact`'s `t_slots` was tuned at two
  plies with the budget lifted; at the one effective ply the default budget
  allows, it digs a slot it can never cash — **six top-outs in eight games**,
  the uncapped-`well_rows` failure for the third time. `ftm-pilot --exact` brings
  its own `--nodes` unless told otherwise, and anything else that offers the walk
  has to answer this before it offers it.
- **A spectator's keys are dropped at the input boundary** (`PILOT.md` §P7.3,
  §10.1). While the pilot plays, §10.1's movement, rotation, hold and drop keys
  do nothing — `play_key` returns before `InputState` sees them, exactly where a
  disabled mechanic's key is dropped. Ignoring what they *produce* instead would
  leave a held direction charging DAS and a press resetting a lock-delay timer
  behind a game nobody is playing. Pause, the pause menu, §10.1's restart hold,
  quit and §16's Ctrl-C stay live: a spectator can stop, look and leave.
- **The PILOT indicator is the `I`-piece cyan, in both front-ends, for the whole
  game** (`TUI.md` §12.4, `GUI.md` §G4.5, `PILOT.md` §P7.3). Left-aligned on the
  status row, over the centred content rather than joined to it. Deliberately
  not a transient: a screenshot of an automated game must never be mistakable
  for a player's. Both front-ends assert the colour as well as the position,
  because the first implementation had it in the plain text colour and no test
  would have known.

## Working agreements

- Tests land **with** their stage, not after it.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` all clean at
  every stage boundary. **`--all-features` matters**: a bare `cargo test` builds
  only the `tui` half and the other front-end silently stops being compiled.
- `#![forbid(unsafe_code)]`.
- One coherent change per commit; every commit builds.
- **The batch-invariance test is the canary** (§19.4): the same input log fed as
  1 × 6 ticks and as 6 × 1 ticks must produce identical snapshots. It lives in
  `tests/scripted_game.rs`, has been in CI since Stage 5, and must never be
  marked ignored. If it fails, stop and find the desync — do not proceed.
- **The I1 snapshot** (`tests/snapshots/scripted_game.txt`) is regenerated with
  `UPDATE_SNAPSHOT=1 cargo test --test scripted_game`. Read the diff before
  committing it: a snapshot that moves for no reason is exactly the bug the test
  exists to catch.
- **Two snapshots are *meant* to move, and they are not I1.**
  `tests/snapshots/pilot_bench.txt` and `tests/snapshots/pilot_plan.txt` (the
  plans three fixed seeds produce, a character to a tick). A weight, generator,
  tie-break or search change moves both by design (§P8.3); read the diff rather
  than chase it. `tests/pilot_plan.rs` is also where the hidden-future isolation
  test lives, outside the crate because it has to read a game's bag — which §P3.4
  forbids the planner's own tests to do.
- **§12.4's mock-up is a test.**
  `tui::playfield::tests::the_screen_matches_the_spec_mock_up` renders the state
  the mock-up depicts through a `TestBackend` and compares character for
  character; §12.6's two boxes are checked the same way. It is the cheapest way
  to notice a layout that has drifted by one column.

## Scope discipline

- §18 (other modes, sound, replays, themes, per-piece statistics) and §19
  (networking) are **not work items**. §19 is a list of constraints to honour;
  the only networking deliverable in the entire plan is that one CI test.
- **One §18 bullet was taken up, and only one**: the self-playing bot, now
  `PILOT.md` and `PILOT-PLAN.md`. It is a menu item the player chooses, not
  §13's idle demo — the attract screen still never starts playing by itself.
  This is not a precedent: the bullet was promoted by a written-down decision,
  not by a session deciding it was in.
- **`PILOT-PLAN.md`'s scope is its stages.** An automated player makes weight
  training, a replay format, an idle demo and a difficulty setting all feel
  newly reachable. None is in it. Automated weight tuning in particular is named
  as *not* in the plan.
- **`EGUI-PLAN.md`'s scope is its stages.** A window, and especially a browser
  tab, makes sound, themes, mouse and touch input feel newly reachable. §1.2 is
  unchanged: the game is keyboard-driven. Touch got a real answer rather than a
  reflex — **no touch controls**, and the page says so (§1.2, `GUI.md` §G8.8).
  Adding them is a §1.2 amendment.
- **Macroquad is `EGUI-PLAN.md`'s §19**: constraints so a fourth front-end stays
  cheap later, not a thing to build. Do not write `MACROQUAD.md`, and do not add
  a `trait Frontend` — the front-ends share the shell by calling it, not by
  satisfying an interface designed before the third one existed.
- The attract screen (§13) is explicitly provisional. Iterate on it if it wants
  it, but §17.3 never judged its looks.

## Open decisions

- **The legacy key path's feel (§8.2).** Measured over two seconds of holding
  left: enhanced moves at 0 ms then every 33 ms from 166 ms; legacy moves at
  0 ms, stalls until the OS's first auto-repeat (~500 ms on macOS defaults),
  then runs identically from ~670 ms. Releasing a held direction overshoots by
  two or three cells. Both are inherent to a 90 ms hold timeout, not tuning.
  Whether §8.2 should say so is undecided — ask before amending it.
- **`RESTART_QUIET` is a second constant of the same shape**, 700 ms rather than
  90 because §10.1's restart has to survive the OS's *first* auto-repeat and a
  soft drop does not. If §8.2 is ever amended, that number belongs in the same
  paragraph rather than in `tui/run.rs` on its own.

## Commands

```
make check           # everything CI runs but trunk: fmt, clippy, test, shell,
                     # portable, web-check, release build
cargo check          # fast feedback
cargo test --all-features               # unit + integration
cargo test --all-features --test pump   # the shell, pumped headlessly
cargo check --no-default-features       # core + shell alone: the G2 boundary
make portable        # ...and with no platform under them: the G3 boundary
                     # (needs `rustup target add wasm32-unknown-unknown`)
cargo clippy --all-features --all-targets -- -D warnings
cargo fmt --check
make bench           # PILOT.md §P8's baseline: 8 seeds x 2000 pieces, release.
                     # BENCH_ARGS="--seeds 5 --pieces 20000" for anything else;
                     # `--json` for the same figures with a `timing` key.
                     # BENCH_ARGS="--exact --pieces 400" is §P4.2's walk -- 11x,
                     # so keep the batch short. `--exact` brings its own node
                     # budget; `--nodes` overrides it.
cargo run --release  # play it (`default-run` picks `ftm` of the three bins)
make run-gui         # the window
make run-web         # the same in a tab: http://127.0.0.1:8080/?seed=42
make web             # the web artefact, `trunk build --release` into dist/
make web-check       # clippy for the web front-end on wasm32
cargo run -- --print-config    # effective config
cargo run -- --seed 42         # deterministic run, not recorded to high scores
tools/drive.py c c             # drive the release binary on a pty
tools/drive.py --arg=--seed=42 --arg=--config=/tmp/t.toml esc down down enter
tools/drive.py enter resize:20x50   # §8.4 and §12.1, without a window to drag
```

`HOME=/tmp/somewhere tools/drive.py ...` redirects both the §6.2 config path and
the §14 data path, which is how a full game can be played to a top out without
touching the real files. A `--seed` run is never recorded and needs no such care.

**The terminal** is checked with `tools/drive.py`, which drives the *release*
binary on a pty and replays the capture into a character grid. Pass binary
arguments through `--arg`, glued with `=` both times (`--arg=--seed=42`) or
`argparse` claims them; without a seed only frames *within* one run may be
compared. It answers the §8.2 capability queries, so pass `--legacy` for the
fallback path, and a "key" of `resize:ROWSxCOLS` resizes the pty.

**The native window** can be driven and looked at (macOS): start the release
binary in the background, send keys with `osascript` (`key code` 125 Down, 126
Up, 123 Left, 124 Right, 36 Enter, 53 Esc, 49 Space), and capture with
`screencapture -x -o` plus `sips -c H W --cropOffset Y X`. Needs Accessibility
and Screen Recording permission. Allow several seconds after launch before the
first key, and use **one shell invocation** for launch-wait-keys-capture — a
second brings the terminal back to the front and the keys go there.

> **A key is dropped now and then, and that is the thing to design around.**
> `osascript` exits `0` whether or not the key arrived, so **script towards an
> observable outcome and assert on it** — a config file that gained a value, a
> process that exited. A run that asserts nothing proves nothing. And the
> capture is of the whole screen, so when the keys go astray the shot is of
> whatever *is* in front: treat "the shot does not show the game" as a reason to
> stop and re-run, not to look closer.

`tests/pump.rs` is the shell driven headlessly — §15.2's steps called the way a
front-end calls them, with no screen and no clock. Cadence invariance, the
catch-up cap, §7's phases, `deadline`'s bounds, §8.4's two forced pauses and
§10.3's DAS and ARR. It is the test a fourth front-end inherits for free.

`tests/gui_render.rs` is the window's I4: seven viewport sizes in *physical
pixels* at four densities, every screen the program can show, through a real
`egui::Context` and no window (`GUI.md` §G9.1).

Both substitute for, but do not replace, playing the game. `NOTES.md` has
the traps each of these tools has already sprung.
