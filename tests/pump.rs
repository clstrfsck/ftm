//! The shell driven headlessly (`EGUI-PLAN.md` stage G4, `FRONTEND.md` F7).
//!
//! `tools/drive.py` can play a whole game, but only through a pty against the
//! release binary; §17.1 is core-only by design, and what `cargo test` reaches
//! of `tui/` it reaches through a `TestBackend`. This is the third thing: the
//! state *above* the core, pumped the way a front-end pumps it, with no screen
//! and no clock at all.
//!
//! It exists because §15.2's steps stopped being the body of a `while` loop in
//! G4. A front-end now decides when to call [`Round::advance`] and how often,
//! and the property that has to survive that is §19.2's: **the same inputs at
//! the same moments give the same game, however the front-end was scheduled**.
//! The batch-invariance canary of §19.4 asserts the same thing one layer down,
//! over ticks; this asserts it over frames.
//!
//! Nothing here is terminal-specific, so nothing here is feature-gated: it is
//! the test a fourth front-end inherits for free.

use std::time::Duration;

use ftm::core::GameView;
use ftm::shell::config::{self, ConfigFile, MAX_CATCH_UP_TICKS, Startup, TICK};
use ftm::shell::host::Host;
use ftm::shell::input::InputMode;
use ftm::shell::keys::{Key, KeyEvent};
use ftm::shell::menus::{Overlay, Setting};
use ftm::shell::round::Round;
use ftm::shell::session::{Next, Session};
use ftm::shell::storage::{Memory, Slot, Storage};
use ftm::shell::time::Stamp;

/// §14's date stamp, fixed (`FRONTEND.md` F4).
fn date() -> String {
    "2026-09-11".to_string()
}

/// A session over a store that never touches a file.
///
/// `seeded` is §6.4's flag and it does two things: a seeded run replays the
/// same game on every restart, and it is never written to §14's table. The
/// seed itself is constant either way, so an unseeded session here is still a
/// reproducible game — it is only the recording rule that differs.
fn session(storage: &mut dyn Storage, seeded: bool) -> Session<'_> {
    let file = ConfigFile::default();
    let startup = Startup {
        on_disk: file.clone(),
        file,
        existed: false,
        wrote_config: false,
        seed: 42,
        seeded,
        warnings: Vec::new(),
    };
    Session::new(
        &startup,
        InputMode::Enhanced,
        Host::new(storage, || 42, date),
    )
}

fn press(key: Key) -> KeyEvent {
    KeyEvent::press(key)
}

fn release(key: Key) -> KeyEvent {
    KeyEvent::release(key)
}

fn at(micros: u64) -> Stamp {
    Stamp::from_micros(micros)
}

// ---------------------------------------------------------------------------
// Cadence invariance

/// Where the run ends, in microseconds. Every cadence stops at this exact
/// stamp, so they are compared after the same amount of *time* rather than
/// after the same number of frames.
const END: u64 = 4_000_000;

/// The keys, and the moment each arrives (§15.2 step 2).
///
/// The stamps are multiples of 50 ms, which is deliberate and is the whole
/// reason this comparison is meaningful. A key is consumed by the first tick
/// that runs at or after it reaches the shell, so "the same inputs at the same
/// moments" has to mean the same *tick* — a front-end that polled at 10 fps
/// would genuinely deliver its keys later, and would genuinely play a
/// different game. 50 ms is comfortably inside a tick, no cadence below runs
/// more than one tick in a frame, and every cadence is woken at each of these
/// stamps (§15.2 step 6 wakes a loop early on a key), so every cadence hands
/// each key to the same tick.
fn script() -> Vec<(u64, KeyEvent)> {
    let mut log = Vec::new();
    let mut push = |slot: u64, event| log.push((slot * 50_000, event));
    push(1, press(Key::Up));
    push(2, press(Key::Char(' ')));
    push(3, press(Key::Char('c')));
    push(4, press(Key::Left));
    // Held for three quarters of a second, which is well past DAS and several
    // ARR steps in: the one path above the core whose answer is resolved
    // against the wall clock rather than the tick (§10.3).
    push(19, release(Key::Left));
    push(20, press(Key::Char(' ')));
    push(21, press(Key::Char('z')));
    push(22, press(Key::Right));
    push(30, release(Key::Right));
    push(31, press(Key::Down));
    push(40, release(Key::Down));
    push(41, press(Key::Char(' ')));
    push(45, press(Key::Up));
    push(46, press(Key::Char(' ')));
    log
}

