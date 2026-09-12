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
//! [`overlays`](crate::gui::overlays).
//!
//! §12.5's animations are here too, as §G6 has them: the same [`Cosmetics`]
//! the terminal reads, timed by the same events and the same clock, drawn with
//! the alpha a character cell has not got. Nothing in this module may ask
//! `Cosmetics` for a number the terminal's screen does not have — that would be
//! amending §12.5 for both front-ends — so the flash's two halves are two
//! strengths of one wash and the gradients are worked out from the geometry.

use crate::core::{GameView, PieceKind, Rotation, VIEW_HEIGHT, VIEW_WIDTH};
use crate::gui::layout::{Layout, MAX_SLOTS};
use crate::gui::paint::{self, Face};
use crate::shell::cosmetics::{Banner, Cosmetics};
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
/// §12.5's banners, in cells, and the band behind one (§G6.3).
const BANNER_SIZE: f32 = 1.1;
const BANNER_BAND: f32 = 2.0;
/// How dark that band is at the banner's brightest.
const BANNER_SCRIM: f32 = 192.0;

/// The hard-drop trail just above where the piece landed, and at the top of
/// the drop (§12.5, §G6.1). A terminal draws every trail cell at one
/// brightness; a window fades along the drop, which is the same animation with
/// the alpha a character cell has not got.
const TRAIL_NEAR: u8 = 38;
const TRAIL_FAR: u8 = 8;
/// The two halves of §12.5's 12 Hz line-clear flash, as two strengths of one
/// wash toward white rather than two colours (§G6.1).
const FLASH_ON: u8 = 100;
const FLASH_OFF: u8 = 40;
/// The lock flash (§12.5): white enough to read as a flash, not so white that
/// the piece stops being the colour it is.
const LOCK_WASH: u8 = 75;
/// How many rows of the game-over wipe's front are part-greyed, so that the
/// front reads as a gradient and not as a line (§G6.3).
const WIPE_EDGE: u8 = 4;

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
        fx,
        chrome.show_grid,
        state.overlay.blanks(),
    );
    // §12.5's banners are not in the well, so they are drawn whether it was
    // blanked or not: a level that arrived as the player hit pause is still
    // worth telling them about (§G6.2).
    if let Some(up) = fx.banner() {
        banner(painter, layout, up);
    }
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

/// The well's cells, composed: what each one is drawn in, if anything (§G6.2).
type Field = [[Option<egui::Color32>; VIEW_WIDTH]; VIEW_HEIGHT];

/// §G4.1: the well, its walls, and what is in it.
///
/// `blanked` is §9.17's anti-pause-scumming rule, and it is the game's rather
/// than this screen's ([`Overlay::blanks`]): while paused the well is drawn
/// empty, grid and all, and the player gets no free look at the stack — nor at
/// an animation that outlived the pause, which is why the whole of [`compose`]
/// is skipped rather than each of its steps asked (§G6.2).
fn well(
    painter: &egui::Painter,
    layout: &Layout,
    view: &GameView,
    fx: &Cosmetics,
    grid: bool,
    blanked: bool,
) {
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
    for (row, cells) in compose(view, fx).iter().enumerate() {
        for (col, colour) in cells.iter().enumerate() {
            if let Some(colour) = colour {
                let at = layout.field_cell(col as u8, row as u8);
                paint::mino(painter, layout, at, *colour);
            }
        }
    }
    // §G6.5: the falling piece last, and a fraction of a row low. It is drawn
    // outside the grid because a grid cannot say "half a row down", and last
    // for the reason it went down last in `compose` — where it overlaps the
    // ghost, the piece is what the player sees.
    for ((col, row), colour) in falling(view, fx).into_iter().flatten() {
        let at = layout.falling_cell(col, row, view.fall_progress);
        paint::mino(painter, layout, at, colour);
    }
}

