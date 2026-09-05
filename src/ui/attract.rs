//! The attract screen (§13).
//!
//! §13 is explicitly provisional: build it plainly, look at it, then iterate.
//!
//! Like the rest of `ui` this draws and nothing else. [`Attract`] is the whole
//! of its state, it is advanced by [`Attract::step`] against a clock the caller
//! owns, and the only thing it can change is the config the Options sub-screen
//! edits (§13.5). It has no path to a `Game`, because on this screen there
//! isn't one.

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::config::ConfigFile;
use crate::core::geometry::Rotation;
use crate::core::piece::PieceKind;
use crate::highscore::{Entry, Table};
use crate::input::InputMode;
use crate::ui::cells::CELL_WIDTH;
use crate::ui::overlays::{Setting, box_over, centre};
use crate::ui::theme::{self, Theme};
use crate::ui::{Chrome, centred};

// ---------------------------------------------------------------------------
// §13.2 the wordmark
// ---------------------------------------------------------------------------

/// The wordmark is five rows tall (§13.2).
const WORDMARK_ROWS: usize = 5;

/// The seven letters of `TERMINO`, drawn from single-width block characters so
/// the whole thing fits in 60 columns (§13.2).
///
/// An **original** block-letter wordmark: the official logo must not be used,
/// reproduced or approximated, and no official colours-as-branding, styling or
/// artwork may be copied (§1.3).
const WORDMARK: [[&str; WORDMARK_ROWS]; 7] = [
    [
        "\u{2588}\u{2588}\u{2588}\u{2588}",
        " \u{2588}\u{2588} ",
        " \u{2588}\u{2588} ",
        " \u{2588}\u{2588} ",
        " \u{2588}\u{2588} ",
    ],
    [
        "\u{2588}\u{2588}\u{2588}\u{2588}",
        "\u{2588}   ",
        "\u{2588}\u{2588}\u{2588} ",
        "\u{2588}   ",
        "\u{2588}\u{2588}\u{2588}\u{2588}",
    ],
    [
        "\u{2588}\u{2588}\u{2588} ",
        "\u{2588}  \u{2588}",
        "\u{2588}\u{2588}\u{2588} ",
        "\u{2588} \u{2588} ",
        "\u{2588}  \u{2588}",
    ],
    [
        "\u{2588}   \u{2588}",
        "\u{2588}\u{2588} \u{2588}\u{2588}",
        "\u{2588} \u{2588} \u{2588}",
        "\u{2588}   \u{2588}",
        "\u{2588}   \u{2588}",
    ],
    [
        "\u{2588}\u{2588}\u{2588}\u{2588}",
        " \u{2588}\u{2588} ",
        " \u{2588}\u{2588} ",
        " \u{2588}\u{2588} ",
        "\u{2588}\u{2588}\u{2588}\u{2588}",
    ],
    [
        "\u{2588}   \u{2588}",
        "\u{2588}\u{2588}  \u{2588}",
        "\u{2588} \u{2588} \u{2588}",
        "\u{2588}  \u{2588}\u{2588}",
        "\u{2588}   \u{2588}",
    ],
    [
        "\u{2588}\u{2588}\u{2588}\u{2588}",
        "\u{2588}  \u{2588}",
        "\u{2588}  \u{2588}",
        "\u{2588}  \u{2588}",
        "\u{2588}\u{2588}\u{2588}\u{2588}",
    ],
];

/// One tetromino colour per letter, left to right (§13.2).
const WORDMARK_COLOURS: [PieceKind; 7] = [
    PieceKind::I,
    PieceKind::J,
    PieceKind::L,
    PieceKind::O,
    PieceKind::S,
    PieceKind::T,
    PieceKind::Z,
];

/// The block the screen is laid out in: as wide as the wordmark (§13.3).
const BLOCK_WIDTH: usize = 36;
/// Wordmark, gap, menu, gap, panel, gap, footer (§13.3).
const BLOCK_HEIGHT: u16 = 20;
/// How far the menu is indented inside the block (§13.3).
const MENU_X: usize = 9;
/// The panel's interior: two entry columns and a margin either side.
const PANEL_WIDTH: usize = BLOCK_WIDTH - 2;
/// Rows inside the panel box.
///
/// §13.3 draws three, which is one short of the seven control entries every
/// setting turned on produces. Four rows is the smallest grid that can show
/// them all, and it is what the high-score face wants anyway: a heading and
/// the top three.
const PANEL_ROWS: usize = 4;
/// One control entry: a right-aligned key and its label.
const ENTRY_WIDTH: usize = 16;

// ---------------------------------------------------------------------------
// §13.4 the background animation
// ---------------------------------------------------------------------------

/// A new drifting piece roughly this often (§13.4).
const SPAWN: Duration = Duration::from_millis(1_200);
/// At most this many exist at once (§13.4).
const DRIFTERS: usize = 12;
/// A drifting piece falls one row about this often (§13.4), jittered per piece
/// so they do not march in lockstep.
const FALL: Duration = Duration::from_millis(600);
const FALL_JITTER: Duration = Duration::from_millis(150);
/// Heavily dimmed (§13.4) — below even the ghost's 45 %, because this is meant
/// to be noticed only when nothing else is happening.
const DRIFT_BRIGHTNESS: u8 = 28;
/// The outline glyph of §13.4.
const DRIFT_GLYPH: &str = "\u{2591}\u{2591}";

