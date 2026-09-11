//! §G4: the playing screen — §12.4's information, drawn as pixels.
//!
//! The same information the terminal's screen carries, and no more: the well,
//! the hold slot, the next queue, the figures and the status line, over
//! §G3's cell metric. What it is *not* is a translation of §12.4's 44 x 23
//! block. Pixels leave room the characters did not, so the combo counter and
//! the back-to-back chain sit with the other figures instead of on the status
//! line, and the score is grouped in threes live, not only at rest.
//!
//! Everything is drawn from the [`FrameState`] the pump reported and nothing
//! reads `Game` (§12.7); what the view cannot answer — whether the running game
//! has a hold slot at all, whether the grid is on — arrives as a [`Chrome`],
//! exactly as it does in the terminal. §12.6's boxes are drawn over this by
//! [`overlays`](crate::gui::overlays); §12.5's animations are `EGUI.md` G9's.

use crate::core::{GameView, PieceKind, Rotation, VIEW_HEIGHT, VIEW_WIDTH};
use crate::gui::layout::{Layout, MAX_SLOTS};
use crate::gui::paint::{self, Face};
use crate::shell::cosmetics::Cosmetics;
use crate::shell::figures::{clock, thousands};
use crate::shell::palette;
use crate::shell::round::{Debug, FrameState};

/// A panel's label, in cells.
const LABEL_SIZE: f32 = 0.55;
/// A figure, in cells.
const FIGURE_SIZE: f32 = 0.85;
/// The status line, in cells.
const STATUS_SIZE: f32 = 0.8;
/// The figures in the stats panel, two and a half rows apart (§G4.4).
const FIGURE_PITCH: f32 = 2.5;
/// The debug panel's text, in points: a developer's read-out, not part of the
/// screen, so it does not scale with it (§G4.6).
const DEBUG_SIZE: f32 = 11.0;

/// The settings the screen needs that are not game state (§12.7).
///
/// `hold_enabled` is the one layout question the view cannot answer — an empty
/// hold slot and an absent hold mechanic are both `hold: None` — and it is the
/// running game's answer, [`Round::hold_enabled`], not the config's (§13.5).
///
/// [`Round::hold_enabled`]: crate::shell::round::Round::hold_enabled
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chrome {
    pub show_grid: bool,
    pub hold_enabled: bool,
}

/// Draw one frame of the playing screen.
pub fn draw(
    painter: &egui::Painter,
    layout: &Layout,
    state: &FrameState,
    chrome: Chrome,
    fx: &Cosmetics,
) {
    let view = &state.view;
    well(
        painter,
        layout,
        view,
        chrome.show_grid,
        state.overlay.blanks(),
    );
    // §12.4: with hold off the hold panel is omitted entirely, not drawn empty,
    // and the stats take its place at the top of the column.
    if chrome.hold_enabled {
        hold(painter, layout, view);
    }
    stats(painter, layout, view, chrome.hold_enabled);
    next(painter, layout, view);
    status(painter, layout, fx, state.restart);
    // What goes over the screen is `overlays`' (§12.6, §G5), and the front-end
    // draws it after this: a box is not part of the playing screen, and the
    // screen must be complete underneath it.
}

