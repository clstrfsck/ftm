//! §G3: the playing screen's one unit, and where everything is in it.
//!
//! Everything on the screen is measured in **cells** — one mino of the well —
//! and the cell is derived from the viewport by one rule:
//!
//! ```text
//! cell = floor(min(width / LAYOUT_COLS, height / LAYOUT_ROWS))
//! ```
//!
//! taken in *physical pixels*, with the block centred on a whole pixel. So
//! every rect this module hands out — the well, each panel, each mino — is a
//! whole number of pixels at a whole-pixel position, at any window size and
//! any DPI, without fighting `egui`'s float coordinates; a resize scales the
//! screen rather than reflowing it, and what is left over is margin, exactly
//! as §12.1 has it for a terminal. The well keeps its 1:2 aspect because
//! nothing is ever stretched.
//!
//! The arithmetic is done in pixels and handed out in points, and it is pure:
//! a viewport and a scale factor go in, rectangles come out, and nothing here
//! paints or asks `egui` anything. That is what lets the tests below hold the
//! metric's promises without a window.

use crate::core::{VIEW_HEIGHT, VIEW_WIDTH};

/// The playing screen's width, in cells (§G3.2): a one-cell margin, the
/// six-cell hold-and-stats column, a gutter, the ten-cell well, a gutter, the
/// six-cell next column, a margin.
pub const LAYOUT_COLS: u32 = 1 + PANEL_COLS + 1 + WELL_COLS + 1 + PANEL_COLS + 1;
/// The playing screen's height, in cells (§G3.2): the well's open mouth, its
/// twenty rows, the two-row status line, a margin.
pub const LAYOUT_ROWS: u32 = TOP + WELL_ROWS + STATUS_ROWS + 1;

/// The smallest cell the playing screen is drawn at, in **points** (§G3.3).
///
/// Points rather than pixels because the question is legibility, and a pixel
/// on a high-density display is half the size it is on another: at this cell a
/// panel's label is about eight points high. Below it every screen is replaced
/// by [`Measure::TooSmall`]'s message and a game in progress is paused (§8.4).
pub const MIN_CELL: f32 = 14.0;

/// The cell a fresh window opens at, in points (§G3.3): big enough to read
/// across a room, small enough for a laptop's screen.
pub const INITIAL_CELL: f32 = 28.0;

/// The window's initial inner size, in points: the layout at [`INITIAL_CELL`].
pub const INITIAL_SIZE: [f32; 2] = [
    LAYOUT_COLS as f32 * INITIAL_CELL,
    LAYOUT_ROWS as f32 * INITIAL_CELL,
];

const WELL_COLS: u32 = VIEW_WIDTH as u32;
const WELL_ROWS: u32 = VIEW_HEIGHT as u32;
/// The hold, stats and next panels: four cells for the widest piece, and one
/// either side of it.
const PANEL_COLS: u32 = 6;
/// The row the well starts on. Row 0 is margin and, above the well, the mouth a
/// piece comes in through: the well has no lid (§12.4, §G4.1).
const TOP: u32 = 1;
const LEFT: u32 = 1;
const WELL: u32 = LEFT + PANEL_COLS + 1;
const RIGHT: u32 = WELL + WELL_COLS + 1;
const STATUS: u32 = TOP + WELL_ROWS;
const STATUS_ROWS: u32 = 2;
/// A panel's label row, one two-row slot and a row of padding.
const HOLD_ROWS: u32 = 1 + SLOT_ROWS + 1;
/// Six figures at two and a half rows each; the stats panel's foot is level
/// with the well's floor when the hold panel is above it.
const STATS_ROWS: u32 = WELL_ROWS - HOLD_ROWS - 1;
/// A hold or preview slot: two rows, enough for any piece lying `North`.
const SLOT_ROWS: u32 = 2;
/// A slot and the row between it and the next.
const SLOT_PITCH: u32 = SLOT_ROWS + 1;
/// As many preview slots as the next panel has room for beside the well, which
/// is exactly the `preview_count` range's top (§6.3), so every slot fits.
pub const MAX_SLOTS: usize = ((WELL_ROWS - 1) / SLOT_PITCH) as usize;

/// What the viewport can hold (§G3.3, `FRONTEND.md` F6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Measure {
    /// The playing screen, at the cell the viewport allows.
    Fits(Layout),
    /// Below the minimum: what is needed and what there is, in whole points,
    /// for the message that replaces the screen.
    TooSmall { need: [u32; 2], have: [u32; 2] },
}

