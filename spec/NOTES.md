# Notes — what each stage settled, and the tooling that checked it

Lifted out of `CLAUDE.md`, which had grown to 135 KB and is read on every
prompt. **This is history and hard-won detail, not instructions.** The rules
that still bind are `CLAUDE.md`'s invariants; the specification is `FTM.md` and
the four documents beside it. Read a section here when you want the
measurement, the failed alternative, or the reasoning behind one of those
rules — and see `PILOT-PLAN.md`, which has a "What it settled" of its own per
stage. For the G stages these notes are the only home that reasoning has.

Three things worth pulling out of the whole of it, because they recur:

- **A watched game beats a green test suite.** It has happened three times —
  G13's B8 (a backgrounded tab that never paused), the quad the planner never
  built, and the T-spins it never scored. Every test was passing each time,
  because no test was in a position to ask the question.
- **Confirm a weight on held-out seeds before believing it.** The surface is
  noisy at eight seeds, and a measured-and-rejected change looked like a fix
  right up to the held-out run.
- **A seam predicted a stage ahead predicts the wrong thing.** §P2.3 booked
  accessors for P3 that P3 did not want and P7 needed a different one.

---

## The §17.3 sign-off

Each of these was checked on its own, most of them on a pty through
`tools/drive.py`. Re-run any of them the same way.

| | Criterion | How it was checked |
|---|---|---|
| A1 | Build clean | `cargo build --release` and `cargo clippy -- -D warnings`, both silent. `#![allow(dead_code)]` is gone. |
| A2 | §17.1 and §17.2 pass | `cargo test --all-features`: 314 unit, 5 + 7 integration. |
| A3 | Attract on launch, PLAY starts a game | `tools/drive.py --size 24x60 enter`. |
| A4 | §10.1 controls, working DAS | A 50 ms kitty tap moves exactly one cell either way; a 0.6 s hold slides to the wall and stops. T13 pins the arithmetic. |
| A5 | `preview_count` 1-6, both sources | Next-box height measured for all six from `--preview` and from the file: 5, 8, 11, 14, 17, 20 rows, matching §12.4's `2 + 1 + 2n + (n-1)`. |
| A6 | Full game recorded, on the attract screen | `HOME=<throwaway> tools/drive.py enter <40 x space> enter`; the JSON is written, and a fresh process shows it on the panel and the sub-screen. |
| A7 | Terminal restored | `stty -a` byte-identical either side of a run on the same pty, and the teardown emits §8.3's sequence in order. |
| A8 | `mono` and `NO_COLOR` | Both play, with §9.2's letters and `..` ghosts. Counting SGR sequences in the raw capture: truecolor 16, `256` 17, `16` 8, `mono` 0, `NO_COLOR=1` 0. |
| A9 | Hold and 180 off/on, three ways | Off from the file and from `--no-hold`/`--no-rot180`: the key is inert, the hold box is gone, and both bindings vanish from the controls overlay and the attract panel. On from the Options panel, which writes the file. |
| A10 | UI builds against the view alone | The compiler's job now: every module in `core` is `pub(crate)`. See the invariant below. |

Two things A8 turned up that are worth knowing. `--color 16` reaches the
terminal as `38;5;N`, not as `30`-`37`: that is crossterm's encoding of a named
colour and not ours, and §12.3 does not specify a wire form. And a `--legacy`
run pays about two seconds before its first frame, waiting for a capability
query the terminal will never answer — crossterm's timeout, not the game's, and
the same two seconds `--print-config` now pays on such a terminal.

§16's failure paths were exercised deliberately, each end to end on a pty:
an unwritable config (the Options panel over a read-only file), unwritable high
scores (a top out over a read-only data directory), a config that is not TOML
(I3: exactly one warning), and a panic mid-frame — patched in temporarily,
which restored the terminal *before* printing, left `stty -a` unchanged, gave a
readable backtrace and exited 101.

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
  movement keys and nothing else, so `Confirm` in `tui/run.rs` tracks the
  restart hold, off the action path entirely (`Bindings::action_of` is what
  lets the shell pick it out before `InputState` sees it). In legacy mode it
  survives silence for `RESTART_QUIET` — 700 ms, chosen to outlast the OS's
  *first* auto-repeat, not the 90 ms `HOLD_TIMEOUT` that separates later ones.
- **The high-score path is not overridable**, so a `drive.py` run writes to the
  real data directory unless it is given `--seed` (never recorded) or a
  throwaway `HOME`. `HOME=/tmp/... tools/drive.py ...` is how A6 was checked.

## What Stage 12 settled

- **A10 is the compiler's now, not an audit's.** Performing a one-off audit
  proves nothing about the commit after it, so every module in `core` became
  `pub(crate)`. Doing it found one real leak — `shell/input.rs` reaching for
  `core::matrix::WIDTH` where it wanted `VIEW_WIDTH`, the same ten in the
  vocabulary the shell is entitled to — and one imprecision in A10's own
  wording, which §17.3 is amended for: `PieceKind`, `Colour` and `Rotation` are
  view vocabulary, because a client handed a `GameView` cannot draw it without
  §9.3's cell patterns.
- **`#![allow(dead_code)]` is gone.** It was there because the core was built
  stages ahead of its callers. Removing it left exactly two warnings, both
  honest: `LockDown::is_landed` and `resets_used` are read only by T7, and are
  `#[cfg(test)]` now.
- **`tools/drive.py` can resize.** `resize:ROWSxCOLS` is a pseudo-key that
  resizes the pty and signals the child; each frame is replayed at the size in
  force when it was taken. §8.4 and §12.1 cannot be reached any other way
  without a human dragging a window.
- **§12.1's message is centred in the room there is**, not in the room it
  wants. Padding it to its full width pushes the first line off the left of a
  narrow screen, and 1 x 1 — which I4 requires and a dragged window passes
  through — comes back blank instead of showing a `T`.

## What G2 settled

- **The shell boundary is the compiler's, exactly as A10 made the core's.**
  `cargo check --no-default-features` builds `core/` + `shell/` with neither
  front-end feature on, and it is a `make check` step. **Everything else in the
  Makefile grew `--all-features`**, and this is the single easiest thing to
  forget: a bare `cargo test` builds only the `tui` half, and the failure mode
  is silent — the other front-end simply stops being compiled.
- **`Session` and `App` stayed in `tui/run.rs`, and that was deliberate.** They
  are shell objects, but they could not move while they owned crossterm's event
  queue and ratatui's `Size`. G2 only moved files; G4 inverted the loop and
  then they moved, to `shell/session.rs` and `shell/round.rs`.
- **`Attract` lost its `Background`.** §13.4's drift is positioned in matrix
  cells of a *character grid*, so it stays in `tui/attract.rs`; a window will
  want its own. `Attract::step(now)` reports whether the *state* changed and
  the front-end folds its drift's answer in beside it. Both halves are stepped
  unconditionally — short-circuiting `step` would stop the panel's six-second
  cycle rather than the redraw.
- **The menu models are the specification's words.** `label` and `value` moved
  to `shell/menus.rs` with their types and became `pub`, because a second
  front-end that invented its own labels would be a second §13.5.

## What G3 settled

- **The shell is portable, and the compiler says so.** `make portable` is
  `cargo check --no-default-features --target wasm32-unknown-unknown`, it is a
  `make check` step, and CI installs the target for it. It is the stage's real
  deliverable: it is what makes G6 a short stage rather than a long one.
- **`rand` is the one that would have crept back.** `getrandom` refuses to
  *compile* for `wasm32-unknown-unknown` without a build-time `--cfg`, so the
  shared `rand` is `default-features = false` and `tui` turns the OS source back
  on with `rand/thread_rng`. `core/bag.rs` is untouched, and the warning above
  about not simplifying §9.6 back into `rand`'s own API stands unchanged.