/// Everything that can occupy a cell of the well, in the order it is allowed to
/// win (§G6.2).
///
/// The same order as the terminal's, and for the same reasons: the trail goes
/// down before the stack can be drawn over and never over a mino that is really
/// there (§12.5); the ghost goes under the falling piece (§9.8). The flash and
/// the wipe are **transformations of what is already in a cell** rather than
/// things drawn over it, so neither can hide a mino.
///
/// The falling piece is the one thing not here: it may be between two rows
/// (§G6.5), which a grid of cells has no way to hold. It is [`falling`]'s, and
/// it is given the same colour and the same washes, so that a piece at rest is
/// drawn exactly as it would have been if it had stayed in the grid.
fn compose(view: &GameView, fx: &Cosmetics) -> Field {
    let mut field: Field = [[None; VIEW_WIDTH]; VIEW_HEIGHT];
    for (row, cells) in view.rows.iter().enumerate() {
        for (col, cell) in cells.iter().enumerate() {
            if let Some(kind) = cell {
                field[row][col] = Some(paint::piece(*kind, palette::FULL));
            }
        }
    }
    // §12.5's hard-drop trail, fading along the drop (§G6.1), and only where
    // the well has nothing in it: a trail is behind the stack.
    if let Some((kind, cells)) = fx.trail() {
        for &(col, row) in cells {
            if let Some(cell) = at(&mut field, (col, row))
                && cell.is_none()
            {
                *cell = Some(paint::piece(kind, trail_percent(cells, col, row)));
            }
        }
    }
    // §12.5's lock flash. The cells are the stack's by now — the piece has
    // locked — so this washes what is there and keeps the piece's hue.
    for &cell in fx.lock_flash() {
        if let Some(cell) = at(&mut field, cell) {
            *cell = Some(match *cell {
                Some(colour) => paint::wash(colour, paint::FLASH, LOCK_WASH),
                None => paint::FLASH,
            });
        }
    }
    // §9.8: the ghost goes down here so that where it and the piece overlap the
    // piece — drawn afterwards, by `falling` — is what shows. It may be absent,
    // when `ghost_piece` is off, and that is the view's answer rather than a
    // special case here (§12.7).
    if let Some(ghost) = &view.ghost {
        for &cell in &ghost.cells {
            // `OFF_SCREEN` is how the view says a mino is above the field;
            // clipping happened in `core/view.rs`, and a coordinate that is not
            // on the field is simply not drawn — which is `at`'s answer.
            if let Some(cell) = at(&mut field, cell) {
                *cell = Some(paint::piece(ghost.kind, palette::GHOST));
            }
        }
    }
    // §12.5's line-clear flash and game-over wipe, over everything that is in
    // the well by now.
    let wiped = fx.wiped_rows(VIEW_HEIGHT as u8);
    for (row, cells) in field.iter_mut().enumerate() {
        for cell in cells.iter_mut().flatten() {
            *cell = washed(*cell, row as u8, fx, wiped);
        }
    }
    field
}

/// §12.5's two whole-row transformations, applied to one cell (§G6.2).
///
/// A function of the cell rather than a pass over the grid, because the falling
/// piece is not in the grid and has to be given exactly the same treatment.
fn washed(colour: egui::Color32, row: u8, fx: &Cosmetics, wiped: u8) -> egui::Color32 {
    // The line-clear flash: one wash toward white at two strengths, so the row
    // pulses rather than blinking between two colours (§G6.1).
    let colour = match fx.flashing(row) {
        Some(white) => paint::wash(
            colour,
            paint::FLASH,
            if white { FLASH_ON } else { FLASH_OFF },
        ),
        None => colour,
    };
    // The game-over wipe, greying from the top row down, with the rows just
    // above the front part-greyed (§G6.3).
    if row < wiped {
        paint::wash(colour, paint::GREYED, wipe_percent(wiped, row))
    } else {
        colour
    }
}

/// The minos of a tetromino: how long `PieceView::cells` is (§12.7).
const MINOS: usize = 4;

/// The falling piece: the cells it covers, and the colours they are drawn in
/// (§G6.5).
///
/// Kept out of [`compose`] because [`GameView::fall_progress`] may put it
/// between two rows, and the grid there has no way to hold that. Everything
/// else about it is unchanged — the same colour, and [`washed`] with the same
/// two transformations — so at rest it is drawn exactly where and as it was
/// before this became a separate step.
///
/// The rows it is washed by are the ones it *occupies*, not the ones it is
/// sliding toward: a piece is in the row the rules say it is in, and the offset
/// is how far between rows it is drawn (§12.7).
fn falling(view: &GameView, fx: &Cosmetics) -> [Option<((u8, u8), egui::Color32)>; MINOS] {
    let mut minos = [None; MINOS];
    let Some(piece) = &view.current else {
        return minos;
    };
    let wiped = fx.wiped_rows(VIEW_HEIGHT as u8);
    let colour = paint::piece(piece.kind, palette::FULL);
    for (mino, &(col, row)) in minos.iter_mut().zip(&piece.cells) {
        // `OFF_SCREEN`, and anything else off the field, is simply not drawn —
        // the same answer `at` gives `compose`.
        if usize::from(col) < VIEW_WIDTH && usize::from(row) < VIEW_HEIGHT {
            *mino = Some(((col, row), washed(colour, row, fx, wiped)));
        }
    }
    minos
}

