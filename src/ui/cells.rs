//! Cell glyph rendering primitives (§12.2).
//!
//! One matrix cell is **two terminal columns**, so that a cell is roughly
//! square in a typical font. Every width in `ui` is given in cells; the
//! character width is twice it.
//!
//! A [`Paint`] is what one composited cell shows *after* the ghost, the falling
//! piece and the §12.5 animations have all had their say. Turning it into a
//! span is the only place that knows about glyphs and colour depth.

use ratatui::text::Span;

use crate::core::piece::PieceKind;
use crate::ui::theme::{self, Theme};

/// The character width of one matrix cell (§12.2).
pub const CELL_WIDTH: u16 = 2;

// TODO(stage 10): the configured `cell_filled` / `cell_empty` / `cell_ghost`
// glyphs, once the loader has checked they are two columns wide (§12.2).

/// What one composited cell shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint {
    /// Nothing — drawn as the grid dot when `show_grid` is on.
    Empty,
    /// A mino, at a brightness from `ui::theme`: full for the board and the
    /// falling piece, dimmer for the later preview slots (§12.4).
    Filled(PieceKind, u8),
    /// The landing position (§9.8), in the piece's colour, dimmed.
    Ghost(PieceKind),
    /// White: the line-clear and lock flashes of §12.5.
    Flash,
    /// The game-over wipe of §12.5.
    Greyed,
}

impl Paint {
    /// A mino at full brightness.
    pub const fn filled(kind: PieceKind) -> Self {
        Paint::Filled(kind, theme::FULL)
    }

    /// The glyph is chosen by what the cell *is*, and the style by how bright
    /// it is, so a flashing mino keeps its shape while it changes colour.
    fn glyph(self, theme: Theme, grid: bool) -> &'static str {
        match self {
            Paint::Empty => theme.empty_glyph(grid),
            Paint::Ghost(_) => theme.ghost_glyph(),
            Paint::Filled(kind, _) => theme.filled_glyph(kind),
            // A flashing or greyed cell is always one that holds a mino; which
            // piece it came from no longer matters, but in `mono` the glyph
            // still has to be a block of some sort.
            Paint::Flash | Paint::Greyed => theme.filled_glyph(PieceKind::O),
        }
    }
}

/// One cell of the field, ready to draw.
pub fn span(theme: Theme, paint: Paint, grid: bool) -> Span<'static> {
    let style = match paint {
        Paint::Empty if grid => theme.faint(),
        Paint::Empty => theme.plain(),
        Paint::Filled(kind, percent) => theme.piece(kind, percent),
        Paint::Ghost(kind) => theme.piece(kind, theme::GHOST),
        Paint::Flash => theme.flash(),
        Paint::Greyed => theme.greyed(),
    };
    Span::styled(paint.glyph(theme, grid), style)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::Depth;

    #[test]
    fn every_paint_is_two_columns_wide() {
        // §12.2, and the reason the playfield never has to pad: whatever a cell
        // turns out to be, it occupies exactly one cell's worth of terminal.
        let theme = Theme::new(Depth::Truecolor);
        for paint in [
            Paint::Empty,
            Paint::filled(PieceKind::T),
            Paint::Ghost(PieceKind::T),
            Paint::Flash,
            Paint::Greyed,
        ] {
            for grid in [false, true] {
                assert_eq!(span(theme, paint, grid).content.chars().count(), 2);
            }
        }
    }
}
