# Falling Tetromino Manager — The egui Front-End

**Version:** 1.0 — §G1-§G9 are written; B1-B12 are signed off.
**Date:** 2026-09-12
**Companion to:** [FTM.md](FTM.md) (the specification),
[FRONTEND.md](FRONTEND.md) (the contract every front-end is written against),
[TUI.md](TUI.md) (the terminal front-end), [EGUI-PLAN.md](EGUI-PLAN.md) (the plan that
builds this one)

This document is normative for the **egui front-end** — the `ftm-gui`
binary and the same code compiled to `wasm32-unknown-unknown` and served in a
browser. They are one front-end with two hosts, not two front-ends, so the
differences between them are called out in place rather than in a document of
their own.

**It was written stage by stage, not up front.** `EGUI-PLAN.md`'s stages G5–G13
each named the section they filled, and each filled it in the same commit as the
code. §G1 and §G2 were written by G5, §G8's web build by G6, §G3 and §G4 by G7,
§G5 by G8, §G6's animations by G9 and its sub-cell half by G10, §G7 by G11,
§G8.10 and §G8.11 — the `[gui]` table and which flag exists in which build — by
G12, and §G9 by G13, which is where B1–B12 were signed off and the document
became whole.

## The `§G` namespace

`FTM.md`'s section numbers are stable and **do not move for this document**.
Several of them now live in `TUI.md` under the numbers they always had; this
document takes a fresh namespace so it can never collide with either. A future
`MACROQUAD.md` would take `§M`.

An unqualified `§n` here means `FTM.md` §n, or `TUI.md` §n for the five numbers
that document owns. `§Gn` means this file, and `§Pn` `PILOT.md`.

| § | Section | Filled by | Covers |
|---|---|---|---|
| G1 | The application | `EGUI-PLAN.md` G5 ✅ | The `eframe` application, the pinned `egui` / `eframe` versions and the MSRV they set, the window, the loop that pumps the shell. |
| G2 | Input | `EGUI-PLAN.md` G5 ✅ | The `egui` → `shell::keys` adapter (`FRONTEND.md` F5), repeats, focus loss, and the keys the browser wants for itself. |
| G3 | Layout | `EGUI-PLAN.md` G7 ✅ | The integer-cell metric, `LAYOUT_COLS` / `LAYOUT_ROWS`, the minimum `cell` and the too-small state below it (`FRONTEND.md` F6, §8.4). |
| G4 | The playing screen | `EGUI-PLAN.md` G7 ✅ | §12.4's information — field, hold, next, stats, status — drawn as pixels rather than characters, `show_debug`, and the pause that losing the keyboard forces. |
| G5 | Overlays | `EGUI-PLAN.md` G8 ✅ | §12.6's pause, game-over and name-entry boxes, and the §13.5 Options and §10.1 controls panels. |
| G6 | Animations | `EGUI-PLAN.md` G9 ✅, G10 ✅ | §12.5's six animations in a pixel-native idiom, and the sub-cell gravity that `GameView::fall_progress` makes drawable. |
| G7 | The attract screen | `EGUI-PLAN.md` G11 ✅ | §13's wordmark, menu, cycling panel and drifting background, laid out for a window rather than a 36 × 20 grid. |
| G8 | The web build, and the `[gui]` table | `EGUI-PLAN.md` G6 ✅, G12 ✅ | The canvas and its keyboard focus, `localStorage` for §6.2 and §14, URL query parameters in place of §6.4's flags, this front-end's own config table, and which flag exists in which build. |
| G9 | Testing and acceptance | `EGUI-PLAN.md` G13 ✅ | The headless render test, what stands in for `tools/drive.py`, and **B1–B12**, this front-end's answer to §17.3's A1–A10. |

---

## G1. The application

### G1.1 The dependency pin, and the MSRV it sets

`egui` and `eframe` are pinned at **0.36** and move together. `egui` breaks its
API across minor versions more freely than the project's other dependencies do,
and `eframe` and the `web-sys` the wasm build needs (§G8) are both versioned
against it, so the pin is one decision rather than three. (`egui_kittest` would
have been a fourth; G13 did not take it — §G9.1 says why.)

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
non-zero exit, and on the web it is `eframe::WebRunner`'s own panic hook (§G8.5).
Natively, §16's warnings are printed on stderr **after** the window has closed,
which is this front-end's reading of §16's "after teardown"; a tab has no such
moment, and reports them on the console as they arise (§G8.6).

### G1.3 The loop

`eframe` calls the application and the application asks to be called again, so
§15.2's seven steps are **not** a loop here — they are calls into
`shell::round::Round`, which `EGUI-PLAN.md` G4 made a front-end's to drive
(`FRONTEND.md` F7).

`eframe` 0.36 splits its callback in two, and the split lands where §15.2's does:

| `eframe` | §15.2 | What it does |
|---|---|---|
| `App::logic(ctx, frame)` | steps 1–4, 6–7 | Drains `ctx.input(…).events` into `Round::key`, tells the round whether the viewport fits (§G3.3) and whether the keyboard is heard (§G4.7), calls `Round::advance`, then `ctx.request_repaint_after(Round::deadline(now))`. |
| `App::ui(ui, frame)` | step 5 | Paints what the last `Round::frame` reported. |

Three rules bind that, and each is `FTM.md`'s rather than this document's:

1. **The game advances by §15.1's tick and by nothing else.** `stable_dt` is
   never a step size and frames are never counted. `App::logic` is called while
   the window is hidden and `App::ui` is not, so a hidden window keeps pumping
   and simply is not drawn. A backgrounded tab is the same case at a throttled
   cadence (§G8.7), and §15.2 step 4's catch-up cap is what stops either from
   resuming into an instant death. (Since `EGUI-PLAN.md` G7 a *game* in either is
   paused rather than played, because neither has the keyboard — §G4.7. The
   pump runs regardless, and has to be correct at that cadence.)
2. **`deadline` is advice, not a frame rate** (§15.2 step 6). The compositor may
   call back sooner, a key will, and `advance` is correct either way.
3. **There is no frame comparison.** §15.2 step 5's decision to skip an unchanged
   draw exists because `ratatui` diffs against a previous buffer; immediate mode
   rebuilds every repaint, so this front-end draws unconditionally and
   `Session::generation` is ignored.

### G1.4 What G5 built, and what it did not

The G5 slice drew the locked cells of `GameView::rows` and the falling piece, as
rectangles in §12.3's levelled palette, over a well centred in the window. A
non-`None` `Overlay` darkened the well and nothing more; there was no hold box,
no next queue, no stats, no ghost and no grid. §12.4's information arrived at
G7 (§G3, §G4) and §12.6's boxes at G8 (§G5), which is what the scrim was
standing in for.

§7's screen field had one arm, and since G11 it has two (§G7.5). A game that
hands back `Next::Play` — §10.1's held restart — starts a fresh one;
`Next::Attract` goes to §13's screen, which at G5 did not exist, so both it and
`Next::Quit` closed the window instead.

A window or a canvas that does not have the keyboard says so, across the middle
of the screen: **Click to play** (§G8.2). It is drawn in both builds, over
whatever else is on screen. At G5 it paused nothing; since G7 losing the keyboard
pauses a game in progress (§G4.7).

**G7 replaced the slice's screen** with §G3's layout and §G4's playing screen,
**G8 put §G5's boxes over it**, **G9 §G6's animations under them**, **G10
§G6.5's sub-cell gravity** and **G11 §G7's attract screen beside the lot**.
**G12 gave both binaries one config file and §6.4 per build**, and **G13 signed
the whole thing off** (§G9). Nothing of this document is reserved any more.

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

Focus loss also **pauses a game in progress**, which is §G4.7's rule and is
amended here from what G5 wrote: that §8.4's forced pause was about a viewport
alone, and a game left unattended should top out as it would in a terminal behind
another window. G6 measured what that meant in a hidden tab (§G8.7) — a game
that went on placing pieces for its absent player — and `EGUI-PLAN.md` G7 settled it
the other way. The adapter's part is unchanged: it synthesises the releases, and
the pause that follows them is the shell's.

### G2.4 The keys the browser wants for itself

A browser tab puts a page between the keyboard and the game, and the page wants
`Space`, the arrows, `Tab` and `Backspace` for itself. §G8.2 is normative for
what the web build does about it: the canvas takes the keyboard on load, those
keys are kept from the page for as long as it has it, and it says so when it
has not. The adapter above is unchanged by any of it — the keys reach it as
they do natively. There is no pointer path in either build (§1.2).

---

## G3. Layout

### G3.1 The cell

Everything on the playing screen is measured in one unit, the **cell** — one
mino of the well — and the cell is derived from the viewport by one rule:

```
cell = floor(min(width / LAYOUT_COLS, height / LAYOUT_ROWS))
```

