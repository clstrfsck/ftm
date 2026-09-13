//! §P4.2's exact generator: a breadth-first search over forks.
//!
//! §P4.1 rotates at spawn, shifts and hard-drops, which reaches every column
//! while the well is open and nothing else. What it cannot reach is everything
//! a hard drop cannot: a tuck under an overhang, a piece slid sideways into a
//! notch, a T rotated into a slot, and — once gravity is fast enough to move a
//! piece in the tick an action is delivered — a good deal of the ordinary board
//! as well. This module reaches those by *walking*: from the position the piece
//! is in, one legal [`TickInput`] at a time, until every distinct position it
//! can stand in has been visited.
//!
//! **It is the same kind of object as §P4.1 and produces the same kind of
//! answer.** Both generators produce input sequences, never board positions
//! (§P4.2): a placement nothing can reach is not a placement, and the only way
//! to know what a sequence reaches is to play it. Every edge of this graph is a
//! real `Game::tick` on a [`Fork`], so kicks, lock delay, §9.12's ordering and
//! §9.16's two top-outs are all ordinary rather than modelled.
//!
//! ## The shape of the walk
//!
//! A node is a fork with a piece in play. Its edges are the five things one
//! tick may do to that piece — two quarter turns, §6.3's optional half turn,
//! and a shift either way — plus a **descent**, which holds soft drop until the
//! piece changes row, and which is the only way down: a plain tick descends at
//! §9.9's fall period, and at level 1 that is once every sixty ticks.
//!
//! A hard drop is not an edge from every node. It is played from the nodes that
//! **rest** — the ones a descent leaves where they stand — and that is exact
//! rather than an economy: a hard drop from a node in mid-air lands on a resting
//! node directly below it, which the descent edges reach anyway. One hard drop
//! per resting pose is one terminal per placement there is.
//!
//! Hold is played once, at the root, before anything else. §9.7 allows it once
//! per piece and the swapped piece spawns where any piece spawns, so holding
//! after a shift reaches the same positions by a longer road.
//!
//! ## What two positions being "the same" means
//!
//! §P4.2 deduplicates states by pose, rotation metadata, lock state, hold state
//! and queue position, and this keys on all of those but the lock state, for a
//! reason that is the search order rather than an approximation: a breadth-first
//! walk reaches a pose by its **shortest** path first, and the shortest path is
//! the one that has spent the least of §9.11's lock delay and the fewest of its
//! resets getting there. The state a lock-state key would have kept alongside it
//! is strictly worse at everything it could go on to do. Keying on it instead
//! would multiply every resting pose by the thirty ticks of a delay counting
//! down — thirty thousand states where there are now under four.
//!
//! §9.9's sub-row accumulator is not in the key either, and that one *is* a
//! gap — two arrivals at one pose can be a tick or two apart in when they next
//! fall. It is bounded by a tick of gravity and is what keeps the descent a
//! macro-edge rather than three keyed states per row.
//!
//! Terminals are deduplicated by §P4.2's own list: the board they leave, the
//! hold slot and what is left of the queue — and, since a top out leaves a board
//! that can look like any other, whether the branch ended in §9.16.
//!
//! **Legality is the fork's, not the key's.** Every edge is a real `Game::tick`,
//! so a walk that slid a piece along the floor past §9.11's reset budget does
//! not produce an illegal placement: the fork locks the piece where it was, and
//! the walk files that as the placement it turned out to be.
//!
//! ## What it costs
//!
//! An empty well is the worst board there is — everywhere is reachable — and
//! measures 3,685 states, 26,031 forks advanced and 68 distinct placements.
//! Over a real game the stack does most of the pruning: `make bench` records
//! **~6,400 forks a generation**, against §P4.1's flat 104. That is the whole of
//! the trade, and `PILOT-PLAN.md` P7 is where what it buys is written down.

use std::collections::{HashMap, HashSet};

use crate::core::{Action, GameEvent, PieceKind, Pose, Shift, TickInput};
use crate::pilot::eval::ROWS;
use crate::pilot::fork::Fork;
use crate::pilot::placement::{self, Generated, Placement, Tally};
use crate::shell::config::RulesConfig;

