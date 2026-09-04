//! Cell glyph rendering primitives (§12.2).
//!
//! One matrix cell is **two terminal columns**, so that a cell is roughly
//! square in a typical font. Every width in `ui` is given in cells; the
//! character width is twice it.

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::core::piece::PieceKind;

/// The character width of one matrix cell (§12.2).
pub const CELL_WIDTH: u16 = 2;

/// The default `cell_filled` glyph (§6.3). Exactly two display columns.
pub const FILLED: &str = "██";
/// The default `cell_empty` glyph (§6.3).
pub const EMPTY: &str = "  ";

// TODO(stage 9): take the colour from `ui::theme` so the §12.3 depths and the
// ghost's dimming apply.
// TODO(stage 10): the configured `cell_filled` / `cell_empty` / `cell_ghost`
// glyphs, once the loader has checked they are two columns wide (§12.2).

/// The guideline colour of a piece as a terminal colour (§9.2).
///
/// Truecolor for now: the colour-depth fallback of §12.3 is Stage 9's, and a
/// terminal that cannot manage 24-bit colour approximates it in the meantime.
pub fn colour(kind: PieceKind) -> Color {
    let (r, g, b) = kind.colour().rgb();
    Color::Rgb(r, g, b)
}

/// One occupied cell, in its piece's colour on the default background.
pub fn filled(kind: PieceKind) -> Span<'static> {
    Span::styled(FILLED, Style::new().fg(colour(kind)))
}

/// One empty cell.
pub fn empty() -> Span<'static> {
    Span::raw(EMPTY)
}
