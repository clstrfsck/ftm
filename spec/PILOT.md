# PILOT — the automated player

**Status: normative.** This document owns **§P1-§P9** and is cited as `PILOT.md
§P3`, the way `GUI.md §G4` is. `PILOT-PLAN.md`'s stages are P0-P8; the `§` is
what tells a section from a stage, exactly as it does for `GUI.md` and
`EGUI-PLAN.md`.

PILOT is a player, not a game mode. §11's Marathon is the game it plays, under
the same `RulesConfig` a person would get, through the same `Game::tick`. What
changes is who holds the controls.

The name is deliberate, and it is not ROBOTO: Roboto is Google's typeface, and
§1.3 keeps a trademarked word off the screen rather than beside it — the
decision that made a four-line clear a `QUAD`.

---

## §P1 The mode

- **PILOT is chosen from §13.3's menu**, on its own row under **PLAY**. It
  starts a game the player watches. There is no idle demo: §13's attract screen
  does not start playing by itself, and §13.6's sixty-second idle behaviour is
  unchanged.
- **A PILOT game is an ordinary game.** It resolves its rules from the config
  like any other (§6.1), takes its seed from the front-end like any other
  (`FRONTEND.md` F3), raises the same events, and draws through the same
  playing screen. §9's rules, §11's mode and §15's timing are untouched.
- **The two native front-ends offer it. A browser tab does not** — a tab would
  run the search on the frame thread, and nothing requires it. Which items a
  screen offers is already `Session::menu`'s answer (§13.3, `GUI.md` §G8.11), so
  this is a shorter list rather than a `cfg`.
- §7 gains no phase and no screen. What it gains is a **player** on the edge
  into `Playing`:

  ```rust
  enum Player {            // who holds the controls
      Human,
      Pilot,
  }

  enum Next {
      Attract,
      Play(Player),
      Quit,
  }
  ```

  §7's table gains one row — `Attract`, menu item **PILOT** activated,
  `Playing` with `Player::Pilot` — and every other transition is unchanged. A
  restart, from §10.1's held key or from the pause menu, **keeps the player**: a
  spectator who asks for another game gets another PILOT game, and is never
  silently handed a game nobody is holding the keyboard for.
- **A PILOT game ends when the spectator ends it, or when it tops out.** There
  is no piece cap and no time limit; a planner that does not top out plays until
  someone stops it. §P8's benchmark is where caps live.

---

## §P2 The information boundary

The planner may know exactly what a person watching the screen could know, and
nothing else. This is the section the rest of the design is arranged around,
and it is held by **a type rather than by a convention** — an audit proves
nothing about the commit after it (§17.3 A10 made the same trade).

### §P2.1 What is fair

- The locked cells of the **visible** field, as `GameView` reports them (§12.7).
- The falling piece, the held piece and whether hold is spent (§9.7).
- The next queue, to **exactly `preview_count`** — what §12.4 and `GUI.md` §G4
  draw, no more.
- The **inferred bag remainder**: §9.6 deals seven distinct pieces per bag, so a
  person who counts knows which are left in the open one. Counting what has
  already been seen is fair; reading what has not is not.
- The configuration, the score, the level, the line count, the combo and the
  back-to-back state — all of `GameView`.

### §P2.2 What is not

The real bag, the generator, and any piece past the preview. `Game` is `Clone`
and a clone carries all three; `Game::bag_remaining` is hidden information with
an accessor. **So the planner never holds a `Game`.**

### §P2.3 The fork

```rust
// crate-private, re-exported from core/mod.rs with `pub(crate) use`.
impl Game {
    /// A fork for search: these rules and this board, with the randomiser
    /// replaced by exactly the pieces the caller supplies.
    pub(crate) fn fork(&self, queue: &[PieceKind]) -> SearchGame;
}

#[derive(Clone)]
pub(crate) struct SearchGame { /* … */ }

impl SearchGame {
    pub(crate) fn tick(&mut self, input: &TickInput, out: &mut Vec<GameEvent>);
    pub(crate) fn state(&self) -> PlayState;
    pub(crate) fn view(&self) -> GameView;
    pub(crate) fn current(&self) -> Option<ActivePiece>;
    pub(crate) fn held(&self) -> Option<PieceKind>;
    pub(crate) fn hold_locked(&self) -> bool;
    /// What is left of the scripted queue — the caller's own pieces.
    pub(crate) fn scripted(&self) -> &[PieceKind];
    /// Whether the scripted queue ran out and the fork stopped.
    pub(crate) fn exhausted(&self) -> bool;
    /// Where the piece in play stands, and what was last done to it (§P4.2).
    pub(crate) fn pose(&self) -> Option<Pose>;
}

/// The five numbers §P4.2 deduplicates positions on. Deliberately not
/// `ActivePiece`, which is not in the core's façade and may not join it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Pose {
    pub(crate) kind: PieceKind,
    pub(crate) col: i32,
    pub(crate) row: i32,
    pub(crate) rotation: Rotation,
    /// §9.13's "the last successful action was a rotation", and the kick it
    /// used: a pose reached by turning and the same pose reached by shifting
    /// are two different placements, and only one of them is a spin.
    pub(crate) spun: bool,
    pub(crate) kick: u8,
}
```