/// How many ticks a descent will hold soft drop before giving up on a row.
///
/// §9.10 divides the fall period rather than replacing it, so the slowest a
/// descent can be is level 1 with `soft_drop_factor` at §6.3's minimum of 1 —
/// no speed-up at all, and sixty ticks to the row. Anything faster arrives
/// sooner; nothing is slower, because a period only shortens.
const DESCENT_LIMIT: u32 = 64;

/// A ceiling on the walk, in distinct states.
///
/// Not a budget — §P6.4's budget is the search's, is counted in nodes and is
/// spent across the whole of a search — but a bound on one generation, so a rule
/// changing under this module could never turn a bounded graph into an unbounded
/// one. The worst board there is, an **empty** one, measures 3,685 states with
/// hold enabled: a ten-column well twenty rows deep in four orientations is 800
/// poses, §9.13's metadata multiplies that, and a hold offers two pieces' worth.
/// This is a little over twice it. Reaching it is a bug in debug and a truncated
/// answer in release, which is the mildest thing it could be — the placements
/// already found are all legal.
const WALK_LIMIT: usize = 8_192;

/// Every placement the piece in play can actually be brought to (§P4.2).
///
/// The signature is §P4.1's, deliberately: the two generators are
/// interchangeable, [`crate::pilot::Settings::exact`] is what chooses between
/// them, and nothing above here knows which one answered.
pub(crate) fn placements(fork: &Fork, rules: &RulesConfig) -> Generated {
    let Some(pose) = fork.pose() else {
        // Nothing in play: the fork has run out of the caller's queue, or is
        // waiting out §9.12's delays. Neither is an error and neither has a
        // placement in it (§P2.3).
        return Generated::default();
    };
    let mut walk = Walk::new(rules);
    walk.enter(fork.clone(), pose, Trail::ROOT);

    // §9.7's hold, played first and only. The swapped piece spawns where every
    // piece spawns, so a hold after a shift reaches nothing a hold before one
    // does not — and §P4.3 leaves the mechanic out of the walk altogether when
    // the rules have it off, because §10.1 drops the key at the input boundary
    // and a plan that pressed it would plan a move the game declines to make.
    let view = fork.view();
    if rules.hold_enabled && !view.hold_locked {
        let held = TickInput::action(Action::Hold);
        let step = walk.trail.push(Trail::ROOT, held);
        walk.advance(fork.clone(), &held, step);
    }

    walk.run(rules);
    Generated {
        placements: walk.out,
        nodes: walk.nodes,
    }
}

/// One generation's walk: what it has seen, where it is, and what it found.
struct Walk {
    /// Positions already entered, by §P4.2's key.
    seen: HashSet<StateKey>,
    /// Distinct placements already emitted, by §P4.2's outcome key, and the
    /// index of the one that got there first.
    kept: HashMap<OutcomeKey, usize>,
    /// The frontier, in breadth-first order, walked with an index rather than
    /// popped: a node's inputs are reconstructed from [`Trail`] on demand, so
    /// nothing here owns a sequence.
    frontier: Vec<Node>,
    trail: Trail,
    /// The five single-tick edges, in §P6.4's canonical order.
    moves: Vec<TickInput>,
    /// Reused across every edge of every node, because `LinesCleared` carries a
    /// `Vec` and this runs ten thousand times a generation.
    events: Vec<GameEvent>,
    out: Vec<Placement>,
    /// Forks advanced, which is what a node costs the budget (§P8.2).
    nodes: u64,
}

/// A position the walk has reached and will expand.
struct Node {
    fork: Fork,
    /// Where its input sequence ends in the [`Trail`].
    step: u32,
}

impl Walk {
    fn new(rules: &RulesConfig) -> Self {
        // §P6.4's canonical order, and §P4.3's absent mechanic: a half turn the
        // rules forbid is not generated and then refused, it is simply not one
        // of the things a tick may do.
        let mut moves = vec![
            TickInput::action(Action::RotateCw),
            TickInput::action(Action::RotateCcw),
        ];
        if rules.allow_180_rotation {
            moves.push(TickInput::action(Action::Rotate180));
        }
        moves.push(TickInput::shift(Shift::Left));
        moves.push(TickInput::shift(Shift::Right));
        Self {
            seen: HashSet::new(),
            kept: HashMap::new(),
            frontier: Vec::new(),
            trail: Trail::default(),
            moves,
            events: Vec::new(),
            out: Vec::new(),
            nodes: 0,
        }
    }

