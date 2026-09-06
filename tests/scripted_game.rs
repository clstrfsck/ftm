//! I1 (§17.2): a scripted game against a checked-in snapshot, and the
//! batch-invariance canary of §19.4.
//!
//! The harness is the whole point of the core's layering rule (§3.1): a
//! `RulesConfig`, a seed and a `Vec<TickInput>` go in, a `GameView` comes out,
//! and no terminal is involved at any stage.
//!
//! **The batch-invariance test must never be marked `#[ignore]`.** If it fails,
//! something in the rules has started depending on how the shell batched its
//! calls, and §19 is quietly foreclosed. Find the desync; do not skip the test.

use std::collections::BTreeMap;

use ftm::config::RulesConfig;
use ftm::core::{Action, Game, GameEvent, GameView, PlayState, Shift, TickInput};

/// The checked-in snapshot of the scripted game (§17.2).
const SNAPSHOT: &str = include_str!("snapshots/scripted_game.txt");

const SEED: u64 = 42;

/// The recorded input log (§17.2): one placement per piece, `240` of them.
///
/// Each placement is two characters — the rotation applied at spawn (`N` none,
/// `R` clockwise, `L` anticlockwise, `2` a half turn) and the column the piece
/// is walked out to after being slammed against the left wall. It was recorded
/// once by a throwaway greedy bot and checked in; nothing reads the board at
/// run time, so this is a fixed log and not a player.
///
/// A blind formula was tried first and buried itself under thirty pieces
/// without completing a row, which made for a snapshot that proved nothing
/// about clearing, levelling or the back-to-back state Stage 7 will add. This
/// log clears 94 rows and reaches level 10.
const PLACEMENTS: &[&str] = &[
    "N0N3N2N6N5N4N8N021N6N4R3N0R8R9R6R2N7R0N0R2L4L3N6R8R521N7N6N3",
    "R025R4L1L0R4L8R320N6R6N0R2R9R7N0R4L7R2R6L8N023R6N3R82024N0R7",
    "R2N7N3N3N0N2R8R0N5R4R6R0R2R1R8R7N3N6R3R0N1R5N4N6N7N0N4N3R9L8",
    "N1N4R0N7N3R7N4N2R0N6L8N1N6N32125R0N1R8R3N424N7N2L6N0R926R2R1",
    "N8R5R2R6R7R0R4R0N3R9N5R124L723L0R0N6N5R2R4L8R7N1R2L5R9R4N0R2",
    "L7R5R3L6R8R0R1N5R7L8R2N4R0R1N3R7N222R6R5N6R0N3N6L4R9N6L0R2R8",
    "N1R0N8N0R4R8L2R9N1N5N5R821N1R2R4N4L3R8R7R0N0R5N0L6R7N2L4R9R3",
    "R6N0L7N2N5N3R0R1N8N5R0R9L7R224R0N422R6R1R5R3N7R021L3N6R0L8R8",
];

/// Ticks per placement (§15.1).
///
/// The first six do the work; the rest are idle. The tail exists to absorb a
/// line-clear pause, during which the core accepts no input (§9.12 step 5) — a
/// shorter slot would have the *next* piece's rotation and shift swallowed by
/// the flash, and the log would drift out of step with the board it was
/// recorded against.
const SLOT: usize = 24;

/// Total ticks: the whole log, played out.
const TICKS: usize = 240 * SLOT;

/// The same idea at level 15, where gravity does the landing (§9.9).
///
/// A second recording rather than the first one replayed faster, because the
/// first stops meaning anything above about level 12. It slams the piece to the
/// wall on one tick and walks it out on the next, which works only while the
/// piece is still where it spawned; at 2.36 rows a tick it is five rows down
/// and inside the stack by then, the recorded columns stop being reachable, and
/// the game tops out inside forty pieces however fast it is played.
///
/// So each placement here is a single tick — the rotation and the whole shift
/// together, while the piece is still on its spawn row — and nothing hard drops
/// or soft drops: what puts every piece where it lands is gravity, and what
/// ends its fall is §9.11's lock delay. Two characters each: the rotation (`N`
/// none, `R` clockwise, `L` anticlockwise, `2` a half turn) and the shift, `0`
/// to `9` cells right or `a` to `i` one to nine cells left. Recorded once by a
/// throwaway greedy bot and checked in, as the log above was; nothing reads the
/// board at run time.
const HIGH_GRAVITY: &[&str] = &[
    "NcN4N2R0N3R0N2R1L0NcRbNb20R3RfR4R3NaLbN4R123R0NdN2R32cR4NbNd",
    "NaR3NcR2R0R0RcRaRaR0RaN4R0R1N4N1L4R2RaR120NdR5NcN4R4RcLcNcL2",
    "R5R3N0L4NcR4NaR1R0RcL4N3RcN3LbR0RaL4RcRcN3N2NcNaR0R5NbR0L0Nb",
    "RaR1R1L020RaN2R4R3L5L2N0RcR5LaNcR3",
];

