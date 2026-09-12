//! §P3's controller: what the planner decides, and when it decides it.
//!
//! [`Pilot`] is the whole of the automated player seen from outside. It is fed
//! a tick's events and asked for a tick's input, and it holds no clock, no
//! frame count and no wall-time budget (§P3.3) — so a PILOT round plays the
//! same game at 60 Hz, at 144 Hz and at a jittery cadence, which is §19.4's
//! property one layer up.
//!
//! **The order it is called in matters**, and it is the order §15.2 already
//! runs: [`Pilot::input`] for a tick, then `Game::tick`, then
//! [`Pilot::observe`] of what that tick raised. A tick's input is decided
//! before the tick happens; its events exist only afterwards.
//!
//! **It thinks once per piece** (§P3.1). The tick a piece spawns produces a
//! plan — one [`TickInput`] per tick, until the piece locks — and every tick
//! after it pops one input from that plan. Nothing else re-plans: a hold is
//! part of a plan rather than a reason to make a new one, and a plan is never
//! recomputed from a partly executed state, so the live game replays exactly
//! what the search simulated. A plan that diverges from what the search
//! predicted is a **bug**, and is caught by a debug assertion as it is
//! consumed.
//!
//! What it does *not* do yet is look ahead. P3's choice is one ply: every
//! placement §P4.1's generator can reach, scored by §P5's evaluation of the
//! board it leaves behind, with §P6.4's tie-breaks. The beam, the chance nodes
//! and the node budget are P6's, and [`Settings`] carries their numbers from
//! here so the signature they arrive into is the one already written down.

use crate::core::{Game, GameEvent, GameView, PlayState, TickInput};
use crate::pilot::eval::Weights;
use crate::pilot::knowledge::Knowledge;
use crate::pilot::placement::{self, Placement, Prediction};
use crate::shell::config::RulesConfig;

/// How hard the planner is asked to think (§P3.4).
///
/// P3 reads **none of these**: its search is one ply over §P4.1's placements,
/// which has no horizon to clamp, no states to prune and no budget to run out
/// of. They are here rather than in P6 because `Pilot::new`'s signature is
/// §P3.4's and a settings-shaped argument that appears two stages later is a
/// signature that changes under its callers. P6 is where they start meaning
/// something.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Plies, clamped by `preview_count` (§P6.1).
    pub depth: u8,
    /// States kept per ply (§P6.2).
    pub beam: u16,
    /// The budget, an integer count and never a duration (§P6.4).
    pub nodes: u32,
}

impl Default for Settings {
    /// §P6.1's two plies, and provisional numbers for the other two: P5
    /// measures the budget that fits a frame and P6 is where it is written
    /// down.
    fn default() -> Self {
        Self {
            depth: 2,
            beam: 16,
            nodes: 100_000,
        }
    }
}

/// The automated player (`PILOT.md`).
///
/// It never holds a `Game` (§P2.2): one is *lent* to it for the length of an
/// [`Pilot::input`] call, and what it does with it is fork it (§P2.3) and read
/// its view. Everything it remembers between calls is fair — what the events
/// said, and a plan it made from them.
pub struct Pilot {
    rules: RulesConfig,
    settings: Settings,
    weights: Weights,
    /// §P2.4's bag arithmetic. Built on the first [`Pilot::input`] rather than
    /// in [`Pilot::new`], because the first piece of a game is the one deal no
    /// event stream mentions and the only place it can be read is the view.
    knowledge: Option<Knowledge>,
    plan: Plan,
}

impl Pilot {
    /// A planner for a game that is about to start.
    ///
    /// `rules` are the game's own (§P1): a PILOT game resolves its rules from
    /// the config like any other, and the planner is told them so it can leave
    /// a disabled mechanic out of the search entirely (§P4.3).
    pub fn new(rules: &RulesConfig, settings: Settings) -> Self {
        Self {
            rules: rules.clone(),
            settings,
            weights: Weights::default(),
            knowledge: None,
            plan: Plan::spent(),
        }
    }

    /// The settings it was made with (§P3.4).
    ///
    /// §P8.1's report header prints what the run resolved, and this is where
    /// half of it comes from — the other half being the `RulesConfig` the
    /// benchmark pins itself.
    pub fn settings(&self) -> Settings {
        self.settings
    }

