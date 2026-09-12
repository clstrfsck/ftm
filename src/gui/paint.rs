//! What the window draws *with* (`GUI.md` §G3, §G4): its colours, a mino, a
//! line of text, and the two notices that replace or cover a screen.
//!
//! `playfield.rs` says what goes where; this says how a thing looks. Two rules
//! bind it, and both are `FRONTEND.md`'s. It draws from what it is handed and
//! has no path to the core (§12.7); and every piece colour is
//! [`shell::palette`](crate::shell::palette)'s, because §9.2's seven are not
//! equally bright and the levelled lift that fixes that is shared presentation
//! rather than a terminal technique (§12.3). How one of §12.3's brightness
//! percentages lands is this front-end's own: here it is a plain RGB scale,
//! as in a truecolor terminal, so a piece and its ghost are one hue.

use crate::core::PieceKind;
use crate::gui::layout::Layout;
use crate::shell::palette;

/// The window's ground. Not black: §9.2's blue sits at luma 84 even after the
/// lift, and a piece has to be visible against what is behind it.
pub const BACKGROUND: egui::Color32 = egui::Color32::from_rgb(0x12, 0x12, 0x16);
/// The well and the panels, a shade up from the ground so their edges read
/// without a border.
pub const PANEL: egui::Color32 = egui::Color32::from_rgb(0x1E, 0x1E, 0x24);
/// The well's walls and floor (§G4.1).
pub const WALL: egui::Color32 = egui::Color32::from_rgb(0x4A, 0x4A, 0x56);
/// An empty cell of the well when `show_grid` is on (§G4.1): the pixel reading
/// of §12.2's `··`, a tile barely lifted off the well.
pub const GRID: egui::Color32 = egui::Color32::from_rgb(0x28, 0x28, 0x30);
/// A figure, and the status line.
pub const TEXT: egui::Color32 = egui::Color32::from_rgb(0xE8, 0xE8, 0xEE);
/// A panel's label, the same grey as the web page's footer (§G8.1).
pub const LABEL: egui::Color32 = egui::Color32::from_rgb(0x9A, 0x9A, 0xA6);
/// Something that is not in force: a locked-out hold's label, a figure with
/// nothing to say.
pub const FAINT: egui::Color32 = egui::Color32::from_rgb(0x58, 0x58, 0x64);
/// What the line-clear and lock flashes wash toward (§12.5, §G6.3). Not pure
/// white: the screen's own text is `TEXT`, and a flash brighter than anything
/// else on it reads as a hole rather than as a highlight.
pub const FLASH: egui::Color32 = egui::Color32::from_rgb(0xF2, 0xF2, 0xF8);
/// What the game-over wipe washes toward (§12.5, §G6.3): the stack draining of
/// its colour, still clearly above the well's ground.
pub const GREYED: egui::Color32 = egui::Color32::from_rgb(0x55, 0x55, 0x5E);

/// §9.2's colour for a piece, through §12.3's levelled palette, at one of
/// §12.3's brightness percentages.
///
/// Every piece colour on every screen goes through `palette::levelled`, in
/// every front-end: `Colour::rgb` is the guideline table a §19 client is
/// handed, and this is what is fit to draw.
pub fn piece(kind: PieceKind, percent: u8) -> egui::Color32 {
    let (r, g, b) = palette::levelled(kind.colour());
    let scale = |channel: u8| (u16::from(channel) * u16::from(percent.min(100)) / 100) as u8;
    egui::Color32::from_rgb(scale(r), scale(g), scale(b))
}

/// `colour` blended `percent` of the way toward `toward`.
///
/// What §12.5's animations are made of in a window (§G6.1): a terminal reaches
/// for a *second colour* where a window can move part of the way to one, so a
/// flash keeps the mino's hue and the wipe can have a soft edge. In the same
/// whole-percent vocabulary as
/// [`shell::palette`](crate::shell::palette)'s brightness steps, and for the
/// same reason — a float here would be the only one on the screen.
pub fn wash(colour: egui::Color32, toward: egui::Color32, percent: u8) -> egui::Color32 {
    let percent = u16::from(percent.min(100));
    let mix = |from: u8, to: u8| {
        ((u16::from(from) * (100 - percent) + u16::from(to) * percent) / 100) as u8
    };
    egui::Color32::from_rgb(
        mix(colour.r(), toward.r()),
        mix(colour.g(), toward.g()),
        mix(colour.b(), toward.b()),
    )
}

/// One mino, filling `cell` less its gutter.
///
/// The gutter is a sixteenth of the cell and never less than a pixel, and it
/// is taken on whole pixels, so neighbouring minos read as separate tiles at
/// any size and none of them straddles a pixel boundary (§G3.1).
pub fn mino(painter: &egui::Painter, layout: &Layout, cell: egui::Rect, colour: egui::Color32) {
    let gutter = (layout.cell_pixels() / 16).max(1);
    painter.rect_filled(layout.inset(cell, gutter), 0.0, colour);
}

