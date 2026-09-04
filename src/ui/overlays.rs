//! Pause, game-over and name-entry overlays (§12.6).
//!
//! Overlays are drawn centred over the playfield, on a cleared background with
//! a double-line border. Like everything else in `ui` they read `GameView` and
//! never `Game` (§12.7).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::core::GameView;
use crate::ui::playfield::clock;
use crate::ui::{Chrome, centred};

// TODO(stage 11): name entry with the $USER pre-fill (§12.6).

/// The pause menu of §9.17, in the order it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseChoice {
    Resume,
    Restart,
    Controls,
    QuitToMenu,
}

impl PauseChoice {
    pub const ALL: [PauseChoice; 4] = [
        PauseChoice::Resume,
        PauseChoice::Restart,
        PauseChoice::Controls,
        PauseChoice::QuitToMenu,
    ];

    const fn label(self) -> &'static str {
        match self {
            PauseChoice::Resume => "Resume",
            PauseChoice::Restart => "Restart",
            PauseChoice::Controls => "Controls",
            PauseChoice::QuitToMenu => "Quit to menu",
        }
    }
}

/// The pause overlay (§12.6). The playfield underneath it has already been
/// blanked (§9.17); this is only the menu.
pub fn paused(frame: &mut Frame, over: Rect, chrome: &Chrome, selected: usize) {
    let mut lines = vec![
        Line::styled(centre("PAUSED", PAUSE_WIDTH), chrome.theme.bold()),
        Line::raw(" ".repeat(PAUSE_WIDTH)),
    ];
    for (index, choice) in PauseChoice::ALL.iter().enumerate() {
        let marker = if index == selected {
            "    \u{25b8} "
        } else {
            "      "
        };
        let style = if index == selected {
            chrome.theme.bold()
        } else {
            chrome.theme.plain()
        };
        lines.push(Line::styled(
            format!("{marker}{:<12}", choice.label()),
            style,
        ));
    }
    box_over(frame, over, PAUSE_WIDTH, lines);
}

/// The 3-2-1 resume countdown (§9.17), over a playfield that is visible again.
pub fn resuming(frame: &mut Frame, over: Rect, chrome: &Chrome, count: u8) {
    let lines = vec![Line::styled(
        centre(&count.to_string(), COUNTDOWN_WIDTH),
        chrome.theme.bold(),
    )];
    box_over(frame, over, COUNTDOWN_WIDTH, lines);
}

/// The game-over overlay (§12.6).
///
/// Every figure comes from the view: elapsed time is the tick count converted
/// here, and pieces-per-second is derived from the same two numbers, so the
/// box cannot disagree with the stats panel behind it (§11).
pub fn game_over(frame: &mut Frame, over: Rect, view: &GameView, chrome: &Chrome) {
    let blank = Line::raw(" ".repeat(OVER_WIDTH));
    let lines = vec![
        Line::styled(centre("GAME OVER", OVER_WIDTH), chrome.theme.bold()),
        blank.clone(),
        Line::raw(figure("SCORE", &view.score.to_string())),
        Line::raw(figure("LEVEL", &view.level.to_string())),
        Line::raw(figure("LINES", &view.lines.to_string())),
        Line::raw(figure("TIME", &clock(view.ticks))),
        Line::raw(figure("PIECES", &view.pieces.to_string())),
        Line::raw(figure("PPS", &pps(view.pieces, view.ticks))),
        blank,
        Line::raw(centre("Press any key", OVER_WIDTH)),
    ];
    box_over(frame, over, OVER_WIDTH, lines);
}

/// The interior width of the pause box (§12.6).
const PAUSE_WIDTH: usize = 18;
/// The interior width of the game-over box (§12.6).
const OVER_WIDTH: usize = 22;
/// Wide enough for one digit and some air.
const COUNTDOWN_WIDTH: usize = 7;

/// Draw `lines` in a cleared, double-bordered box centred over `over`.
fn box_over(frame: &mut Frame, over: Rect, width: usize, lines: Vec<Line<'static>>) {
    let area = centred(over, width as u16 + 2, lines.len() as u16 + 2);
    let block = Block::bordered().border_type(BorderType::Double);
    let interior = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(Text::from(lines)), interior);
}

