//! §17.2's last requirement for the window front-end: every screen the program
//! can show renders at every viewport size without panicking (`GUI.md` §G9).
//!
//! This is `tests/render_sizes.rs` for pixels — the same test, asking the same
//! question of the other front-end — and it is deliberately shaped the same
//! way. Like that one it drives the *drawing* entry points rather than the
//! loop above them: `tui::draw` there, [`playfield::draw`] and its neighbours
//! here. `tests/pump.rs` is what drives the loop, headlessly and with no
//! screen, and the two together are what this front-end has in place of
//! `tools/drive.py` (§G9.1).
//!
//! Several of the sizes are *below* §G3.3's minimum by design, as two of the
//! terminal's four are below §12.1's: the point is that the replacement
//! message is reached rather than a layout that assumes room it has not got. A
//! window can be dragged through a size of nothing and a canvas can be squeezed
//! to one, so those have to survive too.
//!
//! Every size is drawn at several pixel densities, which is the axis a
//! character grid has not got: §G3's metric floors the cell in *physical*
//! pixels, so 1.25 is a different arithmetic from 2 and not merely a larger
//! one. It is also why the boundary below is a pixel wide rather than a point.
//!
//! Nothing is asserted about how any of it looks. §17.2 asks only that it
//! renders; what it looks like is `src/gui/`'s own tests, which measure where
//! the text landed, and B3-B11, which were checked by running the thing.

#![cfg(feature = "gui")]

use std::time::Duration;

use ftm::core::{
    Action, Actions, ClearKind, DebugView, Game, GameEvent, GameView, PieceKind, TickInput,
    TopOutCause,
};
use ftm::gui::attract::Drift;
use ftm::gui::layout::{LAYOUT_COLS, LAYOUT_ROWS, Layout, MIN_CELL, Measure};
use ftm::gui::overlays::Panels;
use ftm::gui::playfield::Chrome;
use ftm::gui::{attract, overlays, paint, playfield};
use ftm::shell::attract::Attract;
use ftm::shell::config::{ConfigFile, Startup};
use ftm::shell::cosmetics::Cosmetics;
use ftm::shell::highscore::{Entry, Table};
use ftm::shell::host::Host;
use ftm::shell::input::InputMode;
use ftm::shell::keys::{Key, KeyEvent};
use ftm::shell::menus::{MenuChoice, Overlay, Setting};
use ftm::shell::round::{Debug, FrameState};
use ftm::shell::session::Session;
use ftm::shell::storage::{Memory, Storage};
use ftm::shell::time::Stamp;

/// The sizes of `GUI.md` §G9.1, in **physical pixels** — which is what a window
/// and a canvas actually are, and what §G3's metric divides into cells.
///
/// The first two are the degenerate cases a dragged window and a squeezed
/// canvas pass through; 364 x 336 is §G3.3's minimum at 1x, and is below it at
/// every other density, which is the point of drawing it at all of them. The
/// boundary itself is a test of its own, below.
const SIZES: [(f32, f32); 7] = [
    (0.0, 0.0),
    (1.0, 1.0),
    (320.0, 240.0),
    (364.0, 336.0),
    (728.0, 672.0),
    (1920.0, 1080.0),
    (3840.0, 2160.0),
];

/// The densities of §G9.1. 1.25 is the one that matters: a fractional scale is
/// where a rect computed in points lands between pixels (§G3.1).
const DENSITIES: [f32; 4] = [1.0, 1.25, 2.0, 3.0];

/// Paint one frame of a viewport `points` points across at `ppp` physical
/// pixels to the point, into a headless `egui` context: everything a real
/// window would have been sent, with no window under it.
///
/// The density is the *viewport's*, not a zoom factor. That is what a display
/// gives a real window, and it is also the one of the two that leaves
/// `screen_rect` in points, so a case below names the size it means. §G8.10's
/// `scale_percent` is the zoom on top of it, and is the application's rather
/// than this test's.
///
/// One `Context` for a whole test, as a window has: making one lays out the
/// fonts, and that is most of the cost of every case here.
fn frame(
    ctx: &egui::Context,
    (width, height): (f32, f32),
    ppp: f32,
    mut draw: impl FnMut(&Painted),
) {
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width, height),
        )),
        ..Default::default()
    };
    input
        .viewports
        .entry(input.viewport_id)
        .or_default()
        .native_pixels_per_point = Some(ppp);
    let output = ctx.run_ui(input, |ui| {
        let area = ui.max_rect();
        let painter = ui.painter().clone();
        draw(&Painted {
            painter,
            area,
            measure: Measure::of(area, ui.ctx().pixels_per_point()),
        });
    });
    // There is no renderer to hand the font atlas to, and `egui` insists that
    // saying so is deliberate.
    output.drop_without_applying_deltas();
}

