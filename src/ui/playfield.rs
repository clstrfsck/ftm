//! The in-game screen (§12.4).
//!
//! A fixed block of **44 characters by 23 rows**, centred in the terminal, laid
//! out as three columns: hold and stats on the left, the playfield in the
//! middle, the preview queue on the right, and the status line underneath. The
//! mock-up in §12.4 is drawn to exact size and this reproduces it character for
//! character; `the_screen_matches_the_spec_mock_up` is the proof.
//!
//! Everything is taken from [`GameView`] and nothing reads `Game` (§12.7). The
//! §12.5 animations arrive as a [`Cosmetics`], which is itself built from
//! nothing but the event stream (§12.8).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::core::GameView;
use crate::core::geometry::Rotation;
use crate::core::piece::PieceKind;
use crate::core::view::{VIEW_HEIGHT, VIEW_WIDTH};
use crate::ui::cells::{CELL_WIDTH, Paint, span};
use crate::ui::theme;
use crate::ui::{Banner, Chrome, Cosmetics, centred};

/// The whole screen, in characters (§12.4).
pub const SCREEN_WIDTH: u16 = 44;
/// The whole screen, in rows (§12.4).
pub const SCREEN_HEIGHT: u16 = 23;

/// The hold, stats and next boxes: 4 cells of interior plus a border (§12.4).
const COLUMN_WIDTH: u16 = PANEL_CELLS * CELL_WIDTH + 2;
/// The interior of the hold and next boxes, in cells — enough for any piece in
/// its `North` orientation (§12.4).
const PANEL_CELLS: u16 = 4;
/// The playfield box: 10 cells of interior plus its border.
const FIELD_WIDTH: u16 = VIEW_WIDTH as u16 * CELL_WIDTH + 2;
/// Twenty visible rows plus the border.
const FIELD_HEIGHT: u16 = VIEW_HEIGHT as u16 + 2;
/// One blank column between boxes: 10 + 1 + 22 + 1 + 10 = 44.
const GUTTER: u16 = 1;
const FIELD_X: u16 = COLUMN_WIDTH + GUTTER;
const NEXT_X: u16 = FIELD_X + FIELD_WIDTH + GUTTER;
/// A label row and two cell rows, plus the border.
const HOLD_HEIGHT: u16 = 5;
/// Four labelled figures, blank-separated, plus the border.
const STATS_HEIGHT: u16 = 13;
/// The status line sits on the last row of the block, under the playfield.
const STATUS_Y: u16 = SCREEN_HEIGHT - 1;
/// The interior width of every box in the two side columns, in characters.
const LABEL_WIDTH: usize = (PANEL_CELLS * CELL_WIDTH) as usize;

/// Draw the playing screen, returning the whole block so an overlay can be
/// centred over it (§12.6).
///
/// The block rather than the playfield's interior, because §12.6's game-over
/// box is 24 characters wide and the interior is 20. It comes to the same thing
/// for a box that does fit: the playfield is itself centred in the block, so a
/// 20-wide overlay lands exactly over it either way.
///
/// `blanked` is §9.17's anti-pause-scumming rule: while the game is paused the
/// cells are drawn empty, and the player gets no free look at the stack.
pub fn render(
    frame: &mut Frame,
    view: &GameView,
    chrome: &Chrome,
    fx: &Cosmetics,
    blanked: bool,
) -> Rect {
    let screen = centred(frame.area(), SCREEN_WIDTH, SCREEN_HEIGHT);
    let at = |x: u16, y: u16, width: u16, height: u16| Rect {
        x: screen.x + x,
        y: screen.y + y,
        width,
        height,
    };

    // §12.4: with `hold_enabled = false` the hold box is omitted entirely and
    // the left column holds nothing but the stats.
    let stats_y = if chrome.hold_enabled {
        panel(
            frame,
            at(0, 0, COLUMN_WIDTH, HOLD_HEIGHT),
            hold(view, chrome),
        );
        HOLD_HEIGHT + 1
    } else {
        0
    };
    panel(
        frame,
        at(0, stats_y, COLUMN_WIDTH, STATS_HEIGHT),
        stats(view),
    );

    // TODO(stage 12): §12.4's "+N" rule for a layout with too little room for
    // every slot. At `preview_count` up to 6 the box is 20 rows and the screen
    // is 23, so within this layout it cannot arise.
    let next_height = 3 * view.next.len() as u16 + 2;
    panel(
        frame,
        at(NEXT_X, 0, COLUMN_WIDTH, next_height),
        next(view, chrome),
    );

    let block = Block::bordered().border_style(chrome.theme.plain());
    let outer = at(FIELD_X, 0, FIELD_WIDTH, FIELD_HEIGHT);
    let interior = block.inner(outer);
    frame.render_widget(block, outer);
    frame.render_widget(Paragraph::new(field(view, chrome, fx, blanked)), interior);
    if let Some(banner) = fx.banner() {
        overlay_banner(frame, interior, chrome, banner);
    }

    frame.render_widget(
        Paragraph::new(status(view, fx)).style(chrome.theme.plain()),
        at(0, STATUS_Y, SCREEN_WIDTH, 1),
    );
    screen
}