/// The panel cycles every six seconds (§13.3).
const FACE: Duration = Duration::from_secs(6);
/// §13.6: the wordmark's colours start cycling after a minute of no keys...
const IDLE: Duration = Duration::from_secs(60);
/// ...one step per second.
const IDLE_STEP: Duration = Duration::from_secs(1);

/// The one-line rules reminders the third panel face rotates through (§13.3).
const REMINDERS: [&str; 5] = [
    "Clear 4 rows at once for a QUAD",
    "Back-to-back QUADs score 1.5x",
    "A T-spin double outscores a QUAD",
    "Hold parks a piece for later",
    "Every soft-dropped row is a point",
];

// ---------------------------------------------------------------------------
// state
// ---------------------------------------------------------------------------

/// §13.3's menu, in the order it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuChoice {
    Play,
    HighScores,
    Controls,
    Options,
    Quit,
}

impl MenuChoice {
    pub const ALL: [MenuChoice; 5] = [
        MenuChoice::Play,
        MenuChoice::HighScores,
        MenuChoice::Controls,
        MenuChoice::Options,
        MenuChoice::Quit,
    ];

    const fn label(self) -> &'static str {
        match self {
            MenuChoice::Play => "PLAY",
            MenuChoice::HighScores => "HIGH SCORES",
            MenuChoice::Controls => "CONTROLS",
            MenuChoice::Options => "OPTIONS",
            MenuChoice::Quit => "QUIT",
        }
    }
}

/// A sub-screen over the attract screen (§13.5). `Esc` returns from each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sub {
    HighScores,
    Controls,
    Options { selected: usize },
}

/// What the attract screen asks the caller to do next (§7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing to do; the screen may or may not have changed.
    Stay,
    /// **PLAY**: start a fresh game.
    Play,
    /// **QUIT**, or the quit key.
    Quit,
    /// The Options panel was left: §13.5 asks for the config to be saved and
    /// the presentation half applied at once.
    OptionsClosed,
}

/// The attract screen's whole state (§13).
pub struct Attract {
    now: Instant,
    selected: usize,
    sub: Option<Sub>,
    /// §13.6: when a key was last pressed.
    last_key: Instant,
    /// §13.3: when the panel's face last changed. Held at `now` — so the
    /// elapsed time stays zero — while the cycle is paused.
    face_since: Instant,
    /// Counts faces shown, not the face on show: the third face's reminder is
    /// `face / FACES` so the tips rotate without a second counter.
    face: usize,
    background: Background,
}

impl Attract {
    pub fn new(now: Instant) -> Self {
        Self {
            now,
            selected: 0,
            sub: None,
            last_key: now,
            face_since: now,
            face: 0,
            background: Background::new(now),
        }
    }

    /// Advance the clock, reporting whether anything on screen moved.
    ///
    /// §15.3 redraws only when the background steps or the selection changes,
    /// and this is the "background steps" half; a key press is the other.
    ///
    /// `cells` is the frame in matrix cells, which is what the drifting pieces
    /// are positioned in. `animate` is §13.4's two exclusions — `mono` and
    /// `show_debug` — resolved by the caller.
    pub fn step(&mut self, now: Instant, cells: (u16, u16), animate: bool) -> bool {
        let was = (self.face, self.idle_shift());
        self.now = now;
        // §13.3: the cycle pauses while a menu item other than PLAY is
        // selected. Holding the mark at `now` keeps the elapsed time at zero,
        // so the face that is up stays up rather than jumping when it resumes.
        if self.selected == 0 && self.sub.is_none() {
            while now.saturating_duration_since(self.face_since) >= FACE {
                self.face_since += FACE;
                self.face += 1;
            }
        } else {
            self.face_since = now;
        }
        let drifted = animate && self.background.step(now, cells);
        drifted || was != (self.face, self.idle_shift())
    }

    /// Fold in one key (§10.1: `↑`/`↓`, `Enter`/`Space`, `Esc`, always).
    ///
    /// `config` is borrowed because the Options sub-screen edits it in place
    /// (§13.5); nothing else here touches it.
    pub fn key(&mut self, event: &KeyEvent, config: &mut ConfigFile, now: Instant) -> Outcome {
        if event.kind == KeyEventKind::Release {
            return Outcome::Stay;
        }
        // §13.6: any key stops the idle colour cycle.
        self.last_key = now;
        match self.sub {
            Some(Sub::Options { selected }) => self.options_key(event, config, selected),
            Some(_) => {
                if matches!(
                    event.code,
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ')
                ) {
                    self.sub = None;
                }
                Outcome::Stay
            }
            None => self.menu_key(event),
        }
    }