    /// Fold in one tick's events: deals, holds and locks (§P2.4).
    ///
    /// Cheap and unconditional — the common tick raises nothing — and it must
    /// be called for **every** tick of the game, or the bag arithmetic is a
    /// piece out for the rest of the round.
    pub fn observe(&mut self, events: &[GameEvent]) {
        if let Some(knowledge) = self.knowledge.as_mut() {
            knowledge.observe(events);
        }
    }

    /// The input for this tick, planning first if a piece has just spawned.
    ///
    /// The plan is made on the first tick at which a piece is in play and no
    /// plan is running, which is the tick after the one that spawned it: its
    /// first input is delivered then, and its last is the one that locks the
    /// piece. Between a lock and the next spawn — §9.12's clear and entry
    /// delays — there is nothing to decide and nothing is pressed.
    pub fn input(&mut self, game: &Game) -> TickInput {
        let view = game.view();
        self.verify(&view);
        self.start(&view);
        if self.plan.is_spent() && view.current.is_some() {
            self.plan = self.think(game, &view);
        }
        self.plan.emit().unwrap_or_default()
    }

    /// Plan the piece in play: every placement §P4.1 can reach, scored by §P5's
    /// evaluation of the board it leaves behind.
    ///
    /// The queue the forks are dealt is `view.next` and nothing else — exactly
    /// `preview_count` pieces, which is what the player can see (§P2.1). One
    /// ply needs no hypothesis beyond it; §P6.3's chance nodes are where
    /// [`Knowledge::hypotheses`] is read.
    fn think(&self, game: &Game, view: &GameView) -> Plan {
        let placements = placement::placements(game, &view.next, &self.rules);
        let Some(chosen) = choose(&placements, &self.weights) else {
            // Unreachable: the generator always offers the piece where it
            // stands. Falling back to an empty plan rather than panicking means
            // the worst a future change here can do is let gravity play the
            // piece.
            debug_assert!(false, "§P4.1's generator offered nothing");
            return Plan::spent();
        };
        let inputs = placements[chosen].inputs.clone();
        // §P3.1's divergence check, and the only thing in this file that costs
        // anything it does not have to: one more replay of the chosen sequence,
        // recording what it predicts tick by tick. `PILOT-PLAN.md` has it
        // debug-shaped for exactly that reason, and a release build records
        // nothing and compares nothing.
        let predicted = if cfg!(debug_assertions) {
            placement::predict(game, &view.next, &inputs)
        } else {
            Vec::new()
        };
        Plan {
            inputs,
            predicted,
            pending: None,
            at: 0,
        }
    }

    /// Start the bag tracker from the piece already in play (§P2.4).
    ///
    /// `Game::new` spawns the first piece during construction and discards its
    /// `PieceSpawned`, so there is no event to start from and the view is the
    /// only place it can be read. That is also why a `Pilot` belongs to a game
    /// from its first tick: one adopted mid-round would count a bag it had not
    /// watched being dealt.
    fn start(&mut self, view: &GameView) {
        if self.knowledge.is_some() {
            return;
        }
        if let Some(piece) = view.current {
            debug_assert_eq!(
                view.ticks, 0,
                "a Pilot takes a game from its first tick (§P2.4)",
            );
            self.knowledge = Some(Knowledge::new(piece.kind));
        }
    }

    /// §P3.1: what the plan predicted, against what actually happened.
    ///
    /// A divergence is a bug rather than a state to resync from, so this is an
    /// assertion and not a re-plan. Everything is compared but the preview
    /// queue: a fork's is the scripted remainder the caller gave it and the
    /// live game's is refilled from §9.6's bag, and that difference is §P2.3
    /// working rather than failing.
    fn verify(&mut self, view: &GameView) {
        let Some(prediction) = self.plan.pending.take() else {
            return;
        };
        let reduce = if prediction.whole {
            comparable
        } else {
            settled
        };
        debug_assert_eq!(
            reduce(view.clone()),
            reduce(prediction.view),
            "§P3.1: the plan diverged from what the search predicted",
        );
    }
}

/// A `GameView` without the one field a fork never matches: its preview is the
/// scripted remainder it was given and the live game's is refilled from §9.6's
/// bag.
fn comparable(mut view: GameView) -> GameView {
    view.next = Vec::new();
    view
}