/// Play `script` at one cadence and report the view it ends on, plus the ticks
/// it threw away (§15.2 step 4).
///
/// `gaps` is the cadence, cycled: microseconds between frames, zeroes
/// included. The frame list is the cadence plus a wake-up at every key and one
/// at [`END`], so every cadence delivers each key at the same stamp and stops
/// at the same one.
fn play(gaps: &[u64], script: &[(u64, KeyEvent)]) -> (GameView, u64) {
    let mut frames: Vec<u64> = Vec::new();
    let mut now = 0u64;
    for step in 0.. {
        frames.push(now);
        if now >= END {
            break;
        }
        now += gaps[step % gaps.len()];
    }
    frames.extend(script.iter().map(|(stamp, _)| *stamp));
    frames.push(END);
    frames.sort_unstable();

    let mut storage = Memory::new();
    let mut session = session(&mut storage, true);
    let mut round = Round::new(&session, at(0));
    let mut next = 0;
    for stamp in frames {
        let now = at(stamp);
        // Step 2: every key that has arrived, one at a time, before the pump.
        while next < script.len() && script[next].0 <= stamp {
            assert_eq!(
                round.key(&mut session, &script[next].1, now),
                None,
                "the script never leaves the round",
            );
            next += 1;
        }
        // Steps 1 and 3-5.
        assert_eq!(round.advance(&mut session, now), None);
    }
    assert_eq!(next, script.len(), "every key was delivered");
    (round.frame(at(END)).view, round.debug(0).dropped)
}

#[test]
fn the_same_inputs_at_the_same_moments_give_the_same_game_at_any_cadence() {
    // The point of the whole stage. `advance` is correct at any cadence
    // because its accumulator is over real elapsed time and never over frames
    // (§15.2 step 4, §19.2) — and a GUI is called at whatever rate the
    // compositor feels like, including twice in a millisecond and not at all
    // for a second.
    let script = script();
    // 60 Hz: what the terminal front-end does.
    let (steady, dropped) = play(&[16_667], &script);
    assert_eq!(dropped, 0, "nothing was ever behind");

    // 144 Hz: more than half of these frames run no tick at all, which is what
    // makes `Pending` load-bearing rather than an edge case (§15.2 step 6).
    let (fast, fast_dropped) = play(&[6_944], &script);
    assert_eq!(fast, steady, "144 Hz plays the same game as 60 Hz");
    assert_eq!(fast_dropped, 0);

    // And a deliberately nasty one: two frames in the same microsecond, a
    // frame that is nearly a whole tick late, and everything in between.
    let (jittery, jittery_dropped) = play(&[0, 1, 11_000, 0, 3_000, 16_000, 0, 7_777], &script);
    assert_eq!(jittery, steady, "and so does a jittery one");
    assert_eq!(jittery_dropped, 0);

    // The run is worth comparing: a game that never started would pass this.
    assert!(steady.score > 0, "the script played something");
}

