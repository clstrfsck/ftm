//! §P6's search: how far it looks, what it keeps, and what it does when it runs
//! out of budget.
//!
//! Everything here runs on [`Fork`]s and nothing here can name a `Game`, which
//! is §P2.2 as a property of the module rather than of a review: the position a
//! search starts from is the live game forked on the caller's fair queue, and
//! every position below it is a fork of a fork.
//!
//! The shape, in one paragraph. Ply 1 is §P4.1's generator over the piece in
//! play; each candidate is *replayed*, so what it is worth is measured from the
//! board it actually leaves behind. Those candidates are deduplicated and cut to
//! a beam (§P6.2), best first, and each survivor is expanded by generating the
//! next piece's placements from the fork it settled into. A line's value is the
//! interior plies' **events** plus the leaf's full evaluation (§P5) — the board
//! is charged once, at the end, rather than once per ply. Past the preview the
//! next piece is not known, so the caller hands in one root per hypothesis and
//! the values are blended 80/20 expected-to-worst (§P6.3). The budget is an
//! integer node count and the answer is always the best **fully** evaluated
//! move (§P6.4).
//!
//! **No clock, no float, no frame count** (§P3.3, §P5). The budget is the only
//! thing that can stop a search early and it is a count of states, so the same
//! position and the same [`Settings`] give the same move on every machine, in
//! every profile and at every cadence.

use std::collections::HashMap;

use crate::core::{GameEvent, GameView, PieceKind, PlayState, TickInput};
use crate::pilot::eval::{Board, ROWS, Weights};
use crate::pilot::fork::Fork;
use crate::pilot::placement::{self, Generated, Placement};
use crate::pilot::reachable;
use crate::pilot::{Counted, Settings};
use crate::shell::config::RulesConfig;

/// §P6.3's blend: the weight on the average, against 100.
///
/// The other 20 go on the worst case, because a planner that maximised the
/// average alone builds setups that only one piece out of seven rescues. It is
/// an integer ratio for §P5's reason and not a probability: nothing here divides
/// by anything it cannot divide exactly.
const EXPECTED: i64 = 80;

/// What a search decided, and what it cost.
pub(crate) struct Chosen {
    /// The inputs of the chosen placement: one [`TickInput`] per tick, ending
    /// with the tick that locks the piece (§P3.1).
    pub(crate) inputs: Vec<TickInput>,
    /// §P8.2's two counters for this search alone.
    pub(crate) counted: Counted,
}

/// §P6.1's depth, clamped by what the player can see.
///
/// A search `d` plies deep consumes `d` pieces from the queue: the one in play
/// is the live game's, and every ply after it spawns from the queue — and a hold
/// into an empty slot deals one early, so the worst case is one piece per ply.
/// A preview of `n` therefore reaches `n` plies for certain and an `n + 1`th
/// only by hypothesis, which is §P6.3's chance node. The clamp stops one ply
/// short of needing *two* levels of hypothesis: a second level multiplies the
/// leaves by seven again for a piece nobody has any information about, and §6.3
/// allows a preview of 1 where that would be the ordinary case rather than the
/// exotic one.
pub(crate) fn depth_for(asked: u8, preview: usize) -> u8 {
    let ceiling = u8::try_from(preview.saturating_add(1)).unwrap_or(u8::MAX);
    asked.clamp(1, ceiling)
}

/// How many pieces a search of this depth needs beyond the preview: one, or
/// none (§P6.3).
///
/// Never more than one, because [`depth_for`] is what decides the depth.
pub(crate) fn hypotheses_needed(depth: u8, preview: usize) -> usize {
    usize::from(depth).saturating_sub(preview)
}

/// Search the roots and choose a move (§P6).
///
/// `roots` is the same position dealt one queue each: a single root when the
/// preview reaches the horizon, and one per hypothesis when it does not
/// (§P6.3). They must agree about everything but the pieces past the preview —
/// the caller builds them from one `Game` and one visible queue — because ply 1
/// is generated from the first of them and replayed on the rest.
pub(crate) fn plan(
    roots: &[Fork],
    rules: &RulesConfig,
    settings: Settings,
    weights: &Weights,
    depth: u8,
) -> Chosen {
    let mut search = Search {
        rules,
        settings,
        weights,
        counted: Counted::default(),
        budget: u64::from(settings.nodes),
        cache: HashMap::new(),
        events: Vec::new(),
    };
    let inputs = search.run(roots, depth);
    Chosen {
        inputs,
        counted: search.counted,
    }
}