/// The same, without the piece in play either.
///
/// For the tick that locked the piece *and* used up the fork's queue with it
/// (a hold at a preview of 1 does exactly that): what spawned next is past the
/// horizon the caller gave the fork, so the search never claimed anything about
/// it. A plan is a claim about the piece it controls, and the board, the
/// figures and the hold slot are still every bit of it.
fn settled(view: GameView) -> GameView {
    GameView {
        current: None,
        ghost: None,
        fall_progress: 0,
        state: PlayState::default(),
        ..comparable(view)
    }
}

/// One piece's worth of decision: an input per tick, and what each is predicted
/// to leave behind.
struct Plan {
    /// §P3.2's cap holds for every one of these, by construction in
    /// [`placement`].
    inputs: Vec<TickInput>,
    /// The view each input is predicted to produce. Empty in a release build,
    /// where nothing is compared.
    predicted: Vec<Prediction>,
    /// What the game should look like at the next call, given the input just
    /// emitted. Taken by [`Pilot::verify`], so a plan that has run out cannot
    /// go on being compared against a game that has moved on.
    pending: Option<Prediction>,
    /// How many inputs have been emitted.
    at: usize,
}

impl Plan {
    /// A plan with nothing left in it, which is what a piece nobody has thought
    /// about yet has.
    fn spent() -> Self {
        Self {
            inputs: Vec::new(),
            predicted: Vec::new(),
            pending: None,
            at: 0,
        }
    }

    /// Whether every input has been emitted. A piece is thought about when this
    /// is true and one is in play.
    fn is_spent(&self) -> bool {
        self.at >= self.inputs.len()
    }

    /// The next input, and the prediction that goes with it.
    fn emit(&mut self) -> Option<TickInput> {
        let input = *self.inputs.get(self.at)?;
        self.pending = self.predicted.get(self.at).cloned();
        self.at += 1;
        Some(input)
    }
}

