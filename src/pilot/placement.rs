//! §P4.1's simple generator: rotate at spawn, shift, hard drop.
//!
//! A placement is an **input sequence**, never a board position (§P4.2): a
//! placement nothing can reach is not a placement, and the only way to know
//! what a sequence reaches is to play it. So every candidate here is replayed
//! through the game's own rules on a [`Fork`] — kicks, lock down, §9.12's
//! ordering and all — and what it is worth is measured from the board that
//! replay leaves behind. Nothing predicts; everything is played.
//!
//! Two consequences of that, both of which would otherwise want special cases.
//! A rotation that kicks, or fails, at a stack that has grown into the spawn
//! area simply produces whatever it produces. And a piece that gravity locks
//! before the sequence finishes — everything at high gravity, which §P4.2 is
//! eventually for — is *truncated* at the tick it locked, so the plan can never
//! hand a leftover input to the piece after it.
//!
//! What this generator cannot reach is everything a hard drop cannot: tucks,
//! spins and placements under an overhang. That is §P4.2's exact generator, in
//! stage P7, and this is the one `PILOT.md` recommends starting from.

use crate::core::{Action, GameEvent, GameView, PlayState, Rotation, Shift, TickInput};
use crate::pilot::eval::{Board, Features, Outcome};
use crate::pilot::fork::Fork;
use crate::shell::config::RulesConfig;

/// How far either way a candidate will try to shift, in cells.
///
/// Five reaches either wall from any spawn — §9.4 spawns at column 3, or 4 for
/// the `O`, and the deepest rotation puts a mino in column 5 — and six is that
/// plus the two cells an SRS kick can carry a piece sideways as it turns
/// (§9.5). A shift into a wall is refused rather than an error, so a candidate
/// that asks for more than it needs merely duplicates a shorter one and loses
/// to it on §P6.4's second tie-break.
const SPAN: i8 = 6;

/// A safety net on the settling loop below, in ticks. The clear and entry
/// delays are milliseconds in §6.3 and cannot approach ten seconds; this is
/// here so a rule that changed under it could never spin.
const SETTLE_LIMIT: u32 = 600;

/// One candidate placement: the inputs that reach it, and what they lead to.
pub(crate) struct Placement {
    /// One [`TickInput`] per tick under §P3.2's cap, ending with the tick that
    /// locked the piece.
    pub(crate) inputs: Vec<TickInput>,
    /// §P5's features, measured once the board has settled — after the clear
    /// delay, because §9.12 leaves a completed row on the screen through it and
    /// a board measured during it still holds the rows it is about to lose.
    pub(crate) features: Features,
    /// The fork this candidate left behind, settled and with the next piece in
    /// play. It is what the ply after this one is generated from (§P6.2): a
    /// search deeper than one ply continues from a position rather than
    /// replaying to it, which is the whole reason a [`Fork`] is `Clone`.
    pub(crate) after: Fork,
}

/// Every placement the simple generator can reach from this position (§P4.1).
///
/// The position is a [`Fork`] rather than the live game, at every ply including
/// the first: the root is the live game forked on the caller's fair queue
/// (§P2.1), and everything below it is a fork of a fork. Nothing in here can
/// name a `Game`, so there is no path from a placement to §9.6's bag even by
/// accident.
///
/// What it reads of the position is `view()` — the board, the piece in play and
/// the hold slot — and `state()`; the queue it deals from is the fork's own.
///
/// The order is **canonical** and is the last of §P6.4's tie-breaks: no hold
/// before hold, §9.3's four orientations in numbering order, and shifts from
/// the far left to the far right. Every machine therefore enumerates the same
/// candidates in the same sequence, and two builds cannot choose differently
/// from one position.
pub(crate) fn placements(fork: &Fork, rules: &RulesConfig) -> Vec<Placement> {
    let view = fork.view();
    // §P4.3: a disabled mechanic is *absent* from the search rather than
    // unused, because §10.1 drops its key at the input boundary and a plan that
    // counted on it would be a plan the game declines to perform. The same goes
    // for a hold already spent on this piece (§9.7) — the key is live, and the
    // game would refuse it.
    let holds: &[bool] = if rules.hold_enabled && !view.hold_locked {
        &[false, true]
    } else {
        &[false]
    };

    let mut placements = Vec::new();
    let mut events = Vec::new();
    for &hold in holds {
        for rotation in Rotation::ALL {
            for dx in -SPAN..=SPAN {
                let inputs = sequence(hold, rotation, dx, rules);
                placements.push(replay(fork, inputs, &mut events));
            }
        }
    }
    placements
}