- **The shared dependency set is now exactly §3's five**: `rand`, `serde`,
  `toml`, `serde_json`, `thiserror`. `clap`, `directories`, `chrono` and
  `anyhow` all moved behind `tui`; `thiserror`, which had become unused, is
  what `StorageError` is written with, so §3's row for it is true again.
- **The failure paths were re-run end to end, and one of them found a bug in
  this stage.** Writing the config atomically "for symmetry" would have silently
  started replacing a read-only config file, and naming the temp file in §16's
  warning would have told the player about a file they have never seen. Both are
  fixed and both now have tests; §17.3's A7 and I3 are unchanged.
- **`Session` grew a lifetime**, because it borrows the front-end's store for
  the length of the run and `main` needs it back afterwards for §6.2's
  first-clean-exit write. That is `Session<'a>` until G4 moves it to
  `shell/session.rs`, where it will still have one.
- **Two identical warnings are still possible**, and were before this stage: a
  config that fails to save from the Options panel *and* on the first-clean-exit
  write says so twice, because `Session::warn` dedupes and `main`'s own push
  does not. It is only reachable when the file did not already exist, which is
  why §17.3 never saw it. Left alone deliberately — it is a §16 wording question
  rather than a G3 one.

## What G4 settled

- **`Round` is §15.2's loop body as an object, and `tests/pump.rs` is the proof
  it can be driven by something that is not a terminal.** Cadence invariance is
  the test that matters: the same key log over the same span of stamps, at
  60 Hz, at 144 Hz and at a jittery cadence with several zero-length frames,
  gives a byte-identical `GameView`. It also plays a whole game to a top out
  and through name entry, headless, in half a second — the part of §17.3's A6
  that used to need a pty.
- **Key stamps in that test sit at multiples of 50 ms, and that is load-bearing
  rather than tidy.** A key is consumed by the first tick that runs at or after
  it reaches the shell, so "the same inputs at the same moments" has to mean
  the same *tick*: a front-end that polled at 10 fps would genuinely deliver
  its keys later and would genuinely play a different game. Every cadence in
  the test is woken at each key's stamp (§15.2 step 6 wakes a loop early on a
  key), and no cadence runs more than one tick in a frame, so every cadence
  hands each key to the same tick. Loosen either and the test starts failing
  for a reason that is not a bug.
- **The countdown is settled at the top of `key` as well as `advance`.** §9.17's
  3-2-1 is the one non-running phase that ends by itself, and it ended *before*
  the event queue was drained when these steps were a loop body. `Round::settle`
  is what keeps that true now that they are not: a key arriving in the frame the
  countdown expires is the player's first input of the resumed game, not one
  swallowed by the overlay.
- **`Glyphs` left `Session`.** Leaking three `&'static str` so a ratatui `Theme`
  can be `Copy` is a terminal's concession; `tui::run::run` interns them once and
  carries them down. A browser tab that is reloaded repeatedly is exactly the
  wrong place to have inherited it.
- **`Cosmetics` lives inside `Round`**, not beside it, so no front-end has to
  remember to feed it — and §15.2 step 5's rule that nothing below it can reach
  the core is kept by the shape rather than by a comment.
- **`Attract::key` takes the `Session` and saves the config itself.** §13.5's
  "leaving the panel writes the file" is the specification's, and a second
  front-end that forgot it would be a second §13.5. What the front-end notices
  instead is `Session::generation`, which the panel bumps.
- **The terminal's behaviour is unchanged, and that was checked rather than
  assumed.** A3, A4, A6 and A7 were re-run on a pty: the attract screen on
  launch, a 50 ms kitty tap moving exactly one cell either way against a 0.6 s
  hold sliding to the wall, a full game recorded to a throwaway `HOME` and
  shown by a fresh process, `stty -a` byte-identical either side, and §8.3's
  teardown bytes in order. §8.4's forced pause and §12.1's message were checked
  with `resize:`.

## What G5 settled

- **`eframe` 0.36, and the MSRV is 1.95.** The plan's open decision, taken as
  recommended: pinning to 0.33 to keep 1.88 means tracking a stale `egui` for
  the life of the project, and §3 already says the floor moves with a
  dependency. **The number lives in three places** — `rust-version` in
  `Cargo.toml`, §3, and the `msrv` job in `.github/workflows/ci.yml` — and they
  move together or that job stops checking what it claims to. `eframe` is taken
  with `default-features = false` and `glow`, not the default `wgpu`: the
  front-end draws rectangles, and OpenGL reaches a browser as WebGL2 without a
  WebGPU fallback path.
- **`tui/host.rs` became `src/native.rs`**, and that is a deliberate amendment
  to `EGUI-PLAN.md`'s target layout, which had a copy under `gui/`. See the
  invariant above.
- **`eframe` 0.36 splits its callback in two, and the split is §15.2's.**
  `App::logic` is steps 1-4 and 6-7; `App::ui` is step 5. `logic` is called
  while the window is *hidden* and `ui` is not, so a hidden window keeps playing
  and simply is not drawn — G6's backgrounded tab, arriving three stages early,
  and §15.2 step 4's catch-up cap is what makes that safe. A `FrameState` is
  carried between the two halves so they agree about the moment.
- **The GUI is unconditionally §8.2's enhanced case**, so `HOLD_TIMEOUT` and
  `RESTART_QUIET` are unreachable from it and `--legacy` has no meaning there.
- **Focus loss synthesises releases**, because the release of a key let go
  outside the window never arrives and a held direction would keep charging DAS.
  `gui::keys::Keyboard` remembers what it reported as held; F5 makes the stream
  synthesisable precisely so a front-end may do this. At G5 it did not pause
  the game; **G7 reversed that** (`GUI.md` §G4.7, and the invariant above), and
  the releases still come first.
- **A tap shorter than a frame is lost, in both front-ends.** Press and release
  of a movement key inside one pump cancel before any tick consumes the pending
  cell (`KeyTimer::release` resets `initial`). This is pre-existing and shared —
  the terminal loop drains every waiting event before `advance` too — and it is
  why §17.3's A4 measures a *50 ms* tap. A synthetic instantaneous key-down/up
  is not a human tap, and a test that sends one will conclude movement is
  broken when it is not.
- **`egui` reports a key, not a character**, so the adapter decides a *letter's*
  case from the shift modifier — and the **punctuation needs an explicit
  table**, because `egui::Key::name` spells those out (`"Minus"`, not `"-"`)
  where it gives the letters and digits a single character. §10.1 lets a player
  bind any single character and §6.2 gives the two binaries one file, so the
  invariant is: every `Char(c)` the adapter can produce must be the key
  `parse_key` resolves `c` to. That is a test, and it is what caught this. The
  one gap left is the shifted digits — `egui` has a variant for `?` and `|` but
  none for `!` — recorded in `GUI.md` §G2.1 as accepted. `Event::Text` is
  deliberately unused: §12.6's name-entry rules are `shell::menus`'s.
- **CI grew an `apt-get`.** `eframe` needs X11/Wayland/GL *headers* to compile,
  in both jobs. Nothing in `cargo test` opens a window.

## What G6 settled