- The randomiser is **replaced, not hidden**: a `SearchGame` holds a scripted
  queue and no generator, so there is no hidden future inside it to read by
  accident or on purpose. The replacement is at the level of §9.6's bag itself,
  which is one of two *sources* — a seeded generator with the bag it shuffles,
  or a list — so a fork does not hold a randomiser it has promised not to ask.
- It is **`Clone`**, which is P6's one addition to the seam and is not an
  accessor: a search deeper than one ply *continues* from a position it has
  already reached rather than replaying to it from the root, so a node has to be
  copyable. A clone leaks nothing, because there is nothing in a fork to leak —
  the randomiser was replaced before the fork existed, and a copy of a scripted
  queue is the same list of the caller's own pieces. The same derive on `Fork`
  is what carries it to the planner's side.
- That list above is an **upper bound on this surface, not a shopping list**.
  Each accessor lands with the stage that reads it, and nothing here may ever be
  wider: `tick`, `state`, `view`, `scripted` and `exhausted` are P2's, and
  **that is still the whole of it after P6** — P3 needed none of the three it was
  expected to, and P6 needed a derive rather than a method. The pose and the hold
  state were
  expected with P3's placement generator and were not needed by it: §P4.1
  produces input sequences and *replays* them, so what it wants to know about
  the position it reads from `view()` — the board, the piece in play, the hold
  slot and whether hold is spent are all on a `GameView` (§12.7). They are
  §P4.2's, and **P7 is where they landed** — as one accessor rather than three,
  and not the ones expected. The hold state was on `GameView` all along; what
  the seam actually lacked was §9.13's metadata, which no view reports and which
  is the difference between a spin and a piece resting in a hole. The second
  problem §P4.2 had to solve first was solved by not raising it: `ActivePiece`
  is not in the core's façade and may not become part of it (§17.3 A10), so
  `pose()` returns a `Pose` of five numbers, which is crate-private beside
  `SearchGame` and re-exported the same way. `Fork::pose` is `pub(crate)` for
  the same reason, which is what keeps the planner's *public* surface at §P3.4's
  four items: nothing outside the crate deduplicates positions.
- When the scripted queue runs out the fork **refuses to spawn** and reports
  `exhausted()`. A search cannot run past its own horizon, because the object it
  runs on cannot. It waits in §9.12's entry delay: a fork that has run out is
  **not over and has not topped out**, so a search may stop looking rather than
  have to tell the end of its knowledge from the end of a game.
- `Game::fork` is `SearchGame`'s only constructor, and `SearchGame` never hands
  back the `Game` inside it.
- **§17.3's A10 is unchanged.** `core/mod.rs` re-exports this with
  `pub(crate) use`, so the core's *public* surface does not grow and a §19
  client is handed exactly what it was handed before. One consequence is
  structural rather than incidental: a crate-private type may not appear in a
  public signature, so `src/pilot/` wraps it in a `Fork` of its own, and that
  is what anything outside the crate — `tests/`, and §P8's runner — searches
  through.

### §P2.4 Observation

The queue a fork is given is built from observation, not from the live game:

- `GameEvent::PieceSpawned(kind)` names every piece as it is dealt, and
  `HoldUsed` is a separate event (§12.8). The bag tracker counts **deals**; it
  never reconstructs them from a changing preview.
- A hold swap is not a deal. A swap into an empty slot *is* followed by one, and
  the event stream says which happened.
- **The first piece of a game is the one deal no event stream mentions.**
  `Game::new` spawns it during construction and discards the `PieceSpawned`,
  because construction is not a tick (§9.4). A tracker that waited for the event
  would be one piece out for the whole game, and would offer as a hypothesis a
  piece already on the screen.