    fn menu_key(&mut self, event: &KeyEvent) -> Outcome {
        let items = MenuChoice::ALL.len();
        match event.code {
            KeyCode::Up => self.selected = (self.selected + items - 1) % items,
            KeyCode::Down => self.selected = (self.selected + 1) % items,
            KeyCode::Enter | KeyCode::Char(' ') => match MenuChoice::ALL[self.selected] {
                MenuChoice::Play => return Outcome::Play,
                MenuChoice::HighScores => self.sub = Some(Sub::HighScores),
                MenuChoice::Controls => self.sub = Some(Sub::Controls),
                MenuChoice::Options => self.sub = Some(Sub::Options { selected: 0 }),
                MenuChoice::Quit => return Outcome::Quit,
            },
            // §16: Ctrl-C is delivered as a key event and means quit from the
            // attract screen. §10.1's `q` is the ordinary way.
            KeyCode::Char('c')
                if event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                return Outcome::Quit;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return Outcome::Quit,
            _ => {}
        }
        Outcome::Stay
    }

    /// §13.5, the same panel the pause menu opens: `↑`/`↓` choose, `←`/`→`
    /// change, `Esc` saves and returns.
    fn options_key(
        &mut self,
        event: &KeyEvent,
        config: &mut ConfigFile,
        selected: usize,
    ) -> Outcome {
        let items = Setting::ALL.len();
        match event.code {
            KeyCode::Up => {
                self.sub = Some(Sub::Options {
                    selected: (selected + items - 1) % items,
                })
            }
            KeyCode::Down => {
                self.sub = Some(Sub::Options {
                    selected: (selected + 1) % items,
                })
            }
            KeyCode::Left | KeyCode::Right => {
                Setting::ALL[selected].step(config, event.code == KeyCode::Right);
            }
            KeyCode::Esc | KeyCode::Enter => {
                self.sub = None;
                return Outcome::OptionsClosed;
            }
            _ => {}
        }
        Outcome::Stay
    }

    /// §13.6: how many steps the wordmark's colours have rotated.
    fn idle_shift(&self) -> usize {
        let idle = self.now.saturating_duration_since(self.last_key);
        let Some(cycling) = idle.checked_sub(IDLE) else {
            return 0;
        };
        (cycling.as_nanos() / IDLE_STEP.as_nanos()) as usize
    }
}

/// One drifting tetromino outline (§13.4).
///
/// Positions are in matrix cells, and `row` starts negative so a piece drifts
/// in from above rather than appearing whole.
struct Drifter {
    kind: PieceKind,
    rotation: Rotation,
    col: i16,
    row: i16,
    period: Duration,
    since: Instant,
}

/// §13.4's ambient animation.
struct Background {
    pieces: Vec<Drifter>,
    rng: SmallRng,
    spawned: Instant,
}

impl Background {
    fn new(now: Instant) -> Self {
        Self {
            pieces: Vec::new(),
            // Cosmetic, so it is seeded from the environment: nothing here is
            // ever replayed, and §15.4's obligations are the core's alone.
            rng: SmallRng::from_entropy(),
            spawned: now,
        }
    }

    /// Spawn, fall and retire, reporting whether anything moved.
    fn step(&mut self, now: Instant, (columns, rows): (u16, u16)) -> bool {
        let mut moved = false;
        if now.saturating_duration_since(self.spawned) >= SPAWN {
            self.spawned = now;
            if self.pieces.len() < DRIFTERS && columns > 4 {
                let jitter = self.rng.gen_range(0..=FALL_JITTER.as_millis() as u64 * 2);
                self.pieces.push(Drifter {
                    kind: PieceKind::ALL[self.rng.gen_range(0..PieceKind::ALL.len())],
                    rotation: Rotation::from_index(self.rng.gen_range(0..4)),
                    col: self
                        .rng
                        .gen_range(0..i16::try_from(columns - 3).unwrap_or(1)),
                    row: -4,
                    period: FALL - FALL_JITTER + Duration::from_millis(jitter),
                    since: now,
                });
                moved = true;
            }
        }
        for piece in &mut self.pieces {
            while now.saturating_duration_since(piece.since) >= piece.period {
                piece.since += piece.period;
                piece.row += 1;
                moved = true;
            }
        }
        let floor = i16::try_from(rows).unwrap_or(i16::MAX);
        self.pieces.retain(|piece| piece.row <= floor);
        moved
    }
}

// ---------------------------------------------------------------------------
// drawing
// ---------------------------------------------------------------------------

/// What the attract screen needs that is not its own state.
///
/// `hold_enabled` and `allow_180_rotation` are read from `config` rather than
/// from `Chrome`: there is no game running, so the config *is* the answer, and
/// §13.3 hides the bindings those two settings turn off.
pub struct Context<'a> {
    pub chrome: &'a Chrome,
    pub config: &'a ConfigFile,
    pub scores: &'a Table,
    /// The entry the run that just finished added, highlighted in the
    /// high-score sub-screen (§13.5).
    pub recent: Option<usize>,
    /// Which of §8.2's two paths is live, for the controls sub-screen.
    pub mode: InputMode,
}