taken in **physical pixels**, with the block centred on a whole pixel. Every
rect on the screen — the well, each panel, each mino — is then a whole number of
pixels at a whole-pixel position, at any window size and any display density,
which is what keeps the grid crisp without fighting `egui`'s float coordinates.
A resize scales the whole screen rather than reflowing it, and what the viewport
has beyond the block is margin, exactly as §12.1 has it for a terminal: the
screen does not stretch, because the well's 1 : 2 must stay 1 : 2. A browser
window gets responsive sizing for free, which matters because it is whatever
size the visitor's happens to be.

Text is placed in cells too but is not held to the grid; `egui` rounds it to the
pixel. The one rect that is not a whole number of cells from the block's origin
is a piece in a hold or preview slot, which is centred by the cells it occupies
(§G3.2) — and that half cell is rounded down to a whole pixel, so it is crisp all
the same.

`gui/layout.rs` is the metric, and it is pure: a viewport and a scale factor go
in, rectangles come out. Its tests hold §G3's promises without a window.

### G3.2 The arrangement

**`LAYOUT_COLS` is 26 and `LAYOUT_ROWS` is 24.** In cells, from the block's
top-left:

```
col  0   1 ─────── 6   7   8 ─────────────── 17   18  19 ────── 24  25
row  0   ·                 ·   (the mouth: no lid)  ·                ·
     1   ┌ HOLD ───┐       ┃                    ┃       ┌ NEXT ───┐
     2   │  slot   │       ┃                    ┃       │  slot 0 │
     4   └─────────┘       ┃                    ┃       │         │
     6   ┌ SCORE ──┐       ┃        well        ┃       │  slot 1 │
         │ LEVEL   │       ┃     10 x 20        ┃       │   ...   │
         │ LINES   │       ┃                    ┃       │         │
         │ TIME    │       ┃                    ┃       └─────────┘
         │ COMBO   │       ┃                    ┃
    20   └ B2B ────┘       ┗━━━━━━━━━━━━━━━━━━━━┛
    21   ·                  status line                              ·
    23   ·                                                           ·
```

| Rect | Cells: column, row, width × height |
|---|---|
| Well | 8, 1, 10 × 20 — the visible field, one cell per matrix cell |
| Hold panel | 1, 1, 6 × 4 — **absent** when the running game has no hold |
| Stats panel | 1, 6, 6 × 15; or 1, 1 when there is no hold panel above it |
| Next panel | 19, 1, 6 × (3*n* + 1), for *n* = `preview_count` |
| Status line | 0, 21, 26 × 2 |

The panels are six cells wide: the widest piece, and one either side. The hold
panel and the next panel have the same insides — a label row, then two-row slots
a row apart, then a row of padding — so a next panel with one slot is exactly the
hold panel's shape, and one with six is 19 rows, which fits beside the 20-row
well. §6.3's `preview_count` range therefore never needs §12.4's `+N`, and the
layout leaves room for exactly six slots and no more. The stats panel's foot is
level with the well's floor when the hold panel is above it; without hold, it
moves to the top of the column, as §12.4's does.

A piece in a slot lies `North` and is **centred by the cells it occupies**, both
ways: a three-wide piece sits a cell and a half in from either side, and an `I`,
which occupies one row of its box, sits half a cell down rather than on the
slot's lower row. This is a deliberate difference from §12.4, where two
characters to the cell leave no half to centre by.

The well is the block's middle column, so the status line, centred in the whole
width, is centred under the well too.

### G3.3 The minimum, and below it

**The minimum cell is 14 points.** Points rather than pixels, because the
question is legibility and a pixel on a high-density display is half the size it
is on another: at 14 points a panel's label is about eight points high and a
figure twelve. That is a viewport of **364 × 336 points** at any whole density;
at a fractional one the cell must still be whole pixels, so a little more is
needed — 375 × 346 at 1.25 — and the viewport fits when its cell in pixels is at
least `ceil(14 × pixels_per_point)`.

Below it, every screen is replaced by a message, centred, after §12.1's:

```
Window too small
Need 364 x 336, have 300 x 200
Resize to continue
```

The sizes are in points — what a player dragging an edge can relate to — and
*need* is what fits at this display's density, not a round number at which it
still would not. The text is a fixed size rather than one scaled to the window,
since the window is by definition too small to scale anything to; in a viewport
smaller than the message, the message is clipped. **Rendering never panics at
any size** (`FRONTEND.md` F6), a zero-area viewport included — which is what a
minimised window reports, and what a canvas squeezed out of its page is.

**§8.4's rule is the shell's and is not restated here**: a game in progress is
forced into `Paused` *before* the screen goes, its held keys are released, and
the pause does not undo itself when there is room again. The window reports
whether it fits on every pass of `App::logic` — not `App::ui`, because a hidden
window has a size and no paint — through `Round::viewport` (F6). Only the
threshold is this front-end's.

The window itself has **no minimum size**. It opens at a cell of 28 points,
728 × 672, and can be dragged below 364 × 336 like any other. A tiling window
manager and a browser window would ignore a minimum anyway, and a too-small state
that only a tab could reach would be one that was only ever tested in a tab.

---

## G4. The playing screen

§12.4's information — the well, the hold slot, the next queue, the figures and
the status line — drawn as pixels over §G3's cell. Not a translation of §12.4's
44 × 23 block: pixels leave room the characters did not, and where that changes
where something goes, this section says so. Everything is drawn from the
`FrameState` the pump reported and never from `Game` (§12.7), and what the view
cannot answer arrives beside it, as it does in the terminal: whether the
*running game* has a hold slot, from `Round::hold_enabled` (§13.5), and whether
the grid and the debug read-out are on, from the config as it stands. There is no
generation counter to watch for a change to either (§G1.3): immediate mode reads
them every repaint.

Colours: the ground is `#121216` and the page's (§G8.1); the well and the panels
are a shade up, `#1E1E24`, so their edges read without a border. Every piece
colour is §12.3's **levelled** palette, and each of §12.3's brightness
percentages lands as a plain RGB scale of it — a piece, its ghost and its preview
are one hue.

### G4.1 The well

