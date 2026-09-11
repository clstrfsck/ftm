//! The attract screen, drawn (§13).
//!
//! §13 is explicitly provisional: build it plainly, look at it, then iterate.
//!
//! Like the rest of `tui` this draws and nothing else. `shell::attract` holds
//! the state — what is selected, which sub-screen is open, how far the panel's
//! cycle has come — and the one thing that stays here is [`Background`], the
//! §13.4 drift, because it is positioned in a character grid's own cells.
//! Neither half has a path to a `Game`, because on this screen there isn't one.

use std::time::Duration;

use rand::RngExt;
use rand::rngs::SmallRng;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::core::{PieceKind, Rotation};
use crate::shell::attract::Attract;
use crate::shell::config::ConfigFile;
use crate::shell::highscore::{Entry, Table};
use crate::shell::input::InputMode;
use crate::shell::menus::{MenuChoice, Sub};
use crate::shell::palette;
use crate::shell::time::Stamp;
use crate::tui::cells::CELL_WIDTH;
use crate::tui::overlays::{self, box_over, centre};
use crate::tui::theme::{self, Theme};
use crate::tui::{Chrome, centred};

// ---------------------------------------------------------------------------
// §13.2 the wordmark
// ---------------------------------------------------------------------------

/// The wordmark is five rows tall (§13.2).
const WORDMARK_ROWS: usize = 5;

/// The three letters of `FTM`, drawn from block characters — doubled
/// horizontally, so that three letters still carry the screen — and 30
/// characters wide altogether (§13.2).
///
/// An **original** block-letter wordmark: the official logo must not be used,
/// reproduced or approximated, and no official colours-as-branding, styling or
/// artwork may be copied (§1.3).
const WORDMARK: [[&str; WORDMARK_ROWS]; 3] = [
    [
        "\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}",
        "\u{2588}\u{2588}      ",
        "\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}  ",
        "\u{2588}\u{2588}      ",
        "\u{2588}\u{2588}      ",
    ],
    [
        "\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}",
        "  \u{2588}\u{2588}\u{2588}\u{2588}  ",
        "  \u{2588}\u{2588}\u{2588}\u{2588}  ",
        "  \u{2588}\u{2588}\u{2588}\u{2588}  ",
        "  \u{2588}\u{2588}\u{2588}\u{2588}  ",
    ],
    [
        "\u{2588}\u{2588}      \u{2588}\u{2588}",
        "\u{2588}\u{2588}\u{2588}\u{2588}  \u{2588}\u{2588}\u{2588}\u{2588}",
        "\u{2588}\u{2588}  \u{2588}\u{2588}  \u{2588}\u{2588}",
        "\u{2588}\u{2588}      \u{2588}\u{2588}",
        "\u{2588}\u{2588}      \u{2588}\u{2588}",
    ],
];

/// The full name, spelled out under the wordmark (§13.2).
const SUBTITLE: &str = "FALLING TETROMINO MANAGER";

/// The wordmark's width in characters: 8 + 2 + 8 + 2 + 10 (§13.2).
const WORDMARK_WIDTH: usize = 30;
/// Two spaces between letters, so the doubled strokes stay separate.
const LETTER_GAP: &str = "  ";

/// §9.2's seven colours in §13.2's order, which is what §13.6's idle cycle
/// walks along.
const WORDMARK_CYCLE: [PieceKind; 7] = [
    PieceKind::I,
    PieceKind::J,
    PieceKind::L,
    PieceKind::O,
    PieceKind::S,
    PieceKind::T,
    PieceKind::Z,
];

/// Where each letter starts in [`WORDMARK_CYCLE`]: `I`, `S` and `T` — cyan,
/// green and purple (§13.2).
///
/// Indices rather than three `PieceKind`s, because §13.6's idle cycle advances
/// every letter one step along all seven colours. Three fixed colours would
/// have made that cycle repeat after three seconds instead of seven.
const WORDMARK_START: [usize; 3] = [0, 4, 5];

