//! §G7: the attract screen, drawn for a window.
//!
//! The state is [`shell::attract::Attract`](crate::shell::attract::Attract) and
//! is shared — what is selected, which sub-screen is open, how far the panel's
//! six-second cycle has come, how long the keyboard has been quiet — and so are
//! the words: §13.2's letterforms, §13.3's reminders and §13.4's numbers all
//! live beside it, because a second front-end that drew a different wordmark
//! would be a second piece of branding (§1.3). What is here is pixels.
//!
//! Two of §13.5's three sub-screens are not here either. The Options panel and
//! the controls table are the *same* boxes the pause menu opens, and they are
//! drawn by [`gui::overlays`](crate::gui::overlays) over whichever screen asked
//! for them; the high-score table is this screen's alone.
//!
//! The screen is laid out in §G3's grid — the same block, the same cell, the
//! same minimum — so that a window that has been dragged too small says one
//! thing rather than two, and so that leaving a game for the attract screen
//! does not resize anything (§G7.1).

use rand::rngs::Xoshiro256PlusPlus;
use rand::{RngExt, SeedableRng};

use crate::core::{PieceKind, Rotation};
use crate::gui::layout::{LAYOUT_COLS, Layout};
use crate::gui::overlays::{self, Panels};
use crate::gui::paint::{self, Face};
use crate::shell::attract::{
    self, Attract, DRIFT_FALL, DRIFT_JITTER, DRIFT_SPAWN, DRIFTERS, REMINDERS,
};
use crate::shell::config::ConfigFile;
use crate::shell::figures::thousands;
use crate::shell::highscore::{CAPACITY, Entry, Table};
use crate::shell::menus::{MenuChoice, Sub};
use crate::shell::palette;
use crate::shell::time::Stamp;

// ---------------------------------------------------------------------------
// §13.3: where everything is, in §G3's cells
// ---------------------------------------------------------------------------

/// The wordmark's five rows, under a two-cell margin (§13.2).
const WORDMARK_ROW: u32 = 2;
/// The full name, a row under the wordmark (§13.2).
const SUBTITLE_ROW: u32 = WORDMARK_ROW + attract::WORDMARK_ROWS as u32 + 1;
/// The band the menu is centred in: five rows, whatever the menu's length
/// (§13.3, `Session::menu`). Reserving the rows rather than following the menu
/// is what keeps the panel under it in the same place in both builds.
const MENU_ROW: u32 = SUBTITLE_ROW + 2;
const MENU_ROWS: u32 = 5;
/// The lit bar a menu item is drawn in: the well's ten cells and its walls, as
/// §12.6's pause menu takes.
const MENU_COLS: u32 = 12;
/// The cycling panel, a row under the menu (§13.3).
const PANEL_ROW: u32 = MENU_ROW + MENU_ROWS + 1;
/// Its four interior rows, a label row's worth of chrome either side.
const PANEL_ROWS: u32 = FACE_ROWS as u32 + 2;
const PANEL_COLS: u32 = LAYOUT_COLS - 2;
/// §13.3: the panel is four rows, because seven control entries do not fit in
/// six slots and the high-score face wants a heading and three names.
const FACE_ROWS: usize = 4;
/// The footer, under the panel (§13.3).
const FOOTER_ROW: u32 = PANEL_ROW + PANEL_ROWS;

/// The high-score sub-screen's box (§13.5): rank, name, score, level, lines and
/// date, which is as wide as anything on this screen gets.
const SCORES_COLS: u32 = 24;

/// A menu item, in cells.
const MENU_SIZE: f32 = 0.8;
/// The name under the wordmark.
const SUBTITLE_SIZE: f32 = 0.8;
/// A row of the panel, and of the high-score table.
const FACE_SIZE: f32 = 0.6;
/// The footer.
const FOOTER_SIZE: f32 = 0.55;

/// §13.4: heavily dimmed — the same 28 % the terminal's outlines take, which is
/// below even the ghost's 45 %, because the drift is meant to be noticed only
/// when nothing else is happening.
const DRIFT_BRIGHTNESS: u8 = 28;

// ---------------------------------------------------------------------------
// §13.4: the drift
// ---------------------------------------------------------------------------

/// One drifting tetromino outline (§13.4).
struct Drifter {
    kind: PieceKind,
    rotation: Rotation,
    /// In cells of the viewport, not of the block: the drift is behind the
    /// whole screen. `row` starts negative, so a piece drifts in from above
    /// rather than appearing whole.
    col: i32,
    row: i32,
    period: std::time::Duration,
    since: Stamp,
}