The locked cells of `GameView::rows`, the ghost when the view has one (§9.8,
at §12.3's 45 %), and the falling piece over it, each mino a tile: the cell less
a gutter of a sixteenth of it, never under a pixel, on whole pixels. The ghost
goes down before the piece, so where they overlap the piece is what is drawn.
Either may be absent — the ghost when `ghost_piece` is off, the piece during the
clear and entry delays — and that is the view's answer rather than a case here;
a mino above the field is `OFF_SCREEN` in the view, and is not drawn.

The well has **walls and a floor and no lid**, an eighth of a cell thick and
outside the ten-by-twenty interior, so the interior is exactly the field. The row
above it is the mouth a piece comes in through, as in §12.4.

With `show_grid` on, every cell of the well is drawn first as a faint tile, the
same shape as a mino: the pixel reading of §12.2's `··`. The well only — §6.3
puts the dots in the empty *playfield*, and a tiled ground behind a preview is
noise.

**While the game is paused, the well is drawn empty** (§9.17) — the pause menu,
and the Options and Controls boxes it opens, but not the countdown, which exists
so the board can be read again. That is `Overlay::blanks`, and it is the game's
rule rather than this screen's. The dimming under a box is §G5's, and is a
separate thing: it is what a game-over box sits over, where the stack is still
there to be read.

### G4.2 The hold panel

Labelled **HOLD**, with the held piece in its slot. While hold is locked out for
the current piece (§9.7) the label and the piece are both drawn dimmed, the piece
at 45 %. **When the running game has no hold, the panel is absent, not empty**,
and the stats panel takes its place at the top of the column — the
`Chrome::hold_enabled` rule, for a front-end that can no more ask the config
than the terminal can: an empty slot and an absent mechanic are both `hold:
None` in the view.

### G4.3 The next panel

Labelled **NEXT**, with one slot per previewed piece, slot 0 — the next to spawn
— at the top. Slot 0 is at full brightness, slot 1 at 75 % and the rest at 55 %:
§12.4's three steps, which are shared presentation (`shell::palette`). The
panel's height follows the preview count (§G3.2), 1 to 6.

### G4.4 The figures

Six, each a label over its figure, two and a half rows apart, the label on the
left and the figure right-aligned:

| Label | Figure |
|---|---|
| `SCORE` | the score, **grouped in threes** (`12,480`) |
| `LEVEL` | the level |
| `LINES` | lines cleared |
| `TIME` | `MM:SS` from the view's ticks, capped at `99:59` (§12.4, §11) |
| `COMBO` | `xN` while the combo counter is 1 or more, `-` otherwise |
| `B2B` | `ON` while the back-to-back chain is live, `-` otherwise |

Two differences from §12.4, and both are room. **The score is grouped live**, not
only at rest: §12.4's bare digits are a concession to eight characters of
interior, and a six-cell panel holds `9,999,999` with a cell to spare. And
**combo and back-to-back are figures here**, not words on the status line: they
are standing state, and the panel has the rows. Each is always listed and says
`-` when it has nothing to report, so the panel does not reflow as a chain
starts and ends. `B2B` is `GameView::back_to_back` — whether the chain is live —
and not whether the last clear was paid at the chained rate (§9.15, §12.8).

Figures are set in monospace, so a number that changes does not shuffle the
digits beside it. A figure too wide for its panel — a score of twenty digits —
is drawn smaller rather than cut off at the edge, because a truncated figure is a
wrong one. `shell::figures` writes the score and the time, for both front-ends.

### G4.5 The status line

Centred under the well: the most recent clear's name — `QUAD`, `T-SPIN DOUBLE`,
`PERFECT CLEAR` — for the 1.5 s `Cosmetics::clear_name` keeps it. While §10.1's
restart key is held it shows **RESTART** and a bar filling over the key's second
instead, as §12.4's does: a hold with no feedback is indistinguishable from a
key that did nothing, and a player about to throw the game away is not reading
the name of their last clear.

While the automated player holds the controls, **`PILOT` is drawn at the left
end of this row** for the whole game, in the `I`-piece cyan — `TUI.md` §12.4's
indicator in this front-end's units. The centred content is unchanged and does
not collide with it. It is not a transient, for the reason given there: a
screenshot of an automated game must never be mistakable for a player's, and
§G9.2's whole method here is screenshots (`PILOT.md` §P7.3).

### G4.6 The debug read-out

With `show_debug` on, a plain panel over the bottom-left corner of the window:
§12.4's nine figures — frame rate, ticks, dropped ticks, gravity in G, the lock
delay, `fall_period`, DAS charge, what is left of the bag, and the input mode —
in three rows of three, in 11-point monospace on a dark translucent ground.
**The figures and their words are `Debug::figures`'**, and the terminal's strip
prints the same nine; only the setting-out is this front-end's. The frame rate is
the frames this front-end drew — `App::ui` calls — in the last second.

It sits **outside §G3's metric**: a developer's read-out and not a supported
layout, so it moves no minimum, changes no cell, and is drawn over whatever is
under it. The bottom corner, because what is under it there is the status band
and the margin, which hold the least of the game — over the top one it hid the
hold panel. Beneath the block, where the terminal puts its strip, would have
taken rows from a viewport whose spare room is usually at its sides.

### G4.7 Losing the keyboard pauses the game

A window — or a tab — that does not have the keyboard **forces a game in
progress into `Paused`**, by §8.4's path: `Round::keyboard(false, now)`, which is the same
pause `Round::viewport` forces below the minimum, held keys released and all. A
game with lock delay that goes on without its player places pieces where the
player did not put them, and a window, unlike a terminal, is told when that has
happened.

- **Every pass without it, not only the first.** A countdown the player left
  running when they clicked away is not `Playing` and has nothing to pause, so
  the pass it runs out on is the one that pauses — before a tick is played that
  nobody could answer.
- **An automated game pauses too** (`PILOT.md` §P7.3). Nobody was going to
  answer those ticks either way, so the reason above does not apply and a
  narrower rule was available; it was not taken. A paused game blanks the stack
  under §9.17 whoever is playing, the spectator gets the same countdown back,
  and a window that has lost the keyboard has no business spending a search
  budget on every frame.
- **It does not undo itself.** Focus coming back shows the pause menu, and the
  player leaves it and gets §9.17's countdown, exactly as after a resize.
- **The releases come first.** The adapter's synthesised releases (§G2.3) reach
  the shell on the pass focus is lost, before the pause; the pause then lets go
  of whatever else was held.
- **Click to play** (§G8.2) is still drawn over the screen while the keyboard is
  away. It says what to do; the pause is what makes it safe to take a moment
  doing it.
- **A hidden tab has no keyboard**, so this is also what stops a backgrounded
  game from creeping (§G8.7) — but it is not `egui` that says so. "Have I the
  keyboard?" is two questions here: `egui`'s focus report, and `host::visible`,
  which is `!document.hidden` in a tab and a constant `true` natively. A hidden
  tab keeps the canvas's DOM focus and hears nothing, so the first alone answers
  `true` while the game goes on locking pieces nobody placed. That was measured
  in G13, after G7 had assumed otherwise; §G8.7 has the numbers.

This front-end has the rule and the terminal does not: `TUI.md` does not ask a
terminal for focus reports, and a terminal behind another window plays on.

---

## G5. Overlays

§12.6's three boxes, §9.17's countdown, and the two panels both screens open:
the §13.5 Options panel and the §10.1 controls table. They are drawn over a
complete playing screen (§G4), never instead of one.

**What each of them does is not this front-end's.** The menus are walked,
edited and closed by `shell::menus` through `Round::key`, exactly as the
terminal's are: the items, their order, their words, the twelve-character name
field, the setting each row steps through and the rule that `Esc` saves the
config on the way out of the panel. A front-end that invented any of that would
be a second §12.6. What is here is the boxes they are drawn in.

### G5.1 The box

A box is a rounded rect of whole cells, **centred over the block** as §12.6 has
it — not over the well, so that a box wider than ten cells is still centred on
the screen — on a ground a shade above the panels, with a thin border. Inside:
a title a cell down, a body of one-cell rows starting two cells down, and a
**hint** at the foot in the faintest grey, saying which keys the box answers to.
A box with *n* rows is *n* + 4 cells tall. `overlays::rect_of` is where the
sizes live, so "does it fit inside the block?" can be asked without drawing
anything, and it is a test.

The well **dims** under every box, so the box reads over it. That is not
§9.17's blanking, which has already emptied the well where it applies
(`Overlay::blanks`) and is the game's rule rather than this screen's: the
dimming is what a game-over box sits over, where the stack is still there to be
seen.

The cursor of a menu is **a drawn triangle**, not `▸`: a window has no
guarantee that its fonts carry that glyph, and a shape cannot go missing. For
the same reason every word in a box is ASCII — `Left / Right change, Esc saves`
rather than `←→ change`.

### G5.2 The pause menu, and the countdown

**PAUSED**, and §9.17's five items — Resume, Restart, Options, Controls, Quit
to menu — the selected one in a lit bar with the cursor beside it. Twelve cells
wide, so it covers the well and its walls and not the columns beside it, which
is what §12.6's box does in the terminal.

**The countdown is not a box.** §9.17 resumes through 3-2-1 over a playfield
that is visible again, and the point of it is that the player can read the
board: it is a single numeral drawn large and part-transparent over the well —
and it is the one overlay that does not dim what is behind it.

### G5.3 Game over, and name entry

**GAME OVER** lists §12.6's six figures — score grouped in threes, level,
lines, time, pieces, and pieces per second to one decimal — every one of them
the view's, so the box cannot disagree with the stats panel behind it (§11).
`Press any key`, after §9.16's second of lockout, which is the shell's.

**NEW HIGH SCORE** shows the rank the table has offered, the field, and
`Enter confirms, Esc discards`. The field is `shell::menus::NameEntry` drawn as
text with a caret after it — **not** an `egui::TextEdit`. §12.6's twelve
printable ASCII and its `ANON` default are rules about §14's table rather than
about a text field, and they belong where both front-ends reach them. It also
keeps the web build honest: a mobile browser's soft keyboard is a question of
its own, and it must not be answered by accident here (§1.2, §G8.8).

### G5.4 The Options panel

**OPTIONS**, one row per setting — label left, value right, the selected row
lit — and `Left / Right change, Esc saves`.

**The panel offers a different list in each of the three builds**, because a
panel must offer what it can actually apply. Seven rows are shared —
`Setting::SHARED`, and the list a fourth front-end starts from. §12.3's colour
depth is the terminal's alone: a window has no depths, no `mono` and no
`$NO_COLOR`. §G8.10's scale and full-screen switch are this front-end's, and a
browser tab takes only the first of the two, for the same reason it has no
**QUIT** (§G8.11).

| Build | List | Rows |
|---|---|---|
| `ftm` | `Setting::ALL` | the seven, then `Colour` |
| `ftm-gui` | `Setting::WINDOW` | the seven, then `Scale` and `Full screen` |
| Browser tab | `Setting::CANVAS` | the seven, then `Scale` |

Which one is carried on `Session::settings`, set once at start-up in
`gui::run` and `gui::start`. **Both the screen and the shell navigate that same
list**, so the cursor can never land on a row the screen is not drawing — the
failure a panel that simply drew fewer rows than the menu walked would have.

What §13.5 says about the panel is unchanged: presentation applies at once,
rules never — a game keeps the rules it started under, and a `Hold` switched
off here leaves the running game's hold box exactly where it was. The two new
rows are presentation, and both apply the moment the panel is left; the rest of
§G8.10 is start-up's (§G8.10's table says which is which).