/// The block the screen is laid out in (§13.3).
///
/// The width is the controls panel's, not the wordmark's — the wordmark is 30
/// and is centred inside it.
const BLOCK_WIDTH: usize = 36;
/// Wordmark, gap, subtitle, gap, menu, gap, panel, footer (§13.3).
const BLOCK_HEIGHT: u16 = 21;
/// The wordmark and its subtitle: five rows, a blank, the name, a blank
/// (§13.2, §13.3).
const HEADER_ROWS: u16 = WORDMARK_ROWS as u16 + 3;
/// The wordmark's left margin inside the block, which centres it.
const WORDMARK_X: usize = (BLOCK_WIDTH - WORDMARK_WIDTH) / 2;
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

/// The one-line rules reminders the third panel face rotates through (§13.3).
const REMINDERS: [&str; 5] = [
    "Clear 4 rows at once for a QUAD",
    "Back-to-back QUADs score 1.5x",
    "A T-spin double outscores a QUAD",
    "Hold parks a piece for later",
    "Every soft-dropped row is a point",
];

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
    since: Stamp,
}

/// §13.4's ambient animation, in the terminal's own units.
///
/// It stays here rather than in `shell::attract` because it is positioned in
/// **matrix cells of a character grid**: a window measures the same drift in
/// pixels, so each front-end keeps its own and folds its "did anything move"
/// answer in beside [`Attract::step`]'s (`EGUI.md` G2).
pub struct Background {
    pieces: Vec<Drifter>,
    rng: SmallRng,
    spawned: Stamp,
}

impl Background {
    pub fn new(now: Stamp) -> Self {
        Self {
            pieces: Vec::new(),
            // Cosmetic, so it is seeded from the environment: nothing here is
            // ever replayed, and §15.4's obligations are the core's alone.
            rng: rand::make_rng(),
            spawned: now,
        }
    }