/// One labelled figure of the game-over box: label left, value right, three
/// characters clear of the border (§12.6).
fn figure(label: &str, value: &str) -> String {
    format!("  {label:<7}{value:>10}   ")
}

/// `text` centred in `width`, padded on both sides so the line fills the box.
fn centre(text: &str, width: usize) -> String {
    let left = width.saturating_sub(text.chars().count()) / 2;
    format!("{:left$}{text:<pad$}", "", pad = width - left)
}

/// Pieces per second over the whole run, to one decimal place (§11).
///
/// Integer arithmetic: the tenths are computed, not rounded off a float, so the
/// figure is the same on every platform.
fn pps(pieces: u32, ticks: u64) -> String {
    if ticks == 0 {
        return "0.0".to_string();
    }
    let tenths = u64::from(pieces) * 600 / ticks;
    format!("{}.{}", tenths / 10, tenths % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §12.6, transcribed literally.
    const PAUSE: &str = "\
╔══════════════════╗
║      PAUSED      ║
║                  ║
║    ▸ Resume      ║
║      Restart     ║
║      Controls    ║
║      Quit to menu║
╚══════════════════╝";

    /// §12.6, transcribed literally.
    const OVER: &str = "\
╔══════════════════════╗
║      GAME OVER       ║
║                      ║
║  SCORE       12480   ║
║  LEVEL           4   ║
║  LINES          37   ║
║  TIME        02:14   ║
║  PIECES        128   ║
║  PPS           0.9   ║
║                      ║
║    Press any key     ║
╚══════════════════════╝";

    fn drawn(width: usize, lines: &[Line<'static>]) -> String {
        let mut out = format!("╔{}╗", "═".repeat(width));
        for line in lines {
            let text: String = line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            out.push_str(&format!("\n║{text}║"));
        }
        out.push_str(&format!("\n╚{}╝", "═".repeat(width)));
        out
    }

    #[test]
    fn the_pause_box_matches_the_spec() {
        let chrome = Chrome {
            theme: crate::ui::theme::Theme::new(crate::ui::theme::Depth::Truecolor),
            show_grid: false,
            hold_enabled: true,
        };
        let mut lines = vec![
            Line::styled(centre("PAUSED", PAUSE_WIDTH), chrome.theme.bold()),
            Line::raw(" ".repeat(PAUSE_WIDTH)),
        ];
        for (index, choice) in PauseChoice::ALL.iter().enumerate() {
            let marker = if index == 0 {
                "    \u{25b8} "
            } else {
                "      "
            };
            lines.push(Line::raw(format!("{marker}{:<12}", choice.label())));
        }
        assert_eq!(drawn(PAUSE_WIDTH, &lines), PAUSE);
    }

    #[test]
    fn the_game_over_box_matches_the_spec() {
        let blank = Line::raw(" ".repeat(OVER_WIDTH));
        let lines = vec![
            Line::raw(centre("GAME OVER", OVER_WIDTH)),
            blank.clone(),
            Line::raw(figure("SCORE", "12480")),
            Line::raw(figure("LEVEL", "4")),
            Line::raw(figure("LINES", "37")),
            Line::raw(figure("TIME", &clock((2 * 60 + 14) * 60))),
            Line::raw(figure("PIECES", "128")),
            Line::raw(figure("PPS", &pps(128, (2 * 60 + 14) * 60))),
            blank,
            Line::raw(centre("Press any key", OVER_WIDTH)),
        ];
        assert_eq!(drawn(OVER_WIDTH, &lines), OVER);
    }

    #[test]
    fn pieces_per_second_is_over_the_whole_run() {
        // §11, and the §12.6 mock-up's own numbers: 128 pieces in 2:14.
        assert_eq!(pps(0, 0), "0.0");
        assert_eq!(pps(128, (2 * 60 + 14) * 60), "0.9");
        assert_eq!(pps(60, 60 * 60), "1.0");
        assert_eq!(pps(150, 60 * 60), "2.5");
    }
}
