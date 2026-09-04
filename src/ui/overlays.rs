//! Pause, game-over and name-entry overlays (§12.6).
//!
//! Overlays are drawn centred over the playfield, on a cleared background with
//! a double-line border.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::ui::centred;

// TODO(stage 9): pause with the 3-2-1 resume countdown, and the §12.6 game-over
// box with score, level, lines, time, pieces and PPS — none of which the core
// counts yet.
// TODO(stage 11): name entry with the $USER pre-fill.

/// The game-over overlay (§12.6), in its Stage 6 form: the two lines that a
/// player at a terminal actually needs.
pub fn game_over(frame: &mut Frame, over: Rect) {
    let text = Text::from(vec![
        Line::from("GAME OVER").style(Style::new().add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from("Q to quit"),
    ]);
    let area = centred(over, 18, 5);
    let block = Block::bordered().border_type(BorderType::Double);
    let interior = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(text).centered(), interior);
}