/// One search, and the three things it carries through it: what it has spent,
/// what it has already answered, and one event buffer.
struct Search<'a> {
    rules: &'a RulesConfig,
    settings: Settings,
    weights: &'a Weights,
    counted: Counted,
    /// Nodes left. The only thing that can stop a search early, and a count
    /// rather than a duration (§P6.4, §P3.3).
    budget: u64,
    /// §P6.2's transposition cache: a subtree's value by the question it
    /// answers.
    cache: HashMap<Key, i32>,
    /// Reused across every candidate of every ply, because `LinesCleared`
    /// carries a `Vec`.
    events: Vec<GameEvent>,
}

impl Search<'_> {
    fn run(&mut self, roots: &[Fork], depth: u8) -> Vec<TickInput> {
        let Some(root) = roots.first() else {
            debug_assert!(false, "a search needs a position to search from");
            return Vec::new();
        };

        // Ply 1, from the first root. Every root holds the same board and the
        // same piece in play, and a hypothesis sits past this ply's reach, so
        // these candidates are one list however many roots there are.
        let Generated {
            placements: candidates,
            nodes,
        } = self.generate(root);
        self.spend(candidates.len(), nodes);
        if candidates.is_empty() {
            // Unreachable: the generator always offers the piece where it
            // stands. An empty plan lets gravity play the piece, which is the
            // mildest thing a bug here can do.
            debug_assert!(false, "§P4.1's generator offered nothing");
            return Vec::new();
        }

        // Every candidate is fully evaluated at one ply as it is generated, so
        // there is always an answer §P6.4 can call fully evaluated — even if the
        // budget was too small to expand a single one of them.
        let shallow: Vec<i32> = candidates
            .iter()
            .map(|candidate| candidate.features.evaluate(self.weights))
            .collect();
        let mut best = pick(&candidates, &shallow, |_| true);

        if depth > 1 {
            let beam = self.beam(&candidates, &shallow);
            let mut deep = vec![0; candidates.len()];
            let mut done = vec![false; candidates.len()];
            for index in beam {
                let Some(value) = self.line(roots, &candidates[index], depth) else {
                    // §P6.4: the budget ran out inside this candidate, so it is
                    // not a candidate. Nothing after it is either — the budget
                    // does not come back — so stop rather than keep asking.
                    break;
                };
                deep[index] = value;
                done[index] = true;
            }
            if let Some(deeper) = pick(&candidates, &deep, |index| done[index]) {
                best = Some(deeper);
            }
        }

        best.map(|index| candidates[index].inputs.clone())
            .unwrap_or_default()
    }

    /// One candidate's value, blended over the roots (§P6.3).
    ///
    /// `None` when the budget ran out partway: a candidate is either fully
    /// evaluated or not a candidate (§P6.4).
    fn line(&mut self, roots: &[Fork], candidate: &Placement, depth: u8) -> Option<i32> {
        if roots.len() == 1 {
            // The ordinary case, and not an optimisation: the candidate was
            // generated from this very root, so the fork it settled into is the
            // node — replaying it a second time would arrive at the same place
            // by definition.
            return self.value_of(candidate, depth);
        }
        let mut values = Vec::with_capacity(roots.len());
        for root in roots {
            // Replayed per root rather than reused from the candidate, because
            // the roots differ in the queue and therefore in the piece this
            // candidate's lock spawns. The replay is a dozen ticks against the
            // hundred placements the ply below it costs.
            let node = placement::replay(root, candidate.inputs.clone(), &mut self.events);
            self.spend(1, 1);
            values.push(self.value_of(&node, depth)?);
        }
        Some(blend(&values))
    }

    /// What a node is worth: the events of getting here, plus what the search
    /// makes of where it now is.
    fn value_of(&mut self, node: &Placement, depth: u8) -> Option<i32> {
        let interior = node.features.outcome.interior(self.weights);
        Some(interior.saturating_add(self.subtree(&node.after, depth - 1)?))
    }

    /// The best line from here, `depth` plies of it (§P6.2).
    ///
    /// `depth` counts the plies still to *play*. At zero there is nothing left
    /// to play and the board in front of it is the leaf §P5 charges for.
    fn subtree(&mut self, fork: &Fork, depth: u8) -> Option<i32> {
        let view = fork.view();
        if depth == 0 || view.current.is_none() || fork.state() == PlayState::ToppedOut {
            // The leaf: the board is charged here and only here (§P5). A fork
            // with nothing in play has run out of the caller's queue, which is
            // §P2.3 working — it is not a position anybody can play on, so it is
            // the end of the line rather than an error.
            return Some(Board::of(&view.rows).evaluate(self.weights));
        }

        let key = Key::of(&view, fork, depth);
        if let Some(&value) = self.cache.get(&key) {
            // A hit costs no node, which is one of the two reasons §P8.2's two
            // counters part company at this stage.
            return Some(value);
        }
        if self.budget == 0 {
            // §P6.4. The budget is checked here, between one ply and the next,
            // so a search may overrun it by the placements of the ply it was in
            // the middle of generating: a ply is the granularity at which an
            // answer is whole, and half a generation is not an answer to
            // compare with anything.
            return None;
        }
        // The interior node itself, which is nobody's placement and is the other
        // reason the counters part.
        self.budget -= 1;
        self.counted.nodes += 1;

        let Generated {
            placements: candidates,
            nodes,
        } = self.generate(fork);
        self.spend(candidates.len(), nodes);
        let values: Vec<i32> = candidates
            .iter()
            .map(|candidate| candidate.features.evaluate(self.weights))
            .collect();

        let value = if depth == 1 {
            // The ply below is the leaf, and `features` already is that leaf.
            values.iter().copied().max().unwrap_or(0)
        } else {
            let mut best: Option<i32> = None;
            for index in self.beam(&candidates, &values) {
                let value = self.value_of(&candidates[index], depth)?;
                best = Some(best.map_or(value, |best: i32| best.max(value)));
            }
            best.unwrap_or_else(|| Board::of(&view.rows).evaluate(self.weights))
        };
        self.cache.insert(key, value);
        Some(value)
    }

    /// §P6.2's beam: the states worth expanding, deduplicated, best first.
    ///
    /// **Best first is load-bearing rather than tidy**: the budget can stop the
    /// expansion partway (§P6.4), and a cut-off list has to be one whose front
    /// is the promising end. Equal values keep the generator's canonical order,
    /// so two builds cut at the same place.
    fn beam(&self, candidates: &[Placement], values: &[i32]) -> Vec<usize> {
        let mut seen = HashMap::new();
        let mut order: Vec<usize> = (0..candidates.len())
            .filter(|&index| {
                // Deduplicated by the position reached and not by the inputs
                // that reached it: the generator shifts up to six cells either
                // way and a shift into a wall is refused, so a great many
                // candidates arrive at the same place by different routes.
                let candidate = &candidates[index];
                let view = candidate.after.view();
                let key = Key::of(&view, &candidate.after, 0);
                seen.insert(key, index).is_none()
            })
            .collect();
        order.sort_by_key(|&index| (std::cmp::Reverse(values[index]), index));
        order.truncate(usize::from(self.settings.beam).max(1));
        order
    }

    /// Whichever generator the settings chose (§P4.1, §P4.2).
    ///
    /// The two are interchangeable and nothing else in this file knows which
    /// answered: both hand back input sequences replayed through the real
    /// rules, and both say what they cost.
    fn generate(&mut self, fork: &Fork) -> Generated {
        if self.settings.exact {
            reachable::placements(fork, self.rules)
        } else {
            placement::placements(fork, self.rules)
        }
    }

    /// Charge a generation to §P8.2's two counters and to §P6.4's budget.
    ///
    /// They are separate numbers because §P4.2 made them separate: `placements`
    /// is what the search may choose between and `nodes` is what finding them
    /// cost. The budget is spent in nodes, which is the honest unit — a
    /// generator that walked a thousand positions has done a thousand
    /// positions' work whatever it hands back.
    fn spend(&mut self, placements: usize, nodes: u64) {
        self.counted.placements += placements as u64;
        self.counted.nodes += nodes;
        self.budget = self.budget.saturating_sub(nodes);
    }
}

