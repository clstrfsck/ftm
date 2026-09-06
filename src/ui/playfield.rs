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
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::core::{GameView, PieceKind, Rotation, VIEW_HEIGHT, VIEW_WIDTH};
use crate::ui::cells::{CELL_WIDTH, Paint, span};
use crate::ui::theme;
use crate::ui::{Banner, Chrome, Cosmetics, Debug, Hud, centred};

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
/// The §12.4 debug strip: three rows of figures plus its border.
const DEBUG_HEIGHT: u16 = 5;
/// Three columns of figures across the strip's 42-character interior, two
/// spaces apart: 12 + 2 + 12 + 2 + 14. The widest figure in each row is put in
/// the last column, which is the one with room for `fall_period`'s seven
/// digits and for the word `enhanced`.
const DEBUG_COLUMNS: [usize; 3] = [12, 12, 14];
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
    hud: &Hud,
) -> Rect {
    let debug = hud.debug;
    // §12.4: the debug strip sits directly beneath the block, so what is
    // centred is the two of them together. A terminal too short for the strip
    // is drawn without it and is otherwise unaffected — `show_debug` is a
    // developer's read-out, not a supported layout, and it does not move
    // §12.1's minimum size.
    let room = frame.area().height >= SCREEN_HEIGHT + DEBUG_HEIGHT;
    let strip = debug.filter(|_| room);
    let height = SCREEN_HEIGHT + if strip.is_some() { DEBUG_HEIGHT } else { 0 };
    let block = centred(frame.area(), SCREEN_WIDTH, height);
    let screen = Rect {
        height: SCREEN_HEIGHT.min(block.height),
        ..block
    };
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

    // §12.4's "+N" rule is for a layout that leaves too little room for every
    // slot. This is not one: at `preview_count` = 6 the box is 20 rows and the
    // screen is 23, so every slot always fits and the rule cannot arise.
    let next_height = 3 * view.next.len() as u16 + 2;
    panel(
        frame,
        at(NEXT_X, 0, COLUMN_WIDTH, next_height),
        next(view, chrome),
    );

    // §12.4: the playfield has no lid. Its walls and floor are drawn, and the
    // row the borders would have taken is left open, which is where a piece
    // comes in from. The field itself keeps the twenty rows it always had, at
    // the bottom of the box, so nothing below the mouth has moved.
    let block = box_border()
        .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
        .border_style(chrome.theme.plain());
    let outer = at(FIELD_X, 0, FIELD_WIDTH, FIELD_HEIGHT);
    let open = block.inner(outer);
    let interior = Rect {
        y: open.y + open.height - VIEW_HEIGHT as u16,
        height: VIEW_HEIGHT as u16,
        ..open
    };
    frame.render_widget(block, outer);
    frame.render_widget(Paragraph::new(field(view, chrome, fx, blanked)), interior);
    if let Some(banner) = fx.banner() {
        overlay_banner(frame, interior, chrome, banner);
    }

    frame.render_widget(
        Paragraph::new(status(view, fx, hud.restart)).style(chrome.theme.plain()),
        at(0, STATUS_Y, SCREEN_WIDTH, 1),
    );
    if let Some(strip) = strip {
        panel(
            frame,
            at(0, SCREEN_HEIGHT, SCREEN_WIDTH, DEBUG_HEIGHT),
            debug_lines(strip, view),
        );
    }
    screen
}

/// The §12.4 debug strip: nine figures in three columns of three.
///
/// Everything here is either the shell's own or comes from a `DebugView`
/// (§12.7). Gravity is printed from the core's integer milli-G rather than
/// computed here, so the strip cannot disagree with the rules about how fast
/// the piece is falling.
fn debug_lines(debug: &Debug, view: &GameView) -> Vec<Line<'static>> {
    // The bag empties as the queue is topped up, so "nothing left" is the
    // common answer at the default `preview_count` and is worth showing as
    // something rather than as a blank column.
    let bag: String = match debug
        .core
        .bag
        .iter()
        .map(|kind| kind.glyph())
        .collect::<String>()
    {
        empty if empty.is_empty() => "-".to_string(),
        bag => bag,
    };
    let rows = [
        [
            ("FPS", debug.fps.to_string()),
            ("TICKS", view.ticks.to_string()),
            ("DROPPED", debug.dropped.to_string()),
        ],
        [
            ("G", gravity(debug.core.milli_g)),
            ("LOCK", optional(debug.core.lock_delay)),
            ("PERIOD", debug.core.fall_period.to_string()),
        ],
        [
            ("DAS", format!("{}%", debug.das_charge)),
            ("BAG", bag),
            ("INPUT", debug.mode.name().to_string()),
        ],
    ];
    rows.iter()
        .map(|row| {
            Line::raw(
                row.iter()
                    .zip(DEBUG_COLUMNS)
                    .map(|((label, value), width)| {
                        let value_width = width - label.len();
                        format!("{label}{value:>value_width$}")
                    })
                    .collect::<Vec<_>>()
                    .join("  "),
            )
        })
        .collect()
}