/// §13.4's ambient animation, in a window's units.
///
/// The terminal's [`Background`](crate::tui::attract::Background) is the same
/// animation on a character grid, and the two are separate for the reason
/// `shell::attract` gives: how often, how many and how fast are §13.4's and are
/// shared, and *where* a piece is is measured in whatever the screen is made
/// of.
///
/// Its entropy is F3's, `host::seed`, rather than the OS's directly: a browser
/// tab has no OS entropy source to reach for (`EGUI-PLAN.md` G3), and the seed
/// is a capability the front-end already supplies. Nothing here is ever
/// replayed — §15.4's obligations are the core's alone — so the generator only
/// has to look random, and it is [`Xoshiro256PlusPlus`] because that is the
/// one this project names (§9.6).
pub struct Drift {
    pieces: Vec<Drifter>,
    rng: Xoshiro256PlusPlus,
    spawned: Stamp,
}

impl Drift {
    pub fn new(seed: u64, now: Stamp) -> Self {
        Self {
            pieces: Vec::new(),
            rng: Xoshiro256PlusPlus::seed_from_u64(seed),
            spawned: now,
        }
    }

    /// Spawn, fall and retire, over a viewport `cells` cells across and down.
    ///
    /// No "did anything move": this front-end draws every repaint and has no
    /// frame to compare (§G1.3), which is the one thing the terminal's version
    /// of this had to answer.
    pub fn step(&mut self, now: Stamp, (columns, rows): (i32, i32)) {
        if now.saturating_since(self.spawned) >= DRIFT_SPAWN {
            self.spawned = now;
            if self.pieces.len() < DRIFTERS && columns > 4 {
                let jitter = self
                    .rng
                    .random_range(0..=DRIFT_JITTER.as_millis() as u64 * 2);
                self.pieces.push(Drifter {
                    kind: PieceKind::ALL[self.rng.random_range(0..PieceKind::ALL.len())],
                    rotation: Rotation::from_index(self.rng.random_range(0..4)),
                    col: self.rng.random_range(0..columns - 3),
                    row: -4,
                    period: DRIFT_FALL - DRIFT_JITTER + std::time::Duration::from_millis(jitter),
                    since: now,
                });
            }
        }
        for piece in &mut self.pieces {
            while now.saturating_since(piece.since) >= piece.period {
                piece.since += piece.period;
                piece.row += 1;
            }
        }
        self.pieces.retain(|piece| piece.row <= rows);
    }
}

// ---------------------------------------------------------------------------
// drawing
// ---------------------------------------------------------------------------

/// What the screen needs that is not its own state.
///
/// `hold_enabled` and `allow_180_rotation` are read from the config rather than
/// from a running game: there is no game on this screen, so the config *is* the
/// answer, and §13.3 hides the bindings those two settings turn off.
pub struct Context<'a> {
    pub config: &'a ConfigFile,
    /// The rows the §13.5 panel offers and the items §13.3's menu offers, both
    /// of them the lists the shell is navigating (`Session::settings`,
    /// `Session::menu`).
    pub panels: Panels<'a>,
    pub menu: &'static [MenuChoice],
    pub scores: &'a Table,
    /// The entry the run that just finished added, highlighted by §13.5's
    /// high-score screen.
    pub recent: Option<usize>,
}

/// Draw the attract screen (§13.3), with a sub-screen over it if one is open.
///
/// `area` is the whole viewport, which is what §13.4's drift is spread over;
/// everything else is in `layout`'s block.
pub fn draw(
    painter: &egui::Painter,
    layout: &Layout,
    area: egui::Rect,
    state: &Attract,
    drift: &Drift,
    cx: &Context,
) {
    // §13.4, behind everything. Its one exclusion here is `show_debug`:
    // §13.4's other is `mono`, and a window has no colour depths at all.
    if !cx.config.display.show_debug {
        self::drift(painter, layout, area, drift);
    }
    wordmark(painter, layout, state.idle_shift());
    paint::text(
        painter,
        layout.cells(0, SUBTITLE_ROW, LAYOUT_COLS, 1).center(),
        egui::Align2::CENTER_CENTER,
        attract::SUBTITLE,
        Face::Label,
        layout.cell() * SUBTITLE_SIZE,
        paint::TEXT,
        layout.block().width(),
    );
    menu(painter, layout, state, cx);
    face(painter, layout, state, cx);
    footer(painter, layout);

    if let Some(sub) = state.sub() {
        // A box reads over the screen it is on, exactly as §G5's do over the
        // playing screen; there is no §9.17 blanking to do here, because there
        // is no stack to give a free look at.
        painter.rect_filled(area, 0.0, overlays::SCRIM);
        match sub {
            Sub::HighScores => high_scores(painter, layout, cx),
            sub => overlays::sub(painter, layout, &sub, &cx.panels),
        }
    }
}

