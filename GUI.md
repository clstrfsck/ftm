# Falling Tetromino Manager — The egui Front-End

**Version:** 0.1 — §G1 and §G2 are written; §G3–§G9 are still reserved.
**Date:** 2026-09-11
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
§G1 and §G2 are written, by G5; the rest is still the namespace and the
reservations below — deliberately, so that a `§G4` written in a doc comment
during G7 has somewhere agreed to land.

## The `§G` namespace

`FTM.md`'s section numbers are stable and **do not move for this document**.
Several of them now live in `TUI.md` under the numbers they always had; this
document takes a fresh namespace so it can never collide with either. A future
`MACROQUAD.md` would take `§M`.

An unqualified `§n` here means `FTM.md` §n, or `TUI.md` §n for the five numbers
that document owns. `§Gn` means this file.

| § | Section | Filled by | Covers |
|---|---|---|---|
| G1 | The application | `EGUI.md` G5 ✅ | The `eframe` application, the pinned `egui` / `eframe` versions and the MSRV they set, the window, the loop that pumps the shell. |
| G2 | Input | `EGUI.md` G5 ✅ | The `egui` → `shell::keys` adapter (`FRONTEND.md` F5), repeats, focus loss, and the keys the browser wants for itself. |
| G3 | Layout | `EGUI.md` G7 | The integer-cell metric, `LAYOUT_COLS` / `LAYOUT_ROWS`, the minimum `cell` and the too-small state below it (`FRONTEND.md` F6, §8.4). |
| G4 | The playing screen | `EGUI.md` G7 | §12.4's information — field, hold, next, stats, status — drawn as pixels rather than characters, and `show_debug`. |
| G5 | Overlays | `EGUI.md` G8 | §12.6's pause, game-over and name-entry boxes, and the §13.5 Options and §10.1 controls panels. |
| G6 | Animations | `EGUI.md` G9, G10 | §12.5's six animations in a pixel-native idiom, and the sub-cell gravity that `GameView::fall_progress` makes drawable. |
| G7 | The attract screen | `EGUI.md` G11 | §13's wordmark, menu, cycling panel and drifting background, laid out for a window rather than a 36 × 20 grid. |
| G8 | The web build | `EGUI.md` G6, G12 | The canvas and its keyboard focus, `localStorage` for §6.2 and §14, URL query parameters in place of §6.4's flags, and the `[gui]` config table. |
| G9 | Testing and acceptance | `EGUI.md` G13 | The headless `egui_kittest` render test, and **B1–B12**, this front-end's answer to §17.3's A1–A10. |

---

## G1. The application

### G1.1 The dependency pin, and the MSRV it sets

`egui` and `eframe` are pinned at **0.36** and move together. `egui` breaks its
API across minor versions more freely than the project's other dependencies do,
and `eframe`, `egui_kittest` (§G9) and the `web-sys` the wasm build needs (§G8)
are all versioned against it, so the pin is one decision rather than four.

That pin sets the project's MSRV, which is **1.95** — `egui`'s own floor. §3
already says the floor is set by a dependency and moves when one moves; `ratatui`
is no longer the binding constraint. The three places that state it —
`rust-version` in `Cargo.toml`, §3, and the `msrv` job in CI — change together or
the job stops checking what it claims to.

`eframe` is taken with `default-features = false` and the **`glow`** backend
rather than the default `wgpu`. What this front-end draws is rectangles; OpenGL
is the smaller of the two and reaches a browser as WebGL2 without a WebGPU
fallback path (§G8). `default_fonts` is kept because there is text to draw from
§G4 onward; `x11` and `wayland` are kept so a Linux build is not a surprise at
G13. `accesskit` is not: §1.2 makes the game keyboard-driven and there is no
widget tree for a screen reader to walk.

### G1.2 The window

One window, one viewport. Its title is `Falling Tetromino Manager` — §1.3's
trademark rules reach the window title, the page title and the tab favicon as
well as the text on a screen, so it is the game's own name and nothing else's.
The window is **resizable**, and has no fixed aspect: §G3 states what happens as
it shrinks.

There is no §8.3 teardown and no panic hook. Those exist to give a terminal back
its mode before a backtrace is printed, and a window holds nothing in that state;
natively the error path is a window that fails to open, reported on stderr with a
non-zero exit, and on the web it is `console_error_panic_hook` (§G8). §16's
warnings are printed on stderr **after** the window has closed, which is this
front-end's reading of §16's "after teardown".

### G1.3 The loop

`eframe` calls the application and the application asks to be called again, so
§15.2's seven steps are **not** a loop here — they are calls into
`shell::round::Round`, which `EGUI.md` G4 made a front-end's to drive
(`FRONTEND.md` F7).

`eframe` 0.36 splits its callback in two, and the split lands where §15.2's does:

| `eframe` | §15.2 | What it does |
|---|---|---|
| `App::logic(ctx, frame)` | steps 1–4, 6–7 | Drains `ctx.input(…).events` into `Round::key`, calls `Round::advance`, then `ctx.request_repaint_after(Round::deadline(now))`. |
| `App::ui(ui, frame)` | step 5 | Paints what the last `Round::frame` reported. |