- **The web build is `gui` on wasm32, and the difference is said by target,
  not by feature.** A feature cannot tell a desktop from a tab, so `clap`,
  `directories`, `chrono`, `anyhow` and rand's `thread_rng` are declared under
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`, and
  `wasm-bindgen`, `wasm-bindgen-futures`, `web-sys` and `js-sys` under the wasm
  table; `gui` names both sets and each target builds its own half. One
  consequence: a native `make shell` now compiles `getrandom`, so it no longer
  proves the shell has no OS entropy source — `make portable` does, as it was
  always the one that could.
- **A 32-bit target found a determinism bug the whole test suite could not.**
  `SmallRng` is a different generator on wasm32, so seed 42 dealt a different
  game in a tab. The fix is its own commit, and the one core change outside
  G10: see the §9.6 invariant above. **A difference only a 32-bit target has is
  invisible to every test in the tree**, because they all run on a 64-bit host;
  a `const` assertion evaluated by `make portable` is where such a guard has to
  live. It was checked in a real tab against `tools/drive.py`'s next queue, and
  under Node against a throwaway wasm32 build of the core.
- **`gui::host` is whichever of `host_native` and `host_web` the target has**,
  so `app.rs` names neither. The one platform difference `app.rs` does see is a
  fact rather than a choice, and it asks `eframe::Frame::is_web()` for it: a
  tab cannot close itself, so leaving a game there starts a fresh one.
- **A tab's run has no end.** No `finish`, no §6.2 default write, nothing handed
  back: the store and the session are leaked once per page load. §16's warnings
  therefore go to the console *as they arise* — `Session::warnings()` is the
  read-only list `gui/app.rs` reports from, through `host::report` (a no-op
  natively, where `main` still prints them at exit).
- **Two crates the plan named are not taken.** `console_error_panic_hook`,
  because `eframe::WebRunner::new` installs a hook that logs message and stack —
  the runner is made *first* in the wasm `main` for that reason. And `web-time`,
  because F1 is `performance.now()` through `web-sys` directly.
- **The canvas takes the keyboard on load, and says so when it has not.**
  `eframe` gives it a `tabindex` but never focuses it, and it only calls
  `preventDefault` on `Space`, `Tab` and the arrows while it has focus. "Click
  to play" is drawn in both builds when `egui` reports no focus; it pauses
  nothing.
- **A hidden tab creeps; it does not stop.** The plan expected
  `requestAnimationFrame` to stop, and it does, but `eframe` then drives
  `logic` from a throttled timer, so each call plays `MAX_CATCH_UP_TICKS` and
  discards the rest. Measured: 36 s hidden advanced a level-1 game ~9 s, with
  no burst on return. `GUI.md` §G8.7 records it as input for G7.
- **Checking the web build needs a visible browser window.** A tab behind other
  windows reports `document.hidden` and is never painted, whatever a screenshot
  shows. Two more traps from the same session: a scripted key press is a
  key-down and key-up inside one frame, so a movement tap is lost exactly as
  G5 recorded — send held keys as `KeyboardEvent`s with a real gap (80 ms
  moves one cell); and **`trunk serve` kept the same hashed file name across
  rebuilds** and serves no `Cache-Control`, so after it rebuilds a plain reload
  can run the old wasm. Hard-reload.
- **CI builds the artefact in a job of its own**, with trunk 0.21.14 fetched as
  a release binary. `make check` gained `web-check` — clippy for the web
  front-end on wasm32, because `--all-features` on the host never compiles
  `host_web.rs` — and does not run trunk, so a developer needs only the target.

## What G8 settled

- **§12.6's boxes are `gui/overlays.rs`, and what they *do* is still
  `shell/menus.rs`.** The window walks the same menus, the same settings and
  the same name-entry rules as the terminal; only the boxes are new.
  `overlays::rect_of` holds every box's size, so "does it fit in the block?" is
  a question a test can ask without drawing.
- **Name entry is not an `egui::TextEdit`**, deliberately: §12.6's twelve
  printable ASCII and the `ANON` default are §14's rules, not a widget's, and
  keeping the field neutral is also what keeps the soft-keyboard question shut
  (§1.2).
- **No non-ASCII in a box.** `▸` and `←→` are terminal glyphs; a window's fonts
  may not carry them. The menu cursor is a drawn triangle and the hints are
  words.
- **The countdown is a numeral over the board, not a box**, because §9.17 is
  for reading the board — and it is the one overlay that does not dim it.
- **Two more shared answers left `tui/`**: `menus::controls` (§10.1's words and
  §13.3's gating rule) and `figures::pps`.
- **The window-to-terminal handoff is proven by a test over the real file
  store**, not by a person at a window: the session that built it believed it
  could not drive the native GUI. `native.rs` has the test. The acceptance
  check itself wanted a human once, and **it has had one** — see the end of
  "What G10 settled". (It need not have: the window *can* be scripted, which
  G12 worked out — see **Commands**. The test is still the right one, because
  what it checks is two binaries over one file store rather than a screen.)

## What G7 settled

- **The playing screen is 26 x 24 cells** (`GUI.md` §G3.2): a one-cell margin,
  a six-cell column, a gutter, the ten-cell well, a gutter, a six-cell column, a
  margin; the mouth, twenty rows, two status rows, a margin. A next panel of one
  slot is the hold panel's shape, and six fit beside the well exactly.
- **Pixels changed three of §12.4's decisions, in the spec.** The score is
  grouped live, combo and back-to-back are figures in the stats panel, and a
  preview is centred by the cells it occupies. `TUI.md` is unchanged; `GUI.md`
  §G4.4 says why each differs.
- **Shared answers moved out of `tui/`**: `shell::figures` (`thousands`,
  `clock`), `Debug::figures` and `Fps` (`shell/round.rs`), `Overlay::blanks`
  (`shell/menus.rs`). The terminal's output is byte-for-byte what it was — its
  mock-up tests say so.
- **A headless `egui` test must reuse one `Context`**, and must call
  `FullOutput::drop_without_applying_deltas` on a pass nobody renders. The first
  costs a minute if forgotten; the second is a debug-assertion panic.
- **Checking the web build without a visible Chrome**: a headless Chrome driven
  over the DevTools protocol paints, because its page is never occluded — but
  an emulated device scale factor leaves the canvas a 1× buffer under a 2×
  `egui`, so the page looks half-size. Use a scale factor of 1. The native
  window has not been looked at by the session that built G7 (no screen
  capture); it runs the same drawing code.

---

## What G10 settled

- **The core grew one field, and only one.** `GameView::fall_progress`, derived
  in `Game::view` from state §9.9 already kept. The I1 snapshot and the §19.4
  canary did not move, which is the assertion rather than a convenience: a
  snapshot that shifted would have meant the field was computed from the wrong
  thing, or that a rule had started reading it.
- **The plan's reasoning about landed pieces was wrong, and the spec is amended
  for it.** See the invariant above; it is the one thing in this stage that a
  test suite would not have caught, because nothing else in the tree looks at
  the number. §9.9 now says the reset on a blocked step is not enough on its
  own, and §12.7 states the requirement.
- **`compose` no longer holds the falling piece.** A grid of cells cannot say
  "half a row down", so the piece is drawn after the grid, and §12.5's flash and
  wipe became `washed`, a per-cell function both paths call. At rest the
  composition is byte-identical to G9's — the shape sweep and the animation
  tests say so.
- **No clipping was needed.** The offset is non-zero only when the piece can
  move down, so the cells below it are empty by construction and a sliding piece
  can never overlap the stack, the floor or the walls.
- **The pop-in the plan told us to accept was not acceptable after all.** §9.4
  spawns every piece but `I` with a mino above the field, `GameView` clipped it,
  and a `J` was drawn as three minos and then abruptly four. Invisible while the
  piece stepped; obvious against smooth neighbours. The fix is the signed row in
  the invariant above, and it is small because the §19 objection did not apply
  to the half that mattered — those minos are the player's own piece, not the
  hidden stack. `GUI.md` §G6.6.
- **`--virtual-time-budget` is not a way to screenshot this.** It advances
  `performance.now()` without running the frames, so every capture came back
  with the piece at spawn and the clock at `00:00`. What works is the G7 recipe
  — headless Chrome over the DevTools protocol, scale factor 1 — screenshotted
  on a real timer; `--enable-unsafe-swiftshader` is needed for WebGL, and
  `--disable-gpu` gets §G8.5's "could not start" message instead. Measured
  there: 13 pixels per 500 ms at a 26-pixel cell, then dead still for the lock
  delay.
- **The native window has now been looked at, by a person**, after G10 — the
  first time any session's work on it has been seen running. The slide and the
  entering piece are right there too, which is what one would expect from shared
  drawing code but had never actually been confirmed.
- **§G8's window-to-terminal high-score handoff has been checked too**, by the
  same person and at the same time: a top out in `ftm-gui` reaches `ftm`'s
  attract screen. That was the last thing in this front-end held only by a test
  standing in for a human, and it is not outstanding any more. §6.2's one config
  file and §14's one table between the two binaries (`src/native.rs`) have now
  been seen working end to end, not just asserted.

---

## What G11 settled

- **§13's words left `tui/`, and that was the stage's real work.** The state
  machine was already shared; the letterforms, their colours, the reminders and
  §13.4's numbers were not. See the invariant above. The terminal's §13.2 art
  test is untouched and now proves the derivation rather than a literal.
- **A tab has no QUIT.** `Session::menu` beside `Session::settings`, and the
  web build sets `MenuChoice::NO_QUIT`. The rejected alternative was reloading
  the page: it is not what the word means, and it would throw away the run's
  §16 warnings and its in-memory table. The quit *key* still works and comes
  back to the attract screen.
- **Both screens are laid out in §G3's grid**, so the window has one too-small
  threshold rather than two and ending a game resizes nothing. §G7.1.
- **§13.4's "painted over it, opaquely" is a character grid's rule**, and §G7.3
  says what it means in a window: the drift is drawn first and everything else
  is opaque on top of it. Nothing is blanked to make room.
- **`rand::make_rng` is not available to the window's drift**, because it is
  the OS entropy source and wasm32 has none. F3's `host::seed` is the
  capability that already exists for exactly this, and the generator is
  `Xoshiro256PlusPlus` — reaching for `SmallRng` here would have been reaching
  for the one that is a different generator on a 32-bit target.
- **A canvas that is not the visible tab is never painted**, so the attract
  screen came up blank on first load and appeared the moment a key was pressed.
  That is G6's recorded trap, not a bug: `App::logic` runs and `App::ui` does
  not. Worth remembering before debugging a renderer that works. It bit twice:
  the second time the tab would not paint at all, and the fix that needed
  looking at had to be checked by a person at the native window instead.
- **The native window's attract screen has been seen, by a person**, and it
  found two things a test had not: the menu sat 0.4 of a cell right of centre,
  and a panel face shorter than four rows sat against the top of the panel
  where the terminal centres it. Both are fixed, and both now have a test that
  measures where the text actually landed — which is the shape this front-end's
  layout regressions want, since nothing else in the tree can see them.

---

## What G12 settled

- **The data-loss bug the stage exists for was real and is now a test.** A GUI
  run that saved the config would have erased `[display]`, because `document`
  rewrites the whole file. Both halves are in `ConfigFile` now — see the
  invariant above — and the round trip is compared **byte for byte within each
  table**, in both directions: a save that reflowed the other front-end's half
  would be the same loss one step removed.
- **`[gui]` is nine keys, four of them start-up answers.** `GUI.md` §G8.10's
  table says which take effect when, because it is not guessable: `vsync` is
  chosen when the GL surface is made and so can never be an Options row, while
  `scale_percent` and `fullscreen` are rows and apply the moment the panel is
  left. `frame_cap` is a floor under §15.2 step 6's deadline — it caps
  *drawing*, and the game plays several ticks per repaint and stays the same
  game, which is `tests/pump.rs`'s cadence invariance reached from a setting.
- **`vsync` lives on `glow_options`, not on `NativeOptions`**, in `eframe`
  0.36 — the renderer's rather than the window's. It is set by assignment
  rather than in the struct literal, because naming its type means naming
  `egui_glow`, which §3's dependency table does not list.
- **`clap`'s `ValueEnum` impls had to leave `tui/cli.rs`**, and `src/argv.rs`
  is where they went. Both native binaries take `--lock-down`; an impl behind
  `feature = "tui"` simply is not there for a `--features gui` build, and the
  failure is a compile error in the *other* front-end. See the amended
  invariant.
- **`--print-config` is shared after all.** `gui/cli.rs` carried a comment
  saying a window does not ask the question; the plan said otherwise and the
  plan was right — what it prints is the whole document, and the player
  comparing what two binaries resolved from one file is exactly who wants it.
- **Two §16 warnings named a directory a browser tab has not got**, which
  `GUI.md` §G8.3 had already booked as this stage's to fix. They say *nowhere
  to keep the settings / the scores on this platform* now: §3.1 means the shell
  does not know where the bytes were going, so it should never have said.
- **A URL is edited by hand and the grammar admits it.** A flag parameter takes
  five spellings for "on" and four for "off" (§G8.4); a command line has only
  "written or not written". `?fullscreen` in a tab is accepted and inert rather
  than warned about, because a link shared from a desktop should not scold
  whoever opens it.
- **The native window turned out to be scriptable, and that is the stage's
  other deliverable.** `osascript` sends it keys; see **Commands**. Every claim
  above was checked against the running binaries rather than against tests
  alone — the Options-panel save in both front-ends, the geometry write-back,
  `remember_window = false`, and the two runs at 125 % that caught the scale
  bug. `remember` still has no unit test, because reading `egui`'s viewport
  info needs a window manager; what it has instead is that it has been *run*,
  which for this function is the better of the two.

---

## What G13 settled

- **B8 found a real bug, and it is the reason this stage is not a formality.**
  A backgrounded browser tab did not pause: it went on playing, throttled — 31
  seconds hidden advanced a level-1 game by 13, which is G6's creep measured
  again a year of stages later. §G4.7's *rule* was right; the *mechanism*
  `GUI.md` §G8.7 named for it was wrong. Switching to another tab fires no
  `blur` at the canvas, so it keeps the document's focus and `egui` goes on
  reporting a focused application that cannot hear a key. See the invariant
  below; both sections are amended and `gui::host::visible` is the fix.
- **The lesson is worth more than the fix.** §G4.7 had a test and the test
  passed, because it asked the *shell* whether it pauses when told the keyboard
  is gone. What was broken was the front-end's answer to "is it?" — a question
  no test in the tree was in a position to ask. That is the shape of what is
  left after twelve stages of tests, and it is what B3-B11 are for.
- **`egui_kittest` was not taken**, against the plan. `egui::Context::run_ui`
  with a `RawInput` *is* the headless harness, and it is what every test in
  `src/gui/` already used; the crate's own contribution is an AccessKit tree
  this front-end has no widget hierarchy for (§G1.1 declines `accesskit` for the
  same reason) and image snapshots the plan already excluded from CI. A
  dependency whose only remaining use is excluded is not a dependency. §3's
  table and §G1.1 no longer name it.
- **The density is the axis a character grid has not got.** `tests/gui_render.rs`
  sweeps seven sizes in *physical pixels* at four densities, and the two are not
  interchangeable: §G3's cell is a whole number of pixels, so fourteen points is
  17.5 at 1.25 and the cell must be eighteen. The minimum window is therefore
  468 x 432 pixels there against 364 x 336 at 1x, and **the boundary the test
  asserts is one pixel wide, not one point**. It also asserts that §G3.3's
  message never asks for less than it takes, because a player who resizes to
  what they are told has to get the screen.
- **Two `egui` traps, for anyone writing a headless test here.** `RawInput`'s
  `screen_rect` is divided by the zoom factor, so `set_pixels_per_point` and a
  size are not independent — set the density on the viewport's
  `native_pixels_per_point` instead and `screen_rect` stays the points you
  meant. And the font atlas is 2048 pixels: a countdown numeral is six cells, so
  a viewport of a few thousand *points* at 3x asks for a glyph that will not fit
  and panics inside `epaint`. Sizes in pixels divided by the density keep every
  case a viewport that could exist.
- **CI needed nothing.** Every step this stage was supposed to add was already
  there — `shell` from G2, `portable` from G3, `web-check` and the web job from
  G6, the `apt-get` and the MSRV bump from G5. That is what `make check` being
  the only list CI runs buys: the stage that adds a check adds it there, and the
  acceptance stage finds the bill paid.
- **B4 is a test now, not a stopwatch.** §17.3's A4 measured a 50 ms tap and a
  0.6 s hold on a pty because the terminal's loop was the only way to reach
  them. Since G4 they are reachable from `tests/pump.rs`, and that is what makes
  "identical to the terminal front-end's" a fact rather than a comparison: it is
  the same `shell::input`. Do not try to measure this through a browser — a
  round trip to a tab is seconds, and the piece has moved on.

---

## What P2 settled

- **The randomiser is replaced in `core/bag.rs`, not worked around in `Game`.**
  A `Bag` is now a `Source`: §9.6's generator with the bag it shuffles, or a
  scripted list with neither. `Bag::next_piece` returns an `Option` and the
  `None` is reachable only from a fork, where `Game::spawn` declines to spawn
  and waits in the entry delay. The alternative — a `Game` that keeps its real
  bag and is asked politely not to deal from it — is what §P2.3 means by
  "replaced, not hidden", and it is the difference between fairness being a
  type and fairness being a habit.
- **The I1 snapshot and the §19.4 canary did not move, and that is the
  assertion.** `Bag` was restructured under the whole game; a snapshot that had
  shifted would have meant the live path had changed shape, not that the
  fixture was stale. Read it that way if it ever does move here.
- **A stage lands only the part of a seam that is read.** `#![allow(dead_code)]`
  is gone and the tree means it, so a `pub(crate)` accessor whose only caller is
  a `#[cfg(test)] mod` is a warning in the ordinary build, and `make check` is
  `-D warnings`. §P2.3's method list is amended to say it is an upper bound; P3
  adds the pose and the hold state when its generator reads them. The general
  lesson for the stages after this one: **a seam built a stage before its
  consumer needs a consumer**, and the honest one is usually the public face
  the layer was going to need anyway.