/// Gravity in G, from the core's integer thousandths (§9.9).
fn gravity(milli_g: u32) -> String {
    format!("{}.{:03}", milli_g / 1000, milli_g % 1000)
}

/// A figure that is only sometimes there — the lock delay, while grounded.
fn optional(value: Option<u32>) -> String {
    value.map_or_else(|| "-".to_string(), |v| v.to_string())
}

/// The border every box on this screen is drawn with (§12.4).
///
/// Quadrant half blocks rather than the line-drawing set: a `│` is inked down
/// the middle of its cell, so the wall it draws sits half a cell away from the
/// field it encloses, and with a matrix cell two characters wide (§12.2) that
/// half is visible. `▐` puts the same wall against the interior instead, so the
/// boundary of the playfield falls exactly on the boundary of a cell.
fn box_border() -> Block<'static> {
    Block::bordered().border_type(BorderType::QuadrantInside)
}

/// A bordered box with `lines` inside it.
fn panel(frame: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
    let block = box_border();
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
    grid.map(|row| paint(chrome, &row, false))
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
    grid.iter()
        .map(|row| paint(chrome, row, chrome.show_grid))
        .collect()
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

/// One row of cells as a line.
///
/// `grid` is passed rather than read off the `Chrome`, because §6.3 puts the
/// dots "in the empty **playfield**": the hold and preview boxes are not the
/// field, and a dotted background behind a preview piece is noise.
fn paint(chrome: &Chrome, row: &[Paint], grid: bool) -> Line<'static> {
    Line::from(
        row.iter()
            .map(|cell| span(chrome.theme, *cell, grid))
            .collect::<Vec<_>>(),
    )
}

/// The status line (§12.4): the standing indicators, then the most recent
/// clear's name while it lasts, centred under the playfield.
///
/// The padding is computed here rather than left to the renderer's alignment,
/// so that "centred" means one thing and the mock-up can be compared against it
/// character for character.
fn status(view: &GameView, fx: &Cosmetics, restart: Option<u8>) -> String {
    // §10.1's restart is a *held* key, and a hold with no feedback is
    // indistinguishable from a key that did nothing. It takes the whole line
    // while it is down: the player is about to throw the game away, and the
    // combo counter is not what they are looking at.
    if let Some(percent) = restart {
        let filled = usize::from(percent).min(100) * RESTART_CELLS / 100;
        return centre_line(&format!(
            "RESTART {}{}",
            "\u{2588}".repeat(filled),
            "\u{2591}".repeat(RESTART_CELLS - filled),
        ));
    }
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
    centre_line(&parts.join("  "))
}

/// The bar the restart hold fills, in characters.
const RESTART_CELLS: usize = 10;

/// One centred line of the status row.
fn centre_line(text: &str) -> String {
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

    /// The plainest possible `Hud`: no overlay, no strip, no restart hold.
    ///
    /// The config is only ever read by the §13.5 Options panel, which is an
    /// overlay and not part of the playfield, so the default will do for every
    /// test in here.
    fn hud(debug: Option<&Debug>) -> Hud<'_> {
        static CONFIG: std::sync::OnceLock<crate::config::ConfigFile> = std::sync::OnceLock::new();
        Hud {
            overlay: &crate::ui::Overlay::None,
            config: CONFIG.get_or_init(crate::config::ConfigFile::default),
            debug,
            mode: crate::input::InputMode::Enhanced,
            restart: None,
        }
    }
    use crate::core::{PieceView, PlayState};
    use crate::ui::theme::{Depth, Theme};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use std::time::{Duration, Instant};

    /// The §12.4 mock-up, transcribed literally. It is drawn to exact size and
    /// is the acceptance criterion for this stage, so it is compared character
    /// for character rather than approximated.
    const MOCK_UP: &str = "\
▗▄▄▄▄▄▄▄▄▖ ▐                    ▌ ▗▄▄▄▄▄▄▄▄▖
▐ HOLD   ▌ ▐                    ▌ ▐ NEXT   ▌
▐  ██    ▌ ▐                    ▌ ▐  ████  ▌
▐██████  ▌ ▐                    ▌ ▐  ████  ▌
▝▀▀▀▀▀▀▀▀▘ ▐        ██          ▌ ▐        ▌
           ▐      ██████        ▌ ▐██      ▌