/// Draw the attract screen (§13.3), with a sub-screen over it if one is open.
pub fn draw(frame: &mut Frame, state: &Attract, cx: &Context) {
    let theme = cx.chrome.theme;
    // §13.4: the animation is disabled in `mono` and when `show_debug` is on.
    if animated(cx) {
        drift(frame, &state.background, theme);
    }

    let area = frame.area();
    let block = centred(area, BLOCK_WIDTH as u16, BLOCK_HEIGHT);
    let mut lines = Vec::with_capacity(BLOCK_HEIGHT as usize);
    let shift = state.idle_shift();
    for row in 0..WORDMARK_ROWS {
        lines.push(wordmark_row(row, shift, theme));
    }
    lines.push(pad(String::new()));
    for (index, choice) in MenuChoice::ALL.iter().enumerate() {
        let selected = index == state.selected && state.sub.is_none();
        let marker = if selected { "\u{25b8} " } else { "  " };
        let style = if selected {
            // §13.3: the selected item is drawn in the I-piece cyan.
            theme.piece(PieceKind::I, theme::FULL).patch(theme.bold())
        } else {
            theme.plain()
        };
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(MENU_X)),
            Span::styled(
                format!(
                    "{marker}{:<width$}",
                    choice.label(),
                    width = BLOCK_WIDTH - MENU_X - 2
                ),
                style,
            ),
        ]));
    }
    // The panel is a bordered box, so it is drawn as a widget of its own; the
    // rows it occupies are left out of the paragraph.
    for _ in 0..PANEL_ROWS + 2 {
        lines.push(pad(String::new()));
    }
    lines.push(pad(String::new()));
    lines.push(Line::styled(
        centre(
            concat!(
                "v",
                env!("CARGO_PKG_VERSION"),
                "   \u{2191}\u{2193} select   ENTER start"
            ),
            BLOCK_WIDTH,
        ),
        theme.faint(),
    ));
    frame.render_widget(Paragraph::new(Text::from(lines)), block);

    let panel = Rect {
        x: block.x,
        y: block.y + WORDMARK_ROWS as u16 + 1 + MenuChoice::ALL.len() as u16 + 1,
        width: BLOCK_WIDTH as u16,
        height: PANEL_ROWS as u16 + 2,
    };
    if panel.bottom() <= block.bottom() {
        face(frame, panel, state, cx);
    }

    match state.sub {
        None => {}
        Some(Sub::HighScores) => high_scores(frame, area, cx),
        Some(Sub::Controls) => controls(frame, area, cx),
        Some(Sub::Options { selected }) => {
            crate::ui::overlays::options(frame, area, cx.chrome, cx.config, selected)
        }
    }
}

/// §13.4's two exclusions.
fn animated(cx: &Context) -> bool {
    cx.chrome.theme.depth() != theme::Depth::Mono && !cx.config.display.show_debug
}

/// The drifting pieces, behind everything else (§13.4).
///
/// Written straight into the buffer rather than through a widget: a piece is
/// four cells that may be anywhere on the screen, and every one of them is
/// about to be painted over by whatever the block draws on top.
fn drift(frame: &mut Frame, background: &Background, theme: Theme) {
    let area = frame.area();
    for piece in &background.pieces {
        let style = theme.piece(piece.kind, DRIFT_BRIGHTNESS);
        for mino in piece.kind.cells(piece.rotation) {
            let (Ok(col), Ok(row)) = (
                u16::try_from(piece.col + mino.x as i16),
                u16::try_from(piece.row + mino.y as i16),
            ) else {
                continue;
            };
            let x = area.x + col * CELL_WIDTH;
            let y = area.y + row;
            if x + CELL_WIDTH > area.right() || y >= area.bottom() {
                continue;
            }
            frame.buffer_mut().set_string(x, y, DRIFT_GLYPH, style);
        }
    }
}

/// One row of the wordmark, a span per letter (§13.2).
fn wordmark_row(row: usize, shift: usize, theme: Theme) -> Line<'static> {
    let mut spans = Vec::with_capacity(WORDMARK.len() * 2);
    for (index, letter) in WORDMARK.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(" "));
        }
        let kind = WORDMARK_COLOURS[(index + shift) % WORDMARK_COLOURS.len()];
        spans.push(Span::styled(letter[row], theme.piece(kind, theme::FULL)));
    }
    Line::from(spans)
}

/// The cycling panel beneath the menu (§13.3).
fn face(frame: &mut Frame, area: Rect, state: &Attract, cx: &Context) {
    let theme = cx.chrome.theme;
    let mut rows = match state.face % 3 {
        0 => control_summary(cx),
        1 => top_three(cx),
        _ => vec![centre(
            REMINDERS[state.face / 3 % REMINDERS.len()],
            PANEL_WIDTH,
        )],
    };
    // Vertically centred, so a face with fewer rows than the grid does not sit
    // against the top border.
    let top = (PANEL_ROWS - rows.len().min(PANEL_ROWS)) / 2;
    for _ in 0..top {
        rows.insert(0, " ".repeat(PANEL_WIDTH));
    }
    rows.resize(PANEL_ROWS, " ".repeat(PANEL_WIDTH));

    let block = Block::bordered().border_style(theme.faint());
    let interior = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(Text::from(
            rows.into_iter()
                .map(|row| Line::styled(row, theme.plain()))
                .collect::<Vec<_>>(),
        )),
        interior,
    );
}