- **`pilot::Fork` is that face, and it is structural.** §P2.3 requires
  `SearchGame` to stay out of the core's façade, and Rust will not let a
  crate-private type appear in a public signature — so `tests/`, §P8's runner
  and anything else outside the crate search through `pilot`'s own wrapper. It
  is also where the scripted queue arrives, which is the half of fairness no
  type can hold: a `Fork` is exactly as fair as the list it was built from.
- **Two bag facts were bugs before they were specification.** The first piece of
  a game is dealt during `Game::new`, which **discards its `PieceSpawned`** — a
  tracker that waited for the event is one piece out for the whole game. And the
  open bag closes on the **seventh** deal, not the eighth; waiting reads
  "nothing can come next" at the one moment all seven can. Both are in §P2.4
  now.
- **A preview can cross a bag boundary**, so a piece on the screen may also be a
  legitimate hypothesis — out of the bag *after* the one it was dealt from. At
  the default preview of 5 this is ordinary, not exotic, and an assertion that
  said otherwise was wrong about §9.6 rather than about the code.
- **The features are §P5's literally, including the two that read backwards.**
  Row transitions count both walls as filled, so an *empty* board scores 40 and
  a full one 0; column transitions count the floor, so an empty board scores 10.
  Both are Dellacherie's formulation and both are meant to be minimised — the
  direction is in the weight, not in the feature. `covered` and `blockades` are
  deliberately two features and not one: a cell above two holes is counted twice
  by the first and once by the second.
