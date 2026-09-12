//! §G5: §12.6's boxes and the two panels behind them, drawn as pixels.
//!
//! The pause menu, §9.17's countdown, the game-over box, name entry, the §13.5
//! Options panel and the §10.1 controls table. **What each of them *does* is
//! not here**: the menus are walked, edited and closed by
//! [`shell::menus`](crate::shell::menus) through
//! [`Round::key`](crate::shell::round::Round::key), exactly as the terminal's
//! are, so the two front-ends cannot come to disagree about what an item means
//! (`EGUI.md` G2, G8). This module is the boxes they are drawn in.
//!
//! Two of them are worth saying out loud. Name entry is fed neutral keys and
//! is **not** an `egui::TextEdit`: §12.6's twelve printable ASCII and its
//! `ANON` default are rules about §14's table rather than about a text field,
//! and they live in `shell::menus::NameEntry` where both front-ends reach them.
//! And the Options panel offers the rows `Round::settings` reports — seven,
//! here, because §12.3's colour depth means nothing in a window — so the
//! cursor can never land on a row this screen is not drawing.

use crate::core::GameView;
use crate::gui::layout::Layout;
use crate::gui::paint::{self, Face};
use crate::shell::config::ConfigFile;
use crate::shell::figures::{clock, pps, thousands};
use crate::shell::menus::{self, Overlay, PauseChoice, Setting, Sub};
use crate::shell::round::FrameState;

/// What the two panels read: the config they show, and the rows the panel is
/// offering (§13.5, `Session::settings`).
///
/// A `ConfigFile` rather than a `GameView`, which is not a breach of §12.7: the
/// config is settings, not game state, and the panel is what edits them.
#[derive(Clone, Copy)]
pub struct Panels<'a> {
    pub config: &'a ConfigFile,
    pub settings: &'static [Setting],
}

/// The box's ground: a shade above the panels, so it reads as something over
/// the screen rather than part of it.
const BOX: egui::Color32 = egui::Color32::from_rgb(0x26, 0x26, 0x30);
/// The selected row of a menu or a panel.
pub(crate) const SELECTED: egui::Color32 = egui::Color32::from_rgb(0x39, 0x39, 0x48);
/// What dims the well under a box. Not a blank: §9.17's blanking is the
/// *game's* answer and has already emptied the well where it applies
/// (`Overlay::blanks`); this is only so the box reads over it.
pub(crate) const SCRIM: egui::Color32 = egui::Color32::from_black_alpha(0xB0);

/// A box's title, in cells.
const TITLE_SIZE: f32 = 0.85;
/// A row of a menu, a panel or a table, in cells.
pub(crate) const ROW_SIZE: f32 = 0.7;
/// The line at the foot of a box that says which keys it answers to.
const HINT_SIZE: f32 = 0.55;
/// The countdown numeral, in cells (§9.17).
const COUNT_SIZE: f32 = 6.0;

/// A box's rows: its title, its body, and the hint at its foot.
///
/// The body starts two cells down and each row is a cell tall, so a box with
/// `n` rows in it is `n + 4` tall: title, body, air, hint.
pub(crate) const CHROME_ROWS: u32 = 4;
/// The pause menu (§9.17), in cells: the well and its walls, as §12.6's box
/// covers the field and not the columns beside it.
const PAUSE_COLS: u32 = 12;
/// The game-over box (§12.6).
const OVER_COLS: u32 = 14;
/// Name entry (§12.6): the label, the longest name and its caret.
const NAME_COLS: u32 = 14;
/// The §13.5 Options panel: a label and the longest value.
const OPTIONS_COLS: u32 = 17;
/// The §10.1 controls table: the longest action and its keys.
const CONTROLS_COLS: u32 = 22;