/// One cell of the well, or `None` for a coordinate that is not on the field.
fn at(field: &mut Field, (col, row): (u8, u8)) -> Option<&mut Option<egui::Color32>> {
    field
        .get_mut(usize::from(row))
        .and_then(|row| row.get_mut(usize::from(col)))
}

/// How bright a trail cell is: brightest just above where the piece landed,
/// almost gone at the top of the drop (§G6.1).
///
/// Worked out per column from the trail's own geometry, because that is what
/// `Cosmetics` reports; asking it for a fade instead would be asking §12.5 for
/// a number the terminal has no use for.
fn trail_percent(cells: &[(u8, u8)], col: u8, row: u8) -> u8 {
    let (mut top, mut bottom) = (row, row);
    for &(_, at) in cells.iter().filter(|(other, _)| *other == col) {
        top = top.min(at);
        bottom = bottom.max(at);
    }
    let span = u16::from(bottom - top);
    if span == 0 {
        return TRAIL_NEAR;
    }
    let fade = u16::from(TRAIL_NEAR - TRAIL_FAR) * u16::from(bottom - row) / span;
    (u16::from(TRAIL_NEAR) - fade) as u8
}

/// How far through the wipe `row` is, with the front at `wiped` (§G6.3): the
/// few rows above the front fade in, and everything above them is fully grey.
///
/// The front is soft only while there *is* a front. A wipe that has reached the
/// floor has finished, and what it settles on is a wholly grey stack — which is
/// what §9.16 leaves under §12.6's box.
fn wipe_percent(wiped: u8, row: u8) -> u8 {
    if usize::from(wiped) >= VIEW_HEIGHT {
        return 100;
    }
    (100 * u16::from((wiped - row).min(WIPE_EDGE)) / u16::from(WIPE_EDGE)) as u8
}