    /// Walk the frontier until it stops growing.
    fn run(&mut self, rules: &RulesConfig) {
        let mut at = 0;
        while at < self.frontier.len() {
            let node = Fork::clone(&self.frontier[at].fork);
            let step = self.frontier[at].step;
            at += 1;

            for index in 0..self.moves.len() {
                let input = self.moves[index];
                let next = self.trail.push(step, input);
                self.advance(node.clone(), &input, next);
            }
            self.descend(&node, step, rules);
        }
    }

    /// Play one tick and file what it produced: a placement, a new position, or
    /// nothing at all.
    ///
    /// "Nothing at all" is the common answer and is not a failure — a rotation
    /// the wall refuses and a shift into it both leave the piece where it was,
    /// which is a position already seen.
    fn advance(&mut self, mut fork: Fork, input: &TickInput, step: u32) {
        self.nodes += 1;
        self.events.clear();
        fork.tick(input, &mut self.events);
        if placement::landed(&self.events, &fork) {
            // A tick that locks the piece is a placement in itself: at a
            // gravity fast enough, a shift or a turn is the last thing that
            // happens to a piece, and §9.16's Block Out never locks at all.
            let tally = self.tally();
            self.keep(fork, step, tally);
            return;
        }
        let Some(pose) = fork.pose() else {
            return;
        };
        self.enter(fork, pose, step);
    }

    /// File a position, if it is one the walk has not stood in before.
    fn enter(&mut self, fork: Fork, pose: Pose, step: u32) {
        if self.frontier.len() >= WALK_LIMIT {
            debug_assert!(false, "§P4.2's walk outgrew its ceiling");
            return;
        }
        let view = fork.view();
        let key = StateKey {
            pose,
            held: view.hold,
            hold_locked: view.hold_locked,
            queue: fork.upcoming().len().min(u8::MAX as usize) as u8,
        };
        if self.seen.insert(key) {
            self.frontier.push(Node { fork, step });
        }
    }

    /// Hold soft drop until the piece changes row (§9.10).
    ///
    /// Three outcomes, and each is one of the walk's three answers. The piece
    /// fell, and there is a new position under this one. The piece locked on
    /// the way down, which at a lock delay of zero is what resting means. Or
    /// nothing moved, which means the piece is **resting**: it is standing where
    /// it would come to rest, so this is the pose a hard drop from here puts it
    /// in, and a hard drop is what turns it into a placement.
    fn descend(&mut self, node: &Fork, step: u32, rules: &RulesConfig) {
        let soft = TickInput {
            soft_drop: true,
            ..TickInput::default()
        };
        // The **row** and not the whole pose: §9.13's flag is cleared by the
        // move gravity makes (`Game::try_move`), so a descent that succeeds
        // changes two fields and a descent that has not happened yet changes
        // neither. Asking about the row is asking the question this loop is
        // actually waiting on.
        let before = node.pose().map(|pose| pose.row);
        let mut fork = node.clone();
        let mut at = step;
        for _ in 0..descent_limit(rules) {
            self.nodes += 1;
            at = self.trail.push(at, soft);
            self.events.clear();
            fork.tick(&soft, &mut self.events);
            if placement::landed(&self.events, &fork) {
                let tally = self.tally();
                self.keep(fork, at, tally);
                return;
            }
            let pose = fork.pose();
            if pose.map(|pose| pose.row) != before {
                if let Some(pose) = pose {
                    self.enter(fork, pose, at);
                }
                return;
            }
        }
        // Nothing moved in a whole fall period: the piece is resting, and the
        // one thing left to do to it is put it down. §9.11's delay would do it
        // eventually and would cost thirty ticks of walking to say the same.
        self.rest(node, step);
    }

    /// Lock a resting piece where it stands.
    fn rest(&mut self, node: &Fork, step: u32) {
        let drop = TickInput::action(Action::HardDrop);
        let mut fork = node.clone();
        self.nodes += 1;
        self.events.clear();
        fork.tick(&drop, &mut self.events);
        let tally = self.tally();
        let at = self.trail.push(step, drop);
        self.keep(fork, at, tally);
    }

    /// Fold the events of the tick just played into a fresh tally.
    ///
    /// Fresh because a piece clears nothing before it locks: everything §P5
    /// counts as an event happens on the locking tick or in the delays after
    /// it, so a placement's tally starts where its lock does.
    fn tally(&self) -> Tally {
        let mut tally = Tally::default();
        tally.fold(&self.events);
        tally
    }