/// Where a box lands, in points, or `None` for the two overlays that are not
/// boxes (§12.6, §G5).
///
/// The sizes are here rather than inside each box so that "does it fit in the
/// block?" is a question that can be asked without drawing anything.
pub fn rect_of(layout: &Layout, overlay: &Overlay, panels: &Panels) -> Option<egui::Rect> {
    let (cols, rows) = match overlay {
        // Nothing, and a numeral over the well rather than a box.
        Overlay::None | Overlay::Resuming { .. } => return None,
        Overlay::Paused { .. } => (PAUSE_COLS, PauseChoice::ALL.len() as u32 + CHROME_ROWS),
        Overlay::GameOver => (OVER_COLS, OVER_FIGURES + CHROME_ROWS),
        Overlay::NameEntry { .. } => (NAME_COLS, NAME_ROWS + CHROME_ROWS),
        Overlay::Options { .. } => (OPTIONS_COLS, panels.settings.len() as u32 + CHROME_ROWS),
        Overlay::Controls => (
            CONTROLS_COLS,
            menus::controls(panels.config).len() as u32 + CHROME_ROWS,
        ),
    };
    Some(layout.overlay(cols, rows))
}

/// One of §13.5's two sub-screens, drawn over the attract screen (§G7).
///
/// The Options panel and the controls table are the *same* boxes the pause menu
/// opens — §13.5 says so of the panel in as many words — so they are drawn by
/// the same code, over whichever screen asked for them. §13.5's third
/// sub-screen, the high-score table, belongs to the attract screen alone and is
/// drawn there.
pub fn sub(painter: &egui::Painter, layout: &Layout, sub: &Sub, panels: &Panels) {
    let overlay = as_overlay(sub);
    let Some(area) = rect_of(layout, &overlay, panels) else {
        return;
    };
    match overlay {
        Overlay::Options { selected } => options(painter, layout, area, panels, selected),
        Overlay::Controls => controls(painter, layout, area, panels.config),
        _ => {}
    }
}

/// Which §12.6 box a §13.5 sub-screen is drawn in, or [`Overlay::None`] for the
/// one that has none of its own.
fn as_overlay(sub: &Sub) -> Overlay {
    match sub {
        Sub::HighScores => Overlay::None,
        Sub::Controls => Overlay::Controls,
        Sub::Options { selected } => Overlay::Options {
            selected: *selected,
        },
    }
}

/// The six figures §12.6's game-over box lists.
const OVER_FIGURES: u32 = 6;
/// The rank, a blank and the field (§12.6).
const NAME_ROWS: u32 = 3;

/// Draw whatever is over the playing screen, if anything (§12.6).
pub fn draw(painter: &egui::Painter, layout: &Layout, state: &FrameState, panels: &Panels) {
    // §9.17: the countdown is the one overlay the board is meant to be read
    // through, so it is the one that does not dim it.
    if !matches!(state.overlay, Overlay::None | Overlay::Resuming { .. }) {
        painter.rect_filled(layout.well(), 0.0, SCRIM);
    }
    // §9.17's countdown is the one overlay that is not a box.
    if let Overlay::Resuming { count } = state.overlay {
        resuming(painter, layout, count);
        return;
    }
    let Some(area) = rect_of(layout, &state.overlay, panels) else {
        return;
    };
    match &state.overlay {
        Overlay::None | Overlay::Resuming { .. } => {}
        Overlay::Paused { selected } => paused(painter, layout, area, *selected),
        Overlay::Options { selected } => options(painter, layout, area, panels, *selected),
        Overlay::Controls => controls(painter, layout, area, panels.config),
        Overlay::GameOver => game_over(painter, layout, area, &state.view),
        Overlay::NameEntry { rank, name } => name_entry(painter, layout, area, *rank, name),
    }
}

/// §9.17's pause menu. The well under it has already been blanked.
fn paused(painter: &egui::Painter, layout: &Layout, area: egui::Rect, selected: usize) {
    frame(painter, layout, area, "PAUSED");
    let cell = layout.cell();
    for (index, choice) in PauseChoice::ALL.iter().enumerate() {
        let row = row_rect(layout, area, index);
        let lit = index == selected;
        if lit {
            painter.rect_filled(row, cell * 0.1, SELECTED);
            marker(painter, row, cell);
        }
        paint::text(
            painter,
            egui::pos2(row.left() + cell * 1.3, row.center().y),
            egui::Align2::LEFT_CENTER,
            choice.label(),
            Face::Label,
            cell * ROW_SIZE,
            if lit { paint::TEXT } else { paint::LABEL },
            row.width() - cell * 2.0,
        );
    }
    hint(painter, layout, area, "Up / Down and Enter");
}