/// A bordered box with `lines` inside it.
fn panel(frame: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
    let block = Block::bordered();
    let interior = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines), interior);
}

/// The hold box (§12.4), dimmed while hold is locked out for this piece (§9.7).
fn hold(view: &GameView, chrome: &Chrome) -> Vec<Line<'static>> {
    let percent = if view.hold_locked {
        theme::GHOST
    } else {
        theme::FULL
    };
    let style = if view.hold_locked {
        chrome.theme.faint()
    } else {
        chrome.theme.plain()
    };
    let mut lines = vec![Line::styled(label("HOLD"), style)];
    lines.extend(slot(chrome, view.hold, percent));
    lines
}

/// The preview queue (§12.4): slot 0 at full brightness, then three steps down.
fn next(view: &GameView, chrome: &Chrome) -> Vec<Line<'static>> {
    let mut lines = vec![Line::styled(label("NEXT"), chrome.theme.plain())];
    for (index, kind) in view.next.iter().enumerate() {
        if index > 0 {
            lines.push(blank());
        }
        let percent = match index {
            0 => theme::FULL,
            1 => theme::SLOT_NEAR,
            _ => theme::SLOT_FAR,
        };
        lines.extend(slot(chrome, Some(*kind), percent));
    }
    lines
}

/// The stats box (§12.4). Elapsed time is counted in ticks and converted here,
/// so it advances with the game rather than with the wall clock (§11).
fn stats(view: &GameView) -> Vec<Line<'static>> {
    vec![
        Line::raw(label("SCORE")),
        Line::raw(figure(view.score)),
        blank(),
        Line::raw(label("LEVEL")),
        Line::raw(figure(u64::from(view.level))),
        blank(),
        Line::raw(label("LINES")),
        Line::raw(figure(u64::from(view.lines))),
        blank(),
        Line::raw(label("TIME")),
        Line::raw(format!(
            "{:>width$} ",
            clock(view.ticks),
            width = LABEL_WIDTH - 1
        )),
    ]
}

/// `MM:SS`, capped at `99:59` (§12.4).
pub fn clock(ticks: u64) -> String {
    let seconds = (ticks / 60).min(99 * 60 + 59);
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// A box label: left-aligned, the full interior width.
fn label(text: &str) -> String {
    format!(" {text:<width$}", width = LABEL_WIDTH - 1)
}

/// A stats figure: right-aligned, one character clear of the border.
fn figure(value: u64) -> String {
    format!("{value:>width$} ", width = LABEL_WIDTH - 1)
}

fn blank() -> Line<'static> {
    Line::raw(" ".repeat(LABEL_WIDTH))
}

/// One two-row preview slot: a piece in its `North` orientation, centred
/// horizontally in the four-cell interior (§12.4).
///
/// The piece's bounding-box rows are used as they are, which is why `I` — whose
/// only occupied row is the second of its 4 x 4 box — sits on the lower of the
/// two rows and everything else on both.
fn slot(chrome: &Chrome, kind: Option<PieceKind>, percent: u8) -> [Line<'static>; 2] {
    let mut grid = [[Paint::Empty; PANEL_CELLS as usize]; 2];
    if let Some(kind) = kind {
        let minos = kind.cells(Rotation::North);
        let width = minos.iter().map(|m| m.x).max().unwrap_or(0) + 1;
        let offset = (i32::from(PANEL_CELLS) - width) / 2;
        for mino in minos {
            let (x, y) = ((mino.x + offset) as usize, mino.y as usize);
            if let Some(cell) = grid.get_mut(y).and_then(|row| row.get_mut(x)) {
                *cell = Paint::Filled(kind, percent);
            }
        }
    }
    grid.map(|row| paint(chrome, &row))
}

