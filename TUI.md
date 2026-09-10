# Falling Tetromino Manager — The Terminal Front-End

**Version:** 1.0
**Date:** 2026-09-10
**Companion to:** [FTM.md](FTM.md) (the specification),
[FRONTEND.md](FRONTEND.md) (the contract every front-end is written against),
[GUI.md](GUI.md) (the egui front-end), [EGUI.md](EGUI.md) (the front-end plan)

This document is normative for the **terminal front-end** — the `ftm` binary,
built with the `tui` feature. It is one of the ways the game of `FTM.md` is
played; `GUI.md` describes another. Everything here is about a character grid
and nothing here is about the rules.

## The sections this document owns

`FTM.md` was once whole, and hundreds of doc comments in `src/` cite its section
numbers. **Those numbers did not change when the sections moved here.** A
comment saying `§12.4` still resolves; it now resolves to this file rather than
to `FTM.md`. `FTM.md` keeps a stub at each vacated number saying where it went.

| § | Section | Was |
|---|---|---|
| 6.3 | [The `[display]` table](#63-the-display-table) | part of `FTM.md` §6.3 |
| 8 | [Terminal handling](#8-terminal-handling) | `FTM.md` §8 |
| 12.1–12.6 | [Rendering](#12-rendering) | `FTM.md` §12.1–§12.6 |
| 13 | [Attract screen](#13-attract-screen) | `FTM.md` §13 |
| 17.3 | [Acceptance criteria A1–A10](#173-acceptance-criteria-a1a10) | `FTM.md` §17.3 |

**§12.7 (the view model) and §12.8 (the event stream) stayed in `FTM.md`.** They
are the front-end contract rather than a rendering technique: every front-end
draws from a `GameView` and animates from a `GameEvent`, so they belong with the
specification and not with any one screen. `FRONTEND.md` is what collects the
obligations that fall out of them.

An unqualified `§n` in this document means `FTM.md` §n unless the number is one
of the five above. `§Gn` means `GUI.md`.

---

## Contents

| § | Section |
|---|---|
| 6.3 | [The `[display]` table](#63-the-display-table) |
| 8 | [Terminal handling](#8-terminal-handling) |
| 12 | [Rendering](#12-rendering) |
| 13 | [Attract screen](#13-attract-screen) |
| 17.3 | [Acceptance criteria A1–A10](#173-acceptance-criteria-a1a10) |

---

## 6.3 The `[display]` table

§6.3's schema, its ranges and its clamping and rejection rules are `FTM.md`'s and
are unchanged. Four of its `[display]` keys describe a character grid and have no
meaning off a terminal, so they are specified here:

```toml
[display]
# "auto" | "truecolor" | "256" | "16" | "mono"   (see §12.3)
color_depth   = "auto"
# Characters used to paint one occupied cell. Each must be exactly 2 display
# columns wide; one that is not is rejected with a warning and the default used
# (§12.2).
cell_filled   = "██"
cell_empty    = "  "
cell_ghost    = "▒▒"
```

The other two `[display]` keys — `show_grid` and `show_debug` — are **shared**
and stay in `FTM.md` §6.3. Both ask a question any front-end can answer: whether
to draw a grid behind the empty field, and whether to show the timing figures.
How each is drawn is the front-end's business; §12.4 has the terminal's answer.

A front-end that cannot honour a key **ignores** it and leaves it in the file
untouched, which is §6.2's existing rule for a key it does not recognise. The
four keys above therefore survive a GUI run, and `GUI.md` §G8's `[gui]` table
survives a terminal run.

---

## 8. Terminal handling

### 8.1 Startup

On start, in this order:

1. Parse CLI arguments; load and merge config (§6).
2. Install a panic hook that restores the terminal (§8.3) **before** printing the
   panic message, so a crash never leaves the user with a broken shell.
3. Enable raw mode.
4. Enter the alternate screen (`EnterAlternateScreen`).
5. Hide the cursor.
6. Enable bracketed paste **off**, mouse capture **off**.
7. Attempt to push keyboard enhancement flags (§8.2).
8. Create the `ratatui` terminal with the `CrosstermBackend` over `stdout`.

### 8.2 Key-release detection

Standard terminals report only key presses; a held key produces a stream of
auto-repeat presses at the operating system's repeat rate and produces no event
at all when released. This is inadequate for a game that needs to know whether
left is *currently* held.

The implementation must therefore support two input modes:

**Enhanced mode (preferred).** Push
`KeyboardEnhancementFlags::REPORT_EVENT_TYPES | DISAMBIGUATE_ESCAPE_CODES` via
crossterm's `PushKeyboardEnhancementFlags`, and verify support by querying
`crossterm::terminal::supports_keyboard_enhancement()`. When supported, the
terminal delivers `KeyEventKind::Press`, `::Repeat` and `::Release`, and the game
tracks true held-key state. DAS and ARR (§10.3) are then driven entirely by the
game clock, and terminal auto-repeat events are **ignored** (`KeyEventKind::Repeat`
is discarded).

**Legacy mode (fallback).** When enhancement is unsupported, there are no release
events. A key is treated as held from its first press until `hold_timeout`
milliseconds have elapsed with no further event for that key, where
`hold_timeout = 90 ms`. This is longer than any common terminal auto-repeat
interval (typically 30–50 ms) and shorter than a deliberate re-press. In legacy
mode:

- DAS and ARR are still driven by the game clock, not by the terminal's repeat
  rate; incoming repeat events only refresh the "still held" timestamp.
- Soft drop is applied while the down key is considered held, and stops
  `hold_timeout` after the last event. The resulting up-to-90 ms of overshoot is
  accepted.
- The status bar shows a small `legacy-keys` indicator when `show_debug` is on.

The active mode must be reported by `--print-config` and on the Controls panel of
the attract screen, because it materially changes feel.

### 8.3 Shutdown

Teardown runs on normal exit, on error, and from the panic hook, and is
idempotent:

1. Pop keyboard enhancement flags (if pushed).
2. Show the cursor.
3. Leave the alternate screen.
4. Disable raw mode.
5. Flush stdout.

Any warning accumulated during the run (bad config keys, unwritable high-score
file) is printed to stderr **after** teardown.

### 8.4 Resize

`Event::Resize` invalidates the whole frame and triggers a full redraw. If the
terminal is smaller than the minimum size (§12.1), the game switches to the
"terminal too small" screen; if a game was in progress it is forced into
`Paused` first, so the player is never killed by a window resize.

---

## 12. Rendering

§12.1–§12.6 below are the terminal's. **§12.7 (the view model) and §12.8 (the
event stream) are not here** — they are the contract every front-end draws
from, and they stayed in `FTM.md` under those same numbers.

### 12.1 Terminal size

- **Minimum supported size: 60 columns × 24 rows.**
- Below the minimum, all screens are replaced by a centred message:

  ```
  Terminal too small
  Need 60x24, have 48x20
  Resize to continue
  ```

  and, if a game was in progress, it is forced into `Paused` (§8.4).
- Above the minimum, the whole UI is centred as a fixed-size block; extra space
  becomes margin. The UI does **not** stretch, because the playfield's aspect
  ratio must stay correct.

### 12.2 Cell rendering

- One matrix cell is drawn as **two terminal columns**, so that a cell is roughly
  square in typical fonts. All widths in this section are given in cells; the
  character width is twice that.
- An occupied cell is `cell_filled` (default `██`) in the piece's foreground
  colour, on the default background.
- An empty cell is `cell_empty` (default two spaces), or `··` dimmed when
  `show_grid = true`.
- A ghost cell is `cell_ghost` (default `▒▒`) in the piece's colour, dimmed.
- All three glyph strings must be exactly two display columns wide; a config that
  violates this is rejected with a warning and the default is used.

### 12.3 Colour depth

`color_depth = "auto"` selects, in order:

1. `mono` if `$NO_COLOR` is set (any value) or stdout is not a TTY. This test
   comes first because it *overrides* the others: a terminal that advertises
   truecolor and sets `$NO_COLOR` wants no colour.
2. `truecolor` if `$COLORTERM` is `truecolor` or `24bit`.
3. `256` if `$TERM` contains `256color`.
4. `16` otherwise.

In `mono`, cells are drawn as the piece's mono glyph (§9.2) — `I`, `O`, `T`, `S`,
`Z`, `J`, `L` — doubled (`II`), the ghost as `..`, and all emphasis uses bold and
reverse video only. The game must be fully playable in `mono`.

#### The levelled palette

§9.2's seven colours are equally saturated but not equally bright: their Rec.709
luma runs from blue's 17 to yellow's 223. A `J` piece at 17 is hard to pick out
of a dark terminal at all, and §13.2's wordmark, whose letters sit side by side,
reads as two different weights. So everything drawn in a piece colour — the
field, the ghost, the previews, the hold box, §13.4's drifting background and
§13.2's wordmark — draws from this palette instead:

| Colour | §9.2 | luma | Drawn | luma | 256-colour |
|---|---|---|---|---|---|
| Cyan | `#00F0F0` | 189 | `#00F0F0` | 189 | 51 |
| Green | `#00F000` | 172 | `#00F000` | 172 | 46 |
| Orange | `#F0A000` | 165 | `#F0A000` | 165 | 208 |
| Yellow | `#F0F000` | 223 | `#F0F000` | 223 | 226 |
| Purple | `#A000F0` | 51 | `#D58FF8` | 165 | 177 |
| Red | `#F00000` | 51 | `#F44040` | 102 | 203 |
| Blue | `#0000F0` | 17 | `#4848F4` | 84 | 63 |

A hue is lifted by blending it toward white, which is the only direction
available: a saturated blue or purple cannot be made as bright as cyan on any
display, so the brightness is bought with saturation. How much of that is worth
spending differs by hue, so the three do **not** land on one number. Purple
already carries two primaries and reaches 165 — orange's, the dimmest of the
four that were already bright — while still reading as purple. Red and blue
carry one primary each and gray out far faster: at 165 they are salmon and
lavender rather than red and blue, so they are lifted 45% of that far instead,
keeping about three-quarters of their saturation. A saturated hue also *looks*
brighter than its luma says (Helmholtz–Kohlrausch), most so for blue, which
closes much of the gap the numbers still show.

The four colours §12.3 leaves alone keep §9.2's own 256-colour entry, which is
authoritative — its orange is a deliberate choice, not the nearest cube cell.
A lifted colour has no such entry, so it takes the cube cell nearest the value
drawn. At 16 colours and in monochrome the palette cannot express a luminance
and §9.2 stands as written.

#### Dimming

Dimming for ghosts and inactive UI uses: an RGB scale of 0.45 in truecolor, a
darker palette entry in 256-colour, and the `DIM` attribute in 16-colour. The
scale runs from the **levelled** colour, not from §9.2's, so a piece and its
ghost are the same hue.

### 12.4 Playfield screen layout

The screen is a fixed block **44 characters wide by 23 rows tall**, centred in
the terminal. All widths below are given in characters; remember that one matrix
cell is two characters wide (§12.2).

```
 hold/stats      playfield        next
┌──────────┐ ┌──────────────┐ ┌──────────┐
│  10 ch   │ │    22 ch     │ │  10 ch   │    10 + 1 + 22 + 1 + 10 = 44 ch
└──────────┘ └──────────────┘ └──────────┘
```

Concrete mock-up at `preview_count = 5`, drawn to exact size (44 × 23):

```
▗▄▄▄▄▄▄▄▄▖ ▐                    ▌ ▗▄▄▄▄▄▄▄▄▖
▐ HOLD   ▌ ▐                    ▌ ▐ NEXT   ▌
▐  ██    ▌ ▐                    ▌ ▐  ████  ▌
▐██████  ▌ ▐                    ▌ ▐  ████  ▌
▝▀▀▀▀▀▀▀▀▘ ▐        ██          ▌ ▐        ▌
           ▐      ██████        ▌ ▐██      ▌
▗▄▄▄▄▄▄▄▄▖ ▐                    ▌ ▐██████  ▌
▐ SCORE  ▌ ▐                    ▌ ▐        ▌
▐  12480 ▌ ▐                    ▌ ▐    ██  ▌
▐        ▌ ▐                    ▌ ▐██████  ▌
▐ LEVEL  ▌ ▐                    ▌ ▐        ▌
▐      4 ▌ ▐                    ▌ ▐        ▌
▐        ▌ ▐                    ▌ ▐████████▌
▐ LINES  ▌ ▐                    ▌ ▐        ▌
▐     37 ▌ ▐                    ▌ ▐  ██    ▌
▐        ▌ ▐                    ▌ ▐██████  ▌
▐ TIME   ▌ ▐                    ▌ ▝▀▀▀▀▀▀▀▀▘
▐  02:14 ▌ ▐        ▒▒          ▌           
▝▀▀▀▀▀▀▀▀▘ ▐      ▒▒▒▒▒▒        ▌           
           ▐      ██████████    ▌           
           ▐██████████████████  ▌           
           ▝▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▘           
               B2B  COMBO x3                
```

Rules for the layout:

- **Box borders**: every box on this screen — playfield, hold, stats, next and
  the debug strip — is drawn one character thick in **quadrant half blocks**,
  inked on the half of the border cell that faces the interior: `▗▄▖` above,
  `▐` and `▌` down the sides, `▝▀▘` below. On all four sides except the
  playfield's top, which is open (below). Not the line-drawing set: a `│` is
  inked down the *middle* of its cell, so the wall it draws stands half a cell
  away from what it encloses, and with a matrix cell two characters wide (§12.2)
  that half cell reads as a gap. Inking the facing half instead puts the
  boundary of the playfield exactly on the boundary of a cell. The overlay boxes
  of §12.6 keep their double line, which is what distinguishes them from the
  screen behind.
- **Playfield box**: 20 characters (10 cells) of interior, 20 rows tall, and
  **open at the top**: the walls and the floor are drawn, the lid is not. The
  row the top border would have occupied is left open above the field — the
  mouth the piece comes in through — and the twenty drawn rows stay at the
  bottom of the box, so the floor, the status line and every row between them
  sit exactly where a closed box put them. Only matrix rows `20..=39` are
  drawn. A piece straddling row 20 is clipped: minos above row 20 are simply
  not drawn, which is why nothing is ever drawn in the mouth itself.
- **Hold box**: interior 4 cells × 2 cells, enough for any piece in `North`
  orientation. The piece is centred horizontally in the box. Drawn dimmed when
  hold is locked out for the current piece. Omitted entirely when
  `hold_enabled = false`, and the left column then contains only the stats box.
- **Next box**: interior 4 cells (8 characters) wide; one slot per previewed
  piece, each slot 2 cell-rows tall, with one blank row between adjacent slots.
  Its height is `2 (border) + 1 (label) + 2 × count + (count − 1)` rows: 17 rows
  at `preview_count = 5` and 20 at 6, so it always fits inside the 22-row
  playfield box and never needs to scroll at the minimum terminal size.
  Slot 0 (the next piece to spawn) is at the top and is drawn at full
  brightness; later slots are drawn progressively dimmer, in three steps
  (100 %, 75 %, and 55 % for slot 2 and beyond) where the colour depth allows.
- Should a future layout leave too little room for every slot, as many as fit are
  drawn and the last visible row of the box shows `+N` right-aligned.
- **Stats box**: score, level, lines, time (`MM:SS`, capped at `99:59`). The
  score is drawn as bare digits here, and here alone: the interior is 8
  characters with one of them spent on the margin, which holds the seven digits
  a game can plausibly reach — but only six once a comma costs a column every
  thousand, and a box grown to fit would take the whole 44-character block with
  it. Wherever a score is instead read at rest — §12.6's game-over box, §13.3's
  panel and §13.5's table — its digits are **grouped in threes** with `,`.
- **Debug strip**: with `show_debug = true` a bordered strip 44 characters wide
  and 5 rows tall is drawn **directly beneath the block**, making the whole
  thing 44 × 28. It shows the frame rate, ticks elapsed, any dropped ticks
  (§15.2), gravity in G, `fall_period`, lock-delay ticks remaining, DAS charge,
  current bag contents, and the input mode (`enhanced` or `legacy`).

  Beneath rather than inside the left column, because the column's interior is
  8 characters wide and the block has at most 3 spare rows under the stats box:
  those nine figures do not fit there at any height. The strip is a developer's
  read-out and not a supported layout, so it does not change the minimum
  terminal size of §12.1 — a terminal with fewer than 28 rows is simply drawn
  without it, and the game is unaffected.
- **Status line** (bottom, centred): shows `B2B` when the back-to-back chain is
  active, `COMBO xN` when the combo counter is ≥ 1, and the most recent clear's
  name (`QUAD`, `T-SPIN DOUBLE`, `PERFECT CLEAR`, …) for 1.5 s after it occurs.

### 12.5 Animations

All animations are purely cosmetic and must not affect the rules or timing of the
core. Each is **started by a `GameEvent`** (§12.8) and then runs on the shell's
own wall clock, independently of the tick rate. The core never knows an animation
is in progress.

| Animation | Duration | Description |
|---|---|---|
| Line clear flash | `line_clear_delay_ms` | Cleared rows alternate white / piece-colour at 12 Hz, then collapse. |
| Hard-drop trail | 120 ms | The columns the piece passed through are drawn dimmed behind it. |
| Lock flash | 80 ms | The locked piece is drawn white for one frame set. |
| Level-up banner | 1.2 s | `LEVEL 5` centred over the playfield, fading. |
| Perfect clear banner | 1.5 s | `PERFECT CLEAR` centred, in the seven piece colours cycling. |
| Game-over wipe | 500 ms | The stack greys from the top row downwards. |

If the frame rate cannot be sustained, animations are skipped rather than slowed.

### 12.6 Overlays

Overlays are drawn centred over the playfield with a cleared (space-filled)
background and a double-line border.

A menu of plain items inside one — the pause menu below — is centred as a
**block**: the marker and the widest label together, with the labels
left-aligned within it, so the items do not shuffle sideways as the cursor moves
and the block is not pushed against one wall by the marker's indent. The attract
screen's menu (§13.3) is laid out the same way. The Options panel of §13.5 is
not: it is a two-column list of setting and value, and fills the box's width.

**Pause**

```
╔══════════════════╗
║      PAUSED      ║
║                  ║
║  ▸ Resume        ║
║    Restart       ║
║    Options       ║
║    Controls      ║
║    Quit to menu  ║
╚══════════════════╝
```

**Options** opens the §13.5 Options panel over the paused playfield. §6.1 calls
it "the in-game Options screen", and this is the in-game way in; the attract
screen's OPTIONS item (§13.5) reaches the same panel. **Controls** likewise
opens the §10.1 binding table over the same blanked playfield, and is the same
box the attract screen's CONTROLS item shows. Both return to the pause menu,
with the cursor back on the item that opened them; neither abandons the run.

**Game over**

```
╔══════════════════════╗
║      GAME OVER       ║
║                      ║
║  SCORE      12,480   ║
║  LEVEL           4   ║
║  LINES          37   ║
║  TIME        02:14   ║
║  PIECES        128   ║
║  PPS           0.9   ║
║                      ║
║    Press any key     ║
╚══════════════════════╝
```

**Name entry** (only when the score qualifies for the top ten):

```
╔══════════════════════╗
║    NEW HIGH SCORE    ║
║          #3          ║
║                      ║
║   Name: msandifo_    ║
║                      ║
║   Enter to confirm   ║
╚══════════════════════╝
```

Name entry accepts up to 12 printable ASCII characters, `Backspace` deletes,
`Enter` confirms (an empty name becomes `ANON`), `Esc` cancels and discards the
score. The field is pre-filled with `$USER` (or `$USERNAME` on Windows),
truncated to 12 characters.

---

## 13. Attract screen

> This section is deliberately provisional: it is the one part of the
> specification expected to be refined (§18), and the only one whose looks
> §17.3 never judged. It has been built and looked at, and what is written here
> is what the code draws — §13.3's mock-up is a test — but the rest of the
> specification does not depend on its details, and changes here should not
> disturb §9.

### 13.1 Purpose and behaviour

The attract screen is what the program shows whenever no game is in progress. It
must: identify the game, show how to start it, teach the controls without being
asked, and be pleasant to leave running.

### 13.2 Wordmark

Drawn from block characters (not game cells, so the whole screen still fits in
60 columns), 30 characters wide and 5 rows tall, with each character of the
letterforms doubled horizontally so that three letters still carry the screen:

```
████████  ████████  ██      ██
██          ████    ████  ████
██████      ████    ██  ██  ██
██          ████    ██      ██
██          ████    ██      ██
```

The three letters take the `I`, `S` and `T` tetromino colours — cyan, green and
purple — left to right, from §12.3's levelled palette, which is what every other
piece colour on the screen is drawn from too. Levelling them is what stops the
purple `M` reading as a lighter weight of the same letterform than the cyan `F`;
§12.3 has the table and the derivation. §13.6's idle cycle walks all seven, so
the four the static wordmark never shows are levelled by the same rule.

This is an original block-letter wordmark. **The official Tetris logo must not be
used, reproduced, or approximated**, and no official colours-as-branding, styling
or artwork may be copied (§1.3).

### 13.3 Layout

```
               ████████  ████████  ██      ██
               ██          ████    ████  ████
               ██████      ████    ██  ██  ██
               ██          ████    ██      ██
               ██          ████    ██      ██

                 FALLING TETROMINO MANAGER

                     ▸ PLAY
                       HIGH SCORES
                       CONTROLS
                       OPTIONS
                       QUIT

            ┌──────────────────────────────────┐
            │    ←→ move          ↑ rotate     │
            │     ↓ soft drop SPACE drop       │
            │     Z rotate ccw    A rotate 180 │
            │     C hold                       │
            └──────────────────────────────────┘
              v1.0   ↑↓ select   ENTER start
```

The screen is a fixed block **36 characters wide by 21 rows tall**, centred in
the terminal like every other screen (§12.1). The mock-up above shows it centred
in 60 columns. The width is the controls panel's, not the wordmark's: the
wordmark is 30 characters and is centred within the block.

The 21 rows are wordmark (5), blank, subtitle, blank, menu (5), blank, panel
(6), footer — the footer sits directly under the panel, with no gap.

- The controls panel lists only the bindings that are actually available: the
  `C hold` entry is omitted when `hold_enabled = false`, and the 180° entry is
  shown only when `allow_180_rotation = true`. The remaining entries reflow to
  fill the panel, two to a row.
- The panel is **four rows** tall inside its border. Two of the seven control
  entries are optional, so three rows of two cannot hold them all with every
  setting turned on; four rows is also what the high-score face wants, which is
  a heading and the top three.
- The menu is navigated with `↑`/`↓` (wrapping) and activated with `Enter` or
  `Space`. The selected item is marked `▸` and drawn in the `I`-piece cyan.
- The panel beneath the menu **cycles every 6 seconds** between three faces:
  1. the quick control summary shown above;
  2. the top three high scores, under a `HIGH SCORES` heading;
  3. a one-line rules reminder (`Clear 4 rows at once for a QUAD` and similar,
     rotating through a short list).
  The cycle pauses while a menu item other than **PLAY** is selected.

### 13.4 Background animation

Behind the wordmark and menu, a slow ambient animation runs at 10 fps:

- Tetromino outlines (drawn with `░░` cells in each piece's colour, heavily
  dimmed) drift down the screen from the top, one new piece every ~1.2 s at a
  random column, falling one row every ~0.6 s, each in a random orientation.
- Pieces are removed when they leave the bottom of the screen; at most 12 exist
  at once.
- The animation never draws over the wordmark, menu, panel or footer: those
  regions are painted after it, opaquely.
- The animation is disabled in `mono` colour mode and when `show_debug` is on.

### 13.5 Sub-screens

- **HIGH SCORES** — the full top-ten table (rank, name, score, level, lines,
  date), with the most recently added entry highlighted. `Esc` returns.
- **CONTROLS** — the full binding table from §10.1, plus the active input mode
  (§8.2). `Esc` returns.
- **OPTIONS** — an editable list of the settings most worth changing without a
  text editor: preview count (1–6), starting level (1–15), ghost piece on/off,
  **hold on/off**, **180° rotation on/off**, lock-down rule, colour depth, grid
  on/off. `←`/`→` change the selected value, wrapping at each end, and `Esc`
  saves the config file (§6.2) and returns.

  The same panel is reached from the pause menu (§12.6), which is what §6.1
  means by "the in-game Options screen". Presentation settings — colour depth
  and the grid — take effect the moment the panel is left. Rules settings do
  not: a game already in progress keeps the rules it started under, so
  toggling hold, 180° rotation, the preview count, the starting level or the
  lock-down rule takes effect for the next game. This is what §6.5's split is
  for, and it is also the only answer that keeps a run deterministic (§15.4).

### 13.6 Idle behaviour

After 60 seconds with no key press on the attract screen, the wordmark's
per-letter colours begin a slow cycle (one step per second) to show the program
is alive. Any key stops it. (A self-playing demo is deliberately **not** in scope
for v1.0; see §18.)

---

## 17.3 Acceptance criteria: A1–A10

These are the **terminal front-end's** acceptance criteria and always were: A3
and A5 name screens, A4 names keys, A7 names `stty`, A8 names SGR sequences. The
egui front-end's are B1–B12 in `GUI.md` §G9, and are checked one by one the way
these were. §17.1 and §17.2 — the core unit tests and the integration tests —
stay in `FTM.md` and are not front-end-specific.

A1–A10 were signed off at `PLAN.md` Stage 12; `CLAUDE.md` records how each was
checked and what two of them turned up.

The terminal front-end is complete when:

1. `cargo build --release` produces a binary with no warnings, and
   `cargo clippy -- -D warnings` is clean.
2. All tests in §17.1 and §17.2 pass.
3. The attract screen appears on launch, and **PLAY** starts a game.
4. All controls in §10.1 behave as specified, with working DAS.
5. `preview_count` is honoured for every value 1–6, from both the config file and
   `--preview`, and the layout adapts.
6. A full game can be played to a top out, the score is recorded, and it appears
   on the attract screen's high-score panel.
7. Quitting at any point restores the terminal exactly as it was found —
   verified by `stty -a` before and after.
8. The game is playable with `--color mono` and with `NO_COLOR=1`.
9. Hold and 180° rotation can each be turned off and on from the config file, the
   command line and the Options screen; when off, the key does nothing and the
   binding disappears from the hold box, the controls overlay and the attract
   screen's controls panel.
10. The renderer compiles against `GameView` alone. This is enforced by the
    compiler rather than audited: every module inside `core` is `pub(crate)`,
    so the core's whole public surface is its façade, and nothing under `ui/`
    can name `Game` or a rules module even by accident.

    The façade is `Game` itself (`new`, `tick`, `view`, `debug`), the input
    types `Action`, `Actions`, `Shift` and `TickInput` — which only `app` and
    `input` use — and the view and event types with the vocabulary they are
    written in: `GameView`, `PieceView`, `DebugView`, `VIEW_WIDTH`,
    `VIEW_HEIGHT`, `PlayState`, `GameEvent`, `ClearKind`, `ScoreReason`,
    `TopOutCause`, `OFF_SCREEN`, `PieceKind`, `Colour` and `Rotation`.

    The last three are in the list because a client handed a `GameView` has to
    *draw* it: the view's cells and its hold and next slots are `PieceKind`s,
    and turning one into minos on a screen needs §9.3's cell patterns and the
    `Rotation` they are indexed by, and §9.2's `Colour`. None of them is a
    rule. This is the check that §19 stays reachable.
