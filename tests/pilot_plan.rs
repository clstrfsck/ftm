//! P6's two checks on the planner, from outside the crate: what it decides, and
//! what it is allowed to know while deciding it (`PILOT.md` §P6, §P9 C2).
//!
//! Outside the crate on purpose. `src/pilot/`'s own tests are held to §P3.4's
//! list — the core's façade, §P2.3's seam and `RulesConfig`, and nothing else —
//! and the hidden-future check needs to look at a game's **bag**, which is
//! exactly the thing the planner may not do. A test that reached for it from in
//! there would be testing a module allowed to do something the module under test
//! is not, so it lives here, where `Game`'s public surface is all there is.
//!
//! The snapshot is `tests/snapshots/pilot_plan.txt` and is **expected to move**
//! when the weights, the generator, the tie-breaks or the search change (§P8.3).
//! It is the opposite of §17.2's I1 snapshot in that one respect: a diff here is
//! something to read and understand, not a bug by itself. Regenerate it with
//! `UPDATE_SNAPSHOT=1 cargo test --test pilot_plan` — and read the diff.

use std::collections::HashMap;

use ftm::core::{Action, Game, GameEvent, GameView, PieceKind, PlayState, TickInput};
use ftm::pilot::{Pilot, Settings};
use ftm::shell::config::RulesConfig;

/// The checked-in plan snapshot (§P8.3).
const SNAPSHOT: &str = include_str!("snapshots/pilot_plan.txt");

/// The seeds it records, and how many pieces of each.
const SEEDS: [u64; 3] = [0, 42, 2024];
const PIECES: u32 = 24;

/// §6.3's defaults with the two settings these tests vary.
fn rules(preview_count: u8, hold_enabled: bool) -> RulesConfig {
    RulesConfig {
        preview_count,
        hold_enabled,
        ..RulesConfig::default()
    }
}

/// One piece's plan, as the game received it.
struct Planned {
    kind: PieceKind,
    inputs: Vec<TickInput>,
}

/// Play `pieces` pieces of a planned game and record every input, piece by
/// piece.
///
/// §15.2's order with no screen and no clock — `input`, `tick`, `observe` —
/// which is what `App::advance` runs a tick of and what §P8's benchmark runs a
/// batch of. The plan for a piece is taken to be every input the planner emitted
/// while that piece was in play, which is how a spectator would see it: the
/// controller's internal plan is not public and does not need to be.
fn record(rules: &RulesConfig, settings: Settings, seed: u64, pieces: u32) -> (Vec<Planned>, Game) {
    let mut game = Game::new(rules.clone(), seed);
    let mut pilot = Pilot::new(rules, settings);
    let mut events = Vec::new();
    let mut planned: Vec<Planned> = Vec::new();
    while game.view().pieces < pieces && game.state() != PlayState::ToppedOut {
        let view = game.view();
        let input = pilot.input(&game);
        if let Some(piece) = view.current {
            // Grouped by `pieces`, which counts locks: every tick a piece is in
            // play belongs to the plan for that piece, and a hold inside a plan
            // changes what is in play without starting a new one (§P3.1). The
            // kind recorded is therefore the one the plan was made for.
            let at = usize::try_from(view.pieces).expect("a piece count fits");
            if planned.len() == at {
                planned.push(Planned {
                    kind: piece.kind,
                    inputs: Vec::new(),
                });
            }
            if let Some(entry) = planned.get_mut(at) {
                entry.inputs.push(input);
            }
        }
        events.clear();
        game.tick(&input, &mut events);
        pilot.observe(&events);
    }
    (planned, game)
}

/// A plan as one word: what the pilot pressed, a tick to a character.
///
/// `H` hold, `R` and `L` the quarter turns, `2` a 180, `<` and `>` a shift of
/// §P3.2's one cell, `D` the hard drop, `.` a tick it pressed nothing — there is
/// no soft drop, because §10.1's is a held key rather than an `Action` and
/// §P4.1's generator never asks for one. §P3.2's
/// cap is what makes this legible at all — one action or one cell per tick means
/// one character per tick.
fn word(inputs: &[TickInput]) -> String {
    inputs
        .iter()
        .map(|input| {
            if let Some(action) = input.actions.iter().next() {
                return match action {
                    Action::Hold => 'H',
                    Action::RotateCw => 'R',
                    Action::RotateCcw => 'L',
                    Action::Rotate180 => '2',
                    Action::HardDrop => 'D',
                    // §P7.3's spectator keys and §13's menu ones. Listed rather
                    // than wildcarded: a planner that emitted one of these would
                    // be a bug, and a new *gameplay* action must break this match
                    // rather than quietly render as a question mark.
                    Action::Pause
                    | Action::Restart
                    | Action::Quit
                    | Action::MenuUp
                    | Action::MenuDown
                    | Action::MenuSelect
                    | Action::MenuBack => {
                        panic!("a plan pressed {action:?}, which is not the game's")
                    }
                };
            }
            match input.shift {
                Some(ftm::core::Shift::Left) => '<',
                Some(ftm::core::Shift::Right) => '>',
                None => '.',
            }
        })
        .collect()
}

