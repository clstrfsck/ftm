# Termino

A guideline-conformant falling-block game for the terminal, in Rust. Single
binary, no server, no unsafe.

**Status: Stage 5 of `PLAN.md` complete — milestone M1.** The core plays itself:
a scripted input log drives a complete game headlessly and deterministically,
with no terminal. `Game::tick(&TickInput, &mut Vec<GameEvent>)` is the single
entry point, `Game::view()` the only way to see the result. T1-T8, T11, T14-T17
and I1 pass, and the batch-invariance canary is in CI. There is no score, hold
or ghost yet — the view's fields for them are present but inert. Stage 6 (the
vertical slice: first playable) is next, and it is the highest-uncertainty stage
in the plan.

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
  60 exactly. Durations are integer ticks, converted once at config load.
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
- **DAS/ARR live in the shell**, not the core (§10.3).
- **T-spin**: the "last action was a rotation" flag must survive a hard drop, and
  kick test 5 always means a proper T-spin (§9.13). This is the most commonly
  botched rule in the spec.
- **Hold** clears its lock-out when the next piece **locks**, not when it spawns
  (§9.7).
- **Disabled keys** (`hold_enabled`, `allow_180_rotation` off) are dropped at the
  input boundary, so they cannot reset a lock-delay timer as a side effect
  (§10.1).

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

## Scope discipline

- §18 (other modes, sound, replays, themes, demo bot) and §19 (networking) are
  **not work items**. §19 is a list of constraints to honour, already baked into
  Stages 3, 5 and 8. The only networking deliverable in the entire plan is that
  one CI test.
- The attract screen (§13) is explicitly provisional. Build it plainly, look at
  it, then iterate. Do not gold-plate it before it has been seen.

## Open decisions

- **The `TETRIS` banner.** The game is called Termino and must not use the Tetris
  logo (§1.3), but "Tetris" is still the on-screen name of a four-line clear
  (§9.14, §12.4, §13.3). Renaming it to `QUAD` is a one-line change. Undecided —
  ask before changing it either way.

## Commands

```
make check           # everything CI runs: fmt, clippy, test, release build
cargo check          # fast feedback
cargo test           # unit + integration
cargo clippy -- -D warnings
cargo fmt --check
cargo run --release  # play it
cargo run -- --print-config    # effective config (from Stage 10)
cargo run -- --seed 42         # deterministic run, not recorded to high scores
```