/// The visible rows, with the ghost, the falling piece and the §12.5
/// animations composited in.
///
/// The view arrives already clipped to rows 20..=39 with the buffer zone
/// removed (§12.7), so there is no clipping to do here — that is the point of
/// the view model.
fn field(view: &GameView, chrome: &Chrome, fx: &Cosmetics, blanked: bool) -> Vec<Line<'static>> {
    let mut grid = [[Paint::Empty; VIEW_WIDTH]; VIEW_HEIGHT];
    if !blanked {
        compose(&mut grid, view, fx);
    }
    grid.iter().map(|row| paint(chrome, row)).collect()
}

/// Everything that can occupy a cell, in the order it is allowed to win.
fn compose(grid: &mut [[Paint; VIEW_WIDTH]; VIEW_HEIGHT], view: &GameView, fx: &Cosmetics) {
    for (row, cells) in view.rows.iter().enumerate() {
        for (col, cell) in cells.iter().enumerate() {
            if let Some(kind) = cell {
                put(grid, (col as u8, row as u8), Paint::filled(*kind));
            }
        }
    }
    // The trail is drawn *behind* the piece (§12.5), so it goes down first and
    // never over a mino that is really there.
    if let Some((kind, cells)) = fx.trail() {
        for &(col, row) in cells {
            if grid[row as usize][col as usize] == Paint::Empty {
                put(grid, (col, row), Paint::Ghost(kind));
            }
        }
    }
    for &cell in fx.lock_flash() {
        put(grid, cell, Paint::Flash);
    }
    // §9.8: the ghost goes down before the falling piece, so that where the two
    // overlap the piece is what is drawn.
    for (piece, paint) in [
        (&view.ghost, Paint::Ghost as fn(PieceKind) -> Paint),
        (&view.current, Paint::filled as fn(PieceKind) -> Paint),
    ] {
        let Some(piece) = piece else { continue };
        for &cell in &piece.cells {
            put(grid, cell, paint(piece.kind));
        }
    }
    // The clear flash alternates white and the piece's own colour (§12.5): on
    // the coloured half there is nothing to do, because that is what is there.
    for (index, row) in grid.iter_mut().enumerate() {
        if fx.flashing(index as u8) == Some(true) {
            for cell in row.iter_mut() {
                if *cell != Paint::Empty {
                    *cell = Paint::Flash;
                }
            }
        }
    }
    // The game-over wipe greys everything it has reached (§12.5).
    for row in grid
        .iter_mut()
        .take(fx.wiped_rows(VIEW_HEIGHT as u8) as usize)
    {
        for cell in row.iter_mut() {
            if *cell != Paint::Empty {
                *cell = Paint::Greyed;
            }
        }
    }
}

/// Set one cell, ignoring a coordinate that is not on the field — which is how
/// [`OFF_SCREEN`](crate::core::events::OFF_SCREEN) minos are dropped (§12.7).
fn put(grid: &mut [[Paint; VIEW_WIDTH]; VIEW_HEIGHT], (col, row): (u8, u8), paint: Paint) {
    if let Some(cell) = grid
        .get_mut(row as usize)
        .and_then(|row| row.get_mut(col as usize))
    {
        *cell = paint;
    }
}

fn paint(chrome: &Chrome, row: &[Paint]) -> Line<'static> {
    Line::from(
        row.iter()
            .map(|cell| span(chrome.theme, *cell, chrome.show_grid))
            .collect::<Vec<_>>(),
    )
}