    /// Settle a locked fork and keep it, unless the walk already has that
    /// placement by a shorter road (§P4.2).
    fn keep(&mut self, fork: Fork, step: u32, tally: Tally) {
        let inputs = self.trail.sequence(step);
        let placement = placement::settle(fork, inputs, tally, &mut self.events);
        let view = placement.after.view();
        let mut rows = [0u16; ROWS];
        for (row, cells) in view.rows.iter().enumerate() {
            for (col, cell) in cells.iter().enumerate() {
                if cell.is_some() {
                    rows[row] |= 1 << col;
                }
            }
        }
        let key = OutcomeKey {
            rows,
            held: view.hold,
            topped_out: placement.features.outcome.topped_out,
            queue: placement.after.upcoming().len().min(u8::MAX as usize) as u8,
        };
        match self.kept.get(&key) {
            // Breadth first, so the placement already filed under this key was
            // reached in no more ticks than this one — but a descent is several
            // ticks to one edge, so "no more ticks" is not quite "no more
            // inputs", and §P6.4's second tie-break is inputs. Keep the cheaper
            // path to the same board.
            Some(&at) if self.out[at].inputs.len() <= placement.inputs.len() => {}
            Some(&at) => self.out[at] = placement,
            None => {
                self.kept.insert(key, self.out.len());
                self.out.push(placement);
            }
        }
    }
}

/// The ticks a descent will wait for a row (§9.10).
///
/// `soft_drop_factor` divides §9.9's period, and the slowest period in the game
/// is level 1's sixty ticks, so this is the arithmetic rather than a guess —
/// with [`DESCENT_LIMIT`] as the ceiling for a rule that changed under it.
fn descent_limit(rules: &RulesConfig) -> u32 {
    (60 / rules.soft_drop_factor.max(1) + 2).min(DESCENT_LIMIT)
}

/// What makes two positions the same one (§P4.2).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct StateKey {
    pose: Pose,
    held: Option<PieceKind>,
    hold_locked: bool,
    /// How much of the caller's queue is left, which is how far through it the
    /// position is: a hold into an empty slot deals one early, so two positions
    /// with the same board can still be at different points in the queue.
    queue: u8,
}

/// What makes two placements the same one (§P4.2): the board they leave, the
/// hold slot and the queue.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct OutcomeKey {
    /// One bit per cell of the visible field, a row to a `u16`.
    rows: [u16; ROWS],
    held: Option<PieceKind>,
    /// §9.16, which leaves a board that can look like any other: a placement
    /// that ended the game is never the same placement as one that did not.
    topped_out: bool,
    queue: u8,
}

/// Every input sequence in the walk, as one tree.
///
/// A thousand nodes each holding forty inputs is forty thousand of them and a
/// `Vec` clone an edge; a parent pointer per tick is one `u32` and no
/// allocation, and a sequence is wanted only for the two hundred nodes that
/// turn out to be placements.
#[derive(Default)]
struct Trail {
    steps: Vec<Step>,
}

#[derive(Clone, Copy)]
struct Step {
    parent: u32,
    input: TickInput,
}

impl Trail {
    /// The empty sequence: the position the walk started from.
    const ROOT: u32 = u32::MAX;

    fn push(&mut self, parent: u32, input: TickInput) -> u32 {
        self.steps.push(Step { parent, input });
        (self.steps.len() - 1) as u32
    }

    /// The inputs that reach `at`, in the order they are played.
    fn sequence(&self, at: u32) -> Vec<TickInput> {
        let mut inputs = Vec::new();
        let mut at = at;
        while at != Self::ROOT {
            let step = self.steps[at as usize];
            inputs.push(step.input);
            at = step.parent;
        }
        inputs.reverse();
        inputs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Game, PieceKind::*, PlayState, VIEW_WIDTH};
    use crate::pilot::placement;

    /// §6.3's defaults, and the three settings the fixtures below vary.
    ///
    /// By field rather than through `RulesConfig::from_settings`, because §P3.4
    /// lets the planner name `RulesConfig` and neither of the two settings
    /// structs it is built from. The same rule is why every board in here is
    /// **played** rather than set: `core::matrix`'s test fixtures are the
    /// core's, and a test that reached for one would be testing a module
    /// allowed to do something this one is not.
    fn rules(hold_enabled: bool, allow_180_rotation: bool, start_level: u32) -> RulesConfig {
        RulesConfig {
            hold_enabled,
            allow_180_rotation,
            start_level,
            ..RulesConfig::default()
        }
    }

