# Falling Tetromino Manager — The egui Front-End

**Version:** 0.5 — §G1-§G6 and the web build's half of §G8 are written;
§G7 and §G9 are still reserved, and §G6's sub-cell half is `EGUI.md` G10's.
**Date:** 2026-09-12
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
§G1 and §G2 were written by G5, §G8's web build by G6 — its `[gui]` table and
the rest of its query parameters wait for G12 — §G3 and §G4 by G7, §G5 by G8
and §G6's animations by G9. The rest is still the namespace and the
reservations below, deliberately, so that a `§G7` written in a doc comment
during G11 has somewhere agreed to land.

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
| G3 | Layout | `EGUI.md` G7 ✅ | The integer-cell metric, `LAYOUT_COLS` / `LAYOUT_ROWS`, the minimum `cell` and the too-small state below it (`FRONTEND.md` F6, §8.4). |
| G4 | The playing screen | `EGUI.md` G7 ✅ | §12.4's information — field, hold, next, stats, status — drawn as pixels rather than characters, `show_debug`, and the pause that losing the keyboard forces. |
| G5 | Overlays | `EGUI.md` G8 ✅ | §12.6's pause, game-over and name-entry boxes, and the §13.5 Options and §10.1 controls panels. |
| G6 | Animations | `EGUI.md` G9 ✅, G10 | §12.5's six animations in a pixel-native idiom, and the sub-cell gravity that `GameView::fall_progress` makes drawable. |
| G7 | The attract screen | `EGUI.md` G11 | §13's wordmark, menu, cycling panel and drifting background, laid out for a window rather than a 36 × 20 grid. |
| G8 | The web build | `EGUI.md` G6 ✅, G12 | The canvas and its keyboard focus, `localStorage` for §6.2 and §14, URL query parameters in place of §6.4's flags, and the `[gui]` config table. |
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
non-zero exit, and on the web it is `eframe::WebRunner`'s own panic hook (§G8.5).
Natively, §16's warnings are printed on stderr **after** the window has closed,
which is this front-end's reading of §16's "after teardown"; a tab has no such
moment, and reports them on the console as they arise (§G8.6).

### G1.3 The loop

