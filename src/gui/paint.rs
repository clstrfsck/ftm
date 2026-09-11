//! What the window draws, in pixels (`GUI.md` §G1).
//!
//! The G5 vertical slice, and no more than that: the locked cells of
//! [`GameView::rows`] and the falling piece, as rectangles. No hold box, no
//! next queue, no stats, no ghost and no grid — §12.4's information arrives at
//! `EGUI.md` G7, over §G3's cell metric, and this module is where it lands.
//!
//! Two rules bind it already, and both are `FRONTEND.md`'s. It draws from
//! [`GameView`] alone and has no path to the core (§12.7); and the colours are
//! [`shell::palette`](crate::shell::palette)'s, because §9.2's seven are not
//! equally bright and the levelled lift that fixes that is shared presentation
//! rather than a terminal technique (§12.3).

use crate::core::{GameView, OFF_SCREEN, PieceKind, VIEW_HEIGHT, VIEW_WIDTH};
use crate::shell::palette;

/// The window's ground. Not black: §9.2's blue sits at luma 17 even after the
/// lift, and a piece has to be visible against what is behind it.
pub const BACKGROUND: egui::Color32 = egui::Color32::from_rgb(0x12, 0x12, 0x16);
/// The well, a shade up from the ground so its edges read without a border.
const WELL: egui::Color32 = egui::Color32::from_rgb(0x1E, 0x1E, 0x24);
/// The smallest cell worth drawing, in points. Below this the window is
/// showing nothing useful — §G3 makes that a state of its own at G7.
const MIN_CELL: f32 = 2.0;
/// The gap between one mino and the next, as a fraction of the cell.
const GUTTER: f32 = 0.06;

/// Draw one frame of a game into `area`.
///
/// `dimmed` is §12.6's blanked playfield — what is under the pause menu, the
/// Options panel and the game-over box. The boxes themselves are §G5's and
/// land at G8; until then the scrim on its own is what says the game is not
/// running, so that a paused window does not merely look frozen.
pub fn playfield(painter: &egui::Painter, area: egui::Rect, view: &GameView, dimmed: bool) {
    let cell = (area.width() / VIEW_WIDTH as f32)
        .min(area.height() / VIEW_HEIGHT as f32)
        .floor();
    if cell < MIN_CELL {
        return;
    }
    let size = egui::vec2(cell * VIEW_WIDTH as f32, cell * VIEW_HEIGHT as f32);
    let well = egui::Rect::from_center_size(area.center(), size);
    painter.rect_filled(well, 0.0, WELL);

    for (row, cells) in view.rows.iter().enumerate() {
        for (col, cell_kind) in cells.iter().enumerate() {
            if let Some(kind) = cell_kind {
                mino(painter, well, cell, (col as u8, row as u8), *kind);
            }
        }
    }
    // The falling piece, over the stack. Absent during the clear and entry
    // delays, which is the view's answer and not a special case here (§12.7).
    if let Some(current) = &view.current {
        for at in current.cells {
            mino(painter, well, cell, at, current.kind);
        }
    }
    if dimmed {
        painter.rect_filled(well, 0.0, egui::Color32::from_black_alpha(0xC0));
    }
}

/// The notice that the keyboard is somewhere else (§G8.2).
///
/// Over everything, across the middle of `area`. In a tab it is the difference
/// between a game that seems broken and one that says what to do: a canvas
/// without focus hears no keys, and the page under it scrolls on the ones meant
/// for the game. A click reaches the *browser*, which focuses the canvas —
/// the game itself sees no pointer (§1.2). A window says the same thing for
/// the same reason, and a click focuses it too.
pub fn unfocused(painter: &egui::Painter, area: egui::Rect) {
    let size = (area.width() / 16.0).clamp(12.0, 28.0);
    let band = egui::Rect::from_center_size(area.center(), egui::vec2(area.width(), size * 3.0));
    painter.rect_filled(band, 0.0, egui::Color32::from_black_alpha(0xD8));
    painter.text(
        band.center(),
        egui::Align2::CENTER_CENTER,
        UNFOCUSED,
        egui::FontId::proportional(size),
        egui::Color32::from_gray(0xE8),
    );
}

/// What [`unfocused`] says.
pub const UNFOCUSED: &str = "Click to play";

/// One mino at visible-field coordinates `(col, row)`.
///
/// [`OFF_SCREEN`] is how the view says a cell is above the field (§12.7);
/// clipping happened in `core/view.rs` and this only has to honour it.
fn mino(
    painter: &egui::Painter,
    well: egui::Rect,
    cell: f32,
    (col, row): (u8, u8),
    kind: PieceKind,
) {
    if (col, row) == OFF_SCREEN {
        return;
    }
    let gutter = (cell * GUTTER).max(1.0);
    let top_left = well.min + egui::vec2(f32::from(col) * cell, f32::from(row) * cell);
    let tile = egui::Rect::from_min_size(top_left, egui::Vec2::splat(cell)).shrink(gutter / 2.0);
    painter.rect_filled(tile, 0.0, colour(kind));
}

/// §9.2's colour for a piece, through §12.3's levelled palette.
///
/// Every piece colour on every screen goes through `palette::levelled`, in
/// every front-end: `Colour::rgb` is the guideline table a §19 client is
/// handed, and this is what is fit to draw.
fn colour(kind: PieceKind) -> egui::Color32 {
    let (r, g, b) = palette::levelled(kind.colour());
    egui::Color32::from_rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Colour;

    #[test]
    fn the_pieces_are_drawn_in_the_levelled_palette_and_not_in_the_raw_table() {
        // §12.3: §9.2's blue is luma 17 and unreadable as written, and the
        // lift is shared presentation rather than a terminal's workaround — so
        // a window has to go through it too.
        let (r, g, b) = Colour::Blue.rgb();
        assert_ne!(colour(PieceKind::J), egui::Color32::from_rgb(r, g, b));
        let (r, g, b) = palette::levelled(Colour::Blue);
        assert_eq!(colour(PieceKind::J), egui::Color32::from_rgb(r, g, b));
        // And the four that were already bright are §9.2 exactly.
        let (r, g, b) = Colour::Cyan.rgb();
        assert_eq!(colour(PieceKind::I), egui::Color32::from_rgb(r, g, b));
    }
}