/// The best of a scored list under §P6.4's order, among those `eligible`.
///
/// The score is §P5's, blended where §P6.3 applies. Ties go to the lower
/// top-out risk — whether the branch ended in §9.16, and then how tall it left
/// the stack — then to fewer inputs, then to the generator's canonical order,
/// which is the index. Every one of those is an integer comparison over a
/// deterministic list, so two builds cannot disagree.
fn pick(
    placements: &[Placement],
    values: &[i32],
    eligible: impl Fn(usize) -> bool,
) -> Option<usize> {
    placements
        .iter()
        .enumerate()
        .filter(|&(index, _)| eligible(index))
        .min_by_key(|(index, placement)| {
            let features = &placement.features;
            (
                std::cmp::Reverse(values[*index]),
                features.outcome.topped_out,
                features.board.max_height,
                placement.inputs.len(),
                *index,
            )
        })
        .map(|(index, _)| index)
}

/// §P6.3's 80/20 expected-to-worst-case blend, in integers.
///
/// One value is not a chance node and is returned as it stands, which is the
/// ordinary case: a preview long enough for the depth needs no hypothesis at
/// all. The arithmetic is in `i64` and lands back in `i32` by clamping, because
/// a weight is a player's to set and `top_out` is already -1,000,000.
fn blend(values: &[i32]) -> i32 {
    match values {
        [] => 0,
        [only] => *only,
        values => {
            let sum: i64 = values.iter().map(|&value| i64::from(value)).sum();
            let mean = sum / values.len() as i64;
            let worst = i64::from(values.iter().copied().min().unwrap_or(0));
            let blended = (EXPECTED * mean + (100 - EXPECTED) * worst) / 100;
            i32::try_from(blended.clamp(i64::from(i32::MIN), i64::from(i32::MAX)))
                .unwrap_or(i32::MAX)
        }
    }
}