/// §13.4's drifting outlines, behind the screen.
///
/// Outlines rather than filled tiles, as §13.4 has them: a terminal draws them
/// with `░░`, and what that says in a window is a stroked cell. They are drawn
/// over the whole viewport and under everything else — a window has alpha, so
/// §13.4's "painted after it, opaquely" is met by the panel's own ground and by
/// how dim these are, rather than by covering the drift up.
fn drift(painter: &egui::Painter, layout: &Layout, area: egui::Rect, drift: &Drift) {
    let cell = layout.cell();
    // A stroke of a twelfth of the cell, never thinner than a physical pixel.
    let width = (layout.cell_pixels() / 12).max(1) as f32 * cell / layout.cell_pixels() as f32;
    for piece in &drift.pieces {
        let colour = paint::piece(piece.kind, DRIFT_BRIGHTNESS);
        for mino in piece.kind.cells(piece.rotation) {
            let at = egui::pos2(
                area.left() + (piece.col + mino.x) as f32 * cell,
                area.top() + (piece.row + mino.y) as f32 * cell,
            );
            let rect = egui::Rect::from_min_size(at, egui::vec2(cell, cell));
            if !area.intersects(rect) {
                continue;
            }
            painter.rect_stroke(
                rect.shrink(cell / 16.0),
                0.0,
                egui::Stroke::new(width.max(1.0), colour),
                egui::StrokeKind::Inside,
            );
        }
    }
}

/// §13.2's wordmark, each block of the letterforms drawn as a mino.
///
/// Which is what a window can do that a terminal cannot: the letters are made
/// of the same tiles the well is, in the same levelled palette (§12.3), so the
/// wordmark is visibly built out of the game rather than set in a font.
fn wordmark(painter: &egui::Painter, layout: &Layout, shift: usize) {
    let band = layout.cells(0, WORDMARK_ROW, LAYOUT_COLS, attract::WORDMARK_ROWS as u32);
    let size = (
        attract::WORDMARK_BLOCKS as u32,
        attract::WORDMARK_ROWS as u32,
    );
    let mut left = 0u32;
    for (index, letter) in attract::WORDMARK.iter().enumerate() {
        let colour = paint::piece(attract::wordmark_colour(index, shift), palette::FULL);
        let mut width = 0;
        for (row, blocks) in letter.iter().enumerate() {
            width = width.max(blocks.chars().count() as u32);
            for (col, block) in blocks.chars().enumerate() {
                if block != '#' {
                    continue;
                }
                let at = (left + col as u32, row as u32);
                paint::mino(painter, layout, layout.in_slot(band, size, at), colour);
            }
        }
        left += width + attract::WORDMARK_GAP as u32;
    }
}

/// §13.3's five items, the selected one lit (`Session::menu`).
fn menu(painter: &egui::Painter, layout: &Layout, state: &Attract, cx: &Context) {
    let cell = layout.cell();
    let band = layout.cells(
        (LAYOUT_COLS - MENU_COLS) / 2,
        MENU_ROW,
        MENU_COLS,
        MENU_ROWS,
    );
    // Centred in the band rather than filling it, so a menu one item shorter
    // than §13.3's — a tab's, which cannot quit — sits where the eye expects.
    let top = band.top() + (band.height() - cx.menu.len() as f32 * cell) / 2.0;
    for (index, choice) in cx.menu.iter().enumerate() {
        let row = egui::Rect::from_min_size(
            egui::pos2(band.left(), top + cell * index as f32),
            egui::vec2(band.width(), cell),
        );
        let lit = index == state.selected() && state.sub().is_none();
        if lit {
            painter.rect_filled(row, cell * 0.1, overlays::SELECTED);
            overlays::marker(painter, row, cell);
        }
        // Centred on the row, not shifted clear of the marker: the marker sits
        // inside the bar's left margin and the longest label is half the bar
        // wide, so there is nothing to make room for — and an item that is a
        // little off-centre under a wordmark that is exactly centred is
        // noticeable. The room allowed for a label keeps a cell and a half
        // either side, which is what the marker occupies on one of them.
        paint::text(
            painter,
            row.center(),
            egui::Align2::CENTER_CENTER,
            choice.label(),
            Face::Label,
            cell * MENU_SIZE,
            if lit { paint::TEXT } else { paint::LABEL },
            row.width() - cell * 3.0,
        );
    }
}