/// The snapshot: three seeds, the plan for each piece, and where each game got
/// to.
fn render(settings: Settings) -> String {
    let rules = rules(5, true);
    let mut out = String::new();
    out.push_str("PILOT.md §P6 -- fixed-seed plans\n\n");
    out.push_str(&format!(
        "rules    preview {}  hold on  180 on\nsearch   depth {}  beam {}  nodes {}\n",
        rules.preview_count, settings.depth, settings.beam, settings.nodes,
    ));
    for seed in SEEDS {
        let (planned, game) = record(&rules, settings, seed, PIECES);
        let view = game.view();
        out.push_str(&format!("\nseed {seed}\n"));
        for (at, piece) in planned.iter().enumerate() {
            out.push_str(&format!(
                "  {:>3}  {:?}  {}\n",
                at + 1,
                piece.kind,
                word(&piece.inputs),
            ));
        }
        out.push_str(&format!(
            "  ==   {} pieces  {} lines  {} score  level {}\n",
            view.pieces, view.lines, view.score, view.level,
        ));
    }
    out
}

#[test]
fn the_plans_match_their_checked_in_snapshot() {
    let rendered = render(Settings::default());
    if std::env::var_os("UPDATE_SNAPSHOT").is_some() {
        std::fs::write("tests/snapshots/pilot_plan.txt", &rendered)
            .expect("the snapshot directory is checked in");
        return;
    }
    assert_eq!(
        rendered, SNAPSHOT,
        "\nthe plans diverged from their snapshot; re-run with UPDATE_SNAPSHOT=1 \
         once you know why. Unlike §17.2's I1 this one is *expected* to move when \
         the weights, the generator, the tie-breaks or the search change (§P8.3)",
    );
}

#[test]
fn the_same_seed_and_settings_plan_the_same_game_twice() {
    // §P9's C3 from outside: the planner is a pure function of the fair state and
    // its settings, so nothing in a second run of one process can differ either.
    assert_eq!(render(Settings::default()), render(Settings::default()));
}

#[test]
fn every_input_a_plan_emits_honours_the_cap() {
    // §P9's C5, at the boundary the game sees. It is also what makes `word`
    // above a faithful rendering rather than a lossy one: a tick that carried two
    // actions would be drawn as one of them.
    let rules = rules(5, true);
    let (planned, _) = record(&rules, Settings::default(), 42, PIECES);
    for piece in &planned {
        for input in &piece.inputs {
            assert!(input.actions.iter().count() <= 1, "{input:?}");
            assert!(input.shift_cells <= 1, "{input:?}");
        }
    }
}

/// Every piece a seed deals, in order, for as long as asked.
///
/// Hold is off so that every `PieceSpawned` is a deal (§P2.4), and the first
/// piece is read from the view because `Game::new` spawns it during construction
/// and discards its event. The planner is what plays, because a game of hard
/// drops tops out after eleven pieces and a bag is seven.
fn deals(seed: u64, count: usize) -> Vec<PieceKind> {
    let rules = rules(5, false);
    let mut game = Game::new(rules.clone(), seed);
    let mut pilot = Pilot::new(
        &rules,
        Settings {
            depth: 1,
            ..Settings::default()
        },
    );
    let mut events = Vec::new();
    let mut dealt = vec![game.view().current.expect("a piece is in play").kind];
    while dealt.len() < count && game.state() != PlayState::ToppedOut {
        let input = pilot.input(&game);
        events.clear();
        game.tick(&input, &mut events);
        pilot.observe(&events);
        dealt.extend(events.iter().filter_map(|event| match event {
            GameEvent::PieceSpawned(kind) => Some(*kind),
            _ => None,
        }));
    }
    dealt.truncate(count);
    dealt
}