/// One tick of a plan, as the search predicted it.
#[derive(Clone)]
pub(crate) struct Prediction {
    /// What the fork held after that tick.
    pub(crate) view: GameView,
    /// Whether the fork still knew what came next.
    ///
    /// A queue used up by the very tick that locked the piece cannot say what
    /// spawned after it, and that is §P2.3 working rather than failing: the
    /// caller's list is all a fork has, and the piece after a hold at a preview
    /// of 1 is past the end of it. Everything the *plan* claims is still
    /// comparable — the board, the figures, the hold slot — and only the piece
    /// the plan does not control is not.
    pub(crate) whole: bool,
}

/// Replay `inputs` and report what each tick is predicted to leave behind.
///
/// This is what §P3.1's divergence assertion is checked against: the live game
/// is handed the same inputs from the same position, so it must arrive at the
/// same place, tick for tick. The caller compares every field but the preview
/// queue, which is a fork's scripted remainder rather than a bag's and is the
/// one thing that always differs (§P2.3).
pub(crate) fn predict(root: &Fork, inputs: &[TickInput]) -> Vec<Prediction> {
    let mut fork = root.clone();
    let mut events = Vec::new();
    inputs
        .iter()
        .map(|input| {
            events.clear();
            fork.tick(input, &mut events);
            Prediction {
                view: fork.view(),
                whole: !fork.exhausted(),
            }
        })
        .collect()
}

/// The inputs for one candidate: hold, turn, shift, drop — one thing per tick.
///
/// §P3.2 is kept by construction here and nowhere else: every input this
/// produces carries at most one action and at most one shift cell, because
/// `TickInput::action` and `TickInput::shift` are the only two ways it builds
/// one.
fn sequence(hold: bool, rotation: Rotation, dx: i8, rules: &RulesConfig) -> Vec<TickInput> {
    let mut inputs = Vec::new();
    if hold {
        inputs.push(TickInput::action(Action::Hold));
    }
    for &turn in turns(rotation, rules.allow_180_rotation) {
        inputs.push(TickInput::action(turn));
    }
    let direction = if dx < 0 { Shift::Left } else { Shift::Right };
    for _ in 0..dx.unsigned_abs() {
        inputs.push(TickInput::shift(direction));
    }
    inputs.push(TickInput::action(Action::HardDrop));
    inputs
}

/// The cheapest way to face `rotation` from spawn, in whole ticks.
///
/// §P4.3 again: with §6.3's `allow_180_rotation` off, South is two quarter
/// turns rather than an orientation the search gives up on. A 180 that the
/// rules forbid is dropped at the input boundary, so asking for one would cost
/// a tick and leave the piece facing North.
fn turns(rotation: Rotation, allow_180: bool) -> &'static [Action] {
    match rotation {
        Rotation::North => &[],
        Rotation::East => &[Action::RotateCw],
        Rotation::South if allow_180 => &[Action::Rotate180],
        Rotation::South => &[Action::RotateCw, Action::RotateCw],
        Rotation::West => &[Action::RotateCcw],
    }
}