- **Test fixtures in `src/pilot/` are held to §P3.4's list too.** The core's
  `from_bottom_rows` is `core::matrix`'s and the planner may not name it, so
  `eval.rs` has its own four-line field fixture; `RulesConfig` is built by field
  rather than through `from_settings`, because the planner may name the one and
  not the other two. A test that reaches past the boundary is testing a module
  allowed to do something the module under test is not.

---

## What P3 settled

- **A placement is played, not predicted**, and that one decision is why the
  generator is ninety lines. Every candidate — hold or not, four orientations,
  thirteen columns — is replayed on a fork through the real rules and measured
  from the board it leaves behind. There is no arithmetic anywhere about where a
  piece lands, so a rotation that kicks at a high stack, a shift that runs into
  a wall and a piece that gravity locks early are all ordinary rather than
  special cases. `PILOT.md`'s "both generators produce input sequences, never
  board positions" is the reason this is the cheap way round and not the
  expensive one.
- **`SearchGame` gained nothing this stage, against the plan's expectation.**
  §P2.3 had the pose and the hold state arriving with P3's generator; a
  generator that replays reads the position from `view()` and wants neither.
  They are §P4.2's — P7 — which will also have to answer how a pose crosses the
  boundary at all, since `ActivePiece` is not in the core's façade and may not
  join it. §P2.3 and the plan are amended.
- **The divergence assertion earned its keep on its first run.** It caught a
  case the design had not: a hold at a `preview_count` of 1 uses the fork's
  queue up, so the tick that locks the piece cannot say what spawns after it —
  the live game deals a piece that was hidden information when the plan was
  made. The prediction now carries whether the fork still knew, and the
  comparison stops exactly there. This is the shape of every fairness question
  in this plan: the answer was to compare *less*, not to give the fork more.
- **It does not play badly, which `PILOT-PLAN.md` P4 says to expect.** One ply
  over §P5's starting weights plays 20,000 pieces on each of five seeds with no
  top out — ~8,000 lines and level 800 apiece. P6's lookahead therefore has a
  harder baseline to beat than the plan assumed, and P5's benchmark is what will
  say whether it beats it.
- **The cost is ~155 µs a piece in release**, about 6,500 *pieces* a second, for
  the 104 candidates one ply generates. (Recorded here as "placements a second",
  which P5 corrected with an instrument: 6,500 is 155 µs's reciprocal and so
  counts pieces, and the placement rate is 104 times it.) A frame is 16 ms, so
  even a full `MAX_CATCH_UP_TICKS` batch of searches is nowhere near it; the
  number worth measuring after P5 is the one after P6's second ply.
- **`cargo test` runs the assertion, and that is the point of it.** Every
  planned game in the suite checks the plan against the fork tick by tick,
  because `cargo test` is a debug build. A release round pays nothing: the
  predictions are not even recorded.

---

## What P4 settled

- **It is offered, and it was watched.** A **PILOT** row under **PLAY** on both
  native menus, and a person looked at a game on each: fourteen lines and level
  two inside six seconds on the terminal at seed 42, the same in the window,
  with the indicator on the status row of both. `PILOT-PLAN.md` P4 said to
  expect it to play badly; P3 had already found otherwise and this is that
  finding with a screen in front of it. The pieces *travel* rather than
  teleport, which is §P3.2's one-cell cap earning its keep as a presentation
  decision and not only an honesty one.