### G5.5 The controls table

**CONTROLS**, §10.1's actions and the keys bound to each. The words, their
order and §13.3's rule that a binding whose setting is off is not listed at all
(§17.3 A9) are `shell::menus::controls`', and both front-ends print the same
list; the two columns are this front-end's. §8.2's input mode, which the
terminal's box names, is **not** shown: there is only the one path here (§G2.2).

---

## G6. Animations

§12.5's six animations and §12.4's status line, drawn as pixels. Each is
started by a `GameEvent` and timed by `shell::cosmetics::Cosmetics`, which sees
the event stream and a clock and has no path to the core (§12.8) — so the
durations, the triggers, and the fact that dropping every event changes nothing
about the game are shared with the terminal and are not restated here.

**`Cosmetics` is not changed by this section.** A window that needed a number
the terminal's screen does not have would be amending §12.5 for both
front-ends, and the first thing that would cost is the guarantee that the two
agree about when an animation is over. Everything below is derived from what
`Cosmetics` already reports.

### G6.1 What pixels change

A character cell has a foreground colour and little else, so §12.5's animations
reach a terminal as a **second colour**. A window has alpha, and the same
timings land softer for it:

| | The terminal | The window |
|---|---|---|
| Line clear | white, then the piece's own colour, at 12 Hz | one wash toward white at two strengths, so the row pulses rather than blinks |
| Hard drop | every trail cell at the ghost's brightness | a fade along the drop: brightest just above where the piece landed, almost gone at the top of it |
| Lock | the cells white | the cells washed toward white, keeping their hue |
| Level up | the banner in a faded style | the banner's alpha, and the band under it |
| Game over | grey from the front row up | the same, with the rows just above the front part-greyed, so the front is a gradient and not a line |

Where `Cosmetics` answers in two states — `flashing` is a boolean — the window
draws the two states as two strengths of one wash rather than as two colours.
Where its answer is spatial — the trail's cells, the wipe's row count — the
gradient is computed from that geometry. Neither needs a timer the terminal
does not have.

### G6.2 The order the well is composed in

Cell by cell, in the terminal's order and for the terminal's reasons (§12.5,
§9.8):

1. the well's ground, and `show_grid`'s tiles (§G4.1);
2. the locked cells of `GameView::rows`;
3. the hard-drop trail, **only where the well is empty** — a trail is behind
   the stack, never over a mino that is really there;
4. the lock flash, over the cells that have just locked;
5. the ghost;
6. the line-clear flash, and then the game-over wipe;
7. the falling piece over all of it, a fraction of a row low (§G6.5).

Steps 6 are **transformations of what is already in a cell** rather than things
drawn over it, so neither can hide a mino: a flashed cell is that cell's own
colour washed toward white, and a greyed one is that cell's own colour on its
way out.

The falling piece is last rather than between the ghost and the flash, and it
is the one thing not in the grid at all, because §G6.5 may put it between two
rows. It is given the same colour and the same two transformations the cell it
occupies would have given it, so at rest the composition is unchanged; drawing
it last is what keeps it over the ghost where the two overlap (§9.8).

§9.17's blanking comes before all of it. A well the pause has emptied
(`Overlay::blanks`) has nothing in it to animate, and steps 2 to 6 are skipped
whole — an animation that outlived the pause would be exactly the free look at
the stack §9.17 forbids. The banners are not in the well and are drawn either
way: a level that arrived as the player hit pause is still worth telling them
about.

### G6.3 The six

| Animation | §12.5 | The window |
|---|---|---|
| Line clear flash | `line_clear_delay_ms`, 12 Hz | every mino of a cleared row washed toward white — fully on one half of the alternation, two fifths of the way on the other. The rows are still there until the core collapses them, and the flash covers that pause exactly. |
| Hard-drop trail | 120 ms | the cells the piece passed through, in the piece's own colour, fading along the drop from just under the ghost's brightness to almost nothing. Empty cells of the well only. |
| Lock flash | 80 ms | the cells that locked, three quarters of the way to white, so the piece is still the colour it was. |
| Level-up banner | 1.2 s | `LEVEL n` across the well, its alpha the fraction `Cosmetics` reports is left of it, on a band that fades with it. |
| Perfect-clear banner | 1.5 s | `PERFECT CLEAR` in the piece colour `Cosmetics` cycles, on the same band. A perfect clear outranks the level-up it so often arrives with, which is `Cosmetics`' rule and not this screen's. |
| Game-over wipe | 500 ms | the stack greys from the top row down, the rows just above the front part-greyed so the front reads as a gradient. It settles on a grey stack and stays, under §12.6's box. |

§12.4's status line is the seventh thing `Cosmetics` drives and was drawn at
G7: the most recent clear's name for its second and a half, and §10.1's restart
bar in its place (§G4.5).

**The banners are centred over the well**, drawn with the playing screen and
therefore **under** anything §G5 puts over it — a box that opened while a
banner was up dims the banner along with the rest of the well, because the box
is what the player is answering. The band behind a banner is this front-end's
own: a word in one of §9.2's seven colours over a stack in the other six is not
reliably legible, and a window, unlike a terminal, cannot clear the row.

### G6.4 There is no `animating()` here

The terminal asks `Cosmetics::animating` because §15.2 step 5 draws only when
the frame has changed, and an animation changes the frame without changing the
view. This front-end makes no such comparison (§G1.3), and `Round::deadline` is
inside a tick whether or not the game's clock is running — a paused game, a
game-over box — so the repaint an animation needs is one that was going to
happen anyway.

That is also §12.5's "skipped rather than slowed", satisfied by construction:
every animation here is a pure function of the moment `Cosmetics` was last
given, so a frame that does not happen is a step that is not drawn, and nothing
is left owing.

### G6.5 Sub-cell gravity

At level 1 a piece falls one row a second. In a terminal that is the blocky
aesthetic; on a forty-pixel tile it is a stutter, once a second, for the first
minute of every game. The window draws the falling piece a fraction of a row
low, and the fraction is `GameView::fall_progress` (§12.7) — §9.9's gravity
accumulator over the fall period in force, which is the piece's exact sub-row
position and not a tween. Nothing is estimated and nothing can drift out of
step with the rules, because it *is* the rules' own number.

It scales itself, which is the part a hand-tuned animation would have got
wrong. At level 1 the period is sixty ticks, so the fraction takes sixty
distinct values on the way down; by level 10 a handful; above 1 G, and under
any soft drop at speed, there is nothing left to interpolate — which is exactly
where nobody could have seen it. Soft drop divides the *period*, so the
denominator goes down with it and the fall stays smooth all the way.

Three requirements, and the first two are §12.7's:

- **The piece is the only thing that moves.** The ghost marks a landing row,
  which is a discrete fact; leaving it snapped while the piece slides is what
  closes the gap smoothly. The stack, the trail and the wipe are all on the
  grid.
- **A landed piece does not hover.** `fall_progress` is 0 whenever the piece
  cannot move down, so it sits still through §9.11's lock delay.
- **The offset is whole physical pixels.** `Layout::falling_cell` floors it the
  way every other rect in §G3 is floored, so a sliding piece's edges are as
  crisp as a settled one's. At the 14-point minimum that is fourteen positions
  within a cell, and forty or more at a comfortable size.

This is why the falling piece is not in §G6.2's composed grid: a grid of cells
has no way to hold "half a row down". It is drawn after the grid, in the colour
and with the washes the cell it occupies would have given it, so a piece at
rest is drawn exactly where and as it was before it became a separate step. It
is washed by the row it **occupies**, not the one it is sliding toward.

A front-end on a display faster than 60 Hz may extrapolate within the tick from
its own elapsed time, clamped so the drawn position can never pass the ghost's
row. Purely cosmetic, never fed back, and worth doing only if the quantisation
is visible. This one does not.

Horizontal movement and rotation are **not** interpolated, here or anywhere: a
piece is where the rules say it is, and §19's second player has to see the same
thing. There is no fractional horizontal state in the core to reveal, a tween
would add lag to the signal the player is most sensitive to, §10.3's `arr = 0`
means "to the wall this tick" and contradicts any duration chosen to animate
it, and an SRS kick has no meaningful intermediate pose (§9.5). What to reach
for instead, if the movement wants softening, is a brief trailing smear over
the vacated cells — the shape §12.5's hard-drop trail already has, and
`GameEvent::PieceMoved` already fires for. `EGUI-PLAN.md` G10 has the long form.

### G6.6 A piece enters the well