/// Play one candidate from `from` and measure what it left behind.
///
/// This is also how a ply below the first reaches a position: the caller hands
/// it the fork a candidate settled into, and the sequence is played from there.
/// The fork is cloned rather than consumed, because one position is the parent
/// of a hundred candidates.
///
/// The event buffer is the caller's and is reused across every candidate of
/// every piece, exactly as `App` reuses one across frames: `LinesCleared`
/// carries a `Vec` and this runs a hundred times a piece — and, since P6, a
/// hundred times per beam member as well.
pub(crate) fn replay(
    from: &Fork,
    mut inputs: Vec<TickInput>,
    events: &mut Vec<GameEvent>,
) -> Placement {
    let mut fork = from.clone();
    let mut lines = 0;
    let mut perfect_clear = false;

    // The plan ends at the tick that locks the piece, whichever tick that turns
    // out to be: the hard drop at the end of the sequence, or an earlier one at
    // a gravity fast enough to land the piece while it is still being shifted.
    // Anything after it would be an input handed to the *next* piece, which is
    // §P3.1's divergence rather than a plan.
    let mut locked = inputs.len();
    for (tick, input) in inputs.iter().enumerate() {
        events.clear();
        fork.tick(input, events);
        tally(events, &mut lines, &mut perfect_clear);
        if events
            .iter()
            .any(|event| matches!(event, GameEvent::PieceLocked { .. }))
        {
            locked = tick + 1;
            break;
        }
        if fork.state() == PlayState::ToppedOut {
            locked = tick + 1;
            break;
        }
    }
    inputs.truncate(locked);

    // §9.12: the completed rows are still on the board through the clear delay,
    // so a board measured now would be measured before it lost them. Wait for
    // the next piece — or for the end of the game, or for the end of what the
    // caller told the fork about, which §P2.3 makes a third thing and not an
    // error.
    let mut settling = 0;
    while fork.state() != PlayState::Falling
        && fork.state() != PlayState::ToppedOut
        && !fork.exhausted()
    {
        events.clear();
        fork.tick(&TickInput::default(), events);
        tally(events, &mut lines, &mut perfect_clear);
        settling += 1;
        debug_assert!(settling < SETTLE_LIMIT, "the fork never settled");
        if settling >= SETTLE_LIMIT {
            break;
        }
    }

    let view = fork.view();
    Placement {
        inputs,
        features: Features {
            board: Board::of(&view.rows),
            outcome: Outcome {
                lines,
                topped_out: fork.state() == PlayState::ToppedOut,
                combo: view.combo,
                back_to_back: view.back_to_back,
                perfect_clear,
            },
        },
        after: fork,
    }
}

