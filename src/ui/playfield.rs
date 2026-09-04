//! The in-game screen (§12.4).
//!
//! Stage 6 draws the bare minimum: the playfield box, the locked cells and the
//! falling piece. Everything is taken from [`GameView`] and nothing reads
//! `Game` (§12.7).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

use crate::core::GameView;
use crate::core::events::OFF_SCREEN;
use crate::core::piece::PieceKind;
use crate::core::view::{VIEW_HEIGHT, VIEW_WIDTH};
use crate::ui::cells::{self, CELL_WIDTH};
use crate::ui::centred;

/// The playfield box: 10 cells of interior plus its border (§12.4).
pub const BOX_WIDTH: u16 = VIEW_WIDTH as u16 * CELL_WIDTH + 2;
/// Twenty visible rows plus the border.
pub const BOX_HEIGHT: u16 = VIEW_HEIGHT as u16 + 2;

// TODO(stage 9): the full 44 x 23 layout — hold box, next box with per-slot
// dimming, stats box and status line.

/// Draw the playfield, returning the interior rectangle so an overlay can be
/// centred over it (§12.6).
pub fn render(frame: &mut Frame, view: &GameView) -> Rect {
    let area = centred(frame.area(), BOX_WIDTH, BOX_HEIGHT);
    let block = Block::bordered();
    let interior = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(rows(view)), interior);
    interior
}

/// The visible rows, with the falling piece composited in.
///
/// The view arrives already clipped to rows 20..=39 with the buffer zone
/// removed (§12.7), so there is no clipping to do here — that is the point of
/// the view model.
fn rows(view: &GameView) -> Vec<Line<'static>> {
    let mut grid = view.rows;
    if let Some(piece) = &view.current {
        for &(col, row) in &piece.cells {
            if (col, row) == OFF_SCREEN {
                continue;
            }
            grid[row as usize][col as usize] = Some(piece.kind);
        }
    }
    grid.iter().map(|row| line(row)).collect()
}

fn line(row: &[Option<PieceKind>; VIEW_WIDTH]) -> Line<'static> {
    Line::from(
        row.iter()
            .map(|cell| match cell {
                Some(kind) => cells::filled(*kind),
                None => cells::empty(),
            })
            .collect::<Vec<_>>(),
    )
}