/// §9.17's 3-2-1 countdown, over a board that is visible again.
///
/// A numeral rather than a box, because what the player is meant to be doing is
/// reading the well behind it: it is drawn over the well, large and part
/// transparent, and covers no row completely.
fn resuming(painter: &egui::Painter, layout: &Layout, count: u8) {
    let well = layout.well();
    painter.text(
        well.center(),
        egui::Align2::CENTER_CENTER,
        count.to_string(),
        egui::FontId::proportional(layout.cell() * COUNT_SIZE),
        paint::TEXT.gamma_multiply(0.55),
    );
}

/// §12.6's game-over box. Every figure is the view's, so the box cannot
/// disagree with the stats panel behind it (§11).
fn game_over(painter: &egui::Painter, layout: &Layout, area: egui::Rect, view: &GameView) {
    frame(painter, layout, area, "GAME OVER");
    let figures = [
        ("SCORE", thousands(view.score)),
        ("LEVEL", view.level.to_string()),
        ("LINES", view.lines.to_string()),
        ("TIME", clock(view.ticks)),
        ("PIECES", view.pieces.to_string()),
        ("PPS", pps(view.pieces, view.ticks)),
    ];
    for (index, (label, value)) in figures.into_iter().enumerate() {
        let row = row_rect(layout, area, index);
        pair(painter, layout, row, label, &value, paint::TEXT);
    }
    hint(painter, layout, area, "Press any key");
}

/// §12.6's name entry, shown only when the score qualifies for the top ten.
///
/// `rank` is one-based, as the box prints it. The field is `shell::menus`'s
/// buffer drawn as text with a caret, not a widget: see the module's head.
fn name_entry(painter: &egui::Painter, layout: &Layout, area: egui::Rect, rank: usize, name: &str) {
    frame(painter, layout, area, "NEW HIGH SCORE");
    let cell = layout.cell();
    paint::text(
        painter,
        egui::pos2(area.center().x, row_rect(layout, area, 0).center().y),
        egui::Align2::CENTER_CENTER,
        &format!("#{rank}"),
        Face::Figure,
        cell * ROW_SIZE,
        paint::LABEL,
        area.width() - cell,
    );
    // The field, with the caret drawn after it rather than typed into it: what
    // the player has entered so far is `NameEntry`'s string and nothing else.
    let field = row_rect(layout, area, 2);
    let typed = format!("Name: {name}");
    paint::text(
        painter,
        egui::pos2(field.left() + cell, field.center().y),
        egui::Align2::LEFT_CENTER,
        &typed,
        Face::Figure,
        cell * ROW_SIZE,
        paint::TEXT,
        field.width() - cell * 2.0,
    );
    let caret = egui::Rect::from_min_size(
        egui::pos2(
            field.left() + cell + text_width(painter, layout, &typed),
            field.center().y - cell * ROW_SIZE * 0.4,
        ),
        egui::vec2((cell * 0.1).max(1.0), cell * ROW_SIZE * 0.8),
    );
    painter.rect_filled(caret, 0.0, paint::TEXT);
    hint(painter, layout, area, "Enter confirms, Esc discards");
}

/// The §13.5 Options panel, over the paused playfield (§12.6).
fn options(
    painter: &egui::Painter,
    layout: &Layout,
    area: egui::Rect,
    panels: &Panels,
    selected: usize,
) {
    frame(painter, layout, area, "OPTIONS");
    let cell = layout.cell();
    for (index, setting) in panels.settings.iter().enumerate() {
        let row = row_rect(layout, area, index);
        let lit = index == selected;
        if lit {
            painter.rect_filled(row, cell * 0.1, SELECTED);
            marker(painter, row, cell);
        }
        pair(
            painter,
            layout,
            row,
            setting.label(),
            &setting.value(panels.config),
            if lit { paint::TEXT } else { paint::LABEL },
        );
    }
    hint(painter, layout, area, "Left / Right change, Esc saves");
}