/// A size in physical pixels, as points at `ppp` — a 4K screen at 2x is a
/// 1920 x 1080 viewport, and both are the same window.
fn points((width, height): (f32, f32), ppp: f32) -> (f32, f32) {
    (width / ppp, height / ppp)
}

/// One pass's painter, the viewport it is painting into, and §G3's answer
/// about it.
struct Painted {
    painter: egui::Painter,
    area: egui::Rect,
    measure: Measure,
}

impl Painted {
    /// The layout, or the too-small message that replaces every screen below the
    /// minimum (§G3.3, §8.4) — which is what a front-end does with a `Measure`
    /// and so is what every case here does with one.
    fn layout(&self) -> Option<Layout> {
        match self.measure {
            Measure::Fits(layout) => Some(layout),
            Measure::TooSmall { need, have } => {
                paint::too_small(&self.painter, self.area, need, have);
                None
            }
        }
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
        for _ in 0..40 {
            game.tick(&TickInput::default(), &mut events);
        }
    }
    game
}

/// Every overlay of §12.6 and §13.5, including none at all.
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

/// §12.5's clear pause, as the timers are told it.
const CLEAR_DELAY: Duration = Duration::from_millis(250);

/// Every §12.5 animation at once, `at` into them.
///
/// Not a state a game reaches — a perfect clear and a top out in one tick — but
/// each of them is a `Cosmetics` field, so this is what puts all six on screen
/// at every size (§G6).
fn busy_fx(at: Duration) -> Cosmetics {
    let mut fx = Cosmetics::new(CLEAR_DELAY, Stamp::ZERO);
    fx.absorb(
        &[
            GameEvent::HardDropped { rows: 6 },
            GameEvent::PieceLocked {
                cells: [(4, 19), (5, 19), (6, 19), (5, 18)],
                kind: PieceKind::T,
            },
            GameEvent::LinesCleared {
                rows: vec![19],
                clear: ClearKind::Quad,
                b2b: true,
                combo: 3,
            },
            GameEvent::LevelUp(5),
            GameEvent::ToppedOut(TopOutCause::LockOut),
        ],
        Stamp::ZERO,
    );
    fx.absorb(&[], Stamp::ZERO + at);
    fx
}

fn frame_state(view: GameView, overlay: Overlay, cramped: bool) -> FrameState {
    FrameState {
        view,
        overlay,
        generation: 0,
        restart: Some(40),
        cramped,
    }
}

fn debug_read_out(game: &Game) -> Debug {
    Debug {
        fps: 60,
        dropped: 3,
        das_charge: 40,
        mode: InputMode::Enhanced,
        core: game.debug(),
    }
}