- Seven deals close a bag. The remainder is the set not yet dealt from the open
  one, which is what §P6's chance nodes branch over. The bag closes on the
  **seventh** piece and not on the eighth: waiting for the eighth would read
  "nothing can come next" at the one moment any of the seven can.
- A preview may cross a bag boundary — §6.3 allows six of them and a bag holds
  seven — so a piece on the screen can also be a legitimate hypothesis, out of
  the bag after the one it was dealt from.
- Beyond the preview, the queue is the planner's own **hypotheses**. Changing
  the live game's hidden future, with visible information held constant, cannot
  change a plan — §P9 C2 asserts it, and §P2.3 is why it is true.

---

## §P3 Control

### §P3.1 The plan

- The planner runs **inside the tick that spawns a piece** — the tick whose
  events contain `PieceSpawned` — and produces a **plan**: one `TickInput` per
  tick, from the following tick until the piece locks.
- Nothing else re-plans. A hold is part of a plan, not a reason to make a new
  one, and the plan is never recomputed from a partly executed state, so the
  live game replays exactly what the search simulated.
- **"Until the piece locks" is whichever tick that turns out to be.** A plan's
  last input is normally the hard drop it ends with, but gravity may lock the
  piece first — §9.11's lock delay running out while it is still being shifted,
  which is ordinary at a high level and is everything §P4.2 exists for. The
  search sees that happen on the fork, so the plan is **truncated** there: an
  input after the lock would be one the *next* piece received, which is a
  divergence rather than a plan.
- A plan that diverges from the outcome the search predicted is a **bug**, and
  is caught by a debug assertion as the plan is consumed (§P9 C5). What the
  assertion may compare is bounded by what the fork was told: the board, the
  figures, the hold slot and the piece the plan controls, always — and the
  piece that spawns *after* the lock only when the fork's queue outlasted it. A
  hold at a `preview_count` of 1 uses the queue up, and what the live game
  deals next was hidden information at the moment the plan was made (§P2.1). A
  fork that cannot say is §P2.3 working rather than failing.
- A batch of ticks (§15.2 step 4) may contain more than one spawn, and may be up
  to `MAX_CATCH_UP_TICKS` long. The node budget is sized so a full catch-up
  batch still fits in a frame (§P6.4).

### §P3.2 The input cap

A `TickInput` may legally carry four actions and shift ten cells at once. PILOT
may not:

- **at most one `Action` per tick**;
- **at most one shift cell per tick** (`shift_cells` ∈ {0, 1});
- `soft_drop` may be held, because it is level-triggered and a person holds it.

One cell a tick is sixty a second — superhuman, but a piece that travels rather
than teleports, which is what §12.5's hard-drop trail and lock flash are drawn
for, and what makes a watched game legible. It also bounds §P4.2's branching
factor. This is a rule, and it is a test.

### §P3.3 No clock

**The planner takes no clock, no frame count and no wall-time budget.** It is a
pure function of the fair state (§P2) and its `Settings`. Three consequences,
all of them load-bearing:

- A PILOT round is **cadence-invariant**: 60 Hz, 144 Hz and a jittery cadence
  with empty frames play the same game (§P9 C4). This is §19.4's property one
  layer up, and `tests/pump.rs` is where it is asserted.
- The search may **never be amortised across frames**. Spreading it would make
  the round depend on the front-end's pace, which §15.4 forbids of the rules and
  this section forbids of the player.
- `src/pilot/` is **platform-free** and builds under `make shell` and
  `make portable`. That is the compiler holding this section, exactly as it
  holds §3.1 for `shell/`. A `cfg(target_arch)` in `src/pilot/` is a bug.

### §P3.4 The controller

```rust
// src/pilot/, a sibling of core/ and shell/.
pub struct Settings {
    pub depth: u8,        // plies, clamped by preview_count (§P6.1)
    pub beam: u16,        // states kept per ply
    pub nodes: u32,       // the budget, an integer count (§P6.4)
    pub exact: bool,      // §P4.2's walk rather than §P4.1's hard drop
}

pub struct Pilot { /* … */ }

impl Pilot {
    pub fn new(rules: &RulesConfig, settings: Settings) -> Self;
    pub fn settings(&self) -> Settings;
    /// Fold in one tick's events: deals, holds, locks (§P2.4).
    pub fn observe(&mut self, events: &[GameEvent]);
    /// The input for this tick, planning first if a piece has just spawned.
    pub fn input(&mut self, game: &Game) -> TickInput;
}
```