/// §G4.1: the well, its walls, and what is in it.
///
/// `blanked` is §9.17's anti-pause-scumming rule, and it is the game's rather
/// than this screen's ([`Overlay::blanks`]): while paused the well is drawn
/// empty, grid and all, and the player gets no free look at the stack.
fn well(painter: &egui::Painter, layout: &Layout, view: &GameView, grid: bool, blanked: bool) {
    let well = layout.well();
    painter.rect_filled(well, 0.0, paint::PANEL);
    // Walls and a floor, and no lid: the row above the well is the mouth a
    // piece comes in through (§12.4). An eighth of a cell, on whole pixels.
    let thickness = (layout.cell_pixels() / 8).max(1);
    for wall in layout.walls(well, thickness) {
        painter.rect_filled(wall, 0.0, paint::WALL);
    }
    if grid {
        for row in 0..VIEW_HEIGHT as u8 {
            for col in 0..VIEW_WIDTH as u8 {
                paint::mino(painter, layout, layout.field_cell(col, row), paint::GRID);
            }
        }
    }
    if blanked {
        return;
    }
    for (row, cells) in view.rows.iter().enumerate() {
        for (col, cell) in cells.iter().enumerate() {
            if let Some(kind) = cell {
                let at = layout.field_cell(col as u8, row as u8);
                paint::mino(painter, layout, at, paint::piece(*kind, palette::FULL));
            }
        }
    }
    // §9.8: the ghost goes down before the falling piece, so that where the two
    // overlap the piece is what is drawn. Either may be absent — the ghost when
    // `ghost_piece` is off, the piece during the clear and entry delays — and
    // that is the view's answer, not a special case here (§12.7).
    for (piece, percent) in [
        (&view.ghost, palette::GHOST),
        (&view.current, palette::FULL),
    ] {
        let Some(piece) = piece else { continue };
        for &(col, row) in &piece.cells {
            // `OFF_SCREEN` is how the view says a mino is above the field;
            // clipping happened in `core/view.rs`, and a coordinate that is not
            // on the field is simply not drawn.
            if usize::from(col) < VIEW_WIDTH && usize::from(row) < VIEW_HEIGHT {
                let at = layout.field_cell(col, row);
                paint::mino(painter, layout, at, paint::piece(piece.kind, percent));
            }
        }
    }
}

/// §G4.2: the hold panel, dimmed — label and piece — while hold is locked out
/// for the current piece (§9.7).
fn hold(painter: &egui::Painter, layout: &Layout, view: &GameView) {
    let panel = layout.hold();
    let (label, percent) = if view.hold_locked {
        (paint::FAINT, palette::GHOST)
    } else {
        (paint::LABEL, palette::FULL)
    };
    frame(painter, layout, panel, "HOLD", label);
    if let Some(kind) = view.hold {
        piece(painter, layout, layout.slot(panel, 0), kind, percent);
    }
}

/// §G4.3: the preview queue, slot 0 at full brightness and then §12.4's two
/// steps down.
fn next(painter: &egui::Painter, layout: &Layout, view: &GameView) {
    let panel = layout.next(view.next.len());
    frame(painter, layout, panel, "NEXT", paint::LABEL);
    for (index, kind) in view.next.iter().take(MAX_SLOTS).enumerate() {
        let percent = match index {
            0 => palette::FULL,
            1 => palette::SLOT_NEAR,
            _ => palette::SLOT_FAR,
        };
        piece(painter, layout, layout.slot(panel, index), *kind, percent);
    }
}

/// §G4.4: the six figures, a label over each.
///
/// Combo and back-to-back are here rather than on the status line, where the
/// terminal puts them for want of room. Each is always listed and says `-`
/// when it has nothing to report, so the panel does not reflow as a chain
/// starts and ends. `B2B` is the *standing* chain, `GameView::back_to_back`,
/// not whether the last clear was paid at the chained rate (§9.15, §12.8).
fn stats(painter: &egui::Painter, layout: &Layout, view: &GameView, hold_enabled: bool) {
    let panel = layout.stats(hold_enabled);
    painter.rect_filled(panel, 0.0, paint::PANEL);
    let figures = [
        ("SCORE", Some(thousands(view.score))),
        ("LEVEL", Some(view.level.to_string())),
        ("LINES", Some(view.lines.to_string())),
        ("TIME", Some(clock(view.ticks))),
        (
            "COMBO",
            (view.combo >= 1).then(|| format!("x{}", view.combo)),
        ),
        ("B2B", view.back_to_back.then(|| "ON".to_string())),
    ];
    let cell = layout.cell();
    let (left, right) = (panel.left() + cell / 2.0, panel.right() - cell / 2.0);
    for (index, (label, figure)) in figures.into_iter().enumerate() {
        let top = panel.top() + cell * FIGURE_PITCH * index as f32;
        let (words, colour) = match &figure {
            Some(figure) => (figure.as_str(), paint::TEXT),
            None => ("-", paint::FAINT),
        };
        paint::text(
            painter,
            egui::pos2(left, top + cell * 0.8),
            egui::Align2::LEFT_CENTER,
            label,
            Face::Label,
            cell * LABEL_SIZE,
            paint::LABEL,
            right - left,
        );
        paint::text(
            painter,
            egui::pos2(right, top + cell * 1.7),
            egui::Align2::RIGHT_CENTER,
            words,
            Face::Figure,
            cell * FIGURE_SIZE,
            colour,
            right - left,
        );
    }
}