/// Fold one tick's events into the two outcome figures no view reports.
///
/// Both are *events* rather than state (§12.8): a clear is gone from the board
/// by the time anything can look at it, and §9.15's perfect clear is a bonus
/// paid once.
fn tally(events: &[GameEvent], lines: &mut i32, perfect_clear: &mut bool) {
    for event in events {
        match event {
            GameEvent::LinesCleared { rows, .. } => *lines += rows.len() as i32,
            GameEvent::PerfectClear => *perfect_clear = true,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Game, VIEW_WIDTH};

    /// The defaults with §6.3's three gameplay settings changed.
    ///
    /// By field rather than through `RulesConfig::from_settings`, because §P3.4
    /// lets the planner name `RulesConfig` and nothing else in the shell.
    fn rules(preview_count: u8, hold_enabled: bool, allow_180_rotation: bool) -> RulesConfig {
        RulesConfig {
            preview_count,
            hold_enabled,
            allow_180_rotation,
            ..RulesConfig::default()
        }
    }

    /// The root of a fresh game, and the placements from it.
    ///
    /// The root is the live game forked on its own visible preview, which is the
    /// only queue §P2.1 allows without a hypothesis — and from here down
    /// everything in this module sees forks alone.
    fn from_spawn(seed: u64, rules: &RulesConfig) -> (Fork, Vec<Placement>) {
        let game = Game::new(rules.clone(), seed);
        let queue = game.view().next;
        let root = Fork::of(&game, &queue);
        let placements = placements(&root, rules);
        (root, placements)
    }

    #[test]
    fn every_input_honours_the_cap() {
        // §P3.2, and §P9's C5: at most one action and at most one shift cell
        // per tick. The cap is a rule and this is the test it says it is.
        for seed in 0..8u64 {
            let rules = rules(5, true, true);
            let (_, placements) = from_spawn(seed, &rules);
            for placement in &placements {
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
    fn every_placement_ends_at_the_tick_that_locks_the_piece() {
        // §P3.1: a plan runs "until the piece locks", so its last input is the
        // one that locked it and there is never one after. At level 1 that is
        // always the hard drop the sequence ends with.
        let rules = rules(5, true, true);
        let (root, placements) = from_spawn(3, &rules);
        for placement in &placements {
            let ticks = predict(&root, &placement.inputs);
            let (last, earlier) = ticks.split_last().expect("a placement is never empty");
            assert!(
                earlier.iter().all(|tick| tick.view.pieces == 0),
                "a piece locked before the end of the sequence",
            );
            assert_eq!(last.view.pieces, 1, "and the last tick locked one");
        }
    }

    #[test]
    fn the_generator_reaches_both_walls() {
        // The point of §P4.1: rotate at spawn, shift, drop reaches every column
        // while the well is open. Whatever the piece, some candidate puts a
        // mino against each wall.
        for seed in 0..8u64 {
            let rules = rules(5, false, true);
            let (root, placements) = from_spawn(seed, &rules);
            let mut columns = [false; VIEW_WIDTH];
            for placement in &placements {
                let landed = predict(&root, &placement.inputs)
                    .pop()
                    .expect("a placement is never empty");
                for row in &landed.view.rows {
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
    fn a_disabled_mechanic_is_absent_from_the_search() {
        // §P4.3. Not "generated and then refused": §10.1 drops the key at the
        // input boundary, so a candidate that used one would plan a move the
        // game declines to make and the piece would sit there for a tick.
        let (_, with) = from_spawn(5, &rules(5, true, true));
        let (_, without) = from_spawn(5, &rules(5, false, false));
        let holds = |placements: &[Placement]| {
            placements.iter().any(|placement| {
                placement.inputs[0]
                    .actions
                    .iter()
                    .any(|a| a == Action::Hold)
            })
        };
        let spins = |placements: &[Placement]| {
            placements.iter().any(|placement| {
                placement
                    .inputs
                    .iter()
                    .any(|input| input.actions.iter().any(|a| a == Action::Rotate180))
            })
        };
        assert!(holds(&with) && spins(&with), "both are on");
        assert!(!holds(&without), "hold is off, so nothing holds");
        assert!(!spins(&without), "180 is off, so nothing asks for one");
        // ...and South is still reachable, by two quarter turns.
        let quarters = without.iter().any(|placement| {
            placement
                .inputs
                .iter()
                .filter(|input| input.actions.iter().any(|a| a == Action::RotateCw))
                .count()
                == 2
        });
        assert!(quarters, "South is two turns away, not unreachable");
    }

    #[test]
    fn a_hold_candidate_plays_the_other_piece() {
        // §P4.1 asks for the current piece *and* the held or next one. With an
        // empty hold slot the swap deals from the queue, so the piece that
        // lands is the preview's first and not the one in play.
        let rules = rules(5, true, true);
        let (root, placements) = from_spawn(9, &rules);
        let view = root.view();
        let current = view.current.expect("a piece is in play").kind;
        let next = view.next[0];
        assert_ne!(current, next, "§9.6's bag is a permutation");

        let held: Vec<_> = placements
            .iter()
            .filter(|placement| {
                placement.inputs[0]
                    .actions
                    .iter()
                    .any(|a| a == Action::Hold)
            })
            .collect();
        assert!(!held.is_empty());
        for placement in held {
            let last = predict(&root, &placement.inputs)
                .pop()
                .expect("a placement is never empty")
                .view;
            let landed = last
                .rows
                .iter()
                .flatten()
                .flatten()
                .copied()
                .next()
                .expect("something locked");
            assert_eq!(landed, next, "the held candidate played the wrong piece");
            assert_eq!(last.hold, Some(current), "and put the other one away");
        }
    }

    #[test]
    fn one_piece_on_an_empty_field_is_measured_as_one_piece() {
        // The features come from the board the replay leaves behind, so on an
        // empty field every candidate is four minos resting on the floor:
        // nothing cleared, nothing chained, and a stack no taller than a
        // standing `I`.
        //
        // That the measurement waits out §9.12's clear delay is not visible
        // here, because nothing clears — it is visible in
        // `controller::tests::it_clears_lines_over_a_long_game`, which is what
        // a planner measuring a board that still held the rows it was about to
        // lose would fail.
        let rules = rules(5, false, true);
        let (_, placements) = from_spawn(11, &rules);
        for placement in &placements {
            let features = &placement.features;
            assert_eq!(features.outcome.lines, 0);
            assert!(!features.outcome.topped_out);
            assert_eq!(features.outcome.combo, -1, "§9.15's resting counter");
            assert!(!features.outcome.back_to_back);
            assert!(
                (1..=4).contains(&features.board.max_height),
                "one piece on the floor: {:?}",
                features.board,
            );
            assert!(features.board.aggregate_height >= 4);
        }
    }
}