impl Measure {
    /// Measure `area`, in points, at `ppp` physical pixels to the point.
    ///
    /// Never panics, whatever it is given: a zero-area viewport — a minimised
    /// window, a canvas squeezed to nothing — or a nonsensical scale is simply
    /// too small (F6).
    pub fn of(area: egui::Rect, ppp: f32) -> Self {
        let ppp = if ppp.is_finite() && ppp > 0.0 {
            ppp
        } else {
            1.0
        };
        // The viewport's physical size is a whole number of pixels, so it is
        // rounded rather than floored: a float a hair under 468 is 468.
        let width = pixels(area.width(), ppp);
        let height = pixels(area.height(), ppp);
        let cell = (width / LAYOUT_COLS).min(height / LAYOUT_ROWS);
        let least = (MIN_CELL * ppp).ceil().max(1.0) as u32;
        if cell < least {
            let points =
                |cells: u32| (f64::from(cells) * f64::from(least) / f64::from(ppp)).ceil() as u32;
            return Measure::TooSmall {
                need: [points(LAYOUT_COLS), points(LAYOUT_ROWS)],
                have: [
                    area.width().max(0.0).round() as u32,
                    area.height().max(0.0).round() as u32,
                ],
            };
        }
        let origin = [
            (area.min.x * ppp).round() as i64 + i64::from((width - cell * LAYOUT_COLS) / 2),
            (area.min.y * ppp).round() as i64 + i64::from((height - cell * LAYOUT_ROWS) / 2),
        ];
        Measure::Fits(Layout { cell, ppp, origin })
    }

    /// Whether the playing screen can be drawn: [`Round::viewport`]'s argument
    /// (F6).
    ///
    /// [`Round::viewport`]: crate::shell::round::Round::viewport
    pub fn fits(&self) -> bool {
        matches!(self, Measure::Fits(_))
    }
}

/// A length in points as whole physical pixels, never negative.
fn pixels(points: f32, ppp: f32) -> u32 {
    (points * ppp).round().max(0.0) as u32
}

/// The playing screen, placed (§G3.2).
///
/// Held in pixels and handed out in points, so that every rect it returns is
/// whole pixels at a whole-pixel position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// The cell, in physical pixels.
    cell: u32,
    /// Physical pixels to the point.
    ppp: f32,
    /// The block's top-left corner, in physical pixels.
    origin: [i64; 2],
}

impl Layout {
    /// The cell, in points.
    pub fn cell(&self) -> f32 {
        self.cell as f32 / self.ppp
    }

    /// The cell, in physical pixels. What a length that must stay crisp — a
    /// gutter, a wall — is worked out from.
    pub fn cell_pixels(&self) -> u32 {
        self.cell
    }

    /// The whole block, margins included.
    pub fn block(&self) -> egui::Rect {
        self.cells(0, 0, LAYOUT_COLS, LAYOUT_ROWS)
    }

    /// The well's interior: ten cells by twenty, the visible field (§12.7).
    pub fn well(&self) -> egui::Rect {
        self.cells(WELL, TOP, WELL_COLS, WELL_ROWS)
    }

    /// One cell of the well, at visible-field coordinates.
    pub fn field_cell(&self, col: u8, row: u8) -> egui::Rect {
        self.cells(WELL + u32::from(col), TOP + u32::from(row), 1, 1)
    }

    /// The hold panel, when there is one (§G4.2).
    pub fn hold(&self) -> egui::Rect {
        self.cells(LEFT, TOP, PANEL_COLS, HOLD_ROWS)
    }

    /// The stats panel: under the hold panel, or at the top of the column when
    /// hold is off and the hold panel is absent (§12.4, §G4.2).
    pub fn stats(&self, hold_enabled: bool) -> egui::Rect {
        let top = if hold_enabled {
            TOP + HOLD_ROWS + 1
        } else {
            TOP
        };
        self.cells(LEFT, top, PANEL_COLS, STATS_ROWS)
    }

    /// The next panel, sized to its slots: a label row, `slots` two-row slots a
    /// row apart, and a row of padding (§G4.3). More than [`MAX_SLOTS`] are
    /// not drawn, which the `preview_count` range makes unreachable.
    pub fn next(&self, slots: usize) -> egui::Rect {
        let slots = slots.min(MAX_SLOTS) as u32;
        let rows = 1 + (SLOT_PITCH * slots).saturating_sub(1) + 1;
        self.cells(RIGHT, TOP, PANEL_COLS, rows)
    }