/// §G4.5: the status line — the most recent clear's name for the second and a
/// half it is shown (§12.4, [`Cosmetics::clear_name`]), and §10.1's restart
/// hold in its place while the key is down.
///
/// The restart takes the whole line, as it does in the terminal: a hold with no
/// feedback is indistinguishable from a key that did nothing, and a player
/// about to throw the game away is not reading the name of their last clear.
fn status(painter: &egui::Painter, layout: &Layout, fx: &Cosmetics, restart: Option<u8>) {
    let band = layout.status();
    let cell = layout.cell();
    let centre = band.center();
    if let Some(percent) = restart {
        paint::text(
            painter,
            centre - egui::vec2(cell / 4.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            "RESTART",
            Face::Label,
            cell * STATUS_SIZE,
            paint::TEXT,
            band.width() / 2.0,
        );
        let bar = egui::Rect::from_min_size(
            centre + egui::vec2(cell / 4.0, -cell / 4.0),
            egui::vec2(cell * 5.0, cell / 2.0),
        );
        painter.rect_filled(bar, 0.0, paint::PANEL);
        let filled = bar.width() * f32::from(percent.min(100)) / 100.0;
        let fill = egui::Rect::from_min_size(bar.min, egui::vec2(filled, bar.height()));
        painter.rect_filled(fill, 0.0, paint::TEXT);
    } else if let Some(name) = fx.clear_name() {
        paint::text(
            painter,
            centre,
            egui::Align2::CENTER_CENTER,
            name,
            Face::Label,
            cell * STATUS_SIZE,
            paint::TEXT,
            band.width(),
        );
    }
}

/// §G4.6: `show_debug`'s read-out, over the bottom-left corner of `area`.
///
/// Outside §G3's metric on purpose: it is a developer's read-out and not a
/// supported layout, so it moves no minimum, changes no cell and is drawn over
/// whatever is under it. The nine figures and their words are
/// [`Debug::figures`]', the same as the terminal's strip; only the setting-out
/// is this front-end's.
pub fn debug(painter: &egui::Painter, area: egui::Rect, debug: &Debug, ticks: u64) {
    let lines: Vec<String> = debug
        .figures(ticks)
        .iter()
        .map(|row| {
            row.iter()
                .zip([12, 12, 14])
                .map(|((label, value), width)| {
                    let value_width = width - label.len();
                    format!("{label}{value:>value_width$}")
                })
                .collect::<Vec<_>>()
                .join("  ")
        })
        .collect();
    let galley = painter.layout_no_wrap(
        lines.join("\n"),
        egui::FontId::monospace(DEBUG_SIZE),
        paint::TEXT,
    );
    let margin = egui::vec2(8.0, 6.0);
    // The bottom corner, over the status band and the margin under it, which
    // hold the least of the game: over the top one it hid the hold panel.
    let size = galley.size() + margin * 2.0;
    let panel = egui::Rect::from_min_size(
        egui::pos2(area.left() + margin.x, area.bottom() - margin.y - size.y),
        size,
    );
    painter.rect_filled(panel, 4.0, egui::Color32::from_black_alpha(0xD0));
    painter.galley(panel.min + margin, galley, paint::TEXT);
}

/// A panel's ground and its label, in the label row.
fn frame(
    painter: &egui::Painter,
    layout: &Layout,
    panel: egui::Rect,
    label: &str,
    colour: egui::Color32,
) {
    let cell = layout.cell();
    painter.rect_filled(panel, 0.0, paint::PANEL);
    paint::text(
        painter,
        egui::pos2(panel.left() + cell / 2.0, panel.top() + cell * 0.6),
        egui::Align2::LEFT_CENTER,
        label,
        Face::Label,
        cell * LABEL_SIZE,
        colour,
        panel.width() - cell,
    );
}

/// A piece in its `North` orientation, centred in `slot` by the cells it
/// occupies (§G3.2) — so an `I`, which lies in one row of its box, sits in the
/// middle of the slot rather than on its lower row.
fn piece(painter: &egui::Painter, layout: &Layout, slot: egui::Rect, kind: PieceKind, percent: u8) {
    let minos = kind.cells(Rotation::North);
    let (mut left, mut top) = (i32::MAX, i32::MAX);
    let (mut right, mut bottom) = (i32::MIN, i32::MIN);
    for mino in &minos {
        left = left.min(mino.x);
        right = right.max(mino.x);
        top = top.min(mino.y);
        bottom = bottom.max(mino.y);
    }
    let size = ((right - left + 1) as u32, (bottom - top + 1) as u32);
    for mino in &minos {
        let at = ((mino.x - left) as u32, (mino.y - top) as u32);
        let cell = layout.in_slot(slot, size, at);
        paint::mino(painter, layout, cell, paint::piece(kind, percent));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{PieceView, PlayState};
    use crate::gui::layout::Measure;
    use crate::shell::menus::Overlay;
    use crate::shell::menus::Setting;
    use crate::shell::time::Stamp;

    /// An empty game, as the view of one would be.
    fn empty_view() -> GameView {
        GameView {
            rows: [[None; VIEW_WIDTH]; VIEW_HEIGHT],
            current: None,
            ghost: None,
            hold: None,
            hold_locked: false,
            next: vec![PieceKind::T; 5],
            score: 0,
            level: 1,
            lines: 0,
            ticks: 0,
            pieces: 0,
            combo: -1,
            back_to_back: false,
            state: PlayState::Falling,
        }
    }

    /// Something in every place the screen can show something.
    fn busy_view(next: usize) -> GameView {
        let mut view = empty_view();
        for (col, cell) in view.rows[19].iter_mut().enumerate().skip(1) {
            *cell = Some(PieceKind::ALL[col % 7]);
        }
        view.current = Some(PieceView {
            kind: PieceKind::T,
            cells: [(4, 0), (5, 0), (6, 0), crate::core::OFF_SCREEN],
        });
        view.ghost = Some(PieceView {
            kind: PieceKind::T,
            cells: [(4, 18), (5, 18), (6, 18), (5, 17)],
        });
        view.hold = Some(PieceKind::I);
        view.hold_locked = true;
        view.next = PieceKind::ALL.iter().copied().cycle().take(next).collect();
        view.score = u64::MAX;
        view.combo = 4;
        view.back_to_back = true;
        view.ticks = u64::MAX;
        view
    }

    fn frame_state(view: GameView, overlay: Overlay) -> FrameState {
        FrameState {
            view,
            overlay,
            generation: 0,
            restart: Some(40),
            cramped: false,
        }
    }

    /// Paint one frame into a headless `egui` context and hand back the
    /// shapes, which is everything a real window would have been sent.
    ///
    /// One context for a whole test, as a window has: making one lays out the
    /// fonts, and that is most of the cost.
    fn shapes(
        ctx: &egui::Context,
        size: egui::Vec2,
        ppp: f32,
        state: &FrameState,
        chrome: Chrome,
        boxes: bool,
    ) -> usize {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        };
        ctx.set_pixels_per_point(ppp);
        let fx = Cosmetics::new(std::time::Duration::from_millis(250), Stamp::ZERO);
        let debug = Debug {
            fps: 60,
            dropped: 0,
            das_charge: 0,
            mode: crate::shell::input::InputMode::Enhanced,
            core: crate::core::DebugView {
                milli_g: 16,
                fall_period: 3_932_160,
                lock_delay: None,
                bag: Vec::new(),
            },
        };
        let config = crate::shell::config::ConfigFile::default();
        let panels = crate::gui::overlays::Panels {
            config: &config,
            settings: &Setting::SHARED,
        };
        let output = ctx.run_ui(input, |ui| {
            let area = ui.max_rect();
            let painter = ui.painter();
            match Measure::of(area, ui.ctx().pixels_per_point()) {
                Measure::Fits(layout) => {
                    draw(painter, &layout, state, chrome, &fx);
                    // §12.6 over the screen, as the front-end draws them —
                    // unless the caller is counting what the screen alone drew.
                    if boxes {
                        crate::gui::overlays::draw(painter, &layout, state, &panels);
                    }
                }
                Measure::TooSmall { need, have } => paint::too_small(painter, area, need, have),
            }
            self::debug(painter, area, &debug, state.view.ticks);
            paint::unfocused(painter, area);
        });
        let shapes = output.shapes.len();
        // There is no renderer to hand the font atlas to, and `egui` insists
        // that saying so is deliberate.
        output.drop_without_applying_deltas();
        shapes
    }

    #[test]
    fn the_screen_draws_at_every_size_without_panicking() {
        // F6, and the headless half of what `EGUI.md` G13's render test will
        // do with a harness: every size from nothing to enormous, at several
        // densities, with and without hold and the grid, paused and not, and
        // with the widest figures a view can hold.
        let ctx = egui::Context::default();
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
        for size in [
            egui::vec2(0.0, 0.0),
            egui::vec2(1.0, 1.0),
            egui::vec2(363.0, 336.0),
            egui::vec2(364.0, 336.0),
            egui::vec2(728.0, 672.0),
            egui::vec2(3840.0, 2160.0),
        ] {
            for ppp in [1.0, 1.5, 2.0] {
                for next in [0, 1, 6, 9] {
                    for overlay in overlays() {
                        for chrome in chromes {
                            let state = frame_state(busy_view(next), overlay.clone());
                            assert!(shapes(&ctx, size, ppp, &state, chrome, true) > 0);
                        }
                    }
                }
            }
        }
    }

    /// Every overlay the program can show (§12.6), each in a state worth
    /// drawing: a menu with the cursor moved, the panel on its last row, the
    /// countdown mid-way, and the longest name the field takes.
    fn overlays() -> Vec<Overlay> {
        vec![
            Overlay::None,
            Overlay::Paused { selected: 4 },
            Overlay::Options {
                selected: Setting::SHARED.len() - 1,
            },
            Overlay::Controls,
            Overlay::Resuming { count: 2 },
            Overlay::GameOver,
            Overlay::NameEntry {
                rank: 10,
                name: "M".repeat(crate::shell::highscore::NAME_MAX),
            },
        ]
    }

    #[test]
    fn every_overlay_fits_inside_the_block() {
        // §12.6: a box is centred over the block, so it must not be wider or
        // taller than one — at any preview count, and with the widest table
        // the controls box can hold.
        let layout = match Measure::of(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(728.0, 672.0)),
            1.0,
        ) {
            Measure::Fits(layout) => layout,
            small => panic!("{small:?}"),
        };
        let block = layout.block();
        let config = crate::shell::config::ConfigFile::default();
        for overlay in overlays() {
            let state = frame_state(empty_view(), overlay.clone());
            let panels = crate::gui::overlays::Panels {
                config: &config,
                settings: &Setting::ALL,
            };
            let boxes = crate::gui::overlays::rect_of(&layout, &state.overlay, &panels);
            let Some(rect) = boxes else { continue };
            assert!(block.contains_rect(rect), "{overlay:?} is {rect:?}");
        }
    }

    #[test]
    fn a_paused_well_draws_no_pieces() {
        // §9.17: blanked while paused. The paused frame draws the same screen
        // less the stack, the ghost and the piece — so it has fewer shapes by
        // at least those, and the grid is still there.
        let size = egui::vec2(728.0, 672.0);
        let chrome = Chrome {
            show_grid: true,
            hold_enabled: true,
        };
        let playing = frame_state(busy_view(5), Overlay::None);
        let paused = frame_state(busy_view(5), Overlay::Paused { selected: 0 });
        let ctx = egui::Context::default();
        // The screen alone: what a box adds over it is `overlays`' business,
        // and is drawn whether the well was blanked or not.
        let (playing, paused) = (
            shapes(&ctx, size, 1.0, &playing, chrome, false),
            shapes(&ctx, size, 1.0, &paused, chrome, false),
        );
        // Nine on the floor, three of the piece on the field, four of ghost.
        // The scrim and the box are `overlays`', and are drawn either way.
        assert_eq!(playing - paused, 9 + 3 + 4);
    }
}