#[test]
fn a_front_end_that_was_suspended_runs_the_cap_and_discards_the_rest() {
    // §15.2 step 4. Ten seconds in one frame is a backgrounded browser tab,
    // which is far more common than the suspended laptop this rule was written
    // for — and resuming into six hundred ticks would be an instant death.
    let mut storage = Memory::new();
    let mut session = session(&mut storage, true);
    let mut round = Round::new(&session, at(0));
    assert_eq!(round.debug(0).dropped, 0);

    round.advance(&mut session, at(10_000_000));
    assert_eq!(
        round.debug(0).dropped,
        10 * 60 - u64::from(MAX_CATCH_UP_TICKS),
        "six ticks ran and the other 594 were thrown away",
    );

    // The arrears are gone rather than banked: the next second runs its own
    // sixty ticks and no more.
    let before = round.debug(0).dropped;
    for tick in 1..=60u64 {
        round.advance(&mut session, at(10_000_000 + tick * 16_667));
    }
    assert_eq!(round.debug(0).dropped, before, "and nothing is still owed");
}

#[test]
fn the_deadline_stays_inside_a_tick() {
    // §15.2 step 6: advice, and what it promises is that waiting this long
    // does not cost a tick. A front-end that treated it as a frame rate would
    // be told the wrong thing by a zero.
    let mut storage = Memory::new();
    let mut session = session(&mut storage, true);
    let mut round = Round::new(&session, at(0));
    for frame in 0..600u64 {
        let now = at(frame * 6_944);
        round.advance(&mut session, now);
        let deadline = round.deadline(now);
        assert!(deadline > Duration::ZERO, "never a busy loop at {frame}");
        assert!(deadline <= TICK, "never a frame rate at {frame}");
    }
}

// ---------------------------------------------------------------------------
// §7's phases, through the methods a front-end actually has

#[test]
fn pause_stops_the_clock_and_resuming_counts_back_in() {
    // §9.17, from outside: the overlay is the only thing a front-end can see,
    // and the view is the only thing it draws.
    let mut storage = Memory::new();
    let mut session = session(&mut storage, true);
    let mut round = Round::new(&session, at(0));
    let playing = round.frame(at(0)).view;
    assert_eq!(round.frame(at(0)).overlay, Overlay::None);

    assert_eq!(round.key(&mut session, &press(Key::Esc), at(100_000)), None);
    assert_eq!(
        round.frame(at(100_000)).overlay,
        Overlay::Paused { selected: 0 },
    );

    // A second of pumping while paused moves nothing.
    for frame in 1..=60u64 {
        round.advance(&mut session, at(100_000 + frame * 16_667));
    }
    assert_eq!(
        round.frame(at(1_200_000)).view,
        playing,
        "a paused game does not tick, however often it is pumped",
    );

    // §9.17's 3-2-1, and only then does the clock start again.
    let resumed = 1_200_000;
    assert_eq!(round.key(&mut session, &press(Key::Esc), at(resumed)), None);
    assert_eq!(
        round.frame(at(resumed)).overlay,
        Overlay::Resuming { count: 3 },
    );
    assert_eq!(
        round.frame(at(resumed + 1_500_000)).overlay,
        Overlay::Resuming { count: 2 },
    );
    for frame in 1..=240u64 {
        round.advance(&mut session, at(resumed + frame * 16_667));
    }
    let now = at(resumed + 240 * 16_667);
    assert_eq!(round.frame(now).overlay, Overlay::None, "back in the game");
    assert_ne!(
        round.frame(now).view,
        playing,
        "and gravity is running again",
    );
}

#[test]
fn a_viewport_below_the_minimum_pauses_the_game_and_says_so() {
    // §8.4 and `FRONTEND.md` F6: the minimum is the front-end's, the forced
    // pause is the shell's. Nothing undoes itself when there is room again —
    // the player leaves the pause, and gets the countdown for it.
    let mut storage = Memory::new();
    let session = session(&mut storage, true);
    let mut round = Round::new(&session, at(0));
    assert!(!round.frame(at(0)).cramped);

    round.viewport(false);
    let frame = round.frame(at(0));
    assert!(frame.cramped, "the front-end's message replaces the screen");
    assert_eq!(frame.overlay, Overlay::Paused { selected: 0 });

    round.viewport(true);
    let frame = round.frame(at(0));
    assert!(!frame.cramped, "there is room again");
    assert_eq!(
        frame.overlay,
        Overlay::Paused { selected: 0 },
        "but the pause is the player's to leave",
    );
}