`eframe` calls the application and the application asks to be called again, so
§15.2's seven steps are **not** a loop here — they are calls into
`shell::round::Round`, which `EGUI.md` G4 made a front-end's to drive
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
   resuming into an instant death. (Since `EGUI.md` G7 a *game* in either is
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

§7's screen field has one arm. A game that hands back `Next::Play` — §10.1's held
restart — starts a fresh one; `Next::Attract` and `Next::Quit` both close the
window, because there is no attract screen to return to until G11. A tab cannot
close itself, so there those two start a fresh game as well (§G8.1).

A window or a canvas that does not have the keyboard says so, across the middle
of the screen: **Click to play** (§G8.2). It is drawn in both builds, over
whatever else is on screen. At G5 it paused nothing; since G7 losing the keyboard
pauses a game in progress (§G4.7).

**G7 replaced the slice's screen** with §G3's layout and §G4's playing screen,
**G8 put §G5's boxes over it** and **G9 §G6's animations under them**. What is
still to come is §G6's sub-cell gravity (G10) and §G7's attract screen (G11).

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
that went on placing pieces for its absent player — and `EGUI.md` G7 settled it
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
- **It does not undo itself.** Focus coming back shows the pause menu, and the
  player leaves it and gets §9.17's countdown, exactly as after a resize.
- **The releases come first.** The adapter's synthesised releases (§G2.3) reach
  the shell on the pass focus is lost, before the pause; the pause then lets go
  of whatever else was held.
- **Click to play** (§G8.2) is still drawn over the screen while the keyboard is
  away. It says what to do; the pause is what makes it safe to take a moment
  doing it.
- **A hidden tab has no keyboard**, so this is also what stops a backgrounded
  game from creeping (§G8.7).

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

**The panel offers seven rows, not §13.5's eight.** §12.3's colour depth is the
terminal's alone: a window has no depths, no `mono` and no `$NO_COLOR`. Which
rows a panel offers is therefore the front-end's own answer, and it is carried
on `Session::settings`, which this front-end sets to `Setting::SHARED` at
start-up. **Both the screen and the shell navigate that same list**, so the
cursor can never land on a row the screen is not drawing — the failure a panel
that simply drew fewer rows than the menu walked would have.

This is the small half of the split `EGUI.md` G12 finishes, when a `[gui]`
table gives this front-end settings of its own to put in `Colour`'s place.
What §13.5 says about the panel is unchanged: presentation applies at once,
rules never — a game keeps the rules it started under, and a `Hold` switched
off here leaves the running game's hold box exactly where it was.

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
`GameEvent::PieceMoved` already fires for. `EGUI.md` G10 has the long form.

**A wrinkle, accepted.** §9.4 spawns a piece with minos in row 19 and drops it
one row, so a freshly spawned `T` has a mino above the visible field that
`GameView` omits (§12.7): three minos are drawn, not four, until it falls
again. That is pre-existing — it is why the well has no lid (§G4.1) — but a
sliding piece makes it more noticeable, because the fourth mino appears
abruptly against neighbours that are moving smoothly. Carrying a row of the
buffer zone in the view would fix it and is a much larger §12.7 change with §19
consequences of its own.

---

§G7 is reserved for `EGUI.md` G11 and is not yet written.

---

## G8. The web build

The same front-end, compiled to `wasm32-unknown-unknown` and served as a page.
It is **not a port**: `gui/app.rs`, `keys.rs`, `paint.rs` and the whole of
`shell/` and `core/` are the native build's code, unchanged. What differs is what
an entry point is made of — where the flags come from, where `FRONTEND.md` F1–F4
come from, and whether a run has an end — and all of it is in
`gui/host_web.rs`, `gui/query.rs` and the wasm `main` of `src/bin/ftm-gui.rs`.

`EGUI.md` G6 wrote §G8.1–§G8.9. G12 adds the `[gui]` table and the rest of
§6.4's flags as query parameters, with the list of which exists in which build.

### G8.1 The page, and a run with no end

`index.html` is a canvas (`id="ftm"`), a status line (`id="status"`) and a
footer, and nothing else. Its `<title>` is the window's — **Falling Tetromino
Manager** — and its icon is empty, because §1.3's trademark rules reach both.
`Trunk.toml` builds it: `trunk serve` to develop (`make run-web`), `trunk build
--release` for the artefact in `dist/` (`make web`). Asset URLs are relative, so
`dist/` works wherever it is put.

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
native build closes its window on `Next::Attract` or `Next::Quit`, the web build
starts a fresh game, the attract screen's stand-in until `EGUI.md` G11.

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
  says so once, on the read. Its wording is the shell's and names a directory,
  which a tab has not got; that is G12's to reword along with the rest of §6.2's
  web half.
- **The default document is never written.** §6.2 writes it on the first clean
  exit, and a tab has none. The config key is written when the §13.5 Options
  panel saves, and not before.

### G8.4 Query parameters for §6.4's flags

A tab has no argv; it has a URL. §6.4's flags are query parameters with the same
names, parsed in `gui/query.rs` into the same `Overrides` that `clap` produces
natively, and §6.1's precedence is unchanged: the query over the stored config
over the defaults. After G6 there is one — **`?seed=N`**, which makes a run
reproducible and, like `--seed`, never recorded (§14). Three rules differ from a
command line, because a URL is not only the game's:

- **A parameter the game does not know is ignored, silently.** Links pick up
  tracking tags on their travels, and a warning about each is noise.
- **A value that does not parse is a warning (§G8.6), and the run goes ahead
  without it** — `clap` refuses a bad flag and exits, which a tab cannot do.
- **A parameter given twice takes its last value.**

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

**Since `EGUI.md` G7 a game in progress does not creep: it pauses.** A hidden
tab reports no focus — `eframe` counts a hidden document as unfocused — so the
first `App::logic` pass after hiding forces the pause of §G4.7, and the player
comes back to the pause menu rather than to a game that went on without them. The
measurement above is G6's, from before that rule, and is kept because it is what
settled it: at a high level, nine seconds of play in thirty-six locks pieces
nobody placed. What still runs while hidden is the pump itself, and it is
correct at that cadence, so anything that is not a game in progress — the
attract screen, from G11 — creeps as described.

A hidden tab can hear no keys. `eframe` hands the same unconsumed input to every
hidden pass until one paints, so the adapter sees each focus change more than
once — harmless, because its answer to a focus change is idempotent (§G2.3).

### G8.8 Touch

There are no touch controls, and the page says so in its footer: *Falling
Tetromino Manager is played with a keyboard. There are no touch controls.* A web
build is a link someone will open on a phone, and a game that silently ignores a
finger is worse than one that explains itself; an on-screen control layer would
be a §1.2 amendment and a piece of design in its own right, and `EGUI.md` G6
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
  and no touch controls in the web build: `EGUI.md` G6 settled that, and the
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
  non-zero exit; on the web it is `eframe::WebRunner`'s hook, which logs the
  message and the stack to the console (§G8.5).
- §12.1's 60 × 24 minimum. §G3 states this front-end's own, in cells rather than
  characters. §8.4's *rule* — the forced pause below it — is shared and has one
  implementation.
- §14's temp-file-and-rename. A filesystem technique, not a durability
  requirement; `localStorage` needs none.