/// §10.1's bindings, the box the pause menu's Controls item opens (§12.6).
///
/// The words and the order are `shell::menus::controls`', including §13.3's
/// rule that a binding whose setting is off is not listed at all; what is this
/// front-end's is the two columns. §8.2's input mode is not shown, as it is in
/// the terminal's box: this front-end has only the one path (§G2.2).
fn controls(painter: &egui::Painter, layout: &Layout, area: egui::Rect, config: &ConfigFile) {
    let rows = menus::controls(config);
    frame(painter, layout, area, "CONTROLS");
    for (index, (label, keys)) in rows.iter().enumerate() {
        let row = row_rect(layout, area, index);
        pair(painter, layout, row, label, keys, paint::TEXT);
    }
    hint(painter, layout, area, "Esc returns");
}

/// A box: its ground, its border and its title.
pub(crate) fn frame(painter: &egui::Painter, layout: &Layout, area: egui::Rect, title: &str) {
    let cell = layout.cell();
    let corner = cell * 0.2;
    painter.rect_filled(area, corner, BOX);
    painter.rect_stroke(
        area,
        corner,
        egui::Stroke::new((layout.cell_pixels() / 12).max(1) as f32 / 1.0, paint::WALL),
        egui::StrokeKind::Inside,
    );
    paint::text(
        painter,
        egui::pos2(area.center().x, area.top() + cell),
        egui::Align2::CENTER_CENTER,
        title,
        Face::Label,
        cell * TITLE_SIZE,
        paint::TEXT,
        area.width() - cell,
    );
}

/// Row `index` of a box's body: full width inside the margins, one cell tall,
/// under the title.
pub(crate) fn row_rect(layout: &Layout, area: egui::Rect, index: usize) -> egui::Rect {
    let cell = layout.cell();
    egui::Rect::from_min_size(
        egui::pos2(
            area.left() + cell * 0.5,
            area.top() + cell * (2.0 + index as f32),
        ),
        egui::vec2(area.width() - cell, cell),
    )
}

/// A label on the left and its value on the right, the width of one row.
pub(crate) fn pair(
    painter: &egui::Painter,
    layout: &Layout,
    row: egui::Rect,
    label: &str,
    value: &str,
    colour: egui::Color32,
) {
    let cell = layout.cell();
    let room = row.width() - cell * 2.5;
    paint::text(
        painter,
        egui::pos2(row.left() + cell, row.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        Face::Label,
        cell * ROW_SIZE,
        if colour == paint::TEXT {
            paint::LABEL
        } else {
            colour
        },
        room,
    );
    paint::text(
        painter,
        egui::pos2(row.right() - cell * 0.5, row.center().y),
        egui::Align2::RIGHT_CENTER,
        value,
        Face::Figure,
        cell * ROW_SIZE,
        colour,
        room,
    );
}

/// The line at the foot of a box saying which keys it answers to.
///
/// §10.1's overlay navigation is fixed and not rebindable, so the words are
/// the keys themselves rather than whatever the `[keys]` table says.
pub(crate) fn hint(painter: &egui::Painter, layout: &Layout, area: egui::Rect, words: &str) {
    let cell = layout.cell();
    paint::text(
        painter,
        egui::pos2(area.center().x, area.bottom() - cell * 0.8),
        egui::Align2::CENTER_CENTER,
        words,
        Face::Label,
        cell * HINT_SIZE,
        paint::FAINT,
        area.width() - cell,
    );
}

/// The cursor of a menu, as a triangle rather than a character.
///
/// §12.6 draws `▸`; a window has no guarantee that its fonts carry that glyph,
/// and a shape cannot go missing.
pub(crate) fn marker(painter: &egui::Painter, row: egui::Rect, cell: f32) {
    let (x, y) = (row.left() + cell * 0.45, row.center().y);
    let (w, h) = (cell * 0.22, cell * 0.28);
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(x - w / 2.0, y - h),
            egui::pos2(x + w, y),
            egui::pos2(x - w / 2.0, y + h),
        ],
        paint::TEXT,
        egui::Stroke::NONE,
    ));
}

/// How wide `text` is when drawn as a row's figure: what the caret is placed
/// after (§12.6).
fn text_width(painter: &egui::Painter, layout: &Layout, text: &str) -> f32 {
    painter
        .layout_no_wrap(
            text.to_owned(),
            egui::FontId::monospace(layout.cell() * ROW_SIZE),
            paint::TEXT,
        )
        .size()
        .x
}