/// The status line (§12.4): the standing indicators, then the most recent
/// clear's name while it lasts, centred under the playfield.
///
/// The padding is computed here rather than left to the renderer's alignment,
/// so that "centred" means one thing and the mock-up can be compared against it
/// character for character.
fn status(view: &GameView, fx: &Cosmetics) -> String {
    let mut parts: Vec<String> = Vec::new();
    if view.back_to_back {
        parts.push("B2B".to_string());
    }
    if view.combo >= 1 {
        parts.push(format!("COMBO x{}", view.combo));
    }
    if let Some(name) = fx.clear_name() {
        parts.push(name.to_string());
    }
    let text = parts.join("  ");
    let left = (SCREEN_WIDTH as usize).saturating_sub(text.chars().count()) / 2;
    format!("{:left$}{text}", "")
}

/// A banner centred over the playfield (§12.5).
fn overlay_banner(frame: &mut Frame, field: Rect, chrome: &Chrome, banner: Banner) {
    let (text, style) = match banner {
        Banner::LevelUp(level, left) => (format!("LEVEL {level}"), chrome.theme.faded(left)),
        Banner::PerfectClear(hue) => (
            "PERFECT CLEAR".to_string(),
            chrome
                .theme
                .piece(hue, theme::FULL)
                .patch(chrome.theme.bold()),
        ),
    };
    let width = text.chars().count() as u16;
    let area = centred(field, width, 1);
    frame.render_widget(Paragraph::new(Line::from(Span::styled(text, style))), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::PlayState;
    use crate::core::view::PieceView;
    use crate::ui::theme::{Depth, Theme};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use std::time::{Duration, Instant};

    /// The §12.4 mock-up, transcribed literally. It is drawn to exact size and
    /// is the acceptance criterion for this stage, so it is compared character
    /// for character rather than approximated.
    const MOCK_UP: &str = "\
┌────────┐ ┌────────────────────┐ ┌────────┐
│ HOLD   │ │                    │ │ NEXT   │
│  ██    │ │                    │ │  ████  │
│██████  │ │                    │ │  ████  │
└────────┘ │        ██          │ │        │
           │      ██████        │ │██      │
┌────────┐ │                    │ │██████  │
│ SCORE  │ │                    │ │        │
│  12480 │ │                    │ │    ██  │
│        │ │                    │ │██████  │
│ LEVEL  │ │                    │ │        │
│      4 │ │                    │ │        │
│        │ │                    │ │████████│
│ LINES  │ │                    │ │        │
│     37 │ │                    │ │  ██    │
│        │ │                    │ │██████  │
│ TIME   │ │                    │ └────────┘
│  02:14 │ │        ▒▒          │           
└────────┘ │      ▒▒▒▒▒▒        │           
           │      ██████████    │           
           │██████████████████  │           
           └────────────────────┘           
               B2B  COMBO x3                ";

    fn chrome() -> Chrome {
        Chrome {
            // Truecolor so the test is about characters, not about which
            // fallback this machine's terminal would have chosen.
            theme: Theme::new(Depth::Truecolor),
            show_grid: false,
            hold_enabled: true,
        }
    }

    fn empty_view() -> GameView {
        GameView {
            rows: [[None; VIEW_WIDTH]; VIEW_HEIGHT],
            current: None,
            ghost: None,
            hold: None,
            hold_locked: false,
            next: Vec::new(),
            score: 0,
            level: 1,
            lines: 0,
            ticks: 0,
            pieces: 0,
            combo: -1,
            back_to_back: false,
            state: PlayState::Falling,
        }
    }

    /// The exact state the §12.4 mock-up depicts.
    fn mock_up_view() -> GameView {
        let mut view = empty_view();
        view.hold = Some(PieceKind::T);
        view.next = vec![
            PieceKind::O,
            PieceKind::J,
            PieceKind::L,
            PieceKind::I,
            PieceKind::T,
        ];
        view.score = 12_480;
        view.level = 4;
        view.lines = 37;
        view.ticks = (2 * 60 + 14) * 60;
        view.combo = 3;
        view.back_to_back = true;
        // A T falling at rows 3-4, its ghost resting on the stack at 16-17.
        view.current = Some(PieceView {
            kind: PieceKind::T,
            cells: [(4, 3), (3, 4), (4, 4), (5, 4)],
        });
        view.ghost = Some(PieceView {
            kind: PieceKind::T,
            cells: [(4, 16), (3, 17), (4, 17), (5, 17)],
        });
        for col in 3..=7 {
            view.rows[18][col] = Some(PieceKind::L);
        }
        for col in 0..=8 {
            view.rows[19][col] = Some(PieceKind::J);
        }
        view
    }

    /// Render one frame at the block's exact size and read the characters back.
    fn screenshot(view: &GameView, chrome: &Chrome) -> String {
        let backend = TestBackend::new(SCREEN_WIDTH, SCREEN_HEIGHT);
        let mut terminal = Terminal::new(backend).expect("a test terminal");
        let fx = Cosmetics::new(Duration::from_millis(250), Instant::now());
        terminal
            .draw(|frame| {
                render(frame, view, chrome, &fx, false);
            })
            .expect("a frame");
        let buffer = terminal.backend().buffer().clone();
        (0..SCREEN_HEIGHT)
            .map(|y| {
                (0..SCREEN_WIDTH)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_screen_matches_the_spec_mock_up() {
        // The Stage 9 acceptance criterion, and the reason §12.4's mock-up is
        // drawn to scale: every box, every label and every glyph, in place.
        let drawn = screenshot(&mock_up_view(), &chrome());
        assert_eq!(
            drawn, MOCK_UP,
            "\n--- drawn ---\n{drawn}\n--- §12.4 ---\n{MOCK_UP}\n",
        );
    }

    #[test]
    fn the_block_is_forty_four_by_twenty_three() {
        // §12.4, and the arithmetic it prints: 10 + 1 + 22 + 1 + 10 = 44.
        assert_eq!(
            COLUMN_WIDTH + GUTTER + FIELD_WIDTH + GUTTER + COLUMN_WIDTH,
            SCREEN_WIDTH
        );
        assert_eq!(MOCK_UP.lines().count(), SCREEN_HEIGHT as usize);
        for line in MOCK_UP.lines() {
            assert_eq!(line.chars().count(), SCREEN_WIDTH as usize, "{line:?}");
        }
    }

    #[test]
    fn an_empty_board_draws_an_empty_field() {
        let drawn = screenshot(&empty_view(), &chrome());
        let rows: Vec<&str> = drawn.lines().collect();
        for row in &rows[1..21] {
            let interior: String = row.chars().skip(12).take(20).collect();
            assert_eq!(interior, " ".repeat(20), "{row:?}");
        }
        assert_eq!(rows[22].trim(), "", "no status line on a fresh board");
    }

    #[test]
    fn the_next_box_is_sized_to_the_preview_count() {
        // §12.4: 2 (border) + 1 (label) + 2 x count + (count - 1).
        for count in 1..=6usize {
            let mut view = empty_view();
            view.next = vec![PieceKind::I; count];
            let drawn = screenshot(&view, &chrome());
            let bottom = drawn
                .lines()
                .position(|line| line.chars().nth(NEXT_X as usize) == Some('└'))
                .expect("the next box has a bottom");
            assert_eq!(bottom + 1, 3 * count + 2, "at preview_count = {count}");
        }
    }

    #[test]
    fn dropping_the_hold_box_leaves_the_stats_at_the_top() {
        // §12.4: with `hold_enabled = false` the hold box is omitted entirely
        // and the left column contains only the stats box.
        let chrome = Chrome {
            hold_enabled: false,
            ..chrome()
        };
        let drawn = screenshot(&mock_up_view(), &chrome);
        let rows: Vec<&str> = drawn.lines().collect();
        assert!(rows[1].starts_with("│ SCORE  │"), "{:?}", rows[1]);
        assert!(rows[12].starts_with("└────────┘"), "{:?}", rows[12]);
        assert!(rows[2].starts_with("│  12480 │"));
    }

    /// The foreground colour of one character cell of the rendered screen.
    fn colour_at(view: &GameView, chrome: &Chrome, x: u16, y: u16) -> Color {
        let backend = TestBackend::new(SCREEN_WIDTH, SCREEN_HEIGHT);
        let mut terminal = Terminal::new(backend).expect("a test terminal");
        let fx = Cosmetics::new(Duration::from_millis(250), Instant::now());
        terminal
            .draw(|frame| {
                render(frame, view, chrome, &fx, false);
            })
            .expect("a frame");
        terminal.backend().buffer()[(x, y)].fg
    }

    /// A truecolor piece colour at `percent` of full brightness (§12.3).
    fn shade((r, g, b): (u32, u32, u32), percent: u32) -> Color {
        let dim = |c: u32| (c * percent / 100) as u8;
        Color::Rgb(dim(r), dim(g), dim(b))
    }

    #[test]
    fn the_preview_slots_dim_in_three_steps() {
        // §12.4: slot 0 at full brightness, slot 1 at 75 %, slot 2 and beyond
        // at 55 %. Four identical `O`s, so only the brightness can differ.
        let mut view = empty_view();
        view.next = vec![PieceKind::O; 4];
        let chrome = chrome();
        // The left mino of an `O`, on the first row of each slot.
        let x = NEXT_X + 1 + 2;
        const YELLOW: (u32, u32, u32) = (0xF0, 0xF0, 0x00);
        assert_eq!(
            colour_at(&view, &chrome, x, 2),
            shade(YELLOW, 100),
            "slot 0"
        );
        assert_eq!(colour_at(&view, &chrome, x, 5), shade(YELLOW, 75), "slot 1");
        assert_eq!(colour_at(&view, &chrome, x, 8), shade(YELLOW, 55), "slot 2");
        assert_eq!(
            colour_at(&view, &chrome, x, 11),
            shade(YELLOW, 55),
            "slot 3"
        );
    }

    #[test]
    fn a_locked_out_hold_box_is_dimmed() {
        // §12.4: "drawn dimmed when hold is locked out for the current piece"
        // (§9.7), which is the only way the screen says the key is spent.
        let mut view = empty_view();
        view.hold = Some(PieceKind::T);
        let chrome = chrome();
        // The stem of a `T` in the hold box, on the label row's first cell row.
        let (x, y) = (1 + 2, 2);
        const PURPLE: (u32, u32, u32) = (0xA0, 0x00, 0xF0);
        assert_eq!(
            colour_at(&view, &chrome, x, y),
            shade(PURPLE, 100),
            "available",
        );
        view.hold_locked = true;
        assert_eq!(colour_at(&view, &chrome, x, y), shade(PURPLE, 45), "spent");
    }

    #[test]
    fn the_grid_draws_dots_in_the_empty_field() {
        // §12.2: an empty cell is two spaces, or `··` dimmed with `show_grid`.
        let chrome = Chrome {
            show_grid: true,
            ..chrome()
        };
        let drawn = screenshot(&empty_view(), &chrome);
        let row = drawn.lines().nth(1).expect("the first field row");
        let interior: String = row.chars().skip(12).take(20).collect();
        assert_eq!(interior, "·".repeat(20));
    }

    #[test]
    fn a_paused_playfield_shows_nothing() {
        // §9.17: the cells are drawn empty, so pausing buys no free look at
        // the stack. The boxes around it stay.
        let backend = TestBackend::new(SCREEN_WIDTH, SCREEN_HEIGHT);
        let mut terminal = Terminal::new(backend).expect("a test terminal");
        let fx = Cosmetics::new(Duration::from_millis(250), Instant::now());
        let view = mock_up_view();
        let chrome = chrome();
        terminal
            .draw(|frame| {
                render(frame, &view, &chrome, &fx, true);
            })
            .expect("a frame");
        let buffer = terminal.backend().buffer().clone();
        for y in 1..21 {
            for x in 12..32 {
                assert_eq!(buffer[(x, y)].symbol(), " ", "at {x},{y}");
            }
        }
        assert_eq!(buffer[(2, 7)].symbol(), "S", "the stats box is still drawn");
    }

    #[test]
    fn the_clock_counts_ticks_and_stops_at_ninety_nine_fifty_nine() {
        // §11: elapsed time is counted in ticks and converted for display, so
        // it cannot drift from the game. §12.4 caps it.
        assert_eq!(clock(0), "00:00");
        assert_eq!(clock(59), "00:00");
        assert_eq!(clock(60), "00:01");
        assert_eq!(clock((2 * 60 + 14) * 60), "02:14");
        assert_eq!(clock(u64::MAX), "99:59");
    }
}