▗▄▄▄▄▄▄▄▄▖ ▐                    ▌ ▐██████  ▌
▐ SCORE  ▌ ▐                    ▌ ▐        ▌
▐  12480 ▌ ▐                    ▌ ▐    ██  ▌
▐        ▌ ▐                    ▌ ▐██████  ▌
▐ LEVEL  ▌ ▐                    ▌ ▐        ▌
▐      4 ▌ ▐                    ▌ ▐        ▌
▐        ▌ ▐                    ▌ ▐████████▌
▐ LINES  ▌ ▐                    ▌ ▐        ▌
▐     37 ▌ ▐                    ▌ ▐  ██    ▌
▐        ▌ ▐                    ▌ ▐██████  ▌
▐ TIME   ▌ ▐                    ▌ ▝▀▀▀▀▀▀▀▀▘
▐  02:14 ▌ ▐        ▒▒          ▌           
▝▀▀▀▀▀▀▀▀▘ ▐      ▒▒▒▒▒▒        ▌           
           ▐      ██████████    ▌           
           ▐██████████████████  ▌           
           ▝▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▘           
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
                render(frame, view, chrome, &fx, false, &hud(None));
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
    fn the_playfield_is_open_at_the_top() {
        // §12.4: walls and a floor, no lid. The mouth takes the row the top
        // border used to, so the twenty drawn rows and the floor are exactly
        // where a closed box left them -- which is the half of this worth
        // pinning, since a box that lost its lid *and* slid down a row would
        // still look open.
        let drawn = screenshot(&empty_view(), &chrome());
        let rows: Vec<&str> = drawn.lines().collect();
        let field = |row: &str| {
            row.chars()
                .skip(FIELD_X as usize)
                .take(FIELD_WIDTH as usize)
                .collect::<String>()
        };
        assert_eq!(
            field(rows[0]),
            format!("▐{}▌", " ".repeat(FIELD_WIDTH as usize - 2)),
            "the mouth: walls, and nothing across them",
        );
        assert_eq!(
            field(rows[(FIELD_HEIGHT - 1) as usize]),
            format!("▝{}▘", "▀".repeat(FIELD_WIDTH as usize - 2)),
            "the floor, still on the last row of the box",
        );
        for (i, row) in rows[1..(FIELD_HEIGHT - 1) as usize].iter().enumerate() {
            let drawn = field(row);
            assert!(
                drawn.starts_with('▐') && drawn.ends_with('▌'),
                "row {} has no walls: {drawn:?}",
                i + 1,
            );
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
                .position(|line| line.chars().nth(NEXT_X as usize) == Some('▝'))
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
        assert!(rows[1].starts_with("▐ SCORE  ▌"), "{:?}", rows[1]);
        assert!(rows[12].starts_with("▝▀▀▀▀▀▀▀▀▘"), "{:?}", rows[12]);
        assert!(rows[2].starts_with("▐  12480 ▌"));
    }

    /// The foreground colour of one character cell of the rendered screen.
    fn colour_at(view: &GameView, chrome: &Chrome, x: u16, y: u16) -> Color {
        let backend = TestBackend::new(SCREEN_WIDTH, SCREEN_HEIGHT);
        let mut terminal = Terminal::new(backend).expect("a test terminal");
        let fx = Cosmetics::new(Duration::from_millis(250), Instant::now());
        terminal
            .draw(|frame| {
                render(frame, view, chrome, &fx, false, &hud(None));
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
        // §12.3's purple, which is §9.2's lifted: the hold box draws a
        // piece exactly as the field does.
        const PURPLE: (u32, u32, u32) = (0xD5, 0x8F, 0xF8);
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
        // §6.3 puts the dots "in the empty playfield": the hold and preview
        // boxes are not the field, and a dotted backing behind a preview piece
        // is noise.
        let slot = drawn.lines().nth(2).expect("the hold box's first cell row");
        assert!(
            !slot[..COLUMN_WIDTH as usize].contains('\u{b7}'),
            "the grid stopped at the field: {slot:?}",
        );
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
                render(frame, &view, &chrome, &fx, true, &hud(None));
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
    /// Render at an arbitrary size with the debug strip on, and read it back.
    fn screenshot_with_debug(view: &GameView, chrome: &Chrome, rows: u16) -> Vec<String> {
        let backend = TestBackend::new(SCREEN_WIDTH, rows);
        let mut terminal = Terminal::new(backend).expect("a test terminal");
        let fx = Cosmetics::new(Duration::from_millis(250), Instant::now());
        let debug = Debug {
            fps: 60,
            dropped: 3,
            das_charge: 100,
            mode: crate::input::InputMode::Enhanced,
            core: crate::core::DebugView {
                milli_g: 16,
                fall_period: 3_932_160,
                lock_delay: Some(27),
                bag: vec![PieceKind::T, PieceKind::S, PieceKind::Z],
            },
        };
        terminal
            .draw(|frame| {
                render(frame, view, chrome, &fx, false, &hud(Some(&debug)));
            })
            .expect("a frame");
        let buffer = terminal.backend().buffer().clone();
        (0..rows)
            .map(|y| {
                (0..SCREEN_WIDTH)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn the_debug_strip_sits_beneath_the_block() {
        // §12.4 as amended: 44 x 5 directly under the 23-row block, with nine
        // figures in three columns of three.
        let lines = screenshot_with_debug(&mock_up_view(), &chrome(), SCREEN_HEIGHT + DEBUG_HEIGHT);
        assert_eq!(
            lines[..SCREEN_HEIGHT as usize].join("\n"),
            MOCK_UP,
            "the block itself is untouched by the strip below it",
        );
        let strip = &lines[SCREEN_HEIGHT as usize..];
        assert_eq!(strip.len(), DEBUG_HEIGHT as usize);
        assert_eq!(
            strip.join("\n"),
            "\
▗▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▖
▐FPS       60  TICKS   8040  DROPPED      3▌
▐G      0.016  LOCK      27  PERIOD 3932160▌
▐DAS     100%  BAG      TSZ  INPUT enhanced▌
▝▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▘",
        );
        for row in strip {
            assert_eq!(row.chars().count(), SCREEN_WIDTH as usize);
        }
    }

    #[test]
    fn a_terminal_too_short_for_the_strip_is_drawn_without_it() {
        // §12.4: `show_debug` is a developer's read-out and does not move
        // §12.1's minimum size, so the game is unaffected by having no room.
        let lines = screenshot_with_debug(&mock_up_view(), &chrome(), SCREEN_HEIGHT);
        assert_eq!(lines.join("\n"), MOCK_UP);
    }

    #[test]
    fn gravity_is_printed_from_the_cores_own_integer() {
        // §9.9: no floating point in the rules, and none introduced here — the
        // strip cannot disagree with the core about how fast a piece falls.
        assert_eq!(gravity(16), "0.016");
        assert_eq!(gravity(1_250), "1.250");
        assert_eq!(gravity(0), "0.000");
        assert_eq!(optional(None), "-");
        assert_eq!(optional(Some(30)), "30");
    }
    #[test]
    fn a_configured_glyph_leaves_the_field_a_rectangle() {
        // Stage 10's exit criterion, and the reason §12.2's width rule is the
        // loader's: every cell is two columns, so the field's every row is the
        // same width whatever the glyphs are.
        let chrome = Chrome {
            theme: Theme::with_glyphs(
                Depth::Truecolor,
                crate::ui::theme::Glyphs::configured(&crate::config::DisplaySettings {
                    cell_filled: "[]".to_string(),
                    cell_empty: "--".to_string(),
                    cell_ghost: "<>".to_string(),
                    ..crate::config::DisplaySettings::default()
                }),
            ),
            ..chrome()
        };
        let drawn = screenshot(&mock_up_view(), &chrome);
        for line in drawn.lines() {
            assert_eq!(line.chars().count(), SCREEN_WIDTH as usize, "{line:?}");
        }
        let field: Vec<String> = drawn
            .lines()
            .skip(1)
            .take(VIEW_HEIGHT)
            .map(|line| line.chars().skip(12).take(20).collect())
            .collect();
        assert!(field.iter().all(|row| row.chars().count() == 20));
        assert!(field.iter().any(|row| row.contains("[]")), "{field:#?}");
        assert!(field.iter().any(|row| row.contains("<>")), "{field:#?}");
        assert!(field.iter().any(|row| row.contains("--")), "{field:#?}");
    }
}