§9.4 spawns every piece but `I` with minos in matrix row 19 and then drops it
one row, so a freshly spawned `T`, `S`, `Z`, `J` or `L` has a mino above the
visible field and an `O` has two. For a whole row of its fall the piece
straddles the top of the field.

`GameView` used to clip those minos away, so a `J` was drawn as three minos and
then, a row later, as four. That was barely visible while the piece stepped a
row at a time; against §G6.5's slide it is a mino appearing out of nothing while
its neighbours move smoothly. §12.7 no longer clips the falling piece — a mino
above the field carries a negative row — and this front-end draws the piece
**clipped to the well**:

- A mino wholly above the field draws nothing.
- One straddling the top edge draws the part of it inside the well, so it is a
  partial square that grows as the piece slides.
- At the row change it is exactly the whole square `field_cell` would have put
  there, so nothing jumps.

The mouth is where a piece comes *from*, not somewhere this screen draws: the
well has no lid (§G4.1), and a mino hovering in the margin above it, beside no
wall, would read as part of the layout rather than as a piece arriving. Clipping
is also what keeps the well's top edge a line — the one the walls stop at.

A front-end with no room above its well skips negative rows entirely, which is
what §12.4 does. Neither has to know a buffer zone exists; a negative row is
simply a row it has no room for.

---

## G7. The attract screen

§13's screen, drawn for a window. What is on it is §13's and is not restated
here: the wordmark, the five-item menu, the panel that cycles every six
seconds, the drifting background, the sixty-second idle colour cycle and the
three sub-screens. **The state is shared** — `shell::attract::Attract` is the
same state machine the terminal walks, and it is what decides what is
selected, which sub-screen is open, how far the cycle has come and how long the
keyboard has been quiet.

So are the *words*, which is the part that would have been easiest to get
wrong. §13.2's letterforms, §13.2's three colours and the order §13.6's cycle
walks them in, §13.3's reminders and §13.4's numbers all live beside that state
machine rather than in either front-end. A second front-end that drew a
different wordmark would be a second piece of branding, and §1.3 is precisely
about that; one that invented its own reminders would be a second §13.3.

§1.3 reaches this screen more than any other. The wordmark is original block
letters, the official logo must not be used, reproduced or approximated, and no
official colours-as-branding, styling or artwork may be copied — and the same
applies to the window title, the page title and any favicon or icon this
front-end ever acquires (§G1.2, §G8.1).

### G7.1 It is laid out in §G3's grid

The attract screen takes **the same block, the same cell and the same
minimum** as the playing screen: 26 × 24 cells, `Layout::cells`, 14 points.
Three things follow, and each is why.

- **A window that is too small says one thing, not two.** §G3.3's message
  replaces this screen exactly as it replaces the playing one, at the same
  size, and a player dragging an edge sees one threshold rather than a screen
  that vanishes at some other width.
- **Leaving a game does not resize anything.** The wordmark's blocks are the
  size the well's minos were a moment ago, because they are the same cell.
- **There is one metric to test.** `gui::layout` is pure and already tested
  without a window; this screen adds rows to it and no arithmetic of its own.

In cells, from the block's top-left: two rows of margin, the wordmark's five,
a blank, the name, a blank, a five-row band the menu is centred in, a blank,
the six-row panel, the footer, a row of margin.

| Rect | Cells: column, row, width × height |
|---|---|
| Wordmark | centred, 2, 15 × 5 — in blocks, not cells of text |
| Name | 0, 8, 26 × 1 |
| Menu band | 7, 10, 12 × 5 — the menu is centred *in* it |
| Panel | 1, 16, 24 × 6 — four rows inside |
| Footer | 0, 22, 26 × 1 |

The menu is centred in a band of a fixed five rows rather than filling however
many rows it has, so that a menu one item shorter than §13.3's — a tab's, which
cannot quit (§G7.5) — leaves the panel under it exactly where it was.

### G7.2 The wordmark is made of minos

§13.2's letterforms are a bitmap, and what a front-end adds is what one block
of it is drawn as. A terminal gives it two characters, so that three letters
still carry the screen. **A window gives it one mino** — the same tile the well
is drawn with, in the same levelled palette (§12.3), with the same gutter — so
the wordmark is visibly built out of the game rather than set in a font. It is
fifteen blocks across: four, a blank column, four, a blank column, five.

The three letters take the `I`, `S` and `T` colours, and §13.6's idle cycle
advances all three one step along §9.2's seven once a second after a minute of
quiet. That is `shell::attract::wordmark_colour`, and both front-ends ask it.

### G7.3 The drift

§13.4's ambient animation, in this front-end's units. How often a piece
arrives, how many exist at once and how fast they fall are §13.4's numbers and
are shared; where a piece *is* is measured in cells of the viewport, so the
drift itself is `gui::attract::Drift` and the terminal keeps its own
(`EGUI-PLAN.md` G2). Three differences from a character grid, and each is the same
animation rather than another one:

- **Outlines are strokes.** §13.4 draws `░░`; a window strokes the cell, a
  twelfth of it thick and never under a physical pixel, at the same 28 % of the
  piece's levelled colour.
- **§13.4's two exclusions become one.** `show_debug` still turns the drift
  off; `mono` has no meaning here, because a window has no colour depths
  (§G8, "What does not carry over").
- **"Painted after it, opaquely" is met by being behind, not by covering
  up.** That sentence is a character grid's: a drifting cell there would
  *replace* a glyph, so the regions that matter have to be painted over it. A
  window draws the drift first and everything else on top of it, and what is on
  top is opaque text, opaque minos and the panel's own ground. The result is
  legible, and nothing has to be blanked to make it so.

The drift's entropy is F3's — `host::seed` — and not the operating system's,
because a browser tab has no OS entropy source to reach for (`EGUI-PLAN.md` G3).
Nothing here is ever replayed, so the generator only has to look random; it is
`Xoshiro256PlusPlus` because that is the generator this project names (§9.6),
and reaching for `SmallRng` here would be reaching for the one that is a
different generator on wasm32 (§G8.9).

### G7.4 The menu, the panel and the sub-screens

The **menu** is one item to a row, the selected one in a lit bar with §G5.1's
drawn triangle beside it — the same bar and the same triangle the pause menu
uses, because they are the same kind of thing. Twelve cells wide, which is the
well and its walls.

The band it sits in is **five rows, reserved whatever the menu's length**, and
the items are centred in it. That is why `PILOT.md` §P7.1's sixth item cost this
screen nothing: six rows of items centre into the blank rows either side of the
band, the panel and the footer do not move, and §G3's 26 × 24 grid and §G3.3's
minimum are untouched. It was checked by looking at it, not by arithmetic
(`PILOT.md` §P7.2) — the terminal's block, laid out the other way round, did
need a row and lost its footer without one.

The **panel** is a rounded rect on the panels' own ground, four rows inside it,
and it holds §13.3's three faces: the quick control summary, the top three
under a heading, and one of §13.3's reminders. The cycle, and its pause while a
menu item other than **PLAY** is selected, are the state machine's. A face
shorter than four rows is **centred in the panel**, as the terminal's is: every
face is short of four at some setting — the summary is three rows with 180°
rotation off, the high-score face two with an empty table — and one that filled
from the top would read as a panel that had lost a row.

The control summary's *words* are this front-end's, and it is the one place
§13.3's panel differs between the two. The terminal names the movement keys
with arrows; a window has no guarantee that its fonts carry them (§G5.1), so it
spells them — `Left / Right`, `Up`, `Down`, `Space`. §13.3's **rule** is
shared: an entry whose setting is off is not listed at all, so `C hold` goes
when hold does and `A rotate 180` with 180° rotation (§17.3 A9).

Of §13.5's three sub-screens, **two are not this screen's at all**. The Options
panel and the controls table are the same boxes the pause menu opens — §13.5
says so of the panel in as many words — and they are drawn by `gui::overlays`
over whichever screen asked for them, with the same seven rows §G5.4 gives the
panel. The **high-score table** is this screen's: a §G5.1 box of the top ten in
six columns — rank, name, score grouped in threes, level, lines, date — with
the entry the run that just finished added drawn in the `I`-piece cyan. The
columns are fractions of the box rather than character counts, and a figure too
wide for its column is drawn smaller rather than over its neighbour.

Under a box the whole screen dims, as the well does under §G5's. There is no
§9.17 blanking to do: there is no stack here to give a free look at.

### G7.5 Two screens, one pump

§7's state machine is a loop over `Next` in the terminal and a **field** here,
for the same reason §15.2's steps became method calls (`EGUI-PLAN.md` G4): the
compositor calls, and what it finds is whichever screen the run is on. §15's
*two* loops are two answers to the same question — a game asks to be woken
inside a tick (§15.2), the attract screen at a flat 10 fps with no accumulator
(§15.3) — rather than two `while`s.