/// The leftmost column the falling piece occupies.
fn left_edge(round: &Round, now: Stamp) -> u8 {
    let view = round.frame(now).view;
    let piece = view.current.expect("a piece is falling");
    piece.cells.iter().map(|&(col, _)| col).min().unwrap()
}

#[test]
fn losing_the_keyboard_pauses_the_game_and_lets_go_of_its_keys() {
    // `GUI.md` §G4.7 and B10: a front-end that cannot hear the keyboard forces
    // §8.4's pause, held keys released, without replacing the screen. A
    // direction held when focus went must not still be charging DAS when the
    // player comes back — the release of a key let go elsewhere never arrives.
    let mut storage = Memory::new();
    let mut session = session(&mut storage, true);

    // What holding left does if nothing intervenes: it slides to the wall.
    let mut control = Round::new(&session, at(0));
    control.key(&mut session, &press(Key::Left), at(0));
    let start = {
        control.advance(&mut session, at(50_000));
        left_edge(&control, at(50_000))
    };
    for frame in 2..=20 {
        control.advance(&mut session, at(frame * 50_000));
    }
    assert!(
        left_edge(&control, at(1_000_000)) < start,
        "held left slides"
    );

    let mut round = Round::new(&session, at(0));
    round.key(&mut session, &press(Key::Left), at(0));
    round.advance(&mut session, at(50_000));
    let before = left_edge(&round, at(50_000));

    round.keyboard(false, at(50_000));
    let frame = round.frame(at(50_000));
    assert_eq!(frame.overlay, Overlay::Paused { selected: 0 });
    assert!(
        !frame.cramped,
        "the screen is not replaced; the front-end says why"
    );

    // Suspended on every pass until focus returns, which changes nothing more.
    round.keyboard(false, at(1_000_000));
    round.advance(&mut session, at(1_000_000));
    assert_eq!(
        round.frame(at(1_000_000)).overlay,
        Overlay::Paused { selected: 0 }
    );

    // The player comes back and leaves the pause; left was never released as
    // far as this front-end heard, and still does not slide.
    round.key(&mut session, &press(Key::Esc), at(1_000_000));
    for frame in 21..=100 {
        round.advance(&mut session, at(frame * 50_000));
    }
    assert_eq!(round.frame(at(5_000_000)).overlay, Overlay::None, "resumed");
    assert_eq!(left_edge(&round, at(5_000_000)), before, "and nothing slid");
}