- **The two amendments this stage needed were both the spec catching the
  code.** §13.3's panel cycle pauses on any item but the two that start a game,
  so PILOT had to join PLAY there — the first implementation compared an index
  to zero — and §12.4 and §G4.5 both say the indicator is drawn in the `I`-piece
  cyan, which the first implementation was not. Neither was a test failure; both
  were found by reading the sections the stage was implementing against. That is
  what "the spec is ground truth" buys, and it is why the invariants above now
  carry the colour.
- **`Next::Play(Player)` was the cheap half and the sixth menu item the dear
  one.** Every front-end already matched `Next` exhaustively, so the payload was
  a compiler-guided edit that could not be got wrong. The menu item reached into
  `tui::attract`'s block height (21 → 22, `PILOT.md` §P7.2), both render
  fixtures, and three of the shell's own tests that had counted `Down` presses
  to a row — those now ask `MenuChoice::ALL` where the row is.
- **The assertion that would have caught the silent clipping was already
  there.** §P7.2's hazard is that a menu taller than the block loses the *footer*
  by clipping, with no error — and `the_screen_holds_the_wordmark_the_menu_and_
  the_panel` looks for `ENTER start`. Worth knowing for the next thing that
  grows: the attract screen's tests are what hold its height.
- **Driving the native window is still a focus problem.** Three of five
  scripted runs sent their keys to whatever was in front instead, and
  `osascript` exits `0` either way (`EGUI-PLAN.md` G13 recorded this; it is
  worse than it reads). What works is **one shell invocation** that launches the
  binary, waits, sends the keys and screenshots, with nothing in between — a
  second invocation brings the terminal back to the front and the keys go
  there. `screencapture -x -o` plus `sips -c H W --cropOffset Y X` is still how
  the result is looked at.

---

## What P5 settled

- **There is a baseline now, and a command that reprints it.** `make bench` is
  8 seeds × 2,000 pieces in release: 16,000 pieces, 6,381 lines, **no top out**,
  a mean of 4,083,485 and level 80 at the cap, in 2.33 s — **145 µs a piece**,
  ~715,000 nodes a second. The longer run P3 asserted headlessly is an
  instrument reading now too: `--seeds 5 --pieces 20000` is 100,000 pieces and
  39,989 lines with no top out, in 14.2 s. One ply over §P5's starting weights
  is a real baseline for P6 to beat, not a placeholder to replace.
- **The node budget is ~2,000 per search**, and that was the open decision the
  stage existed to close. A frame affords ~11,900 nodes at the measured rate,
  and §15.2 step 4 may play `MAX_CATCH_UP_TICKS` ticks before it draws, so the
  worst case is six searches in one frame. `Settings::default().nodes` is
  100,000 — fifty times over, harmless while nothing reads it, and P6's first
  correction. It is conservative twice over on purpose: it comes off the *mean*
  per-piece cost, and six spawns in six consecutive ticks cannot happen at all,
  because §9.12's two delays sit between them.
- **The report's two halves are the no-clock rule showing through.** See the
  invariant above. The thing to remember when reading a report: everything above
  the `timing` heading is diffable and everything below it is not, and a run
  that "changed" is a run whose *rows* changed.
- **A figure derived in a commit message is not a figure an instrument
  printed.** P3's "~6,500 placements a second" is 155 µs's reciprocal and
  therefore counts pieces; the placement rate is 104 times larger. Nothing was
  broken by it and no test could have caught it, and it is exactly why §P8.2
  asks for `nodes` and `placements` by name rather than for "throughput".
- **The smoke batch is small because `cargo test` is a debug build**, where
  §P3.1's divergence assertion replays every plan a second time. Three seeds ×
  60 pieces, in `tests/snapshots/pilot_bench.txt`. **It is the opposite of I1**:
  a weight, generator or tie-break change is *supposed* to move it, and moving
  it is a diff to read rather than a bug to chase — which is why it lives in its
  own file and is regenerated the same way (`UPDATE_SNAPSHOT=1`).
- **§4's tree had drifted since P2, and the spec is corrected in this commit.**
  It named `placements.rs`, `evaluate.rs` and a `mod.rs` holding `Pilot`; none
  of the three was ever written. The rule the repo runs on says the spec is
  wrong until amended, and a tree nobody re-reads is where that goes unnoticed.

---

---

## What P7 settled

- **P6's open question is answered on an axis nobody named.** Exact
  reachability was to be justified "by the placements themselves… do not expect
  a number to go up". Lines went *down* 0.6% and top-outs stayed at zero, both
  exactly as P6's ceiling predicted — and **score went up 12-19%**, on the
  development seeds and on held-out ones. What the walk buys is not spins, which
  §P5 has no feature for; it is **tucks**, and a tuck is a hole not made, which
  the board features charge heavily. The tables are in `PILOT-PLAN.md` P7.
- **A T-spin is now reachable and still worthless**, and that is the most
  useful thing to know before touching §P5 again. `Outcome` counts rows and
  clear kind; a T-spin single is priced as the cheap single it clears. Adding a
  spin feature is a §P5 amendment and a retune, it is **not** in this plan, and
  it is now the obvious next thing to try — P7 is what made it reachable at all.
- **`--exact --depth 2` and `--exact --depth 1` produce byte-identical
  reports**, and only the node count differs, by the 4,457 nodes of ply-2
  expansions begun and abandoned. That is §P6.4's "best fully evaluated move"
  demonstrated at 16,000 pieces rather than in a unit test, and it is the whole
  argument for the default. Read it that way if a future stage wonders whether
  the budget is really binding. (**P8 amends this.** It was true here and stopped
  being true when `t_slots` arrived, because the code did not implement §P6.4 —
  the free branches of an exhausted beam were being ranked. It is nearly true
  again after the fix and not exactly: a beam that bottoms out *entirely* in
  leaves is a legitimately complete second ply and may legitimately differ. What
  is a rule is §P6.4; this was an observation.)
- **There is no cheap corner where the walk wins.** On 8 × 2,000: §P4.1 at one
  ply is 39.2M at 142 µs, §P4.2 at one ply is 40.7M at 2,019 µs, and §P4.1 at
  *two* plies is 43.6M at 2,484 µs. At the same wall cost the second ply is
  worth 7% more than exact reachability is. The choice is the default or 35×.
- **The seam grew by one accessor and not the one it was booked for.** §P2.3
  expected P3 to want the pose and the hold state, then P7. The hold state was
  on `GameView` all along; what was actually missing was §9.13's rotation
  metadata. The general lesson is P2's again, sharper: *a seam predicted a stage
  ahead predicts the wrong thing*, and the accessor that lands is the one a real
  caller turned out to need.
- **`WALK_LIMIT` is measured, not guessed.** An empty well is the worst board
  there is — everywhere is reachable — and measures 3,685 states, 26,031 forks
  advanced and 68 distinct placements. A real game's stack does the pruning:
  `make bench` records ~6,400 forks a generation. The ceiling is 8,192, a little
  over twice the worst case, and reaching it is a debug assertion and a
  truncated answer rather than a wrong one.
- **The strongest test of the walk is the one it inherited.** `cargo test` is a
  debug build, so §P3.1's divergence assertion replays every plan tick by tick
  against the live game —
  `controller::tests::it_plays_a_game_with_the_exact_generator_too` is a round
  played with §P4.2, and what it asserts is that the *sequence* reaches where
  the walk said it reaches, which no board comparison can say.