- **`ftm-gui` opens on the attract screen**, because §13.1 is what the program
  shows whenever no game is in progress and that includes the moment it starts.
- **§G4.7's forced pause has nothing to say here.** There is no game to pause,
  so a window that loses the keyboard keeps drifting and cycling; "Click to
  play" is still drawn over it (§G8.2), because the keys still go to the page.
- **QUIT closes the window, natively.** In a tab it is **not offered**: a page
  the player opened may not close itself (§G8.1), and an item that did nothing
  would be worse than one that is not there. Which items the menu has is
  `Session::menu` — `MenuChoice::ALL` natively, `MenuChoice::CANVAS` in the web
  build — and **the screen draws that list and the shell walks it**, exactly as
  they share `Session::settings` (§G5.4), so the cursor can never land on an
  item nobody can see. §10.1's quit *key* is always live and simply comes back
  to this screen there.
- **PILOT is offered in both builds** (`PILOT.md` §P1). It was offered natively
  and not in a tab at first, on the grounds that a tab would run the search on
  its frame thread; the search was then measured in a tab and the grounds went
  away, so the two lists differ by QUIT alone again. `MenuChoice::NO_QUIT` keeps
  the name it was given when they differed by two — **`MenuChoice::CANVAS`**,
  the browser's list, exactly as `Setting::CANVAS` is the browser's panel,
  rather than "the same list without quit". Nothing is `cfg`-ed out: the variant
  exists in every build and one list omits it.

---

## G8. The web build

The same front-end, compiled to `wasm32-unknown-unknown` and served as a page.
It is **not a port**: `gui/app.rs`, `keys.rs`, `paint.rs` and the whole of
`shell/` and `core/` are the native build's code, unchanged. What differs is what
an entry point is made of — where the flags come from, where `FRONTEND.md` F1–F4
come from, and whether a run has an end — and all of it is in
`gui/host_web.rs`, `gui/query.rs` and the wasm `main` of `src/bin/ftm-gui.rs`.

`EGUI-PLAN.md` G6 wrote §G8.1–§G8.9; G12 added §G8.10 and §G8.11 and finished
§G8.4. The last two are not the web build's alone — the `[gui]` table is this
front-end's in both of its builds, and §G8.11's table has a column for each —
but they are here because a browser tab is where the questions they answer bite
first: a canvas has no window to size and no argv to read.

### G8.1 The page, and a run with no end

`index.html` is a canvas (`id="ftm"`), a status line (`id="status"`) and a
footer, and nothing else. Its `<title>` is the window's — **Falling Tetromino
Manager** — and its icon is empty, because §1.3's trademark rules reach both.
`Trunk.toml` builds it: `trunk serve` to develop (`make run-web`), `trunk build
--release` for the artefact in `dist/` (`make web`). Asset URLs are relative, so
`dist/` works wherever it is put.

**Where it is put is GitHub Pages**, at `https://clstrfsck.github.io/ftm/`, and
CI's `deploy` job publishes what CI's `web` job built on every push to `main`.
That is the whole of the hosting: §1.2 puts network play out of scope and §19 is
a list of constraints rather than a component, so there is nothing behind the
page. A directory of static files is not a reduced deployment here; it is the
only kind this game has. The `/ftm/`
prefix a project page adds is why `public_url` is relative rather than absolute
— the artefact is not told where it is served from, and the same `dist/` runs
under `trunk serve` at the root and under Pages in a subdirectory. A future move
to a domain of its own is a DNS change and nothing else.

The entry point, in order: make the `eframe::WebRunner` (which installs the panic
hook, §G8.5); read the query string (§G8.4); open `localStorage` (§G8.3);
resolve §6.1's three sources; report what that had to say (§G8.6); start. On
success the status line is removed and the canvas is given the keyboard; on
failure the status line says why.

**A tab's run has no end.** The page is closed, not quit, and a reload discards
the wasm instance whole. So there is no §6.2 first-clean-exit write, no `finish`,
and nothing to hand back: the store and the session live exactly as long as the
page, and are leaked once, at start-up, to say so. And a tab cannot close
itself — `window.close()` is refused to a page the player opened — so where the
native build closes its window on `Next::Quit`, the web build goes to the
attract screen, and **QUIT** is not offered at all: the menu a tab draws is
`MenuChoice::CANVAS`, five items rather than §13.3's six, and §10.1's quit key
simply comes back to that screen (§G7.5). **QUIT is the one item it is short.**
It was short of PILOT too for a while, for an unrelated and — unlike this one —
unmeasured reason, and `PILOT.md` §P1 has what measuring it said. The list was
called `NO_QUIT` before that, and keeps the name it was given in between.

### G8.2 The keyboard, and the canvas's focus

A canvas hears keys only while it has focus, and `eframe` keeps `Space`, `Tab`,
`Backspace` and the four arrows from the page (`preventDefault`) only while it
does. A canvas without focus is therefore a page that scrolls when the player
hard-drops. Three rules follow:

1. **The canvas takes focus on load.** `eframe` gives it a `tabindex` so that it
   can be focused; the entry point focuses it as soon as `eframe` has started.
2. **While it has focus, `Space`, `Tab`, `Backspace` and the arrows reach the
   game and nothing else** — no scroll, no focus moved, no history navigation.
3. **Without focus it says so**: **Click to play**, in a band across the middle
   of the screen, over whatever is there. The click is the *browser's* — it
   focuses the canvas, and the game sees no pointer event (§1.2). `Tab` from the
   page reaches the canvas too. The same notice is drawn natively, for a window
   without the keyboard, because the fact it states is the same.

`F1` is one of §6.3's default pause keys and is also the browser's help key;
`eframe` does not keep it from the page, so in some browsers it opens a help tab
as well as pausing. `Esc` is the other default, and does not.

### G8.3 Storage

§6.2's document and §14's table are two `localStorage` keys, **`ftm/config.toml`**
and **`ftm/highscores.json`** — the native file names, so that a player looking
in the browser's storage inspector knows what each is.

- **Per origin, and not the native files.** A player's scores in a tab and on
  their desktop are separate tables by construction, as are their settings
  (§6.2, §14).
- **No temp file and no rename.** §14 asks that a crash leave the old table or
  the new one; a single `setItem` already does.
- **A full store is a warning** (§16): `setItem` throws `QuotaExceededError`,
  which becomes `StorageError::Failed` naming the key.
- **A page with storage switched off plays anyway.** Where `localStorage` is
  blocked — cookies disabled for the site, a sandboxed frame — asking for it
  throws or answers `null`, both are `StorageError::Unavailable`, and the shell
  says so once, on the read. Its wording is the shell's and says *nowhere to
  keep the settings on this platform* rather than naming a directory: the shell
  does not know where the bytes were going (§3.1), and a tab has no directory
  to be told about. (It named one until `EGUI-PLAN.md` G12.)
- **The default document is never written.** §6.2 writes it on the first clean
  exit, and a tab has none. The config key is written when the §13.5 Options
  panel saves, and not before.

### G8.4 Query parameters for §6.4's flags

A tab has no argv; it has a URL. §6.4's flags are query parameters with the same
names, parsed in `gui/query.rs` into the same `Overrides` that `clap` produces
natively, and §6.1's precedence is unchanged: the query over the stored config
over the defaults. G6 had one — **`?seed=N`** — and G12 has the rest: §G8.11
is the list. Four rules differ from a command line, because a URL is not only
the game's:

- **A parameter the game does not know is ignored, silently.** Links pick up
  tracking tags on their travels, and a warning about each is noise.
- **A value that does not parse is a warning (§G8.6), and the run goes ahead
  without it** — `clap` refuses a bad flag and exits, which a tab cannot do. A
  bad value does not retract an earlier good one: `?seed=1&seed=x` is seeded 1.
- **A parameter given twice takes its last value.**
- **A flag's parameter may be written five ways and switched off in four.**
  `?no-hold`, `?no-hold=`, `?no-hold=1`, `?no-hold=true`, `?no-hold=on` all ask
  for it; `=0`, `=false`, `=off`, `=no` leave the file's own answer alone;
  anything else is a warning. A command line has only "written or not written",
  but a URL is edited by hand and pasted between people, and someone turning a
  shared link's setting off will reach for `=0` before they will delete the
  parameter.

The paired halves keep their own names — `?hold` and `?no-hold` are two
parameters, not one with a value — because §6.4's names are the point: a player
who knows the flag knows the parameter. They override each other in the order
written, exactly as the flags do.

### G8.5 Panics, and a canvas that will not start

This is §8.1's panic hook with nothing to restore: a legibility measure, not a
repair. `eframe::WebRunner::new` installs a hook that logs a panic's message and
stack with `console.error`, and the entry point makes the runner **before**
anything that could panic — the web's reading of "installed before raw mode".
`console_error_panic_hook` is not used, because that hook is this one.