#[test]
fn the_playing_screen_renders_at_every_size() {
    // §G4 and §G5 together, which is what a window actually shows: the screen
    // with a box over it. Both halves of §G6 are on — an animation part-way
    // through, and a piece between two rows — because the offset is what makes
    // a cell land somewhere the grid does not put it (§G6.5).
    let ctx = egui::Context::default();
    let game = played();
    let view = game.view();
    let debug = debug_read_out(&game);
    let config = ConfigFile::default();
    let chromes = [
        Chrome {
            show_grid: true,
            hold_enabled: true,
        },
        Chrome {
            show_grid: false,
            hold_enabled: false,
        },
    ];
    for size in SIZES {
        for ppp in DENSITIES {
            for overlay in overlays() {
                for chrome in chromes {
                    let panels: [(&'static [Setting], Cosmetics); 2] = [
                        (&Setting::WINDOW, Cosmetics::new(CLEAR_DELAY, Stamp::ZERO)),
                        (&Setting::CANVAS, busy_fx(Duration::from_millis(40))),
                    ];
                    for (settings, fx) in panels {
                        let state = frame_state(view.clone(), overlay.clone(), false);
                        frame(&ctx, points(size, ppp), ppp, |painted| {
                            let Some(layout) = painted.layout() else {
                                return;
                            };
                            playfield::draw(&painted.painter, &layout, &state, chrome, &fx);
                            overlays::draw(
                                &painted.painter,
                                &layout,
                                &state,
                                &Panels {
                                    config: &config,
                                    settings,
                                },
                            );
                            playfield::debug(
                                &painted.painter,
                                painted.area,
                                &debug,
                                state.view.ticks,
                            );
                            paint::unfocused(&painted.painter, painted.area);
                        });
                    }
                }
            }
        }
    }
}

#[test]
fn the_attract_screen_and_its_sub_screens_render_at_every_size() {
    // §G7, and §13.5's three sub-screens over it — two of which are §G5's own
    // boxes, drawn over a second screen.
    let ctx = egui::Context::default();
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
    let mut storage = Memory::new();
    let now = Stamp::ZERO;
    let later = now + Duration::from_secs(3);
    // The menu, then each of §13.5's three sub-screens over it, reached the way
    // a player reaches them: through the shell's own entry point, which is what
    // both front-ends navigate (`FRONTEND.md` F7).
    let opened: [&[Key]; 4] = [
        &[],
        &[Key::Down, Key::Enter],
        &[Key::Down, Key::Down, Key::Enter],
        &[Key::Down, Key::Down, Key::Down, Key::Enter],
    ];
    for keys in opened {
        // §G7.5 and §G8.1: a desktop's menu and a tab's, which differ by
        // **QUIT** — the list the shell walks, so the cursor cannot reach a row
        // that is not drawn.
        let screens: [(&'static [MenuChoice], &'static [Setting]); 2] = [
            (&MenuChoice::ALL, &Setting::WINDOW),
            (&MenuChoice::NO_QUIT, &Setting::CANVAS),
        ];
        for (menu, settings) in screens {
            let mut session = session(&mut storage);
            session.menu = menu;
            session.settings = settings;
            let mut state = Attract::new(now);
            for key in keys {
                state.key(&mut session, &KeyEvent::press(*key), now);
            }
            state.advance(later);
            for size in SIZES {
                for ppp in DENSITIES {
                    // §13.4's drift is spread over the whole viewport rather
                    // than over the block, so it is stepped at the size in hand
                    // (§G7.3).
                    let mut drift = Drift::new(7, now);
                    frame(&ctx, points(size, ppp), ppp, |painted| {
                        let Some(layout) = painted.layout() else {
                            return;
                        };
                        let cell = layout.cell().max(1.0);
                        drift.step(
                            later,
                            (
                                (painted.area.width() / cell).ceil() as i32,
                                (painted.area.height() / cell).ceil() as i32,
                            ),
                        );
                        let cx = attract::Context {
                            config: &session.config,
                            panels: Panels {
                                config: &session.config,
                                settings,
                            },
                            menu,
                            scores: &session.scores,
                            recent: Some(0),
                        };
                        attract::draw(&painted.painter, &layout, painted.area, &state, &drift, &cx);
                        paint::unfocused(&painted.painter, painted.area);
                    });
                }
            }
        }
    }
}

#[test]
fn the_too_small_message_renders_at_every_size() {
    // §G3.3: it is what the two tests above fall through to below the minimum,
    // and it is drawn directly here so that a size *above* the minimum — where
    // nothing would normally reach it — is covered too.
    let ctx = egui::Context::default();
    for size in SIZES {
        for ppp in DENSITIES {
            frame(&ctx, points(size, ppp), ppp, |painted| {
                let have = [
                    painted.area.width().max(0.0).round() as u32,
                    painted.area.height().max(0.0).round() as u32,
                ];
                paint::too_small(&painted.painter, painted.area, [364, 336], have);
            });
        }
    }
}

#[test]
fn the_minimum_window_gets_the_real_screen_and_one_pixel_short_does_not() {
    // The boundary this test exists to protect, and the window's half of I4:
    // at exactly §G3.3's minimum the playing screen is drawn, and one point
    // narrower it is replaced (§8.4). The arithmetic is `Measure`'s and is
    // pinned by `gui::layout`'s own tests; what is checked here is that a
    // *frame* on either side of it does what the front-end does with the
    // answer.
    let ctx = egui::Context::default();
    let view = played().view();
    let fx = Cosmetics::new(CLEAR_DELAY, Stamp::ZERO);
    let chrome = Chrome {
        show_grid: true,
        hold_enabled: true,
    };
    for ppp in DENSITIES {
        // The minimum is a cell of `MIN_CELL` *points* and the cell is a whole
        // number of *pixels*, so the boundary is a pixel rather than a point:
        // fourteen points is 17.5 pixels at 1.25x and the cell has to be
        // eighteen, which makes the least window 468 x 432 pixels there against
        // 364 x 336 at 1x. The message says the same thing in points, rounded
        // up so that a player who resizes to what it asks for gets the screen —
        // which is the other half of what is checked here.
        let least = (MIN_CELL * ppp).ceil().max(1.0);
        let minimum = (
            least * LAYOUT_COLS as f32 / ppp,
            least * LAYOUT_ROWS as f32 / ppp,
        );
        let Measure::TooSmall { need, .. } = Measure::of(egui::Rect::ZERO, ppp) else {
            panic!("a viewport of nothing is too small");
        };
        assert!(
            need[0] as f32 >= minimum.0 && need[1] as f32 >= minimum.1,
            "{ppp}x: the message asks for {need:?}, which is never less than \
             the {minimum:?} it takes",
        );
        for (size, fits) in [
            (minimum, true),
            ((need[0] as f32, need[1] as f32), true),
            ((minimum.0 - 1.0 / ppp, minimum.1), false),
            ((minimum.0, minimum.1 - 1.0 / ppp), false),
        ] {
            let mut drew_screen = false;
            let state = frame_state(view.clone(), Overlay::None, !fits);
            // In points, not pixels: the minimum is stated in points and the
            // sizes above came from a `Measure`, which is handed points too.
            frame(&ctx, size, ppp, |painted| {
                assert_eq!(
                    painted.measure.fits(),
                    fits,
                    "{size:?} at {ppp}x: {:?}",
                    painted.measure
                );
                if let Some(layout) = painted.layout() {
                    playfield::draw(&painted.painter, &layout, &state, chrome, &fx);
                    drew_screen = true;
                }
            });
            assert_eq!(drew_screen, fits);
        }
    }
}

#[test]
fn the_debug_read_out_renders_with_the_widest_figures_there_are() {
    // §G4.6. It is drawn over the viewport rather than inside the block, so it
    // is the one thing on the playing screen that a size the block fits in can
    // still run out of room for.
    let ctx = egui::Context::default();
    let debug = Debug {
        fps: u32::MAX,
        dropped: u64::MAX,
        das_charge: 100,
        mode: InputMode::Legacy,
        core: DebugView {
            milli_g: u32::MAX,
            fall_period: u32::MAX,
            lock_delay: Some(u32::MAX),
            bag: PieceKind::ALL.to_vec(),
        },
    };
    for size in SIZES {
        for ppp in DENSITIES {
            frame(&ctx, points(size, ppp), ppp, |painted| {
                playfield::debug(&painted.painter, painted.area, &debug, u64::MAX);
            });
        }
    }
}

/// A session over a store that never touches a file, for the screens that are
/// drawn from one (§6.2, §14: both slots are just text).
fn session(storage: &mut dyn Storage) -> Session<'_> {
    let file = ConfigFile::default();
    let startup = Startup {
        on_disk: file.clone(),
        file,
        existed: false,
        wrote_config: false,
        seed: 42,
        seeded: true,
        warnings: Vec::new(),
    };
    Session::new(
        &startup,
        InputMode::Enhanced,
        Host::new(storage, || 42, || "2026-09-05".to_string()),
    )
}