/// §P6.4's choice: the best placement, and the same one on every machine.
///
/// The score is §P5's weighted sum. Ties are broken by lower top-out risk —
/// whether the branch ended in §9.16, and then how tall it left the stack —
/// then by fewer inputs, then by the generator's canonical order, which is the
/// index. Every one of those is an integer comparison over a deterministic
/// list, so two builds cannot disagree.
fn choose(placements: &[Placement], weights: &Weights) -> Option<usize> {
    placements
        .iter()
        .enumerate()
        .min_by_key(|(index, placement)| {
            let features = &placement.features;
            (
                std::cmp::Reverse(features.evaluate(weights)),
                features.outcome.topped_out,
                features.board.max_height,
                placement.inputs.len(),
                *index,
            )
        })
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Action, PlayState};
    use crate::pilot::eval::{Board, Features, Outcome};

    /// The defaults with §6.3's three gameplay settings changed.
    ///
    /// By field rather than through `RulesConfig::from_settings`: §P3.4 lets
    /// the planner name `RulesConfig` and nothing else in the shell, and its
    /// tests are held to the same list.
    fn rules(preview_count: u8, hold_enabled: bool, allow_180_rotation: bool) -> RulesConfig {
        RulesConfig {
            preview_count,
            hold_enabled,
            allow_180_rotation,
            ..RulesConfig::default()
        }
    }

    /// What a round looks like from outside: input, tick, observe — §15.2's
    /// order, with no screen and no clock. Stops at a top out or at `pieces`.
    ///
    /// This is the loop `App::advance` will run in P4, written out here because
    /// the planner must be drivable without a front-end at all (§P3.3).
    fn watch(rules: &RulesConfig, seed: u64, pieces: u32) -> Game {
        let mut game = Game::new(rules.clone(), seed);
        let mut pilot = Pilot::new(rules, Settings::default());
        let mut events = Vec::new();
        while game.view().pieces < pieces && game.state() != PlayState::ToppedOut {
            let input = pilot.input(&game);
            events.clear();
            game.tick(&input, &mut events);
            pilot.observe(&events);
        }
        game
    }

    #[test]
    fn it_places_a_piece_and_then_another() {
        // The whole of P3 in one assertion: a planner that thinks once per
        // piece, emits one input per tick and locks what it planned.
        let rules = rules(5, true, true);
        let game = watch(&rules, 42, 10);
        assert_eq!(game.view().pieces, 10);
        assert_ne!(game.state(), PlayState::ToppedOut);
    }

    #[test]
    fn every_input_it_emits_honours_the_cap() {
        // §P3.2, §P9's C5, at the boundary the game actually sees: whatever the
        // generator produced, what leaves the controller is at most one action
        // and at most one shift cell.
        let rules = rules(5, true, true);
        let mut game = Game::new(rules.clone(), 5);
        let mut pilot = Pilot::new(&rules, Settings::default());
        let mut events = Vec::new();
        for _ in 0..2_000 {
            let input = pilot.input(&game);
            assert!(input.actions.iter().count() <= 1, "{input:?}");
            assert!(input.shift_cells <= 1, "{input:?}");
            events.clear();
            game.tick(&input, &mut events);
            pilot.observe(&events);
            if game.state() == PlayState::ToppedOut {
                break;
            }
        }
    }

    #[test]
    fn it_thinks_once_per_piece_and_not_once_per_tick() {
        // §P3.1. A hold inside a plan raises `PieceSpawned` — the piece coming
        // out of the slot, or the one dealt into an empty one — and a planner
        // that re-planned on every spawn would tear up a plan it was halfway
        // through. Counted from outside: the inputs between one lock and the
        // next are the plan, and a re-plan would show up as a second hold or a
        // second hard drop inside one piece.
        let rules = rules(5, true, true);
        let mut game = Game::new(rules.clone(), 2024);
        let mut pilot = Pilot::new(&rules, Settings::default());
        let mut events = Vec::new();
        let (mut drops, mut holds) = (0, 0);
        let mut pieces = 0;
        while pieces < 40 && game.state() != PlayState::ToppedOut {
            let input = pilot.input(&game);
            for action in input.actions.iter() {
                match action {
                    Action::HardDrop => drops += 1,
                    Action::Hold => holds += 1,
                    _ => {}
                }
            }
            events.clear();
            game.tick(&input, &mut events);
            pilot.observe(&events);
            pieces = game.view().pieces;
        }
        assert_eq!(drops, pieces, "one hard drop per piece locked");
        assert!(holds <= pieces, "and at most one hold per piece: {holds}");
    }

    #[test]
    fn it_clears_lines_over_a_long_game() {
        // Not a quality bar — P3 is expected to play badly (`PILOT-PLAN.md`
        // P4) — but a bar the machinery has to clear to be doing anything at
        // all. It is also the one test that would fail if the features were
        // measured during §9.12's clear delay: a candidate that completed a row
        // would be scored with the row still on the board, so clearing would
        // look like the worst thing a piece could do and the planner would
        // never choose it.
        let rules = rules(5, true, true);
        let game = watch(&rules, 7, 120);
        let view = game.view();
        assert!(view.lines >= 4, "only {} lines in 120 pieces", view.lines);
        assert_ne!(game.state(), PlayState::ToppedOut);
    }

    #[test]
    fn the_same_seed_and_settings_play_the_same_game() {
        // §P9's C3, at the level P3 can assert it: the planner is a pure
        // function of the fair state and its settings, so nothing in it can
        // differ between two runs in one process either.
        let rules = rules(5, true, true);
        for seed in [1u64, 42, 2024] {
            let first = watch(&rules, seed, 30);
            let second = watch(&rules, seed, 30);
            assert_eq!(first.view(), second.view(), "seed {seed}");
            assert_eq!(first.debug(), second.debug(), "the bag included");
        }
    }

    #[test]
    fn it_plays_under_every_rule_it_is_given() {
        // §P9's C6, as far as one ply reaches it: §6.3's hold and 180 rotation
        // each on and off, and a preview of 1 and of 6. A planner that assumed
        // a mechanic it had not been given would plan a move the game refuses,
        // and the piece would sit there until gravity dealt with it.
        for preview_count in [1u8, 6] {
            for hold_enabled in [true, false] {
                for allow_180 in [true, false] {
                    let rules = rules(preview_count, hold_enabled, allow_180);
                    let game = watch(&rules, 11, 30);
                    let view = game.view();
                    assert_eq!(
                        view.pieces, 30,
                        "preview {preview_count}, hold {hold_enabled}, 180 {allow_180}",
                    );
                    assert_eq!(view.next.len(), usize::from(preview_count));
                    if !hold_enabled {
                        assert_eq!(view.hold, None, "nothing was ever held");
                    }
                }
            }
        }
    }

    #[test]
    fn the_plan_is_the_search_replayed() {
        // §P3.1's divergence assertion, from the outside: the game the plan is
        // consumed by must end up exactly where the search said it would. In a
        // debug build `Pilot::verify` asserts it tick by tick, which is what
        // this test is really exercising — every call to `input` above checks
        // the tick before it.
        let rules = rules(5, true, true);
        let game = watch(&rules, 99, 60);
        assert_eq!(game.view().pieces, 60);
    }

    #[test]
    fn it_takes_the_settings_it_was_given() {
        // P3 reads none of them; P6 does. Holding them is the point, so that
        // the signature the benchmark and the front-ends are written against is
        // the one §P3.4 already specifies.
        let settings = Settings {
            depth: 3,
            beam: 8,
            nodes: 1_234,
        };
        let pilot = Pilot::new(&RulesConfig::default(), settings);
        assert_eq!(pilot.settings(), settings);
        assert_eq!(Settings::default().depth, 2, "§P6.1's two plies");
    }

    /// A candidate with no inputs worth reading, described by its features
    /// alone: what is under test in the two below is the ordering, not the
    /// generator.
    fn candidate(inputs: usize, board: Board, outcome: Outcome) -> Placement {
        Placement {
            inputs: vec![TickInput::default(); inputs],
            features: Features { board, outcome },
        }
    }

    #[test]
    fn a_planner_prefers_the_board_it_would_rather_be_left_with() {
        // §P6.4's choice, on the one comparison the starting weights must get
        // right (§P5, and `eval`'s own test of it): given two placements that
        // differ only in a hole, the one without it wins.
        let weights = Weights::default();
        let clean = candidate(1, Board::default(), Outcome::default());
        let holed = candidate(
            1,
            Board {
                holes: 1,
                ..Board::default()
            },
            Outcome::default(),
        );
        assert_eq!(choose(&[holed, clean], &weights), Some(1));
        assert_eq!(choose(&[], &weights), None);
    }

    #[test]
    fn ties_are_broken_the_same_way_on_every_machine() {
        // §P6.4: equal evaluations go to the lower top-out risk, then to the
        // shorter input sequence, then to the generator's canonical order.
        // Scored against an opinion of nothing, so that every candidate here
        // evaluates to zero and only the tie-breaks can separate them.
        let indifferent = Weights {
            lines: 0,
            holes: 0,
            covered: 0,
            aggregate_height: 0,
            max_height: 0,
            bumpiness: 0,
            row_transitions: 0,
            column_transitions: 0,
            blockades: 0,
            wells: 0,
            top_out: 0,
            combo: 0,
            back_to_back: 0,
            perfect_clear: 0,
        };
        let tall = Board {
            max_height: 12,
            ..Board::default()
        };
        let ended = Outcome {
            topped_out: true,
            ..Outcome::default()
        };

        // A branch that ended in §9.16 loses to one that did not, whatever the
        // weights are told to think of it.
        assert_eq!(
            choose(
                &[
                    candidate(1, Board::default(), ended),
                    candidate(9, Board::default(), Outcome::default()),
                ],
                &indifferent,
            ),
            Some(1),
        );
        // Then the shorter stack, then the shorter sequence.
        assert_eq!(
            choose(
                &[
                    candidate(1, tall, Outcome::default()),
                    candidate(9, Board::default(), Outcome::default()),
                ],
                &indifferent,
            ),
            Some(1),
        );
        assert_eq!(
            choose(
                &[
                    candidate(6, Board::default(), Outcome::default()),
                    candidate(2, Board::default(), Outcome::default()),
                ],
                &indifferent,
            ),
            Some(1),
        );
        // ...and then the generator's own order, which is the index.
        assert_eq!(
            choose(
                &[
                    candidate(3, Board::default(), Outcome::default()),
                    candidate(3, Board::default(), Outcome::default()),
                ],
                &indifferent,
            ),
            Some(0),
        );
    }
}