- **P7's own table was measured under weights that no longer exist**, and it is
  annotated rather than deleted. `well_rows` landed straight after this stage
  and took the walk's margin from +18.6% to +7.7%, because a good deal of what
  exact reachability was buying was compensation for an evaluation that could
  not see a quad coming. The general shape of that is worth carrying: **a
  generator and an evaluator can be fixing the same deficiency from opposite
  ends**, and measuring one against a stale version of the other overstates it.

---

## What the second weight retune settled

- **A game was watched, and it found what no figure in §P8.2 reports.** Three
  observations from a player — `I` pieces landing flat, `S` and `Z` in the hold
  slot, holes that looked resolvable by a slide — came back to one cause when
  instrumented: **one quad in 1,000 pieces**, and a well four deep on 8 locks in
  1,000. There is no line in the report that says "it never builds a well", and
  there was not going to be one. Instrument what is *watched*, not only what is
  already counted.
- **Two of the three observations were downstream of the third.** Nothing in the
  fix addresses `I` orientation or the hold slot; they move because the planner
  finally has somewhere to put an `I`. The player's own guess that the flat `I`s
  were "a symptom of other weightings" was right, and it is the right instinct to
  have about this planner generally — the features are few and they interact.
- **The first attempt was the wrong shape and failed loudly**, which was
  useful. Exempting the well column from `row_transitions` is the same idea as
  `well_rows` without a cap, and it tops out seven games in eight. An exemption
  removes a penalty and cannot create an incentive; the cap is what stops the
  incentive running away. Both halves are now in §P5.
- **The plateau is broad and the numbers are not delicate.** 1,400 to 2,800 sit
  within 4% of one another and 2,000 was taken on the development set and tied on
  the held-out one. Do not read 2,000 as a tuned constant — read it as the middle
  of a wide flat region, which is what a hand-tuned weight ought to be.
- **Both weight-sensitive snapshots moved, and that is §P8.3 working.**
  `pilot_bench.txt` and `pilot_plan.txt` are *meant* to move on a weight change;
  I1's snapshot and §19.4's canary did not, and must not.

---

## What the T-spin pass settled

- **A second watched game, a second cause no report names.** The player's
  observation was that there was very little T-spin scoring. Measured: **none at
  all** on the default path over 2,000 pieces, and two accidental mini singles on
  `exact`'s. Three causes, stacked — §P4.1 cannot perform a spin, §P5 could not
  see one when it happened, and pricing it correctly still would not be enough
  at two plies. That is why the answer is two changes: a *fact* fixed (the
  `ClearKind` fold) and a *feature* added (`t_slots`). Both invariants are above.
- **The generator and the evaluator had to move together, again.** P7 recorded
  that "a generator and an evaluator can be fixing the same deficiency from
  opposite ends". This is the same lesson with the sign flipped: here neither
  half is worth anything without the other, and the T-slot weight is *negative*
  value under the generator that cannot cash it. A feature that rewards a shape
  wants asking not only "does it top out?" but "can the generator in force
  actually collect it?".
- **The default path did not move at all.** `pilot_bench.txt` and
  `pilot_plan.txt` are both unchanged: §P4.1 essentially never produces a spin,
  so re-pricing spins changed no plan it makes. The 8 × 2,000 benchmark moved
  +0.06%, which is the whole of the default path's interest in T-spins.