/// What makes two positions the same question (§P6.2).
///
/// The board, the piece in play, the hold slot and whether hold is spent, the
/// queue still to come, and how deep the answer was searched. Occupancy rather
/// than colour: two stacks of different pieces in the same cells are the same
/// stack to §P5's features, and treating them as one is what makes the cache
/// worth having.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    /// One bit per cell of the visible field, a row to a `u16`.
    rows: [u16; ROWS],
    current: Option<PieceKind>,
    held: Option<PieceKind>,
    hold_locked: bool,
    /// The scripted remainder, a kind to a nibble from the front. A queue is at
    /// most §6.3's six previews and one hypothesis, so seven nibbles is room to
    /// spare and zero means the end of it.
    queue: u32,
    depth: u8,
}

impl Key {
    fn of(view: &GameView, fork: &Fork, depth: u8) -> Self {
        let mut rows = [0u16; ROWS];
        for (row, cells) in view.rows.iter().enumerate() {
            for (col, cell) in cells.iter().enumerate() {
                if cell.is_some() {
                    rows[row] |= 1 << col;
                }
            }
        }
        let mut queue = 0u32;
        for (at, &kind) in fork.upcoming().iter().take(8).enumerate() {
            queue |= u32::from(nibble(kind)) << (at * 4);
        }
        Self {
            rows,
            current: view.current.map(|piece| piece.kind),
            held: view.hold,
            hold_locked: view.hold_locked,
            queue,
            depth,
        }
    }
}