/// The quick control summary (§13.3), reflowed around the bindings that are
/// actually available (§13.3, A9).
fn control_summary(cx: &Context) -> Vec<String> {
    let mut entries: Vec<(&str, &str)> = vec![
        ("\u{2190}\u{2192}", "move"),
        ("\u{2191}", "rotate"),
        ("\u{2193}", "soft drop"),
        ("SPACE", "drop"),
        ("Z", "rotate ccw"),
    ];
    if cx.config.gameplay.allow_180_rotation {
        entries.push(("A", "rotate 180"));
    }
    if cx.config.gameplay.hold_enabled {
        entries.push(("C", "hold"));
    }
    entries
        .chunks(2)
        .map(|pair| {
            let cell = |(key, label): &(&str, &str)| {
                format!("{key:>5} {label:<width$}", width = ENTRY_WIDTH - 6)
            };
            let left = cell(&pair[0]);
            let right = pair.get(1).map_or_else(|| " ".repeat(ENTRY_WIDTH), cell);
            format!(" {left}{right} ")
        })
        .collect()
}

/// The top three, as the panel shows them (§13.3).
fn top_three(cx: &Context) -> Vec<String> {
    let mut rows = vec![centre("HIGH SCORES", PANEL_WIDTH)];
    if cx.scores.top(3).is_empty() {
        rows.push(centre("no scores yet", PANEL_WIDTH));
        return rows;
    }
    for (index, entry) in cx.scores.top(3).iter().enumerate() {
        rows.push(format!(
            "  {}. {:<12}{:>15}  ",
            index + 1,
            entry.name,
            entry.score,
        ));
    }
    rows
}

/// The interior width of the high-score sub-screen (§13.5): rank, name,
/// score, level, lines and date, with a margin either side.
const SCORES_WIDTH: usize = 54;
/// The interior width of the controls sub-screen (§13.5).
const CONTROLS_WIDTH: usize = 42;

/// The full top-ten table (§13.5), with the most recent entry highlighted.
fn high_scores(frame: &mut Frame, over: Rect, cx: &Context) {
    let theme = cx.chrome.theme;
    let blank = Line::raw(" ".repeat(SCORES_WIDTH));
    let mut lines = vec![
        Line::styled(centre("HIGH SCORES", SCORES_WIDTH), theme.bold()),
        blank.clone(),
        Line::styled(
            score_row("", "NAME", "SCORE", "LV", "LINES", "DATE"),
            theme.faint(),
        ),
    ];
    if cx.scores.entries.is_empty() {
        lines.push(blank.clone());
        lines.push(Line::raw(centre("no scores yet", SCORES_WIDTH)));
    }
    for (index, entry) in cx.scores.entries.iter().enumerate() {
        let style = if cx.recent == Some(index) {
            theme.piece(PieceKind::I, theme::FULL).patch(theme.bold())
        } else {
            theme.plain()
        };
        lines.push(Line::styled(entry_row(index + 1, entry), style));
    }
    lines.push(blank);
    lines.push(Line::styled(
        centre("Esc returns", SCORES_WIDTH),
        theme.faint(),
    ));
    box_over(frame, over, SCORES_WIDTH, lines);
}

/// One row of the high-score table (§13.5): rank, name, score, level, lines,
/// date.
fn score_row(rank: &str, name: &str, score: &str, level: &str, lines: &str, date: &str) -> String {
    format!("  {rank:>2}  {name:<12}{score:>11}{level:>4}{lines:>7}  {date:<10}  ")
}

fn entry_row(rank: usize, entry: &Entry) -> String {
    score_row(
        &rank.to_string(),
        &entry.name,
        &entry.score.to_string(),
        &entry.level.to_string(),
        &entry.lines.to_string(),
        &entry.date,
    )
}

/// Every action of §10.1 and the key names bound to it, plus §8.2's live path
/// (§13.5).
fn controls(frame: &mut Frame, over: Rect, cx: &Context) {
    let theme = cx.chrome.theme;
    /// The eleven actions of §10.1, by the `[keys]` name that carries them.
    const ACTIONS: [(&str, &str); 11] = [
        ("move_left", "Move left"),
        ("move_right", "Move right"),
        ("soft_drop", "Soft drop"),
        ("hard_drop", "Hard drop"),
        ("rotate_cw", "Rotate clockwise"),
        ("rotate_ccw", "Rotate counter-clockwise"),
        ("rotate_180", "Rotate 180\u{b0}"),
        ("hold", "Hold"),
        ("pause", "Pause"),
        ("restart", "Restart (hold 1 s)"),
        ("quit", "Quit to menu"),
    ];
    let bound = cx.config.keys.each();
    let mut lines = vec![
        Line::styled(centre("CONTROLS", CONTROLS_WIDTH), theme.bold()),
        Line::raw(" ".repeat(CONTROLS_WIDTH)),
    ];
    for (name, label) in ACTIONS {
        // §13.3, A9: a binding whose setting is off is not shown at all.
        let gated = match name {
            "rotate_180" => cx.config.gameplay.allow_180_rotation,
            "hold" => cx.config.gameplay.hold_enabled,
            _ => true,
        };
        if !gated {
            continue;
        }
        let keys = bound
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, names)| names.join(", "))
            .unwrap_or_default();
        lines.push(Line::raw(format!("  {label:<24}{keys:>14}  ")));
    }
    lines.push(Line::raw(" ".repeat(CONTROLS_WIDTH)));
    lines.push(Line::styled(
        centre(
            &format!("input: {}   Esc returns", cx.mode.name()),
            CONTROLS_WIDTH,
        ),
        theme.faint(),
    ));
    box_over(frame, over, CONTROLS_WIDTH, lines);
}

