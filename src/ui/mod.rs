//! Screen dispatch, the terminal-too-small screen (§12.1) and the animation
//! timers (§12.5).
//!
//! Everything here draws from `GameView` alone and never reads `Game` (§12.7).

pub mod attract;
pub mod cells;
pub mod overlays;
pub mod playfield;
pub mod theme;

use std::io::Stdout;

use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;

use crate::core::{GameView, PlayState};

/// The terminal the game draws on.
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

// TODO(stage 9): screen dispatch and the six §12.5 animations, each started by a
// GameEvent and timed on the wall clock.
// TODO(stage 12): the terminal-too-small screen and resize handling (§8.4).

/// Draw one frame of the playing screen.
pub fn draw(frame: &mut Frame, view: &GameView) {
    let field = playfield::render(frame, view);
    if view.state == PlayState::ToppedOut {
        overlays::game_over(frame, field);
    }
}

/// A `width` x `height` block centred in `area` (§12.1: the UI is a fixed-size
/// block and the extra space is margin), clipped to what there is.
pub fn centred(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