/// The cycling panel beneath the menu (§13.3): its box, and whichever of the
/// three faces is up.
fn face(painter: &egui::Painter, layout: &Layout, state: &Attract, cx: &Context) {
    let cell = layout.cell();
    let panel = layout.cells(1, PANEL_ROW, PANEL_COLS, PANEL_ROWS);
    painter.rect_filled(panel, cell * 0.2, paint::PANEL);
    // A face with fewer rows than the panel is **centred in it**, as the
    // terminal's is, rather than sitting against the top: the summary is three
    // rows when 180 rotation is off and four when it is on, and a panel that
    // filled from the top would look like it had lost something.
    let rows = |used: usize| -> Vec<egui::Rect> {
        let top = panel.top() + cell * (1.0 + (FACE_ROWS - used.min(FACE_ROWS)) as f32 / 2.0);
        (0..used)
            .map(|index| {
                egui::Rect::from_min_size(
                    egui::pos2(panel.left() + cell * 0.5, top + cell * index as f32),
                    egui::vec2(panel.width() - cell, cell),
                )
            })
            .collect()
    };
    match state.face() % 3 {
        0 => {
            let entries = summary(cx.config);
            control_summary(painter, layout, &rows(entries.len().div_ceil(2)), &entries);
        }
        1 => top_three(
            painter,
            layout,
            &rows(cx.scores.top(3).len().max(1) + 1),
            cx,
        ),
        _ => {
            // One line, in the middle of the panel.
            let reminder = REMINDERS[state.face() / 3 % REMINDERS.len()];
            paint::text(
                painter,
                panel.center(),
                egui::Align2::CENTER_CENTER,
                reminder,
                Face::Label,
                cell * FACE_SIZE,
                paint::TEXT,
                panel.width() - cell,
            );
        }
    }
}

/// The quick control summary (§13.3), reflowed around the bindings that are
/// actually available (§13.3, §17.3 A9), two entries to a row.
///
/// The words are this front-end's rather than `shell`'s, and that is the one
/// place §13.3's panel differs between the two: the terminal names the movement
/// keys with arrows, and a window has no guarantee that its fonts carry them
/// (§G5.1). The *rule* — which entries are listed at all — is §13.3's, and both
/// front-ends apply it to their own words.
fn summary(config: &ConfigFile) -> Vec<(&'static str, &'static str)> {
    let mut entries: Vec<(&str, &str)> = vec![
        ("Left / Right", "move"),
        ("Up", "rotate"),
        ("Down", "soft drop"),
        ("Space", "hard drop"),
        ("Z", "rotate ccw"),
    ];
    if config.gameplay.allow_180_rotation {
        entries.push(("A", "rotate 180"));
    }
    if config.gameplay.hold_enabled {
        entries.push(("C", "hold"));
    }
    entries
}