    /// Spawn, fall and retire, reporting whether anything moved.
    pub fn step(&mut self, now: Stamp, (columns, rows): (u16, u16)) -> bool {
        let mut moved = false;
        if now.saturating_since(self.spawned) >= SPAWN {
            self.spawned = now;
            if self.pieces.len() < DRIFTERS && columns > 4 {
                let jitter = self
                    .rng
                    .random_range(0..=FALL_JITTER.as_millis() as u64 * 2);
                self.pieces.push(Drifter {
                    kind: PieceKind::ALL[self.rng.random_range(0..PieceKind::ALL.len())],
                    rotation: Rotation::from_index(self.rng.random_range(0..4)),
                    col: self
                        .rng
                        .random_range(0..i16::try_from(columns - 3).unwrap_or(1)),
                    row: -4,
                    period: FALL - FALL_JITTER + Duration::from_millis(jitter),
                    since: now,
                });
                moved = true;
            }
        }
        for piece in &mut self.pieces {
            while now.saturating_since(piece.since) >= piece.period {
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
pub fn draw(frame: &mut Frame, state: &Attract, background: &Background, cx: &Context) {
    let theme = cx.chrome.theme;
    // §12.1: below the minimum this screen is replaced too. The attract loop
    // asks the same question before it animates, but the guard belongs here as
    // well: nothing may reach a layout that assumes room it has not got.
    if !crate::tui::fits(frame.area().as_size()) {
        crate::tui::too_small(frame, theme);
        return;
    }
    // §13.4: the animation is disabled in `mono` and when `show_debug` is on.
    if animated(cx) {
        drift(frame, background, theme);
    }

    let area = frame.area();
    let block = centred(area, BLOCK_WIDTH as u16, BLOCK_HEIGHT);
    let mut lines = Vec::with_capacity(BLOCK_HEIGHT as usize);
    let shift = state.idle_shift();
    for row in 0..WORDMARK_ROWS {
        lines.push(wordmark_row(row, shift, theme));
    }
    lines.push(pad(String::new()));
    // §13.2: the full name under the wordmark. `FTM` is what the wordmark
    // says; this is what it stands for, and it is emphasised rather than
    // merely present — it is the line the screen is named after.
    //
    // `bold` rather than a white: §12.3 keeps to the terminal's own palette,
    // and a hard white would be brighter only on a dark background and close
    // to invisible on a light one. Bold is brighter everywhere, and it is the
    // one emphasis `mono` has as well.
    lines.push(Line::styled(centre(SUBTITLE, BLOCK_WIDTH), theme.bold()));
    lines.push(pad(String::new()));
    for (index, choice) in MenuChoice::ALL.iter().enumerate() {
        let selected = index == state.selected() && state.sub().is_none();
        let marker = if selected { "\u{25b8} " } else { "  " };
        let style = if selected {
            // §13.3: the selected item is drawn in the I-piece cyan.
            theme.piece(PieceKind::I, palette::FULL).patch(theme.bold())
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
        y: block.y + HEADER_ROWS + MenuChoice::ALL.len() as u16 + 1,
        width: BLOCK_WIDTH as u16,
        height: PANEL_ROWS as u16 + 2,
    };
    if panel.bottom() <= block.bottom() {
        face(frame, panel, state, cx);
    }

    match state.sub() {
        None => {}
        Some(Sub::HighScores) => high_scores(frame, area, cx),
        Some(Sub::Controls) => overlays::controls(frame, area, cx.chrome, cx.config, cx.mode),
        Some(Sub::Options { selected }) => {
            overlays::options(frame, area, cx.chrome, cx.config, selected)
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
    let mut spans = Vec::with_capacity(WORDMARK.len() * 2 + 2);
    // Padded to the block on both sides rather than centred by the paragraph:
    // every other line here is a full-width `pad`, and a short line would leave
    // the previous frame's characters behind it (§15.3 redraws only what moved).
    spans.push(Span::raw(" ".repeat(WORDMARK_X)));
    for (index, letter) in WORDMARK.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(LETTER_GAP));
        }
        let kind = WORDMARK_CYCLE[(WORDMARK_START[index] + shift) % WORDMARK_CYCLE.len()];
        spans.push(Span::styled(letter[row], theme.piece(kind, palette::FULL)));
    }
    spans.push(Span::raw(
        " ".repeat(BLOCK_WIDTH - WORDMARK_X - WORDMARK_WIDTH),
    ));
    Line::from(spans)
}

/// The cycling panel beneath the menu (§13.3).
fn face(frame: &mut Frame, area: Rect, state: &Attract, cx: &Context) {
    let theme = cx.chrome.theme;
    let mut rows = match state.face() % 3 {
        0 => control_summary(cx),
        1 => top_three(cx),
        _ => vec![centre(
            REMINDERS[state.face() / 3 % REMINDERS.len()],
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
            crate::shell::figures::thousands(entry.score),
        ));
    }
    rows
}

/// The interior width of the high-score sub-screen (§13.5): rank, name,
/// score, level, lines and date, with a margin either side.
///
/// Thirteen columns of it are the score, which is what a `u32::MAX` of them
/// needs once the digits are grouped in threes — two more than the bare digits
/// took. The box is 58 characters on screen and §12.1's minimum is 60, so it
/// still fits, and `the_high_score_rows_are_all_the_same_width` is what keeps
/// the table from going ragged if it ever stops fitting.
const SCORES_WIDTH: usize = 56;

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
            theme.piece(PieceKind::I, palette::FULL).patch(theme.bold())
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
    format!("  {rank:>2}  {name:<12}{score:>13}{level:>4}{lines:>7}  {date:<10}  ")
}

fn entry_row(rank: usize, entry: &Entry) -> String {
    score_row(
        &rank.to_string(),
        &entry.name,
        &crate::shell::figures::thousands(entry.score),
        &entry.level.to_string(),
        &entry.lines.to_string(),
        &entry.date,
    )
}

/// A block-width line, so the animation behind it is covered (§13.4).
fn pad(text: String) -> Line<'static> {
    Line::raw(format!("{text:<BLOCK_WIDTH$}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::highscore::Entry;
    use crate::tui::theme::{Depth, Glyphs};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Style;

    /// §13.2, transcribed literally.
    /// §13.2's art, transcribed exactly as the spec prints it: 30 characters
    /// wide, without the margins that centre it in the block.
    const ART: &str = "\
████████  ████████  ██      ██
██          ████    ████  ████
██████      ████    ██  ██  ██
██          ████    ██      ██
██          ████    ██      ██";

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
        // §13.2: 30 characters of art, 5 rows tall, centred in the 36-wide
        // block, and the letters are the ones the specification draws.
        let theme = chrome().theme;
        let drawn: Vec<String> = (0..WORDMARK_ROWS)
            .map(|row| plain(&wordmark_row(row, 0, theme)))
            .collect();
        for row in &drawn {
            assert_eq!(row.chars().count(), BLOCK_WIDTH, "{row}");
        }
        // Strip the margins the block adds, and what is left is the spec's art
        // — which also proves the art is centred rather than merely present.
        let art: Vec<String> = drawn
            .iter()
            .map(|row| {
                row.chars()
                    .skip(WORDMARK_X)
                    .take(WORDMARK_WIDTH)
                    .collect::<String>()
            })
            .collect();
        assert_eq!(art.join("\n"), ART);
        for row in &drawn {
            assert!(
                row.starts_with(&" ".repeat(WORDMARK_X)) && row.ends_with(' '),
                "centred in the block: {row:?}",
            );
        }
    }

    #[test]
    fn each_letter_takes_its_own_piece_colour() {
        // §13.2: `F`, `T` and `M` in the I, S and T colours — cyan, green and
        // purple — left to right, from §12.3's levelled palette like every
        // other piece colour on the screen.
        let theme = chrome().theme;
        let colours: Vec<Style> = wordmark_row(0, 0, theme)
            .spans
            .iter()
            .filter(|span| span.content.trim() != "")
            .map(|span| span.style)
            .collect();
        assert_eq!(colours.len(), 3);
        for (span, kind) in colours
            .iter()
            .zip([PieceKind::I, PieceKind::S, PieceKind::T])
        {
            assert_eq!(*span, theme.piece(kind, palette::FULL));
        }
    }

    #[test]
    fn the_idle_cycle_walks_all_seven_colours() {
        // §13.2, §13.6: the letters are *positions* in §9.2's seven, not three
        // fixed colours, so the cycle takes seven steps to come back rather
        // than three. Three colours would have made a much duller minute.
        let theme = chrome().theme;
        let first = |shift| {
            wordmark_row(0, shift, theme)
                .spans
                .iter()
                .find(|span| span.content.trim() != "")
                .map(|span| span.style)
                .expect("a letter")
        };
        let start = first(0);
        for shift in 1..WORDMARK_CYCLE.len() {
            assert_ne!(first(shift), start, "shift {shift} repeats too early");
        }
        assert_eq!(first(WORDMARK_CYCLE.len()), start, "and back after seven");
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
                // The widest the score column is sized for, grouped in threes
                // (§12.4): four digits would fit whatever the field did, and
                // would not notice a score that had outgrown it.
                name: format!("PLAYER{n}"),
                score: u64::from(u32::MAX) - n,
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
            name: "M".repeat(crate::shell::highscore::NAME_MAX),
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
        // The background is deliberately empty: it is the one part of the
        // screen that is not reproducible, and it is drawn behind everything
        // else (§13.4).
        let background = Background::new(Stamp::ZERO);
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).expect("a test terminal");
        terminal
            .draw(|frame| draw(frame, state, &background, cx))
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
        let state = Attract::new(Stamp::ZERO);
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
        assert!(
            screen.contains(SUBTITLE),
            "the name is spelled out\n{screen}"
        );
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
        for n in 0..crate::shell::highscore::CAPACITY as u64 {
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
        let mut state = Attract::new(Stamp::ZERO);

        state.open(Sub::HighScores);
        let screen = render(&state, &cx);
        assert!(screen.contains("HIGH SCORES"), "{screen}");
        assert!(screen.contains("PLAYER9"), "all ten rows\n{screen}");
        assert!(screen.contains("2026-09-05"), "{screen}");

        state.open(Sub::Controls);
        let screen = render(&state, &cx);
        assert!(screen.contains("Rotate counter-clockwise"), "{screen}");
        assert!(screen.contains("input: legacy"), "§8.2\n{screen}");

        state.open(Sub::Options { selected: 0 });
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
        let start = Stamp::ZERO;
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