/// §12.5's banner, centred over the well (§G6.3).
///
/// The band behind it is this front-end's own: a word in one of §9.2's seven
/// colours over a stack in the other six is not reliably legible, and a window,
/// unlike a terminal, cannot simply clear the row. It fades with the banner.
fn banner(painter: &egui::Painter, layout: &Layout, banner: Banner) {
    let (words, colour, left) = match banner {
        Banner::LevelUp(level, left) => (format!("LEVEL {level}"), paint::TEXT, left),
        Banner::PerfectClear(hue) => (
            "PERFECT CLEAR".to_string(),
            paint::piece(hue, palette::FULL),
            100,
        ),
    };
    // `Cosmetics` reports what is left of the level-up banner as a percentage,
    // and here that is alpha rather than a second colour (§G6.1).
    let fade = f32::from(left.min(100)) / 100.0;
    let (well, cell) = (layout.well(), layout.cell());
    let band =
        egui::Rect::from_center_size(well.center(), egui::vec2(well.width(), cell * BANNER_BAND));
    painter.rect_filled(
        band,
        0.0,
        egui::Color32::from_black_alpha((BANNER_SCRIM * fade) as u8),
    );
    paint::text(
        painter,
        well.center(),
        egui::Align2::CENTER_CENTER,
        &words,
        Face::Label,
        cell * BANNER_SIZE,
        colour.gamma_multiply(fade),
        well.width() - cell,
    );
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
    use crate::core::{GameEvent, PieceView, PlayState};
    use crate::gui::layout::Measure;
    use crate::shell::menus::Overlay;
    use crate::shell::menus::Setting;
    use crate::shell::time::Stamp;

    /// An empty game, as the view of one would be.
    fn empty_view() -> GameView {
        GameView {
            rows: [[None; VIEW_WIDTH]; VIEW_HEIGHT],
            current: None,
            fall_progress: 0,
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
        // Part-way between two rows (§G6.5), so the size sweep draws a sliding
        // piece at every size and density rather than only a settled one.
        view.fall_progress = 40_000;
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
        fx: &Cosmetics,
    ) -> usize {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        };
        ctx.set_pixels_per_point(ppp);
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
                    draw(painter, &layout, state, chrome, fx);
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

    /// §12.5's clear pause, as the timers are told it.
    const CLEAR_DELAY: std::time::Duration = std::time::Duration::from_millis(250);

    /// Nothing animating: the screen at rest.
    fn quiet_fx() -> Cosmetics {
        Cosmetics::new(CLEAR_DELAY, Stamp::ZERO)
    }

    /// Every §12.5 animation at once, `at` into them.
    ///
    /// Not a state a game reaches — a perfect clear and a top out in one tick —
    /// but every one of them is a `Cosmetics` field, and this is what makes the
    /// size sweep above draw all of them (§G6).
    fn busy_fx(at: std::time::Duration) -> Cosmetics {
        let mut fx = quiet_fx();
        fx.absorb(
            &[
                GameEvent::HardDropped { rows: 6 },
                GameEvent::PieceLocked {
                    cells: [(4, 19), (5, 19), (6, 19), (5, 18)],
                    kind: PieceKind::T,
                },
                GameEvent::LinesCleared {
                    rows: vec![19],
                    clear: crate::core::ClearKind::Quad,
                    b2b: true,
                    combo: 3,
                },
                GameEvent::LevelUp(5),
                GameEvent::ToppedOut(crate::core::TopOutCause::LockOut),
            ],
            Stamp::ZERO,
        );
        fx.absorb(&[], Stamp::ZERO + at);
        fx
    }

    #[test]
    fn the_screen_draws_at_every_size_without_panicking() {
        // F6, and the headless half of what `EGUI.md` G13's render test will
        // do with a harness: every size from nothing to enormous, at several
        // densities, with and without hold and the grid, paused and not, with
        // the widest figures a view can hold — and with every §12.5 animation
        // part-way through, which is what G9 added to the sweep.
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
                            for fx in [quiet_fx(), busy_fx(std::time::Duration::from_millis(40))] {
                                let state = frame_state(busy_view(next), overlay.clone());
                                assert!(shapes(&ctx, size, ppp, &state, chrome, true, &fx) > 0);
                            }
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
            shapes(&ctx, size, 1.0, &playing, chrome, false, &quiet_fx()),
            shapes(&ctx, size, 1.0, &paused, chrome, false, &quiet_fx()),
        );
        // Nine on the floor, three of the piece on the field, four of ghost.
        // The scrim and the box are `overlays`', and are drawn either way.
        assert_eq!(playing - paused, 9 + 3 + 4);
    }

    #[test]
    fn a_blanked_well_animates_nothing_but_still_banners() {
        // §G6.2: §9.17's blanking comes before the whole composition, so an
        // animation cannot outlive the pause and hand the player exactly the
        // look at the stack §9.17 is there to refuse. The banners are not in
        // the well, and are drawn either way: two shapes, the band and the
        // word.
        let size = egui::vec2(728.0, 672.0);
        let chrome = Chrome {
            show_grid: true,
            hold_enabled: true,
        };
        let paused = frame_state(busy_view(5), Overlay::Paused { selected: 0 });
        let ctx = egui::Context::default();
        let quiet = shapes(&ctx, size, 1.0, &paused, chrome, false, &quiet_fx());
        let busy = shapes(
            &ctx,
            size,
            1.0,
            &paused,
            chrome,
            false,
            &busy_fx(std::time::Duration::from_millis(40)),
        );
        assert_eq!(busy - quiet, 2, "the banner, and nothing in the well");
    }

    /// How bright a colour is, near enough to order two shades of one hue by.
    fn brightness(colour: egui::Color32) -> u32 {
        u32::from(colour.r()) + u32::from(colour.g()) + u32::from(colour.b())
    }

    #[test]
    fn a_hard_drop_trail_fades_along_the_drop_and_stays_behind_the_stack() {
        // §G6.1: a terminal draws every trail cell at one brightness; the fade
        // along the drop is worked out from the trail's own geometry, which is
        // what keeps it out of `Cosmetics`.
        let mut view = empty_view();
        // A mino the trail passes straight through: it must not be painted
        // over, because a trail is behind the stack and never in front of it.
        view.rows[17][4] = Some(PieceKind::O);
        let mut fx = quiet_fx();
        fx.absorb(
            &[
                GameEvent::HardDropped { rows: 3 },
                GameEvent::PieceLocked {
                    cells: [(4, 19), (5, 19), (6, 19), (5, 18)],
                    kind: PieceKind::T,
                },
            ],
            Stamp::ZERO,
        );
        let field = compose(&view, &fx);
        let (near, far) = (
            field[18][4].expect("just above where it landed"),
            field[16][4].expect("the top of the drop"),
        );
        assert_eq!(near, paint::piece(PieceKind::T, TRAIL_NEAR));
        assert_eq!(far, paint::piece(PieceKind::T, TRAIL_FAR));
        assert!(brightness(near) > brightness(far), "it fades upward");
        assert_eq!(
            field[17][4],
            Some(paint::piece(PieceKind::O, palette::FULL)),
            "and never over a mino that is really there",
        );
        // §12.5's 120 ms, which is `Cosmetics`' and not this screen's.
        fx.absorb(&[], Stamp::ZERO + std::time::Duration::from_millis(120));
        assert_eq!(compose(&view, &fx)[18][4], None);
    }

    #[test]
    fn a_flashing_row_is_one_wash_at_two_strengths() {
        // §G6.1: the terminal alternates white and the piece's own colour; the
        // window washes toward white at two strengths, so the row pulses
        // rather than blinking. Same 12 Hz, same `line_clear_delay_ms`.
        let mut view = empty_view();
        view.rows[19][0] = Some(PieceKind::I);
        let mut fx = quiet_fx();
        fx.absorb(
            &[GameEvent::LinesCleared {
                rows: vec![19],
                clear: crate::core::ClearKind::Quad,
                b2b: false,
                combo: 0,
            }],
            Stamp::ZERO,
        );
        let plain = paint::piece(PieceKind::I, palette::FULL);
        let on = compose(&view, &fx)[19][0].expect("the mino is still there");
        fx.absorb(
            &[],
            Stamp::ZERO + std::time::Duration::from_millis(1_000 / 12),
        );
        let field = compose(&view, &fx);
        let off = field[19][0].expect("and on the other half too");
        assert_eq!(on, paint::FLASH, "fully washed on one half");
        assert!(
            brightness(on) > brightness(off) && brightness(off) > brightness(plain),
            "and part-way on the other: {on:?} {off:?} {plain:?}",
        );
        assert_eq!(field[19][1], None, "a flash cannot invent a mino");
        // The flash lasts the core's clear pause exactly, and then the rows go.
        fx.absorb(&[], Stamp::ZERO + CLEAR_DELAY);
        assert_eq!(compose(&view, &fx)[19][0], Some(plain));
    }

    #[test]
    fn the_lock_flash_keeps_the_pieces_hue() {
        // §G6.3: white enough to read as a flash, not so white that the piece
        // stops being the colour it is — which is the thing a character cell
        // could not do.
        let mut view = empty_view();
        view.rows[19][4] = Some(PieceKind::T);
        let mut fx = quiet_fx();
        fx.absorb(
            &[GameEvent::PieceLocked {
                cells: [(4, 19), (5, 19), (6, 19), (5, 18)],
                kind: PieceKind::T,
            }],
            Stamp::ZERO,
        );
        let plain = paint::piece(PieceKind::T, palette::FULL);
        let flashed = compose(&view, &fx)[19][4].expect("the cell that locked");
        assert!(brightness(flashed) > brightness(plain));
        assert_ne!(flashed, paint::FLASH, "a white mino has lost its piece");
        // §12.5's 80 ms.
        fx.absorb(&[], Stamp::ZERO + std::time::Duration::from_millis(80));
        assert_eq!(compose(&view, &fx)[19][4], Some(plain));
    }

    #[test]
    fn the_falling_piece_is_drawn_as_the_grid_would_have_drawn_it() {
        // §G6.5: the piece comes out of `compose`'s grid so it can sit between
        // two rows, and nothing else about it changes. The colour it gets is
        // the colour the cell would have had — including §12.5's washes, which
        // is what keeps a piece that tops out mid-wipe greying with the stack
        // instead of staying bright over it.
        let mut view = empty_view();
        view.current = Some(PieceView {
            kind: PieceKind::T,
            cells: [(4, 5), (5, 5), (6, 5), (5, 4)],
        });
        let quiet = quiet_fx();
        let plain = paint::piece(PieceKind::T, palette::FULL);
        assert_eq!(compose(&view, &quiet)[5][4], None, "not in the grid");
        assert_eq!(
            falling(&view, &quiet),
            [
                Some(((4, 5), plain)),
                Some(((5, 5), plain)),
                Some(((6, 5), plain)),
                Some(((5, 4), plain)),
            ],
        );

        // A mino the view clipped is not drawn, exactly as `compose` does not
        // draw one (§12.7).
        view.current = Some(PieceView {
            kind: PieceKind::T,
            cells: [(4, 0), (5, 0), (6, 0), crate::core::OFF_SCREEN],
        });
        assert_eq!(falling(&view, &quiet)[3], None, "the clipped mino");

        // Mid-wipe, the piece is washed by the row it *occupies* — the row the
        // rules say it is in, not the one it is sliding toward.
        let mut fx = quiet_fx();
        fx.absorb(
            &[GameEvent::ToppedOut(crate::core::TopOutCause::LockOut)],
            Stamp::ZERO,
        );
        fx.absorb(&[], Stamp::ZERO + std::time::Duration::from_millis(250));
        let wiped = falling(&view, &fx)[0].expect("a mino on row 0").1;
        assert_eq!(wiped, paint::GREYED, "row 0 went first");
    }

    #[test]
    fn a_sliding_piece_is_the_only_thing_that_leaves_its_row() {
        // §G6.5: `fall_progress` moves the falling piece and nothing else. The
        // ghost marks a landing row, which is a discrete fact — interpolating
        // it too would hold the gap between them constant, which is the thing
        // that would look wrong (`EGUI.md` G10).
        let layout = match Measure::of(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(728.0, 672.0)),
            1.0,
        ) {
            Measure::Fits(layout) => layout,
            small => panic!("{small:?}"),
        };
        let mut view = busy_view(5);
        view.fall_progress = 0;
        let settled = layout.falling_cell(4, 0, view.fall_progress);
        assert_eq!(
            settled,
            layout.field_cell(4, 0),
            "nothing accrued, nothing moved"
        );

        view.fall_progress = u16::MAX / 2;
        let sliding = layout.falling_cell(4, 0, view.fall_progress);
        assert!(
            (sliding.min.y - settled.min.y - layout.cell() / 2.0).abs() <= 1.0,
            "about half a cell down: {sliding:?} against {settled:?}",
        );
        // The ghost is `compose`'s, and `compose` is not given the fraction at
        // all: it lands on the grid whatever the piece above it is doing.
        assert_eq!(
            compose(&view, &quiet_fx())[18][4],
            Some(paint::piece(PieceKind::T, palette::GHOST)),
            "the ghost stayed on its landing row",
        );
    }

    #[test]
    fn the_game_over_wipe_greys_downward_behind_a_soft_front() {
        // §G6.3: §12.5's 500 ms, top row first — and the rows just above the
        // front part-greyed, so the front reads as a gradient rather than as a
        // line moving down the stack.
        let mut view = empty_view();
        for row in view.rows.iter_mut() {
            row[0] = Some(PieceKind::I);
        }
        let mut fx = quiet_fx();
        fx.absorb(
            &[GameEvent::ToppedOut(crate::core::TopOutCause::LockOut)],
            Stamp::ZERO,
        );
        // Half of 500 ms is ten of the twenty rows.
        fx.absorb(&[], Stamp::ZERO + std::time::Duration::from_millis(250));
        let plain = paint::piece(PieceKind::I, palette::FULL);
        let field = compose(&view, &fx);
        assert_eq!(field[0][0], Some(paint::GREYED), "long gone");
        assert_eq!(
            field[9][0],
            Some(paint::wash(plain, paint::GREYED, 25)),
            "the front row is barely touched",
        );
        assert_eq!(field[10][0], Some(plain), "and below it nothing has begun");
        assert!(
            brightness(field[6][0].unwrap()) <= brightness(field[9][0].unwrap()),
            "the front is a gradient",
        );
        // It settles rather than expiring, and what it settles on is a wholly
        // grey stack: the front is soft only while there is a front, and §9.16
        // leaves this under §12.6's box.
        fx.absorb(&[], Stamp::ZERO + std::time::Duration::from_secs(4));
        assert!(
            compose(&view, &fx)
                .iter()
                .all(|row| row[0] == Some(paint::GREYED)),
            "a finished wipe has no gradient left in it",
        );
    }
}
