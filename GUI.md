# Falling Tetromino Manager — The egui Front-End

**Version:** 0.2 — §G1, §G2 and the web build's half of §G8 are written;
§G3–§G7 and §G9 are still reserved.
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
§G1 and §G2 were written by G5, and §G8's web build by G6 — its `[gui]` table
and the rest of its query parameters wait for G12. The rest is still the
namespace and the reservations below, deliberately, so that a `§G4` written in a
doc comment during G7 has somewhere agreed to land.

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
| `App::logic(ctx, frame)` | steps 1–4, 6–7 | Drains `ctx.input(…).events` into `Round::key`, calls `Round::advance`, then `ctx.request_repaint_after(Round::deadline(now))`. |
| `App::ui(ui, frame)` | step 5 | Paints what the last `Round::frame` reported. |

Three rules bind that, and each is `FTM.md`'s rather than this document's:

1. **The game advances by §15.1's tick and by nothing else.** `stable_dt` is
   never a step size and frames are never counted. `App::logic` is called while
   the window is hidden and `App::ui` is not, so a hidden window keeps playing
   and simply is not drawn. A backgrounded tab is the same case at a throttled
   cadence (§G8.7), and §15.2 step 4's catch-up cap is what stops either from
   resuming into an instant death.
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
window, because there is no attract screen to return to until G11. A tab cannot
close itself, so there those two start a fresh game as well (§G8.1).

A window or a canvas that does not have the keyboard says so, across the middle
of the screen: **Click to play** (§G8.2). It is drawn in both builds, over
whatever else is on screen, and it pauses nothing — whether losing focus should
pause a game is `EGUI.md` G7's question.

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

A browser tab puts a page between the keyboard and the game, and the page wants
`Space`, the arrows, `Tab` and `Backspace` for itself. §G8.2 is normative for
what the web build does about it: the canvas takes the keyboard on load, those
keys are kept from the page for as long as it has it, and it says so when it
has not. The adapter above is unchanged by any of it — the keys reach it as
they do natively. There is no pointer path in either build (§1.2).

---

§G3–§G7 are reserved for `EGUI.md` G7–G11 and are not yet written.

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

It still plays. A long absence at a high level can lock pieces the player did not
place, which is the case `EGUI.md` G7's focus-loss pause is for; until then this
is §G2.3's rule, that losing focus does not pause, in its web form.

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