#[test]
fn a_panel_offers_the_rows_its_front_end_can_apply() {
    // §13.5 and `GUI.md` §G5.4: which rows the Options panel lists is the
    // front-end's answer, because a panel must offer what it can actually
    // apply — §12.3's colour depth means nothing in a window, and `GUI.md`
    // §G8.10's scale and full screen mean nothing anywhere else. Whatever the
    // list, the cursor stays inside it: a cursor that reached a row the screen
    // was not drawing would be a cursor the player could not see.
    //
    // The four lists, and what each front-end may not offer (§G8.11).
    for (list, absent) in [
        (
            &Setting::ALL[..],
            &[Setting::Scale, Setting::Fullscreen][..],
        ),
        (&Setting::WINDOW[..], &[Setting::Colour][..]),
        (
            &Setting::CANVAS[..],
            &[Setting::Colour, Setting::Fullscreen][..],
        ),
        (
            &Setting::SHARED[..],
            &[Setting::Colour, Setting::Scale, Setting::Fullscreen][..],
        ),
    ] {
        for row in absent {
            assert!(!list.contains(row), "{row:?} is not this front-end's");
        }
        for row in Setting::SHARED {
            assert!(list.contains(&row), "{row:?} is every front-end's");
        }
    }

    let mut storage = Memory::new();
    let mut session = session(&mut storage, true);
    // A window's list, which is the longest of the three and the one whose
    // extra rows are the newest (`EGUI-PLAN.md` G12).
    session.settings = &Setting::WINDOW;
    let mut round = Round::new(&session, at(0));
    assert_eq!(round.settings(), &Setting::WINDOW, "what the screen draws");

    // Into the panel: pause, down to Options, Enter.
    round.key(&mut session, &press(Key::Esc), at(0));
    for _ in 0..2 {
        round.key(&mut session, &press(Key::Down), at(0));
    }
    round.key(&mut session, &press(Key::Enter), at(0));
    assert_eq!(round.frame(at(0)).overlay, Overlay::Options { selected: 0 });

    // Up from the top wraps to the last row this front-end offers, which is
    // §G8.10's full-screen switch and not §12.3's colour depth.
    round.key(&mut session, &press(Key::Up), at(0));
    assert_eq!(
        round.frame(at(0)).overlay,
        Overlay::Options {
            selected: Setting::WINDOW.len() - 1
        },
    );
    // And changing it there edits the setting the screen is showing.
    let before = session.config.gui.fullscreen;
    round.key(&mut session, &press(Key::Right), at(0));
    assert_eq!(session.config.gui.fullscreen, !before);
    assert_eq!(
        session.config.display.color_depth,
        ConfigFile::default().display.color_depth,
        "and never the row it is not showing",
    );
}

#[test]
fn the_restart_key_ends_the_round_after_its_second() {
    // §10.1: "Restart (hold 1 s)", and the hold is `advance`'s answer rather
    // than a key's — the key that starts it is not the event that ends the
    // round.
    let mut storage = Memory::new();
    let mut session = session(&mut storage, true);
    let mut round = Round::new(&session, at(0));
    assert_eq!(round.frame(at(0)).restart, None, "the key is up");

    assert_eq!(round.key(&mut session, &press(Key::Char('r')), at(0)), None);
    assert_eq!(round.frame(at(500_000)).restart, Some(50), "halfway");
    assert_eq!(round.advance(&mut session, at(999_000)), None);
    assert_eq!(round.advance(&mut session, at(1_000_000)), Some(Next::Play));
}

#[test]
fn the_quit_key_abandons_the_game_for_the_attract_screen() {
    // §7, §16: quit from a game is not a quit from the program, and the run is
    // abandoned rather than scored (§11).
    let mut storage = Memory::new();
    let mut session = session(&mut storage, true);
    let mut round = Round::new(&session, at(0));
    assert_eq!(
        round.key(&mut session, &press(Key::Char('q')), at(0)),
        Some(Next::Attract),
    );
}

