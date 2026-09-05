//! I4 (§17.2): rendering does not panic at 60 × 24, 80 × 24, 200 × 60 or 1 × 1.
//!
//! §12.1 makes 60 × 24 the minimum supported size and replaces every screen
//! below it, so two of the four sizes here are *below* the minimum by design:
//! the point of the test is that the replacement screen is reached rather than
//! a layout that assumes room it has not got. 1 × 1 is the degenerate case, and
//! it has to survive too — a terminal can be dragged through it.
//!
//! Every screen the program can be showing is drawn at every size: the attract
//! screen and its three sub-screens, the playing screen with each of §12.6's
//! overlays, with and without the debug strip, and in all four colour depths
//! (§12.3). Nothing is asserted about how they look; §17.2 asks only that they
//! render.

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use termino::config::ConfigFile;
use termino::core::{Action, Actions, Game, GameEvent, GameView, TickInput};
use termino::highscore::{Entry, Table};
use termino::input::InputMode;
use termino::ui::attract::{self, Attract};
use termino::ui::theme::{Depth, Glyphs, Theme};
use termino::ui::{Chrome, Cosmetics, Debug, Hud, Overlay};

/// The four sizes of §17.2, exactly.
const SIZES: [(u16, u16); 4] = [(60, 24), (80, 24), (200, 60), (1, 1)];

/// Draw one frame at `size` through a `TestBackend` and let any panic through.
fn at(size: (u16, u16), draw: impl FnOnce(&mut ratatui::Frame)) {
    let backend = TestBackend::new(size.0, size.1);
    let mut terminal = Terminal::new(backend).expect("a test terminal");
    terminal.draw(|frame| draw(frame)).expect("a frame");
}

fn chrome(depth: Depth, show_grid: bool, hold_enabled: bool) -> Chrome {
    Chrome {
        theme: Theme::with_glyphs(depth, Glyphs::DEFAULT),
        show_grid,
        hold_enabled,
    }
}

/// A game with something in it: a stack, a hold piece, a full preview queue and
/// a score, so the boxes have content to lay out rather than blanks.
fn played() -> Game {
    let (rules, _) = ConfigFile::default().resolve();
    let mut game = Game::new(rules, 42);
    let mut events: Vec<GameEvent> = Vec::new();
    for piece in 0..40 {
        let mut actions = Actions::default();
        // Hold the first piece, so the hold box is occupied for the rest.
        if piece == 0 {
            let _ = actions.push(Action::Hold);
        }
        let _ = actions.push(if piece % 3 == 0 {
            Action::RotateCw
        } else {
            Action::HardDrop
        });
        game.tick(
            &TickInput {
                actions,
                ..TickInput::default()
            },
            &mut events,
        );
        // Long enough for the entry delay and any line-clear pause to run out.
        for _ in 0..40 {
            game.tick(&TickInput::default(), &mut events);
        }
    }
    game
}

/// Every overlay of §12.6, including none at all.
fn overlays() -> Vec<Overlay> {
    vec![
        Overlay::None,
        Overlay::Paused { selected: 0 },
        Overlay::Options { selected: 3 },
        Overlay::Controls,
        Overlay::Resuming { count: 2 },
        Overlay::GameOver,
        Overlay::NameEntry {
            rank: 1,
            name: "A PLAYER".to_string(),
        },
    ]
}

#[test]
fn the_playing_screen_renders_at_every_size() {
    let game = played();
    let view: GameView = game.view();
    let config = ConfigFile::default();
    let fx = Cosmetics::new(Duration::from_millis(250), Instant::now());
    let debug = Debug {
        fps: 60,
        dropped: 0,
        das_charge: 40,
        mode: InputMode::Legacy,
        core: game.debug(),
    };
    for size in SIZES {
        for overlay in overlays() {
            for strip in [None, Some(&debug)] {
                let chrome = chrome(Depth::Truecolor, true, true);
                let hud = Hud {
                    overlay: &overlay,
                    config: &config,
                    debug: strip,
                    mode: InputMode::Enhanced,
                    restart: Some(45),
                };
                at(size, |frame| {
                    termino::ui::draw(frame, &view, &chrome, &fx, &hud);
                });
            }
        }
    }
}

