//! The in-game screen (§12.4).
//!
//! Stage 6 draws the bare minimum: the playfield box, the locked cells, the
//! falling piece and Stage 8's ghost, with Stage 7's counters on the bottom
//! border as a debug line until §12.4's status line exists. Everything is taken from [`GameView`] and
//! nothing reads `Game` (§12.7).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::block::Position;
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
    // A debug line, not the §12.4 status line: Stage 9 replaces the whole
    // layout, and until it does this is the only way to see that the score of
    // §9.14 moves at all.
    let block = Block::bordered()
        .title_bottom(counters(view))
        .title_position(Position::Bottom);
    let interior = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(rows(view)), interior);
    interior
}

/// The counters, squeezed onto the bottom border (§9.14, §9.15).
fn counters(view: &GameView) -> String {
    let mut line = format!(" {} L{} {} ", view.score, view.level, view.lines);
    if view.back_to_back {
        line.push_str("B2B ");
    }
    if view.combo >= 1 {
        line.push_str(&format!("x{} ", view.combo));
    }
    line
}

/// What one cell of the composited field shows.
#[derive(Clone, Copy)]
enum Cell {
    Empty,
    Filled(PieceKind),
    Ghost(PieceKind),
}

/// The visible rows, with the ghost and the falling piece composited in.
///
/// The view arrives already clipped to rows 20..=39 with the buffer zone
/// removed (§12.7), so there is no clipping to do here — that is the point of
/// the view model.
fn rows(view: &GameView) -> Vec<Line<'static>> {
    let mut grid = view.rows.map(|row| {
        row.map(|cell| match cell {
            Some(kind) => Cell::Filled(kind),
            None => Cell::Empty,
        })
    });
    // §9.8: the ghost goes down first, so that where the two overlap the
    // falling piece is what is drawn.
    for (piece, cell) in [
        (&view.ghost, Cell::Ghost as fn(PieceKind) -> Cell),
        (&view.current, Cell::Filled as fn(PieceKind) -> Cell),
    ] {
        let Some(piece) = piece else { continue };
        for &(col, row) in &piece.cells {
            if (col, row) == OFF_SCREEN {
                continue;
            }
            grid[row as usize][col as usize] = cell(piece.kind);
        }
    }
    grid.iter().map(|row| line(row)).collect()
}

fn line(row: &[Cell; VIEW_WIDTH]) -> Line<'static> {
    Line::from(
        row.iter()
            .map(|cell| match cell {
                Cell::Filled(kind) => cells::filled(*kind),
                Cell::Ghost(kind) => cells::ghost(*kind),
                Cell::Empty => cells::empty(),
            })
            .collect::<Vec<_>>(),
    )
}
