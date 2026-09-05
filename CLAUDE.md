# Termino

A guideline-conformant falling-block game for the terminal, in Rust. Single
binary, no server, no unsafe.

**Status: Stage 11 of `PLAN.md` complete — milestone M3, feature complete.**
Everything in §1.1 is implemented. `cargo run --release` opens on the §13
attract screen — wordmark, menu, the six-second cycling panel, the drifting
background and the sixty-second idle colour cycle — and **PLAY** starts a game
on the full 44 x 23 screen of §12.4, with the pause menu, the 3-2-1 resume
countdown, the §12.6 game-over box, and the §13.5 Options and §10.1 controls
boxes reachable from both the pause menu and the attract screen. A top out
that earns a top-ten place opens name entry; `Enter` files it and it appears on
the attract screen. The config file loads and saves at the §6.2 path, the §6.4
CLI is complete, the §14 table is written atomically to the data directory, and
the §6.2 and §14 warnings reach stderr after teardown.

`Game::tick(&TickInput, &mut Vec<GameEvent>)` is still the single entry point
and `Game::view()` still the only way to see the result — with `Game::debug()`
beside it for the strip. The shell is `main.rs` (terminal), `app.rs` (§7's
state machine and both loops of §15), `config.rs` (§6), `highscore.rs` (§14),
`input.rs` (§10) and `ui/` (§12, §13). T1-T17 all pass, plus I1, I2 and I3, and
the batch-invariance canary is in CI. A3 and A6 are verified end to end on a
pty; idle CPU on the attract screen measures 0.2 % against §15.3's 2 %.

Stage 12 is next, and it is the last: resize handling and §8.4's force-to-pause,
§12.1's terminal-too-small screen, the `mono` / `NO_COLOR` paths played through,
I4, and A1-A10 signed off one by one.

The mono and `NO_COLOR` paths already work (`NO_COLOR=1 tools/drive.py` shows
the §9.2 letters and `..` ghosts); Stage 12 owns finishing and playing them.

## Read these first

1. **`PLAN.md`** — twelve implementation stages. Find the current stage; it names
   the spec sections that stage depends on and the tests that close it.
2. **`TERMINO.md`** — the specification. Self-sufficient by design: every kick
   table, timing constant and screen layout is in it. Read the sections the
   current stage names, not the whole thing.

`TERMINO.md` is ground truth. **If the code and the spec disagree, the spec is
wrong until it is amended** — fix the spec in the same commit and say so in the
message. Never let the code silently diverge.

## Invariants that are easy to break

These are the ones a fresh session gets wrong. Each is normative in the spec.

- **Coordinates are y-down** (§5): `row` 0 is the top of the 40-row buffer, 39 is
  the floor, positive `dy` moves a piece **down**. The SRS kick tables in §9.5
  are **already converted** to this convention. Do not negate them again.
- **The core takes no clock and no I/O** (§3.1). Time enters only as ticks.
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
  with row 0 the topmost *visible* row, matrix row 20. Clipping happens in
  `core/view.rs`, never in the renderer; a cell above the field is omitted,
  encoded as `(255, 255)`.
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
- **Animations see events and a clock, and nothing else.** `ui::Cosmetics` takes
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
- **`ui::theme::Glyphs` are leaked, once, at start-up** so `Theme` stays `Copy`
  (§12.2). `Glyphs::configured` is a start-up call, not a per-frame one.
- **`Session::generation` is one of the frame's four components.** §15.2 step 5
  draws only when the frame changed, and it compares the `GameView`, the
  `Overlay`, the generation counter and §10.1's restart bar. Anything else the
  screen comes to show has to join that tuple or bump the counter, or it will
  not be redrawn — and, worse, will not be *erased*.
- **A score of 0 never qualifies, ties keep the older entry above, and a
  seeded run is never recorded** (§14, §6.4). All three live in
  `highscore::Table`; the seed rule is `App::finish`'s, because it is the only
  place that knows the run was seeded.
- **The attract screen has been seen now.** §13 stays provisional, but its
  layout is pinned by tests and its mock-up in §13.3 is what the code draws.
  The panel is four rows, not three: seven control entries do not fit in six
  slots.
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
- **§12.4's mock-up is a test.** `ui::playfield::tests::the_screen_matches_the_
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
  plainly and looked at; iterate on it if it wants it, but it is not Stage 12
  work and §17.3 does not judge its looks.

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
  movement keys and nothing else, so `Confirm` in `app.rs` tracks the
  restart hold, off the action path entirely (`Bindings::action_of` is what
  lets the shell pick it out before `InputState` sees it). In legacy mode it
  survives silence for `RESTART_QUIET` — 700 ms, chosen to outlast the OS's
  *first* auto-repeat, not the 90 ms `HOLD_TIMEOUT` that separates later ones.
- **The frame is four components now**, not three: view, overlay, generation
  and the restart bar's percentage. Anything else the screen shows that is in
  none of them will not be redrawn — that is the trap `generation` exists for,
  and the restart bar walked straight into it.
- **The high-score path is not overridable**, so a `drive.py` run writes to the
  real data directory unless it is given `--seed` (never recorded) or a
  throwaway `HOME`. `HOME=/tmp/... tools/drive.py ...` is how A6 was checked.

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
  paragraph rather than in `app.rs` on its own.

## Commands

```
make check           # everything CI runs: fmt, clippy, test, release build
cargo check          # fast feedback
cargo test           # unit + integration
cargo clippy -- -D warnings
cargo fmt --check
cargo run --release  # play it
cargo run -- --print-config    # effective config
cargo run -- --seed 42         # deterministic run, not recorded to high scores
tools/drive.py c c             # drive the release binary on a pty
tools/drive.py --arg=--seed=42 --arg=--config=/tmp/t.toml esc down down enter
```

`HOME=/tmp/somewhere tools/drive.py ...` redirects both the §6.2 config path
and the §14 data path, which is how a full game can be played to a top out and
its score checked without touching the real files. A `--seed` run is never
recorded (§14) and so needs no such care.

`tools/drive.py` is the only way to check the terminal layer without a human at
a terminal: §17.1 is "core, no terminal" by design, so nothing in `cargo test`
reaches `main.rs`, `app.rs` or `ui/`. It drives the **release** binary on a pty,
sends a scripted burst of keys and replays the capture into a character grid,
printing one frame per keystroke. Two things to know: pass binary arguments through
`--arg`, glued on with `=` both times (`--arg=--seed=42`, `--arg=--config=...`)
or `argparse` claims them — without a seed every run is a different game and
only frames *within* one run may be compared; and it answers the §8.2
capability queries, so pass `--legacy` to exercise the fallback path. Its
docstring has the rest. It substitutes for, but
does not replace, playing the game on a real terminal.