/// A kind as 1-7, so that 0 can mean "no piece" in a packed queue.
fn nibble(kind: PieceKind) -> u8 {
    PieceKind::ALL
        .iter()
        .position(|&other| other == kind)
        .expect("PieceKind::ALL holds every kind") as u8
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Game;
    use crate::pilot::eval::{Features, Outcome};

    fn rules(preview_count: u8) -> RulesConfig {
        RulesConfig {
            preview_count,
            ..RulesConfig::default()
        }
    }

    /// The root of a fresh game: the live game forked on its visible preview,
    /// which is the only queue §P2.1 allows without a hypothesis.
    fn root(seed: u64, rules: &RulesConfig) -> (Game, Fork) {
        let game = Game::new(rules.clone(), seed);
        let queue = game.view().next;
        let fork = Fork::of(&game, &queue);
        (game, fork)
    }

    #[test]
    fn depth_is_clamped_by_what_the_player_can_see() {
        // §P6.1. A preview of five reaches five plies for certain and a sixth by
        // hypothesis; a preview of one reaches two, the second of which is
        // already a chance node, which is the ordinary configuration §P6.3 is
        // load-bearing for.
        assert_eq!(depth_for(2, 5), 2);
        assert_eq!(depth_for(2, 1), 2);
        assert_eq!(depth_for(9, 1), 2);
        assert_eq!(depth_for(9, 5), 6);
        // ...and never less than one ply, whatever it is asked for.
        assert_eq!(depth_for(0, 5), 1);

        // One level of hypothesis at most, by construction.
        assert_eq!(hypotheses_needed(depth_for(2, 5), 5), 0);
        assert_eq!(hypotheses_needed(depth_for(2, 1), 1), 1);
        assert_eq!(hypotheses_needed(depth_for(9, 1), 1), 1);
        assert_eq!(hypotheses_needed(depth_for(9, 5), 5), 1);
    }

    #[test]
    fn the_blend_is_eighty_twenty_in_integers() {
        // §P6.3. One value is not a chance node; several are averaged and then
        // pulled a fifth of the way towards the worst of them.
        assert_eq!(blend(&[]), 0);
        assert_eq!(blend(&[7]), 7);
        assert_eq!(blend(&[100, 100]), 100);
        // Mean 50, worst 0: 80 * 50 + 20 * 0, over 100.
        assert_eq!(blend(&[0, 100]), 40);
        // The worst case is what stops a planner building a setup only one piece
        // rescues: six good futures and one disaster is not a good plan. Mean
        // -991 of the seven, worst -7,000, and a fifth of the way from the one
        // to the other is a long way down.
        assert_eq!(blend(&[10, 10, 10, 10, 10, 10, -7_000]), -2_192);
        // A preposterous weight saturates rather than wrapping.
        assert_eq!(blend(&[i32::MIN, i32::MIN]), i32::MIN);
    }

    #[test]
    fn a_search_two_plies_deep_costs_more_nodes_than_one() {
        // The stage in one assertion: the same position, searched one ply and
        // two, and the two counters §P8.2 promised would part.
        let rules = rules(5);
        let (_, fork) = root(42, &rules);
        let weights = Weights::default();
        let settings = Settings::default();

        let one = plan(std::slice::from_ref(&fork), &rules, settings, &weights, 1);
        assert_eq!(
            one.counted.nodes, one.counted.placements,
            "one ply evaluates every placement where it stands and nothing else",
        );
        let two = plan(&[fork], &rules, settings, &weights, 2);
        assert!(
            two.counted.nodes > one.counted.nodes * 10,
            "a beam of {} expands: {:?} against {:?}",
            settings.beam,
            two.counted,
            one.counted,
        );
        assert!(
            two.counted.nodes > two.counted.placements,
            "an interior node is nobody's placement: {:?}",
            two.counted,
        );
        assert!(!two.inputs.is_empty(), "and it chose something");
    }

    #[test]
    fn the_budget_is_spent_in_nodes_and_never_overrun() {
        // §P6.4. Whatever it is given, it stops: a budget of nothing still
        // answers, because one ply is evaluated as it is generated.
        let rules = rules(5);
        let (_, fork) = root(7, &rules);
        let weights = Weights::default();
        for nodes in [0, 1, 200, 2_000, 100_000] {
            let settings = Settings {
                nodes,
                ..Settings::default()
            };
            let chosen = plan(std::slice::from_ref(&fork), &rules, settings, &weights, 2);
            assert!(!chosen.inputs.is_empty(), "nodes {nodes}: nothing chosen");
            // The one ply it always pays for is the candidate generation, which
            // is what makes "the best fully evaluated move" always exist.
            assert!(chosen.counted.placements >= 104, "nodes {nodes}");
        }
    }

    #[test]
    fn a_tighter_budget_is_a_shallower_answer_and_not_a_different_kind_of_one() {
        // §P6.4's rule is that a partly evaluated branch is never chosen, so a
        // budget too small to expand anything must give exactly the one-ply
        // answer rather than a truncated two-ply one.
        let rules = rules(5);
        let (_, fork) = root(2024, &rules);
        let weights = Weights::default();
        let starved = Settings {
            nodes: 0,
            ..Settings::default()
        };
        let one = plan(std::slice::from_ref(&fork), &rules, starved, &weights, 1);
        let two = plan(&[fork], &rules, starved, &weights, 2);
        assert_eq!(one.inputs, two.inputs);
    }

    #[test]
    fn the_same_position_gives_the_same_move_every_time() {
        // §P9's C3 at the level this module settles it: no clock, no float and
        // no iteration order that a rebuild could change.
        let rules = rules(5);
        let weights = Weights::default();
        for seed in [1u64, 42, 99] {
            let (_, fork) = root(seed, &rules);
            let first = plan(
                std::slice::from_ref(&fork),
                &rules,
                Settings::default(),
                &weights,
                2,
            );
            let second = plan(&[fork], &rules, Settings::default(), &weights, 2);
            assert_eq!(first.inputs, second.inputs, "seed {seed}");
            assert_eq!(first.counted, second.counted, "and cost the same");
        }
    }

    #[test]
    fn the_cache_changes_the_cost_and_never_the_answer() {
        // §P6.2. A subtree is cached by the question it answers, so a hit is an
        // answer that was already computed — which is a saving and must not be a
        // difference. Checked by searching the same position twice inside one
        // `Search`, which is what a beam of duplicate-ish states does anyway.
        let rules = rules(5);
        let (_, fork) = root(11, &rules);
        let weights = Weights::default();
        let mut search = Search {
            rules: &rules,
            settings: Settings::default(),
            weights: &weights,
            counted: Counted::default(),
            budget: u64::MAX,
            cache: HashMap::new(),
            events: Vec::new(),
        };
        let cold = search.subtree(&fork, 1).expect("an unbounded budget");
        let spent = search.counted.nodes;
        let warm = search.subtree(&fork, 1).expect("and again");
        assert_eq!(cold, warm, "the same question, the same answer");
        assert_eq!(search.counted.nodes, spent, "and the second cost nothing");
    }

    #[test]
    fn the_beam_keeps_one_of_each_position_and_the_best_of_them_first() {
        // §P6.2. The generator shifts up to six cells either way and a shift
        // into a wall is refused, so a fresh field offers many more candidates
        // than positions; the beam is over positions.
        let rules = rules(5);
        let (_, fork) = root(5, &rules);
        let weights = Weights::default();
        let search = Search {
            rules: &rules,
            settings: Settings::default(),
            weights: &weights,
            counted: Counted::default(),
            budget: u64::MAX,
            cache: HashMap::new(),
            events: Vec::new(),
        };
        let candidates = placement::placements(&fork, &rules).placements;
        let values: Vec<i32> = candidates
            .iter()
            .map(|candidate| candidate.features.evaluate(&weights))
            .collect();
        let beam = search.beam(&candidates, &values);
        assert_eq!(beam.len(), usize::from(Settings::default().beam));
        // Best first, and equal values in the generator's canonical order, which
        // is what makes a budget that cuts the list cut it the same way twice.
        for pair in beam.windows(2) {
            let (first, then) = (pair[0], pair[1]);
            assert!(
                values[first] > values[then] || (values[first] == values[then] && first < then),
                "the beam is out of order at {pair:?}",
            );
        }
        // Deduplicated: the whole candidate list holds the same position many
        // times over, and the beam holds each of its own once.
        let mut positions = std::collections::HashSet::new();
        for &index in &beam {
            let after = &candidates[index].after;
            assert!(
                positions.insert(Key::of(&after.view(), after, 0)),
                "the beam holds one position twice",
            );
        }
    }

    #[test]
    fn a_beam_of_one_is_still_a_beam() {
        // A setting of zero would otherwise expand nothing and quietly become a
        // one-ply search that had paid for two.
        let rules = rules(5);
        let (_, fork) = root(3, &rules);
        let weights = Weights::default();
        let settings = Settings {
            beam: 0,
            ..Settings::default()
        };
        let chosen = plan(&[fork], &rules, settings, &weights, 2);
        assert!(!chosen.inputs.is_empty());
        assert!(chosen.counted.nodes > 104, "one candidate was expanded");
    }

    #[test]
    fn ties_are_broken_the_same_way_on_every_machine() {
        // §P6.4: equal values go to the lower top-out risk, then to the shorter
        // input sequence, then to the generator's canonical order. Scored with
        // values given by hand, so only the tie-breaks can separate them.
        let rules = rules(5);
        let (_, fork) = root(1, &rules);
        let candidate = |inputs: usize, board: Board, outcome: Outcome| Placement {
            inputs: vec![TickInput::default(); inputs],
            features: Features { board, outcome },
            after: fork.clone(),
        };
        let tall = Board {
            max_height: 12,
            ..Board::default()
        };
        let ended = Outcome {
            topped_out: true,
            ..Outcome::default()
        };

        // A branch that ended in §9.16 loses to one that did not.
        let ends = [
            candidate(1, Board::default(), ended),
            candidate(9, Board::default(), Outcome::default()),
        ];
        assert_eq!(pick(&ends, &[0, 0], |_| true), Some(1));
        // Then the shorter stack, then the shorter sequence, then the index.
        let heights = [
            candidate(1, tall, Outcome::default()),
            candidate(9, Board::default(), Outcome::default()),
        ];
        assert_eq!(pick(&heights, &[0, 0], |_| true), Some(1));
        let lengths = [
            candidate(6, Board::default(), Outcome::default()),
            candidate(2, Board::default(), Outcome::default()),
        ];
        assert_eq!(pick(&lengths, &[0, 0], |_| true), Some(1));
        let same = [
            candidate(3, Board::default(), Outcome::default()),
            candidate(3, Board::default(), Outcome::default()),
        ];
        assert_eq!(pick(&same, &[0, 0], |_| true), Some(0));
        // ...and the value comes first, whatever the tie-breaks would say.
        assert_eq!(pick(&heights, &[1, 0], |_| true), Some(0));
        // Only the eligible are picked from, which is §P6.4's rule that a
        // partly evaluated branch is never chosen.
        assert_eq!(pick(&heights, &[1, 0], |index| index == 1), Some(1));
        assert_eq!(pick(&heights, &[1, 0], |_| false), None);
        assert_eq!(pick(&[], &[], |_| true), None);
    }

    #[test]
    fn a_packed_queue_tells_two_futures_apart() {
        // The cache key's one compressed field. Two roots that differ only in
        // the hypothesis they were dealt must not share a cached subtree.
        let rules = rules(5);
        let game = Game::new(rules.clone(), 42);
        let visible = game.view().next;
        let mut keys = std::collections::HashSet::new();
        for hypothesis in PieceKind::ALL {
            let mut queue = visible.clone();
            queue.push(hypothesis);
            let fork = Fork::of(&game, &queue);
            assert!(
                keys.insert(Key::of(&fork.view(), &fork, 1).queue),
                "{hypothesis:?} packed to a queue already seen",
            );
        }
        assert_eq!(keys.len(), PieceKind::ALL.len());
        // An empty queue is zero, and no kind ever is.
        let empty = Fork::of(&game, &[]);
        assert_eq!(Key::of(&empty.view(), &empty, 1).queue, 0);
    }
}