    /// Slot `index` of the hold or next panel whose top-left is `panel`: the
    /// full width of the panel, two rows, under the label row.
    pub fn slot(&self, panel: egui::Rect, index: usize) -> egui::Rect {
        let row = 1 + SLOT_PITCH * index as u32;
        let [x, y] = self.pixels_of(panel.min);
        self.pixels(
            x,
            y + i64::from(row * self.cell),
            PANEL_COLS * self.cell,
            SLOT_ROWS * self.cell,
        )
    }

    /// The status line, the whole block wide, under the well's floor.
    pub fn status(&self) -> egui::Rect {
        self.cells(0, STATUS, LAYOUT_COLS, STATUS_ROWS)
    }

    /// A piece of `cols` x `rows` occupied cells, centred in `slot`, and the
    /// rect of its cell `(x, y)`.
    ///
    /// The one place on the screen a position is not a whole number of cells:
    /// a three-wide piece in a six-wide slot, or an `I` lying in a two-row one,
    /// sits half a cell in. It is still a whole number of pixels — the half is
    /// rounded down — which is the promise that matters for crispness.
    pub fn in_slot(
        &self,
        slot: egui::Rect,
        (cols, rows): (u32, u32),
        (x, y): (u32, u32),
    ) -> egui::Rect {
        let [left, top] = self.pixels_of(slot.min);
        let spare = |room: f32, used: u32| {
            let room = pixels(room, self.ppp);
            i64::from(room.saturating_sub(used * self.cell) / 2)
        };
        self.pixels(
            left + spare(slot.width(), cols) + i64::from(x * self.cell),
            top + spare(slot.height(), rows) + i64::from(y * self.cell),
            self.cell,
            self.cell,
        )
    }

    /// `rect` shrunk by `inset` physical pixels on the left and top and
    /// `inset` rounded up on the right and bottom — a mino's gutter, kept on
    /// whole pixels.
    pub fn inset(&self, rect: egui::Rect, total: u32) -> egui::Rect {
        let [x, y] = self.pixels_of(rect.min);
        let [w, h] = [
            pixels(rect.width(), self.ppp),
            pixels(rect.height(), self.ppp),
        ];
        self.pixels(
            x + i64::from(total / 2),
            y + i64::from(total / 2),
            w.saturating_sub(total),
            h.saturating_sub(total),
        )
    }

    /// A band `thickness` physical pixels deep around `rect`'s left, right and
    /// bottom edges, as three rects: the well's walls and floor (§G4.1).
    pub fn walls(&self, rect: egui::Rect, thickness: u32) -> [egui::Rect; 3] {
        let [x, y] = self.pixels_of(rect.min);
        let [w, h] = [
            pixels(rect.width(), self.ppp),
            pixels(rect.height(), self.ppp),
        ];
        let t = i64::from(thickness);
        [
            self.pixels(x - t, y, thickness, h),
            self.pixels(x + i64::from(w), y, thickness, h),
            self.pixels(x - t, y + i64::from(h), w + 2 * thickness, thickness),
        ]
    }

    /// Whole cells of the layout's grid, as a rect in points.
    fn cells(&self, col: u32, row: u32, cols: u32, rows: u32) -> egui::Rect {
        self.pixels(
            self.origin[0] + i64::from(col * self.cell),
            self.origin[1] + i64::from(row * self.cell),
            cols * self.cell,
            rows * self.cell,
        )
    }

    /// A rect of whole physical pixels, as points.
    fn pixels(&self, x: i64, y: i64, width: u32, height: u32) -> egui::Rect {
        let point = |px: i64| px as f32 / self.ppp;
        egui::Rect::from_min_max(
            egui::pos2(point(x), point(y)),
            egui::pos2(point(x + i64::from(width)), point(y + i64::from(height))),
        )
    }