- **The live baseline is the T-spin commit's, not P6's or P7's.** The weights
  have moved three times since P6, so those stages' tables are history. `make
  bench` is **16,000 pieces, 6,361 lines, 63,281,640 score, 28,488,455 nodes, no
  top out**, and `PILOT-PLAN.md`'s P8 stage records it as the figure §P9's C11 is
  measured against. `--exact` has no committed baseline at that batch size, and
  it now differs by *weight set* as well as by generator.
- **What is still not priced.** A T-spin *triple* has no progress feature — its
  slot is a three-deep overhang, and rewarding one would be rewarding a much
  worse board for a prize far less likely to be collected. And `t_slots` says
  nothing about whether the two rows are near full, which is `well_rows`' other
  trick and the obvious next refinement if anyone returns to this.

---

## What P8 settled

- **C1-C11 and C13 are signed off one by one, and the table in `PILOT-PLAN.md`
  P8 says how each was checked.** Two of them needed something the tree did not
  have. C3's "and under `make portable`'s target" is not something `make
  portable` proves — it compiles for wasm32 and never runs there — so a
  throwaway crate over the `ftm` lib folded twelve configurations into one FNV
  digest and it came back **`e88ff742` on the host and `e88ff742` under Node**.
  That is G6's lesson honoured rather than repeated: a difference only a 32-bit
  target has is invisible to every test in the tree. And C9 and C10 are the
  driven-and-screenshotted checks the two front-ends already had recipes for.
- **C12 was watched and it failed the first time, which is the third time a
  watched game has beaten the whole test suite** — and then passed on both front
  ends after the fix. Every other criterion in §P9 is a test, a capture or a
  `git log`, and every one of them was green while this was wrong. The player's
  `preview_count` is 1; the
  benchmark's is 5 and §P8.1 forbids it to read the config file. At §P6.1's two
  plies **a preview of 1 is the only configuration that needs §P6.3's chance
  node** — previews 2 to 6 are byte-identical — and a chance node costs one
  expansion *per root*, so the ply costs ~7× what §P6.4 sized the budget for.
  The budget truncated the beam, §P6.4 handed back the one-ply answer, and the
  planner was **a one-ply planner running two-ply weights**: it built the well
  `well_rows` asks for and never planned the `I` that cashes it, digging a shaft
  fourteen rows deep and stacking the rest into the ceiling. §P6.2's width is
  divided by the root count now — held-out seeds go from 4 top-outs in 32 and
  47.7M to **none and 61.1M**, and preview 5 is byte-identical.
- **A knob that changes nothing is the loudest clue there is.** Sweeping §P6.3's
  80/20 blend across four values gave byte-identical reports, which is what said
  the second ply was never being evaluated. Before tuning a parameter, check it
  reaches the code at all.
- **Two good stories were measured and rejected, and that is the half worth
  remembering.** Capping the `wells` exemption at the four rows an `I` can clear
  reads exactly like the `well_rows` cap — and on eight seeds it looked like a
  fix, and on held-out seeds it bought nothing at preview 1 and cost 2.7% at
  preview 5. This file has warned since the first retune that the surface is
  noisy at eight seeds. The warning was read and the result was still nearly
  taken; **confirm on held-out seeds before believing a weight, not after.**
- **`make bench` measures one point in a space §6.3 lets the player move.**
  §P8.1's refusal to read the config file is right and stays, but the corollary
  is that `preview_count`, `start_level` and the rest are settings the baseline
  never exercises. A configuration the game offers and no baseline runs is a
  configuration nobody has measured.
- **C11 found a real bug, which is why the acceptance stage is not a
  formality — the third time in this repo, after G13's B8 and P4's two
  amendments.** `ftm-pilot --exact` topped out **six games in eight** where P7
  had measured none. §P6.4's "a partly evaluated branch is never chosen" was
  honoured at the branch and broken at the *beam*: `subtree` charges nothing
  for a fork that has topped out, so with the budget gone the branches that came
  back with a value were exactly the ones that lose, and the search preferred
  that prefix to the complete one-ply answer. A beam the budget cuts short is
  discarded entirely now, and §P6.4 says so in words as well as in code.
- **The bug could not reach the live game, and that was measured, not
  argued.** Over `make bench`'s 16,000 searches the shipped configuration's
  worst search cost **1,784 nodes of its 2,000** and none reached the budget.
  Both baselines are byte-identical across the fix. Before concluding that a
  search-budget change is safe, measure the *maximum*, not the mean — §P8.2
  reports neither.
- **The second cause was a weight set outside the configuration it was tuned
  in.** `Weights::exact`'s `t_slots` was tuned at two plies with the budget
  lifted; at one ply it digs a slot it can never cash, which is the
  uncapped-`well_rows` failure for the third time. **A weight set belongs to a
  search configuration and not only to a generator**, and `ftm-pilot --exact`
  now brings a budget it can spend unless `--nodes` names one.
- **The test that guarded the rule could not have failed, and the replacement
  asserts the rationale instead of an instance.** The old one asked about an
  empty board on the first piece, where every candidate costs the same to expand
  and no free branch exists. The new one sweeps the *budget* and requires every
  answer to be either the one-ply answer or the whole two-ply answer and never a
  third; it fails at `nodes 111` without the fix. Two constructions in between
  did not work and both are instructive — a starved *game* never gets tall,
  because a starved planner plays the one-ply game and the one-ply game plays
  well; and a board built by hand gets so precarious that the whole beam bottoms
  out in leaves, which is a legitimately complete ply and a legitimately
  different answer. In a real `--exact` game the first divergence is at **piece
  649**. When an instance is out of a debug build's reach, test the property.
- **There is an `--exact` baseline now**, the first at `make bench`'s batch size:
  8 × 2,000, **79,852,269 score, 6,364 lines, no top out, 1,590,431,335
  nodes** — +26% on the same lines for 11× the wall cost (29,869 µs a piece
  against 2,738). P7's 35× was measured against different weights and is not
  this number; the conclusion is unchanged, and only the default fits a frame.

---


---

## Driving and looking at the front-ends

The condensed recipes are in `CLAUDE.md` under **Commands**. This is the
reasoning and the traps behind them.

`HOME=/tmp/somewhere tools/drive.py ...` redirects both the §6.2 config path
and the §14 data path, which is how a full game can be played to a top out and
its score checked without touching the real files. A `--seed` run is never
recorded (§14) and so needs no such care.

`tests/pump.rs` is the shell driven headlessly — §15.2's steps called the way a
front-end calls them, with no screen and no clock. It is where cadence
invariance, the catch-up cap, §7's phases, `deadline`'s bounds, §8.4's two
forced pauses and §10.3's DAS and ARR by A4's own two numbers are checked, and
it is the test a fourth front-end inherits for free.

`tests/gui_render.rs` is the window's I4 — `tests/render_sizes.rs` asked in
pixels. Seven viewport sizes in *physical pixels* at four densities, every
screen the program can show, through a real `egui::Context` and no window
(`GUI.md` §G9.1). Between it and `tests/pump.rs`, the window front-end has what
`drive.py` gives the terminal; the native window can also be *driven*, which is
below, and that is what B3-B11 were checked with.

`tools/drive.py` is the only way to check the terminal layer without a human at
a terminal: §17.1 is "core, no terminal" by design, and what `cargo test` does
reach of `tui/` it reaches through a `TestBackend` — nothing in it opens a real
terminal, or touches `src/bin/ftm.rs`, `tui/term.rs` or `tui/run.rs`'s loops.
It drives the **release** binary on a pty,
sends a scripted burst of keys and replays the capture into a character grid,
printing one frame per keystroke. Two things to know: pass binary arguments through
`--arg`, glued on with `=` both times (`--arg=--seed=42`, `--arg=--config=...`)
or `argparse` claims them — without a seed every run is a different game and
only frames *within* one run may be compared; and it answers the §8.2
capability queries, so pass `--legacy` to exercise the fallback path. A
"key" of the form `resize:ROWSxCOLS` is not a key: it resizes the pty and
signals the child, which is how §8.4 and §12.1 are reached. Its
docstring has the rest. It substitutes for, but
does not replace, playing the game on a real terminal.

**The native window can be driven too, and G12 is when that was worked out.**
Earlier sessions recorded that they could not, and reached for a human; they
did not have to. On macOS, start the release binary in the background and send
it keys with `osascript`:

```bash
# The recipe G12 used, whole: drive the §13.5 Options panel and assert on the
# file it saves. `preview_count` moving 5 -> 6 is what says every key landed.
./target/release/ftm-gui --config /tmp/t.toml & pid=$!
sleep 5                                    # the window must take the keyboard
k() { osascript -e "tell application \"System Events\" to key code $1"; sleep 0.4; }
k 125; k 125; k 125    # Down x3 -> OPTIONS
k 36                   # Enter    -> open the panel
k 124                  # Right    -> step the selected row
k 53                   # Esc      -> §13.5 saves the config and returns
# The quit key on the attract screen closes the window, and a *clean* exit is
# what runs §6.2's first-exit write and §G8.10's geometry write-back. `kill` is
# the fallback for a script whose keys went astray, and skips both.
osascript -e 'tell application "System Events" to keystroke "q"'
sleep 2; kill -TERM $pid 2>/dev/null
grep -E '^preview_count' /tmp/t.toml       # the assertion
```

`key code` is what the keys §10.1 names but `keystroke` cannot spell:
125 Down, 126 Up, 123 Left, 124 Right, 36 Enter, 53 Esc, 49 Space. G12
exercised Down, Right, Enter, Esc and a `keystroke` letter; G13 added Space, in
runs of forty for a top out.

**And the window can be looked at, which G13 is when that was worked out.**
`screencapture -x -o shot.png` takes the screen without a shutter sound or a
cursor, and `sips -c H W --cropOffset Y X` trims it to the window — the two
together are how B3, B5, B9 and B10 were checked. It needs **Screen Recording**
permission, granted once beside the Accessibility one.

**It captures the whole screen, which is the developer's screen.** When the keys
go astray the game is not the front window and the shot is of whatever is —
mail, messages, someone else's window. That happened at C12. Crop to the game's
own rectangle rather than reading a full-screen grab, delete the file when the
check is done, and treat "the shot does not show the game" as a reason to stop
and re-run rather than to look closer at what it does show. This is what turns "a
person has to look at it" from a blocker into a step: G13 read the attract
screen, the six previews, the controls table with two bindings gone, and the
pause that another application stealing focus forced, off actual pixels. A
screenshot is worth taking even when a test passes — the §13.5 panel's nine
rows, and the fact that §12.3's colour depth is not among them, is not something
any assertion in the tree was going to tell you.

**A key is dropped now and then, and that is the thing to design around.** It
was measured here: the same six-key script into the §13.5 Options panel failed
once and passed once, and the failure was a lost `Down` — which put the cursor
on CONTROLS, made `Enter` open the controls box, and left the remaining keys
doing nothing at all. Nothing errored, and nothing looked wrong. So **script
towards an observable outcome and assert on it** — a config file that gained a
value, a process that exited — never on the exit status of `osascript`, which
is `0` whether or not the key arrived. A run that asserts nothing is a run that
proves nothing.

Three more things. It needs **Accessibility permission** for whatever runs the
command, granted once in System Settings, and does nothing without it. Keys go
to the **front** window, so nothing else may steal focus mid-script — and
§G4.7 pauses a game the moment something does, which is a correct result and
not a hang. And allow several seconds after launch before the first key: the
window has to exist and take the keyboard.

That is how G12 checked the Options-panel save in both front-ends, the §G8.10
geometry write-back, `remember_window = false`, and the 125 % scale bug that
shrank the window on every run — none of which any test in the tree can see.
It is worth more than it looks: it turns "a person has to look at it" into a
loop. The scale bug in particular was found by running the same binary *twice*
and diffing the file, which is exactly what a human at a window does not think
to do. Like `drive.py`, it substitutes for, but does not replace, playing the
game.