/// The summary, set out two entries to a row.
fn control_summary(
    painter: &egui::Painter,
    layout: &Layout,
    rows: &[egui::Rect],
    entries: &[(&str, &str)],
) {
    let cell = layout.cell();
    for (index, (key, label)) in entries.iter().enumerate() {
        let Some(row) = rows.get(index / 2) else {
            break;
        };
        let half = row.width() / 2.0;
        let left = row.left() + half * (index % 2) as f32;
        paint::text(
            painter,
            egui::pos2(left + half * 0.45, row.center().y),
            egui::Align2::RIGHT_CENTER,
            key,
            Face::Label,
            cell * FACE_SIZE,
            paint::TEXT,
            half * 0.44,
        );
        paint::text(
            painter,
            egui::pos2(left + half * 0.5, row.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            Face::Label,
            cell * FACE_SIZE,
            paint::LABEL,
            half * 0.48,
        );
    }
}

/// The top three, as the panel shows them (§13.3).
fn top_three(painter: &egui::Painter, layout: &Layout, rows: &[egui::Rect], cx: &Context) {
    let cell = layout.cell();
    paint::text(
        painter,
        rows[0].center(),
        egui::Align2::CENTER_CENTER,
        "HIGH SCORES",
        Face::Label,
        cell * FACE_SIZE,
        paint::LABEL,
        rows[0].width(),
    );
    let top = cx.scores.top(3);
    if top.is_empty() {
        paint::text(
            painter,
            rows[1].center(),
            egui::Align2::CENTER_CENTER,
            "no scores yet",
            Face::Label,
            cell * FACE_SIZE,
            paint::FAINT,
            rows[1].width(),
        );
        return;
    }
    for (index, entry) in top.iter().enumerate() {
        let row = rows[index + 1];
        overlays::pair(
            painter,
            layout,
            row,
            &format!("{}. {}", index + 1, entry.name),
            &thousands(entry.score),
            paint::TEXT,
        );
    }
}

/// The line under the panel: the version, and what the two keys do (§13.3).
fn footer(painter: &egui::Painter, layout: &Layout) {
    let cell = layout.cell();
    let row = layout.cells(0, FOOTER_ROW, LAYOUT_COLS, 1);
    paint::text(
        painter,
        row.center(),
        egui::Align2::CENTER_CENTER,
        concat!(
            "v",
            env!("CARGO_PKG_VERSION"),
            "    Up / Down select    Enter start"
        ),
        Face::Label,
        cell * FOOTER_SIZE,
        paint::FAINT,
        row.width(),
    );
}

/// §13.5's high-score sub-screen: the full top ten, the most recent entry
/// highlighted.
fn high_scores(painter: &egui::Painter, layout: &Layout, cx: &Context) {
    let area = layout.overlay(SCORES_COLS, 1 + CAPACITY as u32 + overlays::CHROME_ROWS);
    overlays::frame(painter, layout, area, "HIGH SCORES");
    let cell = layout.cell();
    let heading = overlays::row_rect(layout, area, 0);
    score_row(
        painter,
        layout,
        heading,
        ("", "NAME", "SCORE", "LV", "LINES", "DATE"),
        paint::FAINT,
    );
    if cx.scores.entries.is_empty() {
        paint::text(
            painter,
            overlays::row_rect(layout, area, 2).center(),
            egui::Align2::CENTER_CENTER,
            "no scores yet",
            Face::Label,
            cell * FACE_SIZE,
            paint::FAINT,
            area.width() - cell,
        );
    }
    for (index, entry) in cx.scores.entries.iter().enumerate() {
        let row = overlays::row_rect(layout, area, index + 1);
        // §13.5: the entry the run that just finished added, in the I-piece
        // cyan the terminal highlights it with.
        let colour = if cx.recent == Some(index) {
            paint::piece(PieceKind::I, palette::FULL)
        } else {
            paint::TEXT
        };
        score_row(painter, layout, row, columns(index + 1, entry), colour);
    }
    overlays::hint(painter, layout, area, "Esc returns");
}

/// One entry, as the six columns print it.
fn columns(rank: usize, entry: &Entry) -> (String, String, String, String, String, String) {
    (
        format!("{rank}"),
        entry.name.clone(),
        thousands(entry.score),
        entry.level.to_string(),
        entry.lines.to_string(),
        entry.date.clone(),
    )
}

/// One row of the high-score table: rank, name, score, level, lines, date.
///
/// The columns are fractions of the row rather than character counts, which is
/// the pixel reading of §13.5's table; a figure too wide for its column is
/// drawn smaller rather than over its neighbour ([`paint::text`]).
fn score_row<T: AsRef<str>>(
    painter: &egui::Painter,
    layout: &Layout,
    row: egui::Rect,
    (rank, name, score, level, lines, date): (T, T, T, T, T, T),
    colour: egui::Color32,
) {
    let size = layout.cell() * FACE_SIZE;
    let width = row.width();
    let at = |fraction: f32| egui::pos2(row.left() + width * fraction, row.center().y);
    let put = |text: &str, fraction: f32, align: egui::Align2, room: f32| {
        paint::text(
            painter,
            at(fraction),
            align,
            text,
            Face::Figure,
            size,
            colour,
            width * room,
        );
    };
    put(rank.as_ref(), 0.07, egui::Align2::RIGHT_CENTER, 0.06);
    put(name.as_ref(), 0.10, egui::Align2::LEFT_CENTER, 0.26);
    put(score.as_ref(), 0.60, egui::Align2::RIGHT_CENTER, 0.28);
    put(level.as_ref(), 0.68, egui::Align2::RIGHT_CENTER, 0.07);
    put(lines.as_ref(), 0.78, egui::Align2::RIGHT_CENTER, 0.09);
    put(date.as_ref(), 0.80, egui::Align2::LEFT_CENTER, 0.20);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::layout::{LAYOUT_ROWS, Measure};
    use crate::shell::menus::Setting;

    fn layout(width: f32, height: f32, ppp: f32) -> Layout {
        match Measure::of(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height)),
            ppp,
        ) {
            Measure::Fits(layout) => layout,
            small => panic!("{width} x {height} at {ppp}: {small:?}"),
        }
    }

    fn table(entries: usize) -> Table {
        let mut scores = Table::default();
        for n in 0..entries as u64 {
            scores.insert(Entry {
                name: "M".repeat(crate::shell::highscore::NAME_MAX),
                score: u64::from(u32::MAX) - n,
                level: 15,
                lines: 99_999,
                duration_secs: 90,
                date: "2026-09-12".to_string(),
            });
        }
        scores
    }

    #[test]
    fn the_screen_fits_the_block_it_shares_with_the_playing_screen() {
        // §G7.1: the attract screen is laid out in §G3's grid, so everything on
        // it has to be inside the same 26 x 24 block — including the high-score
        // box, which is the widest thing either screen draws.
        let layout = layout(728.0, 672.0, 1.0);
        let block = layout.block();
        const { assert!(FOOTER_ROW < LAYOUT_ROWS, "the footer is above the margin") };
        for rect in [
            layout.cells(0, WORDMARK_ROW, LAYOUT_COLS, attract::WORDMARK_ROWS as u32),
            layout.cells(
                (LAYOUT_COLS - MENU_COLS) / 2,
                MENU_ROW,
                MENU_COLS,
                MENU_ROWS,
            ),
            layout.cells(1, PANEL_ROW, PANEL_COLS, PANEL_ROWS),
            layout.cells(0, FOOTER_ROW, LAYOUT_COLS, 1),
            layout.overlay(SCORES_COLS, 1 + CAPACITY as u32 + overlays::CHROME_ROWS),
        ] {
            assert!(block.contains_rect(rect), "{rect:?} in {block:?}");
        }
    }

    #[test]
    fn the_wordmark_is_the_letterforms_the_terminal_draws() {
        // §13.2, §1.3: one wordmark, not one per front-end. The blocks here are
        // minos rather than pairs of characters, so what this checks is that
        // the *bitmap* is laid out as the specification prints it — fifteen
        // blocks across, five rows down, with a blank column between letters.
        let across: usize = attract::WORDMARK
            .iter()
            .map(|letter| letter.iter().map(|row| row.chars().count()).max().unwrap())
            .sum::<usize>()
            + attract::WORDMARK_GAP * (attract::WORDMARK.len() - 1);
        assert_eq!(across, attract::WORDMARK_BLOCKS);
        for letter in attract::WORDMARK {
            assert_eq!(letter.len(), attract::WORDMARK_ROWS);
            for row in letter {
                assert!(row.chars().all(|c| c == '#' || c == '.'), "{row}");
            }
        }
        // §13.6 walks all seven colours, so the three letters are positions in
        // §9.2's table and not three fixed colours.
        let first = |shift| attract::wordmark_colour(0, shift);
        for shift in 1..attract::WORDMARK_CYCLE.len() {
            assert_ne!(first(shift), first(0), "shift {shift} repeats too early");
        }
        assert_eq!(first(attract::WORDMARK_CYCLE.len()), first(0));
    }

    #[test]
    fn the_menu_and_the_name_are_centred_on_the_wordmark() {
        // The whole screen is one centred column, and the eye reads it as one:
        // a menu item half a cell off centre under a wordmark that is exactly
        // centred is noticeable, and was. The marker beside the selected item
        // sits inside the bar's margin and is not something the label has to
        // make room for.
        let ctx = egui::Context::default();
        let config = ConfigFile::default();
        let scores = Table::default();
        let layout = layout(728.0, 672.0, 1.0);
        let middle = layout.block().center().x;
        let drawn = placed(&ctx, &config, &scores, 0, None);
        let mut found = 0;
        let mut centred: Vec<&str> = MenuChoice::CANVAS
            .iter()
            .map(|choice| choice.label())
            .collect();
        centred.push(attract::SUBTITLE);
        for (text, rect) in &drawn {
            if !centred.contains(&text.as_str()) {
                continue;
            }
            found += 1;
            assert!(
                (rect.center().x - middle).abs() <= 1.0,
                "{text:?} is centred at {} and the block at {middle}",
                rect.center().x,
            );
        }
        assert_eq!(found, MenuChoice::CANVAS.len() + 1, "{drawn:?}");
    }

    #[test]
    fn the_control_summary_hides_the_bindings_that_are_turned_off() {
        // §13.3, and acceptance A9: "the binding disappears from ... the
        // attract screen's controls panel". The words are this front-end's;
        // the rule is §13.3's, and it is checked in both.
        let ctx = egui::Context::default();
        let mut config = ConfigFile::default();
        let scores = Table::default();
        let all_on = drawn(&ctx, &config, &scores, 0, None);
        assert!(all_on.contains(&"hold".to_string()), "{all_on:?}");
        assert!(all_on.contains(&"rotate 180".to_string()), "{all_on:?}");

        config.gameplay.hold_enabled = false;
        config.gameplay.allow_180_rotation = false;
        let off = drawn(&ctx, &config, &scores, 0, None);
        assert!(!off.contains(&"hold".to_string()), "{off:?}");
        assert!(!off.contains(&"rotate 180".to_string()), "{off:?}");
    }

    #[test]
    fn the_panel_centres_a_face_that_is_short_of_rows() {
        // §13.3's panel is four rows and every face is shorter than that at
        // some setting: the summary is four rows with everything turned on and
        // three with 180 rotation off, and the high-score face is two when the
        // table is empty. A face that filled from the top would look like it
        // had lost a row. The terminal centres its faces for the same reason.
        let ctx = egui::Context::default();
        let scores = Table::default();
        let layout = layout(728.0, 672.0, 1.0);
        let panel = layout.cells(1, PANEL_ROW, PANEL_COLS, PANEL_ROWS);
        for rotate_180 in [true, false] {
            let mut config = ConfigFile::default();
            config.gameplay.allow_180_rotation = rotate_180;
            let labels: Vec<&str> = summary(&config).iter().map(|(_, word)| *word).collect();
            let drawn = placed(&ctx, &config, &scores, 0, None);
            let rows: Vec<egui::Rect> = drawn
                .iter()
                .filter(|(text, _)| labels.contains(&text.as_str()))
                .map(|(_, rect)| *rect)
                .collect();
            assert_eq!(rows.len(), labels.len(), "{drawn:?}");
            let top = rows.iter().map(|r| r.top()).fold(f32::MAX, f32::min);
            let bottom = rows.iter().map(|r| r.bottom()).fold(f32::MIN, f32::max);
            assert!(
                ((top + bottom) / 2.0 - panel.center().y).abs() <= 1.0,
                "{} entries sit at {}..{} in a panel centred on {}",
                labels.len(),
                top,
                bottom,
                panel.center().y,
            );
        }
    }

    #[test]
    fn the_panel_and_the_sub_screen_show_a_recorded_score() {
        // §13.5, and the second half of §17.3's A6: a score that has been filed
        // appears on the panel's high-score face and on the sub-screen.
        let ctx = egui::Context::default();
        let config = ConfigFile::default();
        let scores = table(CAPACITY);
        let panel = drawn(&ctx, &config, &scores, 1, None);
        assert!(panel.iter().any(|text| text.contains("HIGH SCORES")));
        assert!(
            panel.iter().any(|text| text.contains("4,294,967,295")),
            "the top score, grouped in threes: {panel:?}",
        );
        let sub = drawn(&ctx, &config, &scores, 0, Some(Sub::HighScores));
        assert!(
            sub.iter().any(|text| text.contains("2026-09-12")),
            "{sub:?}"
        );
        assert_eq!(
            sub.iter()
                .filter(|text| text.contains("4,294,967,2"))
                .count(),
            CAPACITY,
            "all ten rows: {sub:?}",
        );
    }

    #[test]
    fn the_screen_draws_at_every_size_without_panicking() {
        // F6, as the playing screen's sweep does: every size from nothing to
        // enormous, at several densities, with every sub-screen open and every
        // face of the panel up — and with §13.6's idle cycle part-way round.
        let ctx = egui::Context::default();
        let subs = [
            None,
            Some(Sub::HighScores),
            Some(Sub::Controls),
            Some(Sub::Options {
                selected: Setting::SHARED.len() - 1,
            }),
        ];
        for size in [
            egui::vec2(0.0, 0.0),
            egui::vec2(1.0, 1.0),
            egui::vec2(363.0, 336.0),
            egui::vec2(364.0, 336.0),
            egui::vec2(728.0, 672.0),
            egui::vec2(3840.0, 2160.0),
        ] {
            for ppp in [1.0, 1.5, 2.0] {
                for face in 0..REMINDERS.len() * 3 {
                    for sub in &subs {
                        for scores in [Table::default(), table(CAPACITY)] {
                            let config = ConfigFile::default();
                            assert!(
                                paint(&ctx, size, ppp, &config, &scores, face, *sub, Some(0)) > 0
                            );
                        }
                    }
                }
            }
        }
    }

    /// Paint one frame into a headless `egui` context, as the playing screen's
    /// tests do, and hand back how many shapes a real window would have been
    /// sent.
    #[allow(clippy::too_many_arguments)]
    fn paint(
        ctx: &egui::Context,
        size: egui::Vec2,
        ppp: f32,
        config: &ConfigFile,
        scores: &Table,
        face: usize,
        sub: Option<Sub>,
        recent: Option<usize>,
    ) -> usize {
        let output = run(ctx, size, ppp, config, scores, face, sub, recent);
        let shapes = output.shapes.len();
        // There is no renderer to hand the font atlas to, and `egui` insists
        // that saying so is deliberate.
        output.drop_without_applying_deltas();
        shapes
    }

    /// Every string the screen drew, at a comfortable size: what a test can ask
    /// about without a window to look at.
    fn drawn(
        ctx: &egui::Context,
        config: &ConfigFile,
        scores: &Table,
        face: usize,
        sub: Option<Sub>,
    ) -> Vec<String> {
        placed(ctx, config, scores, face, sub)
            .into_iter()
            .map(|(text, _)| text)
            .collect()
    }

    /// The same, with where each string landed: what it is centred on.
    fn placed(
        ctx: &egui::Context,
        config: &ConfigFile,
        scores: &Table,
        face: usize,
        sub: Option<Sub>,
    ) -> Vec<(String, egui::Rect)> {
        let output = run(
            ctx,
            egui::vec2(728.0, 672.0),
            1.0,
            config,
            scores,
            face,
            sub,
            Some(0),
        );
        let mut text = Vec::new();
        for shape in &output.shapes {
            if let egui::epaint::Shape::Text(drawn) = &shape.shape {
                text.push((
                    drawn.galley.text().to_string(),
                    egui::Rect::from_min_size(drawn.pos, drawn.galley.size()),
                ));
            }
        }
        output.drop_without_applying_deltas();
        text
    }

    #[allow(clippy::too_many_arguments)]
    fn run(
        ctx: &egui::Context,
        size: egui::Vec2,
        ppp: f32,
        config: &ConfigFile,
        scores: &Table,
        face: usize,
        sub: Option<Sub>,
        recent: Option<usize>,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        };
        ctx.set_pixels_per_point(ppp);
        // §13.3's six-second cycle and §13.6's minute of quiet, wound forward
        // by the clock rather than by a setter: the state machine is shared, so
        // this is the same path a run takes.
        let start = Stamp::ZERO;
        let mut state = Attract::new(start);
        state.advance(start + std::time::Duration::from_secs(6 * face as u64));
        state.advance(start + std::time::Duration::from_secs(6 * face as u64 + 90));
        if let Some(sub) = sub {
            state.open(sub);
        }
        let mut drift = Drift::new(42, start);
        drift.step(start + DRIFT_SPAWN * 20, (40, 30));
        ctx.run_ui(input, |ui| {
            let area = ui.max_rect();
            let painter = ui.painter();
            match Measure::of(area, ui.ctx().pixels_per_point()) {
                Measure::Fits(layout) => {
                    let cx = Context {
                        config,
                        panels: Panels {
                            config,
                            settings: &Setting::SHARED,
                        },
                        menu: &MenuChoice::CANVAS,
                        scores,
                        recent,
                    };
                    draw(painter, &layout, area, &state, &drift, &cx);
                }
                Measure::TooSmall { need, have } => paint::too_small(painter, area, need, have),
            }
        })
    }
}
