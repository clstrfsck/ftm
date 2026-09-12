//! The terminal front-end.
//!
//! Screen dispatch, the terminal-too-small screen (§12.1) and the fixed 44 x 23
//! block every playing screen is drawn in (§12.4). `TUI.md` is normative for
//! everything here.
//!
//! Everything draws from `GameView` alone and never reads `Game` (§12.7), and
//! the §12.5 animations it draws are timed in `shell::cosmetics`, from the
//! event stream and a clock — so the core never knows one is in progress and
//! dropping every event costs nothing but the decoration (§12.8).
//!
//! The clock is one of four things the shell has none of (§3.1), and none of
//! them is a *terminal's* question: all four come from
//! [`crate::native`](crate::native), which the window front-end shares
//! (`EGUI-PLAN.md` G5). What stays here is `cli`, §6.4's grammar, which is the
//! one module below `main` in this directory that names `clap`.

pub mod attract;
pub mod cells;
pub mod cli;
pub mod keys;
pub mod overlays;
pub mod playfield;
pub mod run;
pub mod term;
pub mod theme;

use std::io::Stdout;

use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Rect, Size};
use ratatui::text::Line;
use ratatui::widgets::{Clear, Paragraph};

use crate::core::GameView;
use crate::shell::config::ConfigFile;
use crate::shell::cosmetics::Cosmetics;
use crate::shell::input::InputMode;
use crate::shell::menus::Overlay;
use crate::shell::round::Debug;
use crate::tui::theme::Theme;

/// The terminal the game draws on.
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// §12.1: the minimum supported terminal, in characters.
///
/// The playing screen's block is 44 x 23 (§12.4) and the attract screen's is
/// 36 x 20 (§13.3), so both fit inside this with room to spare; the minimum is
/// the spec's, not the layout's, and neither screen may quietly outgrow it.
pub const MIN_WIDTH: u16 = 60;
/// §12.1: the minimum supported terminal, in rows.
pub const MIN_HEIGHT: u16 = 24;

/// Whether there is room for a screen at all (§12.1).
///
/// Both loops ask this as well as the renderers, because §8.4 gives the answer
/// a consequence beyond drawing: a game in progress is forced into `Paused`
/// rather than played on blind.
pub const fn fits(size: Size) -> bool {
    size.width >= MIN_WIDTH && size.height >= MIN_HEIGHT
}

/// §12.1's replacement for every screen, when the terminal is below the
/// minimum.
///
/// Drawn from `frame.area()` alone, so it says what the terminal actually is
/// and follows it down to sizes at which only the first letter survives. The
/// `Clear` is what rubs out the screen it is replacing: a terminal that has
/// just shrunk still holds the frame drawn at the old size.
pub fn too_small(frame: &mut Frame, theme: Theme) {
    let area = frame.area();
    frame.render_widget(Clear, area);
    let rows = [
        "Terminal too small".to_string(),
        format!(
            "Need {MIN_WIDTH}x{MIN_HEIGHT}, have {}x{}",
            area.width, area.height
        ),
        "Resize to continue".to_string(),
    ];
    let width = rows
        .iter()
        .map(|row| row.chars().count())
        .max()
        .unwrap_or(0);
    let block = centred(area, width as u16, rows.len() as u16);
    // Centred within the room there actually is, not within the room the
    // message wants: on a terminal narrower than the message, padding it to
    // its full width would push the first line off the left of the screen and
    // leave the player looking at a blank one.
    let room = width.min(block.width as usize);
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let style = if index == 0 {
                theme.bold()
            } else {
                theme.plain()
            };
            Line::styled(overlays::centre(row, room), style)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(theme.plain()), block);
}

/// The settings the screen needs that are not game state.
///
/// `GameView` carries what the rules produced (§12.7); this carries what the
/// player and the terminal decided. `hold_enabled` is here because it is the
/// one layout question the view cannot answer: an empty hold slot and an
/// absent hold mechanic look identical in `GameView::hold` (§12.4).
#[derive(Clone, Copy, Debug)]
pub struct Chrome {
    pub theme: Theme,
    pub show_grid: bool,
    pub hold_enabled: bool,
    /// `PILOT.md` §P7.3: whether the pilot is playing this game, which puts the
    /// indicator on the status row. Like `hold_enabled` it is the running
    /// game's answer ([`Round::pilot`](crate::shell::round::Round::pilot)) and
    /// is fixed for the life of the round.
    pub pilot: bool,
}