    /// A fork of a fresh game, dealing exactly `queue` and then nothing.
    fn forked(rules: &RulesConfig, seed: u64, queue: &[PieceKind]) -> Fork {
        let game = Game::new(rules.clone(), seed);
        Fork::of(&game, queue)
    }

    /// Hard-drop the piece in play `dx` cells from where it stands, and wait out
    /// §9.12's delays so the next one is up.
    ///
    /// Six either way is past both walls from any spawn, so `-6` and `6` mean
    /// "flush left" and "flush right" whatever the piece is — which is what lets
    /// these fixtures build a board without knowing which piece a seed dealt.
    fn drop_at(fork: &mut Fork, dx: i8) {
        let mut events = Vec::new();
        let shift = if dx < 0 { Shift::Left } else { Shift::Right };
        for _ in 0..dx.unsigned_abs() {
            fork.tick(&TickInput::shift(shift), &mut events);
        }
        fork.tick(&TickInput::action(Action::HardDrop), &mut events);
        for _ in 0..SETTLE {
            if fork.view().current.is_some() || fork.exhausted() {
                break;
            }
            fork.tick(&TickInput::default(), &mut events);
        }
    }

    /// Ticks to wait for §9.12's clear and entry delays in a fixture.
    const SETTLE: u32 = 60;

    /// The visible field a placement leaves behind, as occupancy.
    ///
    /// Occupancy rather than colour, because the two generators reach the same
    /// square by different roads and with different pieces in the hold slot;
    /// what is being compared is where the minos ended up.
    fn board_of(placement: &Placement) -> Vec<Vec<bool>> {
        placement
            .after
            .view()
            .rows
            .iter()
            .map(|row| row.iter().map(|cell| cell.is_some()).collect())
            .collect()
    }

    fn boards(generated: &Generated) -> HashSet<Vec<Vec<bool>>> {
        generated.placements.iter().map(board_of).collect()
    }

    /// §P4.1's answer to the same question, for the comparisons below.
    fn simple(fork: &Fork, rules: &RulesConfig) -> HashSet<Vec<Vec<bool>>> {
        boards(&placement::placements(fork, rules))
    }

    /// The fixture most of these tests stand on: a two-by-two cave, open to the
    /// right and roofed over, which no hard drop can put anything into.
    ///
    /// ```text
    ///     ####......      an I flat on top of an O, both at the left wall
    ///     ##.....#..      -> rows 38 and 39 of columns 2 and 3 are a cave,
    ///     ##.....###         reachable only by sliding in from column 4.
    /// ```
    ///
    /// The live game's own first piece is parked flush right, out of the way and
    /// whatever it happens to be.
    fn cave(rules: &RulesConfig, then: &[PieceKind]) -> Fork {
        let mut queue = vec![O, I];
        queue.extend_from_slice(then);
        let mut fork = forked(rules, 1, &queue);
        drop_at(&mut fork, 6);
        drop_at(&mut fork, -6);
        drop_at(&mut fork, -6);
        fork
    }

    #[test]
    fn every_input_honours_the_cap() {
        // §P3.2, and §P9's C5: at most one action and at most one shift cell a
        // tick. A walk builds its sequences an edge at a time and every edge is
        // one `TickInput::action`, one `TickInput::shift` or a bare soft drop —
        // which is neither an action nor a shift (§9.10) and so costs the cap
        // nothing. This is the test that says the construction held.
        let rules = rules(true, true, 1);
        for seed in 0..6u64 {
            let game = Game::new(rules.clone(), seed);
            let generated = placements(&Fork::of(&game, &game.view().next), &rules);
            assert!(!generated.placements.is_empty());
            for placement in &generated.placements {
                for input in &placement.inputs {
                    assert!(
                        input.actions.iter().count() <= 1,
                        "{input:?} carries more than one action",
                    );
                    assert!(input.shift_cells <= 1, "{input:?} shifts more than a cell");
                    assert!(
                        input.shift.is_none() || input.actions.iter().count() == 0,
                        "{input:?} shifts and acts in one tick",
                    );
                }
            }
        }
    }