#[test]
fn a_game_played_to_a_top_out_reaches_the_table() {
    // The headless half of §17.3's A6: a whole game, its game-over box, its
    // one-second lockout, name entry, and §14's table — with no terminal, no
    // pty and no clock. `tools/drive.py` still checks the same path through
    // the release binary on a real pty; this checks it in half a second.
    let mut storage = Memory::new();
    // Unseeded, so §14 records it. The seed is constant regardless, so the
    // game is still the same one every time this test runs (§6.4).
    let mut session = session(&mut storage, false);
    let mut round = Round::new(&session, at(0));

    // Hard drop, and a couple of ticks to lock and spawn, until the stack
    // reaches the ceiling (§9.16).
    let mut now = 0u64;
    let mut drops = 0;
    while round.frame(at(now)).overlay != Overlay::GameOver {
        assert!(drops < 500, "a hard drop every other tick tops out");
        assert_eq!(
            round.key(&mut session, &press(Key::Char(' ')), at(now)),
            None
        );
        for _ in 0..4 {
            now += 16_667;
            assert_eq!(round.advance(&mut session, at(now)), None);
        }
        drops += 1;
    }
    let view = round.frame(at(now)).view;
    assert!(view.score > 0, "and it scored on the way");

    // §9.16: the box cannot be dismissed for a second, so the keypress that
    // killed you does not also clear it.
    assert_eq!(round.key(&mut session, &press(Key::Enter), at(now)), None);
    assert_eq!(round.frame(at(now)).overlay, Overlay::GameOver);

    now += 1_000_000;
    assert_eq!(round.key(&mut session, &press(Key::Enter), at(now)), None);
    let Overlay::NameEntry { rank, .. } = round.frame(at(now)).overlay else {
        panic!("a qualifying score goes to name entry (§7)");
    };
    assert_eq!(rank, 1, "the box counts from one");

    // §12.6: clear whatever was pre-filled, type a name, `Enter` files it.
    for _ in 0..ftm::shell::highscore::NAME_MAX {
        round.key(&mut session, &press(Key::Backspace), at(now));
    }
    for c in "PUMP".chars() {
        round.key(&mut session, &press(Key::Char(c)), at(now));
    }
    assert_eq!(
        round.key(&mut session, &press(Key::Enter), at(now)),
        Some(Next::Attract),
    );
    assert_eq!(session.scores.entries.len(), 1);
    assert_eq!(session.scores.entries[0].name, "PUMP");
    assert_eq!(session.scores.entries[0].score, view.score);
    assert_eq!(session.recent, Some(0), "§13.5 highlights it");

    // §14: and it was written, not merely remembered.
    drop(session);
    let written = storage.read(Slot::HighScores).expect("a readable store");
    assert!(
        written.is_some_and(|json| json.contains("PUMP")),
        "the table reached the store",
    );
}

#[test]
fn the_options_panel_edits_the_config_without_touching_the_running_game() {
    // §13.5: presentation applies at once and rules never — a game keeps the
    // rules it started under. `hold_enabled` is the one a front-end has to ask
    // the round about rather than the config, because an empty hold slot and
    // an absent hold mechanic are both `hold: None` (§12.4, §12.7).
    let mut storage = Memory::new();
    let mut session = session(&mut storage, true);
    let mut round = Round::new(&session, at(0));
    assert!(round.hold_enabled());
    let generation = round.frame(at(0)).generation;

    // Pause, then Options: the third item down (§12.6).
    round.key(&mut session, &press(Key::Esc), at(0));
    for _ in 0..2 {
        round.key(&mut session, &press(Key::Down), at(0));
    }
    round.key(&mut session, &press(Key::Enter), at(0));
    assert_eq!(round.frame(at(0)).overlay, Overlay::Options { selected: 0 });

    // Down to §13.5's Hold row, and turn it off.
    let row = Setting::ALL
        .iter()
        .position(|setting| *setting == Setting::Hold)
        .expect("§13.5 lists Hold");
    for _ in 0..row {
        round.key(&mut session, &press(Key::Down), at(0));
    }
    assert_eq!(
        round.frame(at(0)).overlay,
        Overlay::Options { selected: row }
    );
    round.key(&mut session, &press(Key::Left), at(0));
    assert!(!session.config.gameplay.hold_enabled, "the config changed");
    assert!(
        round.hold_enabled(),
        "and the running game keeps the rules it started under",
    );
    assert_ne!(
        round.frame(at(0)).generation,
        generation,
        "a front-end comparing frames is told something moved",
    );

    // §6.1: written back on the way out of the panel — and the *next* game is
    // the one that plays by it.
    round.key(&mut session, &press(Key::Esc), at(0));
    assert_eq!(
        round.frame(at(0)).overlay,
        Overlay::Paused { selected: 2 },
        "back to the item that opened the panel",
    );
    assert!(!Round::new(&session, at(0)).hold_enabled());

    drop(session);
    let mut warnings = Vec::new();
    let written = config::load(&storage, &mut warnings).file;
    assert!(
        !written.gameplay.hold_enabled,
        "§6.2's document was rewritten"
    );
    assert!(warnings.is_empty(), "{warnings:?}");
}