#[test]
fn every_colour_depth_renders_at_every_size() {
    // §12.3: `mono` is a playable path, not a degraded one, so it is drawn at
    // the same four sizes as the rest.
    let view = played().view();
    let config = ConfigFile::default();
    let fx = Cosmetics::new(Duration::from_millis(250), Instant::now());
    for size in SIZES {
        for depth in [Depth::Truecolor, Depth::Ansi256, Depth::Ansi16, Depth::Mono] {
            for hold_enabled in [true, false] {
                let chrome = chrome(depth, false, hold_enabled);
                let hud = Hud {
                    overlay: &Overlay::None,
                    config: &config,
                    debug: None,
                    mode: InputMode::Enhanced,
                    restart: None,
                };
                at(size, |frame| {
                    termino::ui::draw(frame, &view, &chrome, &fx, &hud);
                });
            }
        }
    }
}

#[test]
fn the_attract_screen_and_its_sub_screens_render_at_every_size() {
    // A full table of long names, which is the widest the high-score
    // sub-screen ever has to lay out.
    let mut scores = Table::default();
    let finished = played().view();
    for _ in 0..10 {
        scores.insert(Entry::of(
            "A VERY LONG NAME",
            &finished,
            "2026-09-05".to_string(),
        ));
    }
    let mut config = ConfigFile::default();
    let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
    let now = Instant::now();
    // The menu, then each of §13.5's three sub-screens over it.
    let opened: [&[KeyCode]; 4] = [
        &[],
        &[KeyCode::Down, KeyCode::Enter],
        &[KeyCode::Down, KeyCode::Down, KeyCode::Enter],
        &[KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Enter],
    ];
    for keys in opened {
        let mut state = Attract::new(now);
        for key in keys {
            state.key(&press(*key), &mut config, now);
        }
        // A step with the background running, so the drifting pieces of §13.4
        // are on screen for the sizes that have room for them.
        state.step(now + Duration::from_secs(3), (100, 60), true);
        for size in SIZES {
            for depth in [Depth::Truecolor, Depth::Mono] {
                let chrome = chrome(depth, false, true);
                let cx = attract::Context {
                    chrome: &chrome,
                    config: &config,
                    scores: &scores,
                    recent: Some(0),
                    mode: InputMode::Enhanced,
                };
                at(size, |frame| attract::draw(frame, &state, &cx));
            }
        }
    }
}

#[test]
fn the_terminal_too_small_screen_renders_at_every_size() {
    // §12.1: it is what the other three tests fall through to below the
    // minimum, and it is drawn directly here so that a size *above* the
    // minimum — where nothing would normally reach it — is covered too.
    for size in SIZES {
        at(size, |frame| {
            termino::ui::too_small(frame, Theme::new(Depth::Truecolor));
        });
    }
}

#[test]
fn the_minimum_terminal_gets_the_real_screen_and_one_short_of_it_does_not() {
    // The boundary I4 exists to protect: at exactly 60 x 24 the playing screen
    // is drawn, and one column narrower it is replaced (§12.1, §8.4).
    let view = played().view();
    let config = ConfigFile::default();
    let fx = Cosmetics::new(Duration::from_millis(250), Instant::now());
    let chrome = chrome(Depth::Truecolor, false, true);
    let shown = |width, height| {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("a test terminal");
        let hud = Hud {
            overlay: &Overlay::None,
            config: &config,
            debug: None,
            mode: InputMode::Enhanced,
            restart: None,
        };
        terminal
            .draw(|frame| termino::ui::draw(frame, &view, &chrome, &fx, &hud))
            .expect("a frame");
        let buffer = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let full = shown(termino::ui::MIN_WIDTH, termino::ui::MIN_HEIGHT);
    assert!(
        full.contains("SCORE"),
        "the stats box is on screen:\n{full}"
    );
    assert!(!full.contains("Terminal too small"));

    let cramped = shown(termino::ui::MIN_WIDTH - 1, termino::ui::MIN_HEIGHT);
    assert!(
        cramped.contains("Need 60x24, have 59x24"),
        "one column short is replaced:\n{cramped}",
    );
    assert!(!cramped.contains("SCORE"));
}