A failure `eframe` reports rather than panics on — no WebGL2, most often — is
§16's "error that reaches the entry point": it goes to the console with
`console.error`, and replaces the status line on the page, so that a player
without a console open is not left looking at "Loading…".

### G8.6 Warnings

§16's warnings go to the console with `console.warn`, prefixed `ftm-gui:`, **as
they arise, each once**: those from the query string and the stored config at
start-up, and those the session raises later — a table `localStorage` would not
take — on the frame that raised them. Natively they wait for the window to close
and go to stderr (§G1.2); a tab has no such moment to wait for.

### G8.7 A hidden tab

A backgrounded or minimised tab gets no animation frames. `eframe` notices, and
keeps calling `App::logic` on a timer instead — which the browser throttles, and
throttles harder the longer the tab stays hidden — and `App::ui` not at all.
Each of those calls plays at most `MAX_CATCH_UP_TICKS` (§15.1), a tenth of a
second, and discards the rest, so **a hidden game creeps rather than stopping**,
and returning to it costs nothing: there is no burst of arrears, and the player
is not killed by coming back. Measured in Chrome at G6: 36 seconds minimised
advanced a level-1 game by about nine seconds of play.

**Since `EGUI-PLAN.md` G7 a game in progress does not creep: it pauses**, and
since G13 it actually does. The first `App::logic` pass after hiding forces the
pause of §G4.7, and the player comes back to the pause menu rather than to a
game that went on without them. The measurement above is G6's, from before that
rule, and is kept because it is what settled it: at a high level, nine seconds
of play in thirty-six locks pieces nobody placed. What still runs while hidden
is the pump itself, and it is correct at that cadence, so anything that is not a
game in progress — the attract screen — creeps as described, which is what it is
for: a tab left open on §13 goes on drifting and cycling, at whatever cadence
the browser is willing to give it.

**`document.hidden` is what says so, and `egui`'s focus report is not.** G7
assumed a hidden tab reports no focus, and G13 measured that it does not:
switching to another tab in the same window fires no `blur` at the canvas, which
keeps the document's focus — `document.hasFocus()` goes on answering `true` —
so a game went on playing behind a tab that could not hear a key. `host::visible`
is the question this front-end asks instead, and §G4.7's rule is the conjunction
of the two: the keyboard is ours when `egui` says the application has focus
**and** the host says the page is on screen. A tab whose *window* is behind
another one is not hidden by this measure and does report a lost focus, which is
the other half of the same rule; natively `visible` is a constant `true`,
because a window that is hidden, minimised or behind another does not have the
focus either and `egui` already says so.

A hidden tab can hear no keys. `eframe` hands the same unconsumed input to every
hidden pass until one paints, so the adapter sees each focus change more than
once — harmless, because its answer to a focus change is idempotent (§G2.3).

### G8.8 Touch

There are no touch controls, and the page says so in its footer: *Falling
Tetromino Manager is played with a keyboard. There are no touch controls.* A web
build is a link someone will open on a phone, and a game that silently ignores a
finger is worse than one that explains itself; an on-screen control layer would
be a §1.2 amendment and a piece of design in its own right, and `EGUI-PLAN.md` G6
declined it.

### G8.9 The same seed, the same game

§15.4 holds between a tab and a desktop, and G6 found that it did not until then.
`wasm32-unknown-unknown` is a **32-bit** target, and `rand`'s `SmallRng` is a
different generator on one, so seed 42 dealt J L S O Z I T in a tab against
J T S I L Z O natively. §9.6 now names the generator, and the two agree. It is
recorded here because it is the defect this build is best placed to find: every
test in the tree runs on a 64-bit host, so a difference that only a 32-bit target
has is invisible to all of them, and `make portable` — which does build the core
for wasm32 — is where the guard lives.

### G8.10 The `[gui]` table

§6.3's document has a table for this front-end, as it has `TUI.md` §6.3's four
glyph and colour keys for the terminal's. It is **presentation** by §6.5: none
of it changes what happens, only how large it is and how often it is drawn.

```toml
[gui]
# The window's inner size in points when it opens.
# Ranges: 320..=7680 and 240..=4320.
window_width    = 728
window_height   = 672
# Where it opens, in points from the top-left of the primary display. With
# these commented out the platform decides.
# window_x = 0
# window_y = 0
# Write the size and position above back here when the window closes.
remember_window = true
# Interface scale, per cent. Range: 50..=300. Honoured in a browser tab too,
# where nothing else in this table is.
scale_percent   = 100
# Open full screen.
fullscreen      = false
# Wait for the display's refresh before presenting a frame.
vsync           = true
# The most repaints a second the game asks for; 0 is "no cap". Range: 0..=1000.
frame_cap       = 0
```

The defaults are §G3's: `window_width` and `window_height` are the §G3.2 block
at §G3.1's initial cell, which is where `INITIAL_SIZE` came from before this
table existed. `shell/config.rs` holds the two numbers, because a fresh file has
to name a size and the shell may not name a front-end's module (§3.1); a test in
`gui/layout.rs` is the join.

**Every binary parses, validates and writes back every table** (§6.2). A
terminal run clamps `scale_percent = 1000` and says so, because §6.2's warning
is about the *file* and the player who typed it edits one file. Only the window
acts on any of it.

Four of the rows are answered once, when the window is made, and four are not:

| Row | When it takes effect | In a tab |
|---|---|---|
| `window_width` / `window_height` | Start-up, and written back if the window moved | Ignored: a canvas is the size the page gives it |
| `window_x` / `window_y` | Start-up, and written back if the window moved | Ignored |
| `remember_window` | Whenever the window closes | Ignored: there is nothing to remember |
| `scale_percent` | At once — an Options row (§G5.4) | **Honoured**: it is `egui`'s zoom either way |
| `fullscreen` | At once — an Options row, natively only | Not offered (§G8.11) |
| `vsync` | Start-up only: a swap interval is chosen when the surface is made | Meaningless |
| `frame_cap` | At once, on the next deadline | **Honoured** |

Three things about that table are load-bearing.

**`frame_cap` caps drawing and never the game.** It is a floor under §15.2 step
6's deadline — it lengthens the wait, never shortens it — so a capped window
plays several ticks per repaint and plays *the same game*: `Round::advance`'s
accumulator is over elapsed time and never over frames (§15.2 step 4), which is
the cadence invariance `tests/pump.rs` pins. It is advice for the same reason
`deadline` is: a key, a resize or a compositor may wake the window sooner, and
nothing refuses to draw when one does.

**`remember_window` remembers size and position, and not `fullscreen`.** A
window's geometry is a thing the player *did*; full screen is a thing they
*chose*, in the file or in the §13.5 panel — and `--fullscreen` is a flag, which
§6.1 never writes back. Observing it would turn one run's flag into a permanent
setting. For the same reason in reverse, a window that is full screen or
maximised when it closes is not remembered at all: the size it reports is the
display's, and restoring that later would hand the player a window they never
chose.

**The write-back happens only when something moved.** Otherwise every run would
rewrite the player's config for nothing, and every run over a read-only one
would add §16's warning to the pile. What is written is the document *without*
the command line applied (§6.1), with the four numbers copied across —
`gui::host_native::keep_window` is the decision, and it is the native host's
because a tab has no window and no exit to make it at.

### G8.11 Which flag exists in which build

§6.4 splits the way §6.2 does: the grammar belongs to a front-end and the
meaning to the shell, so a flag exists where the setting it names does.
`--color` is §12.3's and there is no colour depth in a window; `--scale` and
`--fullscreen` are §G8.10's and there is no window in a terminal.

| §6.4 | `ftm` | `ftm-gui` | Browser tab | Why not |
|---|---|---|---|---|
| `--preview <N>` | ✅ | ✅ | `?preview=N` | |
| `--level <N>` | ✅ | ✅ | `?level=N` | |
| `--no-ghost` | ✅ | ✅ | `?no-ghost` | |
| `--hold` / `--no-hold` | ✅ | ✅ | `?hold` / `?no-hold` | |
| `--rot180` / `--no-rot180` | ✅ | ✅ | `?rot180` / `?no-rot180` | |
| `--lock-down <RULE>` | ✅ | ✅ | `?lock-down=RULE` | |
| `--seed <N>` | ✅ | ✅ | `?seed=N` | |
| `--color <MODE>` | ✅ | — | — | §12.3 is a character grid's (`TUI.md` §6.3) |
| `--scale <PERCENT>` | — | ✅ | `?scale=N` | §G8.10's, and there is no scale in a terminal |
| `--fullscreen` / `--no-fullscreen` | — | ✅ | `?fullscreen` | §G8.10's. A tab accepts it and can do nothing with it |
| `--config <PATH>` | ✅ | ✅ | — | A tab has one storage key, not a path (§G8.3) |
| `--print-config` | ✅ | ✅ | — | A thing to do instead of playing; a page cannot be asked to do it |