/// Two seeds that look identical and are not (§P9's C2).
///
/// **Identical visible information** at the first piece — the same piece in play
/// and the same `preview` previews, on the same empty board — and a **different
/// hidden future** behind it. §9.6 is what makes both halves reachable: six
/// pieces seen out of a bag of seven fixes the whole of the open bag, so the only
/// place two such seeds can still differ is the bag *after* it, which no preview
/// of §6.3's six can reach and which `Game::bag_remaining` does not show either.
///
/// Found by search rather than constructed, because a board cannot be posed from
/// out here: two seeds agreeing on six pieces is one chance in 5,040, so a few
/// thousand candidates is plenty and each costs a `Game::new`.
///
/// It always searches at §6.3's **widest** preview, whatever preview the caller
/// then plays at, and that is what makes the pair worth having: six pieces seen
/// out of a bag of seven fixes the seventh too, so a pair agreeing on six agrees
/// on the whole first bag — seven deals of head start for the comparison,
/// instead of the two a preview of 1 would have found on its own. §9.6's
/// sequence belongs to the seed and not to the rules, so a pair found at one
/// preview is a pair at every other.
fn a_matching_pair() -> (u64, u64) {
    let preview = 6u8;
    let visible = usize::from(preview) + 1;
    let mut seen: HashMap<Vec<PieceKind>, u64> = HashMap::new();
    for seed in 0..20_000u64 {
        let game = Game::new(rules(preview, true), seed);
        let view = game.view();
        let mut key = vec![view.current.expect("a piece is in play").kind];
        key.extend(view.next.iter().copied());
        assert_eq!(key.len(), visible);
        let Some(&earlier) = seen.get(&key) else {
            seen.insert(key, seed);
            continue;
        };
        // The same front, and the futures have to genuinely part somewhere
        // beyond it or the assertion below would be about nothing.
        if deals(earlier, 16) != deals(seed, 16) {
            return (earlier, seed);
        }
    }
    panic!("no two seeds in 20,000 shared their first {visible} pieces");
}

#[test]
fn a_plan_cannot_see_a_hidden_future() {
    // §P9's C2, and `PILOT-PLAN.md` P6's isolation test. The type settles this
    // already — a `SearchGame` holds a scripted queue and no generator, and
    // `core/search.rs` asserts that at the fork — so this is belt and braces one
    // layer up, over the whole planner: the fair state is the same, so the plan
    // is the same, whatever the two games were about to deal.
    //
    // Both of §6.3's interesting previews. At five the search never reaches past
    // the visible queue; at one the second ply is a chance node (§P6.1), so the
    // hypotheses are in play too — and they are arithmetic over pieces *seen*
    // dealt, which is why they are the same on both sides.
    for preview in [5u8, 1] {
        let (left, right) = a_matching_pair();
        assert_ne!(left, right);
        let rules = rules(preview, true);

        let (one, first) = record(&rules, Settings::default(), left, PIECES);
        let (two, second) = record(&rules, Settings::default(), right, PIECES);

        // The plans are the same while the visible information is, and the
        // comparison has to stop exactly where that stops being true — the two
        // games are *different games*, and once the preview reaches into the
        // second bag they are entitled to be played differently.
        //
        // So the bound is derived rather than assumed. The seeds agree for
        // `agreed` deals; a plan for piece `k` can see as far as piece
        // `k + preview` in the queue, and one more than that because a hold
        // shifts what the queue has reached (§9.7). Comparable while
        // `k + preview + 1 < agreed`.
        //
        // An earlier version of this test compared `preview + 1` pieces, which
        // is not that number and was too many. It passed anyway, because the
        // weights of the day happened to choose the same moves for a while
        // afterwards; the first weight change exposed it. A bound that is a
        // guess is a test that fails on a day nothing broke.
        let agreed = deals(left, 16)
            .iter()
            .zip(&deals(right, 16))
            .take_while(|(left, right)| left == right)
            .count();
        let alike = agreed.saturating_sub(usize::from(preview) + 1);
        assert!(
            alike >= 1,
            "preview {preview}: {agreed} agreed deals leaves nothing to compare",
        );
        for (at, (one, two)) in one.iter().zip(&two).take(alike).enumerate() {
            assert_eq!(one.kind, two.kind, "preview {preview}, piece {at}");
            assert_eq!(
                word(&one.inputs),
                word(&two.inputs),
                "preview {preview}, piece {at}: the plan followed the hidden future",
            );
        }
        // ...and the two games really were different games, or there was nothing
        // for a planner to be tempted by.
        assert_ne!(
            first.view().next,
            second.view().next,
            "preview {preview}: the futures never parted",
        );
    }
}

#[test]
fn a_planner_is_not_told_what_it_is_not_shown() {
    // The other half of C2, stated over the one accessor that would do it. A
    // `Game` hands out §9.6's bag remainder — `Pilot` is given the whole `Game`
    // and could read it — and what makes that safe is that everything a plan is
    // built from goes through a fork of the *visible* queue. So: two games whose
    // views agree and whose bags do not must plan alike, which is the assertion
    // above; what this one adds is that such games exist and the difference is
    // real rather than assumed.
    let (left, right) = a_matching_pair();
    let futures = |seed| deals(seed, 16);
    assert_ne!(futures(left), futures(right), "the hidden futures differ");
    let visible = |seed| {
        let view: GameView = Game::new(rules(5, true), seed).view();
        (view.current.map(|piece| piece.kind), view.next, view.rows)
    };
    assert_eq!(
        visible(left),
        visible(right),
        "and the visible state does not"
    );
}
