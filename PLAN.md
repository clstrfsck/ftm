# Falling Tetromino Manager — Staged Implementation Plan

**Companion to:** [FTM.md](FTM.md) (the specification)
**Date:** 2026-09-04

This plan sequences the work in `FTM.md` into twelve stages. Each stage ends
at a point where the tree builds, the tests pass, and something is demonstrably
better than it was — never at a half-finished refactor.

Section references (§) are to `FTM.md`. Test numbers (T1–T17) are the unit
test groups in §17.1; integration tests are I1–I4 from §17.2; acceptance criteria
are A1–A10 from §17.3.

---

## Contents

- [Sequencing rationale](#sequencing-rationale)
- [Working agreements](#working-agreements)
- [Milestones](#milestones)
- [Stage 0 — Skeleton and guardrails](#stage-0--skeleton-and-guardrails)
- [Stage 1 — Geometry, pieces, matrix](#stage-1--geometry-pieces-matrix)
- [Stage 2 — SRS rotation](#stage-2--srs-rotation)
- [Stage 3 — Config and timing](#stage-3--config-and-timing)
- [Stage 4 — The falling-piece core](#stage-4--the-falling-piece-core)
- [Stage 5 — View, events, determinism](#stage-5--view-events-determinism)
- [Stage 6 — Vertical slice: first playable](#stage-6--vertical-slice-first-playable)
- [Stage 7 — Scoring](#stage-7--scoring)
- [Stage 8 — Hold, ghost, preview queue](#stage-8--hold-ghost-preview-queue)
- [Stage 9 — The full playfield screen](#stage-9--the-full-playfield-screen)
- [Stage 10 — Config file, CLI, options](#stage-10--config-file-cli-options)
- [Stage 11 — Attract screen and high scores](#stage-11--attract-screen-and-high-scores)
- [Stage 12 — Robustness and acceptance](#stage-12--robustness-and-acceptance)
- [Test coverage map](#test-coverage-map)
- [Risk register](#risk-register)

---

## Sequencing rationale

Two forces pull in opposite directions.

**Core-first** is the natural reading of §3.1: the rules are pure, headless and
heavily specified, so they can be built and tested without a terminal. Building
them first means every rule is verified before any pixel is drawn.

**Slice-first** answers a different question: the riskiest unknowns in this
project are not the rules — those are pinned down to the kick table — but the
terminal layer. Whether the kitty keyboard protocol is available, whether the
fallback in §8.2 feels acceptable, and whether a 60 Hz tick loop with `event::poll`
actually behaves on the target terminals are all things no amount of specification
settles.

The plan therefore does core-first **up to the point where a piece can fall and
lock** (Stages 1–5), then immediately cuts a vertical slice to a playable,
featureless game (Stage 6) to retire the terminal risk early, and only then
returns to deepen the rules (Stages 7–8) and the presentation (Stages 9–11).

The one thing that is never deferred is determinism. The batch-invariance test
(§19.4) lands in Stage 5 and runs in CI from that commit onwards, because a
desync introduced in Stage 7 and discovered in Stage 12 is expensive, and the
same bug caught by a red test in Stage 7 is not.

---

## Working agreements

- **Tests before or with the code, never after the stage.** A stage is not done
  until its listed tests exist and pass.
- **`cargo clippy -- -D warnings` and `cargo fmt --check` are clean at every
  stage boundary.** Not at the end.
- `#![forbid(unsafe_code)]` from Stage 0.
- **The core never grows an I/O or clock dependency.** If a stage seems to need
  one, that is a design error in the stage, not a licence.
- **Commit granularity:** one commit per coherent change, each one building. A
  stage is typically 3–10 commits.
- **No speculative generality for §19.** Implement exactly what §19.2 requires
  and nothing more; the constraints are already stated, and honouring them costs
  nothing extra. Do not build a transport, a protocol or a server.
- When the spec and the code disagree, **the spec is wrong until it is amended.**
  Fix `FTM.md` in the same commit, and say so in the message.

---

## Milestones

| Milestone | After stage | Means |
|---|---|---|
| **M1 — Core plays itself** | 5 | A scripted input log drives a complete game headlessly, deterministically, with no terminal. |
| **M2 — First playable** | 6 | A human can move, rotate, drop and clear lines on screen. |
| **M3 — Feature complete** | 11 | Everything in §1.1 is implemented. |
| **M4 — Accepted** | 12 | All of §17.3 passes. |

---

## Stage 0 — Skeleton and guardrails

**Goal:** an empty project that already enforces the rules it will be judged by.

**Spec:** §3, §4

**Deliverables**

- `cargo init --bin ftm`; `Cargo.toml` with the §3 dependency table pinned to
  exact minor versions.
- The full §4 module tree, every file present with a doc comment naming its
  spec section and a `// TODO(stage N)` marker.
- `#![forbid(unsafe_code)]` in `main.rs`.
- CI (or a `just check` / `make check` target) running `fmt --check`, `clippy -D
  warnings`, `test`, `build --release`.
- `README.md`: one paragraph, a build line, and a pointer to `FTM.md`.

**Exit criteria**

- `cargo run` prints the version and exits 0.
- CI is green on an empty test suite.

**Not yet:** any game logic, any terminal code.

**Size:** S

---

## Stage 1 — Geometry, pieces, matrix

**Goal:** the static data of the game, exactly as specified, with no behaviour.

**Spec:** §5, §9.1, §9.2, §9.3, §9.4

**Deliverables**

- `core/geometry.rs`: `Point`, `Rotation` (`North`/`East`/`South`/`West` with
  `cw()`, `ccw()`, `opposite()`), the y-down convention documented at the top of
  the file with a pointer to §5.
- `core/piece.rs`: `PieceKind`, the 4 × 7 orientation patterns from §9.3, spawn
  origins from §9.4, colours from §9.2.
- `core/matrix.rs`: 10 × 40 storage, `is_filled`, `collides(piece, origin,
  rotation)`, out-of-bounds treated as solid on left/right/bottom and empty above
  row 0.

**Exit criteria**

- **T1** passes: four minos in every orientation; four clockwise rotations
  restore the original; spawn coordinates match the §9.4 table cell for cell.
- The §9.4 spawn table is asserted literally, not recomputed by the same code
  that produces it — a transcription bug here poisons everything downstream.

**Not yet:** rotation with kicks, gravity, any mutation of the matrix.

**Size:** M

---

## Stage 2 — SRS rotation

**Goal:** rotation with wall kicks, isolated and verified before anything depends
on it.

**Spec:** §9.5

**Deliverables**

- `core/srs.rs`: both kick tables, transcribed from §9.5 **as written** (they are
  already converted to y-down — do not negate them again; put that sentence in
  the file).
- `try_rotate(matrix, piece, from, to) -> Option<(Point, u8)>` returning the new
  origin and the **kick index used** — the index is required by §9.13 and is easy
  to forget to thread through.
- 180° rotation as a separate path with no kick tests, gated on
  `allow_180_rotation`.

**Exit criteria**

- **T2**, **T3** pass, including the T-spin-triple kick and the I-piece against
  the left wall.
- A property test: for every piece, orientation and empty-board position, a
  rotation followed by its inverse returns the original origin.

**Not yet:** T-spin classification (needs the lock context).

**Size:** M

---

## Stage 3 — Config and timing

**Goal:** the two config structs and the tick conversion, before any code needs
to guess at a duration.

**Spec:** §6.3, §6.5, §6.6, §15.1

**Deliverables**

- `config.rs`: `RulesConfig` and `PresentationConfig` as **separate structs**
  (§6.5), both `Serialize + Deserialize`, `RulesConfig: PartialEq`.
- Defaults matching §6.3 exactly.
- `ms_to_ticks` with both rounding rules from §6.6, and the derived tick counts
  stored in `RulesConfig` so the core never sees milliseconds.
- `TICK_HZ`, `TICK`, `MAX_CATCH_UP_TICKS` constants.

**Exit criteria**

- **T17** passes, including the §6.6 table and the `lock_delay_ms = 1 → 1 tick`
  edge case.
- Defaults round-trip through TOML unchanged (part of **I2**).

**Not yet:** reading or writing the config file, CLI parsing. This stage is the
data model only.

**Size:** S

---

## Stage 4 — The falling-piece core

**Goal:** a piece spawns, falls, is controllable, locks, and clears lines. No
score, no hold, no ghost.

**Spec:** §9.6, §9.9, §9.10, §9.11, §9.12, §9.16

**Deliverables**

- `core/bag.rs`: 7-bag with `SmallRng`, the first-piece `S`/`Z` swap, the queue
  kept filled to `preview_count + 1`.
- `core/gravity.rs`: `fall_period` in 16.16 ticks-per-row (§9.9), the integer
  accumulator, the soft-drop divisor, level progression.
- `core/lockdown.rs`: extended placement with the 15-reset cap and the
  `lowest_row_reached` rule, plus the `infinite` and `classic` variants.
- `core/game.rs`: `Game` state, spawn with the immediate one-row drop, movement,
  hard drop, line clear with the clear/entry delay states, Block Out and Lock Out.

**Exit criteria**

- **T4**, **T5**, **T6**, **T7**, **T8**, **T11** pass.
- The three lock-down variants are each tested; the "grounded piece moved into
  mid-air at timer expiry does not lock" case is explicitly covered.
- A hand-driven test plays twenty pieces into a known board state.

**Not yet:** scoring, T-spins, hold, ghost, view, events.

**Size:** L — the largest single stage. Split the commits by file.

---

## Stage 5 — View, events, determinism

**Goal:** the core's public contract, and the test that protects it forever.
**This is milestone M1.**

**Spec:** §3.1, §12.7, §12.8, §15.1, §15.4, §19.4

**Deliverables**

- `core/view.rs`: `GameView`, `PieceView`, `Game::view(&self)` — clipping to the
  visible rows happens here, not in the renderer.
- `core/events.rs`: `GameEvent`, emitted in rules order, allocation-free in the
  common case.
- `core/game.rs`: `Game::tick(&TickInput, &mut Vec<GameEvent>)` as the single
  entry point. Everything from Stage 4 moves behind it.
- A headless harness in `tests/`: `RulesConfig` + seed + a `Vec<TickInput>` in,
  final `GameView` out.

**Exit criteria**

- **T14**, **T15**, **T16** pass.
- **I1** passes, *including the batch-invariance half*: the same log fed as
  1 × 6 ticks and as 6 × 1 ticks produces byte-identical snapshots. Wire this
  into CI now (§19.4).
- Dropping every event leaves the final state bit-identical (T16).
- `Game::view` is `&self` and provably non-mutating.

**Not yet:** anything that draws.

**Size:** M

---

## Stage 6 — Vertical slice: first playable

**Goal:** the ugliest possible playable game, to retire the terminal risk.
**This is milestone M2.**

**Spec:** §8, §10.2, §10.3, §12.1, §12.2, §15.2

**Deliverables**

- `main.rs`: terminal setup/teardown in the §8.1/§8.3 order, panic hook that
  restores the terminal **first**.
- Keyboard enhancement detection and the §8.2 legacy fallback, both paths
  implemented — not one path with a TODO.
- `input.rs`: key → `Action` / `Held`, DAS/ARR per §10.3 resolved in the shell.
- `app.rs`: the §15.2 loop with the accumulator, catch-up cap and arrears
  discard.
- `ui/cells.rs` and a bare playfield: border, cells, current piece. Fixed colours,
  no hold box, no next box, no stats.

**Exit criteria**

- A human can play: move, rotate, soft drop, hard drop, clear lines, top out.
- **T13** passes.
- Verified by hand on at least two terminals, one with the kitty protocol
  (Kitty, WezTerm, Ghostty) and one without (Terminal.app, or `TERM=xterm`), and
  the legacy path's feel is noted in the commit message.
- Quitting restores the terminal (`stty -a` before and after — this is **A7**,
  worth checking now rather than at the end).

**Not yet:** score, hold, ghost, next, animations, menus, config files.

**Size:** L — and the highest-uncertainty stage. Timebox the terminal
experimentation; if the legacy fallback feels unacceptable, that is a finding to
take back to §8.2, not something to fix by tuning constants forever.

---

## Stage 7 — Scoring

**Goal:** every number in §9.14 correct.

**Spec:** §9.13, §9.14, §9.15

**Deliverables**

- `core/tspin.rs`: the three-corner rule, front/back corners per orientation, the
  kick-index-5 override, and the "last action was a rotation" precondition —
  which means the last-action flag must survive a hard drop (§9.13).
- `core/scoring.rs`: the full table, B2B chain, combo counter, perfect clear,
  soft/hard drop points.
- `GameEvent::LinesCleared` and `ScoreAwarded` populated properly.

**Exit criteria**

- **T9**, **T10** pass. Build the canonical T-spin single/double/triple and mini
  set-ups as board fixtures; each one is a named test.
- The negative cases are tested: a T *moved* into a three-corner slot scores
  nothing; a non-T piece never sets T-spin status.
- **I1**'s snapshot is regenerated and the batch-invariance test still passes.

**Not yet:** displaying any of it beyond a debug line.

**Size:** M

---

## Stage 8 — Hold, ghost, preview queue

**Goal:** the three remaining gameplay mechanics, each individually switchable.

**Spec:** §9.7, §9.8, §6.3 (`preview_count`), §9.5 (180 gate)

**Deliverables**

- Hold with the once-per-piece lock-out cleared on the next **lock** (not spawn),
  Block Out on an obstructed swap, and the `hold_enabled` gate.
- Ghost computed on every change, present in `GameView` as `Option<PieceView>`.
- `next` in the view sized exactly to `preview_count`, clamped 1..=6.
- The `hold_enabled` / `allow_180_rotation` gates enforced **at the input
  boundary** so a disabled key cannot reset a lock timer (§10.1).

**Exit criteria**

- **T12** passes.
- A test that `preview_count` = 1 and = 6 both produce a correctly sized view and
  do not disturb the piece sequence.

**Size:** M

---

## Stage 9 — The full playfield screen

**Goal:** the screen in §12.4, drawn from `GameView` alone.

**Spec:** §12.3, §12.4, §12.5, §12.6, §12.8

**Deliverables**

- `ui/theme.rs`: the §9.2 palette and the four colour depths.
- `ui/playfield.rs`: the 44 × 23 layout — hold box, playfield, next box with
  per-slot dimming, stats box, status line. Match the mock-up in §12.4 exactly;
  it is drawn to scale for this purpose.
- `ui/overlays.rs`: pause (with the 3-2-1 resume countdown), game over.
- `ui/mod.rs`: the §12.5 animations, each started by a `GameEvent` and timed on
  the wall clock.

**Exit criteria**

- The rendered screen matches the §12.4 mock-up character for character on an
  empty board.
- All six animations in §12.5 fire from events; none of them touches `Game`.
- Pause blanks the playfield (§9.17) and stops the clock.

**Not yet:** attract screen, high scores, name entry.

**Size:** L

---

## Stage 10 — Config file, CLI, options

**Goal:** everything in §6 reachable without a text editor.

**Spec:** §6.1, §6.2, §6.4, §13.5 (options panel), §16, and the two `[display]`
settings Stage 9 could not reach: §12.2 (cell glyphs) and §12.4 (the debug
stats box).

**Deliverables**

- TOML load/save at the §6.2 path, the commented default file written on first
  clean exit, malformed-file tolerance with a warning collected for exit.
- `clap` CLI per §6.4, including the paired `--hold/--no-hold` and
  `--rot180/--no-rot180` overrides, and `--print-config`.
- The warning vector and the §16 error-handling contract, including the
  print-after-teardown rule.
- The Options panel (§13.5) writing back on exit.
- The configured `cell_filled` / `cell_empty` / `cell_ghost` glyphs, rejected
  with a warning and replaced by the default unless they are exactly two display
  columns wide (§12.2). The check belongs to the loader, not the renderer —
  `ui::cells` assumes two columns everywhere.
- **The §12.4 debug stats box**, which `show_debug` turns on and which nothing
  could reach until this stage. Frame rate, ticks elapsed, dropped ticks and the
  input mode are the shell's own; gravity in G, `fall_period`, lock-delay ticks
  remaining and the bag contents are **core state that `GameView` does not
  carry**. Decide deliberately how they get out: a `debug: Option<DebugView>`
  on the view is the obvious answer, and it widens §19's wire format, so it is a
  design decision rather than a plumbing job. Do not let the renderer reach into
  `Game` for them (§12.7) — that is the one answer the layering rule forbids.

**Exit criteria**

- **I2**, **I3** pass.
- **A5** passes: `preview_count` honoured for all six values from both the file
  and the flag.
- A malformed config produces exactly one warning and a playable game.
- Every `[display]` setting changes the screen: `show_grid`, `show_debug`, each
  of the three glyphs, and `color_depth` in all five of its values.
- A glyph that is not two columns wide is rejected and the field stays a
  rectangle.

**Size:** M

---

## Stage 11 — Attract screen and high scores

**Goal:** the program has a front door. **This is milestone M3.**

**Spec:** §7, §13, §14, §12.6 (name entry)

**Deliverables**

- `ui/attract.rs`: wordmark, menu, the 6-second cycling panel, the drifting
  background at 10 fps (§15.3), the 60-second idle colour cycle.
- The controls panel hiding unavailable bindings (§13.3).
- Sub-screens: high scores, controls, options.
- `highscore.rs`: top ten, atomic write via temp + rename, the qualification
  rule, seeded runs excluded.
- Name entry per §12.6, `$USER` pre-fill.
- The full §7 state machine wired up, replacing whatever Stage 6 had.

**Exit criteria**

- **A3**, **A6** pass: attract on launch, PLAY starts a game, a completed game
  records a score that appears on the attract panel.
- Idle CPU on the attract screen under 2 % (§15.3), measured.

**Note:** §13 is explicitly provisional. Expect to revise it once it is on
screen; that is what it is for. Do not gold-plate it before it has been seen.

**Size:** L

---

## Stage 12 — Robustness and acceptance

**Goal:** sign off §17.3.

**Spec:** §8.4, §12.1, §12.3, §16, §17.3

**Deliverables**

- Resize handling, including the force-to-pause rule (§8.4).
- The terminal-too-small screen (§12.1).
- `mono` and `NO_COLOR` paths finished and played through.
- Every §16 failure path exercised deliberately: unwritable config, unwritable
  high scores, panic mid-frame.

**Exit criteria**

- **I4** passes (render at 60 × 24, 80 × 24, 200 × 60, 1 × 1 without panicking).
- **A1**–**A10** all pass, checked off one by one. **A10** in particular: strip
  the core's public surface to `tick`, `view` and the view/event types and
  confirm the UI still builds — this is the §19 guarantee, and Stage 12 is the
  last honest chance to verify it.

**Size:** M

---

## Test coverage map

Every test in §17 has an owning stage. No test is left to "the end".

| Test | What | Stage |
|---|---|---|
| T1 | Piece geometry and spawn table | 1 |
| T2, T3 | Kick tables, wall kicks in practice | 2 |
| T4 | 7-bag | 4 |
| T5, T6 | Gravity curve, gravity arithmetic | 4 |
| T7 | Lock down, all three variants | 4 |
| T8 | Line clears | 4 |
| T9 | T-spin detection | 7 |
| T10 | Scoring, B2B, combo, perfect clear | 7 |
| T11 | Block Out and Lock Out | 4 |
| T12 | Optional mechanics off/on | 8 |
| T13 | DAS/ARR | 6 |
| T14 | Determinism | 5 |
| T15 | View model | 5 |
| T16 | Event stream | 5 |
| T17 | Millisecond conversion | 3 |
| I1 | Scripted game + batch invariance | 5 (extended in 7) |
| I2, I3 | Config round-trip, malformed config | 10 |
| I4 | Render at four sizes | 12 |
| A1–A2 | Build clean, tests pass | continuous |
| A3, A6 | Attract, full game recorded | 11 |
| A4 | Controls | 6 |
| A5 | Preview count | 10 |
| A7 | Terminal restored | 6 (re-verified in 12) |
| A8 | mono / NO_COLOR | 12 |
| A9 | Hold and 180 toggles | 8 |
| A10 | UI builds against the view alone | 12 |

---

## Risk register

| Risk | Stage | Impact | Mitigation |
|---|---|---|---|
| The legacy key fallback (§8.2) feels bad — soft drop overshoot, sticky DAS | 6 | Game is unpleasant on common terminals | Implement it in the same stage as the enhanced path and play both. If it is unacceptable, amend §8.2 rather than tuning constants indefinitely; consider requiring the enhanced protocol and saying so on the attract screen. |
| Kick-table transcription error | 2 | Wrong rotation in rare positions; very hard to spot later | Transcribe once, assert the tables' shape, and test the two set-ups whose results are unmistakable (I against the wall, T-spin triple). |
| Determinism regression from an innocent change | 7–11 | Forecloses §19 silently | Batch-invariance test in CI from Stage 5. It fails loudly and immediately. |
| T-spin "last action was a rotation" flag lost through hard drop | 7 | T-spins silently score as ordinary clears | Named test for exactly that path; it is the one everybody gets wrong. |
| §12.4 layout does not fit real terminals at 60 × 24 | 9 | Layout rework late | The mock-up is drawn to scale and was width-checked; verify at 60 × 24 in Stage 6, before the full layout is built on top of it. |
| Attract screen churn | 11 | Wasted effort on a provisional design | Build it plainly, look at it, then iterate. Do not polish before first sight. |
| Scope creep toward networking | any | Delays a working single-player game | §19 is a constraint list, not a work item. The only networking deliverable in this plan is the batch-invariance test. |

---

## What this plan deliberately excludes

Everything in §18: additional game modes, sound, replays, per-piece statistics,
colour themes, and the self-playing attract demo. And everything in §19 beyond
the constraints already baked into Stages 3, 5 and 8 — no transport, no protocol,
no server. Both lists are worth revisiting only after M4.