Two of those rows deserve a word.

**`--print-config` is shared.** What it writes is the whole document, `[gui]`
and `[display]` alike, and the player comparing what the two binaries resolved
from one file is exactly who asks for it. `ftm-gui` prints it before the window
opens, as `ftm` prints it before raw mode — with one line fewer, because §8.2's
input mode has only one value here (§G2.2).

**A tab accepts `?fullscreen` and does nothing.** The alternative is a warning
on a link shared from a desktop, which scolds whoever opens it for something
they did not write. The same reasoning is why an unknown parameter is silent
(§G8.4); the difference is that this one is not unknown, so §G8.11 says out loud
that it is inert rather than leaving it to be discovered.

**The list is a test**, in both directions: `gui::cli::FLAGS` against `clap`'s
own grammar, `gui::query::PARAMETERS` against `FLAGS`, and every shared flag
parsed both ways and compared as `Overrides`. A flag added to one build and not
the other fails a test rather than becoming a link that quietly does nothing.

---

## G9. Testing and acceptance

This front-end has no `tools/drive.py`. It has two tests that between them reach
what that script reaches for the terminal, and one acceptance list.

### G9.1 The render test

`tests/gui_render.rs` is §17.2's last requirement for this front-end — *rendering
does not panic at any viewport size* — and it is `tests/render_sizes.rs`'s
counterpart, asking the same question in pixels. Like that one it drives the
drawing entry points rather than the loop above them, at every size, with every
screen the program can show: §G4's playing screen under each of §12.6's and
§13.5's overlays, §G7's attract screen and its three sub-screens, §G4.6's debug
read-out with the widest figures a `Debug` can hold, and §G3.3's replacement
message.

The sizes, in **physical pixels**, are 0 × 0, 1 × 1, 320 × 240, 364 × 336,
728 × 672, 1920 × 1080 and 3840 × 2160; each is drawn at densities 1, 1.25, 2
and 3. Two things differ from the terminal's four:

- **The density is an axis a character grid has not got.** §G3.1 floors the cell
  in physical pixels, so 1.25 is a different arithmetic from 2 rather than a
  larger one, and a size that fits at one density does not at another.
- **The minimum is therefore a boundary in pixels, not in points.** Fourteen
  points is 17.5 pixels at 1.25, and a cell must be eighteen, so the least
  window there is 468 × 432 pixels — 375 × 346 points against 364 × 336 at 1×.
  The test asserts both halves of that: the least window fits and one pixel less
  does not, and the figure §G3.3's message asks for is never smaller than the
  one it takes, so a player who resizes to what they are told gets the screen.

Nothing is asserted about how any of it looks. What it looks like is `src/gui/`'s
own tests, which measure where the text landed (§G7.4 is why), and B3–B11 below,
which were checked by running the program.

**Image snapshots are deliberately not taken.** `egui_kittest` can render and
compare them, but it needs a GPU or a software rasteriser, and a CI job that
flakes on driver differences is a CI job that gets marked ignored. The headless
render test is the one that runs everywhere. The crate is not a dependency:
`egui::Context::run_ui` with a `RawInput` is the whole of the harness these tests
need, and every test here and in `src/gui/` uses it directly — one `Context` per
test, as a window has, because building one lays out the fonts and that is most
of the cost.

### G9.2 What stands in for `tools/drive.py`

`tests/pump.rs` (`EGUI-PLAN.md` G4) is §15.2's steps driven with no screen and no
clock, and it is where this front-end's *behaviour* above the pixels is pinned:
cadence invariance, the catch-up cap, `deadline`'s bounds, §8.4's two forced
pauses — the viewport's and §G4.7's — §10.3's DAS and ARR by A4's own two
numbers, and a whole game played to a top out and through name entry. None of it
is a window's, which is the point: both front-ends call it.

That leaves exactly what a screen and a keyboard are for, and B3–B11 are it. The
honest caveat `drive.py` already carries applies here twice over: these
substitute for, but do not replace, playing the game.

The native window **can** be scripted, and the recipe is in `CLAUDE.md` —
`osascript` sends it keys, `screencapture` takes its picture. It is not a test
and never will be: a key goes astray now and then and nothing errors when it
does, so a run of it must assert on an observable outcome — a config file that
gained a value, a process that exited, a screen with the right thing on it.

### G9.3 B1–B12

§17.3's A1–A10 for this front-end, checked one by one the way those were. Every
criterion is checked in **both** builds unless marked native or web.

| | Criterion | How it was checked |
|---|---|---|
| B1 | Build clean | `cargo clippy --all-features --all-targets -- -D warnings` silent; `cargo build --release --all-features` builds both binaries; `trunk build --release` succeeds. |
| B2 | §17.1 and §17.2 pass | `cargo test --all-features`: 417 unit, and the integration tests including §19.4's canary, `tests/pump.rs` and `tests/gui_render.rs`. |
| B3 | Attract on launch, PLAY starts a game | Both builds, looked at: the wordmark, the menu, the six-second panel and the drift, then `Enter` and a game. The tab's menu has four items, not five (§G8.1). |
| B4 | §10.1's bindings, DAS/ARR identical to the terminal's | A4's two numbers — a 50 ms tap is one cell either way, a 0.6 s hold reaches the wall and stops — asserted in `tests/pump.rs`, on the `shell::input` both front-ends call. The adapters are pinned separately (§G2.1). |
| B5 | `preview_count` 1–6, every source | `--preview 1` and the file's 6 in the window, `?preview=3` in a tab, each counted on screen; the §13.5 panel is the third source and is B9's. |
| B6 | A full game recorded | Native: a game played in `ftm-gui` to a top out, named, written — and read back by `ftm` on its high-score screen. Web: the same in a tab, then a reload, and the entry is on §13.5's sub-screen. |
| B7 | Clean exit, §16's warnings, a legible panic | A read-only config: the Options panel and the §6.2 first-exit write both warn, on stderr, after the window has gone, and the exit is 0. A config that is not TOML in a tab: exactly one warning, on the console. A panic patched into `logic` temporarily: named its file and line and exited 101. |
| B8 | Cadence invariance | `tests/pump.rs` at 60 Hz, 144 Hz and a jittery cadence; `frame_cap` reached from the setting (§G8.10). A tab backgrounded and restored: see below — this is the one that found a bug. |
| B9 | Hold and 180 off/on, three ways | `--no-hold --no-rot180`: the hold panel is gone, and neither binding is in §10.1's table or §13.3's panel. Both switched back on in the §13.5 panel, which wrote `hold_enabled = true` and `allow_180_rotation = true` to the file. |
| B10 | Focus loss pauses a game | Another application took the keyboard mid-game: the pause menu, over a well §9.17 had blanked. `tests/pump.rs` holds the shell's half, released keys and all. |
| B11 | *Web*: the canvas takes the keyboard | `document.activeElement` is the canvas on load, with no click. `Space` hard-drops rather than scrolling, and `Tab` leaves the focus where it is. |
| B12 | The front-ends build against the shell alone | `cargo check --no-default-features` and `make portable`, both green, and no module in `gui/` can name a `core` internal because there are none to name (§17.3 A10). |

**MG7.**

#### What B8 found

A backgrounded tab did not pause. It went on playing, throttled — 31 seconds
hidden advanced a level-1 game by 13 — exactly as G6 measured it and exactly
what §G4.7 was written to stop. The rule was right and the mechanism named in
§G8.7 was wrong: switching to another tab fires no `blur` at the canvas, which
keeps the document's focus, so `egui` reported a focused application that could
not hear a key. `host::visible` is the fix and §G8.7 now says what it is for.
The lesson is the general one this stage exists for: *§G4.7 had a test, and the
test passed, because the test asked the shell whether it pauses when told it has
lost the keyboard — and what was broken was the front-end's answer to "have I?"*

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
- **No pointer path** (§1.2). No click-to-select in a menu, in either build,
  and no touch controls in the web build: `EGUI-PLAN.md` G6 settled that, and the
  page says so (§G8.8). A click that focuses the canvas is the browser's, not
  the game's.
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
wholesale and inherit its constraints for no benefit. `EGUI-PLAN.md`'s *Hazards that
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
  non-zero exit; on the web it is `eframe::WebRunner`'s hook, which logs the
  message and the stack to the console (§G8.5).
- §12.1's 60 × 24 minimum. §G3 states this front-end's own, in cells rather than
  characters. §8.4's *rule* — the forced pause below it — is shared and has one
  implementation.
- §14's temp-file-and-rename. A filesystem technique, not a durability
  requirement; `localStorage` needs none.
