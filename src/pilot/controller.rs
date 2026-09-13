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
//! **How far it looks is [`search`]'s** (§P6), and what this file owns is the
//! *position* the search is given: the fair roots of §P2.1. One root when the
//! preview reaches the horizon, and one per hypothesis when it does not — which
//! is the only place in the planner that decides what a search is allowed to
//! know, and is deliberately not inside the search itself.

use crate::core::{Game, GameEvent, GameView, PieceKind, PlayState, TickInput};
use crate::pilot::eval::Weights;
use crate::pilot::fork::Fork;
use crate::pilot::knowledge::Knowledge;
use crate::pilot::placement::{self, Prediction};
use crate::pilot::search;
use crate::shell::config::RulesConfig;

/// How hard the planner is asked to think (§P3.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Plies, clamped by `preview_count` (§P6.1).
    pub depth: u8,
    /// States kept per ply (§P6.2).
    pub beam: u16,
    /// The budget, an integer count and never a duration (§P6.4).
    pub nodes: u32,
    /// Which generator produces the candidates: §P4.2's walk, or §P4.1's
    /// rotate-shift-and-drop.
    ///
    /// **Off by default, and that is a measurement rather than a preference.**
    /// §P4.2 reaches everything a hard drop cannot — tucks, spins, everywhere
    /// under an overhang — and it is worth 12-19% more score for **35× the
    /// cost**: ~6,400 forks advanced a generation against §P4.1's flat 104.
    /// `nodes` is 2,000, so a walk at two plies is stopped inside its *first*
    /// generation and §P6.4 hands back the best fully evaluated move, which is
    /// precisely the one-ply answer — byte for byte, which P7 measured rather
    /// than assumed. The two defaults are therefore not combinable.
    /// `PILOT-PLAN.md` P7 has the tables.
    pub exact: bool,
}