/// Ticks per placement at level 15: about nine to fall the depth of the well,
/// thirty of §9.11's lock delay and fifteen of §9.12's clear pause, with room
/// over. The tail is idle, as it is above.
const HIGH_GRAVITY_SLOT: usize = 60;

/// Play `script` through a fresh game, `batch` ticks at a time, and hand back
/// the final view.
///
/// The batching is the shell's business and must not be the core's: the loop of
/// §15.2 runs up to `MAX_CATCH_UP_TICKS` ticks between two frames, so the same
/// log arrives as one tick at a time on an idle machine and six at a time on a
/// busy one. The core is looked at once per batch, exactly as a real shell
/// would look at it, so that any dependence on *being* looked at shows up too.
fn play(rules: RulesConfig, seed: u64, script: &[TickInput], batch: usize) -> GameView {
    let mut game = Game::new(rules, seed);
    let mut events: Vec<GameEvent> = Vec::new();
    for chunk in script.chunks(batch) {
        events.clear();
        for input in chunk {
            game.tick(input, &mut events);
        }
        let _ = game.view();
    }
    game.view()
}

/// Expand one placement into its `SLOT` ticks.
fn placement_ticks(rotation: char, column: u8) -> Vec<TickInput> {
    let mut slot = vec![
        match rotation {
            'R' => TickInput::action(Action::RotateCw),
            'L' => TickInput::action(Action::RotateCcw),
            '2' => TickInput::action(Action::Rotate180),
            'N' => TickInput::default(),
            other => panic!("unknown rotation {other:?} in the recorded log"),
        },
        // Nine cells is wider than the board, so the piece arrives at the left
        // wall whatever the rotation left it under, and `column` means the same
        // thing for every piece.
        TickInput {
            shift: Some(Shift::Left),
            shift_cells: 9,
            ..TickInput::default()
        },
        TickInput {
            shift: Some(Shift::Right),
            shift_cells: column,
            ..TickInput::default()
        },
        TickInput {
            soft_drop: true,
            ..TickInput::default()
        },
        TickInput::default(),
        TickInput::action(Action::HardDrop),
    ];
    slot.resize(SLOT, TickInput::default());
    slot
}

/// The recorded log, expanded into one `TickInput` per tick.
fn recorded_log() -> Vec<TickInput> {
    let mut log = Vec::with_capacity(TICKS);
    for line in PLACEMENTS {
        let chars: Vec<char> = line.chars().collect();
        assert_eq!(chars.len() % 2, 0, "placements come in pairs");
        for pair in chars.chunks(2) {
            let column = pair[1].to_digit(10).expect("a placement column is a digit") as u8;
            log.extend(placement_ticks(pair[0], column));
        }
    }
    assert_eq!(log.len(), TICKS, "the log is 240 placements long");
    log
}

/// Level 15 with everything else left alone.
fn high_gravity_rules() -> RulesConfig {
    RulesConfig {
        start_level: 15,
        ..RulesConfig::default()
    }
}

/// The high-gravity log, expanded into one `TickInput` per tick.
fn high_gravity_log() -> Vec<TickInput> {
    let mut log = Vec::new();
    for line in HIGH_GRAVITY {
        let chars: Vec<char> = line.chars().collect();
        assert_eq!(chars.len() % 2, 0, "placements come in pairs");
        for pair in chars.chunks(2) {
            let (direction, cells) = match pair[1] {
                digit @ '0'..='9' => (Shift::Right, digit as u8 - b'0'),
                letter @ 'a'..='i' => (Shift::Left, letter as u8 - b'a' + 1),
                other => panic!("unknown shift {other:?} in the high-gravity log"),
            };
            // The rotation and the shift share the tick, in that order (§15.1),
            // and are the only input the placement gets.
            let mut slot = vec![TickInput {
                actions: match pair[0] {
                    'R' => Some(Action::RotateCw),
                    'L' => Some(Action::RotateCcw),
                    '2' => Some(Action::Rotate180),
                    'N' => None,
                    other => panic!("unknown rotation {other:?} in the high-gravity log"),
                }
                .into_iter()
                .collect(),
                shift: (cells > 0).then_some(direction),
                shift_cells: cells,
                ..TickInput::default()
            }];
            slot.resize(HIGH_GRAVITY_SLOT, TickInput::default());
            log.extend(slot);
        }
    }
    log
}