/// Monospace or not: a figure is set in monospace, so that a number that
/// changes does not shuffle the digits either side of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Label,
    Figure,
}

/// One line of `text`, `size` points high, placed by `align` at `anchor`.
///
/// Drawn smaller rather than clipped when it would be wider than `room`: a
/// score has no upper bound the layout could be sized for, and a figure cut
/// off at the panel's edge is a wrong figure.
#[allow(clippy::too_many_arguments)]
pub fn text(
    painter: &egui::Painter,
    anchor: egui::Pos2,
    align: egui::Align2,
    text: &str,
    face: Face,
    size: f32,
    colour: egui::Color32,
    room: f32,
) {
    let font = |size: f32| match face {
        Face::Label => egui::FontId::proportional(size),
        Face::Figure => egui::FontId::monospace(size),
    };
    // A font is never asked for at a size it cannot be: nothing on the screen
    // is this small once §G3.3's minimum is met, so this only guards the
    // arithmetic.
    if size.is_nan() || size < 1.0 {
        return;
    }
    let mut galley = painter.layout_no_wrap(text.to_owned(), font(size), colour);
    let width = galley.size().x;
    if room > 0.0 && width > room {
        let smaller = (size * room / width).max(1.0);
        galley = painter.layout_no_wrap(text.to_owned(), font(smaller), colour);
    }
    let rect = align.anchor_size(anchor, galley.size());
    painter.galley(rect.min, galley, colour);
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
        TEXT,
    );
}

/// What [`unfocused`] says.
pub const UNFOCUSED: &str = "Click to play";

/// §G3.3's replacement for every screen, below the minimum.
///
/// The sizes are in points, which is what a player dragging a window or a
/// browser edge can relate to, and what is needed is what fits at this
/// display's density rather than a round number at which it still would not.
/// A fixed size of text rather than one scaled to the window, because the
/// window is by definition too small to scale anything to; in a viewport
/// smaller than the message it is simply clipped (F6).
pub fn too_small(painter: &egui::Painter, area: egui::Rect, need: [u32; 2], have: [u32; 2]) {
    let lines = too_small_lines(need, have);
    let line = 22.0;
    let top = area.center().y - line * (lines.len() as f32 - 1.0) / 2.0;
    for (index, words) in lines.iter().enumerate() {
        let colour = if index == 0 { TEXT } else { LABEL };
        painter.text(
            egui::pos2(area.center().x, top + line * index as f32),
            egui::Align2::CENTER_CENTER,
            words,
            egui::FontId::proportional(16.0),
            colour,
        );
    }
}

/// The three lines of [`too_small`], after §12.1's.
pub fn too_small_lines(need: [u32; 2], have: [u32; 2]) -> [String; 3] {
    [
        "Window too small".to_string(),
        format!(
            "Need {} x {}, have {} x {}",
            need[0], need[1], have[0], have[1]
        ),
        "Resize to continue".to_string(),
    ]
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
        assert_ne!(piece(PieceKind::J, 100), egui::Color32::from_rgb(r, g, b));
        let (r, g, b) = palette::levelled(Colour::Blue);
        assert_eq!(piece(PieceKind::J, 100), egui::Color32::from_rgb(r, g, b));
        // And the four that were already bright are §9.2 exactly.
        let (r, g, b) = Colour::Cyan.rgb();
        assert_eq!(piece(PieceKind::I, 100), egui::Color32::from_rgb(r, g, b));
    }

    #[test]
    fn a_dimmer_step_is_the_same_hue_scaled() {
        // §12.3's dimming runs from the levelled colour, so a piece and its
        // ghost — and a preview slot and the next — are one hue.
        let (r, g, b) = palette::levelled(Colour::Purple);
        let ghost = piece(PieceKind::T, palette::GHOST);
        let scaled = |c: u8| (u16::from(c) * 45 / 100) as u8;
        assert_eq!(
            ghost,
            egui::Color32::from_rgb(scaled(r), scaled(g), scaled(b))
        );
        assert_eq!(piece(PieceKind::T, 0), egui::Color32::BLACK);
        assert_eq!(piece(PieceKind::T, 200), piece(PieceKind::T, 100), "capped");
    }

    #[test]
    fn the_too_small_message_names_both_sizes() {
        // F6: "a legible message naming what is needed and what there is".
        assert_eq!(
            too_small_lines([364, 336], [300, 200]),
            [
                "Window too small",
                "Need 364 x 336, have 300 x 200",
                "Resize to continue",
            ],
        );
    }
}