/// A block-width line, so the animation behind it is covered (§13.4).
fn pad(text: String) -> Line<'static> {
    Line::raw(format!("{text:<BLOCK_WIDTH$}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highscore::Entry;
    use crate::ui::theme::{Depth, Glyphs};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Style;

    /// §13.2, transcribed literally.
    const ART: &str = "\
████ ████ ███  █   █ ████ █   █ ████
 ██  █    █  █ ██ ██  ██  ██  █ █  █
 ██  ███  ███  █ █ █  ██  █ █ █ █  █
 ██  █    █ █  █   █  ██  █  ██ █  █
 ██  ████ █  █ █   █ ████ █   █ ████";

    fn chrome() -> Chrome {
        Chrome {
            theme: Theme::with_glyphs(Depth::Truecolor, Glyphs::DEFAULT),
            show_grid: false,
            hold_enabled: true,
        }
    }

    fn plain(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn context<'a>(chrome: &'a Chrome, config: &'a ConfigFile, scores: &'a Table) -> Context<'a> {
        Context {
            chrome,
            config,
            scores,
            recent: None,
            mode: InputMode::Enhanced,
        }
    }

    #[test]
    fn the_wordmark_matches_the_spec_art() {
        // §13.2: 36 characters wide and 5 rows tall, and the letters are the
        // ones the specification draws.
        let theme = chrome().theme;
        let drawn: Vec<String> = (0..WORDMARK_ROWS)
            .map(|row| plain(&wordmark_row(row, 0, theme)))
            .collect();
        assert_eq!(drawn.join("\n"), ART);
        for row in &drawn {
            assert_eq!(row.chars().count(), BLOCK_WIDTH, "{row}");
        }
    }

    #[test]
    fn each_letter_takes_its_own_piece_colour() {
        // §13.2: one of the seven tetromino colours per letter, left to right
        // in the order I, J, L, O, S, T, Z.
        let theme = chrome().theme;
        let colours: Vec<Style> = wordmark_row(0, 0, theme)
            .spans
            .iter()
            .filter(|span| span.content.trim() != "")
            .map(|span| span.style)
            .collect();
        assert_eq!(colours.len(), 7);
        for (span, kind) in colours.iter().zip(WORDMARK_COLOURS) {
            assert_eq!(*span, theme.piece(kind, theme::FULL));
        }
    }

    #[test]
    fn the_idle_cycle_starts_after_a_minute_and_steps_once_a_second() {
        // §13.6.
        let start = Instant::now();
        let mut state = Attract::new(start);
        let cells = (30, 24);
        state.step(start + IDLE - Duration::from_millis(1), cells, false);
        assert_eq!(state.idle_shift(), 0, "not yet");
        state.step(start + IDLE, cells, false);
        assert_eq!(state.idle_shift(), 0, "the first step is a second later");
        state.step(start + IDLE + IDLE_STEP, cells, false);
        assert_eq!(state.idle_shift(), 1);
        state.step(start + IDLE + IDLE_STEP * 7, cells, false);
        assert_eq!(state.idle_shift(), 7, "and it does not stop");
        // The colours are read modulo seven, so a shift of seven is the start.
        assert_eq!(
            WORDMARK_COLOURS[7 % WORDMARK_COLOURS.len()],
            WORDMARK_COLOURS[0]
        );
    }

    #[test]
    fn any_key_stops_the_idle_cycle() {
        let start = Instant::now();
        let mut state = Attract::new(start);
        let later = start + IDLE + IDLE_STEP * 3;
        state.step(later, (30, 24), false);
        assert_eq!(state.idle_shift(), 3);
        state.key(
            &press(KeyCode::Char('x')),
            &mut ConfigFile::default(),
            later,
        );
        assert_eq!(state.idle_shift(), 0);
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, crossterm::event::KeyModifiers::NONE)
    }

    #[test]
    fn the_menu_wraps_and_play_is_the_first_item() {
        let now = Instant::now();
        let mut state = Attract::new(now);
        let mut config = ConfigFile::default();
        assert_eq!(MenuChoice::ALL[0], MenuChoice::Play);
        assert_eq!(
            state.key(&press(KeyCode::Enter), &mut config, now),
            Outcome::Play
        );

        state.key(&press(KeyCode::Up), &mut config, now);
        assert_eq!(
            state.selected,
            MenuChoice::ALL.len() - 1,
            "up wraps to QUIT"
        );
        assert_eq!(
            state.key(&press(KeyCode::Enter), &mut config, now),
            Outcome::Quit
        );
        state.key(&press(KeyCode::Down), &mut config, now);
        assert_eq!(state.selected, 0, "and down wraps back to PLAY");
    }

    #[test]
    fn every_sub_screen_opens_and_esc_returns() {
        // §13.5.
        let now = Instant::now();
        let mut config = ConfigFile::default();
        for (steps, sub) in [
            (1, Sub::HighScores),
            (2, Sub::Controls),
            (3, Sub::Options { selected: 0 }),
        ] {
            let mut state = Attract::new(now);
            for _ in 0..steps {
                state.key(&press(KeyCode::Down), &mut config, now);
            }
            state.key(&press(KeyCode::Enter), &mut config, now);
            assert_eq!(state.sub, Some(sub));
            state.key(&press(KeyCode::Esc), &mut config, now);
            assert_eq!(state.sub, None, "{sub:?}");
        }
    }

    #[test]
    fn leaving_the_options_panel_asks_for_a_save() {
        // §13.5: "`Esc` saves the config file (§6.2) and returns". The panel
        // itself only edits; saving is the caller's, as it is from the pause
        // menu.
        let now = Instant::now();
        let mut state = Attract::new(now);
        let mut config = ConfigFile::default();
        state.sub = Some(Sub::Options { selected: 0 });
        state.key(&press(KeyCode::Right), &mut config, now);
        assert_eq!(config.gameplay.preview_count, 6);
        assert_eq!(
            state.key(&press(KeyCode::Esc), &mut config, now),
            Outcome::OptionsClosed,
        );
    }

    #[test]
    fn the_panel_cycles_every_six_seconds_and_pauses_off_play() {
        // §13.3: "cycles every 6 seconds between three faces... The cycle
        // pauses while a menu item other than PLAY is selected."
        let start = Instant::now();
        let mut state = Attract::new(start);
        let cells = (30, 24);
        assert_eq!(state.face, 0);
        assert!(state.step(start + FACE, cells, false), "the face changed");
        assert_eq!(state.face, 1);
        state.step(start + FACE * 3, cells, false);
        assert_eq!(state.face, 3, "and round to the first face again");

        state.selected = 1;
        state.step(start + FACE * 9, cells, false);
        assert_eq!(state.face, 3, "held while HIGH SCORES is selected");
        state.selected = 0;
        state.step(
            start + FACE * 9 + FACE - Duration::from_millis(1),
            cells,
            false,
        );
        assert_eq!(state.face, 3, "and it resumes from where it paused");
        state.step(start + FACE * 10, cells, false);
        assert_eq!(state.face, 4);
    }

    #[test]
    fn the_control_summary_hides_the_bindings_that_are_turned_off() {
        // §13.3, and acceptance A9: "the binding disappears from ... the
        // attract screen's controls panel".
        let chrome = chrome();
        let scores = Table::default();
        let all_on = ConfigFile::default();
        let joined = control_summary(&context(&chrome, &all_on, &scores)).join("");
        assert!(joined.contains("hold"));
        assert!(joined.contains("rotate 180"));

        let mut off = ConfigFile::default();
        off.gameplay.hold_enabled = false;
        off.gameplay.allow_180_rotation = false;
        let rows = control_summary(&context(&chrome, &off, &scores));
        assert!(!rows.join("").contains("hold"), "{rows:?}");
        assert!(!rows.join("").contains("rotate 180"), "{rows:?}");
        for row in &rows {
            assert_eq!(row.chars().count(), PANEL_WIDTH, "{row}");
        }
    }

    #[test]
    fn every_panel_face_fills_the_panel_exactly() {
        // A face that is short or long by a character makes the box ragged,
        // and nothing else would notice.
        let chrome = chrome();
        let config = ConfigFile::default();
        let mut scores = Table::default();
        for n in 0..3 {
            scores.insert(Entry {
                name: format!("PLAYER{n}"),
                score: 1_000 - n,
                level: 3,
                lines: 20,
                duration_secs: 90,
                date: "2026-09-05".to_string(),
            });
        }
        let cx = context(&chrome, &config, &scores);
        let empty = Table::default();
        let mut faces = vec![
            control_summary(&cx),
            top_three(&cx),
            top_three(&context(&chrome, &config, &empty)),
        ];
        // Each reminder is a face of its own, one line long (§13.3).
        faces.extend(REMINDERS.map(|tip| vec![centre(tip, PANEL_WIDTH)]));
        for face in faces {
            assert!(face.len() <= PANEL_ROWS, "{face:?}");
            for row in face {
                assert_eq!(row.chars().count(), PANEL_WIDTH, "{row}");
            }
        }
    }

    #[test]
    fn the_high_score_rows_are_all_the_same_width() {
        let entry = Entry {
            name: "M".repeat(crate::highscore::NAME_MAX),
            score: u64::from(u32::MAX),
            level: 999,
            lines: 99_999,
            duration_secs: 0,
            date: "2026-09-05".to_string(),
        };
        assert_eq!(entry_row(10, &entry).chars().count(), SCORES_WIDTH);
        assert_eq!(
            score_row("", "NAME", "SCORE", "LV", "LINES", "DATE")
                .chars()
                .count(),
            SCORES_WIDTH,
        );
    }

    /// A screen wide enough for the sub-screens and tall enough for the block.
    fn render(state: &Attract, cx: &Context) -> String {
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).expect("a test terminal");
        terminal
            .draw(|frame| draw(frame, state, cx))
            .expect("drew a frame");
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_screen_holds_the_wordmark_the_menu_and_the_panel() {
        // §13.3, at the minimum terminal size of §12.1.
        let chrome = chrome();
        let mut config = ConfigFile::default();
        // The background is deliberately off: it is the one part of the screen
        // that is not reproducible, and it is drawn behind everything else.
        config.display.show_debug = true;
        let scores = Table::default();
        let cx = context(&chrome, &config, &scores);
        let state = Attract::new(Instant::now());
        let screen = render(&state, &cx);
        for line in ART.lines() {
            assert!(screen.contains(line.trim_end()), "{screen}");
        }
        for choice in MenuChoice::ALL {
            assert!(
                screen.contains(choice.label()),
                "{}\n{screen}",
                choice.label()
            );
        }
        assert!(screen.contains("\u{25b8} PLAY"), "{screen}");
        assert!(screen.contains("ENTER start"), "{screen}");
        assert!(screen.contains("soft drop"), "the first face\n{screen}");
    }

    #[test]
    fn every_sub_screen_fits_the_minimum_terminal() {
        // §12.1: 60 x 24 is the minimum, and a sub-screen is drawn over the
        // whole frame rather than over the block.
        let chrome = chrome();
        let config = ConfigFile::default();
        let mut scores = Table::default();
        for n in 0..crate::highscore::CAPACITY as u64 {
            scores.insert(Entry {
                name: format!("PLAYER{n}"),
                score: 1_000 - n,
                level: 3,
                lines: 20,
                duration_secs: 90,
                date: "2026-09-05".to_string(),
            });
        }
        let cx = Context {
            recent: Some(0),
            mode: InputMode::Legacy,
            ..context(&chrome, &config, &scores)
        };
        let mut state = Attract::new(Instant::now());

        state.sub = Some(Sub::HighScores);
        let screen = render(&state, &cx);
        assert!(screen.contains("HIGH SCORES"), "{screen}");
        assert!(screen.contains("PLAYER9"), "all ten rows\n{screen}");
        assert!(screen.contains("2026-09-05"), "{screen}");

        state.sub = Some(Sub::Controls);
        let screen = render(&state, &cx);
        assert!(screen.contains("Rotate counter-clockwise"), "{screen}");
        assert!(screen.contains("input: legacy"), "§8.2\n{screen}");

        state.sub = Some(Sub::Options { selected: 0 });
        let screen = render(&state, &cx);
        assert!(screen.contains("OPTIONS"), "{screen}");
        assert!(screen.contains("Lock down"), "{screen}");
    }

    #[test]
    fn the_animation_is_off_in_mono_and_with_the_debug_strip() {
        // §13.4's two exclusions.
        let mut config = ConfigFile::default();
        let scores = Table::default();
        let colour = Chrome {
            theme: Theme::with_glyphs(Depth::Truecolor, Glyphs::DEFAULT),
            ..chrome()
        };
        let mono = Chrome {
            theme: Theme::with_glyphs(Depth::Mono, Glyphs::DEFAULT),
            ..chrome()
        };
        assert!(animated(&context(&colour, &config, &scores)));
        assert!(!animated(&context(&mono, &config, &scores)), "mono");
        config.display.show_debug = true;
        assert!(!animated(&context(&colour, &config, &scores)), "show_debug",);
    }

    #[test]
    fn a_drifting_piece_falls_and_is_retired_at_the_bottom() {
        // §13.4: one new piece every ~1.2 s, falling a row at a time, removed
        // when it leaves the bottom, at most twelve at once.
        let start = Instant::now();
        let mut background = Background::new(start);
        // Tall enough that nothing retires while the cap is being tested.
        let deep = (30u16, 4_000u16);
        let mut now = start;
        for _ in 0..DRIFTERS * 2 {
            now += SPAWN;
            background.step(now, deep);
        }
        assert_eq!(background.pieces.len(), DRIFTERS, "capped at twelve");
        let highest = background
            .pieces
            .iter()
            .map(|p| p.row)
            .max()
            .expect("a piece");
        assert!(highest > -4, "and they fell on the way: {highest}");

        // On a real screen they leave the bottom and are dropped.
        let cells = (30u16, 24u16);
        now += FALL * u32::from(cells.1 + 8) * 2;
        background.step(now, cells);
        assert!(background.pieces.is_empty(), "{}", background.pieces.len());
    }
}