/// The snapshot format: the counters §17.2 names, then the matrix.
fn render(view: &GameView, placements: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!("seed {SEED}, {placements} placements\n"));
    out.push_str(&format!(
        "score {}  level {}  lines {}  pieces {}\n",
        view.score, view.level, view.lines, view.pieces
    ));
    out.push_str(&format!(
        "state {:?}  combo {}  b2b {}\n",
        view.state, view.combo, view.back_to_back
    ));
    let next: String = view.next.iter().map(|k| k.glyph()).collect();
    out.push_str(&format!("next {next}\n"));
    out.push_str("+----------+\n");
    for row in &view.rows {
        out.push('|');
        for cell in row {
            out.push(cell.map_or('.', |kind| kind.glyph()));
        }
        out.push_str("|\n");
    }
    out.push_str("+----------+\n");
    out
}

#[test]
fn a_scripted_game_matches_the_checked_in_snapshot() {
    let log = recorded_log();
    let view = play(RulesConfig::default(), SEED, &log, 1);
    let rendered = render(&view, TICKS / SLOT);

    // Stage 7 changes the score, and so has to regenerate this. Setting
    // UPDATE_SNAPSHOT is how, and reading the diff before committing it is the
    // point: a snapshot that moves without a reason is the bug this test exists
    // to catch.
    if std::env::var_os("UPDATE_SNAPSHOT").is_some() {
        std::fs::write("tests/snapshots/scripted_game.txt", &rendered)
            .expect("the snapshot is writable");
        return;
    }

    assert_eq!(
        rendered, SNAPSHOT,
        "\nthe scripted game diverged from its snapshot; \
         re-run with UPDATE_SNAPSHOT=1 once you know why",
    );
}

/// The high-gravity snapshot (§9.9, level 15).
const HIGH_GRAVITY_SNAPSHOT: &str = include_str!("snapshots/scripted_game_high_gravity.txt");

#[test]
fn the_high_gravity_game_matches_its_checked_in_snapshot() {
    // The companion to the snapshot above, and the only test in the suite where
    // gravity decides where a piece lands. The §17.2 log hard drops every
    // placement, so nothing in it exercises a fall of more than a row a tick,
    // the accumulator that pays one out, or a lock delay that has to expire on
    // its own -- all of which §9.9 and §9.11 specify and only this fixture
    // plays through.
    let log = high_gravity_log();
    let view = play(high_gravity_rules(), SEED, &log, 1);
    let rendered = render(&view, log.len() / HIGH_GRAVITY_SLOT);

    if std::env::var_os("UPDATE_SNAPSHOT").is_some() {
        std::fs::write("tests/snapshots/scripted_game_high_gravity.txt", &rendered)
            .expect("the snapshot is writable");
        return;
    }

    assert_eq!(
        rendered, HIGH_GRAVITY_SNAPSHOT,
        "\nthe high-gravity game diverged from its snapshot; \
         re-run with UPDATE_SNAPSHOT=1 once you know why",
    );
}

#[test]
fn the_high_gravity_game_is_worth_snapshotting() {
    // The same guard the log above gets, and it earns it twice over: a level-15
    // fixture that buried itself in ten pieces would still produce a stable
    // snapshot and would prove nothing about gravity at all.
    let log = high_gravity_log();
    let view = play(high_gravity_rules(), SEED, &log, 1);
    assert_ne!(view.state, PlayState::ToppedOut, "the game ran to the end");
    // Two more pieces than placements: a slot is sixty ticks, and once the
    // stack is tall enough that a piece lands as it spawns, thirty of §9.11's
    // lock delay are all it needs, so a second piece can arrive and lock
    // unsteered before the next placement's tick. That is the log being played
    // faster than it was written, not a placement going missing.
    assert_eq!(view.pieces, 109, "the whole log was played");
    assert!(view.lines >= 25, "lines: {}", view.lines);
    assert_eq!(view.level, 15, "the speed never dropped: {}", view.level);
}