    #[test]
    fn every_placement_replays_to_the_board_the_walk_found() {
        // The walk reaches a placement by advancing a fork one tick at a time,
        // and what it reports is the ticks it took. Handing that sequence back
        // to a fresh fork of the same position must arrive at the same board and
        // lock on the same tick, or the sequence is not what reached it — which
        // is §P9's C5, and is also §P3.1's divergence assertion asked of a
        // generator rather than of a plan.
        let rules = rules(true, true, 1);
        let mut events = Vec::new();
        for seed in 0..4u64 {
            let game = Game::new(rules.clone(), seed);
            let start = Fork::of(&game, &game.view().next);
            for placement in placements(&start, &rules).placements {
                let replayed = placement::replay(&start, placement.inputs.clone(), &mut events);
                assert_eq!(
                    board_of(&replayed),
                    board_of(&placement),
                    "seed {seed}: a placement did not replay to itself",
                );
                assert_eq!(
                    replayed.inputs.len(),
                    placement.inputs.len(),
                    "seed {seed}: the replay locked at a different tick",
                );
            }
        }
    }

    #[test]
    fn on_an_open_well_it_reaches_exactly_what_a_hard_drop_does() {
        // §P4.1 is not wrong, it is *incomplete*: while the well is open there
        // is nothing under an overhang because there is no overhang, and the two
        // generators reach the same set. The equality is worth asserting in both
        // directions — a walk that found extra boards on an empty field would
        // have found a board the rules do not allow.
        let rules = rules(true, true, 1);
        for seed in 0..6u64 {
            let game = Game::new(rules.clone(), seed);
            let start = Fork::of(&game, &game.view().next);
            assert_eq!(
                boards(&placements(&start, &rules)),
                simple(&start, &rules),
                "seed {seed}",
            );
        }
    }

    #[test]
    fn it_reaches_both_walls() {
        let rules = rules(true, true, 1);
        for seed in 0..6u64 {
            let game = Game::new(rules.clone(), seed);
            let mut columns = [false; VIEW_WIDTH];
            for placement in placements(&Fork::of(&game, &game.view().next), &rules).placements {
                for row in &placement.after.view().rows {
                    for (col, cell) in row.iter().enumerate() {
                        columns[col] |= cell.is_some();
                    }
                }
            }
            assert!(
                columns.iter().all(|&reached| reached),
                "seed {seed} left a column unreachable: {columns:?}",
            );
        }
    }

    #[test]
    fn it_tucks_a_piece_under_an_overhang() {
        // The whole of §P4.2 in one assertion. An `O` slid left along the floor
        // fills the cave; a hard drop into those columns lands on the roof, two
        // rows higher. This is a placement §P4.1 cannot reach at all, rather
        // than one it reaches by a longer road.
        let rules = rules(false, true, 1);
        let fork = cave(&rules, &[O, O, O, O]);
        assert_eq!(fork.view().current.map(|piece| piece.kind), Some(O));

        let exact = placements(&fork, &rules);
        let simple = simple(&fork, &rules);
        assert!(
            simple.len() < exact.placements.len(),
            "and a good many more"
        );

        let filled = exact.placements.iter().find(|placement| {
            let rows = &placement.after.view().rows;
            // The cave is matrix rows 38 and 39 of columns 2 and 3, which are
            // the last two rows of the visible field.
            (18..20).all(|row| (2..4).all(|col| rows[row][col].is_some()))
        });
        let filled = filled.expect("the cave is reachable");
        assert!(
            !simple.contains(&board_of(filled)),
            "§P4.1 reached the cave, so the fixture proves nothing",
        );
        assert!(
            filled
                .inputs
                .iter()
                .any(|input| input.shift == Some(Shift::Left)),
            "it got there by sliding in: {:?}",
            filled.inputs,
        );
    }

