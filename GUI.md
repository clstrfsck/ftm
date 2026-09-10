# Falling Tetromino Manager — The egui Front-End

**Version:** 0 — reserved. No stage has filled it yet.
**Date:** 2026-09-10
**Companion to:** [FTM.md](FTM.md) (the specification),
[FRONTEND.md](FRONTEND.md) (the contract every front-end is written against),
[TUI.md](TUI.md) (the terminal front-end), [EGUI.md](EGUI.md) (the plan that
builds this one)

This document will be normative for the **egui front-end** — the `ftm-gui`
binary and the same code compiled to `wasm32-unknown-unknown` and served in a
browser. They are one front-end with two hosts, not two front-ends, so the
differences between them are called out in place rather than in a document of
their own.

**It is written stage by stage, not up front.** `EGUI.md`'s stages G5–G13 each
name the section they fill, and each fills it in the same commit as the code.
What exists today is the namespace and the reservations below — deliberately, so
that a `§G4` written in a doc comment during G7 has somewhere agreed to land.

## The `§G` namespace

`FTM.md`'s section numbers are stable and **do not move for this document**.
Several of them now live in `TUI.md` under the numbers they always had; this
document takes a fresh namespace so it can never collide with either. A future
`MACROQUAD.md` would take `§M`.

An unqualified `§n` here means `FTM.md` §n, or `TUI.md` §n for the five numbers
that document owns. `§Gn` means this file.

| § | Section | Filled by | Covers |
|---|---|---|---|
| G1 | The application | `EGUI.md` G5 | The `eframe` application, the pinned `egui` / `eframe` versions and the MSRV they set, the window, the loop that pumps the shell. |
| G2 | Input | `EGUI.md` G5 | The `egui` → `shell::keys` adapter (`FRONTEND.md` F5), repeats, focus loss, and the keys the browser wants for itself. |
| G3 | Layout | `EGUI.md` G7 | The integer-cell metric, `LAYOUT_COLS` / `LAYOUT_ROWS`, the minimum `cell` and the too-small state below it (`FRONTEND.md` F6, §8.4). |
| G4 | The playing screen | `EGUI.md` G7 | §12.4's information — field, hold, next, stats, status — drawn as pixels rather than characters, and `show_debug`. |
| G5 | Overlays | `EGUI.md` G8 | §12.6's pause, game-over and name-entry boxes, and the §13.5 Options and §10.1 controls panels. |
| G6 | Animations | `EGUI.md` G9, G10 | §12.5's six animations in a pixel-native idiom, and the sub-cell gravity that `GameView::fall_progress` makes drawable. |
| G7 | The attract screen | `EGUI.md` G11 | §13's wordmark, menu, cycling panel and drifting background, laid out for a window rather than a 36 × 20 grid. |
| G8 | The web build | `EGUI.md` G6, G12 | The canvas and its keyboard focus, `localStorage` for §6.2 and §14, URL query parameters in place of §6.4's flags, and the `[gui]` config table. |
| G9 | Testing and acceptance | `EGUI.md` G13 | The headless `egui_kittest` render test, and **B1–B12**, this front-end's answer to §17.3's A1–A10. |

## What already binds this front-end

None of this waits for a stage. Every rule below is normative now, in `FTM.md` or
in `FRONTEND.md`, and is repeated here only because a window and a browser tab
are where each is most likely to be broken.

- **The four capabilities** (`FRONTEND.md` F1–F4). The shell has no clock, no
  filesystem, no entropy and no calendar; this front-end supplies all four, twice
  — `host_native.rs` and `host_web.rs` — and the web host is expected to be the
  smaller of the two.
- **`advance` at any cadence** (§15.2 step 4). A window is repainted by the
  compositor, at 144 Hz, or not at all while its tab is backgrounded. The game
  advances by §15.1's tick and by nothing else; `stable_dt` is never a step size.
- **Only gravity is interpolated** (`FRONTEND.md`). Horizontal movement and
  rotation stay snapped to the cell. This is the rule that a framework with
  per-frame tweening in its idiom makes most tempting to break.
- **No pointer path** (§1.2). No click-to-select in a menu, in either build.
  Touch is an open decision in `EGUI.md` and a §1.2 amendment if it is taken.
- **The other front-end's config survives** (§6.2). `ftm` and `ftm-gui` share one
  file, and §12.3's colour depths and cell glyphs mean nothing here — so they are
  ignored and written back untouched, never dropped.
- **§1.3's trademark rules** cover the window title, the page title, the tab
  favicon and the wordmark, not only the text on a screen. A four-line clear is a
  `QUAD`.
- **The compiler holds both boundaries.** No module here may name a `core`
  internal (§17.3 A10, B12), and adding one to `gui/` must not be what makes
  `cargo check --no-default-features` fail.

## What does not carry over from `TUI.md`

Named here because the temptation is to port the terminal front-end's structure
wholesale and inherit its constraints for no benefit. `EGUI.md`'s *Hazards that
do not carry over* is the longer version.

- §15.2 step 5's frame comparison, and the generation counter behind it. They
  exist because `ratatui` diffs against a previous buffer. Immediate mode
  rebuilds every repaint, so there is nothing to compare and no "anything the
  screen comes to show must join the struct" hazard to inherit.
- §12.3's colour depths, `mono` and `$NO_COLOR`. Meaningless off a terminal.
- §12.2's cell glyphs, and the leaked `Glyphs` that keep `Theme` `Copy`. There
  are no glyphs, and leaking in a tab that is reloaded repeatedly is worse than
  leaking in a process that exits.
- §8.2's legacy key path, `HOLD_TIMEOUT` and `RESTART_QUIET`. `egui` always
  reports releases; this front-end is unconditionally in §8.2's enhanced case.
- §8.3's teardown and the panic hook. There is no terminal to restore: natively
  the error path is a window that fails to open, reported to stderr with a
  non-zero exit; on the web it is `console_error_panic_hook`.
- §12.1's 60 × 24 minimum. §G3 states this front-end's own, in cells rather than
  characters. §8.4's *rule* — the forced pause below it — is shared and has one
  implementation.
- §14's temp-file-and-rename. A filesystem technique, not a durability
  requirement; `localStorage` needs none.