#[test]
fn the_scripted_game_is_worth_snapshotting() {
    // A snapshot of a game that topped out on its third piece would pass for
    // ever and prove nothing, so the fixture's own reach is asserted here.
    let log = recorded_log();
    let view = play(RulesConfig::default(), SEED, &log, 1);
    assert_ne!(view.state, PlayState::ToppedOut, "the game ran to the end");
    assert_eq!(view.pieces, 240, "every placement was played");
    assert!(view.lines >= 90, "lines: {}", view.lines);
    assert!(view.level >= 9, "level: {}", view.level);
}

#[test]
fn every_point_of_the_score_was_announced() {
    // §12.8: the events are a faithful notification of what the rules did, so
    // the running total must be exactly the `ScoreAwarded` points, and each of
    // the §9.14 reasons that this log earns must actually appear. It is the
    // cheapest guard against a score that moves without saying why -- which is
    // the half of a snapshot diff the snapshot itself cannot explain.
    let log = recorded_log();
    let mut game = Game::new(RulesConfig::default(), SEED);
    let mut events = Vec::new();
    for input in &log {
        game.tick(input, &mut events);
    }

    let mut total = 0;
    let mut reasons = BTreeMap::new();
    for event in &events {
        if let GameEvent::ScoreAwarded { points, reason } = event {
            total += points;
            *reasons.entry(format!("{reason:?}")).or_insert(0u64) += points;
        }
    }
    assert_eq!(total, game.view().score, "breakdown: {reasons:?}");
    for reason in ["HardDrop", "SoftDrop", "Combo"] {
        assert!(reasons.contains_key(reason), "no {reason}: {reasons:?}");
    }
    assert!(
        reasons.keys().any(|r| r.starts_with("LineClear")),
        "no rows were scored: {reasons:?}",
    );
}

#[test]
fn the_same_log_batched_differently_gives_the_same_game() {
    // §19.4, the desync canary, and the reason it is in CI from this commit
    // onwards. 1 x 6 versus 6 x 1, and every batching in between: if a rules
    // decision ever depends on how the shell paced its calls, the core cannot
    // be run authoritatively anywhere else, and this is the cheapest test that
    // says so.
    //
    // Both logs, because they stress it differently. The §17.2 log decides
    // every landing with a hard drop, which is one tick's work however the
    // ticks were grouped; the high-gravity log leaves the landings to §9.9's
    // accumulator, which carries a fraction of a row from each tick to the
    // next, and that is the state a batching bug would most easily disturb.
    for (name, rules, log) in [
        ("the recorded log", RulesConfig::default(), recorded_log()),
        (
            "the high-gravity log",
            high_gravity_rules(),
            high_gravity_log(),
        ),
    ] {
        let one_at_a_time = play(rules.clone(), SEED, &log, 1);
        for batch in [2, 3, 5, 6, 7, 60, 600] {
            let batched = play(rules.clone(), SEED, &log, batch);
            assert_eq!(
                batched, one_at_a_time,
                "{name} fed {batch} ticks at a time is a different game",
            );
        }
    }
}

#[test]
fn the_seed_and_the_rules_both_decide_the_game() {
    // The other half of determinism: identical inputs give identical games, and
    // nothing else does.
    let log = recorded_log();
    let base = play(RulesConfig::default(), SEED, &log, 1);
    assert_eq!(base, play(RulesConfig::default(), SEED, &log, 1));
    assert_ne!(base, play(RulesConfig::default(), SEED + 1, &log, 1));

    // `start_level` deliberately is *not* the knob turned here. This log hard
    // drops every piece, so gravity never decides where anything lands, and the
    // level is recomputed from the line count (§9.9) rather than counted up
    // from where it started -- so starting at level 5 and starting at level 1
    // arrive at the same board, and the same level. That is the rules working,
    // not a determinism failure. `lines_per_level` does change the outcome.
    let shallower = RulesConfig {
        lines_per_level: 20,
        ..RulesConfig::default()
    };
    assert_ne!(base, play(shallower, SEED, &log, 1));
}