    #[test]
    fn it_rotates_a_t_into_a_place_a_drop_cannot_reach() {
        // §P4.2's spins. The last thing done to the piece before it is put down
        // is a **rotation**, which is what §9.13 asks of a T-spin — and the hard
        // drop that follows carries the flag, which is the rule the spec calls
        // the most commonly botched one.
        let rules = rules(false, true, 1);
        let fork = cave(&rules, &[T, T, T, T]);
        assert_eq!(fork.view().current.map(|piece| piece.kind), Some(T));

        let exact = placements(&fork, &rules);
        let simple = simple(&fork, &rules);
        let spun = exact.placements.iter().filter(|placement| {
            if simple.contains(&board_of(placement)) {
                return false;
            }
            // Every placement ends with the hard drop that puts the piece down,
            // so the *last action applied to the piece* is the one before it.
            let inputs = &placement.inputs;
            inputs.len() >= 2
                && inputs[inputs.len() - 2].actions.iter().any(|action| {
                    matches!(
                        action,
                        Action::RotateCw | Action::RotateCcw | Action::Rotate180
                    )
                })
        });
        assert!(
            spun.count() > 0,
            "no placement was rotated into a spot a drop cannot reach",
        );
    }

    #[test]
    fn a_disabled_mechanic_is_absent_from_the_walk() {
        // §P4.3, and the same assertion §P4.1's generator carries: not
        // "generated and then refused" but never generated, because §10.1 drops
        // the key at the input boundary and the piece would sit there for a tick
        // while the plan believed it had turned.
        let with = rules(true, true, 1);
        let without = rules(false, false, 1);
        let pressed = |generated: &Generated, wanted: Action| {
            generated.placements.iter().any(|placement| {
                placement
                    .inputs
                    .iter()
                    .any(|input| input.actions.iter().any(|action| action == wanted))
            })
        };

        let both = placements(&cave(&with, &[T, T, T, T]), &with);
        assert!(pressed(&both, Action::Hold), "hold is on");
        assert!(pressed(&both, Action::Rotate180), "and so is the half turn");

        let neither = placements(&cave(&without, &[T, T, T, T]), &without);
        assert!(!pressed(&neither, Action::Hold), "hold is off");
        assert!(!pressed(&neither, Action::Rotate180), "and the half turn");
        // South is still reachable, by two quarter turns rather than one half
        // one — a disabled mechanic costs a tick, not an orientation.
        assert!(pressed(&neither, Action::RotateCw));
        assert!(!neither.placements.is_empty());
    }

    #[test]
    fn hold_is_played_once_and_first() {
        // §9.7 both ways round. With the slot empty the swap deals from the
        // queue, so the piece that lands is the queue's first; with the slot
        // full it swaps, so the piece that lands is the one already in it. Both
        // are one press at the head of the sequence and never a second one.
        let rules = rules(true, true, 1);

        // Empty slot: the `O` in play goes away and the queue's `T` arrives.
        let empty = forked(&rules, 1, &[O, T, I, S, Z]);
        let current = empty.view().current.expect("a piece is in play").kind;
        let generated = placements(&empty, &rules);
        let held: Vec<_> = generated
            .placements
            .iter()
            .filter(|placement| {
                placement.inputs[0]
                    .actions
                    .iter()
                    .any(|action| action == Action::Hold)
            })
            .collect();
        assert!(!held.is_empty(), "a hold is one of the things to try");
        for placement in &held {
            assert_eq!(
                placement.after.view().hold,
                Some(current),
                "the piece in play went into the slot",
            );
            assert_eq!(
                placement
                    .inputs
                    .iter()
                    .filter(|input| input.actions.iter().any(|a| a == Action::Hold))
                    .count(),
                1,
                "§9.7 allows one hold a piece, so the walk presses it once",
            );
        }

        // Occupied slot: hold once to fill it, let that piece lock, and the
        // next piece's hold is a swap rather than a deal.
        let mut occupied = forked(&rules, 1, &[O, T, I, S, Z]);
        let mut events = Vec::new();
        occupied.tick(&TickInput::action(Action::Hold), &mut events);
        drop_at(&mut occupied, 6);
        let stored = occupied.view().hold.expect("the slot is full");
        assert_eq!(stored, current);
        let swapped = placements(&occupied, &rules);
        let in_play = occupied.view().current.expect("a piece is in play").kind;
        assert!(swapped.placements.iter().any(|placement| {
            placement.inputs[0]
                .actions
                .iter()
                .any(|action| action == Action::Hold)
                && placement.after.view().hold == Some(in_play)
        }));
    }