/// Everything the playing screen shows that is not the game itself.
///
/// Bundled rather than passed one by one: the four travel together, they all
/// come from the shell, and none of them is game state — which is the same
/// reason `Chrome` exists (§12.7).
pub struct Hud<'a> {
    pub overlay: &'a Overlay,
    /// Read by the §13.5 Options panel, which is what edits it.
    pub config: &'a ConfigFile,
    /// The rows that panel offers, which is the list the shell is navigating
    /// (`Session::settings`).
    pub settings: &'static [crate::shell::menus::Setting],
    /// The §12.4 debug strip, when `show_debug` is on.
    pub debug: Option<&'a Debug>,
    /// Which of §8.2's two paths is live, for the controls overlay.
    pub mode: InputMode,
    /// §10.1's restart hold, as a percentage of the second it needs. `None`
    /// when the key is not down.
    pub restart: Option<u8>,
}

/// Draw one frame of the playing screen.
pub fn draw(frame: &mut Frame, view: &GameView, chrome: &Chrome, fx: &Cosmetics, hud: &Hud) {
    // §12.1: below the minimum every screen is replaced by the message, this
    // one included. The check is here rather than only in the loop so that no
    // caller can reach a layout that assumes room it has not got.
    if !fits(frame.area().as_size()) {
        too_small(frame, chrome.theme);
        return;
    }
    let screen = playfield::render(frame, view, chrome, fx, hud.overlay.blanks(), hud);
    match hud.overlay {
        Overlay::None => {}
        Overlay::Paused { selected } => overlays::paused(frame, screen, chrome, *selected),
        Overlay::Options { selected } => {
            overlays::options(frame, screen, chrome, hud.config, hud.settings, *selected)
        }
        Overlay::Resuming { count } => overlays::resuming(frame, screen, chrome, *count),
        Overlay::GameOver => overlays::game_over(frame, screen, view, chrome),
        Overlay::NameEntry { rank, name } => {
            overlays::name_entry(frame, screen, chrome, *rank, name)
        }
        Overlay::Controls => overlays::controls(frame, screen, chrome, hud.config, hud.mode),
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

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // §12.1 the terminal-too-small screen
    // -----------------------------------------------------------------------

    /// Draw the §12.1 screen at `width` x `height` and read the characters
    /// back, trailing blanks trimmed.
    fn small_screen(width: u16, height: u16) -> Vec<String> {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("a test terminal");
        terminal
            .draw(|frame| too_small(frame, Theme::new(crate::tui::theme::Depth::Truecolor)))
            .expect("a frame");
        let buffer = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn the_minimum_is_sixty_by_twenty_four() {
        // §12.1, exactly: 60 x 24 fits and one short in either direction does
        // not. The playing screen's own block is 44 x 23, so this boundary is
        // the spec's rather than the layout's and has to be asserted as such.
        assert_eq!((MIN_WIDTH, MIN_HEIGHT), (60, 24));
        let at = |width, height| fits(Size { width, height });
        assert!(at(MIN_WIDTH, MIN_HEIGHT));
        assert!(at(200, 60));
        assert!(!at(MIN_WIDTH - 1, MIN_HEIGHT));
        assert!(!at(MIN_WIDTH, MIN_HEIGHT - 1));
        assert!(!at(1, 1));
    }

    #[test]
    fn the_too_small_screen_says_what_is_needed_and_what_there_is() {
        // §12.1's three lines, verbatim, with the terminal's actual size in
        // the second one — which is what makes it useful while resizing.
        let drawn = small_screen(48, 20);
        let text: Vec<&str> = drawn
            .iter()
            .map(String::as_str)
            .filter(|row| !row.is_empty())
            .collect();
        assert_eq!(
            text,
            [
                "               Terminal too small",
                "             Need 60x24, have 48x20",
                "               Resize to continue",
            ],
        );
    }

    #[test]
    fn the_message_shrinks_rather_than_panicking() {
        // I4: 1 x 1 must render. The message cannot fit, so it is clipped to
        // what there is; the requirement is only that nothing panics and the
        // terminal is not scribbled outside its own bounds.
        assert_eq!(small_screen(1, 1), ["T"]);
        // 21 characters of message in 20 columns: the last is simply lost.
        assert_eq!(
            small_screen(20, 3),
            [
                " Terminal too small",
                "Need 60x24, have 20x",
                " Resize to continue",
            ],
        );
    }
}
