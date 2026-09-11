//! Pause, game-over and name-entry overlays (§12.6).
//!
//! Overlays are drawn centred over the playfield, on a cleared background with
//! a double-line border. Like everything else in `tui` they read `GameView` and
//! never `Game` (§12.7).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::core::GameView;
use crate::shell::config::ConfigFile;
use crate::shell::figures::{clock, pps, thousands};
use crate::shell::highscore::NAME_MAX;
use crate::shell::input::InputMode;
use crate::shell::menus::{self, PauseChoice, Setting};
use crate::tui::{Chrome, centred};

/// The pause overlay (§12.6). The playfield underneath it has already been
/// blanked (§9.17); this is only the menu.
pub fn paused(frame: &mut Frame, over: Rect, chrome: &Chrome, selected: usize) {
    let mut lines = vec![
        Line::styled(centre("PAUSED", PAUSE_WIDTH), chrome.theme.bold()),
        Line::raw(" ".repeat(PAUSE_WIDTH)),
    ];
    for (index, choice) in PauseChoice::ALL.iter().enumerate() {
        let marker = if index == selected { "\u{25b8} " } else { "  " };
        let style = if index == selected {
            chrome.theme.bold()
        } else {
            chrome.theme.plain()
        };
        // The marker and the widest label are `MARKER + CHOICE` characters
        // together, and that block is what is centred -- not each label on its
        // own, which would shuffle them sideways as the cursor moved, and not
        // the marker alone, which is what left the block hard against the
        // right wall with four characters of air on the left.
        let row = format!("{marker}{:<CHOICE$}", choice.label());
        lines.push(Line::styled(centre(&row, PAUSE_WIDTH), style));
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
        Line::raw(figure("SCORE", &thousands(view.score))),
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

/// Name entry (§12.6), shown only when the score qualifies for the top ten.
///
/// `rank` is one-based, as the box prints it.
pub fn name_entry(frame: &mut Frame, over: Rect, chrome: &Chrome, rank: usize, name: &str) {
    let blank = Line::raw(" ".repeat(OVER_WIDTH));
    let lines = vec![
        Line::styled(centre("NEW HIGH SCORE", OVER_WIDTH), chrome.theme.bold()),
        Line::raw(centre(&format!("#{rank}"), OVER_WIDTH)),
        blank.clone(),
        Line::styled(field(name), chrome.theme.bold()),
        blank,
        Line::styled(centre("Enter to confirm", OVER_WIDTH), chrome.theme.faint()),
    ];
    box_over(frame, over, OVER_WIDTH, lines);
}

/// The editable field with its cursor (§12.6), padded so the longest name
/// still fits inside the box without moving anything.
fn field(name: &str) -> String {
    format!(
        "   Name: {:<width$}",
        format!("{name}_"),
        width = NAME_MAX + 1,
    )
}

/// The §13.5 Options panel, over the paused playfield (§12.6).
///
/// It reads a `ConfigFile` rather than a `GameView`, which is not a breach of
/// §12.7: the config is presentation and rules settings, not game state, and
/// the panel is what edits them.
pub fn options(
    frame: &mut Frame,
    over: Rect,
    chrome: &Chrome,
    file: &ConfigFile,
    settings: &[Setting],
    selected: usize,
) {
    let mut lines = vec![
        Line::styled(centre("OPTIONS", OPTIONS_WIDTH), chrome.theme.bold()),
        Line::raw(" ".repeat(OPTIONS_WIDTH)),
    ];
    // The rows the *shell* is navigating (`Session::settings`), which for this
    // front-end is every row §13.5 lists. Drawing any other list would be a
    // cursor that lands where nothing is drawn.
    for (index, setting) in settings.iter().enumerate() {
        let marker = if index == selected { "\u{25b8} " } else { "  " };
        let style = if index == selected {
            chrome.theme.bold()
        } else {
            chrome.theme.plain()
        };
        lines.push(Line::styled(
            format!(
                "  {marker}{:<13}{:>9}  ",
                setting.label(),
                setting.value(file),
            ),
            style,
        ));
    }
    lines.push(Line::raw(" ".repeat(OPTIONS_WIDTH)));
    lines.push(Line::styled(
        centre("\u{2190}\u{2192} change   Esc saves", OPTIONS_WIDTH),
        chrome.theme.faint(),
    ));
    box_over(frame, over, OPTIONS_WIDTH, lines);
}

/// Every action of §10.1 and the key names bound to it, plus §8.2's live path.
///
/// Two ways in and one box: §13.5's CONTROLS item on the attract screen, and
/// §12.6's Controls item on the pause menu. Like the Options panel it reads a
/// `ConfigFile` rather than a `GameView`, which is not a breach of §12.7 — the
/// bindings are a setting, not game state.
pub fn controls(
    frame: &mut Frame,
    over: Rect,
    chrome: &Chrome,
    file: &ConfigFile,
    mode: InputMode,
) {
    let theme = chrome.theme;
    let mut lines = vec![
        Line::styled(centre("CONTROLS", CONTROLS_WIDTH), theme.bold()),
        Line::raw(" ".repeat(CONTROLS_WIDTH)),
    ];
    // The words and the order are the specification's and both front-ends show
    // them, so they live in `shell::menus` — including §13.3's rule that a
    // binding whose setting is off is not listed at all (A9).
    for (label, keys) in menus::controls(file) {
        lines.push(Line::raw(format!("  {label:<24}{keys:>14}  ")));
    }
    lines.push(Line::raw(" ".repeat(CONTROLS_WIDTH)));
    lines.push(Line::styled(
        centre(
            &format!("input: {}   Esc returns", mode.name()),
            CONTROLS_WIDTH,
        ),
        theme.faint(),
    ));
    box_over(frame, over, CONTROLS_WIDTH, lines);
}

/// The interior width of the controls box: the longest action name, the
/// longest list of key names, and a margin either side.
const CONTROLS_WIDTH: usize = 42;

/// The interior width of the pause box (§12.6).
const PAUSE_WIDTH: usize = 18;
/// The longest pause-menu label, `Quit to menu` (§9.17).
const CHOICE: usize = 12;
/// The interior width of the game-over box (§12.6).
const OVER_WIDTH: usize = 22;
/// Wide enough for one digit and some air.
const COUNTDOWN_WIDTH: usize = 7;
/// The interior width of the §13.5 Options panel: a marker, a 13-character
/// label and the longest value (`truecolor`), with a margin either side.
const OPTIONS_WIDTH: usize = 28;

/// Draw `lines` in a cleared, double-bordered box centred over `over`.
pub fn box_over(frame: &mut Frame, over: Rect, width: usize, lines: Vec<Line<'static>>) {
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
pub fn centre(text: &str, width: usize) -> String {
    let left = width.saturating_sub(text.chars().count()) / 2;
    format!("{:left$}{text:<pad$}", "", pad = width - left)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §12.6, transcribed literally.
    const PAUSE: &str = "\
╔══════════════════╗
║      PAUSED      ║
║                  ║
║  ▸ Resume        ║
║    Restart       ║
║    Options       ║
║    Controls      ║
║    Quit to menu  ║
╚══════════════════╝";

    /// §12.6, transcribed literally.
    const OVER: &str = "\
╔══════════════════════╗
║      GAME OVER       ║
║                      ║
║  SCORE      12,480   ║
║  LEVEL           4   ║
║  LINES          37   ║
║  TIME        02:14   ║
║  PIECES        128   ║
║  PPS           0.9   ║
║                      ║
║    Press any key     ║
╚══════════════════════╝";

    fn chrome() -> Chrome {
        Chrome {
            theme: crate::tui::theme::Theme::new(crate::tui::theme::Depth::Truecolor),
            show_grid: false,
            hold_enabled: true,
        }
    }

    /// Render one overlay into a terminal exactly the size of its box.
    ///
    /// What is compared is then what the function actually draws — border,
    /// padding and all. The transcription this replaced rebuilt the lines by
    /// hand and never called the overlay at all, so it could only prove that
    /// two copies of the same code agreed; when the game-over box learned to
    /// group its digits (§12.4) the copy did not, and nothing failed.
    fn shot(width: usize, rows: usize, draw: impl FnOnce(&mut Frame, Rect)) -> String {
        let (width, height) = (width as u16 + 2, rows as u16 + 2);
        let area = Rect {
            x: 0,
            y: 0,
            width,
            height,
        };
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
                .expect("a test terminal");
        terminal
            .draw(|frame| draw(frame, area))
            .expect("drew a frame");
        let buffer = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The view the §12.6 game-over mock-up depicts.
    fn over_view() -> GameView {
        let mut view = crate::tui::playfield::tests::empty_view();
        view.score = 12_480;
        view.level = 4;
        view.lines = 37;
        view.ticks = (2 * 60 + 14) * 60;
        view.pieces = 128;
        view
    }

    #[test]
    fn the_pause_box_matches_the_spec() {
        assert_eq!(
            shot(PAUSE_WIDTH, 7, |frame, area| paused(
                frame,
                area,
                &chrome(),
                0
            )),
            PAUSE,
        );
    }

    #[test]
    fn the_game_over_box_matches_the_spec() {
        assert_eq!(
            shot(OVER_WIDTH, 10, |frame, area| game_over(
                frame,
                area,
                &over_view(),
                &chrome()
            )),
            OVER,
        );
    }

    /// §12.6, transcribed literally.
    const NAME: &str = "\
╔══════════════════════╗
║    NEW HIGH SCORE    ║
║          #3          ║
║                      ║
║   Name: msandifo_    ║
║                      ║
║   Enter to confirm   ║
╚══════════════════════╝";

    #[test]
    fn the_name_entry_box_matches_the_spec() {
        assert_eq!(
            shot(OVER_WIDTH, 6, |frame, area| name_entry(
                frame,
                area,
                &chrome(),
                3,
                "msandifo"
            )),
            NAME,
        );
    }

    #[test]
    fn the_longest_name_still_fits_the_field() {
        // The field is padded to the longest name plus its cursor, so nothing
        // in the box moves as the player types.
        let longest = "M".repeat(NAME_MAX);
        assert_eq!(field(&longest).chars().count(), OVER_WIDTH);
        assert_eq!(field("").chars().count(), OVER_WIDTH);
    }

    /// §13.5's panel, as the code draws it. Not in the specification as a
    /// mock-up — §12.6 draws only its three boxes — so this pins the layout the
    /// same way, and would catch a label or a value column drifting.
    const OPTIONS: &str = "\
╔════════════════════════════╗
║          OPTIONS           ║
║                            ║
║  ▸ Preview              5  ║
║    Start level          1  ║
║    Ghost piece         on  ║
║    Hold                on  ║
║    180 rotation        on  ║
║    Lock down     extended  ║
║    Colour            auto  ║
║    Grid               off  ║
║                            ║
║   ←→ change   Esc saves    ║
╚════════════════════════════╝";

    #[test]
    fn the_options_panel_lists_every_setting_in_thirteen_five() {
        let file = ConfigFile::default();
        assert_eq!(
            shot(OPTIONS_WIDTH, 12, |frame, area| options(
                frame,
                area,
                &chrome(),
                &file,
                &Setting::ALL,
                0
            )),
            OPTIONS,
        );
    }

    #[test]
    fn the_panel_fits_inside_the_block() {
        // §12.6: an overlay is centred over the whole 44 x 23 block, so it must
        // not be wider than one.
        assert!(OPTIONS_WIDTH + 2 <= crate::tui::playfield::SCREEN_WIDTH as usize);
        assert!(Setting::ALL.len() + 4 + 2 <= crate::tui::playfield::SCREEN_HEIGHT as usize);
    }
}