    #[test]
    fn it_walks_where_gravity_moves_the_piece_under_it() {
        // §P4.2's "same-tick ordering, which matters once gravity is fast enough
        // to move a piece in the tick an action is delivered". In *this* game
        // the level curve never gets there — §9.9 bottoms out at level 15, which
        // is about eleven ticks to the row — so the way to reach it is §9.10's
        // `soft_drop_factor`, which divides the period and which §6.3 lets a
        // player set to 100. At level 15 that is a row a tick or faster, and it
        // is what the walk's own descent edge holds down.
        let mut fast = rules(true, true, 15);
        fast.soft_drop_factor = 100;
        let fork = cave(&fast, &[T, T, T, T]);

        let generated = placements(&fork, &fast);
        assert!(!generated.placements.is_empty(), "it still finds moves");
        let mut events = Vec::new();
        for placement in &generated.placements {
            let replayed = placement::replay(&fork, placement.inputs.clone(), &mut events);
            assert_eq!(
                board_of(&replayed),
                board_of(placement),
                "a placement did not replay to itself under fast gravity",
            );
        }
        assert!(
            boards(&generated).difference(&simple(&fork, &fast)).count() > 0,
            "and still reaches what a hard drop cannot",
        );
    }

    #[test]
    fn a_top_out_is_an_outcome_and_not_an_error() {
        // §9.16 through §P4.2: "Block Out and Lock Out as outcomes, not as
        // errors". A column of `O`s straight up the middle fills the spawn
        // columns, and the placement that adds one more to it leaves nowhere for
        // the next piece to spawn — §9.16's Block Out, which the walk reports as
        // a feature of that placement rather than declining to generate it.
        //
        // Ten is the fixture rather than a round number: at nine the stack is
        // not yet high enough for any placement to end the game and at eleven
        // the game is already over, so ten is the one position where the walk
        // has to offer both.
        let rules = rules(false, true, 1);
        let mut fork = forked(&rules, 1, &[O; 16]);
        drop_at(&mut fork, 6);
        for _ in 0..10 {
            drop_at(&mut fork, 0);
        }
        assert_eq!(fork.state(), PlayState::Falling, "a piece is still in play");

        let generated = placements(&fork, &rules);
        assert!(!generated.placements.is_empty(), "the walk still answers");
        let topped = generated
            .placements
            .iter()
            .filter(|placement| placement.features.outcome.topped_out)
            .count();
        assert!(
            topped > 0,
            "stacking into the ceiling is one of its options"
        );
        assert!(
            topped < generated.placements.len(),
            "...and is not the only one, or there would be nothing to choose",
        );
    }

    #[test]
    fn it_costs_what_it_says_it_costs() {
        // §P8.2's two counters, and the reason P7 made them two: the walk
        // advances thousands of forks to find scores of placements, and a budget
        // charged the placements would be charged a fraction of the work it
        // paid for.
        let rules = rules(true, true, 1);
        let game = Game::new(rules.clone(), 42);
        let generated = placements(&Fork::of(&game, &game.view().next), &rules);
        assert!(
            generated.nodes > 4 * generated.placements.len() as u64,
            "nodes {} against {} placements",
            generated.nodes,
            generated.placements.len(),
        );
    }

    #[test]
    fn the_walk_is_the_same_walk_twice() {
        // §P3.3 and §P9's C3, asked of the generator. A `HashSet` decides which
        // positions are new and a `HashMap` which placements are kept, and
        // neither is what fixes the order — the frontier is a `Vec` walked from
        // the front, so two runs enumerate the same graph in the same sequence.
        // A generator whose output order drifted would put §P6.4's last
        // tie-break on a different candidate each run.
        let rules = rules(true, true, 1);
        let fork = cave(&rules, &[T, T, T, T]);
        let first = placements(&fork, &rules);
        let second = placements(&fork, &rules);
        assert_eq!(first.nodes, second.nodes);
        assert_eq!(first.placements.len(), second.placements.len());
        for (a, b) in first.placements.iter().zip(&second.placements) {
            assert_eq!(a.inputs, b.inputs, "the walk chose a different road");
            assert_eq!(board_of(a), board_of(b));
        }
    }

    #[test]
    fn a_position_with_nothing_in_play_has_no_placements() {
        // A fork that has run out of the caller's queue is at its horizon, not
        // broken (§P2.3), and the generator says so by finding nothing rather
        // than by asserting.
        let rules = rules(true, true, 1);
        let mut fork = forked(&rules, 1, &[]);
        drop_at(&mut fork, 0);
        assert!(fork.exhausted());
        let generated = placements(&fork, &rules);
        assert!(generated.placements.is_empty());
        assert_eq!(generated.nodes, 0);
    }
}