Three rules bind that, and each is `FTM.md`'s rather than this document's:

1. **The game advances by §15.1's tick and by nothing else.** `stable_dt` is
   never a step size and frames are never counted. `App::logic` is called while
   the window is hidden and `App::ui` is not, so a hidden window keeps playing
   and simply is not drawn — which is §G8's backgrounded tab arriving early, and
   §15.2 step 4's catch-up cap is what stops it resuming into an instant death.
2. **`deadline` is advice, not a frame rate** (§15.2 step 6). The compositor may
   call back sooner, a key will, and `advance` is correct either way.
3. **There is no frame comparison.** §15.2 step 5's decision to skip an unchanged
   draw exists because `ratatui` diffs against a previous buffer; immediate mode
   rebuilds every repaint, so this front-end draws unconditionally and
   `Session::generation` is ignored.

### G1.4 What G5 built, and what it did not

The G5 slice draws the locked cells of `GameView::rows` and the falling piece, as
rectangles in §12.3's levelled palette, over a well centred in the window. A
non-`None` `Overlay` darkens the well and nothing more — §12.6's boxes are §G5's
and arrive at `EGUI.md` G8; until then the scrim is what says a game is not
running, so that a paused window does not merely look frozen. There is no hold
box, no next queue, no stats, no ghost and no grid: §12.4's information is §G4's
and arrives at G7, over §G3's metric.

§7's screen field has one arm. A game that hands back `Next::Play` — §10.1's held
restart — starts a fresh one; `Next::Attract` and `Next::Quit` both close the
window, because there is no attract screen to return to until G11.

---

## G2. Input

### G2.1 The adapter

`gui/keys.rs` is the only module in this front-end that may name an `egui` key
type, exactly as `tui/keys.rs` is for crossterm's (`FRONTEND.md` F5). What
crosses it upward is `shell::keys::KeyEvent` and nothing else, so a `[keys]`
table written by either binary means the same thing in both (§10.1, §6.2).

The mapping is §10.1's names, one for one: the four arrows, `Enter`, `Tab`,
`Escape`, `Backspace`, `F1`–`F12`, and `egui::Key::Space` as `Char(' ')` —
because §10.1 spells it `Space` and `parse_key` resolves that to a blank.
Everything else is §10.1's "single character" case. The letters and the digits
come from `egui::Key::name`, which is a single character for exactly those; the
**punctuation needs an explicit table**, because `name` spells those out
(`"Minus"`, `"Colon"`) and a `[keys]` entry of `-` must mean the same key in
both binaries (§6.2). A key with no §10.1 name at all — `F13`, `PageUp`,
`BrowserBack` — is **dropped at the adapter**, which is why §13.6 can say "any
key the game can *name*".

`egui` reports a key rather than a character, so the adapter decides a *letter's*
case from the shift modifier: `Z` with shift, `z` without, which is what a
terminal sends and what the default bindings list both halves of. Nothing else
consults it. `egui` has its own variant for each shifted punctuation key it can
reach — `Questionmark`, `Pipe`, `Plus` — so those carry their character
directly; but it has none for a shifted **digit**, so `Shift`+`1` arrives as
`Num1` and not as `!`. A player who binds a shifted digit can press it in the
terminal and not in the window. That is a known and accepted gap, not a defect
to work around by reading `Event::Text`.

The invariant that holds all of this together is testable and is tested: every
`Char(c)` this adapter can produce must be the key `parse_key` resolves `c` to.

`Event::Text` is deliberately unused. §12.6's name entry has rules of its own —
twelve printable ASCII, `ANON` when empty — and they belong to `shell::menus`,
where both front-ends reach them.

### G2.2 Repeats, and why there is no legacy path

This front-end is **unconditionally §8.2's enhanced case**. `egui` reports a true
release for every press and flags the operating system's auto-repeat, so
`InputState` is constructed with `InputMode::Enhanced` always, repeats are
carried across as `KeyKind::Repeat` and discarded above, and DAS is driven by the
clock alone (§10.3). §8.2's legacy path, `HOLD_TIMEOUT` and `RESTART_QUIET` are
never reached from here, and `--legacy` has no meaning in this front-end.

### G2.3 Focus

A window that loses focus stops being sent key events, and the release of a key
let go outside it never arrives — so a held direction would still be charging
DAS when focus came back. The adapter therefore **remembers what it has reported
as held and synthesises the releases itself** on `Event::WindowFocused(false)`.

That is legitimate rather than a workaround: F5 makes the event stream
synthesisable precisely so that a front-end may manufacture it, and a polled
front-end will manufacture the whole of it. A repeat does not enqueue a second
release, and a key released normally is not released twice.

Focus loss does **not** pause the game. §8.4's forced pause is about a viewport
that cannot host the screen, and a window behind another one still can; §9.17's
pause is the player's. A game left unattended tops out, which is what it does in
a terminal whose window is behind another one.

### G2.4 The keys the browser wants for itself

Not yet normative: the canvas, its focus, and the `preventDefault` that stops
`Space` scrolling the page and `Tab` moving focus are §G8's, and land with the
web build at `EGUI.md` G6. There is no pointer path in either build (§1.2).

---

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