impl Default for Settings {
    /// §P6.1's two plies, a beam of sixteen, and the budget P5 measured.
    ///
    /// The three are one decision rather than three. P5 measured a frame at
    /// ~11,900 nodes and §15.2 step 4 may play `MAX_CATCH_UP_TICKS` ticks before
    /// it draws, so a search may spend a sixth of that: **~2,000 nodes**. Two
    /// plies over a beam of sixteen costs 104 candidates plus sixteen
    /// expansions of about 104 each — a little under 1,800 — which is what makes
    /// these particular three numbers the ones that fit. A wider beam or a
    /// third ply would be spent by the budget rather than played, and §P6.4's
    /// answer would quietly become a shallower one.
    fn default() -> Self {
        Self {
            depth: 2,
            beam: 16,
            nodes: 2_000,
            exact: false,
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
    /// §P8.2's two counters, and the only thing a `Pilot` accumulates that is
    /// not a decision. Both are integers rather than durations for §P3.3's
    /// reason: a benchmark that compared machines by wall clock would compare
    /// the machines, and the acceptance limits are node counts precisely
    /// because they are the same everywhere.
    ///
    /// They are equal at one ply, where every node evaluated is a placement
    /// generated. P6 is where they part: a beam search evaluates interior
    /// states that are nobody's placement, and a transposition hit is a
    /// placement that costs no node at all.
    counted: Counted,
}

/// What a search cost, in the two units §P8.2 reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counted {
    /// States evaluated.
    pub nodes: u64,
    /// Candidate placements generated.
    pub placements: u64,
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
            counted: Counted::default(),
        }
    }

    /// What the searches so far have cost (§P8.2).
    ///
    /// Cumulative over the game, so a benchmark divides by the pieces it
    /// played. Nothing in the planner reads it and no decision depends on it:
    /// a counter a search consulted would be a clock by another name.
    pub fn counted(&self) -> Counted {
        self.counted
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

    /// Plan the piece in play (§P6).
    ///
    /// Two steps, and the order of them is the information boundary. First the
    /// **roots**: the live game forked on what the player can see, and — when
    /// the search wants a piece past the preview — one such fork per hypothesis
    /// §P2.4 says is still in the bag. Then the search, which sees forks and
    /// settings and nothing else, and so cannot widen what it was given.
    fn think(&mut self, game: &Game, view: &GameView) -> Plan {
        let depth = search::depth_for(self.settings.depth, view.next.len());
        let roots = self.roots(game, view, depth);
        let chosen = search::plan(&roots, &self.rules, self.settings, &self.weights, depth);
        self.counted.nodes += chosen.counted.nodes;
        self.counted.placements += chosen.counted.placements;
        debug_assert!(
            !chosen.inputs.is_empty(),
            "§P4.1's generator offered nothing",
        );
        // §P3.1's divergence check, and the only thing in this file that costs
        // anything it does not have to: one more replay of the chosen sequence,
        // recording what it predicts tick by tick. `PILOT-PLAN.md` has it
        // debug-shaped for exactly that reason, and a release build records
        // nothing and compares nothing.
        //
        // Predicted against a fork of the **visible** queue, never against one
        // of the hypothesis roots: a hypothesis is a piece the live game has not
        // promised to deal, so a prediction made on one would diverge whenever
        // the guess was wrong, which is not what the assertion is looking for.
        let predicted = if cfg!(debug_assertions) {
            placement::predict(&Fork::of(game, &view.next), &chosen.inputs)
        } else {
            Vec::new()
        };
        Plan {
            inputs: chosen.inputs,
            predicted,
            pending: None,
            at: 0,
        }
    }

    /// The positions a search may start from (§P2.1, §P6.3).
    ///
    /// One when the preview reaches the horizon, which is the ordinary case at
    /// §6.3's default of five and a depth of two. When it does not — a preview
    /// of 1, where §P6.1's second ply is already a chance node — it is one root
    /// per piece that could still come out of the open bag, and §P6.3's blend
    /// over them is what a chance node means here. The hypotheses are the
    /// planner's own arithmetic over pieces it *saw* dealt, which is the whole of
    /// why a fork cannot be fed a piece the player could not have worked out.
    fn roots(&self, game: &Game, view: &GameView, depth: u8) -> Vec<Fork> {
        if search::hypotheses_needed(depth, view.next.len()) == 0 {
            return vec![Fork::of(game, &view.next)];
        }
        let hypotheses = match self.knowledge.as_ref() {
            Some(knowledge) => knowledge.hypotheses(&view.next),
            // Unreachable: `start` builds the tracker before anything plans.
            None => return vec![Fork::of(game, &view.next)],
        };
        hypotheses
            .iter()
            .map(|hypothesis| {
                let mut queue: Vec<PieceKind> = view.next.clone();
                queue.push(hypothesis);
                Fork::of(game, &queue)
            })
            .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Action, PlayState};

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
    /// This is the loop `App::advance` runs since P4, written out here because
    /// the planner must be drivable without a front-end at all (§P3.3).
    fn watch(rules: &RulesConfig, seed: u64, pieces: u32) -> Game {
        played(rules, Settings::default(), seed, pieces).0
    }

    /// §P4.2's generator at one ply, which is what a walk fits in.
    fn exact() -> Settings {
        Settings {
            depth: 1,
            exact: true,
            ..Settings::default()
        }
    }

    /// The default settings at another depth.
    fn settings(depth: u8) -> Settings {
        Settings {
            depth,
            ..Settings::default()
        }
    }

    /// The same round, and the two things worth knowing about it afterwards:
    /// where it got to, and the most any one search cost on the way.
    fn played(
        rules: &RulesConfig,
        settings: Settings,
        seed: u64,
        pieces: u32,
    ) -> (Game, Counted, u64) {
        let mut game = Game::new(rules.clone(), seed);
        let mut pilot = Pilot::new(rules, settings);
        let mut events = Vec::new();
        let mut dearest = 0;
        while game.view().pieces < pieces && game.state() != PlayState::ToppedOut {
            let before = pilot.counted().nodes;
            let input = pilot.input(&game);
            dearest = dearest.max(pilot.counted().nodes - before);
            events.clear();
            game.tick(&input, &mut events);
            pilot.observe(&events);
        }
        let counted = pilot.counted();
        (game, counted, dearest)
    }

    /// `played`, for the tests that want the figures rather than the game.
    fn counted_watch(
        rules: &RulesConfig,
        settings: Settings,
        seed: u64,
        pieces: u32,
    ) -> (GameView, Counted) {
        let (game, counted, _) = played(rules, settings, seed, pieces);
        (game.view(), counted)
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
    fn it_plays_a_game_with_the_exact_generator_too() {
        // §P4.2 driven the way a round drives it, which is the strongest
        // correctness check in the tree for it: `cargo test` is a debug build,
        // so §P3.1's divergence assertion replays every plan the walk produced
        // and compares it to the live game tick by tick. A walk that reported a
        // sequence reaching somewhere it does not would be caught here rather
        // than by a board comparison.
        //
        // Short, because a walk is a thousand positions a piece against §P4.1's
        // hundred — and one ply, because that is what fits: at two plies the
        // default budget is spent inside the first generation and §P6.4 hands
        // back the one-ply answer anyway (`PILOT-PLAN.md` P7).
        let rules = rules(5, true, true);
        let (game, counted, _) = played(&rules, exact(), 7, 40);
        let view = game.view();
        assert_eq!(view.pieces, 40, "it played the pieces");
        assert_ne!(game.state(), PlayState::ToppedOut);
        assert!(view.lines >= 4, "only {} lines in 40 pieces", view.lines);
        assert!(
            counted.nodes > 10 * counted.placements,
            "a walk costs many nodes per candidate: {counted:?}",
        );
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
        // §P3.4's signature, and §P6's three numbers. The default is one
        // decision rather than three: P5 measured the budget, and two plies over
        // a beam of sixteen is what fits inside it.
        let settings = Settings {
            depth: 3,
            beam: 8,
            nodes: 1_234,
            exact: true,
        };
        let pilot = Pilot::new(&RulesConfig::default(), settings);
        assert_eq!(pilot.settings(), settings);
        assert_eq!(Settings::default().depth, 2, "§P6.1's two plies");
        assert_eq!(Settings::default().beam, 16, "§P6.2's beam");
        assert_eq!(Settings::default().nodes, 2_000, "§P6.4, measured by P5");
    }

    #[test]
    fn no_one_search_costs_more_than_the_budget_and_the_ply_it_was_inside() {
        // §P6.4, over a game rather than a position. The budget stops a search
        // between one ply and the next, so what it bounds is the budget plus the
        // generation it was in the middle of — 104 candidates at §P4.1's span,
        // and a search is always allowed the first ply whatever it is given.
        let rules = rules(5, true, true);
        let (_, _, dearest) = played(&rules, Settings::default(), 42, 30);
        let budget = u64::from(Settings::default().nodes);
        assert!(
            dearest <= budget + 2 * 104,
            "one search cost {dearest} nodes against a budget of {budget}",
        );
        assert!(dearest > 1_000, "and a two-ply search is not cheap");
    }

    #[test]
    fn two_plies_look_further_than_one_and_cost_more_for_it() {
        // The stage from outside: the same seed at depth 1 and depth 2 is two
        // different games, and the deeper one paid in nodes for the difference.
        let rules = rules(5, true, true);
        let (one, shallow) = counted_watch(&rules, settings(1), 42, 40);
        let (two, deep) = counted_watch(&rules, settings(2), 42, 40);
        assert!(
            deep.nodes > shallow.nodes * 5,
            "{deep:?} against {shallow:?}",
        );
        assert_ne!(
            one.rows, two.rows,
            "two plies chose the same moves as one for forty pieces",
        );
    }

    #[test]
    fn a_preview_of_one_searches_over_hypotheses_and_still_plays() {
        // §P6.1 and §P6.3 in the configuration that makes them ordinary: at
        // §6.3's smallest preview the second ply is a chance node, so the roots
        // are one per piece still in the bag and the values are blended. It has
        // to play, and it has to cost more than a preview that needs no
        // hypothesis at all.
        let narrow = rules(1, true, true);
        let wide = rules(5, true, true);
        let (view, counted) = counted_watch(&narrow, Settings::default(), 42, 30);
        assert_eq!(view.pieces, 30);
        assert!(view.lines > 0, "it cleared nothing in thirty pieces");
        let (_, cheap) = counted_watch(&wide, Settings::default(), 42, 30);
        assert!(
            counted.nodes > cheap.nodes,
            "a chance node costs more: {counted:?} against {cheap:?}",
        );
    }
}