**The order those are called in is §15.2's**: `input` for a tick, then
`Game::tick`, then `observe` of what that tick raised. A tick's input is
decided before the tick happens and its events exist only after it, so there is
no other order to call them in — and a `Pilot` therefore belongs to its game
from that game's first tick, because the first piece is a deal no event mentions
(§P2.4) and the view is the only place it can be read.

`src/pilot/` may name the core's façade and §P2.3's seam, and
`shell::config::RulesConfig`, and nothing else in either layer. It must not
learn what a screen, a key or a stamp is. Its own tests are held to the same
list, because a test that reaches past it is testing a module allowed to do
something the module under test is not.

Beside the controller, the module is public in three places, and each is
something a caller outside the crate genuinely needs: `knowledge::Knowledge`
and its `PieceSet` (§P2.4's counting), `fork::Fork` (§P2.3's seam, which cannot
appear in a public signature under its own name), and `eval`'s `Board`,
`Outcome`, `Features` and `Weights` (§P5's facts and the opinion of them).
§P8's runner and the acceptance checks of §P9 are written against exactly
those.

---

## §P4 Placements

### §P4.1 The simple generator

The first implementation enumerates, for each of the current piece and — when
hold is enabled — the held or next piece: every rotation, every column the piece
fits in, reached by rotating at spawn, shifting, and hard dropping. This is what
a placement search can do while the well is open, and it is enough to play.

### §P4.2 Exact reachability

The simple generator misses everything a hard drop cannot reach: tucks, spins,
placements under an overhang, and everything at high gravity. The exact
generator is a breadth-first search over **forks**, using legal `TickInput`s
under §P3.2's cap, and must model:

- shifts, both quarter turns and §6.3's optional 180;
- gravity, soft drop, hard drop and §9.11's lock delay with its reset budget;
- SRS wall and floor kicks (§9.5), including §9.13's kick-test-5 T-spin;
- hold, with §9.7's once-per-piece rule and both the empty and occupied cases;
- same-tick ordering, which matters once gravity is fast enough to move a piece
  in the tick an action is delivered;
- Block Out and Lock Out as outcomes, not as errors (§9.16).

Both generators produce **input sequences**, never board positions: a placement
nothing can reach is not a placement. States are deduplicated by pose, rotation
metadata, hold state and queue position; outcomes by board, hold, queue and
whether the branch ended in §9.16.

**Both generators exist, and `Settings::exact` chooses.** They answer the same
question and are interchangeable at the point a search calls one, so the search
does not know which answered. §P4.1 is the default, and P7 measured why: a walk
advances some 6,400 forks a generation where a hard drop enumerates 104, and
§P6.4's budget is spent inside the walk's first generation. The report header names the generator the
run used, because a baseline that did not say would be one nobody could repeat.

Three things the walk's shape settles, each of which would otherwise be a
special case:

- **Descent is soft drop, not waiting.** A plain tick falls at §9.9's period,
  which is sixty ticks to the row at level 1; §9.10's divided period is what
  makes a graph of a thousand states cost a thousand-odd ticks rather than sixty
  thousand. Soft drop is neither an action nor a shift, so it costs §P3.2's cap
  nothing.
- **A hard drop is played from the poses that rest**, not from every pose. A
  hard drop from mid-air lands on a resting pose directly below it, which the
  descent edges reach anyway, so one drop per resting pose is one terminal per
  placement there is.
- **Hold is played once, at the root.** §9.7 allows it once per piece and the
  swapped piece spawns where any piece spawns, so a hold after a shift reaches
  the same positions by a longer road.

**Lock state is not in the state key, and the search order is why.** A
breadth-first walk reaches a pose by its shortest path first, and the shortest
path has spent the least of §9.11's delay and the fewest of its resets getting
there — so the state a lock-state key would have kept beside it can do nothing
the kept one cannot. Keying on it would multiply every resting pose by the
thirty ticks of a delay counting down. §9.9's sub-row accumulator is not in the
key either, and that one is a real gap rather than a dominated one: two arrivals
at a pose can differ by a tick in when they next fall. It is bounded by one tick
of gravity, and it is what lets a descent be one edge instead of three states to
the row.

**Legality is the fork's and not the key's.** Every edge is a real `Game::tick`,
so a walk that slid a piece along the floor past §9.11's reset budget does not
produce an illegal placement — the fork locks the piece, and the walk files that
as the placement it is.

### §P4.3 Disabled mechanics

`hold_enabled` and `allow_180_rotation` are §6.3's and may be off. A disabled
mechanic is **absent from the search**, not merely unused: §10.1 drops those
keys at the input boundary, and a planner that assumed them would plan a move
the game will not perform.

---

## §P5 Evaluation

- **Integers only.** No floating point anywhere in the planner, for §9.9's
  reason: a plan must be identical on every target and in every profile.
- The full board evaluation is applied **at leaves only**, so a persistent
  defect is not charged once per ply. Interior plies are scored from the core's
  own events and score deltas — what actually happened, rather than a second
  opinion about it.
- **"Events" excludes combo and back-to-back**, and the line is worth drawing
  because the table below lists all three together. A clear, a perfect clear and
  a top out are things that *happened* on the way past and are charged at the ply
  they happened in; §9.15's combo counter and back-to-back flag are **state**,
  and state is read once, at the leaf, where the branch leaves it. Charging a
  chain at every ply it survives pays for one back-to-back two or three times
  over, and a planner paid twice over-values it by exactly the depth it is
  searching.
- The features, each an integer over the visible field:

  | Feature | Measured as |
  |---|---|
  | Completed lines | rows cleared by the lock |
  | Clear kind | those clears counted by how many rows each took |
  | Holes | empty cells with a filled cell above them in the same column |
  | Covered-hole depth | filled cells above each hole, summed |
  | Aggregate height | sum of column heights |
  | Maximum height | the tallest column |
  | Bumpiness | sum of \|height difference\| between adjacent columns |
  | Row transitions | filled↔empty changes across each row, walls counted |
  | Column transitions | filled↔empty changes down each column, floor counted |
  | Blockades | filled cells sitting above a hole |
  | Wells | depth of each column relative to both neighbours, **less the deepest** |
  | Well rows | rows filled but for the exempt well column, **capped at four** |
  | Top out | whether the branch ended in §9.16 |
  | Combo, back-to-back | §9.15's state as the branch leaves it |
  | Perfect clear | §9.15's bonus, as an event |

- **Two of those features exist to make the planner play for score rather than
  for rows**, and both read oddly until §9.14 is put beside them. That table pays
  **800** for a quad and **400** for four singles — 1,200 for a chained quad —
  so a figure linear in rows prices identically the two things the game prices
  most differently, and a planner built on one leaves two thirds of the score
  unclaimed. *Clear kind* is what tells them apart. And the shape a quad is
  scored out of is a stack with exactly one open column, so *wells* exempts the
  deepest single one: a planner charged for that column can never build the
  thing the scoring table is trying to buy, because the well costs its depth
  every ply it exists and pays only once. Every other well is still a defect.
- **A quad cannot be bought with a bonus. It has to be bought with a price on
  the alternative and a reward for the progress.** At §P6.1's two plies a quad
  is invisible until the stack that earns one already exists, so a reward
  attached to the quad itself is never collected — measured, and the figures did
  not move by a single point. Both weights that do work are ones the planner can
  see at *every* ply:

  - a **negative** weight on the cheap clear, so a row is not spent singly. The
    band is narrow and §P8's benchmark found both ends of it.
  - a **positive** weight on *well rows* — rows already filled but for the well
    column, which is a quad one `I` away. This is the larger of the two by some
    distance, and without it a planner with every other weight right builds no
    wells, lands half its `I` pieces flat, and clears 35 quads in 16,000 pieces
    against 1,262 with it.

  **The cap at four is what makes the second one safe**, and is §9.14's
  arithmetic rather than a taste: an `I` is four cells, so the fifth row of a
  well is one nothing can clear. Rewarding by the row uncapped — or, equivalently,
  exempting the well column from *row transitions*, which was tried first — gives
  a planner that digs a well it can never cash and tops out seven games in eight.
  The failure is the same one an over-large negative on the single produces, and
  for the same reason: a stack nothing is allowed to clear reaches the ceiling.
- **Feature extraction and weights are separate structures.** Weights are
  hand-tuned integers in this release; automated tuning is not in
  `PILOT-PLAN.md`'s scope and would consume §P8's report.

---

## §P6 Search

### §P6.1 Depth

Two plies by default, and **clamped by `preview_count`** — §6.3 allows a preview
of 1, in which case the second ply is already a chance node. That is an ordinary
configuration, not an edge case, which is why §P6.3 is load-bearing rather than
decorative.

The clamp is `preview_count + 1`, and the `+ 1` is exactly one level of
hypothesis. A search `d` plies deep consumes `d` pieces from the queue — the
piece in play is the live game's and every ply after it spawns from the queue,
and a hold into an empty slot deals one early, so the worst case is a piece a
ply. A preview of `n` therefore reaches `n` plies for certain and an `n + 1`th by
hypothesis alone. **A second level of hypothesis is out**, and deliberately: it
multiplies the leaves by seven again for a piece nothing whatever is known
about, and at a preview of 1 that would be the ordinary case rather than the
exotic one.

### §P6.2 Beam

A fixed-width beam after each ply, over deduplicated states in canonical order.
Subtrees are cached by board, current and held piece, scripted queue, inferred
remainder and remaining depth.

Two details that follow from §P6.4 rather than from the beam itself. **A beam is
expanded best first**, because the budget can stop the expansion partway and a
list that may be cut has to have its promising end at the front; equal values
keep the generator's canonical order, so two builds cut at the same place.
And states are deduplicated **by the position reached and not by the inputs that
reached it** — §P4.1 shifts up to six cells either way and a shift into a wall is
refused, so a great many candidates arrive at the same place by different routes.
The board is compared as **occupancy** rather than colour: two stacks of
different pieces in the same cells are the same stack to §P5's features, and
treating them as one is most of what makes the cache worth having.

### §P6.3 Beyond the horizon

Past the preview, branch uniformly over §P2.4's inferred remainder and combine
the results at an integer **80/20** expected-to-worst-case ratio: a planner that
maximised the average alone will build setups that only one piece rescues.

The branch is **the caller's**, not the search's: the planner builds one fair
root per hypothesis and the search is handed the set. That is where the
information boundary lives, and it is why nothing inside the search can widen
what it was given — a chance node is a list of positions somebody else decided
it was allowed to consider.

### §P6.4 The budget, and ties

- The budget is an **integer node count**, never a duration (§P3.3). A **node**
  is a position the search evaluated or expanded, which is §P8.2's counter of
  that name.
- Exhausting it returns the **best fully evaluated move**. A partly evaluated
  branch is never chosen, which is what keeps the answer independent of where
  the count ran out. **A ply is the granularity of "fully"**: the budget is
  checked between one ply and the next, so a search may overrun it by the
  placements of the ply it was in the middle of generating — half a generation is
  not an answer that can be compared with anything. The first ply is therefore
  always paid for, whatever the budget, and there is always an answer to give.
- Ties are broken by lower top-out risk, then fewer inputs, then canonical
  action order. Every machine therefore chooses the same move.
- **The three settings are one decision, and since P7 the fourth is too.** P5
  measured a frame at ~11,900 nodes and §15.2 step 4 may play
  `MAX_CATCH_UP_TICKS` ticks before it draws, so a search may spend a sixth of
  that: ~2,000 nodes. Two plies over a beam of sixteen costs the first ply's
  ~104 candidates plus sixteen expansions of about the same, a little under
  1,800 — which is what makes *those* three numbers the defaults. A wider beam or
  a third ply would be spent by the budget rather than played, and the answer
  would quietly become a shallower one than the settings asked for.
- **`exact` is the same arithmetic reaching the opposite conclusion.** §P4.2's
  walk advances some 6,400 forks in a single generation, so at the 2,000-node
  budget the rule above fires on the *first* ply and the search returns the
  one-ply answer. That is not an approximation of one: P7 measured
  `--exact --depth 2` and `--exact --depth 1` producing byte-identical reports.
  Turning the walk on therefore means lifting the budget, and lifting the budget
  means leaving the frame — which is why it is not the default and why
  `Settings` carries it rather than a build deciding.

---

## §P7 The screens

### §P7.1 The menu

`MenuChoice` gains `Pilot`, drawn under `Play` and labelled **PILOT**. The
lists (§13.3, `GUI.md` §G8.11):

| List | Items | Whose |
|---|---|---|
| `MenuChoice::ALL` | PLAY, PILOT, HIGH SCORES, CONTROLS, OPTIONS, QUIT | the terminal and the native window |
| `MenuChoice::CANVAS` | PLAY, HIGH SCORES, CONTROLS, OPTIONS | a browser tab: no quit (`GUI.md` §G8.1), no PILOT (§P1) |

`NO_QUIT` is **renamed to `CANVAS`**, because it is no longer "the same list
without quit" — it is the browser's list, exactly as `Setting::CANVAS` is the
browser's panel.

### §P7.2 The room it takes

Both were measured before they were specified, on the running binaries:

- **The terminal**: the attract block grows from 21 rows to **22**
  (`tui::attract::BLOCK_HEIGHT`), because the sixth item pushes the footer line
  out of a 21-row block — which it does silently, by clipping. The panel's
  position already follows the menu's length. §12.1's 60 × 24 minimum **does not
  move**: 22 rows still leaves one above and one below.
- **The window**: nothing changes. §G7's menu band is five rows *reserved*
  regardless of the menu's length, and six items centre into the blank rows
  either side of it; the panel and the footer do not move, and §G3's 26 × 24
  grid and §G3.3's minimum are untouched.

### §P7.3 While it plays

- An unambiguous **PILOT indicator** on both playing screens, so a screenshot of
  an automated game cannot be mistaken for a player's.
- **Gameplay keys are inert.** §10.1's movement, rotation, hold and drop keys do
  nothing; they are dropped where a disabled mechanic is dropped (§10.1).
- **The spectator's keys are live**: pause and the pause menu, §13.5's Options
  panel, §10.1's controls table, the held restart key and quit. The controls
  table is still the truth about the bindings; it is the *game* that is not
  listening.
- §8.4's forced pause and `GUI.md` §G4.7's keyboard loss **still pause**. The
  spectator was not typing, but a paused game blanks the stack under §9.17
  either way, and a window that has lost the keyboard should not be spending a
  search budget on every frame.

### §P7.4 Never recorded

**No automated run reaches §14's table**, on any path: not the table, not
§12.6's name entry, and not §13's recent-entry highlight. This sits beside
§14's seeded-run rule, in the one place that knows a run's provenance. A
spectator is not a player, and a table of PILOT's scores is a table with no
players in it.

---

## §P8 The benchmark

`ftm-pilot` is a native, no-render runner over the same controller the
interactive mode uses. It is not a front-end: it draws nothing and has no
screen, no keys and no menus. It is behind neither front-end feature but a
`bench` one of its own, which names an argv and nothing that renders.

**It is split at §P3.3's line, and that is why it is two files.**
`pilot::bench` plays the games and folds them into integers, and has no clock in
it, because `make portable` compiles `src/pilot/` for a target that has none;
`src/bench.rs` is the grammar below and the wall clock, and sits beside
`src/argv.rs` for the same reason that file exists — a flag's spelling is a
desktop's and its meaning is not.

### §P8.1 Grammar

| Flag | Meaning |
|---|---|
| Flag | Meaning | Default |
|---|---|---|
| `--seeds A..B`, `--seeds A..=B`, `--seeds N` | a range either way inclusive, or N seeds from 0 | `8` |
| `--pieces N` | the piece cap per game | `1000` |
| `--depth N`, `--beam N`, `--nodes N` | §P6's settings | `Settings::default()` |
| `--exact`, `--simple` | §P4.2's walk or §P4.1's hard drop | `Settings::default()` |
| `--preview N`, `--start-level N` | §6.3's rules | §6.3's |
| `--hold`, `--no-hold`, `--rot180`, `--no-rot180`, `--lock-down R` | §6.3's rules | §6.3's |
| `--json` | the report as JSON rather than text | off |

Every spelling of `--seeds` resolves to a **list**, which is what the report
prints and what a checked-in seed set is: a set worth keeping need not be
contiguous, and a report should say what it played rather than what it was asked
for. A range that runs backwards is an error; an empty one is a legal batch of
nothing.

**It never reads §6.2's config file.** `preview_count` alone changes how well
the planner plays, so a baseline that depended on whoever ran it would compare
nothing. The defaults are the specification's, and the resolved `RulesConfig` is
printed in the report's header. This is also why the rules flags are *not*
§6.4's `Overrides`: an override means "leave the file alone", and there is no
file here. They are clamped to §6.3's ranges all the same, and silently, for the
reason `RulesConfig::from_settings` is silent — there is nobody to warn.

### §P8.2 Report

Score, lines, pieces, top-out rate, nodes searched and placements per second,
per seed and in aggregate. Wall-clock throughput is *reported*; **acceptance
limits use node counts**, which are deterministic and comparable between
machines.

**The report is two parts, and only the first is comparable.** Everything above
is integers — the resolved rules, the per-seed rows, the aggregate — and is
byte-for-byte reproducible on any machine, in any profile and on any target,
which is what §P9's C11 asks of it and what a baseline is diffed against. The
throughput figures are a **separate trailing block**, headed as this machine's
and excluded from that promise: they move with the load, the hardware and the
optimiser, and a report that mixed them into the rows would be one nobody could
diff. In JSON the same split is a `timing` key beside the rest.

The two cost counters are **nodes** (states evaluated) and **placements**
(candidates generated). They are equal at one ply *over §P4.1*, where every node
is a placement; P6 is where they first part, since a beam evaluates interior
states that are nobody's placement and a transposition hit is a placement that
costs no node. §P4.2 parts them by nearly two orders of magnitude: a walk
advances thousands of forks to find scores of distinct ways to put the piece
down, and it is the thousands that §P6.4's budget is spent in — a budget charged
the scores would be charged a fraction of the work it paid for.

The report's header names the generator beside §P6's three numbers, because
the two answer the same question at very different prices and a row that did not
say which was asked is a row nobody can reproduce. Neither is a duration, and nothing in `src/pilot/` may read either back —
a counter a search consulted would be §P3.3's clock under another name.

**The node budget §P6.4 needs is measured here**, and the measurement is of the
worst case rather than the average frame: §15.2 step 4 plays up to
`MAX_CATCH_UP_TICKS` ticks before it draws, so a frame may in principle contain
that many searches, and the budget a search may spend is one frame's nodes
divided by that count.

### §P8.3 Baselines

Fixed seed sets are checked in, and a small deterministic batch runs under
`cargo test`. **Plan snapshots live in their own file and are expected to move
when weights change** — which is the opposite of `tests/snapshots/scripted_game.txt`,
and the reason they are not kept beside it. §17.2's I1 snapshot and §19.4's
batch-invariance canary must not move at any stage of this work; one that does
has found a bug.

There are two of them, and they are checked in beside each other.
`tests/snapshots/pilot_plan.txt` is the **plans**: three fixed seeds, and what
the planner pressed for each piece a tick at a time, which §P3.2's cap is what
makes legible as one character a tick. It lives in `tests/` rather than in
`src/pilot/` because the isolation test beside it has to look at a game's bag,
and §P3.4 forbids the planner's own tests to do the very thing the planner may
not.

The checked-in batch is `tests/snapshots/pilot_bench.txt`, and it is small on
purpose: `cargo test` is a debug build, where §P3.1's divergence assertion
replays every plan a second time, so a batch sized for a baseline would be
measuring the assertion. The recorded release baseline is the other thing kept,
and it is kept in `PILOT-PLAN.md` beside the stage that measured it rather than
here — a number this document promised would be a number every machine had to
reproduce.

---

## §P9 Acceptance

Each criterion is checked on its own, and the record says *how* — the shape
§17.3's A1-A10 and `GUI.md` §G9.3's B1-B12 both take.

| | Criterion | How it is checked |
|---|---|---|
| C1 | Build clean | `make check`, silent. |
| C2 | Fair | Changing the live game's hidden future with visible information held constant does not change a plan — and `SearchGame` has no accessor that could (§P2.3). |
| C3 | Deterministic | The same seed and settings give the same game twice, on the host and under `make portable`'s target; no floating point in `src/pilot/`. |
| C4 | Cadence-invariant | A PILOT round at 60 Hz, 144 Hz and a jittery cadence with empty frames gives a byte-identical `GameView` (`tests/pump.rs`). |
| C5 | Legal | Every chosen path replays through unmodified rules, and no emitted `TickInput` exceeds §P3.2's cap. |
| C6 | Rules honoured | Hold and 180 rotation each on and off; `preview_count` 1 and 6. |
| C7 | Not recorded | A PILOT top out reaches neither §14's table nor §13's panel (§P7.4). |
| C8 | Spectator controls | Pause, pause menu, restart, Options, Controls, quit — on both front ends; restart keeps the player (§P1). |
| C9 | Terminal | `tools/drive.py`: the menu row, a game started and watched, the indicator on screen, the footer still drawn (§P7.2). |
| C10 | Window | `osascript` to the menu row, `screencapture` of the result; a focus steal pauses it (§P7.3). |
| C11 | Benchmark | Fixed-seed reports reproduce byte for byte; node counts and throughput recorded. |
| C12 | Watched | A person watches a full game on each front end and judges that it plays sensibly. No test in the tree can see this. |
| C13 | Baseline intact | I1's snapshot and §19.4's canary unmoved; the terminal's output byte-for-byte what it was, outside the menu row. |