    /// A point this module handed out, back in whole physical pixels.
    fn pixels_of(&self, point: egui::Pos2) -> [i64; 2] {
        [
            (point.x * self.ppp).round() as i64,
            (point.y * self.ppp).round() as i64,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: f32, height: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height))
    }

    fn layout(width: f32, height: f32, ppp: f32) -> Layout {
        match Measure::of(area(width, height), ppp) {
            Measure::Fits(layout) => layout,
            small => panic!("{width} x {height} at {ppp}: {small:?}"),
        }
    }

    /// Whether a length in points is a whole number of pixels at `ppp`.
    fn whole(points: f32, ppp: f32) -> bool {
        let px = points * ppp;
        (px - px.round()).abs() < 1e-3
    }

    fn crisp(rect: egui::Rect, ppp: f32) -> bool {
        [rect.min.x, rect.min.y, rect.max.x, rect.max.y]
            .into_iter()
            .all(|v| whole(v, ppp))
    }

    #[test]
    fn the_layout_is_twenty_six_cells_by_twenty_four() {
        // §G3.2's arithmetic, pinned: a margin, the left column, a gutter, the
        // well, a gutter, the right column, a margin — and the mouth, the
        // well, two status rows and a margin.
        assert_eq!(LAYOUT_COLS, 1 + 6 + 1 + 10 + 1 + 6 + 1);
        assert_eq!(LAYOUT_ROWS, 1 + 20 + 2 + 1);
        assert_eq!(MAX_SLOTS, 6, "the top of `preview_count`'s range fits");
        assert_eq!(INITIAL_SIZE, [728.0, 672.0]);
    }

    #[test]
    fn the_cell_is_the_largest_whole_number_of_pixels_that_fits() {
        // Width-bound, height-bound, and a window with room to spare in both,
        // whose remainder becomes margin rather than stretch.
        assert_eq!(layout(26.0 * 20.0, 2000.0, 1.0).cell_pixels(), 20);
        assert_eq!(layout(2000.0, 24.0 * 20.0, 1.0).cell_pixels(), 20);
        assert_eq!(
            layout(26.0 * 20.0 + 25.0, 24.0 * 20.0 + 23.0, 1.0).cell_pixels(),
            20
        );
        // At twice the density the same window has twice the pixels, and the
        // cell in points is unchanged.
        let retina = layout(26.0 * 20.0, 24.0 * 20.0, 2.0);
        assert_eq!(retina.cell_pixels(), 40);
        assert_eq!(retina.cell(), 20.0);
    }

    #[test]
    fn every_rect_is_whole_pixels_at_any_density() {
        // The point of the metric: the grid stays crisp at any size and any
        // DPI, including the awkward fractional ones a desktop is set to.
        for ppp in [1.0, 1.25, 1.5, 1.75, 2.0, 3.0] {
            for (width, height) in [(728.0, 672.0), (1001.0, 777.0), (513.3, 900.7)] {
                let area =
                    egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(width, height));
                let Measure::Fits(layout) = Measure::of(area, ppp) else {
                    continue;
                };
                let well = layout.well();
                let mut rects = vec![layout.block(), well, layout.hold(), layout.status()];
                rects.push(layout.stats(true));
                rects.push(layout.stats(false));
                for slots in 1..=6 {
                    let next = layout.next(slots);
                    rects.push(next);
                    rects.push(layout.slot(next, slots - 1));
                    rects.push(layout.in_slot(layout.slot(next, 0), (3, 2), (2, 1)));
                    rects.push(layout.in_slot(layout.slot(next, 0), (4, 1), (0, 0)));
                }
                rects.push(layout.field_cell(9, 19));
                rects.push(layout.inset(layout.field_cell(0, 0), 3));
                rects.extend(layout.walls(well, 2));
                for rect in rects {
                    assert!(crisp(rect, ppp), "{rect:?} at {ppp}x in {width} x {height}");
                }
            }
        }
    }

    #[test]
    fn the_block_is_centred_and_the_well_keeps_its_shape() {
        let layout = layout(1000.0, 500.0, 1.0);
        let cell = layout.cell();
        let block = layout.block();
        assert_eq!(cell, 20.0, "height-bound: 500 / 24");
        assert_eq!(block.size(), egui::vec2(26.0 * cell, 24.0 * cell));
        assert!((block.center().x - 500.0).abs() <= 0.5, "{block:?}");
        let well = layout.well();
        assert_eq!(well.size(), egui::vec2(10.0 * cell, 20.0 * cell));
        assert_eq!(well.min.y - block.min.y, cell, "under the open mouth");
        assert!(
            (well.center().x - block.center().x).abs() < f32::EPSILON,
            "the well is the block's middle, so the status line centres on both"
        );
        assert_eq!(layout.field_cell(0, 0).min, well.min);
        assert_eq!(layout.field_cell(9, 19).max, well.max);
    }

    #[test]
    fn the_panels_sit_beside_the_well_and_inside_the_block() {
        let layout = layout(728.0, 672.0, 1.0);
        let (block, well) = (layout.block(), layout.well());
        let hold = layout.hold();
        let stats = layout.stats(true);
        assert_eq!(hold.min.y, well.min.y, "level with the mouth");
        assert_eq!(stats.max.y, well.max.y, "level with the floor");
        assert!(hold.max.y < stats.min.y, "a gutter between them");
        assert!(hold.max.x < well.min.x && layout.next(6).min.x > well.max.x);
        // Without hold, the stats take its place at the top (§12.4).
        assert_eq!(layout.stats(false).min.y, well.min.y);
        for slots in 1..=6 {
            let next = layout.next(slots);
            assert!(block.contains_rect(next), "{slots} slots");
            assert!(
                next.max.y <= well.max.y,
                "{slots} slots fit beside the well"
            );
            let last = layout.slot(next, slots - 1);
            assert!(next.contains_rect(last), "{slots}: {last:?} in {next:?}");
        }
        // One preview slot is exactly the hold panel's shape.
        assert_eq!(layout.next(1).size(), hold.size());
        assert!(block.contains_rect(layout.status()));
        assert_eq!(layout.status().min.y, well.max.y);
    }

    #[test]
    fn a_piece_is_centred_in_its_slot() {
        let layout = layout(728.0, 672.0, 1.0);
        let slot = layout.slot(layout.hold(), 0);
        // `O` is two by two in a six by two slot: two cells in, flush.
        let o = layout.in_slot(slot, (2, 2), (0, 0));
        assert_eq!(o.min - slot.min, egui::vec2(2.0 * 28.0, 0.0));
        // `T` is three wide: a cell and a half in, rounded to the pixel.
        let t = layout.in_slot(slot, (3, 2), (0, 0));
        assert_eq!(t.min - slot.min, egui::vec2(42.0, 0.0));
        // `I` lies in one row of two: half a cell down, and a cell in.
        let i = layout.in_slot(slot, (4, 1), (0, 0));
        assert_eq!(i.min - slot.min, egui::vec2(28.0, 14.0));
    }

    #[test]
    fn below_the_minimum_the_viewport_is_too_small_and_says_by_how_much() {
        // §G3.3: the minimum is a cell of fourteen points, which is a window of
        // 364 x 336 at any density.
        let need = [26 * 14, 24 * 14];
        assert!(Measure::of(area(364.0, 336.0), 1.0).fits());
        assert!(Measure::of(area(364.0, 336.0), 2.0).fits());
        assert_eq!(
            Measure::of(area(363.0, 400.0), 1.0),
            Measure::TooSmall {
                need,
                have: [363, 400]
            },
        );
        assert_eq!(
            Measure::of(area(400.0, 335.0), 2.0),
            Measure::TooSmall {
                need,
                have: [400, 335]
            },
        );
        // At a fractional density the cell must still be a whole number of
        // pixels, so what is needed is a little more — and the message says
        // exactly how much, rather than a size at which it still would not fit.
        let Measure::TooSmall { need, .. } = Measure::of(area(10.0, 10.0), 1.25) else {
            panic!("ten points is not a screen");
        };
        assert_eq!(need, [375, 346]);
        assert!(Measure::of(area(375.0, 346.0), 1.25).fits());
        // A viewport is whole pixels, so at 1.25 the widths either side of the
        // threshold are 467 and 468 of them.
        assert!(Measure::of(area(468.0 / 1.25, 346.0), 1.25).fits());
        assert!(!Measure::of(area(467.0 / 1.25, 346.0), 1.25).fits());
    }

    #[test]
    fn nothing_is_a_viewport_that_does_not_fit_rather_than_a_panic() {
        // F6: rendering never panics at any size. A minimised window reports
        // nothing at all, and a canvas can be squeezed to less than that.
        for rect in [
            area(0.0, 0.0),
            area(1.0, 1.0),
            egui::Rect::NOTHING,
            egui::Rect::from_min_max(egui::pos2(10.0, 10.0), egui::pos2(0.0, 0.0)),
        ] {
            for ppp in [1.0, 2.0, 0.0, f32::NAN, f32::INFINITY] {
                assert!(!Measure::of(rect, ppp).fits(), "{rect:?} at {ppp}");
            }
        }
        // And an enormous one is just a big cell.
        assert!(Measure::of(area(100_000.0, 100_000.0), 2.0).fits());
    }
}
