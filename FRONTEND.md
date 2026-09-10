# Falling Tetromino Manager — The Front-End Contract

**Version:** 1.0
**Date:** 2026-09-10
**Companion to:** [FTM.md](FTM.md) (the specification), [TUI.md](TUI.md) (the
terminal front-end), [GUI.md](GUI.md) (the egui front-end),
[EGUI.md](EGUI.md) (the plan that builds it)

This is the document a **fourth** front-end reads first. It is short on purpose:
it says what a front-end must provide, what it may assume, and what it must never
do, and nothing about how any particular screen looks. If adding a front-end is a
bounded task, this is the document that makes it one.

It owns **no section numbers**. `FTM.md`'s numbering is stable and every rule
here is normative somewhere in it; this document collects the obligations that
fall on the outermost layer, with a reference to where each is stated. Where this
document and `FTM.md` disagree, `FTM.md` wins and one of them is amended in the
same commit.

> **Status.** The contract is written; the code that expresses it lands across
> `EGUI.md` stages G1–G4. Until it has, the terminal front-end reaches straight
> through the shell for its clock and its files, and `shell/` does not exist as a
> directory. Nothing below is aspirational about the *rules* — every one of them
> is already normative — but the signatures are named as the plan will build
> them.

---

## Contents

- [The three layers](#the-three-layers)
- [What a front-end must provide](#what-a-front-end-must-provide)
- [What a front-end may assume](#what-a-front-end-may-assume)
- [What a front-end must never do](#what-a-front-end-must-never-do)
- [Adding a front-end](#adding-a-front-end)

---

## The three layers

```
  core  ── the rules. No I/O, no clock, fixed 1/60 s ticks, deterministic.
    │     §3.1. Every module pub(crate); the façade is the whole surface (§17.3 A10).
    │     Speaks GameView (§12.7) and GameEvent (§12.8) and nothing else.
    ▼
  shell ── everything above the rules that is not a screen: §6's config, §10's
    │     bindings and DAS/ARR, §7's state machine, §14's table, §12.5's timers,
    │     §9.2's palette. Front-end-agnostic AND platform-free: no clock, no
    │     filesystem, no entropy, no calendar (§3.1).
    ▼
  front-end ── a terminal, a window, a browser tab, a game framework.
              Owns the loop, the screen, the keyboard, and the four capabilities.
```

The inner boundary is held by the compiler: nothing above `core` can name a
`core` module (§17.3 A10). The outer boundary is held by the compiler too, in two
CI steps that are worth more than any review:

```
cargo check --no-default-features                                  # shell alone
cargo check --no-default-features --target wasm32-unknown-unknown  # and portably
```

If the second is green, the shell needs nothing from the platform, and a new
front-end is free to be a browser tab or a game framework with its own asset
story. **A `cfg(target_arch)` in `shell/` is a bug**, not a portability measure.

---

## What a front-end must provide

Seven things. The first four are `FTM.md` §3.1's *capabilities* — the four
facilities the shell deliberately does not have.

### F1 — Time: a monotonic stamp

The shell never calls a clock. It is handed a `Stamp`: microseconds since the
front-end started, as a `u64`. Microseconds rather than milliseconds because
§12.5's line-clear flash alternates at 12 Hz and §10.3's ARR can be a single
tick, and millisecond resolution is visibly coarse at both.

Two properties are **normative**, because every timing decision above the core
rests on them:

- **Monotonic.** A stamp handed over must never be earlier than one already
  handed over. Every subtraction in the shell is saturating, so a violation
  degrades rather than panics — but DAS goes wrong and the §12.5 animations
  stutter, and nothing will point at the front-end when they do.
- **Wall-clock-paced.** One second of stamps is one second. A front-end may not
  slow, speed or pause the stamp to slow, speed or pause the game; §7 and §15 are
  where the game is paused, and §15.2 step 4 is where a suspended machine's
  arrears are discarded.

A stamp is what makes the whole shell testable without a clock — construct them
arithmetically and the property §17.1 gives the core extends one layer out.

Natively this is an `Instant` captured at start-up; in a browser it is
`performance.now()`; in a frame-based game framework it is `get_time()` in
seconds, converted to microseconds **once, at the boundary, and never in the
rules**.

### F2 — Storage: two slots of text

```rust
pub trait Storage {
    fn read(&self, slot: Slot) -> Result<Option<String>, StorageError>;
    fn write(&mut self, slot: Slot, contents: &str) -> Result<(), StorageError>;
}
pub enum Slot { Config, HighScores }
```

Two slots, both text: §6.2 is TOML and §14 is JSON, and neither is large. A
`Slot` rather than a path, because a browser tab has no paths.

The obligations are `FTM.md`'s, not the trait's:

- **§6.2's location** for the native front-ends, `localStorage` for the web build.
  Native front-ends share one file deliberately, so a setting written by one is
  read by the other.
- **§14's durability**: a run that dies mid-write leaves the old table or the new
  one, never a truncated file. On a filesystem that is a temp file renamed over
  the target; `localStorage` needs nothing, because `setItem` is already atomic.
  The trait promises durability, not a technique.
- **§16's rule that a failure is a warning and never an abort.** A missing file, a
  malformed one, an unwritable directory: all four of §16's failure paths degrade
  to a documented default and add a line to the warnings. The game must never
  fail to start because of storage.
- **§6.2's preservation rule.** Saving must not drop a table or a key that
  another front-end wrote. This one is easy to pass by accident and easy to fail
  silently, because each front-end's own settings survive perfectly.

### F3 — Entropy: a seed

One `u64` per game, from `FnMut() -> u64`. That is the *whole* entropy budget of
the program: `core/bag.rs` expands the seed with §9.6's own PCG32 and draws its
range with Lemire's method, both written out, because `rand` has changed each of
those once already and either change silently makes every recorded seed name a
different game.

A front-end supplies bits, not a generator. Natively that is `rand::random`; in a
browser it is two calls to `Math.random()` composed into a `u64` — which is what
keeps `getrandom`'s wasm backend and its build-time `--cfg` out of the tree
entirely.

### F4 — Date: a string

§14's entries carry a date stamp. The shell is handed one, `FnOnce() -> String`,
in `YYYY-MM-DD`. A calendar is a platform facility like the other three;
`chrono` is a front-end dependency and not a shared one.

### F5 — A key event stream

The front-end decodes its own keyboard into the neutral type and hands events
over one at a time. §10.1's key-name grammar is shared property — it is what a
player's `[keys]` table is written in — so the vocabulary is the shell's:

```rust
pub enum Key { Left, Right, Up, Down, Enter, Tab, Esc, Backspace, F(u8), Char(char) }
pub struct Mods { pub ctrl: bool, pub alt: bool, pub shift: bool, pub logo: bool }
pub enum KeyKind { Press, Repeat, Release }
pub struct KeyEvent { pub key: Key, pub mods: Mods, pub kind: KeyKind }
```

`Space` is `Key::Char(' ')`. A key the neutral type cannot express — `Insert`, a
media key — is dropped by the adapter, which is what the bindings table does with
it anyway.

Four obligations:

- **Drain, don't ration.** §15.2 step 2 requires *all* available events to reach
  the shell before the frame advances. A front-end that hands over one event per
  frame makes fast typing lag behind.
- **Repeats are distinguishable and are discarded.** The operating system's
  auto-repeat is not the game's: DAS and ARR are §10.3's, driven by the stamp.
  A front-end that reports repeats must mark them `Repeat` so the shell can
  ignore them (§8.2). One that cannot report releases at all must synthesise held
  state the way §8.2's legacy path does, and say so on the Controls panel,
  because it materially changes feel.
- **The stream must be synthesisable.** Not every front-end has events. A
  framework with polled per-frame input diffs its key set between frames and
  manufactures the stream. Nothing in the shell may therefore depend on receiving
  every intermediate event, on sub-frame ordering between two keys, or on an
  event arriving at a moment other than a frame boundary. DAS/ARR already
  satisfies this, because it works off accumulated time and not off event counts.
  **This is the constraint most easily broken by an innocent-looking change.**
- **Disabled keys are dropped at this boundary** (§10.1). With `hold_enabled` or
  `allow_180_rotation` off, the key is inert — and inert means it cannot reset a
  lock-delay timer as a side effect either.

### F6 — A viewport, and a minimum for it

A front-end draws §12.7's `GameView` however it likes; `TUI.md` §12.4 and
`GUI.md` §G4 are two answers and neither constrains a third. What is shared:

- **It states a minimum viewport** and reports whether the viewport currently
  meets it. Below it, every screen is replaced by a legible "too small" message
  naming what is needed and what there is.
- **§8.4's rule is the shell's**: below the minimum, a game in progress is forced
  into `Paused` *before* the screen is replaced, and its held keys are released,
  so a player is never killed by a resize. The pause does not undo itself when
  there is room again; the player leaves it and gets §9.17's countdown for it.
- **Rendering never panics at any size**, including one absurdly small (§17.2).

### F7 — The loop

The front-end owns the loop, because a windowing system calls an application
rather than being called by one. §15.2's steps become methods it calls:

```rust
round.key(session, &event, now);      // step 2, once per event drained
round.advance(session, now);          // steps 1, 3-5; returns Some(next) when over
round.frame(now);                     // everything needed to draw, as one value
round.deadline(now);                  // how long it may wait before advancing again
round.viewport(fits);                 // F6
```

- **`advance` must be correct at any cadence**, and it is: the accumulator is
  over real elapsed time, not over frames. 60 Hz, 144 Hz, twice in a millisecond,
  or once after a minute in a backgrounded tab all give the same game.
- **`deadline` is advice, not a frame rate.** A front-end may be woken sooner by a
  key, by a compositor or by a tab regaining focus, and one that renders at vsync
  may ignore it entirely. `advance` must be correct without it.
- **Drawing is the front-end's decision.** `frame()` returns the *state*. A
  retained-mode front-end that diffs against a previous buffer may compare it and
  skip an unchanged draw; an immediate-mode one rebuilds every repaint and must
  not grow a comparison.

---

## What a front-end may assume

- **`GameView` is complete.** Everything any screen needs to draw is in it
  (§12.7), it is owned and serialisable, and building it does not mutate the
  game. If a screen needs something the view has not got, the answer belongs in
  the view or in an event — never in a back channel to the core.
- **`GameEvent` says what happened** (§12.8), in the order the rules produced it,
  with coordinates already in visible-field terms so the buffer zone need never
  be thought about. Dropping every event changes nothing about the game, which is
  what makes animation provably free of side effects.
- **The shell has already resolved input.** DAS, ARR, held state, bindings,
  disabled keys, the restart hold: all done. A front-end delivers key events and
  receives a drawable frame.
- **Input the shell has resolved is held until a tick consumes it.** A frame may
  legitimately run zero ticks — at 144 Hz most frames do — and a keypress in one
  of them is not lost.
- **The same `RulesConfig` and seed give the same game, in every front-end.**
  That is §15.4, and it is the property that lets a bug reported against one
  front-end be reproduced in another.

---

## What a front-end must never do

- **Name a `core` module.** `GameView`, `GameEvent` and the façade's vocabulary
  are the whole surface (§17.3 A10). This is checked by the compiler.
- **Own the tick rate.** Never advance the game by frame time, by `stable_dt`, or
  once per repaint. §15.1's 1/60 s tick and §15.2 step 4's accumulator are the
  only path, and every alternative is a desync (§19.2, constraints 2–4).
- **Read a clock the shell has not been told about**, or reach the filesystem,
  entropy or a calendar on the shell's behalf. That is what F1–F4 are for.
- **Interpolate anything but gravity.** Sub-cell *falling* is the one fractional
  position a front-end may draw. Horizontal movement and rotation stay snapped to
  the cell in every front-end, because no fractional state for them exists in the
  core and inventing one costs input latency exactly where players feel it.
  Motion there is conveyed by trails and pulses off `PieceMoved`, never by
  drawing the piece off its true cell.
- **Let an animation reach the rules.** §12.5's timers see events and a stamp and
  nothing else. If an animation seems to need to ask the core something, the
  answer belongs in `GameView` or in the event.
- **Add a pointer path.** §1.2 makes mouse input a non-goal in every front-end,
  and it is a rule about the game rather than about terminals. Touch is an open
  decision in `EGUI.md`; whichever way it goes it is a §1.2 amendment, not a
  front-end's addition.
- **Merge `RulesConfig` into `PresentationConfig`** (§6.5), or let the Options
  panel apply a rules change to a running game (§13.5). A game keeps the rules it
  started under, everywhere.
- **Drop another front-end's config.** §6.2. Silent, permanent, and invisible in
  every test that only checks its own settings.
- **Call a four-line clear anything but a `QUAD`** (§1.3) — and §1.3's trademark
  rules cover the window title, the page title, the wordmark and any icon or
  favicon, not just the text on a screen.

---

## Adding a front-end

There is deliberately **no `trait Frontend`** and there should not be one. The
front-ends share the shell by *calling* it, not by satisfying an interface
designed before the third one existed — the same reasoning §19.2 uses for not
building a transport before there is a second peer. Three front-ends that each
call `Round` are simpler than three that each satisfy an abstraction.

So adding one is a directory, a feature, a `[[bin]]`, and this checklist:

1. **A document**, in a fresh section namespace (`GUI.md` took `§G`; a Macroquad
   one would take `§M`). `FTM.md`'s numbers do not move for it, ever.
2. **F1–F4**, the four capabilities, in one `host` module. Usually under 200
   lines; the web build's is the proof of that.
3. **F5**, a key adapter, `Option<KeyEvent>` per native event, plus a
   round-trip test over every §10.1 key name.
4. **F6 and F7**: a minimum, a loop, and a draw path from `GameView` alone.
5. **Its own acceptance list**, mirroring §17.3's A1–A10 and checked one by one,
   and its own headless render test at several viewport sizes.
6. **The two `cargo check` lines above stay green**, and every Makefile target
   grows the new feature — a bare `cargo test` that stops compiling a front-end
   is the easiest thing here to forget and the failure is silent.
